//! La clave con que Programator protege su `llama-server`: `[motor] clave`.
//!
//! Sin clave, `llama-server` acepta peticiones de cualquier página web abierta en el navegador del
//! mismo equipo (lo avisa él mismo al arrancar; PR https://github.com/ggml-org/llama.cpp/pull/25655).
//! Con `"auto"`, que es lo de fábrica, Programator inventa una nueva en cada arranque y nadie más la
//! conoce. Un texto fijo sirve para usar el mismo motor desde otra herramienta.

use serde::Deserialize;

/// La palabra que deja la clave en manos de Programator.
const PALABRA_AUTO: &str = "auto";

/// Lo que dice el TOML de la clave del motor.
#[derive(Clone, PartialEq, Eq, Default)]
pub enum ClaveConfigurada {
    /// Una clave nueva en cada arranque del motor.
    #[default]
    Auto,
    /// Esta clave, siempre.
    Fija(String),
}

/// La clave no se imprime nunca, ni siquiera al depurar la configuración.
impl std::fmt::Debug for ClaveConfigurada {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaveConfigurada::Auto => f.write_str("Auto"),
            ClaveConfigurada::Fija(_) => f.write_str("Fija([oculta])"),
        }
    }
}

impl<'de> Deserialize<'de> for ClaveConfigurada {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)
            .map_err(|_| serde::de::Error::custom(mensaje_de_tipo_invalido()))?;
        leer(&texto).map_err(serde::de::Error::custom)
    }
}

/// Lo que vale un texto del TOML como clave.
fn leer(texto: &str) -> Result<ClaveConfigurada, String> {
    if texto.trim().is_empty() {
        return Err(mensaje_de_clave_vacia());
    }
    if texto.eq_ignore_ascii_case(PALABRA_AUTO) {
        return Ok(ClaveConfigurada::Auto);
    }
    Ok(ClaveConfigurada::Fija(texto.to_string()))
}

/// El mensaje para una clave que no es un texto.
fn mensaje_de_tipo_invalido() -> String {
    format!("[motor] clave solo admite un texto o la palabra «{PALABRA_AUTO}»")
}

/// El mensaje para una clave vacía, que dejaría el motor abierto creyendo que está cerrado.
fn mensaje_de_clave_vacia() -> String {
    format!(
        "[motor] clave no puede estar vacía: escribe «{PALABRA_AUTO}» para que Programator invente \
         una en cada arranque, o la clave que quieras"
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[derive(Deserialize)]
    struct Envoltorio {
        clave: ClaveConfigurada,
    }

    fn de(valor: &str) -> Result<ClaveConfigurada, String> {
        toml::from_str::<Envoltorio>(&format!("clave = {valor}"))
            .map(|e| e.clave)
            .map_err(|fallo| fallo.to_string())
    }

    #[test]
    fn la_clave_por_defecto_es_auto() {
        assert_eq!(ClaveConfigurada::default(), ClaveConfigurada::Auto);
        assert_eq!(de("\"auto\""), Ok(ClaveConfigurada::Auto));
        assert_eq!(de("\"AUTO\""), Ok(ClaveConfigurada::Auto));
    }

    #[test]
    fn una_clave_fija_se_lee() {
        assert_eq!(
            de("\"mi-clave\""),
            Ok(ClaveConfigurada::Fija("mi-clave".into()))
        );
    }

    #[test]
    fn una_clave_vacia_es_un_error() {
        for vacia in ["\"\"", "\"   \""] {
            let fallo = de(vacia).unwrap_err();
            assert!(fallo.contains("no puede estar vacía"), "{fallo}");
        }
        let numero = de("7").unwrap_err();
        assert!(numero.contains("solo admite un texto"), "{numero}");
    }

    #[test]
    fn el_debug_no_muestra_la_clave_fija() {
        let depurado = format!("{:?}", ClaveConfigurada::Fija("secreta".into()));
        assert!(!depurado.contains("secreta"), "{depurado}");
    }
}
