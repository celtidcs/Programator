//! El proxy trampa de la comprobación local: escucha en `127.0.0.1`, en un puerto que elige el
//! sistema, y no reenvía nada. De cada conexión lee la primera línea, anota el destino que se le
//! pidió y corta.
//!
//! Las variables de proxy de `[comprobacion_local]` apuntan aquí, así que lo que el código del
//! modelo intente descargar respetando el proxy queda anotado sin salir del equipo.
//!
//! El hilo atiende las conexiones de una en una, en el orden en que llegan. Para recoger, la
//! trampa se manda a sí misma una conexión con su marca, que entra en la cola detrás de todas las
//! anteriores, y espera a que el hilo la lea: así cuenta también lo que ya había conectado y aún no
//! había dicho adónde iba, o seguía en la cola sin aceptar.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{Error, Resultado};

/// Cuánto se espera la primera línea de una conexión. Quien usa un proxy la manda nada más
/// conectar; si no llega, se anota la conexión sin destino.
const ESPERA_DE_LA_PRIMERA_LINEA: Duration = Duration::from_secs(2);
/// Lo que se anota cuando la conexión no dijo adónde quería ir.
const DESTINO_DESCONOCIDO: &str = "un destino que no dijo (conexión al proxy sin petición)";
/// Cuánto espera una recogida a que el hilo llegue a su marca. Cada conexión muda delante cuesta
/// [`ESPERA_DE_LA_PRIMERA_LINEA`]: con quince, se pasa del tope y lo que falte se recoge la vez
/// siguiente, en vez de colgar la comprobación.
const ESPERA_MAXIMA_DE_LA_RECOGIDA: Duration = Duration::from_secs(30);
/// El principio de la marca con que la trampa se pide a sí misma una recogida. Lleva detrás un
/// número de esta trampa, para que el código que se comprueba no la pueda imitar.
const PREFIJO_DE_LA_MARCA: &str = "PROGRAMATOR-RECOGER-";

/// El destino de la primera línea de una petición a un proxy: `CONNECT host:443 HTTP/1.1` da
/// `host:443`, y `GET http://host/ruta HTTP/1.1`, `host`. `None` si la línea no es una petición.
pub fn destino_de_la_peticion(linea: &str) -> Option<String> {
    let mut partes = linea.split_whitespace();
    let metodo = partes.next()?;
    let objetivo = partes.next()?;
    if metodo.eq_ignore_ascii_case("CONNECT") {
        return Some(objetivo.to_string());
    }
    let sin_esquema = objetivo
        .split_once("://")
        .map_or(objetivo, |(_, resto)| resto);
    let anfitrion = sin_esquema.split('/').next().unwrap_or_default();
    (!anfitrion.is_empty()).then(|| anfitrion.to_string())
}

type Anotados = Arc<Mutex<Vec<String>>>;

/// Lo que comparten la trampa y su hilo.
#[derive(Default)]
struct Compartido {
    anotados: Anotados,
    /// Cuántas marcas de recogida ha leído el hilo.
    recogidas: Mutex<u64>,
    recogida_leida: Condvar,
}

/// La trampa abierta. Al soltarla, el hilo termina.
pub struct Trampa {
    puerto: u16,
    marca: String,
    compartido: Arc<Compartido>,
    /// Cuántas marcas de recogida se han mandado.
    pedidas: AtomicU64,
    parar: Arc<AtomicBool>,
    hilo: Option<JoinHandle<()>>,
}

impl Trampa {
    pub fn abrir() -> Resultado<Self> {
        let escucha = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|escucha| {
                escucha
                    .local_addr()
                    .map(|carpeta| (escucha, carpeta.port()))
            })
            .map_err(|causa| {
                Error::Banco(crate::banco::mensajes::proxy_trampa_sin_abrir(
                    Ipv4Addr::LOCALHOST,
                    &causa,
                ))
            })?;
        let (escucha, puerto) = escucha;
        let marca = marca_de_recogida(puerto);
        let compartido = Arc::new(Compartido::default());
        let parar = Arc::new(AtomicBool::new(false));
        let hilo = {
            let compartido = Arc::clone(&compartido);
            let parar = Arc::clone(&parar);
            let marca = marca.clone();
            std::thread::spawn(move || escuchar(escucha, &compartido, &marca, &parar))
        };
        Ok(Self {
            puerto,
            marca,
            compartido,
            pedidas: AtomicU64::new(0),
            parar,
            hilo: Some(hilo),
        })
    }

    /// El puerto donde escucha, para las variables de proxy.
    pub fn puerto(&self) -> u16 {
        self.puerto
    }

    /// Lo anotado desde la última vez que se preguntó, con todo lo que había conectado antes de
    /// preguntar. Si la marca no se puede mandar, da lo anotado hasta ahora.
    pub fn recoger(&self) -> Vec<String> {
        if self.pedir_la_recogida().is_ok() {
            let pedidas = self.pedidas.fetch_add(1, Ordering::SeqCst) + 1;
            self.esperar_la_recogida(pedidas);
        }
        anotados_hasta_ahora(&self.compartido.anotados)
    }

    fn pedir_la_recogida(&self) -> std::io::Result<()> {
        let mut conexion = TcpStream::connect((Ipv4Addr::LOCALHOST, self.puerto))?;
        conexion.write_all(format!("{}\n", self.marca).as_bytes())
    }

    fn esperar_la_recogida(&self, pedidas: u64) {
        let leidas = self
            .compartido
            .recogidas
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = self.compartido.recogida_leida.wait_timeout_while(
            leidas,
            ESPERA_MAXIMA_DE_LA_RECOGIDA,
            |leidas| *leidas < pedidas,
        );
    }
}

/// La marca de recogida de la trampa que escucha en `puerto`: el prefijo, el puerto y el instante.
fn marca_de_recogida(puerto: u16) -> String {
    let instante = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|desde| desde.as_nanos())
        .unwrap_or_default();
    format!(
        "{PREFIJO_DE_LA_MARCA}{puerto}-{instante}-{}",
        std::process::id()
    )
}

impl Drop for Trampa {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::SeqCst);
        // `accept` no se despierta solo: una conexión propia lo saca de la espera.
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.puerto));
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

fn escuchar(escucha: TcpListener, compartido: &Compartido, marca: &str, parar: &AtomicBool) {
    for conexion in escucha.incoming() {
        if parar.load(Ordering::SeqCst) {
            return;
        }
        let Ok(conexion) = conexion else { continue };
        let linea = leer_la_primera_linea(conexion);
        if linea.as_deref().map(str::trim_end) == Some(marca) {
            let mut leidas = compartido
                .recogidas
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *leidas += 1;
            compartido.recogida_leida.notify_all();
            continue;
        }
        let destino = linea
            .as_deref()
            .and_then(destino_de_la_peticion)
            .unwrap_or_else(|| DESTINO_DESCONOCIDO.to_string());
        anotar(&compartido.anotados, destino);
    }
}

/// Saca lo anotado y deja la lista vacía.
fn anotados_hasta_ahora(anotados: &Anotados) -> Vec<String> {
    match anotados.lock() {
        Ok(mut anotados) => std::mem::take(&mut *anotados),
        // Un hilo que entró en pánico con el cerrojo puesto deja la lista como estaba, que
        // sigue siendo válida.
        Err(envenenado) => std::mem::take(&mut *envenenado.into_inner()),
    }
}

/// Anota un destino. Con el cerrojo envenenado, la lista sigue siendo válida (como en
/// [`anotados_hasta_ahora`]): perder un intento de salir a la red ocultaría una trampa.
fn anotar(anotados: &Anotados, destino: String) {
    anotados
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(destino);
}

/// Lee la primera línea y cierra la conexión al soltarla.
fn leer_la_primera_linea(conexion: TcpStream) -> Option<String> {
    conexion
        .set_read_timeout(Some(ESPERA_DE_LA_PRIMERA_LINEA))
        .ok()?;
    let mut linea = String::new();
    BufReader::new(conexion).read_line(&mut linea).ok()?;
    Some(linea)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Write;
    use std::time::Instant;

    /// Con el cerrojo envenenado, lo anotado no se pierde en silencio.
    #[test]
    fn un_cerrojo_envenenado_no_pierde_lo_anotado() {
        let anotados: Anotados = Arc::new(Mutex::new(Vec::new()));
        let otro = Arc::clone(&anotados);
        let _ = std::thread::spawn(move || {
            let _cerrojo = otro.lock().unwrap();
            panic!("envenena el cerrojo");
        })
        .join();
        assert!(anotados.is_poisoned());

        anotar(&anotados, "pypi.org:443".to_string());

        assert_eq!(anotados_hasta_ahora(&anotados), vec!["pypi.org:443"]);
    }

    #[test]
    fn el_destino_sale_de_connect_y_de_una_url_absoluta() {
        assert_eq!(
            destino_de_la_peticion("CONNECT pypi.org:443 HTTP/1.1\r\n").as_deref(),
            Some("pypi.org:443")
        );
        assert_eq!(
            destino_de_la_peticion("GET http://ejemplo.com:8080/ruta?x=1 HTTP/1.1").as_deref(),
            Some("ejemplo.com:8080")
        );
        assert_eq!(destino_de_la_peticion("").as_deref(), None);
        assert_eq!(destino_de_la_peticion("hola").as_deref(), None);
    }

    /// Espera a que el hilo anote algo, con un tope para no colgar la suite.
    fn esperar_anotados(trampa: &Trampa) -> Vec<String> {
        let limite = Instant::now() + Duration::from_secs(10);
        loop {
            let anotados = trampa.recoger();
            if !anotados.is_empty() || Instant::now() > limite {
                return anotados;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn una_peticion_a_la_trampa_queda_anotada_y_se_recoge_una_vez() {
        let trampa = Trampa::abrir().unwrap();
        let mut conexion = TcpStream::connect((Ipv4Addr::LOCALHOST, trampa.puerto())).unwrap();
        conexion
            .write_all(b"CONNECT files.pythonhosted.org:443 HTTP/1.1\r\n\r\n")
            .unwrap();

        assert_eq!(esperar_anotados(&trampa), ["files.pythonhosted.org:443"]);
        assert!(trampa.recoger().is_empty());
    }

    /// Una conexión hecha antes de recoger cuenta aunque su petición llegue después: la orden ya
    /// terminó, pero la trampa aún no había leído su primera línea.
    #[test]
    fn lo_que_ya_conecto_se_recoge_aunque_pida_tarde() {
        let trampa = Trampa::abrir().unwrap();
        let mut conexion = TcpStream::connect((Ipv4Addr::LOCALHOST, trampa.puerto())).unwrap();
        let cliente = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            conexion.write_all(b"CONNECT tarde.example:443 HTTP/1.1\r\n\r\n")
        });

        let recogido = trampa.recoger();

        cliente.join().unwrap().unwrap();
        assert_eq!(recogido, ["tarde.example:443"]);
    }

    /// Varias conexiones en la cola, sin aceptar todavía, se recogen todas.
    #[test]
    fn las_conexiones_en_cola_se_recogen_todas() {
        let trampa = Trampa::abrir().unwrap();
        let conexiones: Vec<TcpStream> = (0..3)
            .map(|n| {
                let mut conexion =
                    TcpStream::connect((Ipv4Addr::LOCALHOST, trampa.puerto())).unwrap();
                conexion
                    .write_all(format!("CONNECT n{n}.example:443 HTTP/1.1\r\n\r\n").as_bytes())
                    .unwrap();
                conexion
            })
            .collect();

        let mut recogido = trampa.recoger();
        recogido.sort();

        assert_eq!(
            recogido,
            ["n0.example:443", "n1.example:443", "n2.example:443"]
        );
        drop(conexiones);
    }

    /// Sin nada que recoger, la lista sale vacía: la propia recogida no se anota.
    #[test]
    fn recoger_sin_conexiones_no_anota_nada() {
        let trampa = Trampa::abrir().unwrap();

        assert!(trampa.recoger().is_empty());
        assert!(trampa.recoger().is_empty());
    }

    /// Recoger espera a su marca, no al tope: sin nada delante, vuelve enseguida.
    #[test]
    fn recoger_vuelve_en_cuanto_el_hilo_lee_su_marca() {
        let trampa = Trampa::abrir().unwrap();
        let inicio = Instant::now();

        trampa.recoger();
        trampa.recoger();

        assert!(inicio.elapsed() < ESPERA_DE_LA_PRIMERA_LINEA);
    }

    /// `Drop` espera al hilo: si `accept` no se despertara, esta prueba no terminaría.
    #[test]
    fn soltar_la_trampa_no_se_queda_esperando_al_hilo() {
        let trampa = Trampa::abrir().unwrap();
        let inicio = Instant::now();

        drop(trampa);

        assert!(inicio.elapsed() < ESPERA_DE_LA_PRIMERA_LINEA * 2);
    }
}
