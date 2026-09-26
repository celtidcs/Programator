//! Tipos de error del proyecto. Ningún módulo entra en pánico: todo se propaga.

use std::path::PathBuf;

/// Error de cualquier operación de Programator.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no se pudo leer «{ruta}»: {causa}")]
    Lectura {
        ruta: PathBuf,
        causa: std::io::Error,
    },

    #[error("no se pudo escribir «{ruta}»: {causa}")]
    Escritura {
        ruta: PathBuf,
        causa: std::io::Error,
    },

    #[error("configuración inválida: {0}")]
    Configuracion(String),

    #[error("ruta fuera del ámbito permitido: «{0}»")]
    FueraDeAmbito(PathBuf),

    #[error("el motor local no respondió: {0}")]
    MotorSinRespuesta(String),

    #[error("propuesta de poda descartada: {0}")]
    PodaInvalida(String),

    #[error("cabecera GGUF inválida en «{ruta}»: {causa}")]
    GgufInvalido { ruta: PathBuf, causa: String },
}

/// Alias corto para los resultados del proyecto.
pub type Resultado<T> = std::result::Result<T, Error>;
