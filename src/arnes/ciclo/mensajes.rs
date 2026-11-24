//! Lo que el ciclo de un encargo le dice al modelo y lo que deja escrito cuando lo corta.
//!
//! Un solo sitio para estos textos: los motivos de aborto acaban en el canal, a la vista de
//! quien encargó, y los avisos del arnés acaban en la conversación del modelo. El ciclo decide;
//! aquí solo se redacta.

use crate::error::Error;
use crate::motor::proceso::EstadoServidor;

/// Lo que se le dice al modelo cuando escribe la etiqueta de una herramienta como texto en vez de
/// invocarla (visto en uso real).
pub const AVISO_ETIQUETA_COMO_TEXTO: &str = "[Aviso del arnés: Has escrito la petición de una herramienta como texto plano, no como una llamada real. Para usar una herramienta, invócala mediante la llamada estructurada; no escribas su etiqueta.]";

/// Cuántas veces se le preguntó al motor, en palabras. Sin reintentos, cada desenlace lo dice a su
/// manera (`sin_reintentos`); con uno o más, siempre igual.
fn detalle_de_intentos(reintentos_realizados: u32, sin_reintentos: &str) -> String {
    match reintentos_realizados {
        0 => sin_reintentos.to_string(),
        1 => "1 reintento (2 intentos en total)".to_string(),
        n => format!("{n} reintentos ({} intentos en total)", n + 1),
    }
}

/// El motivo de abortar porque el motor falló y `/health` dice que no hay servidor: no tiene
/// sentido reintentar.
pub fn motor_no_disponible(
    reintentos_realizados: u32,
    salud: EstadoServidor,
    error: &Error,
) -> String {
    let detalle_intentos = detalle_de_intentos(reintentos_realizados, "1 intento");
    format!(
        "🔴 Programator no pudo completar el encargo: el motor no respondió tras \
         {detalle_intentos} y la comprobación de salud indica que {salud}. Último error del \
         motor: {error}"
    )
}

/// El motivo de abortar porque el motor siguió fallando tras todos los reintentos configurados.
pub fn motor_sin_respuesta(
    reintentos_realizados: u32,
    salud: EstadoServidor,
    error: &Error,
) -> String {
    let detalle_intentos =
        detalle_de_intentos(reintentos_realizados, "0 reintentos (1 intento en total)");
    format!(
        "🔴 Programator no pudo completar el encargo: el motor no respondió tras \
         {detalle_intentos}. Última comprobación de salud: {salud}. Último error del motor: \
         {error}"
    )
}

/// El motivo de un encargo cortado porque se acabó su tiempo máximo.
pub fn fuera_de_plazo(detalle_entrega: &str) -> String {
    format!(
        "🔴 Programator cortó el encargo: se acabó el tiempo máximo antes de que \
         terminara. {detalle_entrega}"
    )
}

/// El motivo de un encargo abortado porque el motor no respondió: el de los reintentos, con lo que
/// quedó en disco.
pub fn abortado_por_el_motor(motivo: &str, detalle_entrega: &str) -> String {
    format!("{motivo}. {detalle_entrega}")
}

/// El motivo de un encargo abortado porque el modelo siguió escribiendo etiquetas como texto.
pub fn abortado_por_etiqueta_como_texto(detalle_entrega: &str) -> String {
    format!(
        "🔴 Programator abortó el encargo: el modelo escribió la etiqueta de una herramienta como texto en vez de invocarla, repetidamente. {detalle_entrega}"
    )
}

/// El motivo de un encargo abortado por pasarse del tope de solicitudes.
pub fn abortado_por_el_tope(max_herramientas: u32, detalle_entrega: &str) -> String {
    format!(
        "🔴 Programator abortó el encargo: superó las {max_herramientas} solicitudes de herramienta \
         sin cerrarlo. {detalle_entrega}"
    )
}

/// El motivo de un encargo abortado porque el modelo insistió en una herramienta denegada.
pub fn abortado_por_insistir(nombre: &str, motivo: &str, detalle_entrega: &str) -> String {
    format!(
        "🔴 Programator abortó el encargo: insistió en «{nombre}» tras una \
         denegación. Último motivo: {motivo}. {detalle_entrega}"
    )
}

/// Lo que recibe el modelo tras repetir una llamada idéntica a otra anterior.
pub fn resultado_repetido(nombre: &str, salida_previa: &str) -> String {
    format!(
        "Resultado de «{nombre}» (repetición):\n\n{salida_previa}\n\n[Aviso del arnés: Esta llamada es idéntica a una anterior. Se devuelve el resultado previo sin volver a ejecutarla y sin gastar cupo. Rectifica o pasa a la siguiente acción.]"
    )
}

/// Lo que recibe el modelo tras una herramienta concedida.
pub fn resultado_concedido(nombre: &str, salida: &str) -> String {
    format!("Resultado de «{nombre}»:\n\n{salida}")
}

/// Lo que recibe el modelo tras una herramienta denegada.
pub fn resultado_denegado(nombre: &str, motivo: &str) -> String {
    format!("Denegada «{nombre}»: {motivo}. Rectifica o usa otra herramienta.")
}

/// Los nombres entre comillas latinas y separados por comas: «a», «b». La única forma de listar
/// propuestas en los avisos.
pub(crate) fn lista_entre_comillas(nombres: &[String]) -> String {
    nombres
        .iter()
        .map(|nombre| format!("«{nombre}»"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Describe el estado de las propuestas dejadas en disco al momento de abortar un encargo.
///
/// Si el modelo alcanzó a escribir propuestas antes de que el arnés cortara el encargo,
/// el mensaje debe reflejar fielmente lo que quedó en disco en vez de afirmar que
/// «No hay entrega» (visto en uso real).
pub fn estado_entrega_propuestas(propuestas: &[String]) -> String {
    if propuestas.is_empty() {
        "No hay entrega.".to_string()
    } else if propuestas.len() == 1 {
        format!("Quedó en disco la propuesta «{}».", propuestas[0])
    } else {
        let lista = lista_entre_comillas(propuestas);
        format!("Quedaron en disco las propuestas: {lista}.")
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El texto entero de los dos motivos de abortar por el motor. Si se pierden las
    /// continuaciones `\`, quedan tiras de espacios dentro del texto, y una prueba que solo busque
    /// trozos no lo ve.
    #[test]
    fn los_motivos_del_motor_se_leen_de_corrido() {
        let error = Error::MotorSinRespuesta("conexión rechazada".into());
        assert_eq!(
            motor_no_disponible(0, EstadoServidor::NoDisponible, &error),
            format!(
                "🔴 Programator no pudo completar el encargo: el motor no respondió tras 1 intento \
                 y la comprobación de salud indica que el servidor no está disponible. Último \
                 error del motor: {error}"
            )
        );
        assert_eq!(
            motor_sin_respuesta(1, EstadoServidor::Listo, &error),
            format!(
                "🔴 Programator no pudo completar el encargo: el motor no respondió tras 1 \
                 reintento (2 intentos en total). Última comprobación de salud: el servidor está \
                 listo y responde. Último error del motor: {error}"
            )
        );
    }

    #[test]
    fn la_lista_va_entre_comillas_latinas_y_separada_por_comas() {
        let nombres = ["a.rs".to_string(), "b.rs".to_string()];
        assert_eq!(lista_entre_comillas(&nombres), "«a.rs», «b.rs»");
        assert_eq!(lista_entre_comillas(&nombres[..1]), "«a.rs»");
        assert_eq!(lista_entre_comillas(&[]), "");
    }
}
