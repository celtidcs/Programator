//! Cantidades con el sustantivo concordado, para los textos que lee una persona: que no salga
//! «1 rúbricas», «1 ficheros» ni «1 entradas».

/// La forma que toca con `n`: el singular con uno y el plural con cualquier otro número, también
/// con cero («0 rúbricas»).
pub fn segun<'a>(n: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if n == 1 {
        singular
    } else {
        plural
    }
}

/// `n` seguido de la forma que toca: «1 rúbrica», «3 rúbricas».
pub fn cantidad(n: usize, singular: &str, plural: &str) -> String {
    format!("{n} {}", segun(n, singular, plural))
}

/// «a», «a y b», «a, b y c»; nada si no hay partes. La única forma de juntar una lista en un
/// texto para personas.
pub fn juntar_con_y(mut partes: Vec<String>) -> String {
    match partes.len() {
        0 => String::new(),
        1 => partes.remove(0),
        _ => {
            let ultima = partes.pop().unwrap_or_default();
            format!("{} y {ultima}", partes.join(", "))
        }
    }
}

/// Los nombres entre comillas latinas y separados por comas: «a», «b». La única forma de listar
/// nombres de ficheros o de modelos en los avisos.
pub fn lista_entre_comillas(nombres: &[String]) -> String {
    nombres
        .iter()
        .map(|nombre| format!("«{nombre}»"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_ultima_parte_va_con_y_y_las_demas_con_coma() {
        let partes = |p: &[&str]| p.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(juntar_con_y(partes(&[])), "");
        assert_eq!(juntar_con_y(partes(&["a"])), "a");
        assert_eq!(juntar_con_y(partes(&["a", "b"])), "a y b");
        assert_eq!(juntar_con_y(partes(&["a", "b", "c"])), "a, b y c");
    }

    #[test]
    fn uno_va_en_singular_y_los_demas_en_plural() {
        assert_eq!(cantidad(1, "rúbrica", "rúbricas"), "1 rúbrica");
        assert_eq!(cantidad(0, "rúbrica", "rúbricas"), "0 rúbricas");
        assert_eq!(cantidad(2, "rúbrica", "rúbricas"), "2 rúbricas");
        assert_eq!(segun(1, "Queda", "Quedan"), "Queda");
        assert_eq!(segun(5, "Queda", "Quedan"), "Quedan");
    }
}
