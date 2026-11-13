//! La configuración del banco: un `banco.toml` propio, separado de `programator.toml`.
//!
//! El motor, el modelo y el agente siguen saliendo de `programator.toml`; aquí solo está lo que
//! añade el banco. Cada clave tiene su valor por defecto y está documentada, clave a clave, en
//! `evaluacion/banco/banco.ejemplo.toml`.
//!
//! Las pruebas ocultas —ejecutar en este equipo el código que escribe el modelo para ver si
//! funciona— van encendidas por defecto, también sin sección `[comprobacion_local]`, y se apagan
//! con `activa = false`. Apagadas es un estado válido, no un
//! error, y el banco lo dice al arrancar.
//!
//! Programator solo usa la GPU del equipo donde corre, nunca otra máquina: la sección `[comprobacion]`, que llevaba las pruebas a una máquina aparte, ya no
//! existe, y un `banco.toml` que todavía la traiga se rechaza diciendo qué poner en su lugar.

use std::path::Path;

use serde::Deserialize;

use super::ficha::Herramienta;
use super::mensajes;
use crate::config::{CapasGpu, Config};
use crate::error::{Error, Resultado};
pub use local::{
    Aislamiento, ComprobacionLocal, CLAVE_ACTIVA, MAQUINA_LOCAL, MARCADOR_CACHE, MARCADOR_PROXY,
    MARCADOR_VACIO,
};

mod local;

/// Las semillas de la evaluación del 22/09/2026, más dos para medir la dispersión.
const SEMILLAS_POR_DEFECTO: [i64; 3] = [42, 43, 44];
/// La temperatura de la evaluación del 22/09/2026, la misma que usa `[muestreo]` por defecto.
const TEMPERATURA_POR_DEFECTO: f32 = 0.2;
/// A cuántas líneas de un defecto sembrado puede caer un hallazgo para contar como acierto: el
/// modelo cuenta mal las líneas (visto en uso real) y el arnés solo corrige las que puede.
const MARGEN_LINEAS_POR_DEFECTO: usize = 2;
/// Un encargo tardó entre dos y tres minutos con Devstral; cinco veces el peor caso.
const TIEMPO_MAXIMO_ENCARGO_POR_DEFECTO: u64 = 900;
/// El buzón desde el que el banco escribe los encargos, como si fuera el agente que dirige.
const BUZON_QUE_ENCARGA_POR_DEFECTO: &str = "banco";

/// El contenido de `banco.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigBanco {
    #[serde(default = "semillas_por_defecto")]
    pub semillas: Vec<i64>,
    #[serde(default = "temperatura_por_defecto")]
    pub temperatura: f32,
    #[serde(default = "margen_lineas_por_defecto")]
    pub margen_lineas: usize,
    #[serde(default = "tiempo_maximo_encargo_por_defecto")]
    pub tiempo_maximo_encargo_segundos: u64,
    #[serde(default = "buzon_que_encarga_por_defecto")]
    pub buzon_que_encarga: String,
    /// Las capas del modelo en la GPU durante la tanda, con la misma sintaxis que en
    /// `programator.toml`. **Sustituye** a la de allí solo para el banco. El reparto de capas forma
    /// parte de lo que hay que repetir: con 30 capas la misma semilla dio otra respuesta que con 35
    /// (12/11/2026). Ausente: se usa la de `programator.toml`, y la ficha anota cuántas fueron.
    pub capas_gpu: Option<CapasGpu>,
    /// Las pruebas ocultas, en una carpeta de este equipo. Ausente: todos los valores por
    /// defecto, y encendidas; se apagan con `activa = false`.
    #[serde(default)]
    pub comprobacion_local: ComprobacionLocal,
    /// La sección `[comprobacion]` de la máquina aparte, que ya no existe. Se lee solo para
    /// rechazarla con un error que diga qué hacer, en vez del «clave desconocida» de serde.
    #[serde(default, rename = "comprobacion")]
    comprobacion_retirada: Option<toml::Value>,
}

/// Las herramientas que el banco sabe comprobar sin que nadie configure nada, con la orden que
/// mira si están instaladas: termina con 0 si lo están. Son las que piden los encargos del banco
/// que trae Programator (`evaluacion/banco/encargos/README.md`), y sus órdenes dan por hechos los
/// nombres exactos del `PATH` que usan esas fichas.
///
/// No es una lista cerrada: `[comprobacion_local.herramientas]`
/// de `banco.toml` cambia cualquiera de estas órdenes y añade las que el usuario quiera.
pub const HERRAMIENTAS_POR_DEFECTO: [(&str, &str); 6] = [
    ("rust", "cargo --version"),
    ("python", "python3 --version"),
    ("node", "node --version && npm --version"),
    ("dotnet", "dotnet --list-sdks"),
    ("lua", "lua5.4 -v"),
    ("sqlite", "sqlite3 -version"),
];

/// La orden de serie que comprueba `herramienta`, si el banco la trae de serie.
pub fn orden_de_herramienta_por_defecto(herramienta: &Herramienta) -> Option<&'static str> {
    HERRAMIENTAS_POR_DEFECTO
        .iter()
        .find(|(nombre, _)| *nombre == herramienta.nombre())
        .map(|(_, orden)| *orden)
}

/// Las herramientas de serie con su orden. Un nombre de la tabla que no fuera válido no se puede
/// escribir sin que lo cace la prueba `las_herramientas_de_serie_tienen_nombres_validos`; aun
/// así, aquí se descarta en vez de entrar en pánico.
pub fn herramientas_por_defecto() -> Vec<(Herramienta, String)> {
    HERRAMIENTAS_POR_DEFECTO
        .iter()
        .filter_map(|(nombre, orden)| {
            Herramienta::nueva(nombre)
                .ok()
                .map(|h| (h, (*orden).to_string()))
        })
        .collect()
}

/// Si el banco ejecuta código del modelo, y con qué reglas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoDeComprobacion<'a> {
    /// Las pruebas ocultas están apagadas (`activa = false`): todas las comprobaciones salen
    /// `NoEvaluado`.
    NoEjecutaCodigo,
    Local(&'a ComprobacionLocal),
}

impl ConfigBanco {
    /// Lee y valida `banco.toml`.
    pub fn cargar(ruta: &Path) -> Resultado<Self> {
        let texto = std::fs::read_to_string(ruta).map_err(|causa| Error::Lectura {
            ruta: ruta.to_path_buf(),
            causa,
        })?;
        Self::desde_texto(&texto)
    }

    /// Interpreta y valida el texto de un `banco.toml`.
    pub fn desde_texto(texto: &str) -> Resultado<Self> {
        let config: Self = toml::from_str(texto)
            .map_err(|causa| Error::Banco(format!("banco.toml no se puede leer: {causa}")))?;
        config.validar()?;
        Ok(config)
    }

    fn validar(&self) -> Resultado<()> {
        if self.semillas.is_empty() {
            return Err(Error::Banco(
                "banco.toml: «semillas» no puede estar vacía".into(),
            ));
        }
        if self.tiempo_maximo_encargo_segundos == 0 {
            return Err(Error::Banco(
                "banco.toml: «tiempo_maximo_encargo_segundos» tiene que ser mayor que 0".into(),
            ));
        }
        if self.comprobacion_retirada.is_some() {
            return Err(Error::Banco(
                mensajes::mensaje_comprobacion_en_otra_maquina(),
            ));
        }
        // Apagadas no se usa nada de la sección: un ajuste que en este equipo no valdría, como
        // un aislamiento de otro sistema, no tiene por qué impedir pasar el banco.
        let local = &self.comprobacion_local;
        if let Some(problema) = local.problema().filter(|_| local.activa) {
            return Err(Error::Banco(format!(
                "banco.toml: [comprobacion_local]: {problema}"
            )));
        }
        Ok(())
    }

    /// La configuración de Programator con lo que el banco sustituye para la tanda. La del
    /// Director no se toca: se devuelve una copia.
    pub fn para_la_tanda(&self, config: &Config) -> Config {
        let mut tanda = config.clone();
        if let Some(capas) = self.capas_gpu {
            tanda.motor.capas_gpu = capas;
        }
        tanda
    }

    pub fn modo_de_comprobacion(&self) -> ModoDeComprobacion<'_> {
        if self.comprobacion_local.activa {
            ModoDeComprobacion::Local(&self.comprobacion_local)
        } else {
            ModoDeComprobacion::NoEjecutaCodigo
        }
    }
}

impl Default for ConfigBanco {
    fn default() -> Self {
        Self {
            semillas: semillas_por_defecto(),
            temperatura: temperatura_por_defecto(),
            margen_lineas: margen_lineas_por_defecto(),
            tiempo_maximo_encargo_segundos: tiempo_maximo_encargo_por_defecto(),
            buzon_que_encarga: buzon_que_encarga_por_defecto(),
            capas_gpu: None,
            comprobacion_local: ComprobacionLocal::default(),
            comprobacion_retirada: None,
        }
    }
}

fn semillas_por_defecto() -> Vec<i64> {
    SEMILLAS_POR_DEFECTO.to_vec()
}
fn temperatura_por_defecto() -> f32 {
    TEMPERATURA_POR_DEFECTO
}
fn margen_lineas_por_defecto() -> usize {
    MARGEN_LINEAS_POR_DEFECTO
}
fn tiempo_maximo_encargo_por_defecto() -> u64 {
    TIEMPO_MAXIMO_ENCARGO_POR_DEFECTO
}
fn buzon_que_encarga_por_defecto() -> String {
    BUZON_QUE_ENCARGA_POR_DEFECTO.to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_herramientas_de_serie_tienen_nombres_validos() {
        // `herramientas_por_defecto` descarta en silencio un nombre que no valga: esta prueba es
        // la que impide que eso llegue a pasar.
        assert_eq!(
            herramientas_por_defecto().len(),
            HERRAMIENTAS_POR_DEFECTO.len()
        );
    }

    #[test]
    fn la_orden_de_serie_solo_existe_para_las_herramientas_de_serie() {
        let rust = Herramienta::nueva("rust").unwrap();
        let php = Herramienta::nueva("php").unwrap();

        assert_eq!(
            orden_de_herramienta_por_defecto(&rust),
            Some("cargo --version")
        );
        assert_eq!(orden_de_herramienta_por_defecto(&php), None);
    }

    #[test]
    fn un_fichero_vacio_da_todos_los_valores_por_defecto() {
        let config = ConfigBanco::desde_texto("").unwrap();

        assert_eq!(config, ConfigBanco::default());
        assert_eq!(config.semillas, vec![42, 43, 44]);
        assert!((config.temperatura - 0.2).abs() < f32::EPSILON);
        assert_eq!(config.margen_lineas, 2);
        assert_eq!(config.tiempo_maximo_encargo_segundos, 900);
        assert_eq!(config.buzon_que_encarga, "banco");
    }

    fn config_de_programator(capas: &str) -> Config {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("programator.toml");
        std::fs::write(
            &ruta,
            format!("[motor]\nmodelo = \"modelos/x.gguf\"\ncapas_gpu = {capas}\n"),
        )
        .unwrap();
        Config::cargar(&ruta).unwrap()
    }

    #[test]
    fn sin_capas_en_el_banco_se_usan_las_de_programator() {
        let banco = ConfigBanco::desde_texto("").unwrap();

        let tanda = banco.para_la_tanda(&config_de_programator("35"));

        assert_eq!(tanda.motor.capas_gpu, CapasGpu::Fijas(35));
    }

    #[test]
    fn las_capas_del_banco_prevalecen_y_la_configuracion_original_no_cambia() {
        let banco = ConfigBanco::desde_texto("capas_gpu = 30").unwrap();
        let original = config_de_programator("\"auto\"");

        let tanda = banco.para_la_tanda(&original);

        assert_eq!(tanda.motor.capas_gpu, CapasGpu::Fijas(30));
        assert_eq!(original.motor.capas_gpu, CapasGpu::Auto);
    }

    #[test]
    fn unas_capas_invalidas_en_el_banco_dan_el_mismo_error_que_en_programator() {
        let fallo = ConfigBanco::desde_texto("capas_gpu = \"maximas\"")
            .unwrap_err()
            .to_string();

        assert!(fallo.contains("capas_gpu"), "{fallo}");
        assert!(fallo.contains("auto"), "{fallo}");
    }

    #[test]
    fn cada_clave_se_puede_cambiar() {
        let config = ConfigBanco::desde_texto(
            "semillas = [7]\ntemperatura = 0.7\nmargen_lineas = 0\n\
             tiempo_maximo_encargo_segundos = 60\n",
        )
        .unwrap();

        assert_eq!(config.semillas, vec![7]);
        assert!((config.temperatura - 0.7).abs() < f32::EPSILON);
        assert_eq!(config.margen_lineas, 0);
        assert_eq!(config.tiempo_maximo_encargo_segundos, 60);
    }

    #[test]
    fn una_lista_de_semillas_vacia_es_un_error() {
        let error = ConfigBanco::desde_texto("semillas = []").unwrap_err();

        assert!(error
            .to_string()
            .contains("«semillas» no puede estar vacía"));
    }

    #[test]
    fn un_tiempo_maximo_cero_es_un_error() {
        assert!(ConfigBanco::desde_texto("tiempo_maximo_encargo_segundos = 0").is_err());
    }

    #[test]
    fn con_comprobacion_local_el_banco_comprueba_en_este_equipo() {
        let config = ConfigBanco::desde_texto(
            "semillas = [1]\n\
             [comprobacion_local]\n",
        )
        .unwrap();

        let ModoDeComprobacion::Local(local) = config.modo_de_comprobacion() else {
            panic!("debería comprobar en local");
        };
        assert_eq!(local, &ComprobacionLocal::default());
    }

    #[test]
    fn la_seccion_de_la_maquina_aparte_ya_no_vale_y_dice_que_poner() {
        // Programator nunca usa otra máquina.
        let texto = "[comprobacion]\n\
                     maquina = \"maquina-linux\"\n\
                     subir = \"vm subir {carpeta_local}\"\n";

        let fallo = ConfigBanco::desde_texto(texto).unwrap_err().to_string();

        assert!(fallo.contains("[comprobacion]"), "{fallo}");
        assert!(fallo.contains("[comprobacion_local]"), "{fallo}");
        assert!(!fallo.contains("unknown field"), "{fallo}");
    }

    #[test]
    fn una_comprobacion_local_sin_interprete_es_un_error() {
        let texto = "semillas = [1]\n\
                     [comprobacion_local]\n\
                     interprete = []\n";

        assert!(ConfigBanco::desde_texto(texto).is_err());
    }

    #[test]
    fn una_clave_desconocida_es_un_error() {
        assert!(ConfigBanco::desde_texto("semilla = [1]").is_err());
    }

    #[test]
    fn sin_seccion_las_pruebas_ocultas_van_encendidas_con_los_valores_por_defecto() {
        // Las pruebas ocultas son opcionales y van encendidas por defecto. Antes, sin sección no se ejecutaba código del modelo.
        let config = ConfigBanco::desde_texto("semillas = [1]").unwrap();

        let ModoDeComprobacion::Local(local) = config.modo_de_comprobacion() else {
            panic!("sin sección, las pruebas ocultas van encendidas");
        };
        assert_eq!(local, &ComprobacionLocal::default());
        assert!(local.activa);
    }

    #[test]
    fn con_activa_false_el_banco_no_ejecuta_codigo_y_no_es_un_error() {
        let config =
            ConfigBanco::desde_texto("semillas = [1]\n[comprobacion_local]\nactiva = false\n")
                .unwrap();

        assert_eq!(
            config.modo_de_comprobacion(),
            ModoDeComprobacion::NoEjecutaCodigo
        );
    }

    #[test]
    fn apagadas_un_ajuste_que_aqui_no_valdria_no_impide_pasar_el_banco() {
        let apagadas = "[comprobacion_local]\nactiva = false\ninterprete = []\n";
        let encendidas = "[comprobacion_local]\ninterprete = []\n";

        assert!(ConfigBanco::desde_texto(apagadas).is_ok());
        assert!(ConfigBanco::desde_texto(encendidas).is_err());
    }

    #[test]
    fn una_herramienta_nueva_se_acepta_y_un_nombre_no_valido_es_un_error() {
        // Hasta el 13/11/2026 cualquier herramienta fuera de las seis de serie era un error. Desde
        // que los lenguajes son abiertos, una nueva se declara aquí con su orden; lo
        // que sigue siendo un error es un nombre que no se puede escribir en una ficha.
        let nueva = "[comprobacion_local.herramientas]\ncobol = \"cobc --version\"\n";
        let config = ConfigBanco::desde_texto(nueva).unwrap();
        let local = config.comprobacion_local;
        assert_eq!(
            local.orden_de_herramienta(&Herramienta::nueva("cobol").unwrap()),
            Some("cobc --version")
        );

        let mal_escrita = "[comprobacion_local.herramientas]\nCOBOL = \"x\"\n";
        let fallo = ConfigBanco::desde_texto(mal_escrita).unwrap_err();
        assert!(fallo.to_string().contains("«COBOL»"), "{fallo}");
    }

    #[test]
    fn el_fichero_de_ejemplo_se_lee() {
        let ruta =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("evaluacion/banco/banco.ejemplo.toml");
        if !ruta.is_file() {
            eprintln!("sin {}: un árbol sin el banco", ruta.display());
            return;
        }

        assert!(ConfigBanco::cargar(&ruta).is_ok());

        // La sección de comprobación va comentada; descomentada, también tiene que valer.
        let texto = std::fs::read_to_string(&ruta).unwrap();
        let texto_local = descomentar(&texto, "# [comprobacion_local]\n");
        // Cada aislamiento existe en un solo sistema: uno escrito haría que el ejemplo valiera solo
        // en ese. En Linux falló así la aceptación del 12/11/2026, con «integridad_baja».
        assert!(
            !texto_local
                .lines()
                .any(|linea| linea.trim_start().starts_with("aislamiento")),
            "el ejemplo descomentado tiene que valer en Windows y en Linux"
        );
        let local = ConfigBanco::desde_texto(&texto_local).unwrap();
        let ModoDeComprobacion::Local(local) = local.modo_de_comprobacion() else {
            panic!("debería comprobar en local");
        };
        assert_eq!(local.nombres["python3"], "python");
        // El ejemplo enseña el interruptor y cómo se añade un lenguaje que no viene de serie.
        assert!(local.activa);
        assert_eq!(
            local.orden_de_herramienta(&Herramienta::nueva("php").unwrap()),
            Some("php --version")
        );
        // El ejemplo enseña el envoltorio de por defecto: si se separan, quien lo descomente
        // pierde lo que el de por defecto hace, como encender `lo`.
        assert_eq!(
            local.envoltorio,
            super::ComprobacionLocal::default().envoltorio
        );
    }

    /// Las líneas comentadas desde `inicio` hasta la raya que separa las secciones, sin el «# ».
    fn descomentar(texto: &str, inicio: &str) -> String {
        let texto = texto.replace("\r\n", "\n");
        let desde = texto.find(inicio).unwrap();
        texto[desde..]
            .lines()
            .take_while(|linea| !linea.starts_with("# ---"))
            .filter_map(|linea| linea.strip_prefix("# "))
            .map(|linea| format!("{linea}\n"))
            .collect()
    }
}
