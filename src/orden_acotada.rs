//! Ejecutar una orden externa sin shell y con un tiempo máximo.
//!
//! Lo usan la verificación de propuestas del arnés y las pruebas ocultas del banco de modelos.
//! Sin shell: el programa y sus argumentos van por separado, así que nada de lo que venga en un
//! argumento puede convertirse en otra orden.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
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

/// Cuánto se espera a los hilos que leen la salida de un hijo ya cortado. Al morir el hijo, el
/// sistema cierra sus tuberías al momento y los lectores terminan en milisegundos; dos segundos son
/// margen de sobra. Si pasan, es que un proceso que lanzó el hijo sigue con la tubería abierta, y
/// ese hilo se deja: terminará cuando ese proceso la cierre.
const PLAZO_PARA_LOS_LECTORES: Duration = Duration::from_secs(2);

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

    // Sin límite si el tope no cabe en un `Instant`: sumarlo entraría en pánico.
    let limite = Instant::now().checked_add(tiempo_maximo);
    let estado = match esperar(|| hijo.try_wait(), limite, contencion) {
        FinDeLaEspera::Termino(estado) => estado,
        FinDeLaEspera::SinTerminar(interrupcion) => {
            return cerrar_sin_terminar(
                interrupcion,
                &mut hijo,
                contencion,
                [lector_salida, lector_errores],
            );
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

/// Por qué se dejó de esperar a un hijo que no había terminado.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Interrupcion {
    /// Llegó el tiempo máximo.
    Vencio,
    /// El sistema no dejó preguntar por el hijo; el texto es su motivo.
    Fallo(String),
}

/// Cómo acabó la espera.
#[derive(Debug)]
enum FinDeLaEspera {
    Termino(ExitStatus),
    SinTerminar(Interrupcion),
}

/// Espera con tope. `Child` no sabe esperar con límite por sí solo, y una orden colgada no puede
/// colgar un arnés pensado para vivir días. `consultar` es `try_wait` del hijo; `limite`, `None`
/// para no tener límite.
fn esperar(
    mut consultar: impl FnMut() -> std::io::Result<Option<ExitStatus>>,
    limite: Option<Instant>,
    contencion: &mut dyn Contencion,
) -> FinDeLaEspera {
    loop {
        match consultar() {
            Ok(Some(estado)) => return FinDeLaEspera::Termino(estado),
            Ok(None) if limite.is_some_and(|limite| Instant::now() >= limite) => {
                return FinDeLaEspera::SinTerminar(Interrupcion::Vencio);
            }
            Ok(None) => {
                contencion.mientras_espera();
                std::thread::sleep(LATIDO_ESPERA);
            }
            Err(e) => return FinDeLaEspera::SinTerminar(Interrupcion::Fallo(e.to_string())),
        }
    }
}

/// Lo que se hace con un hijo al que se dejó de esperar sin que terminara, venciera el plazo o
/// fallara la espera: se corta por la contención, se recoge y se recogen sus lectores. Así,
/// aunque falle la espera, no queda el hijo vivo ni los lectores sueltos.
fn cerrar_sin_terminar(
    interrupcion: Interrupcion,
    hijo: &mut Child,
    contencion: &mut dyn Contencion,
    lectores: [Option<Lector>; 2],
) -> DesenlaceDeOrden {
    contencion.cortar(hijo);
    let _ = hijo.wait();
    // Lo que no se recoja en el plazo es de un nieto que sigue con la tubería abierta: ese hilo
    // termina solo cuando el nieto la cierre, y esperarlo colgaría a quien llamó.
    let _ = recoger_con_plazo(lectores, PLAZO_PARA_LOS_LECTORES);
    match interrupcion {
        Interrupcion::Vencio => DesenlaceDeOrden::Agotada,
        Interrupcion::Fallo(motivo) => DesenlaceDeOrden::NoSePudoEsperar(motivo),
    }
}

/// Espera a que terminen los lectores, como mucho `plazo` entre todos, y recoge los que hayan
/// terminado. Devuelve cuántos quedaron sin recoger.
fn recoger_con_plazo(lectores: [Option<Lector>; 2], plazo: Duration) -> usize {
    let limite = Instant::now().checked_add(plazo);
    let mut pendientes: Vec<Lector> = lectores.into_iter().flatten().collect();
    loop {
        let (terminados, siguen): (Vec<Lector>, Vec<Lector>) = pendientes
            .into_iter()
            .partition(|lector| lector.is_finished());
        for lector in terminados {
            let _ = lector.join();
        }
        pendientes = siguen;
        if pendientes.is_empty() || limite.is_some_and(|limite| Instant::now() >= limite) {
            return pendientes.len();
        }
        std::thread::sleep(LATIDO_ESPERA);
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

    /// Un tope enorme no hace entrar en pánico la suma con `Instant::now()`.
    #[test]
    fn con_un_tope_enorme_la_orden_termina_sin_panico() {
        let dir = tempfile::tempdir().unwrap();

        let desenlace = ejecutar(&eco_y_codigo("hola", 0), dir.path(), Duration::MAX);

        assert!(
            matches!(desenlace, DesenlaceDeOrden::Terminada { exito: true, .. }),
            "{desenlace:?}"
        );
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

    /// Si no se puede esperar al hijo, no se devuelve el error dejándolo vivo: se corta y se
    /// recoge.
    #[test]
    fn si_no_se_puede_esperar_al_hijo_se_corta_y_se_recoge() {
        let dir = tempfile::tempdir().unwrap();
        let orden = crate::plataforma::ordenes_de_prueba::tardar_mucho();
        let mut hijo = Command::new(&orden[0])
            .args(&orden[1..])
            .current_dir(dir.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut contencion = ContencionDePrueba::default();
        // Un lector que tarda un poco en terminar, como el de una tubería que se está cerrando:
        // al volver, cerrar_sin_terminar tiene que haberlo esperado. Sin esta parte, soltar los
        // lectores sin esperarlos pasaría la suite.
        let leido = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let marca = std::sync::Arc::clone(&leido);
        let lector: Lector = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            marca.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(Vec::new())
        });

        let desenlace = cerrar_sin_terminar(
            Interrupcion::Fallo("sin respuesta".into()),
            &mut hijo,
            &mut contencion,
            [Some(lector), None],
        );
        let lector_recogido = leido.load(std::sync::atomic::Ordering::SeqCst);
        let sigue_vivo = matches!(hijo.try_wait(), Ok(None));
        if sigue_vivo {
            let _ = hijo.kill();
            let _ = hijo.wait();
        }

        assert_eq!(
            desenlace,
            DesenlaceDeOrden::NoSePudoEsperar("sin respuesta".into())
        );
        assert_eq!(contencion.cortes, 1);
        assert!(!sigue_vivo, "el hijo quedó vivo");
        assert!(lector_recogido, "volvió sin esperar al lector");
    }

    #[test]
    fn la_espera_que_falla_lo_dice_con_su_motivo() {
        let mut contencion = ContencionDePrueba::default();

        let fin = esperar(
            || Err(std::io::Error::other("sin respuesta")),
            None,
            &mut contencion,
        );

        assert!(
            matches!(&fin, FinDeLaEspera::SinTerminar(Interrupcion::Fallo(m)) if m == "sin respuesta"),
            "{fin:?}"
        );
    }

    /// Un lector que termina se recoge; uno que sigue bloqueado (un nieto que sostiene la tubería)
    /// se deja al pasar el plazo, sin colgar a quien espera.
    #[test]
    fn los_lectores_se_recogen_con_plazo_y_sin_colgarse() {
        let terminado: Lector = std::thread::spawn(|| Ok(Vec::new()));
        let (suelta, espera) = std::sync::mpsc::channel::<()>();
        let bloqueado: Lector = std::thread::spawn(move || {
            let _ = espera.recv();
            Ok(Vec::new())
        });
        let plazo = Duration::from_millis(300);
        let inicio = Instant::now();

        let abandonados = recoger_con_plazo([Some(terminado), Some(bloqueado)], plazo);
        let tardo = inicio.elapsed();
        drop(suelta);

        assert_eq!(abandonados, 1);
        assert!(tardo >= plazo, "no esperó el plazo: {tardo:?}");
        assert!(tardo < Duration::from_secs(5), "se colgó: {tardo:?}");
    }

    #[test]
    fn sin_lectores_pendientes_no_se_espera_el_plazo() {
        let terminado: Lector = std::thread::spawn(|| Ok(Vec::new()));
        let inicio = Instant::now();

        let abandonados = recoger_con_plazo([Some(terminado), None], Duration::from_secs(30));

        assert_eq!(abandonados, 0);
        assert!(inicio.elapsed() < Duration::from_secs(5));
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
