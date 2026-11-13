//! `candidatos.toml`: lo que encontró una búsqueda, para que el Director lo lea y para que la
//! descarga compruebe contra él el nombre y la huella (especificación, §3.2, punto 6, y §4).

use serde::{Deserialize, Serialize};

use super::candidatos::FicheroCandidato;
use crate::error::{Error, Resultado};

/// El fichero de la lista dentro de la carpeta del mes.
pub const NOMBRE_DE_LA_LISTA: &str = "candidatos.toml";

/// La lista de una búsqueda, con la tarjeta contra la que se midió.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListaDeCandidatos {
    /// El día de la búsqueda, `AAAA-MM-DD`.
    pub fecha: String,
    pub tarjeta: String,
    pub vram_mib: u64,
    /// Lo que podía pesar un fichero para caber, en MiB.
    pub cabe_mib: u64,
    #[serde(default, rename = "candidato")]
    pub candidatos: Vec<EntradaDeLaLista>,
    #[serde(default, rename = "ya_descargado")]
    pub ya_descargados: Vec<EntradaDeLaLista>,
}

/// Un fichero de la lista, con el enlace a su repositorio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntradaDeLaLista {
    #[serde(flatten)]
    pub fichero: FicheroCandidato,
    pub enlace: String,
}

impl EntradaDeLaLista {
    pub fn nueva(fichero: FicheroCandidato, sitio: &str) -> Self {
        let enlace = enlace_del_repositorio(sitio, &fichero.repositorio);
        Self { fichero, enlace }
    }
}

/// La página del repositorio en la web de Hugging Face.
pub fn enlace_del_repositorio(sitio: &str, repositorio: &str) -> String {
    format!("{}/{repositorio}", sitio.trim_end_matches('/'))
}

pub fn escribir(lista: &ListaDeCandidatos) -> Resultado<String> {
    toml::to_string(lista)
        .map_err(|causa| Error::Revision(format!("la lista de candidatos no se compone: {causa}")))
}

pub fn leer(texto: &str) -> Resultado<ListaDeCandidatos> {
    toml::from_str(texto)
        .map_err(|causa| Error::Revision(format!("«{NOMBRE_DE_LA_LISTA}» no se entiende: {causa}")))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn fichero(tamano: Option<u64>, licencia: Option<&str>) -> FicheroCandidato {
        FicheroCandidato {
            repositorio: "bartowski/Modelo-GGUF".into(),
            fichero: "Modelo-Q4_K_M.gguf".into(),
            cuantizacion: "Q4_K_M".into(),
            tamano,
            huella_sha256: Some("ab".repeat(32)),
            licencia: licencia.map(str::to_string),
            ultima_modificacion: Some("2026-11-01T10:00:00.000Z".into()),
            descargas: Some(31_173),
        }
    }

    #[test]
    fn la_lista_va_y_vuelve_igual_con_y_sin_los_datos_opcionales() {
        let lista = ListaDeCandidatos {
            fecha: "2026-11-12".into(),
            tarjeta: "NVIDIA GeForce RTX 4080".into(),
            vram_mib: 16_376,
            cabe_mib: 12_280,
            candidatos: vec![EntradaDeLaLista::nueva(
                fichero(Some(14_000_000_000), Some("apache-2.0")),
                "https://huggingface.co",
            )],
            ya_descargados: vec![EntradaDeLaLista::nueva(
                fichero(None, None),
                "https://huggingface.co",
            )],
        };
        let texto = escribir(&lista).unwrap();
        assert!(texto.contains("[[candidato]]"), "{texto}");
        assert!(!texto.contains("licencia = \"\""), "{texto}");
        assert_eq!(leer(&texto).unwrap(), lista);
    }

    #[test]
    fn el_enlace_es_la_pagina_del_repositorio() {
        assert_eq!(
            enlace_del_repositorio("https://huggingface.co/", "a/b"),
            "https://huggingface.co/a/b"
        );
    }

    #[test]
    fn una_lista_ilegible_es_un_error_que_la_nombra() {
        let fallo = leer("fecha = 3").unwrap_err().to_string();
        assert!(fallo.contains(NOMBRE_DE_LA_LISTA), "{fallo}");
    }
}
