//! Las claves del TOML que admiten la palabra `"auto"` o un número entero, como `capas_gpu` y
//! `soltar_modelo`. Un solo lector y unos mismos mensajes para todas.

use serde::Deserialize;

/// La palabra que deja el valor en manos de Programator.
const PALABRA_AUTO: &str = "auto";

/// Lo que se escribió: `None` si es «auto», sin distinguir mayúsculas, o el número.
///
/// `clave` y `unidad` (por ejemplo `capas_gpu` y `capas`) solo sirven para el mensaje de error, que
/// lee el usuario cuando edita el TOML a mano.
pub(crate) fn leer<'de, D: serde::Deserializer<'de>>(
    d: D,
    clave: &str,
    unidad: &str,
) -> Result<Option<u32>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Crudo {
        Numero(u32),
        Palabra(String),
    }

    // Sin este `map_err`, un decimal o un negativo agotaría las variantes de Crudo y llegaría al
    // usuario con el nombre en inglés de este enum auxiliar.
    let crudo = Crudo::deserialize(d)
        .map_err(|_| serde::de::Error::custom(mensaje_de_tipo_invalido(clave, unidad)))?;

    match crudo {
        Crudo::Numero(numero) => Ok(Some(numero)),
        Crudo::Palabra(p) if p.eq_ignore_ascii_case(PALABRA_AUTO) => Ok(None),
        Crudo::Palabra(p) => Err(serde::de::Error::custom(mensaje_de_palabra_invalida(
            &p, clave, unidad,
        ))),
    }
}

/// El mensaje para un valor que no es ni número ni cadena.
fn mensaje_de_tipo_invalido(clave: &str, unidad: &str) -> String {
    format!("{clave} solo admite un número entero de {unidad} o la palabra «{PALABRA_AUTO}»")
}

/// El mensaje para una palabra que no es «auto».
fn mensaje_de_palabra_invalida(palabra: &str, clave: &str, unidad: &str) -> String {
    format!(
        "«{palabra}» no vale para {clave}: escribe un número de {unidad} o la palabra \
         «{PALABRA_AUTO}»"
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[derive(Debug)]
    struct Valor(Option<u32>);

    impl<'de> Deserialize<'de> for Valor {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            leer(d, "clave", "cosas").map(Valor)
        }
    }

    #[derive(Deserialize)]
    struct Envoltorio {
        clave: Valor,
    }

    fn de(valor: &str) -> Result<Option<u32>, String> {
        toml::from_str::<Envoltorio>(&format!("clave = {valor}"))
            .map(|e| e.clave.0)
            .map_err(|fallo| fallo.to_string())
    }

    #[test]
    fn auto_en_cualquier_caja_es_none_y_un_numero_es_ese_numero() {
        assert_eq!(de("\"auto\""), Ok(None));
        assert_eq!(de("\"AUTO\""), Ok(None));
        assert_eq!(de("7"), Ok(Some(7)));
    }

    #[test]
    fn los_errores_nombran_la_clave_y_la_unidad_en_castellano() {
        let palabra = de("\"todas\"").unwrap_err();
        assert!(
            palabra.contains("«todas» no vale para clave: escribe un número de cosas"),
            "{palabra}"
        );

        let decimal = de("1.5").unwrap_err();
        assert!(
            decimal.contains("clave solo admite un número entero de cosas"),
            "{decimal}"
        );
        assert!(!decimal.contains("Crudo"), "{decimal}");
    }
}
