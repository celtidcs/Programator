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

    /// Un fallo de configuración de la carpeta de trabajo. Se lee igual que `Configuracion`, pero
    /// el consejo del arranque sale de la variante y no de buscar palabras en el texto, que
    /// lleva rutas del usuario.
    #[error("configuración inválida: {0}")]
    ConfiguracionDeLaCarpeta(String),

    /// Un fallo de configuración del motor: su binario, su modelo o su arranque. Se lee igual que
    /// `Configuracion`, por la misma razón que `ConfiguracionDeLaCarpeta`.
    #[error("configuración inválida: {0}")]
    ConfiguracionDelMotor(String),

    #[error("ruta fuera del ámbito permitido: «{0}»")]
    FueraDeAmbito(PathBuf),

    #[error("el motor local no respondió: {0}")]
    MotorSinRespuesta(String),

    #[error("propuesta de poda descartada: {0}")]
    PodaInvalida(String),

    #[error("cabecera GGUF inválida en «{ruta}»: {causa}")]
    GgufInvalido { ruta: PathBuf, causa: String },

    #[error("banco de modelos: {0}")]
    Banco(String),

    #[error("registro de modelos: {0}")]
    Registro(String),

    #[error("no se pudo aislar la orden: {0}")]
    Aislamiento(String),

    #[error("revisión de modelos: {0}")]
    Revision(String),

    #[error("cola de encargos pendientes: {0}")]
    Pendientes(String),

    #[error("registro de encargos: {0}")]
    RegistroDeEncargos(String),

    #[error("registro de entregas: {0}")]
    RegistroDeEntregas(String),
}

/// Alias corto para los resultados del proyecto.
pub type Resultado<T> = std::result::Result<T, Error>;
