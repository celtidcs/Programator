//! Identificación del programa que hace la consulta, para la cabecera User-Agent.
//!
//! GitHub pide que la cabecera User-Agent identifique a la aplicación que hace la petición.
//! El programa que usa la biblioteca puede presentarse con su nombre y su versión, que se
//! validan aquí para que no puedan colar otra cabecera ni romper la petición.
// Fuente: https://docs.github.com/en/rest/using-the-rest-api/getting-started-with-the-rest-api#user-agent

use crate::version::{FalloDeComprobacion, VersionSemantica};

/// Caracteres que puede tener, como mucho, el nombre del programa.
///
/// No sale de ninguna norma: es un límite propio para que la cabecera no crezca sin medida.
/// Sobra para cualquier nombre de programa real.
const LONGITUD_MAXIMA_DEL_NOMBRE: usize = 64;

/// Nombre y versión del programa que pregunta por la versión nueva, ya validados.
///
/// Solo se construye con nuevo, así que quien tiene uno sabe que el nombre empieza por una
/// letra o un dígito y solo lleva letras, dígitos, puntos, guiones y guiones bajos, y que la
/// versión tiene la forma X.Y.Z. Todos esos caracteres valen en un producto de la cabecera
/// User-Agent, que no admite espacios, barras, saltos de línea ni caracteres de control.
// Fuente: RFC 9110, apartados 10.1.5 (User-Agent) y 5.6.2 (tokens).
// https://www.rfc-editor.org/rfc/rfc9110#section-10.1.5
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramaQueConsulta {
    /// Nombre del programa, como mi-programa.
    nombre: String,
    /// Versión del programa.
    version: VersionSemantica,
}

impl ProgramaQueConsulta {
    /// Valida el nombre y la versión del programa.
    ///
    /// El nombre tiene de 1 a 64 caracteres, empieza por una letra o un dígito y solo lleva
    /// letras, dígitos, puntos, guiones y guiones bajos. La versión tiene la forma X.Y.Z, con
    /// una v opcional delante, como la que da env!("CARGO_PKG_VERSION").
    ///
    /// Devuelve FalloDeComprobacion::IdentificacionNoValida si el nombre no cumple esas reglas
    /// y FalloDeComprobacion::EtiquetaNoValida si la versión no tiene la forma X.Y.Z.
    pub fn nuevo(nombre: &str, version: &str) -> Result<Self, FalloDeComprobacion> {
        if !nombre_valido(nombre) {
            return Err(FalloDeComprobacion::IdentificacionNoValida(
                nombre.to_string(),
            ));
        }
        let version = VersionSemantica::desde_etiqueta(version)
            .ok_or_else(|| FalloDeComprobacion::EtiquetaNoValida(version.to_string()))?;
        Ok(Self {
            nombre: nombre.to_string(),
            version,
        })
    }

    /// El producto que representa al programa en la cabecera User-Agent: nombre/X.Y.Z.
    pub(crate) fn producto(&self) -> String {
        format!("{}/{}", self.nombre, self.version)
    }
}

/// Un nombre de programa: de 1 a 64 caracteres, el primero letra o dígito, y el resto letras,
/// dígitos, puntos, guiones o guiones bajos.
fn nombre_valido(nombre: &str) -> bool {
    (1..=LONGITUD_MAXIMA_DEL_NOMBRE).contains(&nombre.len())
        && nombre.starts_with(|c: char| c.is_ascii_alphanumeric())
        && nombre
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
}
