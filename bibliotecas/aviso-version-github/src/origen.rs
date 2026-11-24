//! Origen de las versiones: los ajustes ya validados y las direcciones que se componen con ellos.

use crate::ajustes::AjustesDeVersionNueva;
use crate::version::FalloDeComprobacion;
use std::num::NonZeroU64;
use std::time::Duration;

/// Esquema único que se admite en las direcciones.
const ESQUEMA: &str = "https://";

/// Caracteres que puede tener, como mucho, cada parte de un nombre de servidor.
// Fuente: RFC 1035, apartado 2.3.4, «labels 63 octets or less».
// https://www.rfc-editor.org/rfc/rfc1035#section-2.3.4
const LONGITUD_MAXIMA_DE_UNA_PARTE: usize = 63;

/// Caracteres que puede tener, como mucho, un nombre de servidor escrito sin punto final.
///
/// El RFC 1035 limita el nombre a 255 octetos en su forma de transmisión, que añade un octeto
/// de longitud delante de cada parte y uno nulo al final: escrito con puntos, quedan 253.
// Fuente: RFC 1035, apartado 2.3.4, «names 255 octets or less».
// https://www.rfc-editor.org/rfc/rfc1035#section-2.3.4
const LONGITUD_MAXIMA_DEL_SERVIDOR: usize = 253;

/// Dónde se pregunta por la última versión, dónde se enlaza y con qué límites, ya validado.
///
/// Solo se construye con desde_ajustes, que rechaza lo que no sea seguro: así, quien tiene uno
/// puede componer direcciones y preparar la consulta sin volver a comprobar nada. El tiempo
/// de espera y el límite de respuesta nunca son cero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrigenDeLasVersiones {
    /// Raíz de la API, sin barra final: el servidor api.github.com o, en GitHub Enterprise
    /// Server, el servidor seguido de la ruta api/v3.
    url_api: String,
    /// Raíz de la web, sin barra final, como el servidor github.com.
    url_web: String,
    /// Repositorio con la forma dueño/nombre.
    repositorio: String,
    /// Tiempo máximo de espera de la respuesta, en segundos.
    segundos_de_espera: NonZeroU64,
    /// Tamaño máximo de la respuesta, en bytes.
    limite_de_respuesta_en_bytes: NonZeroU64,
}

impl OrigenDeLasVersiones {
    /// Valida unos ajustes, que son texto que cualquiera puede haber editado a mano.
    ///
    /// Es la única forma de obtener un origen, y la consulta HTTPS solo se prepara a partir de
    /// uno, así que unos ajustes sin validar nunca llegan a la red.
    ///
    /// Devuelve FalloDeComprobacion::AjustesNoValidos si alguna dirección no es https con un
    /// nombre de servidor y, como mucho, una ruta limpia; si el repositorio no es dueño/nombre
    /// con caracteres seguros; o si el tiempo de espera o el límite de respuesta son cero.
    pub fn desde_ajustes(ajustes: &AjustesDeVersionNueva) -> Result<Self, FalloDeComprobacion> {
        let no_nulo = |valor: u64, que_es: &str| {
            NonZeroU64::new(valor)
                .ok_or_else(|| FalloDeComprobacion::AjustesNoValidos(format!("{que_es} es cero")))
        };
        let segundos_de_espera = no_nulo(ajustes.segundos_de_espera, "el tiempo de espera")?;
        let limite_de_respuesta_en_bytes = no_nulo(
            ajustes.limite_de_respuesta_en_bytes,
            "el límite de respuesta",
        )?;
        Ok(Self {
            url_api: raiz_https_valida(&ajustes.url_api)?,
            url_web: raiz_https_valida(&ajustes.url_web)?,
            repositorio: repositorio_valido(&ajustes.repositorio)?,
            segundos_de_espera,
            limite_de_respuesta_en_bytes,
        })
    }

    /// Tiempo máximo de espera de la respuesta, nunca cero.
    pub(crate) fn tiempo_de_espera(&self) -> Duration {
        Duration::from_secs(self.segundos_de_espera.get())
    }

    /// Tamaño máximo de la respuesta, en bytes, nunca cero.
    pub(crate) fn limite_de_respuesta_en_bytes(&self) -> u64 {
        self.limite_de_respuesta_en_bytes.get()
    }

    /// Dirección de la API que devuelve la última versión publicada.
    ///
    /// Es la operación «Get the latest release», que devuelve «the most recent non-prerelease,
    /// non-draft release».
    // Fuente: https://docs.github.com/en/rest/releases/releases#get-the-latest-release
    pub fn url_de_consulta(&self) -> String {
        format!(
            "{}/repos/{}/releases/latest",
            self.url_api, self.repositorio
        )
    }

    /// Página web de la versión con la etiqueta dada, que ya debe estar validada por
    /// VersionSemantica::desde_etiqueta.
    pub(crate) fn enlace_de_la_version(&self, etiqueta: &str) -> String {
        format!(
            "{}/{}/releases/tag/{etiqueta}",
            self.url_web, self.repositorio
        )
    }
}

/// Comprueba que una dirección es https seguida de un servidor y, si acaso, de una ruta limpia.
///
/// El servidor solo admite letras, dígitos, puntos y guiones, así que no caben credenciales
/// con arroba ni puerto con dos puntos. La ruta es opcional y existe para GitHub Enterprise
/// Server, cuya API vive en el servidor seguido de la ruta api/v3, como muestra el baseUrl del
/// ejemplo de Octokit en la guía de inicio rápido de su API REST. Cada tramo de la ruta
/// tiene al menos un carácter, solo letras, dígitos, puntos, guiones y guiones bajos, y no puede
/// ser un punto ni dos. Así quedan fuera la consulta, el fragmento, las secuencias de escape con
/// tanto por ciento, las barras invertidas, los espacios, los caracteres de control, las barras
/// dobles y cualquier retroceso. Se toleran barras al final.
///
/// Devuelve la dirección sin las barras finales.
// Fuente: https://docs.github.com/en/enterprise-server@latest/rest/quickstart
fn raiz_https_valida(url: &str) -> Result<String, FalloDeComprobacion> {
    let no_valida = || FalloDeComprobacion::AjustesNoValidos(format!("dirección «{url}»"));
    let resto = url
        .strip_prefix(ESQUEMA)
        .ok_or_else(no_valida)?
        .trim_end_matches('/');
    let mut tramos = resto.split('/');
    let servidor = tramos.next().unwrap_or_default();
    if !servidor_valido(servidor) || !tramos.all(tramo_de_ruta_valido) {
        return Err(no_valida());
    }
    Ok(format!("{ESQUEMA}{resto}"))
}

/// Un nombre de servidor: partes separadas por puntos, cada una de una a 63 letras, dígitos o
/// guiones, sin guion al principio ni al final, y 253 caracteres como mucho en total.
///
/// Así se rechazan un punto suelto, dos puntos seguidos, el punto final y las partes que
/// empiezan o acaban en guion, que la resolución de nombres no admitiría. Una dirección IP
/// como 1.2.3.4 cumple estas reglas y se sigue aceptando.
fn servidor_valido(servidor: &str) -> bool {
    servidor.len() <= LONGITUD_MAXIMA_DEL_SERVIDOR
        && servidor.split('.').all(parte_de_servidor_valida)
}

/// Una parte de un nombre de servidor: de una a 63 letras, dígitos o guiones, sin guion en
/// los extremos.
fn parte_de_servidor_valida(parte: &str) -> bool {
    (1..=LONGITUD_MAXIMA_DE_UNA_PARTE).contains(&parte.len())
        && !parte.starts_with('-')
        && !parte.ends_with('-')
        && parte
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Un tramo de ruta: no vacío, distinto de un punto y de dos, y solo con caracteres que no
/// cambian el sentido de la dirección.
fn tramo_de_ruta_valido(tramo: &str) -> bool {
    !tramo.is_empty()
        && tramo != "."
        && tramo != ".."
        && tramo
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
}

/// Comprueba que el repositorio es dueño/nombre con caracteres que GitHub admite.
fn repositorio_valido(repositorio: &str) -> Result<String, FalloDeComprobacion> {
    let no_valido =
        || FalloDeComprobacion::AjustesNoValidos(format!("repositorio «{repositorio}»"));
    let partes: Vec<&str> = repositorio.split('/').collect();
    let parte_valida = |parte: &&str| {
        !parte.is_empty()
            && !parte.starts_with('.')
            && parte
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
    };
    if partes.len() != 2 || !partes.iter().all(parte_valida) {
        return Err(no_valido());
    }
    Ok(repositorio.to_string())
}
