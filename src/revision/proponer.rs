//! `--revision-proponer`: lee lo que hay en la carpeta de modelos, decide con `propuesta` y deja
//! `propuesta.md` y `propuesta.toml` en la carpeta del mes (especificación, §6).

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

use super::mensajes;
use super::mes;
use super::propuesta::{proponer, Entrada, Propuesta};
use super::redaccion::{self, NOMBRE_DE_LAS_RETIRADAS, NOMBRE_DE_LA_PROPUESTA};
use crate::error::{Error, Resultado};
use crate::ficheros;
use crate::registro::modelo::Registro;

const FORMATO_DEL_DIA: &str = "%Y-%m-%d";

/// Lo que la orden necesita del mundo.
pub struct Entorno<'a> {
    pub registro: &'a Registro,
    /// De dónde se leyó el registro, para decirlo en la propuesta.
    pub ruta_del_registro: PathBuf,
    /// El nombre de la tarjeta de esta máquina; sin él no se sabe qué cifras valen.
    pub tarjeta: Option<String>,
    pub carpeta_de_modelos: PathBuf,
    pub carpeta_de_revisiones: PathBuf,
    /// El nombre, sin extensión, del modelo de `programator.toml`.
    pub modelo_en_uso: Option<String>,
    pub ahora: NaiveDateTime,
}

/// Lo que dejó la propuesta.
#[derive(Debug, Clone, PartialEq)]
pub struct PropuestaHecha {
    pub ruta: PathBuf,
    pub se_quedan: usize,
    pub para_retirar: usize,
    pub sin_medir: usize,
    /// El nombre con que se apartó la propuesta anterior del mes, si la había.
    pub anterior: Option<String>,
    /// La propuesta entera, para quien tenga que aplicarla (`revisión: cerrar`).
    pub propuesta: Propuesta,
}

pub fn proponer_y_escribir(entorno: &Entorno) -> Resultado<PropuestaHecha> {
    let tarjeta = entorno
        .tarjeta
        .as_deref()
        .ok_or_else(|| Error::Revision(mensajes::SIN_TARJETA_PARA_PROPONER.to_string()))?;
    let en_modelos = ficheros_con_tamano(&entorno.carpeta_de_modelos)?;
    let propuesta = proponer(&Entrada {
        registro: entorno.registro,
        tarjeta,
        en_modelos: &en_modelos,
        modelo_en_uso: entorno.modelo_en_uso.as_deref(),
    });
    let fecha = entorno.ahora.format(FORMATO_DEL_DIA).to_string();
    let texto = redaccion::componer(&propuesta, &entorno.ruta_del_registro, &fecha);
    let retiradas = redaccion::escribir_retiradas(&redaccion::retiradas(&propuesta))?;
    let carpeta = mes::carpeta_del_mes(&entorno.carpeta_de_revisiones, entorno.ahora);
    ficheros::crear_carpeta(&carpeta)?;
    let anterior = mes::apartar_lo_anterior(
        &carpeta,
        NOMBRE_DE_LA_PROPUESTA,
        &[NOMBRE_DE_LAS_RETIRADAS],
        entorno.ahora,
    )?;
    ficheros::escribir(&carpeta.join(NOMBRE_DE_LAS_RETIRADAS), &retiradas)?;
    let ruta = carpeta.join(NOMBRE_DE_LA_PROPUESTA);
    ficheros::escribir(&ruta, &texto)?;
    Ok(PropuestaHecha {
        ruta,
        se_quedan: propuesta.se_quedan.len(),
        para_retirar: propuesta.para_retirar.len(),
        sin_medir: propuesta.sin_medir.len(),
        anterior,
        propuesta,
    })
}

/// Los ficheros de la carpeta de modelos con su tamaño. Sin carpeta, ninguno.
fn ficheros_con_tamano(carpeta: &Path) -> Resultado<Vec<(String, u64)>> {
    if !carpeta.is_dir() {
        return Ok(Vec::new());
    }
    let mut ficheros: Vec<(String, u64)> = ficheros::entradas(carpeta)?
        .into_iter()
        .filter(|ruta| ruta.is_file())
        .filter_map(|ruta| {
            let tamano = std::fs::metadata(&ruta).ok()?.len();
            Some((ruta.file_name()?.to_string_lossy().into_owned(), tamano))
        })
        .collect();
    ficheros.sort();
    Ok(ficheros)
}

#[cfg(test)]
mod pruebas {
    use chrono::NaiveDate;

    use super::*;
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;

    fn a_las(hora: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 12)
            .unwrap()
            .and_hms_opt(hora, 0, 0)
            .unwrap()
    }

    fn entorno<'a>(registro: &'a Registro, raiz: &Path, hora: u32) -> Entorno<'a> {
        Entorno {
            registro,
            ruta_del_registro: raiz.join("registro-de-modelos.toml"),
            tarjeta: Some(entrada_de_ejemplo().tarjeta),
            carpeta_de_modelos: raiz.join("modelos"),
            carpeta_de_revisiones: raiz.join("revisiones"),
            modelo_en_uso: None,
            ahora: a_las(hora),
        }
    }

    #[test]
    fn deja_la_propuesta_y_las_retiradas_en_la_carpeta_del_mes() {
        let raiz = tempfile::tempdir().unwrap();
        let modelos = raiz.path().join("modelos");
        ficheros::crear_carpeta(&modelos).unwrap();
        ficheros::escribir(&modelos.join("nuevo.gguf"), "x").unwrap();
        let registro = Registro {
            modelo: vec![entrada_de_ejemplo()],
        };
        let hecha = proponer_y_escribir(&entorno(&registro, raiz.path(), 9)).unwrap();
        let mes = raiz.path().join("revisiones").join("2026-11");
        assert_eq!(hecha.ruta, mes.join(NOMBRE_DE_LA_PROPUESTA));
        assert_eq!(hecha.se_quedan, 1);
        assert_eq!(hecha.sin_medir, 1);
        let texto = ficheros::leer(&hecha.ruta).unwrap();
        assert!(
            texto.starts_with("# Propuesta de la revisión del 2026-11-12"),
            "{texto}"
        );
        assert!(texto.contains("nuevo.gguf"), "{texto}");
        let retiradas =
            redaccion::leer_retiradas(&ficheros::leer(&mes.join(NOMBRE_DE_LAS_RETIRADAS)).unwrap())
                .unwrap();
        assert!(retiradas.retiradas.is_empty());
    }

    #[test]
    fn una_segunda_propuesta_del_mes_aparta_la_anterior() {
        let raiz = tempfile::tempdir().unwrap();
        let registro = Registro::default();
        proponer_y_escribir(&entorno(&registro, raiz.path(), 9)).unwrap();
        let hecha = proponer_y_escribir(&entorno(&registro, raiz.path(), 15)).unwrap();
        assert_eq!(hecha.anterior.as_deref(), Some("propuesta-150000.md"));
        let mes = raiz.path().join("revisiones").join("2026-11");
        assert!(mes.join("propuesta-150000.toml").is_file());
        assert!(mes.join(NOMBRE_DE_LA_PROPUESTA).is_file());
    }

    #[test]
    fn sin_tarjeta_se_niega_y_no_escribe_nada() {
        let raiz = tempfile::tempdir().unwrap();
        let registro = Registro::default();
        let mut sin_tarjeta = entorno(&registro, raiz.path(), 9);
        sin_tarjeta.tarjeta = None;
        let fallo = proponer_y_escribir(&sin_tarjeta).unwrap_err().to_string();
        assert!(fallo.contains("tarjeta"), "{fallo}");
        assert!(!raiz.path().join("revisiones").exists());
    }
}
