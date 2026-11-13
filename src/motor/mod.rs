//! Capa de inferencia. No sabe qué es un buzón, ni un agente, ni el protocolo: solo conversa.
//! Esa ignorancia deliberada es lo que permite probar todo lo demás sin encender la GPU.

pub mod consumo;
pub mod continuidad;
pub mod doble;
pub mod encaje;
pub mod encaje_historico;
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
    /// El resultado de una llamada a herramienta, enlazado con ella por su identificador.
    Herramienta,
}

/// Una llamada a herramienta ya hecha, tal como queda en el historial.
///
/// **Por qué el historial guarda la llamada entera.** Hasta el 12/11/2026 el turno del modelo se
/// guardaba como el texto `[solicito escribir_propuesta]`, sin argumentos. El modelo no veía lo que
/// acababa de escribir y lo volvía a generar: trece de dieciocho encargos de código de la primera
/// tanda del banco acabaron en doce reescrituras del mismo fichero.
#[derive(Debug, Clone, PartialEq)]
pub struct Llamada {
    pub id: String,
    pub nombre: String,
    pub argumentos: serde_json::Value,
}

/// Un turno de la conversación.
///
/// Se puede comparar porque el cliente del motor necesita saber si una conversación continúa la
/// anterior (`motor::continuidad`).
#[derive(Debug, Clone, PartialEq)]
pub struct Mensaje {
    pub papel: Papel,
    pub contenido: String,
    /// En un turno del modelo, la llamada que hizo; en un resultado, la llamada a la que responde.
    pub llamada: Option<Llamada>,
}

impl Mensaje {
    /// Un turno de texto, sin llamada.
    pub fn nuevo(papel: Papel, contenido: impl Into<String>) -> Self {
        Self {
            papel,
            contenido: contenido.into(),
            llamada: None,
        }
    }

    /// El turno en que el modelo llamó a una herramienta.
    pub fn llamada_del_modelo(llamada: Llamada) -> Self {
        Self {
            papel: Papel::Modelo,
            contenido: String::new(),
            llamada: Some(llamada),
        }
    }

    /// Lo que el arnés contesta a una llamada.
    pub fn resultado_de(llamada: &Llamada, contenido: impl Into<String>) -> Self {
        Self {
            papel: Papel::Herramienta,
            contenido: contenido.into(),
            llamada: Some(llamada.clone()),
        }
    }
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
    /// El motor cortó la respuesta antes de que el modelo terminara, porque se acabó el contexto
    /// o el tope de tokens. Lleva lo que llegó a escribir, que suele ser nada: en la tanda del
    /// banco del 13/11/2026, un modelo que razonaba sin parar llenó así el contexto en 20 de 51
    /// pasadas, y el arnés lo contaba como «no invocó herramientas».
    Cortada(String),
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

    /// Tokens y tiempos de la última respuesta, si el motor los cuenta. Por defecto, `None`: un
    /// motor que no los conoce no tiene que hacer nada.
    fn ultimo_consumo(&self) -> Option<Consumo> {
        None
    }
}

pub use consumo::Consumo;
pub use doble::MotorDoble;
pub use gguf::MetadatosModelo;
pub use llama::MotorLlama;
