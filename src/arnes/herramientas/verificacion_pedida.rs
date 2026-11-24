//! `verificar`: la verificación del proyecto que pide el modelo.
//!
//! Hoy no la ejecuta nadie, y su ficha lo dice (`Estado::Pendiente`). Aun así se comprueba lo que
//! se pide —el argumento y la lista blanca— para que la denegación le diga al modelo si su petición
//! sería válida. No ejecuta nada ni toca disco.

use super::{argumentos, mensajes, Decision};
use crate::motor::SolicitudHerramienta;

/// La lista blanca: las únicas verificaciones que el modelo puede pedir. El arnés compone el
/// comando entero; el modelo solo elige de aquí.
const VERIFICACIONES: &[&str] = &["formato", "clippy", "pruebas", "estado_git", "diferencias"];

/// Atiende un `verificar` mientras su ficha lo tenga pendiente, con el `motivo` de la ficha.
pub(super) fn atender(s: &SolicitudHerramienta, motivo: &str) -> Decision {
    let cual = match argumentos::texto(s, "cual") {
        Ok(c) => c,
        Err(d) => return d,
    };
    if !VERIFICACIONES.contains(&cual) {
        return Decision::Denegada(mensajes::fuera_de_la_lista_blanca(cual, VERIFICACIONES));
    }
    // El arnés compone el comando entero: el modelo nunca aporta argumentos sueltos. Pero hoy
    // *nadie ejecuta* la verificación: `ciclo.rs` trata toda concesión igual y no mira el
    // nombre de la herramienta. Conceder aquí sería decirle al modelo que su verificación
    // pasó; él lo publicaría como hecho comprobable en un fichero que leen los otros tres
    // agentes, y el protocolo dice que nada se da por bueno porque lo diga quien lo hizo. Mientras
    // no haya quien la ejecute, lo honrado es denegar.
    Decision::Denegada(mensajes::verificacion_pendiente(cual, motivo))
}
