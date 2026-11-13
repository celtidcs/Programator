//! Ejecutar una orden externa sin shell y con un tiempo máximo.
//!
//! Lo usan la verificación de propuestas del arnés y las pruebas ocultas del banco de modelos.
//! Sin shell: el programa y sus argumentos van por separado, así que nada de lo que venga en un
//! argumento puede convertirse en otra orden.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Lo que rodea al proceso lanzado: qué se hace con él nada más lanzarlo y cómo se corta al
/// vencer el plazo. La de siempre, [`SoloElHijo`], mata al hijo y nada más; el banco tiene una que
/// mete el árbol entero en un Job de Windows.
pub trait Contencion {
    /// Justo después de lanzar. Si falla, se mata al hijo y la orden no se lanzó.
    fn tras_lanzar(&mut self, hijo: &mut Child) -> Result<(), String>;
    /// Al vencer el plazo.
    fn cortar(&mut self, hijo: &mut Child);
    /// Cuando el hijo ya terminó, antes de leer lo que escribió. Un proceso que el hijo dejara
    /// vivo con la salida abierta dejaría la lectura esperando para siempre; la contención que lo
    /// tiene a mano lo mata aquí.
    fn al_terminar(&mut self) {}
    /// En cada latido de la espera, mientras el hijo sigue en marcha. Para vigilar lo que hace.
    fn mientras_espera(&mut self) {}
}

/// Mata al hijo al cortar. Lo que el hijo hubiera lanzado puede seguir vivo.
pub struct SoloElHijo;

impl Contencion for SoloElHijo {
    fn tras_lanzar(&mut self, _hijo: &mut Child) -> Result<(), String> {
        Ok(())
    }

    fn cortar(&mut self, hijo: &mut Child) {
        let _ = hijo.kill();
    }
}

/// Cada cuánto se comprueba si la orden ya terminó, mientras se espera su tiempo máximo.
const LATIDO_ESPERA: Duration = Duration::from_millis(50);

/// Cómo acabó una orden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesenlaceDeOrden {
    /// Terminó por sí sola. `exito` es que salió con código 0.
    Terminada {
        exito: bool,
        /// El código de salida; `None` si terminó por una señal.
        codigo: Option<i32>,
        estado: String,
        salida: String,
        errores: String,
    },
    /// Seguía en marcha al llegar al tiempo máximo y se cortó.
    Agotada,
    /// No se pudo lanzar: el programa no existe o no se deja ejecutar.
    NoSeLanzo(String),
    /// Se lanzó, pero no se pudo esperar a que terminara.
    NoSePudoEsperar(String),
    /// Terminó, pero su salida no se pudo leer.
    SalidaIlegible(String),
}

/// Las variables con que los programas buscan dónde dejar sus temporales: `TEMP` y `TMP` en
/// Windows, `TMPDIR` en Linux y macOS.
const VARIABLES_DE_TEMPORALES: [&str; 3] = ["TEMP", "TMP", "TMPDIR"];

/// Ejecuta `programa` con `argumentos` en `directorio`, cortándolo si pasa de `tiempo_maximo`.
pub fn ejecutar_con_tope(
    programa: &str,
    argumentos: &[String],
    directorio: &Path,
    tiempo_maximo: Duration,
) -> DesenlaceDeOrden {
    ejecutar(
        Command::new(programa),
        argumentos,
        directorio,
        tiempo_maximo,
        &mut SoloElHijo,
    )
}

/// Como [`ejecutar_con_tope`], pero con los temporales del programa dentro de `directorio`.
///
/// Es para la verificación de propuestas, que corre en la máquina de quien usa Programator: un
/// compilador que escribiera en el temporal del sistema dejaría restos fuera del proyecto.
pub fn ejecutar_con_tope_y_temporales_dentro(
    programa: &str,
    argumentos: &[String],
    directorio: &Path,
    tiempo_maximo: Duration,
) -> DesenlaceDeOrden {
    let entorno: Vec<(String, String)> = VARIABLES_DE_TEMPORALES
        .iter()
        .map(|variable| (variable.to_string(), directorio.display().to_string()))
        .collect();
    ejecutar_con_tope_y_entorno(programa, argumentos, directorio, tiempo_maximo, &entorno)
}

/// Como [`ejecutar_con_tope`], con estas variables de entorno añadidas a las que ya hay.
pub fn ejecutar_con_tope_y_entorno(
    programa: &str,
    argumentos: &[String],
    directorio: &Path,
    tiempo_maximo: Duration,
    entorno: &[(String, String)],
) -> DesenlaceDeOrden {
    ejecutar_con_tope_entorno_y_contencion(
        programa,
        argumentos,
        directorio,
        tiempo_maximo,
        entorno,
        &mut SoloElHijo,
    )
}

/// Como [`ejecutar_con_tope_y_entorno`], con otra [`Contencion`] alrededor del hijo. La entrada
/// estándar del hijo va por una tubería, para que la contención pueda escribirle.
pub fn ejecutar_con_tope_entorno_y_contencion(
    programa: &str,
    argumentos: &[String],
    directorio: &Path,
    tiempo_maximo: Duration,
    entorno: &[(String, String)],
    contencion: &mut dyn Contencion,
) -> DesenlaceDeOrden {
    let mut orden = Command::new(programa);
    aplicar_entorno(&mut orden, entorno);
    ejecutar(orden, argumentos, directorio, tiempo_maximo, contencion)
}

/// Pone las variables de `entorno` encima de las heredadas, también las vacías: una vacía no es
/// lo mismo que ninguna, y cargo, por ejemplo, solo deja de usar el `rustc-wrapper` de su
/// configuración si `CARGO_BUILD_RUSTC_WRAPPER` está puesta y vacía (máquina Linux de pruebas, 12/11/2026).
fn aplicar_entorno(orden: &mut Command, entorno: &[(String, String)]) {
    orden.envs(entorno.iter().map(|(nombre, valor)| (nombre, valor)));
}

fn ejecutar(
    mut orden: Command,
    argumentos: &[String],
    directorio: &Path,
    tiempo_maximo: Duration,
    contencion: &mut dyn Contencion,
) -> DesenlaceDeOrden {
    let hijo = orden
        .args(argumentos)
        .current_dir(directorio)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut hijo = match hijo {
        Ok(h) => h,
        Err(e) => return DesenlaceDeOrden::NoSeLanzo(e.to_string()),
    };
    if let Err(motivo) = contencion.tras_lanzar(&mut hijo) {
        let _ = hijo.kill();
        let _ = hijo.wait();
        return DesenlaceDeOrden::NoSeLanzo(motivo);
    }
    // Lo que la contención no se quedó para escribir se cierra: el hijo lee el final al momento,
    // como con la entrada nula de antes.
    drop(hijo.stdin.take());

    // Las dos salidas se leen mientras se espera, cada una en su hilo. Si nadie las leyera, una
    // orden que escribe más de lo que cabe en la tubería se quedaría bloqueada escribiendo y
    // acabaría cortada por tiempo sin tener nada malo (un `cargo test` con errores lo hace).
    let lector_salida = hijo.stdout.take().map(leer_en_otro_hilo);
    let lector_errores = hijo.stderr.take().map(leer_en_otro_hilo);

    // Espera con tope. `Child` no sabe esperar con límite por sí solo, y una orden colgada no
    // puede colgar un arnés pensado para vivir días.
    let limite = Instant::now() + tiempo_maximo;
    let estado = loop {
        match hijo.try_wait() {
            Ok(Some(estado)) => break estado,
            Ok(None) => {
                if Instant::now() >= limite {
                    contencion.cortar(&mut hijo);
                    let _ = hijo.wait();
                    return DesenlaceDeOrden::Agotada;
                }
                contencion.mientras_espera();
                std::thread::sleep(LATIDO_ESPERA);
            }
            Err(e) => return DesenlaceDeOrden::NoSePudoEsperar(e.to_string()),
        }
    };
    contencion.al_terminar();

    let salida = match recoger(lector_salida) {
        Ok(texto) => texto,
        Err(motivo) => return DesenlaceDeOrden::SalidaIlegible(motivo),
    };
    let errores = match recoger(lector_errores) {
        Ok(texto) => texto,
        Err(motivo) => return DesenlaceDeOrden::SalidaIlegible(motivo),
    };
    DesenlaceDeOrden::Terminada {
        exito: estado.success(),
        codigo: estado.code(),
        estado: estado.to_string(),
        salida,
        errores,
    }
}

type Lector = std::thread::JoinHandle<std::io::Result<Vec<u8>>>;

fn leer_en_otro_hilo<R: Read + Send + 'static>(mut origen: R) -> Lector {
    std::thread::spawn(move || {
        let mut leido = Vec::new();
        origen.read_to_end(&mut leido).map(|_| leido)
    })
}

/// Espera a que el hilo termine de leer y da el texto; sin hilo, texto vacío.
fn recoger(lector: Option<Lector>) -> Result<String, String> {
    let Some(lector) = lector else {
        return Ok(String::new());
    };
    match lector.join() {
        Ok(Ok(leido)) => Ok(String::from_utf8_lossy(&leido).into_owned()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("el hilo que leía la salida terminó con un pánico".to_string()),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn una_variable_vacia_se_pone_vacia() {
        let mut orden = Command::new("programa");

        aplicar_entorno(
            &mut orden,
            &[
                ("RUSTC_WRAPPER".into(), String::new()),
                ("TEMP".into(), "/tanda/tmp".into()),
            ],
        );

        let entorno: Vec<_> = orden.get_envs().collect();
        assert!(entorno.contains(&(OsStr::new("RUSTC_WRAPPER"), Some(OsStr::new("")))));
        assert!(entorno.contains(&(OsStr::new("TEMP"), Some(OsStr::new("/tanda/tmp")))));
    }
    use crate::plataforma::ordenes_de_prueba::{eco_de_variable, eco_y_codigo, volcar_fichero};

    fn ejecutar(orden: &[String], directorio: &Path, tope: Duration) -> DesenlaceDeOrden {
        ejecutar_con_tope(&orden[0], &orden[1..], directorio, tope)
    }

    #[test]
    fn una_orden_que_sale_con_cero_termina_con_exito_y_deja_su_salida() {
        let dir = tempfile::tempdir().unwrap();

        let desenlace = ejecutar(
            &eco_y_codigo("hola", 0),
            dir.path(),
            Duration::from_secs(30),
        );

        match desenlace {
            DesenlaceDeOrden::Terminada { exito, salida, .. } => {
                assert!(exito);
                assert!(salida.contains("hola"));
            }
            otro => panic!("{otro:?}"),
        }
    }

    #[test]
    fn la_orden_ve_las_variables_de_entorno_que_se_le_pasan() {
        let dir = tempfile::tempdir().unwrap();
        let orden = eco_de_variable("PROGRAMATOR_PRUEBA");

        let desenlace = ejecutar_con_tope_y_entorno(
            &orden[0],
            &orden[1..],
            dir.path(),
            Duration::from_secs(30),
            &[("PROGRAMATOR_PRUEBA".into(), "valor-de-prueba".into())],
        );

        match desenlace {
            DesenlaceDeOrden::Terminada { exito, salida, .. } => {
                assert!(exito);
                assert!(salida.contains("valor-de-prueba"), "{salida}");
            }
            otro => panic!("{otro:?}"),
        }
    }

    #[test]
    fn una_orden_que_sale_con_uno_termina_sin_exito() {
        let dir = tempfile::tempdir().unwrap();

        let desenlace = ejecutar(&eco_y_codigo("mal", 1), dir.path(), Duration::from_secs(30));

        assert!(matches!(
            desenlace,
            DesenlaceDeOrden::Terminada {
                exito: false,
                codigo: Some(1),
                ..
            }
        ));
    }

    /// Anota lo que le pasa y, si se le pide, falla tras lanzar.
    #[derive(Default)]
    struct ContencionDePrueba {
        fallar: bool,
        cortes: usize,
        terminadas: usize,
        latidos: usize,
    }

    impl Contencion for ContencionDePrueba {
        fn tras_lanzar(&mut self, _hijo: &mut Child) -> Result<(), String> {
            if self.fallar {
                Err("sin Job".into())
            } else {
                Ok(())
            }
        }

        fn cortar(&mut self, hijo: &mut Child) {
            self.cortes += 1;
            let _ = hijo.kill();
        }

        fn al_terminar(&mut self) {
            self.terminadas += 1;
        }

        fn mientras_espera(&mut self) {
            self.latidos += 1;
        }
    }

    #[test]
    fn cuando_el_hijo_termina_se_avisa_a_la_contencion_y_al_cortar_no() {
        let dir = tempfile::tempdir().unwrap();
        let orden = eco_y_codigo("hola", 0);
        let mut contencion = ContencionDePrueba::default();

        let desenlace = ejecutar_con_tope_entorno_y_contencion(
            &orden[0],
            &orden[1..],
            dir.path(),
            Duration::from_secs(30),
            &[],
            &mut contencion,
        );

        assert!(matches!(desenlace, DesenlaceDeOrden::Terminada { .. }));
        assert_eq!((contencion.terminadas, contencion.cortes), (1, 0));
    }

    #[test]
    fn si_la_contencion_falla_la_orden_no_se_lanzo() {
        let dir = tempfile::tempdir().unwrap();
        let orden = eco_y_codigo("hola", 0);
        let mut contencion = ContencionDePrueba {
            fallar: true,
            ..ContencionDePrueba::default()
        };

        let desenlace = ejecutar_con_tope_entorno_y_contencion(
            &orden[0],
            &orden[1..],
            dir.path(),
            Duration::from_secs(30),
            &[],
            &mut contencion,
        );

        assert_eq!(desenlace, DesenlaceDeOrden::NoSeLanzo("sin Job".into()));
    }

    #[test]
    fn al_vencer_el_plazo_corta_la_contencion() {
        let dir = tempfile::tempdir().unwrap();
        let orden = crate::plataforma::ordenes_de_prueba::tardar_mucho();
        let mut contencion = ContencionDePrueba::default();

        let desenlace = ejecutar_con_tope_entorno_y_contencion(
            &orden[0],
            &orden[1..],
            dir.path(),
            Duration::from_millis(300),
            &[],
            &mut contencion,
        );

        assert_eq!(desenlace, DesenlaceDeOrden::Agotada);
        assert_eq!((contencion.cortes, contencion.terminadas), (1, 0));
        // 300 ms de espera con latidos de 50 dan varios; basta con que se avise mientras espera.
        assert!(contencion.latidos > 0);
    }

    #[test]
    fn un_programa_que_no_existe_no_se_lanza() {
        let dir = tempfile::tempdir().unwrap();

        let desenlace = ejecutar_con_tope(
            "programa-que-no-existe-en-ninguna-parte",
            &[],
            dir.path(),
            Duration::from_secs(5),
        );

        assert!(matches!(desenlace, DesenlaceDeOrden::NoSeLanzo(_)));
    }

    /// Con la salida en tuberías que nadie lee, una orden que escribe más de lo que cabe en la
    /// tubería se bloquea y acaba cortada por tiempo aunque no tenga nada malo.
    #[test]
    fn una_orden_que_escribe_mucho_no_se_queda_bloqueada() {
        let dir = tempfile::tempdir().unwrap();
        let fichero = dir.path().join("grande.txt");
        let linea = "0123456789".repeat(10);
        let contenido = format!("{linea}\n").repeat(10_000);
        std::fs::write(&fichero, &contenido).unwrap();
        let ruta = fichero.display().to_string();
        let orden: Vec<String> = volcar_fichero()
            .into_iter()
            .map(|a| a.replace("{fichero}", &ruta))
            .collect();

        let desenlace = ejecutar(&orden, dir.path(), Duration::from_secs(20));

        match desenlace {
            DesenlaceDeOrden::Terminada { exito, salida, .. } => {
                assert!(exito);
                assert!(salida.len() >= contenido.len());
            }
            otro => panic!("{otro:?}"),
        }
    }
}
