//! La clave con que Programator habla con su `llama-server`.
//!
//! El motor la recibe en `LLAMA_API_KEY` y exige que cada petición la traiga en
//! `Authorization: Bearer <clave>`, salvo `/health`, que es pública
//! (https://raw.githubusercontent.com/ggml-org/llama.cpp/b10993/tools/server/server-http.cpp,
//! `middleware_validate_api_key`). Así ninguna página web abierta en el navegador puede usarlo.

mod guardada;

pub use guardada::{para_arrancar, vigente, FICHERO_DE_LA_CLAVE};

use crate::config::ClaveConfigurada;
use crate::error::{Error, Resultado};

/// Bytes al azar de una clave inventada: 256 bits, que nadie adivina probando.
const BYTES_DE_UNA_CLAVE: usize = 32;

/// La variable de entorno de la que `llama-server` lee la clave (`common/arg.cpp` de b10993,
/// opción `--api-key`).
pub const VARIABLE_DE_LA_CLAVE: &str = "LLAMA_API_KEY";

/// La clave del motor. No se imprime nunca: su `Debug` la oculta y no tiene `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct ClaveDelMotor(String);

impl std::fmt::Debug for ClaveDelMotor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClaveDelMotor([oculta])")
    }
}

impl ClaveDelMotor {
    /// La clave de un arranque: la fija del TOML o, con `"auto"`, una nueva sacada del generador
    /// del sistema.
    pub fn nueva(configurada: &ClaveConfigurada) -> Resultado<Self> {
        match configurada {
            ClaveConfigurada::Fija(texto) => Ok(Self(texto.clone())),
            ClaveConfigurada::Auto => {
                let mut bytes = [0u8; BYTES_DE_UNA_CLAVE];
                getrandom::getrandom(&mut bytes).map_err(|fallo| {
                    Error::ConfiguracionDelMotor(super::mensajes::sin_azar_para_la_clave(&fallo))
                })?;
                Ok(Self::de_bytes(&bytes))
            }
        }
    }

    /// Una clave ya conocida, por ejemplo la que se guardó al arrancar un motor que sigue vivo.
    pub fn de_texto(texto: &str) -> Self {
        Self(texto.to_string())
    }

    /// La clave escrita en hexadecimal, a partir de sus bytes.
    fn de_bytes(bytes: &[u8]) -> Self {
        Self(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    /// El texto de la clave, para la variable de entorno y para el fichero donde se guarda.
    pub fn texto(&self) -> &str {
        &self.0
    }

    /// El valor de la cabecera `Authorization`.
    pub fn cabecera(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El nombre que lee `llama-server` b10993 para `--api-key`, escrito a mano: si la constante
    /// cambiara, el motor arrancaría sin clave. Fuente: la fila de `--api-key` en
    /// https://github.com/ggml-org/llama.cpp/blob/b10993/tools/server/README.md
    #[test]
    fn la_variable_de_la_clave_es_la_que_lee_llama_server() {
        assert_eq!(VARIABLE_DE_LA_CLAVE, "LLAMA_API_KEY");
    }

    #[test]
    fn dos_claves_auto_son_distintas() {
        let una = ClaveDelMotor::nueva(&ClaveConfigurada::Auto).unwrap();
        let otra = ClaveDelMotor::nueva(&ClaveConfigurada::Auto).unwrap();
        assert_ne!(una, otra);
    }

    #[test]
    fn la_clave_tiene_64_caracteres_hexadecimales() {
        let clave = ClaveDelMotor::nueva(&ClaveConfigurada::Auto).unwrap();
        assert_eq!(clave.texto().len(), BYTES_DE_UNA_CLAVE * 2);
        assert!(clave.texto().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn los_bytes_se_escriben_en_hexadecimal_con_dos_cifras() {
        assert_eq!(ClaveDelMotor::de_bytes(&[0, 15, 255]).texto(), "000fff");
    }

    #[test]
    fn el_debug_no_muestra_la_clave() {
        let clave = ClaveDelMotor::de_texto("secreta");
        assert!(!format!("{clave:?}").contains("secreta"));
    }

    #[test]
    fn la_fija_se_usa_tal_cual_y_va_en_la_cabecera() {
        let clave = ClaveDelMotor::nueva(&ClaveConfigurada::Fija("mia".into())).unwrap();
        assert_eq!(clave.texto(), "mia");
        assert_eq!(clave.cabecera(), "Bearer mia");
    }
}
