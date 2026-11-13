//! El proxy trampa de la comprobación local: escucha en `127.0.0.1`, en un puerto que elige el
//! sistema, y no reenvía nada. De cada conexión lee la primera línea, anota el destino que se le
//! pidió y corta.
//!
//! Las variables de proxy de `[comprobacion_local]` apuntan aquí, así que lo que el código del
//! modelo intente descargar respetando el proxy queda anotado sin salir del equipo.

use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::error::{Error, Resultado};

/// Cuánto se espera la primera línea de una conexión. Quien usa un proxy la manda nada más
/// conectar; si no llega, se anota la conexión sin destino.
const ESPERA_DE_LA_PRIMERA_LINEA: Duration = Duration::from_secs(2);
/// Lo que se anota cuando la conexión no dijo adónde quería ir.
const DESTINO_DESCONOCIDO: &str = "un destino que no dijo (conexión al proxy sin petición)";

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

/// La trampa abierta. Al soltarla, el hilo termina.
pub struct Trampa {
    puerto: u16,
    anotados: Anotados,
    parar: Arc<AtomicBool>,
    hilo: Option<JoinHandle<()>>,
}

impl Trampa {
    pub fn abrir() -> Resultado<Self> {
        let escucha = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|escucha| escucha.local_addr().map(|dir| (escucha, dir.port())))
            .map_err(|causa| {
                Error::Banco(format!(
                    "no se pudo abrir el proxy trampa en 127.0.0.1: {causa}"
                ))
            })?;
        let (escucha, puerto) = escucha;
        let anotados = Anotados::default();
        let parar = Arc::new(AtomicBool::new(false));
        let hilo = {
            let anotados = Arc::clone(&anotados);
            let parar = Arc::clone(&parar);
            std::thread::spawn(move || escuchar(escucha, &anotados, &parar))
        };
        Ok(Self {
            puerto,
            anotados,
            parar,
            hilo: Some(hilo),
        })
    }

    /// El puerto donde escucha, para las variables de proxy.
    pub fn puerto(&self) -> u16 {
        self.puerto
    }

    /// Lo anotado desde la última vez que se preguntó.
    pub fn recoger(&self) -> Vec<String> {
        match self.anotados.lock() {
            Ok(mut anotados) => std::mem::take(&mut *anotados),
            // Un hilo que entró en pánico con el cerrojo puesto deja la lista como estaba, que
            // sigue siendo válida.
            Err(envenenado) => std::mem::take(&mut *envenenado.into_inner()),
        }
    }
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

fn escuchar(escucha: TcpListener, anotados: &Anotados, parar: &AtomicBool) {
    for conexion in escucha.incoming() {
        if parar.load(Ordering::SeqCst) {
            return;
        }
        let Ok(conexion) = conexion else { continue };
        let destino = leer_destino(conexion).unwrap_or_else(|| DESTINO_DESCONOCIDO.to_string());
        if let Ok(mut lista) = anotados.lock() {
            lista.push(destino);
        }
    }
}

/// Lee la primera línea y cierra la conexión al soltarla.
fn leer_destino(conexion: TcpStream) -> Option<String> {
    conexion
        .set_read_timeout(Some(ESPERA_DE_LA_PRIMERA_LINEA))
        .ok()?;
    let mut linea = String::new();
    BufReader::new(conexion).read_line(&mut linea).ok()?;
    destino_de_la_peticion(&linea)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Write;
    use std::time::Instant;

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

    /// `Drop` espera al hilo: si `accept` no se despertara, esta prueba no terminaría.
    #[test]
    fn soltar_la_trampa_no_se_queda_esperando_al_hilo() {
        let trampa = Trampa::abrir().unwrap();
        let inicio = Instant::now();

        drop(trampa);

        assert!(inicio.elapsed() < ESPERA_DE_LA_PRIMERA_LINEA * 2);
    }
}
