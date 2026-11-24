//! Aviso, puramente informativo, de si hay una versión más nueva de Programator en GitHub.
//!
//! Es el único módulo que habla con la biblioteca `aviso-version-github`, copiada en
//! `bibliotecas/aviso-version-github` y conectada según su `documentacion/norma-de-integracion.md`
//! (el módulo `aviso_de_version` de su `examples/integracion_de_referencia.rs`).
//!
//! **Solo avisa.** No descarga nada, no reemplaza el ejecutable y no decide nada por el usuario.
//! Y **nunca puede impedir que Programator arranque ni atienda encargos**: la consulta va en un
//! hilo de trabajo mientras el arranque mide la máquina, y sin red, con GitHub caído o con una
//! respuesta que no se entiende, el arranque sigue igual y la consola dice que no se pudo
//! comprobar.
//!
//! Dos diferencias con la referencia, anotadas en el registro común de proyectos y bibliotecas:
//! - Los ajustes siguen en `[actualizaciones]` del TOML (`comprobar`, `repositorio`,
//!   `tiempo_espera_segundos`), con los nombres de siempre, y se traducen aquí a los de la
//!   biblioteca.
//! - El fallo de la consulta viaja por el canal en vez de registrarse desde el hilo: el registro
//!   de Programator es su consola, y escribir desde el hilo lo metería en mitad del informe de
//!   arranque.

use crate::config::Actualizaciones;
use aviso_version_github::{
    comprobar, proxy_del_sistema, AjustesDeVersionNueva, AvisoDeVersionNueva, ConsultaDeVersion,
    ConsultaHttps, FalloDeComprobacion, OrigenDeLasVersiones, ProgramaQueConsulta,
};
use std::fmt;
use std::sync::{mpsc, Arc};
use std::thread;

/// Nombre con que Programator se presenta en la cabecera User-Agent.
pub const NOMBRE_DEL_PROGRAMA: &str = "programator";

/// Repositorio donde se publican las versiones de Programator (01/10/2026). Es el valor de
/// fábrica de `[actualizaciones] repositorio`.
pub const REPOSITORIO_DE_VERSIONES_POR_DEFECTO: &str = "celtidcs/Programator";

/// Nombre del hilo de trabajo, para reconocerlo en un depurador o en un volcado.
const NOMBRE_DEL_HILO: &str = "aviso-de-version";

/// Lo que dice la configuración sobre el aviso, con los nombres de la norma de la biblioteca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenciasDeVersion {
    /// Si se comprueba al arrancar (`[actualizaciones] comprobar`).
    pub comprobar_version_nueva: bool,
    /// De qué repositorio se pregunta y cuánto se espera.
    pub ajustes_de_version: AjustesDeVersionNueva,
}

impl PreferenciasDeVersion {
    /// Traduce `[actualizaciones]` a los ajustes de la biblioteca: el repositorio y el tiempo de
    /// espera salen del TOML; las direcciones de GitHub y el límite de la respuesta, de la
    /// biblioteca.
    pub fn desde_la_configuracion(actualizaciones: &Actualizaciones) -> Self {
        let mut ajustes = AjustesDeVersionNueva::para_repositorio(&actualizaciones.repositorio);
        ajustes.segundos_de_espera = actualizaciones.tiempo_espera_segundos;
        Self {
            comprobar_version_nueva: actualizaciones.comprobar,
            ajustes_de_version: ajustes,
        }
    }
}

/// Dónde deja Programator un fallo que no viaja por el canal: la consola, como aviso.
pub type Registro = Arc<dyn Fn(&dyn fmt::Display) + Send + Sync>;

/// Lo que llega por el canal cuando termina la comprobación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultadoDeVersion {
    /// Hay una versión publicada más nueva que la instalada.
    VersionNueva(AvisoDeVersionNueva),
    /// La publicada es igual o anterior a la instalada.
    SinVersionNueva,
    /// No se pudo saber: el texto técnico del fallo, para registrarlo.
    NoComprobado(String),
}

/// La consulta real: HTTPS, con el proxy del sistema y Programator identificado.
pub fn consulta_real(origen: &OrigenDeLasVersiones) -> Box<dyn ConsultaDeVersion> {
    match ProgramaQueConsulta::nuevo(NOMBRE_DEL_PROGRAMA, env!("CARGO_PKG_VERSION")) {
        Ok(programa) => Box::new(ConsultaHttps::para_programa(
            origen,
            proxy_del_sistema(),
            &programa,
        )),
        // No ocurre con un nombre constante válido, que una prueba vigila; si ocurriera, la
        // consulta sale solo con el nombre de la biblioteca.
        Err(_) => Box::new(ConsultaHttps::nueva(origen, proxy_del_sistema())),
    }
}

/// Arranca la comprobación en un hilo de trabajo y devuelve el canal por el que llegará el
/// resultado, o `None` si no hay nada que esperar.
///
/// Con la opción desactivada, o con unos ajustes que no superan la validación, no se llama a
/// `fabricar`: no llega a existir nada capaz de salir a la red. El fallo de validación y el de
/// crear el hilo se registran aquí; el de la consulta llega por el canal. Nada provoca un pánico.
pub fn arrancar<F>(
    preferencias: &PreferenciasDeVersion,
    version_local: &'static str,
    fabricar: F,
    registrar: Registro,
) -> Option<mpsc::Receiver<ResultadoDeVersion>>
where
    F: FnOnce(&OrigenDeLasVersiones) -> Box<dyn ConsultaDeVersion>,
{
    if !preferencias.comprobar_version_nueva {
        return None;
    }
    let origen = match OrigenDeLasVersiones::desde_ajustes(&preferencias.ajustes_de_version) {
        Ok(origen) => origen,
        Err(fallo) => {
            registrar(&fallo);
            return None;
        }
    };
    let consulta = fabricar(&origen);
    let (enviar, recibir) = mpsc::channel();
    let hilo = thread::Builder::new()
        .name(NOMBRE_DEL_HILO.to_string())
        .spawn(move || {
            let resultado = decidir(comprobar(version_local, &origen, consulta.as_ref()));
            // Si el arranque ya cerró el receptor, nadie espera el resultado: no es un fallo.
            let _ = enviar.send(resultado);
        });
    match hilo {
        Ok(_) => Some(recibir),
        Err(error) => {
            registrar(&error);
            None
        }
    }
}

/// Convierte el resultado de la biblioteca en lo que espera el arranque.
fn decidir(
    resultado: Result<Option<AvisoDeVersionNueva>, FalloDeComprobacion>,
) -> ResultadoDeVersion {
    match resultado {
        Ok(Some(aviso)) => ResultadoDeVersion::VersionNueva(aviso),
        Ok(None) => ResultadoDeVersion::SinVersionNueva,
        Err(fallo) => ResultadoDeVersion::NoComprobado(fallo.to_string()),
    }
}

/// Lo que dice la consola de un resultado. El fallo no se cuenta aquí: va al registro.
pub fn mensaje(version_local: &str, resultado: &ResultadoDeVersion) -> Option<String> {
    match resultado {
        ResultadoDeVersion::VersionNueva(aviso) => Some(format!(
            "📦 Hay una versión nueva de Programator, la {} (tienes la {version_local}). Está en {}",
            aviso.version, aviso.enlace
        )),
        ResultadoDeVersion::SinVersionNueva => Some(format!(
            "✅ No hay en GitHub ninguna versión de Programator más nueva que la {version_local}."
        )),
        ResultadoDeVersion::NoComprobado(_) => None,
    }
}

/// El aviso de que no se pudo comprobar, con su causa técnica para quien quiera saberla.
pub fn mensaje_no_comprobado(causa: &dyn fmt::Display) -> String {
    format!(
        "⚠️ No se pudo comprobar si hay una versión más nueva de Programator en GitHub. No afecta \
         al arranque y no hace falta hacer nada. Causa: {causa}"
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;
    use std::sync::Mutex;

    /// Versión local de las pruebas.
    const VERSION_LOCAL: &str = "1.2.0";

    /// Consulta falsa que devuelve siempre la misma respuesta.
    struct ConsultaFalsa(Result<String, FalloDeComprobacion>);

    impl ConsultaDeVersion for ConsultaFalsa {
        fn pedir(&self, _url: &str) -> Result<String, FalloDeComprobacion> {
            self.0.clone()
        }
    }

    /// Respuesta de la API con la etiqueta dada.
    fn publicada(etiqueta: &str) -> Result<String, FalloDeComprobacion> {
        Ok(format!(r#"{{"tag_name":"{etiqueta}"}}"#))
    }

    fn de_fabrica() -> PreferenciasDeVersion {
        PreferenciasDeVersion::desde_la_configuracion(&Actualizaciones::default())
    }

    /// Un registro que guarda lo que recibe, para comprobarlo.
    fn registro_de_prueba() -> (Registro, Arc<Mutex<Vec<String>>>) {
        let apuntes = Arc::new(Mutex::new(Vec::new()));
        let destino = Arc::clone(&apuntes);
        let registro: Registro = Arc::new(move |fallo: &dyn fmt::Display| {
            destino.lock().unwrap().push(fallo.to_string());
        });
        (registro, apuntes)
    }

    /// Arranca con las preferencias y la respuesta dadas y devuelve si se fabricó la consulta,
    /// lo que llegó por el canal (`None` si no hubo canal) y lo registrado.
    fn arrancar_con(
        preferencias: &PreferenciasDeVersion,
        respuesta: Result<String, FalloDeComprobacion>,
    ) -> (bool, Option<ResultadoDeVersion>, Vec<String>) {
        let fabricada = Cell::new(false);
        let (registro, apuntes) = registro_de_prueba();
        let canal = arrancar(
            preferencias,
            VERSION_LOCAL,
            |_origen: &OrigenDeLasVersiones| -> Box<dyn ConsultaDeVersion> {
                fabricada.set(true);
                Box::new(ConsultaFalsa(respuesta))
            },
            registro,
        );
        let llegado = canal.map(|recibir| recibir.recv().unwrap());
        let registrado = apuntes.lock().unwrap().clone();
        (fabricada.get(), llegado, registrado)
    }

    /// Norma N10.1: con la opción desactivada no se fabrica ninguna consulta.
    #[test]
    fn desactivada_no_fabrica_la_consulta() {
        let preferencias = PreferenciasDeVersion {
            comprobar_version_nueva: false,
            ..de_fabrica()
        };
        assert_eq!(
            arrancar_con(&preferencias, publicada("v9.0.0")),
            (false, None, Vec::new())
        );
    }

    /// Norma N10.2: con ajustes no válidos no se fabrica la consulta y el fallo se registra.
    #[test]
    fn ajustes_no_validos_se_registran_sin_fabricar() {
        let preferencias = PreferenciasDeVersion::desde_la_configuracion(&Actualizaciones {
            tiempo_espera_segundos: 0,
            ..Actualizaciones::default()
        });
        let (fabricada, llegado, registrado) = arrancar_con(&preferencias, publicada("v9.0.0"));
        assert!(!fabricada && llegado.is_none());
        assert_eq!(registrado.len(), 1, "{registrado:?}");
    }

    /// Norma N10.3: una versión publicada mayor llega como aviso, con el enlace a su página.
    #[test]
    fn version_mayor_llega_como_aviso() {
        let (fabricada, llegado, registrado) = arrancar_con(&de_fabrica(), publicada("v1.3.0"));
        assert!(fabricada && registrado.is_empty());
        let Some(ResultadoDeVersion::VersionNueva(aviso)) = llegado else {
            panic!("se esperaba un aviso: {llegado:?}");
        };
        assert_eq!(aviso.version, "1.3.0");
        assert!(
            aviso
                .enlace
                .starts_with("https://github.com/celtidcs/Programator/"),
            "{}",
            aviso.enlace
        );
    }

    /// Norma N10.4: la misma versión no avisa ni registra nada.
    #[test]
    fn misma_version_no_avisa() {
        let (_, llegado, registrado) = arrancar_con(&de_fabrica(), publicada("v1.2.0"));
        assert_eq!(llegado, Some(ResultadoDeVersion::SinVersionNueva));
        assert!(registrado.is_empty());
    }

    /// Norma N10.5: un fallo de red no avisa, llega con su causa y no provoca un pánico.
    #[test]
    fn fallo_de_red_llega_con_su_causa_sin_avisar() {
        let (_, llegado, registrado) = arrancar_con(
            &de_fabrica(),
            Err(FalloDeComprobacion::Red("sin conexión".into())),
        );
        let Some(ResultadoDeVersion::NoComprobado(causa)) = llegado else {
            panic!("se esperaba el fallo: {llegado:?}");
        };
        assert!(causa.contains("sin conexión"), "{causa}");
        assert!(registrado.is_empty());
    }

    /// Las constantes son válidas para la biblioteca y `[actualizaciones]` llega entero.
    #[test]
    fn las_constantes_y_la_configuracion_llegan_a_la_biblioteca() {
        assert!(ProgramaQueConsulta::nuevo(NOMBRE_DEL_PROGRAMA, "1.0.0").is_ok());
        let preferencias = PreferenciasDeVersion::desde_la_configuracion(&Actualizaciones {
            comprobar: true,
            repositorio: "otro/repositorio".to_string(),
            tiempo_espera_segundos: 7,
        });
        assert!(preferencias.comprobar_version_nueva);
        assert_eq!(
            preferencias.ajustes_de_version.repositorio,
            "otro/repositorio"
        );
        assert_eq!(preferencias.ajustes_de_version.segundos_de_espera, 7);
        let apagada = PreferenciasDeVersion::desde_la_configuracion(&Actualizaciones {
            comprobar: false,
            ..Actualizaciones::default()
        });
        assert!(!apagada.comprobar_version_nueva);
        assert!(OrigenDeLasVersiones::desde_ajustes(&de_fabrica().ajustes_de_version).is_ok());
        assert_eq!(
            de_fabrica().ajustes_de_version.repositorio,
            REPOSITORIO_DE_VERSIONES_POR_DEFECTO
        );
    }

    #[test]
    fn la_consola_cuenta_cada_resultado_menos_el_fallo() {
        let aviso = AvisoDeVersionNueva {
            version: "1.3.0".to_string(),
            enlace: "https://github.com/celtidcs/Programator/releases/tag/v1.3.0".to_string(),
        };
        let nueva = mensaje("1.2.0", &ResultadoDeVersion::VersionNueva(aviso)).unwrap();
        assert!(
            nueva.contains("la 1.3.0") && nueva.contains("tienes la 1.2.0"),
            "{nueva}"
        );
        assert!(nueva.contains("/releases/tag/v1.3.0"), "{nueva}");

        // Con una versión instalada más nueva que la publicada, «ya es la más reciente publicada»
        // no era verdad. Esta frase vale para los dos casos.
        let sin = mensaje("1.0.0", &ResultadoDeVersion::SinVersionNueva).unwrap();
        assert!(
            sin.contains("ninguna versión") && sin.contains("1.0.0"),
            "{sin}"
        );

        assert_eq!(
            mensaje("1.2.0", &ResultadoDeVersion::NoComprobado("x".into())),
            None
        );
    }

    #[test]
    fn el_aviso_de_fallo_da_la_causa_y_dice_que_no_hace_falta_actuar() {
        let texto = mensaje_no_comprobado(&"tiempo agotado");
        assert!(texto.contains("No se pudo comprobar"), "{texto}");
        assert!(texto.contains("no hace falta hacer nada"), "{texto}");
        assert!(texto.contains("Causa: tiempo agotado"), "{texto}");
    }
}
