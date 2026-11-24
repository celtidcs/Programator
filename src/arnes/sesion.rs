//! El encadenado del ciclo: lee el canal, decide si hay algo que atender, llama al modelo cuando
//! toca y publica el desenlace. Es la pieza que convierte las demás capas —protocolo, repertorio,
//! motor— en un agente de verdad: antes de ella todas existían probadas por separado y
//! ninguna se invocaba desde producción.
//!
//! Cuatro decisiones que no se repiten en cada función de aquí abajo:
//! - **El desenlace sale al canal únicamente por `Canal::publicar`.** Ninguna otra escritura al
//!   buzón, por ningún camino: es lo que neutraliza las marcas falsas que el modelo pueda haber
//!   colado en el texto (el nombre de una herramienta lo elige él, y acaba dentro del motivo de un
//!   aborto).
//! - **La primera pasada nunca atiende nada.** Solo fotografía dónde está cada buzón ajeno ahora
//!   mismo. Sobre un canal con meses de historia, atender todo lo que un día fue un encargo sería
//!   una avalancha de respuestas a cosas ya resueltas.
//! - **El motor se prepara una vez por grupo de encargos con el mismo modelo**, y solo si hay algo
//!   que atender: cargar el modelo es lo caro.
//! - **Ningún encargo se olvida.** Lo que no llega a publicarse pasa a la cola de pendientes
//!   (`pendientes.toml`), que cada pasada atiende antes que lo nuevo.
//!
//! Este módulo guarda el contrato de la sesión: los ajustes, los motores, el revisor y cómo
//! termina una pasada. Lo demás va aparte, una cosa por submódulo: la foto inicial del canal
//! (`instantanea`), los pasos de una pasada (`pasada`), lo que hay que atender y en qué orden
//! (`reparto`), la atención con el modelo (`atencion_del_modelo`), el cierre de cada encargo
//! (`cierre`), los encargos que no llegan al modelo (`sin_atender`), el revisor (`revisor`), la
//! única salida al canal (`respuesta`), las instrucciones del modelo (`instrucciones`) y los
//! textos (`publicacion`).

mod atencion_del_modelo;
mod cierre;
mod instantanea;
mod instrucciones;
mod pasada;
#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_motores;
#[cfg(test)]
mod pruebas_respuesta;
mod publicacion;
mod reparto;
mod respuesta;
mod revisor;
mod sin_atender;

pub use self::instantanea::{tomar_instantanea_inicial, Instantanea};
pub use self::pasada::ejecutar_pasada_con_motores;
pub use self::revisor::{Revisor, SinRevision};
use crate::arnes::ciclo::Limites;
use crate::config::Verificacion;
use crate::error::Resultado;
use crate::motor::Motor;
use crate::motores::preparar::Procedencia;
use crate::protocolo::Encargo;
use crate::terminal::Terminal;
use std::path::Path;

/// Lo que se obtuvo al intentar preparar el motor de inferencia para esta pasada.
pub enum EstadoMotor {
    /// Listo para responder encargos.
    Disponible(Box<dyn Motor>),
    /// Hay un servidor cargando el modelo todavía. No es un error, es «todavía no»: se reintenta
    /// en el ciclo siguiente sin arrancar nada más, porque hacerlo fallaría contra un puerto ya
    /// ocupado.
    Esperando,
}

/// Cuántos encargos se atendieron en la pasada y cuántos no llegaron a publicarse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumenPasada {
    pub atendidos: usize,
    pub fallidos: usize,
    /// Cuántos pasaron a la cola de pendientes (o siguieron en ella) en esta pasada.
    pub pendientes: usize,
    pub avisos: Vec<String>,
}

/// Cómo terminó una pasada completa del ciclo. `main` decide qué imprimir a partir de esto.
#[derive(Debug)]
pub enum DesenlacePasada {
    /// El registro de lectura estaba vacío: se ha fotografiado el canal y no se ha llamado al
    /// modelo, para no responder de golpe a encargos que ya llevan meses resueltos.
    PrimeraPasada,
    /// Hubo novedades en el canal, pero ninguna era un encargo dirigido a este agente.
    SinEncargos,
    /// Hay un servidor cargando el modelo. Se reintenta en el ciclo siguiente sin perder los
    /// encargos detectados: el registro no avanza.
    MotorCargando,
    /// Se atendió al menos un encargo.
    Atendidos(ResumenPasada),
}

/// La configuración que gobierna una pasada, agrupada.
///
/// Iban sueltos como parámetros hasta que fueron tres y `ejecutar_pasada` empezó a pedir seis
/// argumentos posicionales, que es donde se confunden entre sí al llamar. Son lo mismo —ajustes
/// que vienen del TOML— y viajan juntos.
pub struct Ajustes<'a> {
    pub limites: &'a Limites,
    pub verificacion: &'a Verificacion,
    /// Las instrucciones de sistema que ve el modelo. `None` usa `PROGRAMATOR.md`, las normas
    /// completas, que es lo que se hacía antes de que el preámbulo existiera.
    pub preambulo: Option<&'a Path>,
    /// Lo que se le recuerda pegado al encargo. `None` no añade nada.
    ///
    /// Va aparte del preámbulo porque **el sitio importa**: medido el 22/09/2026, las mismas normas
    /// de código en el mensaje de sistema no cambian lo que escribe, y pegadas al encargo sí.
    pub recordatorio: Option<&'a Path>,
    /// Tamaño del contexto en tokens configurado para el motor. Se usa para estimar si la
    /// conversación va justa de espacio.
    pub contexto: u32,
    /// A partir de qué porcentaje de la ventana se avisa de que la conversación va justa.
    pub umbral_aviso_contexto: u32,
    /// Cuántos bytes se cuentan por token al estimar la ocupación. Es una aproximación que
    /// depende del modelo y del idioma.
    pub bytes_por_token: u32,
    /// Tope de bytes que se entregan al modelo en una lectura de fichero.
    pub tope_lectura_bytes: usize,
    /// Nombre del fichero donde se escribe el estado del arnés en cada sondeo.
    pub fichero_latido: &'a str,
    /// Segundos entre dos sondeos configurados en el ciclo.
    pub intervalo_segundos: u64,
    /// Si cada encargo deja escrita su conversación con el modelo (`[agente]
    /// registro_de_conversacion`).
    pub registro_de_conversacion: bool,
    /// Tras cuántos intentos fallidos se avisa en el canal de un encargo pendiente.
    pub intentos_por_encargo: u32,
    /// Por dónde se cuenta en la terminal lo que pasa en la pasada. La pone `main`, que es quien
    /// decide qué se imprime; las pruebas pasan una de mentira para ver lo que se avisa.
    pub terminal: &'a dyn Terminal,
}

impl<'a> Ajustes<'a> {
    /// Los ajustes mínimos: los límites del ciclo, sin verificación y con las normas completas.
    pub fn con_limites(
        limites: &'a Limites,
        verificacion: &'a Verificacion,
        terminal: &'a dyn Terminal,
    ) -> Self {
        Self {
            limites,
            verificacion,
            preambulo: None,
            recordatorio: None,
            contexto: crate::config::CONTEXTO_POR_DEFECTO,
            umbral_aviso_contexto: crate::config::UMBRAL_AVISO_CONTEXTO_POR_DEFECTO_PORCENTAJE,
            bytes_por_token: crate::config::BYTES_POR_TOKEN_POR_DEFECTO,
            tope_lectura_bytes: crate::config::tope_lectura_por_defecto(),
            fichero_latido: crate::config::FICHERO_LATIDO_POR_DEFECTO,
            intervalo_segundos: crate::config::intervalo_por_defecto(),
            registro_de_conversacion: false,
            intentos_por_encargo: crate::config::INTENTOS_POR_ENCARGO_POR_DEFECTO,
            terminal,
        }
    }

    /// Los ajustes del ciclo tal como los fija la configuración. La verificación va aparte para
    /// que quien no deba ejecutar nada en este equipo (el banco de modelos) pueda pasar una vacía.
    pub fn de_la_config(
        config: &'a crate::config::Config,
        limites: &'a Limites,
        verificacion: &'a Verificacion,
        preambulo: &'a Path,
        recordatorio: &'a Path,
        terminal: &'a dyn Terminal,
    ) -> Self {
        Self {
            limites,
            verificacion,
            preambulo: Some(preambulo),
            recordatorio: Some(recordatorio),
            contexto: config.motor.contexto,
            umbral_aviso_contexto: config.motor.umbral_aviso_contexto,
            bytes_por_token: config.motor.bytes_por_token,
            tope_lectura_bytes: config.ciclo.tope_lectura_bytes,
            fichero_latido: &config.ciclo.fichero_latido,
            intervalo_segundos: config.ciclo.intervalo_segundos,
            registro_de_conversacion: config.agente.registro_de_conversacion,
            intentos_por_encargo: config.ciclo.intentos_por_encargo,
            terminal,
        }
    }
}

/// Qué modelo atiende cada encargo y cómo se prepara su motor. En producción lo decide la línea
/// `Clase:` con el registro de modelos; en las pruebas y en el banco, un solo motor.
pub trait Motores {
    /// El modelo con que se atiende el encargo y, si toca, un aviso para su entrega.
    fn modelo_para(&self, encargo: &Encargo) -> ModeloPedido;
    /// El modelo que hay cargado ahora en el motor propio, si lo hay.
    fn cargado(&self) -> Option<String>;
    /// Prepara el motor con ese modelo. Con `Espera::HastaElTope`, si tiene que cargarlo espera
    /// en el sitio hasta su tope; con `Espera::Ninguna`, devuelve `Cargando` en cuanto lo lanza.
    fn preparar(&mut self, modelo: Option<&str>, espera: Espera) -> Resultado<Preparacion>;
    /// Lo que la entrega cuenta de la memoria al terminar un encargo (y, si toca, el aviso de que
    /// cupieron menos capas que las medidas). Quien lo mide lo anota también en el historial de
    /// encaje. Sin motor propio no hay nada que contar.
    fn tras_el_encargo(&mut self) -> Option<String> {
        None
    }
}

/// El modelo pedido para un encargo. `modelo` es `None` cuando no hay elección posible (un solo
/// motor): todos van juntos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModeloPedido {
    pub modelo: Option<String>,
    /// Lo que la entrega tiene que decir sobre la elección (una clase que no existe).
    pub aviso: Option<String>,
    /// Si el encargo pidió una clase válida: con un servidor ajeno, la entrega dice que no se
    /// pudo cambiar de modelo.
    pub por_clase: bool,
}

/// Si preparar un motor que tiene que cargar espera en el sitio o vuelve en seguida. Es un tipo
/// propio y no un `bool`, que partiría `preparar` en dos operaciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Espera {
    /// Vuelve en cuanto lanza la carga, con `Cargando`: el ciclo sigue con otros encargos.
    Ninguna,
    /// Espera en el sitio hasta el tope de carga.
    HastaElTope,
}

/// Cómo quedó el motor al prepararlo.
pub enum Preparacion {
    /// Listo para responder. Si su servidor es ajeno, no lo arrancó Programator y tiene el modelo
    /// que tenga.
    Listo {
        motor: Box<dyn Motor>,
        procedencia: Procedencia,
    },
    /// Cargando todavía; se vuelve en la vuelta siguiente.
    Cargando,
    /// Se esperó en el sitio y no terminó de cargar dentro del tope.
    TiempoAgotado,
    /// No hay modelo con el que trabajar (falta su fichero o no cabe en la tarjeta), con el
    /// motivo: se sabe antes de lanzar nada, así que no se intenta.
    SinModelo(String),
}

/// Un único motor, el que devuelve `asegurar_motor`: todos los encargos van con él.
struct UnSoloMotor<'f> {
    asegurar: &'f mut dyn FnMut() -> Resultado<EstadoMotor>,
}

impl Motores for UnSoloMotor<'_> {
    fn modelo_para(&self, _encargo: &Encargo) -> ModeloPedido {
        ModeloPedido::default()
    }
    fn cargado(&self) -> Option<String> {
        None
    }
    fn preparar(&mut self, _modelo: Option<&str>, espera: Espera) -> Resultado<Preparacion> {
        Ok(match (self.asegurar)()? {
            EstadoMotor::Disponible(motor) => Preparacion::Listo {
                motor,
                procedencia: Procedencia::Propio,
            },
            EstadoMotor::Esperando if espera == Espera::HastaElTope => Preparacion::TiempoAgotado,
            EstadoMotor::Esperando => Preparacion::Cargando,
        })
    }
}

/// Ejecuta una pasada completa del ciclo sin revisor: todo encargo va al modelo.
pub fn ejecutar_pasada(
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
    asegurar_motor: &mut dyn FnMut() -> Resultado<EstadoMotor>,
) -> Resultado<DesenlacePasada> {
    ejecutar_pasada_con_revisor(carpeta, agente, ajustes, asegurar_motor, &mut SinRevision)
}

/// Igual que [`ejecutar_pasada_con_motores`], con un solo motor: el que devuelve
/// `asegurar_motor`. En las pruebas basta con envolver un `MotorDoble` y devolverlo, sin tocar
/// red ni proceso alguno.
pub fn ejecutar_pasada_con_revisor(
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
    asegurar_motor: &mut dyn FnMut() -> Resultado<EstadoMotor>,
    revisor: &mut dyn Revisor,
) -> Resultado<DesenlacePasada> {
    let mut motores = UnSoloMotor {
        asegurar: asegurar_motor,
    };
    ejecutar_pasada_con_motores(carpeta, agente, ajustes, &mut motores, revisor)
}
