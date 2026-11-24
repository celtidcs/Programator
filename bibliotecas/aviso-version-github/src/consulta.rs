//! Consulta real por HTTPS y comprobación completa.
//!
//! Garantías de la consulta real. Solo HTTPS verificado: usa rustls con las raíces de
//! webpki-roots y rechaza cualquier dirección que no sea https. Lectura acotada: no lee más
//! bytes de los que permiten los ajustes. Respeta el proxy: las variables HTTPS_PROXY,
//! HTTP_PROXY y ALL_PROXY, con las excepciones de NO_PROXY, y, en Windows, el proxy de las
//! opciones de internet si no hay ninguna de esas variables. No interpreta archivos PAC ni
//! WPAD ni se autentica ante el proxy con NTLM o Kerberos.
//!
//! La consulta bloquea mientras espera: quien tenga una interfaz debe llamarla desde un hilo
//! de trabajo.

use crate::origen::OrigenDeLasVersiones;
use crate::programa::ProgramaQueConsulta;
use crate::version::{
    decidir_aviso, interpretar_respuesta, AvisoDeVersionNueva, FalloDeComprobacion,
    VersionSemantica,
};

/// Cabecera Accept que recomienda GitHub para su API REST.
// Fuente: https://docs.github.com/en/rest/using-the-rest-api/getting-started-with-the-rest-api#media-types
const TIPO_DE_RESPUESTA_DE_GITHUB: &str = "application/vnd.github+json";

/// Quién pide la versión: GitHub exige una cabecera User-Agent en toda petición a su API.
///
/// Es la cabecera completa cuando el programa no se identifica, y el último producto cuando sí.
// Fuente: https://docs.github.com/en/rest/using-the-rest-api/getting-started-with-the-rest-api#user-agent
const AGENTE_DE_USUARIO: &str = concat!("aviso-version-github/", env!("CARGO_PKG_VERSION"));

/// Algo capaz de pedir una dirección y devolver el cuerpo de la respuesta.
///
/// Existe para poder probar la decisión sin red: las pruebas usan una consulta falsa y el
/// programa, ConsultaHttps.
pub trait ConsultaDeVersion: Send {
    /// Pide la dirección y devuelve el cuerpo de la respuesta como texto, o
    /// FalloDeComprobacion::Red si no se obtuvo una respuesta correcta.
    fn pedir(&self, url: &str) -> Result<String, FalloDeComprobacion>;
}

/// Consulta real por HTTPS con ureq y rustls.
pub struct ConsultaHttps {
    /// Cliente ya configurado: solo HTTPS, tiempo de espera, cabecera User-Agent y proxy.
    agente: ureq::Agent,
    /// Tamaño máximo del cuerpo que se acepta leer.
    limite_de_respuesta_en_bytes: u64,
}

impl ConsultaHttps {
    /// Prepara el cliente con el origen ya validado y el proxy por el que salir.
    ///
    /// El proxy se recibe en vez de leerlo aquí para poder comprobar, sin red ni registro de
    /// Windows, que el cliente usa el que se le da. Lo normal es pasarle proxy_del_sistema().
    /// Del origen salen el tiempo de espera y el límite de respuesta, que nunca son cero; None
    /// como proxy conecta directamente. La cabecera User-Agent solo nombra a la biblioteca.
    pub fn nueva(origen: &OrigenDeLasVersiones, proxy: Option<ureq::Proxy>) -> Self {
        Self::con_cabecera(origen, proxy, AGENTE_DE_USUARIO.to_string())
    }

    /// Como nueva, pero la cabecera User-Agent presenta primero al programa, con su nombre y
    /// su versión, y después a la biblioteca, como mi-programa/1.2.0 aviso-version-github/0.2.0.
    pub fn para_programa(
        origen: &OrigenDeLasVersiones,
        proxy: Option<ureq::Proxy>,
        programa: &ProgramaQueConsulta,
    ) -> Self {
        let cabecera = format!("{} {AGENTE_DE_USUARIO}", programa.producto());
        Self::con_cabecera(origen, proxy, cabecera)
    }

    /// Prepara el cliente con la cabecera User-Agent dada, ya compuesta con partes validadas.
    fn con_cabecera(
        origen: &OrigenDeLasVersiones,
        proxy: Option<ureq::Proxy>,
        cabecera_de_usuario: String,
    ) -> Self {
        let configuracion = ureq::Agent::config_builder()
            .proxy(proxy)
            .https_only(true)
            .timeout_global(Some(origen.tiempo_de_espera()))
            .user_agent(cabecera_de_usuario)
            .build();
        Self {
            agente: configuracion.into(),
            limite_de_respuesta_en_bytes: origen.limite_de_respuesta_en_bytes(),
        }
    }
}

/// El proxy que lleva configurado una consulta, para que las pruebas lo comprueben.
#[cfg(test)]
pub(crate) fn proxy_configurado(consulta: &ConsultaHttps) -> Option<&ureq::Proxy> {
    consulta.agente.config().proxy()
}

/// La cabecera User-Agent, si se dio una, el tiempo de espera y el límite que lleva
/// configurados una consulta, para que las pruebas los comprueben.
#[cfg(test)]
pub(crate) fn configuracion_de(
    consulta: &ConsultaHttps,
) -> (Option<String>, Option<std::time::Duration>, u64) {
    let configuracion = consulta.agente.config();
    let cabecera = match configuracion.user_agent() {
        ureq::config::AutoHeaderValue::Provided(texto) => Some(texto.to_string()),
        _ => None,
    };
    (
        cabecera,
        configuracion.timeouts().global,
        consulta.limite_de_respuesta_en_bytes,
    )
}

/// El proxy del sistema, tal como lo entiende ureq.
///
/// Mira, por este orden, ALL_PROXY, HTTPS_PROXY y HTTP_PROXY, en mayúsculas o minúsculas, y,
/// solo en Windows y si ninguna está definida, el proxy activado en las opciones de internet,
/// que el sistema guarda en el registro del usuario (Internet Settings, valores ProxyEnable y
/// ProxyServer). Las excepciones salen de NO_PROXY o, si no está, de ProxyOverride. Es lo que
/// hace Proxy::try_from_env de ureq con la característica win-system-proxy, que el manifiesto
/// activa solo en Windows.
// Fuente: https://docs.rs/crate/ureq/3.4.2/source/src/proxy.rs
pub fn proxy_del_sistema() -> Option<ureq::Proxy> {
    ureq::Proxy::try_from_env()
}

impl ConsultaDeVersion for ConsultaHttps {
    /// Hace la petición GET y lee el cuerpo, como mucho hasta el límite configurado.
    ///
    /// Un estado HTTP de error, como 404 o 403 por límite de peticiones, llega como error,
    /// porque ureq los trata así por defecto.
    fn pedir(&self, url: &str) -> Result<String, FalloDeComprobacion> {
        let red = |error: ureq::Error| FalloDeComprobacion::Red(error.to_string());
        let mut respuesta = self
            .agente
            .get(url)
            .header("Accept", TIPO_DE_RESPUESTA_DE_GITHUB)
            .call()
            .map_err(red)?;
        respuesta
            .body_mut()
            .with_config()
            .limit(self.limite_de_respuesta_en_bytes)
            .read_to_string()
            .map_err(red)
    }
}

/// Pregunta por la última versión y decide si hay que avisar.
///
/// Recibe la versión que se está ejecutando, el origen ya validado y quien hace la petición.
/// Devuelve cualquier FalloDeComprobacion de la consulta o de la respuesta como error, nunca
/// como pánico: un pánico en un hilo de trabajo puede cerrar el programa entero si se compila
/// con panic = "abort".
pub fn comprobar(
    version_local: &str,
    origen: &OrigenDeLasVersiones,
    consulta: &dyn ConsultaDeVersion,
) -> Result<Option<AvisoDeVersionNueva>, FalloDeComprobacion> {
    let etiqueta = pedir_etiqueta_publicada(origen, consulta)?;
    decidir_aviso(version_local, &etiqueta, origen)
}

/// Pregunta por la última versión estable publicada, sin compararla con ninguna otra.
///
/// Recibe el origen ya validado y quien hace la petición. Sirve, por ejemplo, para mostrar
/// cuál es la última versión de otro programa. Devuelve la versión, o FalloDeComprobacion si
/// falla la red, si la respuesta no se puede leer o si la etiqueta no tiene la forma X.Y.Z.
/// Como comprobar, nunca provoca un pánico y bloquea mientras espera la red.
pub fn ultima_version(
    origen: &OrigenDeLasVersiones,
    consulta: &dyn ConsultaDeVersion,
) -> Result<VersionSemantica, FalloDeComprobacion> {
    let etiqueta = pedir_etiqueta_publicada(origen, consulta)?;
    VersionSemantica::desde_etiqueta(&etiqueta)
        .ok_or(FalloDeComprobacion::EtiquetaNoValida(etiqueta))
}

/// Pide la última versión publicada y devuelve su etiqueta tal como llega, sin validar su
/// forma, o el fallo de la red o de la respuesta.
fn pedir_etiqueta_publicada(
    origen: &OrigenDeLasVersiones,
    consulta: &dyn ConsultaDeVersion,
) -> Result<String, FalloDeComprobacion> {
    let cuerpo = consulta.pedir(&origen.url_de_consulta())?;
    interpretar_respuesta(&cuerpo)
}
