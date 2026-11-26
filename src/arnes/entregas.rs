//! El registro de entregas: lo que Programator propuso en este proyecto y en qué quedó cada
//! propuesta.
//!
//! Una entrada por propuesta, que el arnés anota al escribirla (`escribir_propuesta`), con el
//! encargo que la pidió, el modelo que la hizo, su veredicto y la huella de su contenido. El
//! estado empieza en `propuesta`; lo cambian después lo que deduzca Programator y lo que diga el
//! agente que encargó. Sirve para tres cosas: que el modelo sepa lo que ya entregó, ordenar la
//! carpeta de candidatos y medir qué parte de lo que entrega cada modelo se acepta. Aquí solo hay
//! datos y lo que se decide con ellos; el disco está en `disco`.

pub mod al_dia;
pub mod archivo;
pub mod busqueda;
pub mod catalogo;
pub mod confirmacion;
pub mod deduccion;
pub mod disco;

use crate::arnes::verificacion::Veredicto;
use serde::{Deserialize, Serialize};

/// Quién dio el estado cuando lo dedujo el propio Programator.
pub const LO_DIO_PROGRAMATOR: &str = "programator";

/// Lo que salió de comprobar la propuesta al escribirla, sin el detalle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VeredictoDeLaEntrega {
    SinComprobador,
    NoSePudo,
    Pasa,
    NoPasa,
}

impl From<&Veredicto> for VeredictoDeLaEntrega {
    fn from(veredicto: &Veredicto) -> Self {
        match veredicto {
            Veredicto::SinComprobador { .. } => Self::SinComprobador,
            Veredicto::NoSePudo { .. } => Self::NoSePudo,
            Veredicto::Pasa { .. } => Self::Pasa,
            Veredicto::NoPasa { .. } => Self::NoPasa,
        }
    }
}

/// En qué quedó una propuesta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoDeLaPropuesta {
    /// Escrita, sin que se sepa todavía qué se hizo con ella.
    Propuesta,
    /// Está en el proyecto, tal cual, o el agente dijo que la aplicó.
    Aceptada,
    /// El agente dijo que la descartó.
    Rechazada,
    /// Una propuesta posterior para el mismo nombre la sustituye.
    Sustituida,
    /// Pasó el plazo sin que nadie dijera nada ni se dedujera nada.
    SinConfirmar,
}

/// Una propuesta del registro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntradaDeEntrega {
    /// El nombre con que quedó en la carpeta de candidatos, relativo a ella.
    pub propuesta: String,
    /// El identificador del encargo que la pidió.
    pub encargo: String,
    /// Cómo se nombra ese encargo en la respuesta.
    pub nombre_del_encargo: String,
    /// El buzón de quien encargó.
    pub de: String,
    /// El modelo que la escribió, si se sabe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modelo: Option<String>,
    /// Cuándo se escribió.
    pub fecha: String,
    pub veredicto: VeredictoDeLaEntrega,
    /// sha256 del contenido, con los saltos de línea normalizados ([`huella`]).
    pub huella: String,
    pub estado: EstadoDeLaPropuesta,
    /// `programator` si lo dedujo Programator; si no, la identidad del agente que lo dijo.
    pub estado_lo_dio: String,
    /// El fichero del proyecto donde se encontró, si se dedujo que se aceptó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destino: Option<String>,
    /// Por qué se rechazó, si el agente lo dijo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
    /// Dónde quedó en el archivo, relativo a la carpeta de candidatos, una vez resuelta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archivada: Option<String>,
    /// Los ficheros del proyecto que ya tenían su nombre y su contenido cuando se escribió: que
    /// sigan ahí no dice que nadie la aceptara. Sin esto, una propuesta que devolvía sin cambios un
    /// fichero existente se deducía aceptada.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ya_en_el_proyecto: Vec<String>,
}

/// El registro entero, en orden de escritura.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistroDeEntregas {
    /// Cuándo se buscó por última vez en el proyecto lo aceptado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultima_busqueda: Option<String>,
    #[serde(default, rename = "entrega")]
    pub entregas: Vec<EntradaDeEntrega>,
}

/// Lo que se sabe del encargo en curso cuando el modelo escribe una propuesta: con qué se anota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncargoDeLaEntrega {
    pub encargo: String,
    pub nombre_del_encargo: String,
    pub de: String,
    pub modelo: Option<String>,
}

impl EncargoDeLaEntrega {
    /// La entrada de una propuesta recién escrita: en estado `propuesta`, dado por Programator.
    pub fn entrada(
        &self,
        propuesta: &str,
        contenido: &str,
        veredicto: &Veredicto,
        fecha: &str,
    ) -> EntradaDeEntrega {
        EntradaDeEntrega {
            propuesta: propuesta.to_string(),
            encargo: self.encargo.clone(),
            nombre_del_encargo: self.nombre_del_encargo.clone(),
            de: self.de.clone(),
            modelo: self.modelo.clone(),
            fecha: fecha.to_string(),
            veredicto: veredicto.into(),
            huella: huella(contenido),
            estado: EstadoDeLaPropuesta::Propuesta,
            estado_lo_dio: LO_DIO_PROGRAMATOR.to_string(),
            destino: None,
            motivo: None,
            archivada: None,
            ya_en_el_proyecto: Vec::new(),
        }
    }
}

/// La huella de un contenido: sha256 con los saltos de línea `\r\n` pasados a `\n`, para que una
/// copia que solo cambia los saltos de línea cuente como la misma.
pub fn huella(contenido: &str) -> String {
    crate::ficheros::huella_sha256_de_bytes(contenido.replace("\r\n", "\n").as_bytes())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn encargo() -> EncargoDeLaEntrega {
        EncargoDeLaEntrega {
            encargo: "0123456789abcdef".to_string(),
            nombre_del_encargo: "contar líneas".to_string(),
            de: "claude.md".to_string(),
            modelo: Some("devstral".to_string()),
        }
    }

    #[test]
    fn la_huella_no_cambia_con_los_saltos_de_linea() {
        assert_eq!(huella("a\r\nb\r\n"), huella("a\nb\n"));
        assert_ne!(huella("a\nb\n"), huella("a\nc\n"));
    }

    #[test]
    fn una_propuesta_recien_escrita_queda_como_propuesta_y_con_su_veredicto() {
        let veredicto = Veredicto::NoPasa {
            orden: "cargo check".to_string(),
            salida: "error".to_string(),
        };

        let entrada =
            encargo().entrada("x.rs", "fn main() {}\n", &veredicto, "2026-11-25 16:00:00");

        assert_eq!(entrada.propuesta, "x.rs");
        assert_eq!(entrada.encargo, "0123456789abcdef");
        assert_eq!(entrada.modelo.as_deref(), Some("devstral"));
        assert_eq!(entrada.veredicto, VeredictoDeLaEntrega::NoPasa);
        assert_eq!(entrada.estado, EstadoDeLaPropuesta::Propuesta);
        assert_eq!(entrada.estado_lo_dio, LO_DIO_PROGRAMATOR);
        assert_eq!(entrada.huella, huella("fn main() {}\n"));
    }

    #[test]
    fn cada_veredicto_se_resume() {
        let casos = [
            (
                Veredicto::SinComprobador {
                    extension: "lua".to_string(),
                },
                VeredictoDeLaEntrega::SinComprobador,
            ),
            (
                Veredicto::NoSePudo {
                    motivo: "m".to_string(),
                },
                VeredictoDeLaEntrega::NoSePudo,
            ),
            (
                Veredicto::Pasa {
                    orden: "o".to_string(),
                },
                VeredictoDeLaEntrega::Pasa,
            ),
        ];
        for (veredicto, esperado) in casos {
            assert_eq!(VeredictoDeLaEntrega::from(&veredicto), esperado);
        }
    }
}
