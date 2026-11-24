//! Integración de referencia de aviso-version-github en un programa.
//!
//! Es la forma única de conectar la biblioteca que describe documentacion/norma-de-integracion.md.
//! El módulo aviso_de_version se copia en el programa, se cambian sus dos constantes y se adapta
//! el tipo de las preferencias al archivo de configuración del programa. Cargo compila este
//! ejemplo, lo revisa con clippy y ejecuta sus pruebas con cargo test, así que la plantilla no
//! puede quedarse atrás respecto de la biblioteca sin que algo falle.
//!
//! Al ejecutarlo con cargo run --example integracion_de_referencia sí sale a la red: pregunta por
//! un repositorio de ejemplo que no existe y registra el fallo.

/// El único módulo del programa que habla con la biblioteca.
mod aviso_de_version {
    use aviso_version_github::{
        comprobar, proxy_del_sistema, AjustesDeVersionNueva, AvisoDeVersionNueva,
        ConsultaDeVersion, ConsultaHttps, FalloDeComprobacion, OrigenDeLasVersiones,
        ProgramaQueConsulta,
    };
    use serde::{Deserialize, Serialize};
    use std::fmt;
    use std::sync::{mpsc, Arc};
    use std::thread;

    /// Nombre con que el programa se presenta en la cabecera User-Agent. Cada programa pone el
    /// suyo: letras, cifras, puntos, guiones y guiones bajos, empezando por letra o cifra.
    pub const NOMBRE_DEL_PROGRAMA: &str = "programa-de-ejemplo";

    /// Repositorio dueño/nombre donde el programa publica sus versiones. Es el valor de fábrica
    /// de las preferencias; el usuario puede cambiarlo.
    pub const REPOSITORIO_DE_VERSIONES_POR_DEFECTO: &str = "dueno/programa-de-ejemplo";

    /// Nombre del hilo de trabajo, para reconocerlo en un depurador o en un volcado.
    const NOMBRE_DEL_HILO: &str = "aviso-de-version";

    /// Lo que el programa guarda en sus preferencias sobre el aviso de versión nueva.
    ///
    /// Los dos campos tienen estos nombres en todos los programas. Al leer unas preferencias
    /// sin ellos, la comprobación queda activada y con los ajustes de fábrica.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct PreferenciasDeVersion {
        /// Si el usuario quiere que el programa compruebe al arrancar si hay versión nueva.
        #[serde(default = "activada_por_defecto")]
        pub comprobar_version_nueva: bool,
        /// De qué repositorio y servidor se pregunta, y con qué límites.
        #[serde(default = "ajustes_de_fabrica")]
        pub ajustes_de_version: AjustesDeVersionNueva,
    }

    impl Default for PreferenciasDeVersion {
        /// Comprobación activada con los ajustes de fábrica.
        fn default() -> Self {
            Self {
                comprobar_version_nueva: activada_por_defecto(),
                ajustes_de_version: ajustes_de_fabrica(),
            }
        }
    }

    /// La comprobación viene activada de fábrica.
    fn activada_por_defecto() -> bool {
        true
    }

    /// Ajustes de fábrica: el repositorio del programa en github.com y los límites de la
    /// biblioteca.
    pub fn ajustes_de_fabrica() -> AjustesDeVersionNueva {
        AjustesDeVersionNueva::para_repositorio(REPOSITORIO_DE_VERSIONES_POR_DEFECTO)
    }

    /// Dónde deja el programa los fallos. El programa real pasa su sistema de registro; nunca
    /// se muestran al usuario como un error.
    pub type Registro = Arc<dyn Fn(&dyn fmt::Display) + Send + Sync>;

    /// Lo que llega por el canal cuando termina la comprobación: el aviso, o nada si el programa
    /// ya está al día o la comprobación falló (y el fallo ya se registró).
    pub type ResultadoDeVersion = Option<AvisoDeVersionNueva>;

    /// La consulta real: HTTPS, con el proxy del sistema y el programa identificado.
    ///
    /// Es la que pasa el programa como fabricar. Las pruebas pasan una consulta falsa.
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
    /// resultado, o None si no hay nada que esperar.
    ///
    /// Con la opción desactivada, o con unos ajustes que no superan la validación, no se llama a
    /// fabricar: no llega a existir nada capaz de salir a la red. Un fallo de validación, de red
    /// o de respuesta se registra y nunca provoca un pánico. El hilo de la interfaz no espera:
    /// recoge el resultado con try_recv cuando le venga bien.
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
        let registrar_en_el_hilo = Arc::clone(&registrar);
        let hilo = thread::Builder::new()
            .name(NOMBRE_DEL_HILO.to_string())
            .spawn(move || {
                let aviso = decidir(
                    comprobar(version_local, &origen, consulta.as_ref()),
                    &registrar_en_el_hilo,
                );
                // Si el programa ya cerró el receptor, nadie espera el resultado: no es un fallo.
                let _ = enviar.send(aviso);
            });
        match hilo {
            Ok(_) => Some(recibir),
            Err(error) => {
                registrar(&error);
                None
            }
        }
    }

    /// Convierte el resultado de la biblioteca en lo que espera la interfaz, registrando el
    /// fallo si lo hay.
    fn decidir(
        resultado: Result<Option<AvisoDeVersionNueva>, FalloDeComprobacion>,
        registrar: &Registro,
    ) -> ResultadoDeVersion {
        resultado.unwrap_or_else(|fallo| {
            registrar(&fallo);
            None
        })
    }
}

fn main() {
    use std::sync::Arc;

    let preferencias = aviso_de_version::PreferenciasDeVersion::default();
    let registro: aviso_de_version::Registro = Arc::new(|fallo: &dyn std::fmt::Display| {
        eprintln!("comprobación de versión nueva: {fallo}");
    });
    let Some(recibir) = aviso_de_version::arrancar(
        &preferencias,
        env!("CARGO_PKG_VERSION"),
        aviso_de_version::consulta_real,
        registro,
    ) else {
        return;
    };
    // Un programa con interfaz llamaría a try_recv en cada repintado; aquí basta con esperar.
    if let Ok(Some(aviso)) = recibir.recv() {
        println!(
            "Hay una versión nueva, la {}: {}",
            aviso.version, aviso.enlace
        );
    }
}

/// Las cinco pruebas mínimas que la norma exige a toda integración, más la de las constantes.
/// Ninguna sale a la red.
#[cfg(test)]
mod pruebas {
    use super::aviso_de_version::{
        arrancar, PreferenciasDeVersion, Registro, NOMBRE_DEL_PROGRAMA,
        REPOSITORIO_DE_VERSIONES_POR_DEFECTO,
    };
    use aviso_version_github::{
        AjustesDeVersionNueva, ConsultaDeVersion, FalloDeComprobacion, OrigenDeLasVersiones,
        ProgramaQueConsulta,
    };
    use std::cell::Cell;
    use std::sync::{Arc, Mutex};

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

    /// Un registro que guarda lo que recibe, para comprobarlo.
    fn registro_de_prueba() -> (Registro, Arc<Mutex<Vec<String>>>) {
        let apuntes = Arc::new(Mutex::new(Vec::new()));
        let destino = Arc::clone(&apuntes);
        let registro: Registro = Arc::new(move |fallo: &dyn std::fmt::Display| {
            destino
                .lock()
                .expect("registro no envenenado")
                .push(fallo.to_string());
        });
        (registro, apuntes)
    }

    /// Arranca con las preferencias y la respuesta dadas y devuelve si se fabricó la consulta,
    /// lo que llegó por el canal (None si no hubo canal) y lo registrado.
    fn arrancar_con(
        preferencias: &PreferenciasDeVersion,
        respuesta: Result<String, FalloDeComprobacion>,
    ) -> (bool, Option<Option<String>>, Vec<String>) {
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
        let llegado = canal.map(|recibir| {
            recibir
                .recv()
                .expect("el hilo tiene que enviar un resultado")
                .map(|aviso| aviso.version)
        });
        let registrado = apuntes.lock().expect("registro no envenenado").clone();
        (fabricada.get(), llegado, registrado)
    }

    /// 1. Con la opción desactivada no se fabrica ninguna consulta.
    #[test]
    fn desactivada_no_fabrica_la_consulta() {
        let preferencias = PreferenciasDeVersion {
            comprobar_version_nueva: false,
            ..PreferenciasDeVersion::default()
        };
        assert_eq!(
            arrancar_con(&preferencias, publicada("v9.0.0")),
            (false, None, Vec::new())
        );
    }

    /// 2. Con ajustes no válidos no se fabrica la consulta y el fallo se registra.
    #[test]
    fn ajustes_no_validos_se_registran_sin_fabricar() {
        let preferencias = PreferenciasDeVersion {
            ajustes_de_version: AjustesDeVersionNueva::para_repositorio(""),
            ..PreferenciasDeVersion::default()
        };
        let (fabricada, llegado, registrado) = arrancar_con(&preferencias, publicada("v9.0.0"));
        assert!(!fabricada && llegado.is_none());
        assert_eq!(
            registrado.len(),
            1,
            "el fallo tiene que registrarse: {registrado:?}"
        );
    }

    /// 3. Una versión publicada mayor llega como aviso.
    #[test]
    fn version_mayor_llega_como_aviso() {
        let (fabricada, llegado, registrado) =
            arrancar_con(&PreferenciasDeVersion::default(), publicada("v1.3.0"));
        assert!(fabricada && registrado.is_empty());
        assert_eq!(llegado, Some(Some("1.3.0".to_string())));
    }

    /// 4. La misma versión no produce aviso ni fallo.
    #[test]
    fn misma_version_no_avisa() {
        let (_, llegado, registrado) =
            arrancar_con(&PreferenciasDeVersion::default(), publicada("v1.2.0"));
        assert_eq!(llegado, Some(None));
        assert!(registrado.is_empty());
    }

    /// 5. Un fallo de red no avisa, se registra y no provoca un pánico.
    #[test]
    fn fallo_de_red_se_registra_sin_avisar() {
        let (_, llegado, registrado) = arrancar_con(
            &PreferenciasDeVersion::default(),
            Err(FalloDeComprobacion::Red("sin conexión".into())),
        );
        assert_eq!(llegado, Some(None));
        assert_eq!(
            registrado.len(),
            1,
            "el fallo tiene que registrarse: {registrado:?}"
        );
    }

    /// Las constantes del programa son válidas para la biblioteca, y las preferencias leídas
    /// sin estos campos quedan activadas con los ajustes de fábrica.
    #[test]
    fn las_constantes_y_los_valores_de_fabrica_son_validos() {
        assert!(ProgramaQueConsulta::nuevo(NOMBRE_DEL_PROGRAMA, "1.0.0").is_ok());
        assert!(
            OrigenDeLasVersiones::desde_ajustes(&AjustesDeVersionNueva::para_repositorio(
                REPOSITORIO_DE_VERSIONES_POR_DEFECTO
            ))
            .is_ok()
        );
        let leidas: PreferenciasDeVersion = serde_json::from_str("{}").expect("JSON válido");
        assert_eq!(leidas, PreferenciasDeVersion::default());
    }
}
