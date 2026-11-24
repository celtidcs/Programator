//! La línea `Clase:` de un encargo: con qué clase de trabajo lo etiqueta el agente que dirige, para
//! que Programator cargue el modelo que mejor la hace.
//!
//! Es opcional. Solo cuenta al principio de una línea propia: `Clase:` a mitad de una frase es
//! texto del encargo, no una petición. Si hay varias, vale la primera. Quién puede escribirla lo
//! decide quien llama: la lee del encargo que escribió el agente, nunca de lo que diga el modelo.

use crate::banco::resultados::FilaDeDesempeno;

/// La palabra que abre la línea, sin los dos puntos. Se compara sin distinguir mayúsculas.
const ETIQUETA: &str = "clase";

/// Lo que pide un encargo sobre la clase de trabajo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClasePedida {
    /// No trae línea `Clase:`: se usa el modelo por defecto.
    Ninguna,
    /// Una clase que existe.
    Valida(FilaDeDesempeno),
    /// Trae la línea, pero con un nombre que no es de ninguna clase. Se guarda tal cual para
    /// decirle al agente qué escribió.
    Desconocida(String),
}

/// Lee la primera línea `Clase:` del texto de un encargo.
pub fn clase_pedida(texto: &str) -> ClasePedida {
    let Some(valor) = texto.lines().find_map(valor_de_la_linea) else {
        return ClasePedida::Ninguna;
    };
    match FilaDeDesempeno::por_nombre(valor) {
        Some(fila) => ClasePedida::Valida(fila),
        None => ClasePedida::Desconocida(valor.to_string()),
    }
}

/// El aviso que va en la entrega cuando la clase pedida no existe: dice lo que se escribió, que
/// se usó el modelo por defecto y qué clases valen.
pub fn mensaje_clase_desconocida(escrita: &str) -> String {
    let validas: Vec<String> = crate::banco::resultados::FILAS_DE_DESEMPENO
        .iter()
        .map(|fila| format!("`{}`", fila.nombre_corto()))
        .collect();
    format!(
        "⚠️ La línea `Clase: {escrita}` no nombra ninguna clase: este encargo se ha atendido con \
         el modelo por defecto. Las clases que valen son {}.",
        validas.join(", ")
    )
}

/// Lo que va tras `Clase:` si la línea empieza por ella, sin las marcas de lista ni de negrita que
/// un agente pone a veces (`- **Clase:** diagnostico`).
fn valor_de_la_linea(linea: &str) -> Option<&str> {
    let limpia = linea
        .trim_start()
        .trim_start_matches(['-', '*', ' '])
        .trim_start();
    let (etiqueta, resto) = limpia.split_once(':')?;
    if !etiqueta
        .trim()
        .trim_end_matches('*')
        .eq_ignore_ascii_case(ETIQUETA)
    {
        return None;
    }
    Some(resto.trim().trim_matches(['*', '`']).trim())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::FILAS_DE_DESEMPENO;

    #[test]
    fn la_negrita_vale_con_los_dos_puntos_dentro_o_fuera() {
        for linea in ["- **Clase:** diagnostico", "- **Clase**: diagnostico"] {
            assert_eq!(
                clase_pedida(linea),
                ClasePedida::Valida(FilaDeDesempeno::Diagnostico),
                "{linea}"
            );
        }
    }

    #[test]
    fn cada_clase_se_lee_por_su_nombre_corto() {
        for fila in FILAS_DE_DESEMPENO {
            let texto = format!("Audita src/pago.rs.\nClase: {}\n", fila.nombre_corto());
            assert_eq!(clase_pedida(&texto), ClasePedida::Valida(fila));
        }
    }

    #[test]
    fn da_igual_mayusculas_tildes_y_marcas_de_lista() {
        assert_eq!(
            clase_pedida("Busca el fallo.\n- **Clase:** Diagnóstico"),
            ClasePedida::Valida(FilaDeDesempeno::Diagnostico)
        );
        assert_eq!(
            clase_pedida("CLASE: `validaciones`"),
            ClasePedida::Valida(FilaDeDesempeno::Validaciones)
        );
    }

    #[test]
    fn sin_linea_no_pide_nada() {
        assert_eq!(clase_pedida("Audita src/pago.rs."), ClasePedida::Ninguna);
    }

    #[test]
    fn una_clase_que_no_existe_se_devuelve_tal_cual() {
        assert_eq!(
            clase_pedida("Clase: traducir"),
            ClasePedida::Desconocida("traducir".to_string())
        );
    }

    #[test]
    fn con_dos_lineas_vale_la_primera() {
        assert_eq!(
            clase_pedida("Clase: formas\nClase: diagnostico"),
            ClasePedida::Valida(FilaDeDesempeno::Formas)
        );
    }

    #[test]
    fn clase_a_mitad_de_una_frase_no_cuenta() {
        assert_eq!(
            clase_pedida("Revisa la clase: diagnostico que te pasé."),
            ClasePedida::Ninguna
        );
        assert_eq!(
            clase_pedida("Usa la Clase: formas, por favor."),
            ClasePedida::Ninguna
        );
    }
}
