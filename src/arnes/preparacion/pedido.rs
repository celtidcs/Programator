//! `preparar: <propuesta> en <ruta>`: el agente pide ver una propuesta formateada, compilada y
//! probada en una copia del proyecto, sin que nadie toque el proyecto.
//!
//! Lo reconoce el arnés, como las confirmaciones, y solo con los dos puntos: «preparar el
//! informe…» sin ellos es un encargo normal, para el modelo.

/// La palabra, con los dos puntos. Se compara sin distinguir mayúsculas.
const PALABRA_DE_PREPARAR: &str = "preparar:";

/// Lo que separa la propuesta de la ruta. Se toma el último, por si el nombre de la propuesta lo
/// lleva dentro.
const SEPARADOR_DE_LA_RUTA: &str = " en ";

/// Lo que se quita alrededor de la propuesta y de la ruta: como se citan en el canal.
const ADORNOS: [char; 5] = ['`', '*', '«', '»', '"'];

/// Lo que pide un `preparar:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pedido {
    /// El nombre de la propuesta en la carpeta de candidatos; vacío si no lo trae.
    pub propuesta: String,
    /// Dónde iría en el proyecto, relativa a su raíz; vacía si no la trae.
    pub ruta: String,
}

impl Pedido {
    /// Si trae las dos cosas. Si no, se contesta cómo se pide.
    pub fn esta_completo(&self) -> bool {
        !self.propuesta.is_empty() && !self.ruta.is_empty()
    }
}

/// El pedido que es el cuerpo de un encargo, si lo es.
pub fn reconocer(cuerpo: &str) -> Option<Pedido> {
    let primera = crate::protocolo::encargo::primera_linea_con_texto(cuerpo)?;
    let cabeza = primera.get(..PALABRA_DE_PREPARAR.len())?;
    if !cabeza.eq_ignore_ascii_case(PALABRA_DE_PREPARAR) {
        return None;
    }
    let resto = primera[PALABRA_DE_PREPARAR.len()..].trim();
    let (propuesta, ruta) = resto
        .rsplit_once(SEPARADOR_DE_LA_RUTA)
        .unwrap_or((resto, ""));
    let limpio = |texto: &str| texto.trim().trim_matches(ADORNOS).trim().to_string();
    Some(Pedido {
        propuesta: limpio(propuesta),
        ruta: limpio(ruta),
    })
}

/// Si el cuerpo de un encargo es un `preparar:`.
pub fn es_preparacion(cuerpo: &str) -> bool {
    reconocer(cuerpo).is_some()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn pedido(propuesta: &str, ruta: &str) -> Option<Pedido> {
        Some(Pedido {
            propuesta: propuesta.into(),
            ruta: ruta.into(),
        })
    }

    #[test]
    fn se_reconoce_con_la_propuesta_y_la_ruta() {
        assert_eq!(
            reconocer("preparar: `lib-002.rs` en `src/lib.rs`"),
            pedido("lib-002.rs", "src/lib.rs")
        );
        assert_eq!(
            reconocer("  Preparar:  «a en b.py» en src/a.py\nmás texto"),
            pedido("a en b.py", "src/a.py")
        );
    }

    /// Sin los dos puntos es un encargo para el modelo.
    #[test]
    fn sin_los_dos_puntos_no_es_un_pedido() {
        assert_eq!(reconocer("preparar el informe en src/x.rs"), None);
        assert!(!es_preparacion("prepara: x en y"));
        assert!(!es_preparacion(""));
    }

    /// Con los dos puntos es suyo aunque falte algo: se le contesta cómo se pide.
    #[test]
    fn un_pedido_a_medias_es_suyo_pero_no_esta_completo() {
        let sin_ruta = reconocer("preparar: x.rs").unwrap();
        assert_eq!(sin_ruta, pedido("x.rs", "").unwrap());
        assert!(!sin_ruta.esta_completo());
        assert!(!reconocer("preparar:").unwrap().esta_completo());
        assert!(!reconocer("preparar: en src/x.rs").unwrap().esta_completo());
        assert!(reconocer("preparar: x en y").unwrap().esta_completo());
    }
}
