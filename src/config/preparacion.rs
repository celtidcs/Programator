//! `[preparacion]`: cómo prepara Programator una propuesta en una copia del proyecto, sin tocar el
//! proyecto (encargo `preparar:`).
//!
//! Las órdenes van por extensión, como los comprobadores de `[verificacion]`, y en tres clases:
//! `previas`, sin aislar y solo para bajar dependencias; `formatear`, aislada, cuyo fallo corta lo
//! que sigue; y `aisladas`, que formatean, compilan o prueban sin red. Ninguna pasa por un shell.

use serde::Deserialize;
use std::collections::BTreeMap;

/// Dentro del espacio de Programator en el proyecto: lo que no se versiona y se puede borrar.
pub const CARPETA_POR_DEFECTO: &str = ".gestor/programator/preparacion";
/// Medio gigabyte: un proyecto de código con su historia cabe de sobra, y una carpeta de datos o
/// de compilaciones que se coló en git no llena el disco a escondidas.
pub const TOPE_MIB_POR_DEFECTO: u64 = 512;
/// Dos días: lo que tarda de sobra quien encargó en mirar la copia, sin que se acumulen.
pub const DIAS_DE_VIDA_POR_DEFECTO: u64 = 2;
/// Diez minutos por orden: una compilación de Rust desde cero de un proyecto mediano.
pub const TIEMPO_MAXIMO_POR_DEFECTO_SEGUNDOS: u64 = 600;

/// La sección entera.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Preparacion {
    /// Dónde se hacen las copias, relativa a la carpeta de trabajo.
    #[serde(default = "carpeta_por_defecto")]
    pub carpeta: String,
    /// Lo más que puede ocupar lo que se copia, en MiB. Por encima no se copia nada.
    #[serde(default = "tope_mib_por_defecto")]
    pub tope_mib: u64,
    /// Tras cuántos días se borra una copia de la que nadie dijo nada.
    #[serde(default = "dias_de_vida_por_defecto")]
    pub dias_de_vida: u64,
    /// Lo más que puede durar cada orden.
    #[serde(default = "tiempo_maximo_por_defecto")]
    pub tiempo_maximo_segundos: u64,
    /// Extensión (sin punto) → sus órdenes.
    #[serde(default)]
    pub ordenes: BTreeMap<String, OrdenesDeLaExtension>,
    /// En Linux, lo que se antepone a las órdenes aisladas en lugar del envoltorio de fábrica de
    /// las comprobaciones del banco. Una lista vacía no vale: se avisa y se usa el de fábrica.
    #[serde(default)]
    pub envoltorio: Option<Vec<String>>,
}

/// Las órdenes con que se prepara un fichero de una extensión. `{fichero}` se sustituye por la ruta
/// de la propuesta dentro de la copia.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrdenesDeLaExtension {
    /// Sin aislar, antes que nada, y solo para bajar dependencias: nunca código del proyecto.
    #[serde(default)]
    pub previas: Vec<Vec<String>>,
    /// Aislada; si falla, no se sigue, porque lo demás probaría un fichero sin formatear.
    #[serde(default)]
    pub formatear: Vec<String>,
    /// Aisladas y sin red; una que falla se cuenta y las demás siguen.
    #[serde(default)]
    pub aisladas: Vec<Vec<String>>,
}

fn carpeta_por_defecto() -> String {
    CARPETA_POR_DEFECTO.to_string()
}
fn tope_mib_por_defecto() -> u64 {
    TOPE_MIB_POR_DEFECTO
}
fn dias_de_vida_por_defecto() -> u64 {
    DIAS_DE_VIDA_POR_DEFECTO
}
fn tiempo_maximo_por_defecto() -> u64 {
    TIEMPO_MAXIMO_POR_DEFECTO_SEGUNDOS
}

impl Default for Preparacion {
    fn default() -> Self {
        Self {
            carpeta: carpeta_por_defecto(),
            tope_mib: tope_mib_por_defecto(),
            dias_de_vida: dias_de_vida_por_defecto(),
            tiempo_maximo_segundos: tiempo_maximo_por_defecto(),
            ordenes: BTreeMap::new(),
            envoltorio: None,
        }
    }
}

impl Preparacion {
    /// Las órdenes para una extensión, sin distinguir mayúsculas.
    pub fn ordenes_de(&self, extension: &str) -> Option<&OrdenesDeLaExtension> {
        self.ordenes
            .iter()
            .find(|(clave, _)| clave.eq_ignore_ascii_case(extension))
            .map(|(_, ordenes)| ordenes)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_seccion_vale_lo_de_fabrica_y_no_hay_ordenes() {
        let preparacion: Preparacion = toml::from_str("").unwrap();
        assert_eq!(preparacion, Preparacion::default());
        assert_eq!(preparacion.carpeta, ".gestor/programator/preparacion");
        assert_eq!(preparacion.tope_mib, 512);
        assert_eq!(preparacion.dias_de_vida, 2);
        assert!(preparacion.ordenes_de("rs").is_none());
    }

    #[test]
    fn las_ordenes_se_leen_por_extension_y_clase() {
        let preparacion: Preparacion = toml::from_str(
            r#"
[ordenes.rs]
previas = [["cargo", "fetch"]]
formatear = ["cargo", "fmt"]
aisladas = [["cargo", "build", "--offline"], ["cargo", "test", "--offline"]]
"#,
        )
        .unwrap();

        let rs = preparacion.ordenes_de("RS").unwrap();
        assert_eq!(rs.previas, [["cargo", "fetch"]]);
        assert_eq!(rs.formatear, ["cargo", "fmt"]);
        assert_eq!(rs.aisladas.len(), 2);
    }

    /// Una clase mal escrita (`aislada`) no se puede quedar en nada sin decirlo.
    #[test]
    fn una_clase_desconocida_se_rechaza() {
        let error =
            toml::from_str::<Preparacion>("[ordenes.rs]\naislada = [[\"cargo\", \"test\"]]\n")
                .unwrap_err();
        assert!(error.to_string().contains("aislada"), "{error}");
    }
}
