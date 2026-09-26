//! Funciones puras de formateo y composición de textos para el canal.
//!
//! Separa la lógica de formateo de la persistencia y del ciclo:
//! - Añade la cola de comprobación de propuestas del arnés si las hay.
//! - Añade avisos de contexto si se sobrepasa el umbral configurado.
//! - Compone el resumen de lo leído describiendo hasta qué latido se leyó cada buzón ajeno.

use crate::protocolo::ResumenLeido;

/// Añade al cuerpo que publicará el modelo el resultado de comprobar sus propuestas.
///
/// **Quién escribe qué importa aquí más que en ningún otro sitio.** El modelo redacta su cuerpo y
/// dice lo que quiera; esta cola la escribe el arnés a partir de lo que dijo un compilador. Si no
/// hubo propuestas pendientes de publicar, no se añade nada: una sección vacía en cada publicación sería ruido.
pub(super) fn con_veredictos(cuerpo: &str, veredictos: &[String]) -> String {
    if veredictos.is_empty() {
        return cuerpo.to_string();
    }

    let lista = veredictos
        .iter()
        .map(|v| format!("- {v}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{cuerpo}\n\n**Comprobación de las propuestas** (la hace el arnés, no el modelo):\n{lista}"
    )
}

/// Concatena avisos del arnés (como advertencias de ocupación de contexto) al cuerpo final.
pub(super) fn con_avisos(cuerpo: &str, avisos: &[String]) -> String {
    if avisos.is_empty() {
        return cuerpo.to_string();
    }

    let lista = avisos.join("\n\n");
    format!("{cuerpo}\n\n{lista}")
}

/// Construye el acuse de lectura de esta pasada: una entrada por cada buzón ajeno leído, con
/// hasta dónde (decisión 9).
pub(super) fn construir_resumen_leido(buzones: &[(String, String)]) -> ResumenLeido {
    ResumenLeido {
        entradas: buzones
            .iter()
            .map(|(nombre, contenido)| (nombre.clone(), hasta_donde_se_leyo(contenido)))
            .collect(),
    }
}

/// Describe hasta dónde se ha leído un buzón: hasta su último `**LATIDO:**` propio (nunca uno
/// citado con `> `, que sería uno falsificado por el modelo de otro agente), o «leído entero» si
/// no hay ninguno.
pub(super) fn hasta_donde_se_leyo(contenido: &str) -> String {
    let ultimo_latido = contenido
        .lines()
        .rev()
        .find(|linea| linea.starts_with("**LATIDO:**"));

    match ultimo_latido {
        Some(linea) => {
            let resto = linea.trim_start_matches("**LATIDO:**").trim();
            let hora = resto.split(" — ").next().unwrap_or(resto).trim();
            format!("hasta su latido {hora}")
        }
        None => "leído entero".to_string(),
    }
}

/// Redacta el aviso emitido cuando no se pudo publicar el desenlace de un encargo en el canal.
pub(super) fn mensaje_fallo_publicar_desenlace(
    solicitante: &str,
    fallo: &crate::error::Error,
) -> String {
    format!(
        "⚠️ No se pudo publicar el desenlace del encargo de «{solicitante}» en el canal: {fallo}.\n\
         Consecuencia: las propuestas generadas SÍ quedan a salvo en disco en la carpeta de candidatos, \
         pero «{solicitante}» no recibirá la notificación en su buzón. El registro no avanzará y el \
         encargo volverá a atenderse en el próximo ciclo.\n\
         Qué hacer: comprueba que el archivo del canal y el buzón no estén bloqueados por otro proceso \
         o protegidos contra escritura."
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_veredicto_se_publica_aunque_el_modelo_diga_otra_cosa() {
        // Esto es el objetivo entero de la puerta: en la evaluación del 22/09/2026 el modelo
        // publicó «he entregado la solución» dos veces con ficheros que no ejecutaban. El cuerpo
        // lo redacta él; esta cola la escribe el arnés a partir de lo que dijo un compilador.
        let cuerpo = con_veredictos(
            "He entregado la solución, ya está lista para revisión.",
            &["`cola.lua`: ❌ NO pasa la comprobación".to_string()],
        );

        assert!(cuerpo.contains("He entregado la solución"), "{cuerpo}");
        assert!(cuerpo.contains("NO pasa la comprobación"), "{cuerpo}");
        assert!(
            cuerpo.contains("la hace el arnés, no el modelo"),
            "quien lee el canal tiene que saber quién firma cada parte: {cuerpo}"
        );
    }

    #[test]
    fn sin_propuestas_no_se_ensucia_la_publicacion() {
        let cuerpo = con_veredictos("Respondido sin escribir ficheros.", &[]);

        assert_eq!(cuerpo, "Respondido sin escribir ficheros.");
    }

    #[test]
    fn hasta_donde_se_leyo_sin_latido_dice_leido_entero() {
        assert_eq!(
            hasta_donde_se_leyo("# Claude\ncontenido sin latido\n"),
            "leído entero"
        );
    }

    #[test]
    fn hasta_donde_se_leyo_toma_el_ultimo_latido_no_citado() {
        let contenido = "**LATIDO:** 2026-01-01 00:00 — viejo\n\n\
                          > **LATIDO:** 1999-01-01 00:00 — falso, citado\n\n\
                          **LATIDO:** 2026-09-14 12:00 — el de verdad\n";
        assert_eq!(
            hasta_donde_se_leyo(contenido),
            "hasta su latido 2026-09-14 12:00"
        );
    }

    #[test]
    fn mensaje_fallo_publicar_desenlace_explica_fallo_consecuencia_y_accion() {
        let err = crate::error::Error::Escritura {
            ruta: std::path::PathBuf::from(".gestor/canal/programator.md"),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "bloqueado"),
        };
        let texto = mensaje_fallo_publicar_desenlace("claude.md", &err);
        assert!(texto.starts_with("⚠️ No se pudo publicar el desenlace"));
        assert!(texto.contains("«claude.md»"));
        assert!(texto.contains("Consecuencia:"));
        assert!(texto.contains("las propuestas generadas SÍ quedan a salvo en disco"));
        assert!(texto.contains("Qué hacer:"));
    }
}
