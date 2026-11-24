//! La lectura de los argumentos JSON de una solicitud.
//!
//! Saca cada argumento con el tipo que pide el verbo o deniega diciendo **qué falta y qué hay**,
//! con la firma del verbo sacada de su ficha. No toca disco ni decide si el verbo se concede: solo
//! traduce lo que escribió el modelo.

use super::{mensajes, Decision};
use crate::motor::SolicitudHerramienta;

/// Saca un argumento de texto de la solicitud, o deniega diciendo **qué falta y qué hay**.
///
/// El modelo no tiene memoria entre encargos ni documentación de sus propios verbos: si la
/// denegación no le da la firma, solo puede adivinar. El 23/09/2026 adivinó cinco veces
/// seguidas y costó un encargo entero.
pub(super) fn texto<'a>(s: &'a SolicitudHerramienta, clave: &str) -> Result<&'a str, Decision> {
    match s.argumentos.get(clave) {
        None => Err(Decision::Denegada(mensajes::falta_el_argumento(
            clave,
            &recordatorio_de_firma(&s.nombre),
        ))),
        Some(valor) => valor.as_str().ok_or_else(|| {
            Decision::Denegada(mensajes::no_es_cadena(
                clave,
                &recordatorio_de_firma(&s.nombre),
            ))
        }),
    }
}

/// Saca un argumento numérico de línea (opcional) de la solicitud.
/// Admite números enteros positivos tanto en formato numérico JSON (`10`)
/// como en cadena (`"10"`); el 0 no, porque las líneas son base 1. Si se omite o es `null`,
/// devuelve `Ok(None)`.
pub(super) fn linea(s: &SolicitudHerramienta, clave: &str) -> Result<Option<usize>, Decision> {
    match s.argumentos.get(clave) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => {
            if let Some(u) = n
                .as_u64()
                .and_then(|u| usize::try_from(u).ok())
                .filter(|&u| u > 0)
            {
                Ok(Some(u))
            } else {
                Err(no_es_entero_positivo(s, clave))
            }
        }
        Some(serde_json::Value::String(cadena)) => {
            let limpio = cadena.trim();
            if limpio.is_empty() {
                Ok(None)
            } else if let Some(u) = limpio.parse::<usize>().ok().filter(|&u| u > 0) {
                Ok(Some(u))
            } else {
                Err(no_es_entero_positivo(s, clave))
            }
        }
        Some(_) => Err(no_es_entero_positivo(s, clave)),
    }
}

/// Un argumento sí o no: `true`/`false`, también escritos como texto. Si falta, `false`.
pub(super) fn logico(s: &SolicitudHerramienta, clave: &str) -> Result<bool, Decision> {
    match s.argumentos.get(clave) {
        None | Some(serde_json::Value::Null) => Ok(false),
        Some(serde_json::Value::Bool(valor)) => Ok(*valor),
        Some(serde_json::Value::String(texto)) => match texto.trim() {
            "" | "false" => Ok(false),
            "true" => Ok(true),
            _ => Err(no_es_logico(s, clave)),
        },
        Some(_) => Err(no_es_logico(s, clave)),
    }
}

/// La denegación de un argumento que no es `true` ni `false`, con la firma del verbo.
fn no_es_logico(s: &SolicitudHerramienta, clave: &str) -> Decision {
    Decision::Denegada(mensajes::no_es_logico(
        clave,
        &recordatorio_de_firma(&s.nombre),
    ))
}

/// La denegación de un argumento de línea que no es un entero positivo, con la firma del verbo.
fn no_es_entero_positivo(s: &SolicitudHerramienta, clave: &str) -> Decision {
    Decision::Denegada(mensajes::no_es_entero_positivo(
        clave,
        &recordatorio_de_firma(&s.nombre),
    ))
}

/// La firma del verbo y para qué sirve cada argumento, en una frase.
///
/// Si el verbo no estuviera en la ficha —cosa que `atender` ya impide— se calla en vez de
/// inventar: una firma falsa sería peor que ninguna.
pub(super) fn recordatorio_de_firma(nombre: &str) -> String {
    mensajes::recordatorio_de_firma(crate::arnes::ficha::buscar(nombre))
}
