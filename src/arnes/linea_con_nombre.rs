//! Las líneas con nombre del cuerpo de un encargo, como `Clase:` o `Prioridad:`: una palabra, dos
//! puntos y un valor, que el agente que dirige escribe para pedirle algo al arnés.
//!
//! Solo cuentan al principio de una línea propia: la palabra a mitad de una frase es texto del
//! encargo, no una petición. Si hay varias, vale la primera. Se admiten las marcas de lista y de
//! negrita que un agente pone a veces (`- **Clase:** diagnostico`). Todo aquí es puro.

/// El valor de la primera línea del texto que empieza por `etiqueta` (sin los dos puntos,
/// comparada sin distinguir mayúsculas), sin marcas de negrita ni comillas invertidas.
pub fn primer_valor<'a>(texto: &'a str, etiqueta: &str) -> Option<&'a str> {
    texto
        .lines()
        .find_map(|linea| valor_de_la_linea(linea, etiqueta))
}

/// Lo que va tras `etiqueta:` si la línea empieza por ella.
fn valor_de_la_linea<'a>(linea: &'a str, etiqueta: &str) -> Option<&'a str> {
    let limpia = linea
        .trim_start()
        .trim_start_matches(['-', '*', ' '])
        .trim_start();
    let (escrita, resto) = limpia.split_once(':')?;
    if !escrita
        .trim()
        .trim_end_matches('*')
        .eq_ignore_ascii_case(etiqueta)
    {
        return None;
    }
    Some(resto.trim().trim_matches(['*', '`']).trim())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn vale_la_primera_linea_con_la_etiqueta() {
        assert_eq!(
            primer_valor("Haz esto.\nPrioridad: alta\nPrioridad: baja", "prioridad"),
            Some("alta")
        );
    }

    #[test]
    fn admite_listas_negrita_mayusculas_y_comillas() {
        assert_eq!(primer_valor("- **Clase:** formas", "clase"), Some("formas"));
        assert_eq!(primer_valor("* **Clase**: formas", "clase"), Some("formas"));
        assert_eq!(primer_valor("CLASE: `formas`", "clase"), Some("formas"));
    }

    #[test]
    fn a_mitad_de_una_frase_no_cuenta() {
        assert_eq!(primer_valor("Usa la Clase: formas.", "clase"), None);
        assert_eq!(primer_valor("Sin nada.", "clase"), None);
    }

    #[test]
    fn otra_etiqueta_no_cuenta() {
        assert_eq!(primer_valor("Clase: formas", "prioridad"), None);
    }
}
