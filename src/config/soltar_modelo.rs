//! Cuándo suelta Programator el modelo de la tarjeta entre encargos: `[motor] soltar_modelo`.
//!
//! Se escribe como `capas_gpu`: la palabra `"auto"` o un número. Lo que cuesta cada opción está
//! contado en `programator.ejemplo.toml`, que es donde lo lee el usuario.

use serde::Deserialize;

/// Minutos sin encargos tras los que `"auto"` suelta el modelo: tiempo de sobra para una tanda de
/// encargos seguidos, sin tener la tarjeta ocupada toda la tarde si Programator se queda parado.
pub const MINUTOS_SIN_ENCARGOS_PARA_SOLTAR: u32 = 15;

/// Cuándo se suelta el modelo cargado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoltarModelo {
    /// Se queda cargado y se suelta tras [`MINUTOS_SIN_ENCARGOS_PARA_SOLTAR`] sin encargos.
    #[default]
    Auto,
    /// Se suelta tras estos minutos sin encargos. Con `0`, al terminar cada encargo.
    TrasMinutos(u32),
}

impl SoltarModelo {
    /// Los minutos sin encargos tras los que se suelta.
    pub fn minutos(self) -> u32 {
        match self {
            SoltarModelo::Auto => MINUTOS_SIN_ENCARGOS_PARA_SOLTAR,
            SoltarModelo::TrasMinutos(minutos) => minutos,
        }
    }
}

impl<'de> Deserialize<'de> for SoltarModelo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(super::auto_o_numero::leer(d, "soltar_modelo", "minutos")?
            .map_or(SoltarModelo::Auto, SoltarModelo::TrasMinutos))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[derive(Deserialize)]
    struct Envoltorio {
        soltar_modelo: SoltarModelo,
    }

    fn leer(valor: &str) -> Result<SoltarModelo, toml::de::Error> {
        toml::from_str::<Envoltorio>(&format!("soltar_modelo = {valor}")).map(|e| e.soltar_modelo)
    }

    #[test]
    fn auto_se_lee_sin_distinguir_mayusculas() {
        assert_eq!(leer("\"auto\"").unwrap(), SoltarModelo::Auto);
        assert_eq!(leer("\"AUTO\"").unwrap(), SoltarModelo::Auto);
    }

    #[test]
    fn un_numero_son_minutos_y_cero_es_soltar_siempre() {
        assert_eq!(leer("30").unwrap(), SoltarModelo::TrasMinutos(30));
        assert_eq!(leer("0").unwrap(), SoltarModelo::TrasMinutos(0));
    }

    #[test]
    fn auto_son_quince_minutos() {
        assert_eq!(SoltarModelo::Auto.minutos(), 15);
        assert_eq!(SoltarModelo::TrasMinutos(30).minutos(), 30);
    }

    #[test]
    fn otra_palabra_se_rechaza_nombrandola() {
        let fallo = leer("\"nunca\"").unwrap_err().to_string();
        assert!(
            fallo.contains("«nunca» no vale para soltar_modelo"),
            "{fallo}"
        );
    }

    #[test]
    fn un_decimal_o_un_negativo_se_rechazan_en_espanol() {
        for valor in ["1.5", "-3"] {
            let fallo = leer(valor).unwrap_err().to_string();
            assert!(
                fallo.contains("soltar_modelo solo admite"),
                "{valor}: {fallo}"
            );
        }
    }
}
