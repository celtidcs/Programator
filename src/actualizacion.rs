//! Aviso, puramente informativo, de si hay una versión más nueva de Programator en GitHub.
//!
//! **Solo avisa.** No descarga nada, no reemplaza el ejecutable y no decide nada por el Director:
//! es una línea en consola al arrancar, nada más. Y **nunca puede impedir que Programator arranque
//! ni atienda encargos**: sin red, con GitHub caído o con una respuesta que no se entiende, la
//! comprobación se calla y el arranque sigue exactamente igual que si no existiera.

use serde::Deserialize;
use std::time::Duration;

/// Lo único que se necesita de la respuesta de la API de GitHub: la etiqueta de la última release.
#[derive(Debug, Deserialize)]
struct RespuestaUltimaRelease {
    tag_name: String,
}

/// ¿Es `ultima` una versión más nueva que `actual`?
///
/// Función pura: compara cifra a cifra (mayor, menor, parche), ignorando un posible prefijo `v`.
/// Si cualquiera de las dos no tiene forma de versión reconocible, la respuesta es `false`: ante la
/// duda, no se avisa de una versión nueva que no se puede confirmar que lo sea.
pub fn hay_version_mas_nueva(actual: &str, ultima: &str) -> bool {
    match (
        componentes_de_version(actual),
        componentes_de_version(ultima),
    ) {
        (Some(actual), Some(ultima)) => ultima > actual,
        _ => false,
    }
}

/// Descompone una cadena `"v1.2.3"` o `"1.2.3"` en sus tres componentes numéricos.
///
/// `None` si no tiene exactamente esa forma: una etiqueta de release con sufijo (`"1.2.3-rc1"`) u
/// otro formato no se interpreta a ciegas, se descarta.
fn componentes_de_version(cadena: &str) -> Option<(u32, u32, u32)> {
    let sin_prefijo = cadena.strip_prefix(['v', 'V']).unwrap_or(cadena);
    let partes: Vec<&str> = sin_prefijo.split('.').collect();
    let [mayor, menor, parche] = partes[..] else {
        return None;
    };
    Some((
        mayor.parse().ok()?,
        menor.parse().ok()?,
        parche.parse().ok()?,
    ))
}

/// Lo que se supo al intentar comprobar si hay una versión más nueva en GitHub.
///
/// Antes esto era un `Option<String>`: `Some` con el aviso si había versión nueva, `None` en
/// cualquier otro caso. Eso mezclaba dos desenlaces bien distintos bajo el mismo silencio: «se
/// comprobó y ya tienes la última» y «no se pudo ni comprobar» (sin red, GitHub caído, tiempo
/// agotado). El Director pidió distinguirlos, porque desde la terminal no hay forma de saber cuál
/// de los dos ha pasado (2026-10-01).
#[derive(Debug, PartialEq, Eq)]
pub enum Comprobacion {
    /// Hay una versión más nueva publicada: el aviso ya redactado, listo para imprimir.
    VersionNueva(String),
    /// Se comprobó sin problema y la instalada ya es la más reciente publicada.
    AlDia,
    /// No se pudo completar la comprobación (sin red, GitHub caído, tiempo agotado o una respuesta
    /// que no tiene la forma esperada). Sigue sin ser un error que haya que propagar: solo cambia
    /// de qué informa la terminal.
    NoComprobado,
}

/// Redacta el aviso de que hay una versión más nueva disponible.
pub fn mensaje_version_disponible(actual: &str, ultima: &str, repositorio: &str) -> String {
    format!(
        "📦 Hay una versión nueva de Programator disponible: {ultima} (tienes {actual}). \
         https://github.com/{repositorio}/releases/latest"
    )
}

/// Redacta la confirmación de que la versión instalada ya es la más reciente publicada.
pub fn mensaje_al_dia(actual: &str) -> String {
    format!("✅ Programator {actual} ya es la versión más reciente publicada en GitHub.")
}

/// Redacta el aviso de que no se pudo comprobar si hay una versión más nueva.
///
/// No dice por qué (sin red, GitHub caído, tiempo agotado y un JSON inesperado se tratan igual):
/// ninguno de esos motivos cambia qué puede hacer quien lee el aviso, que es nada, así que
/// distinguirlos en el mensaje solo añadiría ruido.
pub fn mensaje_no_comprobado() -> String {
    "⚠️ No se pudo comprobar si hay una versión más nueva de Programator en GitHub (sin red, \
     GitHub no respondió a tiempo, o la respuesta no tenía el formato esperado). No afecta al \
     arranque; no hace falta hacer nada."
        .to_string()
}

/// Comprueba contra GitHub si hay una versión más nueva que `version_actual`.
///
/// **Nunca falla hacia afuera.** Cualquier problema —sin red, tiempo agotado, GitHub caído, JSON
/// inesperado— se traduce en `Comprobacion::NoComprobado`, nunca en un error que alguien tenga que
/// gestionar. Es coherente con lo que dice este módulo entero: es una comodidad, no una garantía.
pub fn comprobar_version_mas_reciente(
    repositorio: &str,
    version_actual: &str,
    tiempo_espera: Duration,
) -> Comprobacion {
    let url = format!("https://api.github.com/repos/{repositorio}/releases/latest");
    let agente = ureq::AgentBuilder::new().timeout(tiempo_espera).build();
    let Ok(respuesta) = agente
        .get(&url)
        // La API de GitHub exige un User-Agent identificable; sin él, rechaza la petición.
        .set("User-Agent", "Programator")
        .call()
    else {
        return Comprobacion::NoComprobado;
    };
    let Ok(cuerpo) = respuesta.into_json::<RespuestaUltimaRelease>() else {
        return Comprobacion::NoComprobado;
    };

    if hay_version_mas_nueva(version_actual, &cuerpo.tag_name) {
        Comprobacion::VersionNueva(mensaje_version_disponible(
            version_actual,
            &cuerpo.tag_name,
            repositorio,
        ))
    } else {
        Comprobacion::AlDia
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_version_mayor_es_mas_nueva() {
        assert!(hay_version_mas_nueva("0.10.4", "0.11.0"));
        assert!(hay_version_mas_nueva("0.10.4", "1.0.0"));
        assert!(hay_version_mas_nueva("0.10.4", "0.10.5"));
    }

    #[test]
    fn la_misma_version_no_es_mas_nueva() {
        assert!(!hay_version_mas_nueva("0.10.4", "0.10.4"));
    }

    #[test]
    fn una_version_anterior_no_es_mas_nueva() {
        assert!(!hay_version_mas_nueva("0.10.4", "0.9.9"));
        assert!(!hay_version_mas_nueva("0.10.4", "0.10.3"));
    }

    #[test]
    fn el_prefijo_v_no_afecta_a_la_comparacion() {
        assert!(hay_version_mas_nueva("0.10.4", "v0.11.0"));
        assert!(hay_version_mas_nueva("v0.10.4", "0.11.0"));
    }

    #[test]
    fn una_cadena_sin_forma_de_version_no_se_declara_mas_nueva() {
        assert!(!hay_version_mas_nueva("0.10.4", "ultima"));
        assert!(!hay_version_mas_nueva("0.10.4", "0.11.0-rc1"));
        assert!(!hay_version_mas_nueva("0.10.4", ""));
    }

    #[test]
    fn el_mensaje_nombra_las_dos_versiones_y_el_repositorio() {
        let mensaje = mensaje_version_disponible("0.10.4", "0.11.0", "celtidcs/Programator");

        assert!(mensaje.contains("0.10.4"));
        assert!(mensaje.contains("0.11.0"));
        assert!(mensaje.contains("celtidcs/Programator"));
        assert!(mensaje.contains("https://github.com/celtidcs/Programator/releases"));
    }

    #[test]
    fn el_mensaje_al_dia_nombra_la_version_instalada() {
        let mensaje = mensaje_al_dia("0.11.0");
        assert!(mensaje.contains("0.11.0"));
        assert!(mensaje.contains("más reciente"));
    }

    #[test]
    fn el_mensaje_no_comprobado_no_culpa_a_nadie_y_dice_que_no_hace_falta_actuar() {
        let mensaje = mensaje_no_comprobado();
        assert!(mensaje.contains("No se pudo comprobar"));
        assert!(mensaje.contains("no hace falta hacer nada"));
    }

    #[test]
    #[ignore = "toca la red de verdad (api.github.com): se lanza a mano, igual que las pruebas de GPU y GGUF de §3 de CLAUDE.md"]
    fn un_repositorio_que_no_contesta_a_tiempo_no_se_declara_version_nueva() {
        // Un tiempo de espera absurdamente corto contra la API real: la petición no puede
        // completarse a tiempo. Comprueba que un fallo de red se traduce en «no se pudo
        // comprobar», no en un panic ni un error propagado.
        let resultado = comprobar_version_mas_reciente(
            "celtidcs/Programator",
            "0.10.4",
            Duration::from_millis(1),
        );
        assert_eq!(resultado, Comprobacion::NoComprobado);
    }
}
