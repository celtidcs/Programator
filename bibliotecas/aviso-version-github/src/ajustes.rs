//! Ajustes de la comprobación: de qué repositorio, en qué servidor y con qué límites.

use serde::{Deserialize, Serialize};

/// Raíz de la API REST de GitHub.
pub const URL_API_DE_GITHUB: &str = "https://api.github.com";

/// Raíz de la web de GitHub, para el enlace del aviso.
pub const URL_WEB_DE_GITHUB: &str = "https://github.com";

/// Segundos que se espera, como mucho, la respuesta.
///
/// Diez segundos sobran para una petición de unos pocos kilobytes incluso en una conexión
/// lenta, y la espera debe ocurrir en un hilo aparte del de la interfaz.
pub const SEGUNDOS_DE_ESPERA_POR_DEFECTO: u64 = 10;

/// Bytes que se aceptan leer, como mucho, de la respuesta.
///
/// La respuesta de releases/latest ocupa unos pocos kilobytes, con las notas de la versión y la
/// lista de archivos. Un mebibyte deja un margen holgado y, a la vez, impide que una respuesta
/// anómala consuma memoria sin límite.
pub const LIMITE_DE_RESPUESTA_POR_DEFECTO: u64 = 1024 * 1024;

/// Dónde y cómo se pregunta por la última versión publicada.
///
/// Pensado para guardarse en las preferencias del programa, que son texto que cualquiera puede
/// editar a mano: por eso se valida antes de usarse, con OrigenDeLasVersiones::desde_ajustes.
/// Al leerlo, cada campo que falte toma su valor por defecto, salvo el repositorio, que queda
/// vacío y hace que la validación lo rechace, porque la biblioteca no conoce ningún repositorio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AjustesDeVersionNueva {
    /// Repositorio con la forma dueño/nombre.
    #[serde(default)]
    pub repositorio: String,
    /// Raíz de la API, solo https, con una ruta limpia si hace falta.
    #[serde(default = "url_api_de_github")]
    pub url_api: String,
    /// Raíz de la web, solo https, con una ruta limpia si hace falta.
    #[serde(default = "url_web_de_github")]
    pub url_web: String,
    /// Tiempo máximo de espera de la respuesta, en segundos. Cero no es válido.
    #[serde(default = "segundos_de_espera_por_defecto")]
    pub segundos_de_espera: u64,
    /// Tamaño máximo de la respuesta, en bytes. Cero no es válido.
    #[serde(default = "limite_de_respuesta_por_defecto")]
    pub limite_de_respuesta_en_bytes: u64,
}

impl AjustesDeVersionNueva {
    /// Ajustes para un repositorio de github.com, con los demás valores por defecto.
    ///
    /// El parámetro repositorio tiene la forma dueño/nombre. Lo que devuelve supera la
    /// validación si el repositorio la supera.
    pub fn para_repositorio(repositorio: &str) -> Self {
        Self {
            repositorio: repositorio.to_string(),
            url_api: url_api_de_github(),
            url_web: url_web_de_github(),
            segundos_de_espera: SEGUNDOS_DE_ESPERA_POR_DEFECTO,
            limite_de_respuesta_en_bytes: LIMITE_DE_RESPUESTA_POR_DEFECTO,
        }
    }
}

/// Valor por defecto de url_api al leer unos ajustes.
fn url_api_de_github() -> String {
    URL_API_DE_GITHUB.to_string()
}

/// Valor por defecto de url_web al leer unos ajustes.
fn url_web_de_github() -> String {
    URL_WEB_DE_GITHUB.to_string()
}

/// Valor por defecto de segundos_de_espera al leer unos ajustes.
fn segundos_de_espera_por_defecto() -> u64 {
    SEGUNDOS_DE_ESPERA_POR_DEFECTO
}

/// Valor por defecto de limite_de_respuesta_en_bytes al leer unos ajustes.
fn limite_de_respuesta_por_defecto() -> u64 {
    LIMITE_DE_RESPUESTA_POR_DEFECTO
}
