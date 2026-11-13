//! `probados.toml`: lo que se ha probado en la revisión del mes, para que `revisión: cerrar` sepa
//! qué tandas son suyas y cuáles quedaron sin comprobar (especificación de la revisión autónoma,
//! §3 y §7).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Resultado};
use crate::ficheros;

/// El fichero dentro de la carpeta del mes.
pub const NOMBRE_DE_LO_PROBADO: &str = "probados.toml";

/// En qué quedó un candidato.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    /// Las pasadas están guardadas, pero sus pruebas ocultas fallaron: `cerrar` lo vuelve a
    /// intentar.
    SinComprobar,
    /// La tanda está entera: le faltan, como mucho, las rúbricas.
    Comprobada,
    /// No se llegó a tener una tanda: no se descargó, o el motor no lo cargó.
    Fallido,
}

/// Un candidato probado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probado {
    pub repositorio: String,
    pub fichero: String,
    pub estado: Estado,
    /// La carpeta de la tanda, si se llegó a pasar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tanda: Option<PathBuf>,
    /// Por qué falló, o qué falta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
}

/// Todo lo probado en el mes, en el orden en que se probó.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probados {
    #[serde(default, rename = "probado")]
    pub probados: Vec<Probado>,
}

impl Probados {
    /// Apunta un candidato. Si ya estaba, lo sustituye: lo que vale es lo último que pasó con él.
    pub fn anotar(&mut self, probado: Probado) {
        match self
            .probados
            .iter_mut()
            .find(|p| p.repositorio == probado.repositorio && p.fichero == probado.fichero)
        {
            Some(anterior) => *anterior = probado,
            None => self.probados.push(probado),
        }
    }
}

pub fn escribir(probados: &Probados) -> Resultado<String> {
    toml::to_string(probados)
        .map_err(|causa| Error::Revision(format!("lo probado no se compone: {causa}")))
}

pub fn leer(texto: &str) -> Resultado<Probados> {
    toml::from_str(texto).map_err(|causa| {
        Error::Revision(format!("«{NOMBRE_DE_LO_PROBADO}» no se entiende: {causa}"))
    })
}

/// Lo probado en la carpeta de un mes; vacío si todavía no hay nada.
pub fn cargar(carpeta_del_mes: &Path) -> Resultado<Probados> {
    let texto = ficheros::leer_si_existe(&carpeta_del_mes.join(NOMBRE_DE_LO_PROBADO))?;
    if texto.trim().is_empty() {
        return Ok(Probados::default());
    }
    leer(&texto)
}

pub fn guardar(carpeta_del_mes: &Path, probados: &Probados) -> Resultado<()> {
    ficheros::crear_carpeta(carpeta_del_mes)?;
    ficheros::escribir(
        &carpeta_del_mes.join(NOMBRE_DE_LO_PROBADO),
        &escribir(probados)?,
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn probado(fichero: &str, estado: Estado) -> Probado {
        Probado {
            repositorio: "autor/Modelo-GGUF".into(),
            fichero: fichero.into(),
            estado,
            tanda: Some(PathBuf::from("banco/resultados/t1")),
            motivo: None,
        }
    }

    #[test]
    fn lo_probado_va_y_vuelve_igual() {
        let mut probados = Probados::default();
        probados.anotar(probado("a.gguf", Estado::Comprobada));
        probados.anotar(Probado {
            tanda: None,
            motivo: Some("no se descargó".into()),
            ..probado("b.gguf", Estado::Fallido)
        });

        let texto = escribir(&probados).unwrap();
        assert!(texto.contains("estado = \"comprobada\""), "{texto}");
        assert_eq!(leer(&texto).unwrap(), probados);
    }

    #[test]
    fn anotar_otra_vez_el_mismo_candidato_lo_sustituye() {
        let mut probados = Probados::default();
        probados.anotar(probado("a.gguf", Estado::SinComprobar));
        probados.anotar(probado("a.gguf", Estado::Comprobada));

        assert_eq!(probados.probados.len(), 1);
        assert_eq!(probados.probados[0].estado, Estado::Comprobada);
    }

    #[test]
    fn sin_fichero_no_hay_nada_probado_y_se_guarda_donde_se_lee() {
        let dir = tempfile::tempdir().unwrap();
        let mes = dir.path().join("2026-11");
        assert_eq!(cargar(&mes).unwrap(), Probados::default());

        let mut probados = Probados::default();
        probados.anotar(probado("a.gguf", Estado::Comprobada));
        guardar(&mes, &probados).unwrap();
        assert_eq!(cargar(&mes).unwrap(), probados);
    }
}
