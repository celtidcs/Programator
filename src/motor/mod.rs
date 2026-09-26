//! Capa de inferencia. No sabe qué es un buzón, ni un agente, ni el protocolo: solo conversa.
//! Esa ignorancia deliberada es lo que permite probar todo lo demás sin encender la GPU.

pub mod doble;
pub mod encaje;
pub mod gguf;
pub mod hardware;
pub mod llama;
pub mod preparar;
pub mod proceso;
pub mod tipos_tensor;

/// Dirección local en la que se confina la comunicación con el motor de inferencia.
///
/// **Invariante de seguridad de confinamiento**: Programator arranca y gestiona `llama-server`
/// exclusivamente en la interfaz de bucle invertido local (`127.0.0.1`). Exponer el motor a
/// interfaces de red externas (`0.0.0.0` o IPs de red) permitiría acceso no autenticado a la GPU
/// y al proceso del servidor, violando el principio de confinamiento estricto.
pub(crate) const HOST_LOCAL: &str = "127.0.0.1";

use crate::error::Resultado;

/// Quién habla en un turno de la conversación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Papel {
    Sistema,
    Usuario,
    Modelo,
}

/// Un turno de la conversación.
#[derive(Debug, Clone)]
pub struct Mensaje {
    pub papel: Papel,
    pub contenido: String,
}

/// Petición del modelo para usar una herramienta del repertorio.
#[derive(Debug, Clone)]
pub struct SolicitudHerramienta {
    pub nombre: String,
    pub argumentos: serde_json::Value,
}

/// Lo que devuelve el modelo en un turno.
#[derive(Debug, Clone)]
pub enum Respuesta {
    Texto(String),
    Herramienta(SolicitudHerramienta),
}

/// Cualquier cosa capaz de responder a una conversación.
pub trait Motor {
    fn responder(&mut self, conversacion: &[Mensaje]) -> Resultado<Respuesta>;

    /// Comprueba la salud del motor de inferencia sin lanzar una inferencia completa.
    ///
    /// Permite determinar si un fallo fue ocasionado por contención transitoria
    /// (GPU ocupada por otros procesos) o si el motor ha caído definitivamente.
    fn comprobar_salud(&mut self) -> proceso::EstadoServidor {
        proceso::EstadoServidor::Listo
    }
}

pub use doble::MotorDoble;
pub use gguf::MetadatosModelo;
pub use llama::MotorLlama;
