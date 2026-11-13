//! El catálogo de modelos de Hugging Face: buscar repositorios y pedir los ficheros de cada uno.
//!
//! Solo hace HTTP y deserializa; no decide nada. Lo que decide qué fichero sirve está en
//! `candidatos`. Cada respuesta vuelve también en bruto, para guardarla junto a la revisión y poder
//! comprobar después de dónde salió cada fila (especificación, §3.2, punto 6).

use std::time::Duration;

use serde::Deserialize;

use crate::error::{Error, Resultado};

/// El prefijo de las etiquetas de Hugging Face que dicen la licencia: `license:apache-2.0`.
const PREFIJO_DE_LICENCIA: &str = "license:";
/// La API identifica a quien pregunta por el `User-Agent`, como la de GitHub en `actualizacion`.
pub(super) const AGENTE: &str = "Programator";

/// Una búsqueda: el texto, la biblioteca y cuántos resultados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Busqueda {
    pub texto: String,
    pub biblioteca: String,
    pub limite: u32,
}

/// Lo que devolvió la API, interpretado, y el texto tal cual llegó.
#[derive(Debug, Clone, PartialEq)]
pub struct Respuesta<T> {
    pub datos: T,
    pub en_bruto: String,
}

/// Un repositorio de los resultados de una búsqueda.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Repositorio {
    pub id: String,
    /// La fecha de la última modificación, en ISO 8601, como la da la API.
    #[serde(rename = "lastModified", default)]
    pub ultima_modificacion: Option<String>,
    #[serde(rename = "downloads", default)]
    pub descargas: Option<u64>,
}

/// Un repositorio con sus ficheros y su licencia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetalleDeRepositorio {
    pub id: String,
    pub ultima_modificacion: Option<String>,
    pub descargas: Option<u64>,
    pub licencia: Option<String>,
    pub ficheros: Vec<FicheroDelRepositorio>,
}

/// Un fichero de un repositorio. Los pequeños, como el README, no van por LFS y no traen huella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FicheroDelRepositorio {
    pub nombre: String,
    pub tamano: Option<u64>,
    pub huella_sha256: Option<String>,
}

/// De dónde salen los repositorios. Hugging Face en producción; uno de mentira en las pruebas.
pub trait CatalogoDeModelos {
    fn buscar(&self, busqueda: &Busqueda) -> Resultado<Respuesta<Vec<Repositorio>>>;
    fn detalle(&self, repositorio: &str) -> Resultado<Respuesta<DetalleDeRepositorio>>;
}

/// El catálogo de verdad.
pub struct HuggingFace {
    pub api: String,
    pub tiempo_espera: Duration,
}

impl HuggingFace {
    fn pedir(&self, url: &str) -> Resultado<String> {
        let agente = ureq::AgentBuilder::new()
            .timeout(self.tiempo_espera)
            .build();
        agente
            .get(url)
            .set("User-Agent", AGENTE)
            .call()
            .map_err(|causa| Error::Revision(format!("Hugging Face no contestó a {url}: {causa}")))?
            .into_string()
            .map_err(|causa| {
                Error::Revision(format!("la respuesta de {url} no se pudo leer: {causa}"))
            })
    }
}

impl CatalogoDeModelos for HuggingFace {
    fn buscar(&self, busqueda: &Busqueda) -> Resultado<Respuesta<Vec<Repositorio>>> {
        let en_bruto = self.pedir(&url_de_busqueda(&self.api, busqueda))?;
        Ok(Respuesta {
            datos: leer_busqueda(&en_bruto)?,
            en_bruto,
        })
    }

    fn detalle(&self, repositorio: &str) -> Resultado<Respuesta<DetalleDeRepositorio>> {
        let en_bruto = self.pedir(&url_de_detalle(&self.api, repositorio))?;
        Ok(Respuesta {
            datos: leer_detalle(&en_bruto)?,
            en_bruto,
        })
    }
}

/// La consulta de una búsqueda, ordenada por descargas. Sin `expand`, la API no da la fecha de
/// la última modificación, que hace falta para el plazo; con él, da solo lo que se pide.
pub fn url_de_busqueda(api: &str, busqueda: &Busqueda) -> String {
    format!(
        "{api}/models?search={}&library={}&sort=downloads&limit={}\
         &expand%5B%5D=lastModified&expand%5B%5D=downloads",
        codificar(&busqueda.texto),
        codificar(&busqueda.biblioteca),
        busqueda.limite
    )
}

/// La consulta del detalle de un repositorio, con el tamaño y la huella de cada fichero
/// (`blobs=true`). El `/` entre autor y nombre va tal cual: es parte de la ruta.
pub fn url_de_detalle(api: &str, repositorio: &str) -> String {
    let ruta = repositorio
        .split('/')
        .map(codificar)
        .collect::<Vec<_>>()
        .join("/");
    format!("{api}/models/{ruta}?blobs=true")
}

/// Un texto para ir en una URL: todo lo que no sea letra, cifra o `-_.~` va como `%XX`.
fn codificar(texto: &str) -> String {
    texto
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            otro => format!("%{otro:02X}"),
        })
        .collect()
}

/// Los repositorios de una respuesta de búsqueda.
pub fn leer_busqueda(texto: &str) -> Resultado<Vec<Repositorio>> {
    serde_json::from_str(texto)
        .map_err(|causa| Error::Revision(format!("la búsqueda no se entiende: {causa}")))
}

/// El detalle de un repositorio. La licencia sale de la ficha del modelo (`cardData.license`) y,
/// si no la trae, de sus etiquetas `license:`.
pub fn leer_detalle(texto: &str) -> Resultado<DetalleDeRepositorio> {
    let leido: DetalleEnBruto = serde_json::from_str(texto)
        .map_err(|causa| Error::Revision(format!("el detalle no se entiende: {causa}")))?;
    let de_las_etiquetas = || {
        leido
            .tags
            .iter()
            .find_map(|etiqueta| etiqueta.strip_prefix(PREFIJO_DE_LICENCIA))
            .map(str::to_string)
    };
    let licencia = leido
        .card_data
        .and_then(|ficha| ficha.license)
        .and_then(LicenciaEnBruto::texto)
        .or_else(de_las_etiquetas);
    Ok(DetalleDeRepositorio {
        id: leido.id,
        ultima_modificacion: leido.last_modified,
        descargas: leido.downloads,
        licencia,
        ficheros: leido
            .siblings
            .into_iter()
            .map(|fichero| FicheroDelRepositorio {
                nombre: fichero.rfilename,
                tamano: fichero.lfs.as_ref().map(|lfs| lfs.size).or(fichero.size),
                huella_sha256: fichero.lfs.map(|lfs| lfs.sha256),
            })
            .collect(),
    })
}

/// La respuesta del detalle tal como la da la API, con sus nombres.
#[derive(Deserialize)]
struct DetalleEnBruto {
    id: String,
    #[serde(rename = "lastModified", default)]
    last_modified: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(rename = "cardData", default)]
    card_data: Option<FichaEnBruto>,
    #[serde(default)]
    siblings: Vec<FicheroEnBruto>,
}

#[derive(Deserialize)]
struct FichaEnBruto {
    #[serde(default)]
    license: Option<LicenciaEnBruto>,
}

/// La ficha del modelo la escribe su autor, y la licencia llega como texto o como lista
/// (`["apache-2.0"]`, en `eaddario/Devstral-Small-2505-pruned-GGUF` el 12/11/2026).
#[derive(Deserialize)]
#[serde(untagged)]
enum LicenciaEnBruto {
    Una(String),
    Varias(Vec<String>),
}

impl LicenciaEnBruto {
    /// Varias se juntan con comas; una lista vacía es que no hay licencia.
    fn texto(self) -> Option<String> {
        match self {
            Self::Una(licencia) => Some(licencia),
            Self::Varias(licencias) if licencias.is_empty() => None,
            Self::Varias(licencias) => Some(licencias.join(", ")),
        }
    }
}

#[derive(Deserialize)]
struct FicheroEnBruto {
    rfilename: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    lfs: Option<LfsEnBruto>,
}

#[derive(Deserialize)]
struct LfsEnBruto {
    sha256: String,
    size: u64,
}

#[cfg(test)]
mod pruebas {
    //! Contra respuestas reales de la API, pedidas el 12/11/2026 y recortadas a unos pocos
    //! ficheros (`src/revision/pruebas/`).
    use super::*;

    const BUSQUEDA: &str = include_str!("pruebas/busqueda.json");
    const CON_LICENCIA: &str = include_str!("pruebas/detalle-con-licencia.json");
    const SIN_LICENCIA: &str = include_str!("pruebas/detalle-sin-licencia.json");
    const PARTIDO: &str = include_str!("pruebas/detalle-partido.json");

    #[test]
    fn la_busqueda_da_cada_repositorio_con_su_fecha_y_sus_descargas() {
        let repositorios = leer_busqueda(BUSQUEDA).unwrap();
        assert_eq!(repositorios.len(), 3);
        assert_eq!(
            repositorios[2],
            Repositorio {
                id: "bartowski/mistralai_Devstral-Small-2-24B-Instruct-2512-GGUF".into(),
                ultima_modificacion: Some("2025-12-09T21:25:08.000Z".into()),
                descargas: Some(31173),
            }
        );
    }

    #[test]
    fn el_detalle_da_la_licencia_y_cada_fichero_con_tamano_y_huella() {
        let detalle = leer_detalle(CON_LICENCIA).unwrap();
        assert_eq!(detalle.licencia.as_deref(), Some("apache-2.0"));
        assert_eq!(detalle.descargas, Some(31173));
        let q4 = detalle
            .ficheros
            .iter()
            .find(|f| f.nombre.ends_with("Q4_K_M.gguf"))
            .unwrap();
        assert!(q4.tamano.is_some_and(|t| t > 1 << 30), "{q4:?}");
        assert_eq!(q4.huella_sha256.as_ref().map(String::len), Some(64));
    }

    #[test]
    fn un_fichero_sin_lfs_no_tiene_huella_pero_si_tamano() {
        let detalle = leer_detalle(CON_LICENCIA).unwrap();
        let atributos = detalle
            .ficheros
            .iter()
            .find(|f| f.nombre == ".gitattributes")
            .unwrap();
        assert_eq!(atributos.huella_sha256, None);
        assert!(atributos.tamano.is_some());
    }

    #[test]
    fn un_repositorio_sin_licencia_lo_dice_con_none() {
        let detalle = leer_detalle(SIN_LICENCIA).unwrap();
        assert_eq!(detalle.licencia, None);
        assert!(!detalle.ficheros.is_empty());
    }

    #[test]
    fn los_ficheros_partidos_llegan_como_ficheros_sueltos() {
        let detalle = leer_detalle(PARTIDO).unwrap();
        let partes = detalle
            .ficheros
            .iter()
            .filter(|f| f.nombre.contains("-of-"))
            .count();
        assert!(partes >= 2, "{:?}", detalle.ficheros);
    }

    #[test]
    fn la_licencia_sale_de_las_etiquetas_si_la_ficha_no_la_trae() {
        let texto = r#"{"id":"a/b","tags":["gguf","license:mit"],"siblings":[]}"#;
        assert_eq!(
            leer_detalle(texto).unwrap().licencia.as_deref(),
            Some("mit")
        );
    }

    #[test]
    fn la_licencia_de_la_ficha_puede_venir_como_lista() {
        let una = r#"{"id":"a/b","cardData":{"license":["apache-2.0"]},"siblings":[]}"#;
        assert_eq!(
            leer_detalle(una).unwrap().licencia.as_deref(),
            Some("apache-2.0")
        );
        let dos = r#"{"id":"a/b","cardData":{"license":["mit","other"]},"siblings":[]}"#;
        assert_eq!(
            leer_detalle(dos).unwrap().licencia.as_deref(),
            Some("mit, other")
        );
        let vacia = r#"{"id":"a/b","cardData":{"license":[]},"tags":["license:mit"]}"#;
        assert_eq!(
            leer_detalle(vacia).unwrap().licencia.as_deref(),
            Some("mit")
        );
    }

    #[test]
    fn una_respuesta_que_no_se_entiende_es_un_error_y_no_un_panico() {
        assert!(leer_busqueda("<html>").is_err());
        assert!(leer_detalle("{}").is_err());
    }

    #[test]
    fn la_url_de_la_busqueda_lleva_el_texto_codificado_y_pide_las_fechas() {
        let busqueda = Busqueda {
            texto: "devstral GGUF".into(),
            biblioteca: "gguf".into(),
            limite: 50,
        };
        assert_eq!(
            url_de_busqueda("https://huggingface.co/api", &busqueda),
            "https://huggingface.co/api/models?search=devstral%20GGUF&library=gguf\
             &sort=downloads&limit=50&expand%5B%5D=lastModified&expand%5B%5D=downloads"
        );
    }

    #[test]
    fn la_url_del_detalle_conserva_la_barra_del_repositorio() {
        assert_eq!(
            url_de_detalle("https://huggingface.co/api", "Qwen/Qwen2.5 Coder"),
            "https://huggingface.co/api/models/Qwen/Qwen2.5%20Coder?blobs=true"
        );
    }
}
