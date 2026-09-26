//! Qué carpeta de trabajo se usa, y cómo se recuerda para la próxima vez.
//!
//! **Por qué existe este módulo.** Hasta la 0.9.0, sin `[carpeta] ruta` en el TOML el programa
//! abría un diálogo nativo y esperaba a que alguien eligiera. Un agente que dirige a Programator no
//! puede reiniciarlo tras un fallo: solo puede pedírselo a un humano, y en la jornada del
//! 23/09/2026 hubo que hacerlo. Con `--ruta` y con memoria de la última carpeta, el arranque se
//! vuelve automatizable.
//!
//! `decidir` es pura y no toca disco: quien llama le pasa la carpeta recordada **ya comprobada**.

use std::path::{Path, PathBuf};

/// El fichero de una línea donde se recuerda la última carpeta buena.
///
/// Vive junto al TOML, no dentro del proyecto anfitrión: es estado de esta instalación de
/// Programator, no del proyecto, y meterlo en un repositorio ajeno sería una intrusión más.
pub const NOMBRE_DE_LA_MEMORIA: &str = "ultima-carpeta.txt";

/// De dónde salió la carpeta, para poder decirlo en voz alta al arrancar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    LineaDeOrdenes,
    Configuracion,
    Recordada,
}

impl Origen {
    /// Cómo se nombra en el informe de arranque.
    pub fn descripcion(self) -> &'static str {
        match self {
            Origen::LineaDeOrdenes => "pedida en la línea de órdenes",
            Origen::Configuracion => "fijada en programator.toml",
            Origen::Recordada => "la última en la que se trabajó",
        }
    }
}

/// Cuál de las tres fuentes manda, si es que manda alguna.
///
/// Devuelve `None` cuando no hay ninguna: es entonces, y solo entonces, cuando quien llama abre el
/// diálogo.
///
/// **Sobre `preguntar_siempre`.** La memoria de la última carpeta se añadió en la 0.9.0 para que el
/// arnés pudiera reiniciarse sin nadie delante: en NatureLand, el agente que lo dirigía no pudo
/// levantarlo tras un fallo porque el programa se quedaba esperando a que una persona pulsara en una
/// ventana. Resolvió eso y creó otra molestia, la de no poder cambiar de proyecto sin borrar un
/// fichero. Esta bandera deja elegir: con ella puesta, la memoria no decide y se pregunta.
///
/// **Lo que no hace, y es deliberado:** no toca las dos fuentes explícitas. Quien pasa `--ruta` o
/// escribe `ruta` en el TOML ha dicho a qué carpeta quiere ir, y preguntárselo encima sería no
/// escucharle. Así quien automatiza sigue pudiendo arrancar sin intervención, ponga lo que ponga
/// esta bandera.
pub fn decidir(
    de_la_linea: Option<&str>,
    de_la_config: Option<&str>,
    recordada: Option<&Path>,
    preguntar_siempre: bool,
) -> Option<(PathBuf, Origen)> {
    if let Some(ruta) = de_la_linea {
        return Some((PathBuf::from(ruta), Origen::LineaDeOrdenes));
    }
    if let Some(ruta) = de_la_config {
        return Some((PathBuf::from(ruta), Origen::Configuracion));
    }
    if preguntar_siempre {
        return None;
    }
    recordada.map(|ruta| (ruta.to_path_buf(), Origen::Recordada))
}

/// La última carpeta en la que se trabajó, si sigue existiendo.
///
/// Una ruta muerta no impide arrancar: deja de contar y se pasa a la fuente siguiente, que es el
/// diálogo. Cualquier fallo de lectura se trata igual, porque el resultado útil es el mismo.
pub fn recordada(junto_a: &Path) -> Option<PathBuf> {
    let texto = std::fs::read_to_string(junto_a.join(NOMBRE_DE_LA_MEMORIA)).ok()?;
    let ruta = PathBuf::from(texto.trim());
    if ruta.as_os_str().is_empty() || !ruta.is_dir() {
        return None;
    }
    Some(ruta)
}

/// Deja anotada la carpeta para el arranque siguiente.
///
/// **No devuelve error a propósito.** Que no se pueda escribir la memoria —una carpeta de solo
/// lectura, un antivirus— no es motivo para no trabajar: se avisa y se sigue.
pub fn recordar(junto_a: &Path, carpeta: &Path) {
    let destino = junto_a.join(NOMBRE_DE_LA_MEMORIA);
    if let Err(causa) = std::fs::write(&destino, carpeta.display().to_string()) {
        eprintln!("{}", advertencia_fallo_recordar(&destino, &causa));
    }
}

/// Redacta la advertencia emitida cuando no se pudo persistir la memoria de la última carpeta de trabajo.
///
/// Explica:
/// 1. Qué ha fallado: la escritura en disco de la memoria de carpeta.
/// 2. Consecuencia: el ciclo actual sigue con normalidad, pero en el próximo arranque no se recordará la carpeta.
/// 3. Qué hacer: comprobar permisos de escritura en la carpeta del ejecutable o fijar la ruta en `programator.toml`.
pub fn advertencia_fallo_recordar(destino: &Path, causa: &std::io::Error) -> String {
    format!(
        "⚠️ No se pudo guardar la memoria de la carpeta de trabajo en «{}»: {causa}.\n\
         Consecuencia: el ciclo actual continuará con normalidad, pero en el próximo arranque \
         habrá que volver a seleccionar la carpeta de trabajo o pasarla con «--ruta».\n\
         Qué hacer: si deseas que se recuerde automáticamente, comprueba los permisos de escritura \
         en ese directorio o fija la ruta en «programator.toml» o con «--ruta <DIRECTORIO>».",
        destino.display()
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn advertencia_fallo_recordar_explica_fallo_consecuencia_y_accion() {
        let ruta = Path::new("/invalido/memoria.txt");
        let err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denegado");
        let aviso = advertencia_fallo_recordar(ruta, &err);
        assert!(aviso.starts_with("⚠️ No se pudo guardar la memoria"));
        assert!(aviso.contains("Consecuencia:"));
        assert!(aviso.contains("el ciclo actual continuará con normalidad"));
        assert!(aviso.contains("Qué hacer:"));
    }

    #[test]
    fn la_linea_de_ordenes_manda_sobre_todo() {
        let decidida = decidir(
            Some("/linea"),
            Some("/config"),
            Some(Path::new("/memoria")),
            false,
        );
        assert_eq!(
            decidida,
            Some((PathBuf::from("/linea"), Origen::LineaDeOrdenes))
        );
    }

    #[test]
    fn sin_linea_manda_la_configuracion() {
        let decidida = decidir(None, Some("/config"), Some(Path::new("/memoria")), false);
        assert_eq!(
            decidida,
            Some((PathBuf::from("/config"), Origen::Configuracion))
        );
    }

    #[test]
    fn sin_linea_ni_configuracion_manda_la_recordada() {
        let decidida = decidir(None, None, Some(Path::new("/memoria")), false);
        assert_eq!(
            decidida,
            Some((PathBuf::from("/memoria"), Origen::Recordada))
        );
    }

    #[test]
    fn sin_ninguna_fuente_no_se_decide_nada() {
        // Es el único caso en que se abre el diálogo.
        assert_eq!(decidir(None, None, None, false), None);
    }

    #[test]
    fn preguntar_siempre_deja_sin_valor_la_carpeta_recordada() {
        // La memoria existe y es válida, pero el Director ha pedido que se le pregunte.
        let decidida = decidir(None, None, Some(Path::new("/memoria")), true);
        assert_eq!(decidida, None);
    }

    #[test]
    fn preguntar_siempre_no_estorba_a_la_linea_de_ordenes() {
        // Quien arranca con «--ruta» ya ha dicho a dónde va: preguntárselo sería no escucharle.
        let decidida = decidir(Some("/linea"), None, Some(Path::new("/memoria")), true);
        assert_eq!(
            decidida,
            Some((PathBuf::from("/linea"), Origen::LineaDeOrdenes))
        );
    }

    #[test]
    fn preguntar_siempre_no_estorba_a_la_configuracion() {
        // Igual con «ruta» en el TOML: es una elección explícita, no una memoria heredada.
        let decidida = decidir(None, Some("/config"), Some(Path::new("/memoria")), true);
        assert_eq!(
            decidida,
            Some((PathBuf::from("/config"), Origen::Configuracion))
        );
    }

    #[test]
    fn se_recuerda_y_se_vuelve_a_leer() {
        let junto_a = tempfile::tempdir().unwrap();
        let trabajo = tempfile::tempdir().unwrap();

        recordar(junto_a.path(), trabajo.path());

        assert_eq!(
            recordada(junto_a.path()),
            Some(trabajo.path().to_path_buf())
        );
    }

    #[test]
    fn una_carpeta_recordada_que_ya_no_existe_no_cuenta() {
        let junto_a = tempfile::tempdir().unwrap();
        let efimera = tempfile::tempdir().unwrap();
        let ruta = efimera.path().to_path_buf();
        recordar(junto_a.path(), &ruta);
        drop(efimera);

        assert_eq!(recordada(junto_a.path()), None);
    }

    #[test]
    fn sin_fichero_de_memoria_no_hay_carpeta_recordada() {
        let junto_a = tempfile::tempdir().unwrap();
        assert_eq!(recordada(junto_a.path()), None);
    }
}
