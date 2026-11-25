//! El repertorio cerrado de acciones.
//!
//! El modelo nunca escribe un fichero: solicita, y aquí se concede o se deniega. Lo que no figura en
//! `Repertorio::nombres()` no puede hacerse. En particular no existe —ni existirá en la v1— ninguna herramienta
//! para firmar, aprobar o aceptar: el protocolo dice que nada se da por bueno porque lo diga
//! quien lo hizo, y Programator no puede violarlo porque no dispone del verbo.
//!
//! Este módulo guarda el contrato del repertorio: qué reconoce, qué declara, el estado que lleva a
//! lo largo de una pasada y cómo se despacha cada solicitud según el `ficha::Estado` de su verbo.
//! Un verbo que su ficha deniega siempre se deniega aquí con el motivo de la ficha, sin que haga
//! falta nombrarlo. Lo demás va aparte, una cosa por submódulo: la lectura de los argumentos
//! (`argumentos`), la consulta del ámbito (`consulta`), la verificación que pide el modelo
//! (`verificacion_pedida`), la entrega de propuestas (`propuesta`) y su comprobación
//! (`comprobacion`), lo que el modelo anota en el canal (`anotaciones`), el nombre libre de cada
//! candidato (`candidatos`), la preparación de lo leído (`lectura`), las citas de auditoría
//! (`auditoria`), los errores para el modelo (`error`) y los textos (`mensajes`).

mod anotaciones;
mod argumentos;
pub mod auditoria;
mod candidatos;
mod comprobacion;
mod consulta;
mod error;
mod lectura;
mod mensajes;
mod propuesta;
mod verificacion_pedida;

pub use self::propuesta::carpeta_de_candidatos;

use super::verificacion::Comprobadores;
use super::Ambito;
use crate::arnes::ficha::{self, verbo, Estado};
use crate::motor::SolicitudHerramienta;
use crate::protocolo::{Canal, ResumenLeido};
use crate::terminal::Terminal;
use std::sync::OnceLock;
use std::time::Duration;

/// Resultado de atender una solicitud.
#[derive(Debug, Clone)]
pub enum Decision {
    /// Concedida, con lo que se le devuelve al modelo.
    Concedida(String),
    /// Denegada, con el motivo que se le explica al modelo.
    Denegada(String),
}

/// Todo lo que el repertorio **reconoce**, derivado de la ficha.
///
/// Se calcula una vez y se reutiliza: la firma sigue siendo `&'static [&'static str]` para que
/// nada de lo que ya llamaba a `nombres()` tenga que cambiar.
fn nombres_reconocidos() -> &'static [&'static str] {
    static CACHE: OnceLock<Vec<&'static str>> = OnceLock::new();
    CACHE
        .get_or_init(|| super::ficha::todas().iter().map(|f| f.nombre).collect())
        .as_slice()
}

/// Lo que hoy puede concederse, y por tanto lo único que se le **declara** al modelo.
///
/// Reconocer no es conceder. Lo que se deniega siempre no se anuncia: anunciarlo solo sirve para
/// que el modelo queme solicitudes, porque insistir dos veces en la misma aborta el encargo
/// y alternarlas agota el tope sin entregar nada. Qué es concedible lo dice `ficha::Estado`, que es
/// también donde está escrita la condición para que `verificar` vuelva.
fn nombres_concedidos() -> &'static [&'static str] {
    static CACHE: OnceLock<Vec<&'static str>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            super::ficha::todas()
                .iter()
                .filter(|f| f.es_concedible())
                .map(|f| f.nombre)
                .collect()
        })
        .as_slice()
}

/// Tope de bytes de fichero que se le entrega al modelo de una sola lectura, si la configuración no
/// dice otra cosa.
///
/// Un buzón real puede pasar de 260 KB: devolver el fichero entero desborda la ventana y mata el
/// encargo de golpe, sin diagnóstico. 16 KiB dejan sitio a las normas, a la conversación y a las
/// demás solicitudes del encargo con la ventana por defecto, que son **16.384 tokens** desde el
/// 22/09/2026 (`config::contexto_por_defecto`). Quien cambie la ventana en el TOML probablemente
/// quiera cambiar también este tope, y por eso es configurable.
///
/// Cuando se recorta se le dice al modelo, con el tamaño real: leer de menos sabiéndolo es
/// recuperable, creer que has leído todo no lo es.
///
/// Se deriva de `config::tope_lectura_por_defecto`, que es la única fuente de la cifra: este valor
/// solo se usa cuando nadie ha construido el arnés pasando el del TOML (todo camino real, desde
/// `sesion/atencion_del_modelo.rs`, sí lo pasa). Repetir el `16 * 1024` aquí y allá dejó de ser
/// aceptable en la 0.10.0: es la misma fuente múltiple que el resto de la versión elimina.
const TOPE_LECTURA_POR_DEFECTO: usize = crate::config::tope_lectura_por_defecto();

pub struct Repertorio<'t> {
    ambito: Ambito,
    canal: Canal,
    /// Cuerpo que el modelo quiere publicar, si ya lo ha entregado.
    pendiente_de_publicar: Option<String>,
    /// Con qué se comprueba cada extensión. Vacío es «no se verifica nada», que es el
    /// comportamiento de siempre.
    comprobadores: Comprobadores,
    /// Cuánto se espera a un comprobador antes de cortarlo.
    tiempo_de_verificacion: Duration,
    /// Cuántos caracteres de lo que dice un comprobador que rechaza se le pasan al modelo.
    maximo_salida_de_verificacion: usize,
    /// Qué salió de verificar cada propuesta de esta pasada, en orden. Lo lee `sesion` para dejar
    /// constancia en el canal: **el cuerpo lo redacta el modelo, el veredicto no**.
    veredictos: Vec<String>,
    /// Nombres reales de los ficheros de propuesta escritos en disco durante esta pasada, en orden.
    propuestas: Vec<String>,
    /// Tope de bytes que se le entregan al modelo en una lectura de fichero.
    tope_lectura: usize,
    /// Resumen de los buzones leídos al inicio de la pasada, para notificar entregas al canal.
    leido: ResumenLeido,
    /// Los veredictos que no se pudieron publicar al entregar su propuesta, en orden. Son los que
    /// tiene que llevar la respuesta del encargo. Es una lista y no un contador: con un contador,
    /// si fallaba el primero y salía el segundo, se daría por pendiente el segundo.
    sin_publicar: Vec<String>,
    /// Lo que el arnés denegó en el encargo en curso, una línea por denegación distinta, para la
    /// respuesta: en una prueba de uso real el arnés denegó una propuesta que salía de su
    /// carpeta, el modelo reintentó con otro nombre y quien encargó no se enteró.
    denegadas: Vec<String>,
    /// Las líneas de reserva que el modelo anotó en `estado.md` y siguen puestas. Se quitan al
    /// cerrar el encargo, publique o no (las reservas del canal se
    /// borran al terminar la tarea).
    reservas: Vec<String>,
    /// Por dónde se avisa de una entrega que no se pudo publicar en el buzón.
    terminal: &'t dyn Terminal,
}

/// Lo que se espera a un comprobador si el TOML no dice otra cosa.
///
/// Se deriva de `config::tiempo_verificacion_por_defecto()`, que es la única fuente de la cifra.
const TIEMPO_VERIFICACION_POR_DEFECTO: Duration =
    Duration::from_secs(crate::config::tiempo_verificacion_por_defecto());

impl<'t> Repertorio<'t> {
    pub fn nuevo(ambito: Ambito, canal: Canal, terminal: &'t dyn Terminal) -> Self {
        Self {
            ambito,
            canal,
            pendiente_de_publicar: None,
            comprobadores: Comprobadores::new(),
            tiempo_de_verificacion: TIEMPO_VERIFICACION_POR_DEFECTO,
            maximo_salida_de_verificacion: crate::config::MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO,
            veredictos: Vec::new(),
            propuestas: Vec::new(),
            tope_lectura: TOPE_LECTURA_POR_DEFECTO,
            leido: ResumenLeido::vacio(),
            sin_publicar: Vec::new(),
            denegadas: Vec::new(),
            reservas: Vec::new(),
            terminal,
        }
    }

    /// Pone la puerta de verificación: con qué se comprueba cada extensión y cuánto se espera.
    ///
    /// Se construye aparte de `nuevo` a propósito: sin esto, el repertorio se comporta exactamente
    /// como antes de que la puerta existiera, y las pruebas que no van de verificación no tienen
    /// que saber nada de ella.
    pub fn con_verificacion(mut self, comprobadores: Comprobadores, tiempo: Duration) -> Self {
        self.comprobadores = comprobadores;
        self.tiempo_de_verificacion = tiempo;
        self
    }

    /// Pone cuántos caracteres de lo que dice un comprobador que rechaza ve el modelo
    /// (`[verificacion] maximo_salida_al_modelo`).
    pub fn con_maximo_salida_de_verificacion(mut self, maximo: usize) -> Self {
        self.maximo_salida_de_verificacion = maximo;
        self
    }

    /// Pone la puerta de verificación tal como la dice la sección `[verificacion]` del TOML:
    /// comprobadores, tiempo máximo y cuánto ve el modelo de un rechazo. Es el único sitio donde la
    /// configuración pasa al repertorio, para que ninguna de las tres se quede por el camino.
    pub fn con_ajustes_de_verificacion(self, verificacion: &crate::config::Verificacion) -> Self {
        self.con_verificacion(
            verificacion.comprobadores.clone(),
            Duration::from_secs(verificacion.tiempo_maximo_segundos),
        )
        .con_maximo_salida_de_verificacion(verificacion.maximo_salida_al_modelo)
    }

    /// Establece el tope de bytes para lecturas de fichero desde la configuración.
    ///
    /// Si no se llama, se usa el valor por defecto `TOPE_LECTURA_POR_DEFECTO`.
    pub fn con_tope_de_lectura(mut self, bytes: usize) -> Self {
        self.tope_lectura = bytes;
        self
    }

    /// Establece el resumen de lo leído para publicar entregas en el canal de inmediato.
    pub fn con_leido(mut self, leido: ResumenLeido) -> Self {
        self.leido = leido;
        self
    }

    /// Lo que salió de comprobar las propuestas de esta pasada, para dejarlo escrito en el canal.
    pub fn veredictos(&self) -> &[String] {
        &self.veredictos
    }

    /// Cuántos veredictos de propuestas ya han sido publicados al momento de entrega.
    pub fn veredictos_publicados(&self) -> usize {
        self.veredictos.len() - self.sin_publicar.len()
    }

    /// Los veredictos que no se pudieron publicar al entregar su propuesta.
    pub fn veredictos_no_publicados(&self) -> &[String] {
        &self.sin_publicar
    }

    /// Lo que el arnés denegó en el encargo en curso, para contarlo en su respuesta.
    pub fn denegadas_del_encargo(&self) -> &[String] {
        &self.denegadas
    }

    /// Al cerrar un encargo, salga como salga: sus veredictos sin publicar ya fueron en su
    /// respuesta, o el encargo se repetirá, o se cerró; no pasan al siguiente. Sus denegaciones
    /// tampoco.
    pub fn cerrar_los_veredictos_del_encargo(&mut self) {
        self.sin_publicar.clear();
        self.denegadas.clear();
    }

    /// Los nombres reales de los ficheros de propuesta escritos en disco durante esta pasada.
    pub(super) fn propuestas(&self) -> &[String] {
        &self.propuestas
    }

    /// Los nombres de todo lo que el repertorio reconoce. No hay nada más: lo que no está aquí no
    /// puede hacerse. Reconocido no es lo mismo que concedible —para eso está
    /// `nombres_concedibles`—, y cambiarle el sentido a esta rompería a quien la use para
    /// comprobar qué verbos existen.
    pub fn nombres() -> &'static [&'static str] {
        nombres_reconocidos()
    }

    /// Los nombres que se le declaran al modelo como su repertorio: solo lo que hoy puede
    /// concederse. Deja fuera `buscar` y `proponer_poda`, que se deniegan siempre.
    pub fn nombres_concedibles() -> &'static [&'static str] {
        nombres_concedidos()
    }

    /// Lo que el modelo dejó listo para publicar, si lo hizo.
    pub fn pendiente_de_publicar(&self) -> Option<&str> {
        self.pendiente_de_publicar.as_deref()
    }

    /// Se lleva el cuerpo pendiente para publicarlo, y lo deja vacío.
    ///
    /// El repertorio dura toda la pasada. Si el cuerpo se quedara, el encargo siguiente cerraría
    /// con el `publicar` del anterior en cuanto se le concediera la primera herramienta.
    /// Las reservas anotadas desde la última vez, que deja de llevar: quien las toma las quita.
    pub fn tomar_reservas(&mut self) -> Vec<String> {
        std::mem::take(&mut self.reservas)
    }

    pub fn tomar_pendiente_de_publicar(&mut self) -> Option<String> {
        self.pendiente_de_publicar.take()
    }

    /// Atiende una solicitud del modelo.
    ///
    /// Lo que no tiene ficha no está en el repertorio. Lo que sí la tiene se despacha por su
    /// `ficha::Estado`: un verbo que se deniega siempre se deniega con el motivo de su ficha, sin
    /// nombrarlo aquí, y solo los concedibles y los pendientes llegan a su verbo.
    pub fn atender(&mut self, solicitud: &SolicitudHerramienta) -> Decision {
        let decision = self.decidir(solicitud);
        if let Decision::Denegada(motivo) = &decision {
            let linea = mensajes::denegada_para_el_canal(&solicitud.nombre, motivo);
            if !self.denegadas.contains(&linea) {
                self.denegadas.push(linea);
            }
        }
        decision
    }

    /// Lo que se decide para `solicitud`, sin apuntar nada.
    fn decidir(&mut self, solicitud: &SolicitudHerramienta) -> Decision {
        let Some(ficha) = ficha::buscar(&solicitud.nombre) else {
            return Decision::Denegada(mensajes::fuera_del_repertorio(
                &solicitud.nombre,
                nombres_reconocidos(),
            ));
        };

        match ficha.estado {
            Estado::DenegadaSiempre { motivo } => Decision::Denegada(motivo.to_string()),
            Estado::Pendiente { motivo, .. } => Self::atender_pendiente(solicitud, motivo),
            Estado::Concedible => self.atender_concedible(solicitud),
        }
    }

    /// Un verbo que se deniega hoy, con el motivo de su ficha. `verificar` comprueba antes lo que
    /// se le pide, para que el modelo sepa si su petición sería válida cuando se conceda.
    fn atender_pendiente(solicitud: &SolicitudHerramienta, motivo: &str) -> Decision {
        match solicitud.nombre.as_str() {
            verbo::VERIFICAR => verificacion_pedida::atender(solicitud, motivo),
            _ => Decision::Denegada(motivo.to_string()),
        }
    }

    /// Un verbo concedible, atendido por su verbo.
    fn atender_concedible(&mut self, solicitud: &SolicitudHerramienta) -> Decision {
        match solicitud.nombre.as_str() {
            verbo::LEER_FICHERO => self.leer_fichero(solicitud),
            verbo::LISTAR => self.listar(solicitud),
            verbo::PUBLICAR => self.publicar(solicitud),
            verbo::ESCRIBIR_PROPUESTA => self.escribir_propuesta(solicitud),
            verbo::RESERVAR => self.reservar(solicitud),
            _ => Decision::Denegada(mensajes::SOLICITUD_NO_RECONOCIDA.to_string()),
        }
    }
}

#[cfg(test)]
mod pruebas;
