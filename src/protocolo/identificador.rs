//! El identificador estable de un encargo: lo que permite reconocerlo en el buzón aunque cambie
//! lo que lo rodea.
//!
//! Se calcula con el buzón, el `LATIDO:` de la intervención que lo contiene y el texto con los
//! saltos de línea normalizados a `\n` y sin espacios al final de línea. Así un cambio de CRLF no
//! lo hace nuevo, y dos encargos con el mismo texto en intervenciones distintas son dos.

/// Cuántos caracteres hexadecimales de la huella SHA-256 se guardan: 64 bits, de sobra para los
/// encargos de un canal, y corto para nombrar el fichero de su ficha.
pub const LARGO_DEL_IDENTIFICADOR: usize = 16;

/// Lo que separa las partes antes de calcular la huella: un carácter que no aparece en un buzón
/// de texto, para que «a» + «bc» no se confunda con «ab» + «c».
const SEPARADOR: char = '\0';

/// El identificador de un encargo del buzón `de`, escrito en la intervención del latido `latido`
/// (si la hay) con el texto `texto`.
pub fn identificador(de: &str, latido: Option<&str>, texto: &str) -> String {
    let entrada = format!(
        "{de}{SEPARADOR}{}{SEPARADOR}{}",
        latido.unwrap_or_default().trim(),
        texto_normalizado(texto)
    );
    let mut huella = crate::ficheros::huella_sha256_de_bytes(entrada.as_bytes());
    huella.truncate(LARGO_DEL_IDENTIFICADOR);
    huella
}

/// El texto con los saltos en `\n`, sin espacios al final de cada línea ni líneas vacías en los
/// extremos.
fn texto_normalizado(texto: &str) -> String {
    texto
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const LATIDO: Option<&str> = Some("2026-11-25 10:00:00");

    #[test]
    fn el_id_tiene_el_largo_fijado_y_es_hexadecimal() {
        let id = identificador("claude.md", LATIDO, "haz esto");
        assert_eq!(id.len(), LARGO_DEL_IDENTIFICADOR);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "{id}");
    }

    #[test]
    fn el_id_no_cambia_con_crlf() {
        assert_eq!(
            identificador("claude.md", LATIDO, "Para Programator: uno\ndos"),
            identificador("claude.md", LATIDO, "Para Programator: uno\r\ndos")
        );
    }

    #[test]
    fn el_id_no_cambia_con_espacios_al_final() {
        assert_eq!(
            identificador("claude.md", LATIDO, "uno\ndos"),
            identificador("claude.md", LATIDO, "uno   \ndos\t\n")
        );
    }

    #[test]
    fn dos_encargos_iguales_en_intervenciones_distintas_tienen_id_distinto() {
        assert_ne!(
            identificador("claude.md", Some("2026-11-25 10:00:00"), "haz esto"),
            identificador("claude.md", Some("2026-11-25 11:00:00"), "haz esto")
        );
    }

    #[test]
    fn el_mismo_encargo_en_otro_buzon_tiene_id_distinto() {
        assert_ne!(
            identificador("claude.md", LATIDO, "haz esto"),
            identificador("codex.md", LATIDO, "haz esto")
        );
    }

    #[test]
    fn editar_el_texto_cambia_el_id() {
        assert_ne!(
            identificador("claude.md", LATIDO, "haz esto"),
            identificador("claude.md", LATIDO, "haz esto otro")
        );
    }

    /// Sin separador, «a» + «bc» y «ab» + «c» darían la misma entrada.
    #[test]
    fn las_partes_no_se_confunden_entre_si() {
        assert_ne!(
            identificador("a", Some("b"), "c"),
            identificador("ab", None, "c")
        );
    }
}
