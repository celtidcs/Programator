//! Cómo usa `llama-server` la atención rápida: `[motor] flash_attn`, que pasa tal cual a su
//! `--flash-attn`. Lo que cuesta cada opción está contado en `programator.ejemplo.toml`.

use serde::Deserialize;

/// Las tres maneras que admite `--flash-attn` en las builds modernas de llama.cpp (la b10993
/// empaquetada): ya no es una bandera suelta, lleva siempre una de estas palabras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AtencionRapida {
    /// `"on"`. Lo que Programator ha pasado siempre, desde el primer arranque real del
    /// 22/09/2026: con ella cabe la caché cuantizada de `cache_kv`.
    #[default]
    Activada,
    /// `"off"`.
    Desactivada,
    /// `"auto"`: decide el motor.
    Automatica,
}

/// Cada manera con la palabra que la nombra en el TOML y en la línea del motor.
const PALABRAS: [(AtencionRapida, &str); 3] = [
    (AtencionRapida::Activada, "on"),
    (AtencionRapida::Desactivada, "off"),
    (AtencionRapida::Automatica, "auto"),
];

impl AtencionRapida {
    /// La palabra que se le pasa a `--flash-attn`.
    pub fn palabra(self) -> &'static str {
        PALABRAS
            .iter()
            .find(|(manera, _)| *manera == self)
            .map_or("on", |(_, palabra)| palabra)
    }
}

impl<'de> Deserialize<'de> for AtencionRapida {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        let minusculas = texto.trim().to_lowercase();
        PALABRAS
            .iter()
            .find(|(_, palabra)| *palabra == minusculas)
            .map(|(manera, _)| *manera)
            .ok_or_else(|| serde::de::Error::custom(mensaje_flash_attn_no_vale(&texto)))
    }
}

/// El error cuando `flash_attn` trae una palabra que no es ninguna de las tres que valen.
fn mensaje_flash_attn_no_vale(texto: &str) -> String {
    format!("«{texto}» no vale para flash_attn: escribe \"on\", \"off\" o \"auto\"")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[derive(Deserialize)]
    struct Envoltorio {
        flash_attn: AtencionRapida,
    }

    fn leer(valor: &str) -> Result<AtencionRapida, toml::de::Error> {
        toml::from_str::<Envoltorio>(&format!("flash_attn = {valor}")).map(|e| e.flash_attn)
    }

    #[test]
    fn por_defecto_va_activada() {
        assert_eq!(AtencionRapida::default(), AtencionRapida::Activada);
        assert_eq!(AtencionRapida::default().palabra(), "on");
    }

    #[test]
    fn cada_palabra_se_lee_y_se_devuelve_igual() {
        for palabra in ["on", "off", "auto"] {
            let manera = leer(&format!("\"{palabra}\"")).unwrap();
            assert_eq!(manera.palabra(), palabra);
        }
        assert_eq!(leer("\"OFF\"").unwrap(), AtencionRapida::Desactivada);
    }

    #[test]
    fn otra_palabra_se_rechaza_nombrandola() {
        let fallo = leer("\"si\"").unwrap_err().to_string();
        assert!(fallo.contains("«si» no vale para flash_attn"), "{fallo}");
    }
}
