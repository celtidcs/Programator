//! Comprueba si hay una versión más nueva de un programa publicada en GitHub.
//!
//! Pregunta una vez a la API de GitHub, o a la de una instalación propia de GitHub Enterprise
//! Server, cuál es la última versión publicada de un repositorio y, si es más nueva que la que
//! se está ejecutando, devuelve un aviso con la versión y el enlace a su página. No descarga ni
//! instala nada.
//!
//! Uso. Se preparan unos ajustes con AjustesDeVersionNueva::para_repositorio, se validan con
//! OrigenDeLasVersiones::desde_ajustes, se crea la consulta con ConsultaHttps::nueva y
//! proxy_del_sistema, y se llama a comprobar con la versión local, o a ultima_version para
//! saber solo cuál es la última publicada. Con ConsultaHttps::para_programa y un
//! ProgramaQueConsulta, la petición se presenta también con el nombre y la versión del
//! programa. La llamada bloquea mientras espera la red, así que un programa con interfaz debe
//! hacerla en un hilo de trabajo.
//!
//! Errores. Ningún fallo provoca un pánico: todos llegan como FalloDeComprobacion, y quien usa
//! la biblioteca decide si los registra.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
// Ningún fallo puede acabar en pánico fuera de las pruebas: un programa compilado con
// panic = "abort" se cerraría entero por una comprobación que sale mal.
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![cfg_attr(test, deny(dead_code, unused_imports, unused_mut, unused_variables))]

mod ajustes;
mod consulta;
mod origen;
mod programa;
mod version;

pub use ajustes::{
    AjustesDeVersionNueva, LIMITE_DE_RESPUESTA_POR_DEFECTO, SEGUNDOS_DE_ESPERA_POR_DEFECTO,
    URL_API_DE_GITHUB, URL_WEB_DE_GITHUB,
};
pub use consulta::{
    comprobar, proxy_del_sistema, ultima_version, ConsultaDeVersion, ConsultaHttps,
};
pub use origen::OrigenDeLasVersiones;
pub use programa::ProgramaQueConsulta;
pub use version::{
    decidir_aviso, interpretar_respuesta, AvisoDeVersionNueva, FalloDeComprobacion,
    VersionSemantica,
};

/// Ureq se reexporta para que quien use la biblioteca pueda construir un proxy concreto con
/// la misma versión de la biblioteca de red.
pub use ureq;

#[cfg(test)]
mod pruebas;
