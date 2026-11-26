//! Cómo se reparte el modelo entre la tarjeta y la memoria del equipo: las claves de `[motor]`
//! que deciden qué va a la CPU. Los nombres de las opciones de `llama-server` se comprobaron el
//! 25/11/2026 en `common/arg.cpp` de b10993
//! (https://raw.githubusercontent.com/ggml-org/llama.cpp/b10993/common/arg.cpp).
//!
//! Todas valen por defecto lo que hacía la 1.1.0, que no pasaba ninguna de estas opciones: con los
//! valores por defecto la línea de arranque no cambia.

use serde::Deserialize;

/// Lo que se escribe en el TOML para dejar el número de capas en manos del encaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapasEnCpu {
    /// El encaje busca el menor número de capas que hace que el resto quepa.
    Auto,
    /// Las capas 0 a N-1 dejan esa parte en la CPU. Con 0, ninguna.
    Fijas(u32),
}

impl Default for CapasEnCpu {
    fn default() -> Self {
        CapasEnCpu::Fijas(0)
    }
}

/// Los modos de carga de `--load-mode` en b10993 (`common/arg.cpp:2687`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoDeCarga {
    /// `auto`: el del motor si no se dice nada. No se pasa la opción.
    #[default]
    Auto,
    /// `none`: sin modo especial.
    Ninguno,
    /// `mmap`: el modelo se proyecta en memoria.
    Mmap,
    /// `mlock`: el sistema no puede mandar el modelo al disco.
    Mlock,
    /// `mmap+mlock`: las dos cosas.
    MmapMlock,
    /// `dio`: lectura directa del disco, si se puede.
    Dio,
}

/// Cada modo con la palabra que entiende `llama-server`, que es también la que se escribe en el TOML.
const PALABRAS_DE_CARGA: [(ModoDeCarga, &str); 6] = [
    (ModoDeCarga::Auto, "auto"),
    (ModoDeCarga::Ninguno, "none"),
    (ModoDeCarga::Mmap, "mmap"),
    (ModoDeCarga::Mlock, "mlock"),
    (ModoDeCarga::MmapMlock, "mmap+mlock"),
    (ModoDeCarga::Dio, "dio"),
];

impl ModoDeCarga {
    /// La palabra de `--load-mode`.
    pub fn palabra(self) -> &'static str {
        PALABRAS_DE_CARGA
            .iter()
            .find(|(modo, _)| *modo == self)
            .map_or("auto", |(_, palabra)| palabra)
    }
}

impl<'de> Deserialize<'de> for ModoDeCarga {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let palabra = String::deserialize(d)?;
        PALABRAS_DE_CARGA
            .iter()
            .find(|(_, conocida)| palabra.eq_ignore_ascii_case(conocida))
            .map(|(modo, _)| *modo)
            .ok_or_else(|| serde::de::Error::custom(mensaje_de_modo_desconocido(&palabra)))
    }
}

/// El mensaje para un modo de carga que `llama-server` no conoce.
fn mensaje_de_modo_desconocido(palabra: &str) -> String {
    let conocidas: Vec<&str> = PALABRAS_DE_CARGA.iter().map(|(_, p)| *p).collect();
    format!(
        "«{palabra}» no vale para modo_de_carga: escribe una de estas palabras: {}",
        conocidas.join(", ")
    )
}

/// Los caracteres que `--override-tensor` usa para separar: `,` entre reglas y `=` entre el patrón
/// y el tipo de búfer (`parse_tensor_buffer_overrides`, `common/arg.cpp:252` de b10993). Un patrón
/// que los lleve se partiría por donde no debe.
const SEPARADORES_DE_OVERRIDE: [char; 2] = [',', '='];

/// El mensaje para un patrón que `--override-tensor` partiría.
fn mensaje_de_patron_invalido(patron: &str) -> String {
    format!(
        "el patrón «{patron}» de tensores_en_cpu no puede llevar «,» ni «=»: llama-server los usa \
         para separar las reglas"
    )
}

/// El mensaje para un patrón vacío, que mandaría a la CPU todos los tensores.
fn mensaje_de_patron_vacio() -> String {
    "tensores_en_cpu tiene un patrón vacío, que mandaría todo el modelo a la CPU: quítalo".into()
}

/// Lee `tensores_en_cpu` y rechaza los patrones que el motor entendería mal.
fn leer_patrones<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let patrones = Vec::<String>::deserialize(d)?;
    for patron in &patrones {
        if patron.is_empty() {
            return Err(serde::de::Error::custom(mensaje_de_patron_vacio()));
        }
        if patron.contains(SEPARADORES_DE_OVERRIDE) {
            return Err(serde::de::Error::custom(mensaje_de_patron_invalido(patron)));
        }
    }
    Ok(patrones)
}

fn leer_capas<'de, D: serde::Deserializer<'de>>(d: D, clave: &str) -> Result<CapasEnCpu, D::Error> {
    Ok(super::auto_o_numero::leer(d, clave, "capas")?.map_or(CapasEnCpu::Auto, CapasEnCpu::Fijas))
}

fn leer_expertos<'de, D: serde::Deserializer<'de>>(d: D) -> Result<CapasEnCpu, D::Error> {
    leer_capas(d, "expertos_en_cpu")
}

fn leer_ffn<'de, D: serde::Deserializer<'de>>(d: D) -> Result<CapasEnCpu, D::Error> {
    leer_capas(d, "ffn_en_cpu")
}

/// Las claves del reparto, dentro de `[motor]`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct RepartoDeMemoria {
    /// Tipo de la caché de claves (`--cache-type-k`). Sin él, el de `cache_kv`.
    #[serde(default)]
    pub cache_k: Option<String>,
    /// Tipo de la caché de valores (`--cache-type-v`). Sin él, el de `cache_kv`.
    #[serde(default)]
    pub cache_v: Option<String>,
    /// La caché en la memoria del equipo y no en la tarjeta (`--no-kv-offload`).
    #[serde(default)]
    pub cache_en_cpu: bool,
    /// Cómo se carga el modelo (`--load-mode`).
    #[serde(default)]
    pub modo_de_carga: ModoDeCarga,
    /// Capas cuyos expertos se quedan en la CPU (`--n-cpu-moe`). Solo sirve a los modelos de
    /// mezcla de expertos.
    #[serde(default, deserialize_with = "leer_expertos")]
    pub expertos_en_cpu: CapasEnCpu,
    /// Capas cuya FFN se queda en la CPU (`--n-cpu-ffn`). Para los modelos densos, como Devstral.
    #[serde(default, deserialize_with = "leer_ffn")]
    pub ffn_en_cpu: CapasEnCpu,
    /// Patrones de tensores que van a la CPU (`--override-tensor <patrón>=CPU`).
    #[serde(default, deserialize_with = "leer_patrones")]
    pub tensores_en_cpu: Vec<String>,
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn leer(toml: &str) -> Result<RepartoDeMemoria, String> {
        toml::from_str::<RepartoDeMemoria>(toml).map_err(|fallo| fallo.to_string())
    }

    #[test]
    fn sin_nada_escrito_todo_vale_lo_de_la_version_anterior() {
        let reparto = leer("").unwrap();
        assert_eq!(reparto, RepartoDeMemoria::default());
        assert_eq!(reparto.expertos_en_cpu, CapasEnCpu::Fijas(0));
        assert_eq!(reparto.ffn_en_cpu, CapasEnCpu::Fijas(0));
        assert_eq!(reparto.modo_de_carga, ModoDeCarga::Auto);
        assert!(!reparto.cache_en_cpu);
        assert!(reparto.tensores_en_cpu.is_empty());
    }

    #[test]
    fn cada_clave_se_lee() {
        let reparto = leer(
            "cache_k = \"f16\"\ncache_v = \"q4_0\"\ncache_en_cpu = true\n\
             modo_de_carga = \"mmap+mlock\"\nexpertos_en_cpu = \"auto\"\nffn_en_cpu = 6\n\
             tensores_en_cpu = [\"blk\\\\.3\\\\.ffn_up\"]\n",
        )
        .unwrap();
        assert_eq!(reparto.cache_k.as_deref(), Some("f16"));
        assert_eq!(reparto.cache_v.as_deref(), Some("q4_0"));
        assert!(reparto.cache_en_cpu);
        assert_eq!(reparto.modo_de_carga, ModoDeCarga::MmapMlock);
        assert_eq!(reparto.expertos_en_cpu, CapasEnCpu::Auto);
        assert_eq!(reparto.ffn_en_cpu, CapasEnCpu::Fijas(6));
        assert_eq!(
            reparto.tensores_en_cpu,
            vec!["blk\\.3\\.ffn_up".to_string()]
        );
    }

    #[test]
    fn cada_modo_de_carga_se_lee_y_se_escribe_con_su_palabra() {
        for (modo, palabra) in PALABRAS_DE_CARGA {
            let leido = leer(&format!("modo_de_carga = \"{palabra}\"")).unwrap();
            assert_eq!(leido.modo_de_carga, modo, "{palabra}");
            assert_eq!(modo.palabra(), palabra);
        }
        assert_eq!(
            leer("modo_de_carga = \"MMAP\"").unwrap().modo_de_carga,
            ModoDeCarga::Mmap
        );
    }

    #[test]
    fn un_modo_de_carga_que_no_existe_se_rechaza_con_las_palabras_que_valen() {
        // `--no-mmap` y `--mlock` ya no existen en b10993: quien venga de ellos escribirá algo así.
        let fallo = leer("modo_de_carga = \"no-mmap\"").unwrap_err();
        assert!(
            fallo.contains("«no-mmap» no vale para modo_de_carga"),
            "{fallo}"
        );
        assert!(fallo.contains("mmap+mlock"), "{fallo}");
    }

    #[test]
    fn las_capas_en_cpu_nombran_su_clave_al_fallar() {
        let fallo = leer("ffn_en_cpu = \"todas\"").unwrap_err();
        assert!(fallo.contains("no vale para ffn_en_cpu"), "{fallo}");
        let fallo = leer("expertos_en_cpu = -1").unwrap_err();
        assert!(fallo.contains("expertos_en_cpu solo admite"), "{fallo}");
    }

    #[test]
    fn un_patron_que_el_motor_partiria_se_rechaza() {
        for patron in ["exps=CPU", "a,b"] {
            let fallo = leer(&format!("tensores_en_cpu = [\"{patron}\"]")).unwrap_err();
            assert!(fallo.contains(&format!("«{patron}»")), "{fallo}");
        }
        let fallo = leer("tensores_en_cpu = [\"\"]").unwrap_err();
        assert!(fallo.contains("patrón vacío"), "{fallo}");
    }
}
