//! Lo que el ciclo de un encargo le dice al modelo y lo que deja escrito cuando lo corta.
//!
//! Un solo sitio para estos textos: los motivos de aborto acaban en el canal, a la vista de
//! quien encargó, y los avisos del arnés acaban en la conversación del modelo. El ciclo decide;
//! aquí solo se redacta.

/// Lo que se le dice al modelo cuando escribe la etiqueta de una herramienta como texto en vez de
/// invocarla (visto en uso real).
pub const AVISO_ETIQUETA_COMO_TEXTO: &str = "[Aviso del arnés: Has escrito la petición de una herramienta como texto plano, no como una llamada real. Para usar una herramienta, invócala mediante la llamada estructurada; no escribas su etiqueta.]";

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
        let lista = propuestas
            .iter()
            .map(|p| format!("«{p}»"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("Quedaron en disco las propuestas: {lista}.")
    }
}
