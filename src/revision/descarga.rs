//! La descarga de un modelo: a `<fichero>.parcial`, reanudable, y comprobada con su huella antes
//! de quedarse con su nombre (especificación, §4, puntos 2 a 5).
//!
//! De dónde salen los bytes es un [`Fuente`]: HTTP con `ureq` en producción y una de mentira en
//! las pruebas. El espacio libre llega como una función, para que las pruebas digan cuánto hay.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::catalogo::AGENTE;
use super::mensajes;
use crate::error::{Error, Resultado};
use crate::ficheros;

/// Lo que se añade al nombre del fichero mientras se descarga.
const SUFIJO_PARCIAL: &str = ".parcial";
/// Cuánto se lee de una vez: 1 MiB, como al calcular una huella.
const TAMANO_DEL_TROZO: usize = 1024 * 1024;
/// La respuesta HTTP de un trozo pedido con `Range`.
const ESTADO_CONTENIDO_PARCIAL: u16 = 206;

/// De dónde se descarga.
pub trait Fuente {
    /// El contenido desde el byte `desde`. Si la fuente no sabe empezar por ahí, da el contenido
    /// entero y lo dice con `reanuda = false`.
    fn abrir(&self, desde: u64) -> Resultado<Apertura>;
}

pub struct Apertura {
    /// Si lo que llega empieza en el byte pedido o en el primero.
    pub reanuda: bool,
    pub lector: Box<dyn Read>,
}

/// Lo que pasa durante la descarga, para decirlo según ocurre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aviso {
    /// Había un `.parcial` y el servidor acepta seguir desde ahí.
    Reanuda {
        desde: u64,
    },
    /// Había un `.parcial`, pero el servidor no acepta `Range`.
    EmpiezaDeCero {
        tenia: u64,
    },
    Avance {
        llevados: u64,
        total: u64,
    },
    CompruebaLaHuella,
}

/// Una descarga.
pub struct Descarga<'a> {
    pub fuente: &'a dyn Fuente,
    /// Dónde se queda el fichero si la huella cuadra.
    pub destino: &'a Path,
    pub huella_sha256: &'a str,
    /// En bytes.
    pub tamano: u64,
    /// Lo que se deja libre en el disco además de lo que falta por bajar, en bytes.
    pub margen_disco: u64,
    /// Cada cuántos bytes se dice el avance.
    pub avisar_cada: u64,
    pub espacio_libre: &'a dyn Fn(&Path) -> Resultado<u64>,
}

/// Una descarga terminada y comprobada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descargado {
    pub ruta: PathBuf,
    /// Lo que se bajó en esta vez; menos que el tamaño si se reanudó.
    pub bajados: u64,
}

/// Descarga, reanudando lo que haya, y se queda con el fichero solo si su huella es la esperada.
/// Si se corta, el `.parcial` se queda para la siguiente vez; si la huella no cuadra, se borra.
pub fn descargar(descarga: &Descarga, avisar: &mut dyn FnMut(Aviso)) -> Resultado<Descargado> {
    let carpeta = descarga.destino.parent().unwrap_or(Path::new(""));
    ficheros::crear_carpeta(carpeta)?;
    let parcial = ruta_parcial(descarga.destino);
    let mut desde = tamano_de(&parcial);
    if desde > descarga.tamano {
        ficheros::borrar_fichero_si_existe(&parcial)?;
        desde = 0;
    }
    comprobar_el_espacio(descarga, carpeta, desde)?;
    let mut bajados = 0;
    if desde < descarga.tamano {
        let apertura = descarga.fuente.abrir(desde)?;
        if desde > 0 && apertura.reanuda {
            avisar(Aviso::Reanuda { desde });
        } else if desde > 0 {
            avisar(Aviso::EmpiezaDeCero { tenia: desde });
            desde = 0;
        }
        bajados = copiar(apertura.lector, &parcial, desde, descarga, avisar)?;
        let llevados = desde + bajados;
        if llevados < descarga.tamano {
            return Err(Error::Revision(mensajes::descarga_cortada(
                llevados,
                descarga.tamano,
                &parcial,
            )));
        }
    }
    avisar(Aviso::CompruebaLaHuella);
    let huella = ficheros::huella_sha256(&parcial)?;
    if !huella.eq_ignore_ascii_case(descarga.huella_sha256) {
        ficheros::borrar_fichero_si_existe(&parcial)?;
        return Err(Error::Revision(mensajes::huella_distinta(
            descarga.huella_sha256,
            &huella,
        )));
    }
    ficheros::renombrar(&parcial, descarga.destino)?;
    Ok(Descargado {
        ruta: descarga.destino.to_path_buf(),
        bajados,
    })
}

/// `modelo.gguf` → `modelo.gguf.parcial`.
pub fn ruta_parcial(destino: &Path) -> PathBuf {
    let mut nombre = destino.as_os_str().to_owned();
    nombre.push(SUFIJO_PARCIAL);
    PathBuf::from(nombre)
}

/// Lo que ya pesa un fichero; 0 si no existe.
fn tamano_de(ruta: &Path) -> u64 {
    std::fs::metadata(ruta).map_or(0, |datos| datos.len())
}

/// Lo que falta por bajar más el margen tiene que caber en el disco; si no, no se pide nada.
fn comprobar_el_espacio(descarga: &Descarga, carpeta: &Path, desde: u64) -> Resultado<()> {
    let necesita = (descarga.tamano - desde).saturating_add(descarga.margen_disco);
    let libre = (descarga.espacio_libre)(carpeta)?;
    if libre < necesita {
        return Err(Error::Revision(mensajes::sin_espacio(
            necesita, libre, carpeta,
        )));
    }
    Ok(())
}

/// Copia lo que llega al `.parcial`: detrás de lo que había si se reanuda, o desde cero. Devuelve
/// lo que se ha escrito. Si la lectura falla, lo escrito se queda.
fn copiar(
    mut lector: Box<dyn Read>,
    parcial: &Path,
    desde: u64,
    descarga: &Descarga,
    avisar: &mut dyn FnMut(Aviso),
) -> Resultado<u64> {
    let error_de_escritura = |causa| Error::Escritura {
        ruta: parcial.to_path_buf(),
        causa,
    };
    let mut fichero = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(desde > 0)
        .truncate(desde == 0)
        .open(parcial)
        .map_err(error_de_escritura)?;
    let cada = descarga.avisar_cada.max(1);
    let mut siguiente_aviso = (desde / cada + 1) * cada;
    let mut trozo = vec![0u8; TAMANO_DEL_TROZO];
    let mut escritos = 0;
    loop {
        let leidos = match lector.read(&mut trozo) {
            Ok(0) => break,
            Ok(leidos) => leidos,
            Err(causa) if causa.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(causa) => {
                let llevados = desde + escritos;
                return Err(Error::Revision(format!(
                    "{}: {causa}",
                    mensajes::descarga_cortada(llevados, descarga.tamano, parcial)
                )));
            }
        };
        fichero
            .write_all(&trozo[..leidos])
            .map_err(error_de_escritura)?;
        escritos += leidos as u64;
        let llevados = desde + escritos;
        if llevados >= siguiente_aviso {
            avisar(Aviso::Avance {
                llevados,
                total: descarga.tamano,
            });
            siguiente_aviso = (llevados / cada + 1) * cada;
        }
    }
    fichero.flush().map_err(error_de_escritura)?;
    Ok(escritos)
}

/// La descarga de verdad, por HTTP. El tiempo de espera vale para conectar y para cada lectura,
/// no para la descarga entera, que puede durar una hora.
pub struct FuenteHttp {
    pub url: String,
    pub tiempo_espera: Duration,
}

impl Fuente for FuenteHttp {
    fn abrir(&self, desde: u64) -> Resultado<Apertura> {
        let agente = ureq::AgentBuilder::new()
            .timeout_connect(self.tiempo_espera)
            .timeout_read(self.tiempo_espera)
            .build();
        let mut peticion = agente.get(&self.url).set("User-Agent", AGENTE);
        if desde > 0 {
            peticion = peticion.set("Range", &format!("bytes={desde}-"));
        }
        let respuesta = peticion.call().map_err(|causa| {
            Error::Revision(format!("no se pudo descargar «{}»: {causa}", self.url))
        })?;
        Ok(Apertura {
            reanuda: respuesta.status() == ESTADO_CONTENIDO_PARCIAL,
            lector: Box::new(respuesta.into_reader()),
        })
    }
}

/// La dirección de un fichero de un repositorio.
pub fn url_del_fichero(sitio: &str, repositorio: &str, fichero: &str) -> String {
    format!(
        "{}/{repositorio}/resolve/main/{fichero}",
        sitio.trim_end_matches('/')
    )
}

/// El espacio libre de verdad, del disco donde está `carpeta`.
pub fn espacio_libre(carpeta: &Path) -> Resultado<u64> {
    fs4::available_space(carpeta).map_err(|causa| Error::Lectura {
        ruta: carpeta.to_path_buf(),
        causa,
    })
}

#[cfg(test)]
mod pruebas {
    use std::cell::RefCell;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    use sha2::{Digest, Sha256};

    use super::*;

    const CONTENIDO: &[u8] = b"Esto es un modelo de mentira.";

    fn huella(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Da el contenido de tres en tres bytes y, si se le dice, falla tras unos cuantos.
    struct Lector {
        datos: Vec<u8>,
        posicion: usize,
        falla_en: Option<usize>,
    }

    impl Read for Lector {
        fn read(&mut self, destino: &mut [u8]) -> std::io::Result<usize> {
            if self.falla_en.is_some_and(|n| self.posicion >= n) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "se cayó la red",
                ));
            }
            let hasta = (self.posicion + 3)
                .min(self.datos.len())
                .min(self.falla_en.unwrap_or(usize::MAX));
            let n = hasta - self.posicion;
            destino[..n].copy_from_slice(&self.datos[self.posicion..hasta]);
            self.posicion = hasta;
            Ok(n)
        }
    }

    struct FuenteFalsa {
        admite_rango: bool,
        falla_en: Option<usize>,
        pedidos: RefCell<Vec<u64>>,
    }

    impl FuenteFalsa {
        fn nueva() -> Self {
            Self {
                admite_rango: true,
                falla_en: None,
                pedidos: RefCell::new(vec![]),
            }
        }
    }

    impl Fuente for FuenteFalsa {
        fn abrir(&self, desde: u64) -> Resultado<Apertura> {
            self.pedidos.borrow_mut().push(desde);
            let reanuda = self.admite_rango && desde > 0;
            let inicio = if reanuda { desde as usize } else { 0 };
            Ok(Apertura {
                reanuda,
                lector: Box::new(Lector {
                    datos: CONTENIDO[inicio..].to_vec(),
                    posicion: 0,
                    falla_en: self.falla_en,
                }),
            })
        }
    }

    fn sobra_espacio(_: &Path) -> Resultado<u64> {
        Ok(u64::MAX)
    }

    fn descarga<'a>(
        fuente: &'a dyn Fuente,
        destino: &'a Path,
        huella: &'a str,
        espacio: &'a dyn Fn(&Path) -> Resultado<u64>,
    ) -> Descarga<'a> {
        Descarga {
            fuente,
            destino,
            huella_sha256: huella,
            tamano: CONTENIDO.len() as u64,
            margen_disco: 0,
            avisar_cada: 1024,
            espacio_libre: espacio,
        }
    }

    fn sin_avisos(_: Aviso) {}

    #[test]
    fn con_la_huella_buena_se_queda_con_su_nombre() {
        let carpeta = tempfile::tempdir().unwrap();
        let destino = carpeta.path().join("modelos").join("m.gguf");
        let fuente = FuenteFalsa::nueva();
        let buena = huella(CONTENIDO);
        let hecha = descargar(
            &descarga(&fuente, &destino, &buena, &sobra_espacio),
            &mut sin_avisos,
        )
        .unwrap();
        assert_eq!(hecha.ruta, destino);
        assert_eq!(hecha.bajados, CONTENIDO.len() as u64);
        assert_eq!(std::fs::read(&destino).unwrap(), CONTENIDO);
        assert!(!ruta_parcial(&destino).exists());
    }

    #[test]
    fn con_la_huella_mala_se_borra_el_parcial() {
        let carpeta = tempfile::tempdir().unwrap();
        let destino = carpeta.path().join("m.gguf");
        let fuente = FuenteFalsa::nueva();
        let mala = huella(b"otra cosa");
        let fallo = descargar(
            &descarga(&fuente, &destino, &mala, &sobra_espacio),
            &mut sin_avisos,
        )
        .unwrap_err()
        .to_string();
        assert!(
            fallo.contains(&mala) && fallo.contains(&huella(CONTENIDO)),
            "{fallo}"
        );
        assert!(!destino.exists());
        assert!(!ruta_parcial(&destino).exists());
    }

    #[test]
    fn un_corte_deja_el_parcial_y_la_siguiente_vez_pide_lo_que_falta() {
        let carpeta = tempfile::tempdir().unwrap();
        let destino = carpeta.path().join("m.gguf");
        let buena = huella(CONTENIDO);
        let que_se_corta = FuenteFalsa {
            falla_en: Some(10),
            ..FuenteFalsa::nueva()
        };
        let fallo = descargar(
            &descarga(&que_se_corta, &destino, &buena, &sobra_espacio),
            &mut sin_avisos,
        )
        .unwrap_err()
        .to_string();
        assert!(fallo.contains("se cayó la red"), "{fallo}");
        assert_eq!(
            std::fs::read(ruta_parcial(&destino)).unwrap(),
            &CONTENIDO[..10]
        );

        let fuente = FuenteFalsa::nueva();
        let mut avisos = vec![];
        let hecha = descargar(
            &descarga(&fuente, &destino, &buena, &sobra_espacio),
            &mut |aviso| avisos.push(aviso),
        )
        .unwrap();
        assert_eq!(*fuente.pedidos.borrow(), [10]);
        assert_eq!(hecha.bajados, CONTENIDO.len() as u64 - 10);
        assert_eq!(avisos[0], Aviso::Reanuda { desde: 10 });
        assert_eq!(std::fs::read(&destino).unwrap(), CONTENIDO);
    }

    #[test]
    fn un_servidor_sin_range_empieza_de_cero_y_lo_dice() {
        let carpeta = tempfile::tempdir().unwrap();
        let destino = carpeta.path().join("m.gguf");
        std::fs::write(ruta_parcial(&destino), &CONTENIDO[..10]).unwrap();
        let fuente = FuenteFalsa {
            admite_rango: false,
            ..FuenteFalsa::nueva()
        };
        let buena = huella(CONTENIDO);
        let mut avisos = vec![];
        descargar(
            &descarga(&fuente, &destino, &buena, &sobra_espacio),
            &mut |aviso| avisos.push(aviso),
        )
        .unwrap();
        assert_eq!(avisos[0], Aviso::EmpiezaDeCero { tenia: 10 });
        assert_eq!(std::fs::read(&destino).unwrap(), CONTENIDO);
    }

    #[test]
    fn un_parcial_ya_completo_no_pide_nada_y_se_comprueba() {
        let carpeta = tempfile::tempdir().unwrap();
        let destino = carpeta.path().join("m.gguf");
        std::fs::write(ruta_parcial(&destino), CONTENIDO).unwrap();
        let fuente = FuenteFalsa::nueva();
        let buena = huella(CONTENIDO);
        let hecha = descargar(
            &descarga(&fuente, &destino, &buena, &sobra_espacio),
            &mut sin_avisos,
        )
        .unwrap();
        assert!(fuente.pedidos.borrow().is_empty());
        assert_eq!(hecha.bajados, 0);
        assert!(destino.exists());
    }

    #[test]
    fn un_parcial_mas_grande_que_el_fichero_se_tira_y_se_empieza_de_cero() {
        let carpeta = tempfile::tempdir().unwrap();
        let destino = carpeta.path().join("m.gguf");
        std::fs::write(ruta_parcial(&destino), [CONTENIDO, b"sobra"].concat()).unwrap();
        let fuente = FuenteFalsa::nueva();
        let buena = huella(CONTENIDO);
        descargar(
            &descarga(&fuente, &destino, &buena, &sobra_espacio),
            &mut sin_avisos,
        )
        .unwrap();
        assert_eq!(*fuente.pedidos.borrow(), [0]);
        assert_eq!(std::fs::read(&destino).unwrap(), CONTENIDO);
    }

    #[test]
    fn sin_espacio_no_se_pide_nada_y_se_dice_cuanto_falta() {
        let carpeta = tempfile::tempdir().unwrap();
        let destino = carpeta.path().join("m.gguf");
        let fuente = FuenteFalsa::nueva();
        let buena = huella(CONTENIDO);
        let poco = |_: &Path| -> Resultado<u64> { Ok(5) };
        let mut con_margen = descarga(&fuente, &destino, &buena, &poco);
        con_margen.margen_disco = 100;
        let fallo = descargar(&con_margen, &mut sin_avisos)
            .unwrap_err()
            .to_string();
        assert!(fuente.pedidos.borrow().is_empty());
        let necesita = CONTENIDO.len() as u64 + 100;
        assert!(
            fallo.contains(&format!("({necesita} bytes)")) && fallo.contains("(5 bytes)"),
            "{fallo}"
        );
    }

    #[test]
    fn el_avance_se_dice_cada_vez_que_se_pasa_la_cifra() {
        let carpeta = tempfile::tempdir().unwrap();
        let destino = carpeta.path().join("m.gguf");
        let fuente = FuenteFalsa::nueva();
        let buena = huella(CONTENIDO);
        let mut cada_diez = descarga(&fuente, &destino, &buena, &sobra_espacio);
        cada_diez.avisar_cada = 10;
        let mut avances = vec![];
        descargar(&cada_diez, &mut |aviso| {
            if let Aviso::Avance { llevados, .. } = aviso {
                avances.push(llevados);
            }
        })
        .unwrap();
        // De tres en tres: se pasa el 10 al llegar a 12, y el 20 al llegar a 21.
        assert_eq!(avances, [12, 21]);
    }

    #[test]
    fn la_url_de_un_fichero_es_la_de_resolve() {
        assert_eq!(
            url_del_fichero("https://huggingface.co/", "a/b", "m-Q4_K_M.gguf"),
            "https://huggingface.co/a/b/resolve/main/m-Q4_K_M.gguf"
        );
    }

    /// Un servidor HTTP de una sola petición en 127.0.0.1, que entiende `Range` o no. Devuelve
    /// la URL y lo que le llegó en la cabecera `Range`.
    fn servidor(entiende_range: bool) -> (String, std::thread::JoinHandle<Option<String>>) {
        let escucha = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/m.gguf", escucha.local_addr().unwrap());
        let hilo = std::thread::spawn(move || {
            let (mut conexion, _) = escucha.accept().unwrap();
            let mut lector = BufReader::new(conexion.try_clone().unwrap());
            let mut rango = None;
            loop {
                let mut linea = String::new();
                lector.read_line(&mut linea).unwrap();
                if linea.trim().is_empty() {
                    break;
                }
                if let Some(valor) = linea.to_ascii_lowercase().strip_prefix("range: bytes=") {
                    rango = Some(valor.trim().to_string());
                }
            }
            let desde: usize = match (&rango, entiende_range) {
                (Some(r), true) => r.trim_end_matches('-').parse().unwrap(),
                _ => 0,
            };
            let cuerpo = &CONTENIDO[desde..];
            let estado = if desde > 0 {
                "206 Partial Content"
            } else {
                "200 OK"
            };
            write!(
                conexion,
                "HTTP/1.1 {estado}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                cuerpo.len()
            )
            .unwrap();
            conexion.write_all(cuerpo).unwrap();
            rango
        });
        (url, hilo)
    }

    fn leer_todo(apertura: Apertura) -> Vec<u8> {
        let mut leido = vec![];
        let mut lector = apertura.lector;
        lector.read_to_end(&mut leido).unwrap();
        leido
    }

    fn http(url: String) -> FuenteHttp {
        FuenteHttp {
            url,
            tiempo_espera: Duration::from_secs(5),
        }
    }

    #[test]
    fn por_http_se_pide_el_resto_con_range_y_el_206_reanuda() {
        let (url, hilo) = servidor(true);
        let apertura = http(url).abrir(10).unwrap();
        assert!(apertura.reanuda);
        assert_eq!(leer_todo(apertura), &CONTENIDO[10..]);
        assert_eq!(hilo.join().unwrap().as_deref(), Some("10-"));
    }

    #[test]
    fn por_http_un_200_no_reanuda() {
        let (url, hilo) = servidor(false);
        let apertura = http(url).abrir(10).unwrap();
        assert!(!apertura.reanuda);
        assert_eq!(leer_todo(apertura), CONTENIDO);
        hilo.join().unwrap();
    }

    /// Contesta una petición con un 302 hacia `destino`, como `resolve/main` de Hugging Face, que
    /// manda los ficheros grandes a su CDN.
    fn redirigir_a(destino: String) -> String {
        let escucha = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/resolve/main/m.gguf",
            escucha.local_addr().unwrap()
        );
        std::thread::spawn(move || {
            let (mut conexion, _) = escucha.accept().unwrap();
            let mut lector = BufReader::new(conexion.try_clone().unwrap());
            let mut linea = String::new();
            while lector.read_line(&mut linea).unwrap() > 2 {
                linea.clear();
            }
            write!(
                conexion,
                "HTTP/1.1 302 Found\r\nLocation: {destino}\r\nContent-Length: 0\r\n\
                 Connection: close\r\n\r\n"
            )
            .unwrap();
        });
        url
    }

    #[test]
    fn por_http_el_range_sobrevive_a_una_redireccion() {
        let (destino, hilo) = servidor(true);
        let apertura = http(redirigir_a(destino)).abrir(10).unwrap();
        assert!(apertura.reanuda);
        assert_eq!(leer_todo(apertura), &CONTENIDO[10..]);
        assert_eq!(hilo.join().unwrap().as_deref(), Some("10-"));
    }

    #[test]
    fn por_http_desde_el_principio_no_se_manda_range() {
        let (url, hilo) = servidor(true);
        let apertura = http(url).abrir(0).unwrap();
        assert_eq!(leer_todo(apertura), CONTENIDO);
        assert_eq!(hilo.join().unwrap(), None);
    }
}
