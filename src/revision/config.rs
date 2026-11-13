//! La sección `[revision]` de `programator.toml`: qué se busca en Hugging Face y cómo se descarga.
//!
//! Todo tiene su valor por defecto en una función, y está comentado en `programator.ejemplo.toml`.

use serde::Deserialize;

/// Las búsquedas de partida: lo que un modelo de programación lleva en el nombre de su repositorio
/// de GGUF. El Director las ajusta en su TOML (especificación, §3.1).
const BUSQUEDAS_POR_DEFECTO: [&str; 3] = ["coder GGUF", "devstral GGUF", "code instruct GGUF"];
/// La biblioteca de Hugging Face de los ficheros que sabe cargar `llama-server`.
const BIBLIOTECA_POR_DEFECTO: &str = "gguf";
/// Cuántos resultados se piden por búsqueda, los de más descargas.
const RESULTADOS_POR_BUSQUEDA_POR_DEFECTO: u32 = 50;
/// Dos meses: si una revisión se salta un mes, la siguiente no pierde nada.
const MODIFICADOS_EN_LOS_ULTIMOS_DIAS_POR_DEFECTO: u32 = 62;
/// Las cuantizaciones que interesan, en orden de preferencia. `Q4_K_M` es la del modelo en
/// producción; las demás, más fieles y más grandes, por si caben.
const CUANTIZACIONES_POR_DEFECTO: [&str; 4] = ["Q4_K_M", "Q5_K_M", "Q6_K", "Q8_0"];
/// Lo que se reserva de la tarjeta para el contexto y el motor al decidir si un fichero cabe, en
/// MiB. Es una cota: el encaje exacto se hace al pasar el banco (especificación, §3.3).
const RESERVA_MIB_POR_DEFECTO: u64 = 2048;
/// Dónde se guardan las revisiones, relativo a la carpeta del ejecutable.
const CARPETA_POR_DEFECTO: &str = "revisiones";
/// Cuánto se espera a Hugging Face, en segundos: lo mismo que la consulta de actualizaciones.
const TIEMPO_ESPERA_SEGUNDOS_POR_DEFECTO: u64 = 30;
/// La dirección de la API de Hugging Face.
const API_POR_DEFECTO: &str = "https://huggingface.co/api";
/// La web de Hugging Face: el enlace de cada candidato y la descarga de sus ficheros
/// (`<sitio>/<repositorio>/resolve/main/<fichero>`).
const SITIO_POR_DEFECTO: &str = "https://huggingface.co";
/// Dónde están los modelos, relativo a la carpeta del ejecutable: donde la búsqueda mira qué hay
/// ya descargado y donde la descarga deja cada fichero. Es la carpeta del portable.
const CARPETA_DE_MODELOS_POR_DEFECTO: &str = "modelos";
/// Lo que se deja libre en el disco además del fichero que se descarga, en MiB, para que la
/// descarga no llene el disco del todo.
const MARGEN_DISCO_MIB_POR_DEFECTO: u64 = 1024;
/// Cada cuántos MiB descargados se dice el avance. Un modelo de 14 GiB da así unas treinta líneas.
const AVISAR_CADA_MIB_POR_DEFECTO: u64 = 512;
/// Cuántos candidatos se prueban como mucho en una revisión. Cada tanda del banco ocupa la GPU unas
/// dos o tres horas (13/11/2026): tres caben en una tarde.
const CANDIDATOS_POR_REVISION_POR_DEFECTO: usize = 3;
/// La carpeta del banco, relativa a la del ejecutable: la que copia el portable.
const CARPETA_DEL_BANCO_POR_DEFECTO: &str = "banco";

/// La sección `[revision]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigRevision {
    /// Cada búsqueda es el parámetro `search` de la API.
    #[serde(default = "busquedas_por_defecto")]
    pub busquedas: Vec<String>,
    #[serde(default = "biblioteca_por_defecto")]
    pub biblioteca: String,
    #[serde(default = "resultados_por_busqueda_por_defecto")]
    pub resultados_por_busqueda: u32,
    /// 0 quita el filtro.
    #[serde(default = "modificados_en_los_ultimos_dias_por_defecto")]
    pub modificados_en_los_ultimos_dias: u32,
    /// En orden de preferencia; de cada repositorio se toma un fichero por cuantización.
    #[serde(default = "cuantizaciones_por_defecto")]
    pub cuantizaciones: Vec<String>,
    #[serde(default = "reserva_mib_por_defecto")]
    pub reserva_mib: u64,
    #[serde(default = "carpeta_por_defecto")]
    pub carpeta: String,
    #[serde(default = "tiempo_espera_segundos_por_defecto")]
    pub tiempo_espera_segundos: u64,
    #[serde(default = "api_por_defecto")]
    pub api: String,
    #[serde(default = "sitio_por_defecto")]
    pub sitio: String,
    #[serde(default = "carpeta_de_modelos_por_defecto")]
    pub carpeta_de_modelos: String,
    #[serde(default = "margen_disco_mib_por_defecto")]
    pub margen_disco_mib: u64,
    #[serde(default = "avisar_cada_mib_por_defecto")]
    pub avisar_cada_mib: u64,
    /// Los buzones (sin «.md») que pueden pedir una revisión por el canal. Vacía: la revisión por
    /// encargo está deshabilitada.
    #[serde(default)]
    pub buzones_que_ordenan: Vec<String>,
    #[serde(default = "candidatos_por_revision_por_defecto")]
    pub candidatos_por_revision: usize,
    #[serde(default = "carpeta_del_banco_por_defecto")]
    pub carpeta_del_banco: String,
}

impl Default for ConfigRevision {
    fn default() -> Self {
        Self {
            busquedas: busquedas_por_defecto(),
            biblioteca: biblioteca_por_defecto(),
            resultados_por_busqueda: resultados_por_busqueda_por_defecto(),
            modificados_en_los_ultimos_dias: modificados_en_los_ultimos_dias_por_defecto(),
            cuantizaciones: cuantizaciones_por_defecto(),
            reserva_mib: reserva_mib_por_defecto(),
            carpeta: carpeta_por_defecto(),
            tiempo_espera_segundos: tiempo_espera_segundos_por_defecto(),
            api: api_por_defecto(),
            sitio: sitio_por_defecto(),
            carpeta_de_modelos: carpeta_de_modelos_por_defecto(),
            margen_disco_mib: margen_disco_mib_por_defecto(),
            avisar_cada_mib: avisar_cada_mib_por_defecto(),
            buzones_que_ordenan: Vec::new(),
            candidatos_por_revision: candidatos_por_revision_por_defecto(),
            carpeta_del_banco: carpeta_del_banco_por_defecto(),
        }
    }
}

impl ConfigRevision {
    /// Lo que impide hacer una revisión con esta sección, dicho para quien edita el TOML, o
    /// `None` si se puede.
    pub fn problema(&self) -> Option<String> {
        if self.busquedas.is_empty() {
            return Some("«[revision] busquedas» está vacía: no habría nada que buscar".into());
        }
        if self.cuantizaciones.is_empty() {
            return Some(
                "«[revision] cuantizaciones» está vacía: no se tomaría ningún fichero".into(),
            );
        }
        if self.resultados_por_busqueda == 0 {
            return Some("«[revision] resultados_por_busqueda» es 0: no llegaría ninguno".into());
        }
        if self.tiempo_espera_segundos == 0 {
            return Some(
                "«[revision] tiempo_espera_segundos» es 0: ninguna consulta tendría tiempo".into(),
            );
        }
        if self.avisar_cada_mib == 0 {
            return Some("«[revision] avisar_cada_mib» es 0: tiene que ser al menos 1".into());
        }
        if self.candidatos_por_revision == 0 {
            return Some(
                "«[revision] candidatos_por_revision» es 0: no se probaría ninguno".into(),
            );
        }
        None
    }
}

fn texto(lista: &[&str]) -> Vec<String> {
    lista.iter().map(|s| s.to_string()).collect()
}
fn busquedas_por_defecto() -> Vec<String> {
    texto(&BUSQUEDAS_POR_DEFECTO)
}
fn biblioteca_por_defecto() -> String {
    BIBLIOTECA_POR_DEFECTO.to_string()
}
fn resultados_por_busqueda_por_defecto() -> u32 {
    RESULTADOS_POR_BUSQUEDA_POR_DEFECTO
}
fn modificados_en_los_ultimos_dias_por_defecto() -> u32 {
    MODIFICADOS_EN_LOS_ULTIMOS_DIAS_POR_DEFECTO
}
fn cuantizaciones_por_defecto() -> Vec<String> {
    texto(&CUANTIZACIONES_POR_DEFECTO)
}
fn reserva_mib_por_defecto() -> u64 {
    RESERVA_MIB_POR_DEFECTO
}
fn carpeta_por_defecto() -> String {
    CARPETA_POR_DEFECTO.to_string()
}
fn tiempo_espera_segundos_por_defecto() -> u64 {
    TIEMPO_ESPERA_SEGUNDOS_POR_DEFECTO
}
fn api_por_defecto() -> String {
    API_POR_DEFECTO.to_string()
}
fn sitio_por_defecto() -> String {
    SITIO_POR_DEFECTO.to_string()
}
fn carpeta_de_modelos_por_defecto() -> String {
    CARPETA_DE_MODELOS_POR_DEFECTO.to_string()
}
fn margen_disco_mib_por_defecto() -> u64 {
    MARGEN_DISCO_MIB_POR_DEFECTO
}
fn avisar_cada_mib_por_defecto() -> u64 {
    AVISAR_CADA_MIB_POR_DEFECTO
}
fn candidatos_por_revision_por_defecto() -> usize {
    CANDIDATOS_POR_REVISION_POR_DEFECTO
}
fn carpeta_del_banco_por_defecto() -> String {
    CARPETA_DEL_BANCO_POR_DEFECTO.to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn leer(texto: &str) -> ConfigRevision {
        toml::from_str(texto).unwrap()
    }

    #[test]
    fn sin_nada_escrito_valen_los_valores_por_defecto() {
        let config = leer("");
        assert_eq!(config, ConfigRevision::default());
        assert_eq!(config.cuantizaciones[0], "Q4_K_M");
        assert_eq!(config.modificados_en_los_ultimos_dias, 62);
        assert_eq!(config.margen_disco_mib, 1024);
        assert_eq!(config.avisar_cada_mib, 512);
        assert!(config.buzones_que_ordenan.is_empty());
        assert_eq!(config.candidatos_por_revision, 3);
        assert_eq!(config.carpeta_del_banco, "banco");
        assert_eq!(config.problema(), None);
    }

    #[test]
    fn lo_escrito_manda_sobre_el_valor_por_defecto() {
        let config = leer("busquedas = [\"qwen GGUF\"]\nreserva_mib = 1024\n");
        assert_eq!(config.busquedas, ["qwen GGUF"]);
        assert_eq!(config.reserva_mib, 1024);
        assert_eq!(config.api, "https://huggingface.co/api");
    }

    #[test]
    fn cada_valor_imposible_se_rechaza_diciendo_cual() {
        let casos = [
            ("busquedas = []", "busquedas"),
            ("cuantizaciones = []", "cuantizaciones"),
            ("resultados_por_busqueda = 0", "resultados_por_busqueda"),
            ("tiempo_espera_segundos = 0", "tiempo_espera_segundos"),
            ("avisar_cada_mib = 0", "avisar_cada_mib"),
            ("candidatos_por_revision = 0", "candidatos_por_revision"),
        ];
        for (escrito, clave) in casos {
            let problema = leer(escrito).problema();
            assert!(
                problema.as_deref().is_some_and(|p| p.contains(clave)),
                "{escrito}: {problema:?}"
            );
        }
    }

    #[test]
    fn quien_ordena_se_lee_como_se_escribe() {
        let config = leer("buzones_que_ordenan = [\"claude\"]\n");
        assert_eq!(config.buzones_que_ordenan, ["claude"]);
    }

    #[test]
    fn cero_dias_quita_el_filtro_y_no_es_un_problema() {
        assert_eq!(leer("modificados_en_los_ultimos_dias = 0").problema(), None);
    }

    #[test]
    fn una_clave_desconocida_es_un_error() {
        assert!(toml::from_str::<ConfigRevision>("busqueda = [\"x\"]").is_err());
    }
}
