//! Decide cuántas capas del modelo se piden a la GPU para una configuración, un modelo y una
//! tarjeta concretos, y explica por qué.
//!
//! Aquí vive la resolución: aplica el tope de `vram_maxima_mib`, elige entre la medida del banco y
//! el cálculo, y es el único punto que abre el fichero de pesos —solo su cabecera y su tabla de
//! tensores, nunca los pesos en sí— a través de `gguf::leer_modelo`. El resto es puro y se reparte
//! por submódulos: el tamaño del caché KV (`cache`), la política del reparto (`reparto`) y el
//! encaje de un modelo ya leído (`calculo`). Los textos para una persona se redactan en
//! `motor::mensajes`. Lo que este módulo **no** hace en ningún punto: preguntar al sistema gráfico
//! (la GPU llega por parámetro, nunca por `hardware::describir_gpu()`) ni lanzar ningún proceso.

mod cache;
mod calculo;
mod en_cpu;
mod reparto;

pub use en_cpu::EnCpu;

use super::gguf;
use super::hardware::Gpu;
use super::mensajes::{
    self, aviso_capas_bajadas, aviso_con_en_cpu, aviso_con_tope, aviso_de_medida,
    aviso_tope_sin_tarjeta, redactar_aviso,
};
use crate::config::{CapasEnCpu, CapasGpu, MotorConfig, RepartoDeMemoria};
use crate::unidades::BYTES_POR_MIB;
use calculo::{calcular_con_el_informe, Calculo};
use reparto::{capas_para_llama, capas_que_caben};
use std::path::Path;

/// Cuánto del modelo cabe en la GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Encaje {
    /// Cabe entero, capa de salida incluida. `total` son las capas que tiene el modelo.
    Completo { total: u32 },
    /// `en_gpu` capas de las `total` del modelo; el resto irá por CPU. `0 < en_gpu <= total`:
    /// cuando `en_gpu == total`, las capas caben todas pero `fuera_de_capa` —embeddings, salida y
    /// normalización final— no cupo con ellas, así que la salida se queda en CPU y por eso este
    /// desenlace no es `Completo`.
    Parcial { en_gpu: u32, total: u32 },
    /// No cabe ni la primera capa.
    NadaEnGpu,
    /// Falta un dato para calcular. Quien llame usa el valor configurado.
    NoCalculable { motivo: String },
}

/// Lo que Programator pedía antes de que existiera el encaje. Es el valor al que se vuelve cuando
/// `"auto"` no puede calcular nada: degradar a lo de siempre es mejor que negarse a arrancar.
pub(crate) const CAPAS_SI_NO_SE_SABE: u32 = 99;

/// El número de capas que va a `--n-gpu-layers` y la explicación de por qué.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolucion {
    pub capas: u32,
    /// Qué contar al usuario. Nunca vacío: si no hay nada que avisar, dice que cabe entero.
    pub aviso: String,
    /// Las capas cuyos expertos y cuyas FFN se quedan en la CPU, con `"auto"` ya resuelto.
    pub en_cpu: EnCpu,
}

/// Lo que el banco midió de un modelo en esta tarjeta: con cuántas capas lo cargó y cuánta VRAM
/// ocupó de verdad, y con qué contexto y caché. Sale del registro de modelos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MedidaDelBanco {
    pub capas: u32,
    pub vram_ocupada_mib: u64,
    pub contexto: u32,
    pub cache_kv: String,
    /// Las conversaciones en paralelo con que midió. La medida solo vale con las mismas: la caché
    /// se reserva una vez por conversación.
    pub paralelo: u32,
}

/// Decide el número de capas para este modelo, esta GPU y esta configuración.
///
/// Recibe la GPU en vez de preguntarla para poder probarse con tarjetas inventadas; `main` le pasa
/// la de verdad. El contexto configurado **no se toca nunca**: es una garantía funcional del arnés,
/// y recortarlo trunca encargos sin que nadie se entere, mientras que unas capas en CPU solo cuestan
/// lentitud, que se ve.
///
/// Con `vram_maxima_mib`, el tope manda: `"auto"` reparte la
/// menor entre la libre y el tope, y un número fijo que no quepa en el tope se baja a lo que cabe.
/// Con `"auto"` y una medida del banco que valga para esta configuración y quepa, se usan las capas
/// medidas: salen de lo que ocupó de verdad, no de un cálculo.
pub fn resolver(
    config: &MotorConfig,
    modelo: &Path,
    gpu: Option<&Gpu>,
    medida: Option<&MedidaDelBanco>,
) -> Resolucion {
    let tope = config
        .vram_maxima_mib
        .map(|mib| mib.saturating_mul(BYTES_POR_MIB));
    match config.capas_gpu {
        CapasGpu::Fijas(n) => resolver_fijas(config, modelo, gpu, n, tope),
        CapasGpu::Auto => resolver_auto(config, modelo, gpu, medida, tope),
    }
}

/// Cuánto del modelo cabría en la tarjeta con toda su memoria, recortada al tope
/// (`vram_maxima_mib`) y descontado el margen. Sirve para saber si un modelo cabe en esta tarjeta,
/// no para cargarlo ahora: no mira la memoria libre de este momento.
pub fn encaje_en_tarjeta(config: &MotorConfig, modelo: &Path, gpu: &Gpu) -> Encaje {
    let mut entera = gpu.clone();
    entera.vram_libre = gpu.vram_total;
    let tope = config
        .vram_maxima_mib
        .map(|mib| mib.saturating_mul(BYTES_POR_MIB));
    calcular(config, modelo, Some(&con_tope(&entera, tope))).encaje
}

/// Los bytes de tarjeta que el cálculo atribuye a una carga ya hecha, con `capas` en la tarjeta y
/// `en_cpu` en la CPU. El banco la compara con lo que ocupó de verdad para medir el margen de esa
/// máquina. `None` si el modelo no se deja leer o le falta algún dato.
pub fn vram_calculada(
    config: &MotorConfig,
    modelo: &Path,
    capas: u32,
    en_cpu: EnCpu,
) -> Option<u64> {
    let informe = gguf::leer_modelo(modelo).ok()?;
    calculo::vram_de_la_carga(config, &informe, capas, en_cpu)
}

/// La tarjeta tal como la ve el reparto: con la memoria libre recortada al tope, si lo hay.
fn con_tope(gpu: &Gpu, tope: Option<u64>) -> Gpu {
    let mut vista = gpu.clone();
    if let Some(tope) = tope {
        vista.vram_libre = vista.vram_libre.min(tope);
    }
    vista
}

fn resolver_auto(
    config: &MotorConfig,
    modelo: &Path,
    gpu: Option<&Gpu>,
    medida: Option<&MedidaDelBanco>,
    tope: Option<u64>,
) -> Resolucion {
    let vista = gpu.map(|g| con_tope(g, tope));
    if let (Some(vista), Some(medida)) = (vista.as_ref(), medida) {
        if medida_aplicable(config, medida, vista.vram_libre) {
            return Resolucion {
                capas: medida.capas,
                // El aviso cuenta la libre de verdad, no la recortada al tope: el tope va aparte.
                aviso: aviso_de_medida(medida, gpu.unwrap_or(vista), config.vram_maxima_mib),
                en_cpu: EnCpu::default(),
            };
        }
    }
    let calculo = calcular(config, modelo, vista.as_ref());
    let mut aviso = aviso_del_calculo(&calculo, vista.as_ref(), &config.reparto);
    if let Some(mib) = config.vram_maxima_mib {
        aviso = if gpu.is_some() {
            aviso_con_tope(&aviso, mib)
        } else {
            aviso_tope_sin_tarjeta(&aviso, mib)
        };
    }
    Resolucion {
        capas: capas_para_llama(&calculo.encaje, CAPAS_SI_NO_SE_SABE),
        aviso,
        en_cpu: calculo.en_cpu,
    }
}

/// El aviso del cálculo, con lo que `"auto"` dejó en la CPU.
fn aviso_del_calculo(calculo: &Calculo, gpu: Option<&Gpu>, reparto: &RepartoDeMemoria) -> String {
    let aviso = redactar_aviso(&calculo.encaje, calculo.coste, gpu);
    let si_es_auto = |capas: CapasEnCpu, n: u32| (capas == CapasEnCpu::Auto).then_some(n);
    aviso_con_en_cpu(
        &aviso,
        si_es_auto(reparto.expertos_en_cpu, calculo.en_cpu.expertos),
        si_es_auto(reparto.ffn_en_cpu, calculo.en_cpu.ffn),
    )
}

fn resolver_fijas(
    config: &MotorConfig,
    modelo: &Path,
    gpu: Option<&Gpu>,
    fijadas: u32,
    tope: Option<u64>,
) -> Resolucion {
    let calculo = calcular(config, modelo, gpu);
    let aviso = aviso_del_calculo(&calculo, gpu, &config.reparto);
    let en_cpu = calculo.en_cpu;
    let (Some(tope), Some(mib)) = (tope, config.vram_maxima_mib) else {
        // Sin tope, un número escrito en el TOML se transcribe tal cual: como mucho se avisa.
        return Resolucion {
            capas: fijadas,
            aviso,
            en_cpu,
        };
    };
    let Some(gpu) = gpu else {
        return Resolucion {
            capas: fijadas,
            aviso: aviso_tope_sin_tarjeta(&aviso, mib),
            en_cpu,
        };
    };
    // Lo que cabe en el tope se calcula con el tope como memoria: si el número cabe en él pero no
    // en la libre, se respeta, como cualquier número fijo.
    let mut en_el_tope = gpu.clone();
    en_el_tope.vram_libre = tope;
    let del_tope = calcular(config, modelo, Some(&en_el_tope));
    // Con tope, lo que va a la CPU se decide contra el tope, igual que las capas: un "auto"
    // calculado con toda la memoria libre podría no caber en él.
    match capas_que_caben(&del_tope.encaje) {
        Some(caben) if fijadas > caben => Resolucion {
            capas: caben,
            aviso: aviso_capas_bajadas(&aviso, mib, fijadas, caben),
            en_cpu: del_tope.en_cpu,
        },
        _ => Resolucion {
            capas: fijadas,
            aviso,
            en_cpu: del_tope.en_cpu,
        },
    }
}

/// Si la medida del banco vale aquí: mismo contexto, misma caché, las mismas conversaciones en
/// paralelo, nada en la CPU, y lo que ocupó, más el margen, cabe en la memoria de que se dispone.
/// El banco mide sin reparto: con algo en la CPU, lo que ocupó deja de valer.
fn medida_aplicable(config: &MotorConfig, medida: &MedidaDelBanco, disponible: u64) -> bool {
    let ocupa = medida.vram_ocupada_mib.saturating_mul(BYTES_POR_MIB);
    let margen = config.margen_vram_mib.saturating_mul(BYTES_POR_MIB);
    medida.contexto == config.contexto
        && medida.cache_kv.eq_ignore_ascii_case(config.cache_k())
        && medida.cache_kv.eq_ignore_ascii_case(config.cache_v())
        && config.paralelo.max(1) == medida.paralelo.max(1)
        && sin_nada_en_cpu(&config.reparto)
        && ocupa.saturating_add(margen) <= disponible
}

/// Si el reparto deja todo en la tarjeta, como cuando mide el banco.
fn sin_nada_en_cpu(reparto: &RepartoDeMemoria) -> bool {
    !reparto.cache_en_cpu
        && reparto.expertos_en_cpu == CapasEnCpu::Fijas(0)
        && reparto.ffn_en_cpu == CapasEnCpu::Fijas(0)
        && reparto.tensores_en_cpu.is_empty()
}

/// Junta las piezas: sin tarjeta no hay nada que calcular; con ella, lee la cabecera y la tabla de
/// tensores del GGUF —la única E/S del encaje— y deja el cálculo a `calcular_con_el_informe`. Un
/// modelo que no se deja leer produce un `NoCalculable` que lo dice.
fn calcular(config: &MotorConfig, modelo: &Path, gpu: Option<&Gpu>) -> Calculo {
    let sin_calcular = |motivo: String| Calculo {
        encaje: Encaje::NoCalculable { motivo },
        coste: None,
        en_cpu: EnCpu::sin_calcular(&config.reparto),
    };
    let Some(gpu) = gpu else {
        return sin_calcular(mensajes::SIN_GPU.to_string());
    };

    match gguf::leer_modelo(modelo) {
        Ok(informe) => calcular_con_el_informe(config, &informe, gpu.vram_libre),
        Err(fallo) => sin_calcular(mensajes::modelo_sin_pesar(&fallo)),
    }
}

#[cfg(test)]
mod pruebas_resolver {
    use super::*;
    use crate::motor::gguf::tensores::pruebas_apoyo::{
        escribir_temporal, fichero_con_tensores, gguf_de_juguete, ValorPrueba,
    };

    /// El GGUF de juguete tiene cuatro capas de 1 MiB, 256 KiB fuera de capa, 512 de embedding,
    /// 8 cabezas y 2 de KV. Con `contexto` bajo, el caché es despreciable y manda el peso.
    fn config(capas_gpu: CapasGpu, contexto: u32) -> MotorConfig {
        MotorConfig {
            binario: None,
            modelo: None,
            puerto: 8080,
            contexto,
            capas_gpu,
            cache_kv: "q8_0".to_string(),
            paralelo: 1,
            empezar_en_frio: true,
            margen_vram_mib: 0,
            umbral_aviso_contexto: 70,
            bytes_por_token: 4,
            tiempo_lectura_segundos: crate::config::tiempo_lectura_motor_segundos_por_defecto(),
            tiempo_escritura_segundos: crate::config::tiempo_escritura_motor_segundos_por_defecto(),
            soltar_modelo: crate::config::SoltarModelo::Auto,
            vram_maxima_mib: None,
            espera_de_carga_segundos: crate::config::ESPERA_DE_CARGA_POR_DEFECTO_SEGUNDOS,
            atencion_rapida: crate::config::AtencionRapida::default(),
            clave: crate::config::ClaveConfigurada::Auto,
            reparto: Default::default(),
        }
    }

    fn gpu(vram: u64) -> Gpu {
        Gpu {
            nombre: "inventada".to_string(),
            vram_total: vram,
            vram_libre: vram,
        }
    }

    /// Cuatro capas, cada una con 64 KiB de atención y 1 MiB de FFN llamada `blk.<i>.<ffn>.weight`,
    /// y 64 KiB fuera de capa. Con contexto 512 y caché q8_0, cada capa reserva además 136 KiB de
    /// caché: el modelo entero pide 5 079 040 bytes.
    fn gguf_con_ffn(ffn: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        use crate::motor::gguf::tensores::pruebas_apoyo::TensorPrueba;
        let f32_de = |bytes: u64| vec![bytes / 4];
        let mut tensores: Vec<TensorPrueba> = (0..4)
            .flat_map(|capa| {
                [
                    TensorPrueba {
                        nombre: format!("blk.{capa}.attn_q.weight"),
                        dimensiones: f32_de(64 * 1024),
                        tipo: 0,
                    },
                    TensorPrueba {
                        nombre: format!("blk.{capa}.{ffn}.weight"),
                        dimensiones: f32_de(1024 * 1024),
                        tipo: 0,
                    },
                ]
            })
            .collect();
        tensores.push(TensorPrueba {
            nombre: "token_embd.weight".to_string(),
            dimensiones: f32_de(64 * 1024),
            tipo: 0,
        });
        let pares = vec![
            ("general.architecture", ValorPrueba::Cadena("llama".into())),
            ("llama.block_count", ValorPrueba::U32(4)),
            ("llama.embedding_length", ValorPrueba::U32(512)),
            ("llama.attention.head_count", ValorPrueba::U32(8)),
            ("llama.attention.head_count_kv", ValorPrueba::U32(2)),
            ("llama.context_length", ValorPrueba::U32(32_768)),
        ];
        escribir_temporal(&fichero_con_tensores(&pares, &tensores, 32))
    }

    fn con_reparto(reparto: RepartoDeMemoria) -> MotorConfig {
        let mut config = config(CapasGpu::Auto, 512);
        config.reparto = reparto;
        config
    }

    #[test]
    fn auto_en_expertos_deja_en_la_cpu_las_capas_justas_para_que_quepa_entero() {
        // G4. Hay que liberar 2 079 040 bytes de los 5 079 040: dos capas de expertos de 1 MiB.
        let (_dir, ruta) = gguf_con_ffn("ffn_up_exps");
        let config = con_reparto(RepartoDeMemoria {
            expertos_en_cpu: CapasEnCpu::Auto,
            ..Default::default()
        });

        let resolucion = resolver(&config, &ruta, Some(&gpu(3_000_000)), None);

        assert_eq!(
            resolucion.en_cpu,
            EnCpu {
                expertos: 2,
                ffn: 0
            }
        );
        assert_eq!(resolucion.capas, 5, "cabe entero, capa de salida incluida");
        assert!(
            resolucion
                .aviso
                .contains("los expertos de las 2 primeras capas"),
            "{}",
            resolucion.aviso
        );
    }

    #[test]
    fn auto_en_ffn_hace_lo_mismo_con_un_modelo_denso() {
        // G4, con un modelo denso: auto en los expertos no encuentra nada que mover.
        let (_dir, ruta) = gguf_con_ffn("ffn_up");
        let config = con_reparto(RepartoDeMemoria {
            expertos_en_cpu: CapasEnCpu::Auto,
            ffn_en_cpu: CapasEnCpu::Auto,
            ..Default::default()
        });

        let resolucion = resolver(&config, &ruta, Some(&gpu(3_000_000)), None);

        assert_eq!(
            resolucion.en_cpu,
            EnCpu {
                expertos: 0,
                ffn: 2
            }
        );
        assert_eq!(resolucion.capas, 5);
    }

    #[test]
    fn sin_reparto_el_mismo_modelo_pierde_capas() {
        let (_dir, ruta) = gguf_con_ffn("ffn_up");

        let resolucion = resolver(
            &con_reparto(Default::default()),
            &ruta,
            Some(&gpu(3_000_000)),
            None,
        );

        assert_eq!(resolucion.en_cpu, EnCpu::default());
        assert!(resolucion.capas < 4, "{resolucion:?}");
    }

    #[test]
    fn la_cache_en_cpu_deja_sitio_para_mas_ffn_en_la_tarjeta() {
        // Con 3 600 000 bytes: con la caché en la tarjeta hacen falta dos capas de FFN fuera; sin
        // ella, basta una.
        let (_dir, ruta) = gguf_con_ffn("ffn_down");
        let en_la_tarjeta = con_reparto(RepartoDeMemoria {
            ffn_en_cpu: CapasEnCpu::Auto,
            ..Default::default()
        });
        let en_cpu = con_reparto(RepartoDeMemoria {
            ffn_en_cpu: CapasEnCpu::Auto,
            cache_en_cpu: true,
            ..Default::default()
        });
        let tarjeta = gpu(3_600_000);

        assert_eq!(
            resolver(&en_la_tarjeta, &ruta, Some(&tarjeta), None)
                .en_cpu
                .ffn,
            2
        );
        assert_eq!(resolver(&en_cpu, &ruta, Some(&tarjeta), None).en_cpu.ffn, 1);
    }

    #[test]
    fn la_cache_de_claves_y_la_de_valores_cuentan_cada_una_con_su_tipo() {
        // Con f16 en las claves (64 bytes cada 32 valores frente a los 34 de q8_0), la caché crece
        // y en la misma tarjeta hay que sacar más FFN: tres capas con q8_0, las cuatro con f16.
        let (_dir, ruta) = gguf_con_ffn("ffn_up");
        let mut q8 = con_reparto(RepartoDeMemoria {
            ffn_en_cpu: CapasEnCpu::Auto,
            ..Default::default()
        });
        q8.contexto = 4096;
        let mut f16_en_claves = q8.clone();
        f16_en_claves.reparto.cache_k = Some("f16".into());
        let mut f16_en_valores = q8.clone();
        f16_en_valores.reparto.cache_v = Some("f16".into());
        let tarjeta = gpu(6_000_000);
        let ffn = |config: &MotorConfig| resolver(config, &ruta, Some(&tarjeta), None).en_cpu.ffn;

        assert_eq!(ffn(&q8), 3);
        assert_eq!(ffn(&f16_en_claves), 4);
        assert_eq!(
            ffn(&f16_en_valores),
            4,
            "los valores cuentan con su propio tipo"
        );
    }

    #[test]
    fn con_capas_fijas_y_tope_lo_que_va_a_la_cpu_se_decide_contra_el_tope() {
        // Con 100 MB libres cabría todo sin mover nada; el tope de 3 MiB (3 145 728 bytes) obliga
        // a sacar dos capas de FFN, como en `auto_en_ffn_hace_lo_mismo_con_un_modelo_denso`.
        let (_dir, ruta) = gguf_con_ffn("ffn_up");
        let mut config = con_reparto(RepartoDeMemoria {
            ffn_en_cpu: CapasEnCpu::Auto,
            ..Default::default()
        });
        config.capas_gpu = CapasGpu::Fijas(99);
        config.vram_maxima_mib = Some(3);

        let resolucion = resolver(&config, &ruta, Some(&gpu(100_000_000)), None);

        assert_eq!(resolucion.en_cpu.ffn, 2, "{resolucion:?}");
        assert_eq!(
            resolucion.capas, 99,
            "con todo cabiendo en el tope, el número se respeta"
        );
    }

    #[test]
    fn la_vram_calculada_de_una_carga_cuenta_lo_que_va_a_la_tarjeta() {
        // Las cifras de `gguf_con_ffn`: 1 114 112 de pesos y 139 264 de caché por capa, 65 536
        // fuera de capa.
        let (_dir, ruta) = gguf_con_ffn("ffn_up");
        let config = con_reparto(Default::default());
        let nada = EnCpu::default();
        let dos_ffn = EnCpu {
            expertos: 0,
            ffn: 2,
        };
        let calculada =
            |config: &MotorConfig, capas, en_cpu| vram_calculada(config, &ruta, capas, en_cpu);

        assert_eq!(
            calculada(&config, 5, nada),
            Some(5_079_040),
            "entero, con la salida"
        );
        assert_eq!(
            calculada(&config, 4, nada),
            Some(5_013_504),
            "sin la salida"
        );
        assert_eq!(calculada(&config, 1, nada), Some(1_253_376));
        assert_eq!(calculada(&config, 0, nada), Some(0));
        assert_eq!(calculada(&config, 5, dos_ffn), Some(2_981_888));
        let mut sin_cache = config.clone();
        sin_cache.reparto.cache_en_cpu = true;
        assert_eq!(calculada(&sin_cache, 5, nada), Some(4_521_984));
        assert_eq!(
            vram_calculada(&config, Path::new("no-existe.gguf"), 5, nada),
            None
        );
    }

    #[test]
    fn el_aviso_solo_cuenta_lo_que_decidio_auto() {
        let (_dir, ruta) = gguf_con_ffn("ffn_up");
        let fijas = con_reparto(RepartoDeMemoria {
            ffn_en_cpu: CapasEnCpu::Fijas(2),
            ..Default::default()
        });

        let resolucion = resolver(&fijas, &ruta, Some(&gpu(3_000_000)), None);

        assert_eq!(resolucion.en_cpu.ffn, 2);
        assert!(
            !resolucion.aviso.contains("primeras capas"),
            "un número escrito en el TOML ya lo sabe quien lo escribió: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn sin_tarjeta_auto_no_manda_nada_a_la_cpu_y_un_numero_fijo_se_respeta() {
        let (_dir, ruta) = gguf_con_ffn("ffn_up");
        let config = con_reparto(RepartoDeMemoria {
            expertos_en_cpu: CapasEnCpu::Auto,
            ffn_en_cpu: CapasEnCpu::Fijas(3),
            ..Default::default()
        });

        let resolucion = resolver(&config, &ruta, None, None);

        assert_eq!(
            resolucion.en_cpu,
            EnCpu {
                expertos: 0,
                ffn: 3
            }
        );
    }

    #[test]
    fn la_medida_del_banco_no_vale_con_algo_en_la_cpu() {
        let (_dir, ruta) = gguf_de_juguete();
        let medida = MedidaDelBanco {
            capas: 2,
            vram_ocupada_mib: 1,
            contexto: 512,
            cache_kv: "q8_0".into(),
            paralelo: 1,
        };
        let tarjeta = gpu(100_000_000);
        let sin_reparto = con_reparto(Default::default());
        assert_eq!(
            resolver(&sin_reparto, &ruta, Some(&tarjeta), Some(&medida)).capas,
            2,
            "sin reparto, la medida manda"
        );
        for reparto in [
            RepartoDeMemoria {
                cache_en_cpu: true,
                ..Default::default()
            },
            RepartoDeMemoria {
                ffn_en_cpu: CapasEnCpu::Fijas(1),
                ..Default::default()
            },
            RepartoDeMemoria {
                expertos_en_cpu: CapasEnCpu::Auto,
                ..Default::default()
            },
            RepartoDeMemoria {
                tensores_en_cpu: vec!["output".into()],
                ..Default::default()
            },
            RepartoDeMemoria {
                cache_v: Some("f16".into()),
                ..Default::default()
            },
        ] {
            let config = con_reparto(reparto.clone());
            assert_ne!(
                resolver(&config, &ruta, Some(&tarjeta), Some(&medida)).capas,
                2,
                "con {reparto:?} la medida no vale"
            );
        }
    }

    #[test]
    fn mas_conversaciones_en_paralelo_dejan_menos_capas_en_la_gpu() {
        // El defecto que esta prueba cierra, medido el 22/09/2026: `llama-server` reserva la caché
        // KV **una vez por secuencia**, y el encaje la contaba una sola vez. Con los cuatro slots
        // que el motor trae por defecto, el consumo real era cuatro veces el calculado y el
        // reparto se decidía sobre un presupuesto que no existía.
        // El modelo de juguete son cuatro capas de 1 MiB. Con contexto 2048 y caché q8_0, cada
        // capa carga además unos 0,53 MiB de caché por secuencia. Con estos 5 MB de tarjeta salen
        // tres capas si hay una sola secuencia y una sola si hay cuatro: la diferencia se ve.
        let (_dir, ruta) = gguf_de_juguete();
        let tarjeta = gpu(5_000_000);

        let mut una = config(CapasGpu::Auto, 2048);
        una.paralelo = 1;
        let mut cuatro = config(CapasGpu::Auto, 2048);
        cuatro.paralelo = 4;

        let con_una = resolver(&una, &ruta, Some(&tarjeta), None).capas;
        let con_cuatro = resolver(&cuatro, &ruta, Some(&tarjeta), None).capas;

        assert!(
            con_cuatro < con_una,
            "cuatro secuencias reservan cuatro cachés y deben caber menos capas: \
             con paralelo=1 salieron {con_una} y con paralelo=4, {con_cuatro}"
        );
    }

    #[test]
    fn un_numero_configurado_se_respeta_aunque_el_calculo_diga_que_no_cabe() {
        let (_dir, ruta) = gguf_de_juguete();

        let resolucion = resolver(
            &config(CapasGpu::Fijas(99), 512),
            &ruta,
            Some(&gpu(1_000)),
            None,
        );

        assert_eq!(resolucion.capas, 99, "lo escrito en el TOML manda");
        assert!(
            !resolucion.aviso.is_empty(),
            "pero el aviso tiene que contar que con eso no cabe"
        );
    }

    #[test]
    fn sin_gpu_medible_auto_cae_al_99_de_siempre_diciendo_por_que() {
        let (_dir, ruta) = gguf_de_juguete();

        let resolucion = resolver(&config(CapasGpu::Auto, 512), &ruta, None, None);

        assert_eq!(resolucion.capas, 99);
        assert!(
            resolucion.aviso.to_lowercase().contains("gpu"),
            "tiene que decir que el dato que faltó es la GPU: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn con_un_modelo_ilegible_auto_cae_al_99_sin_tumbar_el_arranque() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("no-existe.gguf");

        let resolucion = resolver(
            &config(CapasGpu::Auto, 512),
            &ruta,
            Some(&gpu(16 << 30)),
            None,
        );

        assert_eq!(resolucion.capas, 99);
        assert!(!resolucion.aviso.is_empty());
    }

    #[test]
    fn con_vram_de_sobra_auto_sube_el_modelo_entero_y_la_salida() {
        let (_dir, ruta) = gguf_de_juguete();

        let resolucion = resolver(
            &config(CapasGpu::Auto, 512),
            &ruta,
            Some(&gpu(16 << 30)),
            None,
        );

        assert_eq!(resolucion.capas, 5, "cuatro capas más la salida");
        assert!(
            resolucion.aviso.contains("piden") && resolucion.aviso.contains("GiB"),
            "el aviso de «cabe entero» tiene que decir cuánto pide el modelo, no solo cuánto \
             hay libre: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn el_encaje_en_la_tarjeta_mira_la_memoria_total_y_el_tope_no_la_libre() {
        let (_dir, ruta) = gguf_de_juguete();
        let ocupada = Gpu {
            nombre: "inventada".to_string(),
            vram_total: 16 << 30,
            vram_libre: 100,
        };

        assert_eq!(
            encaje_en_tarjeta(&config(CapasGpu::Auto, 512), &ruta, &ocupada),
            Encaje::Completo { total: 4 },
            "con la tarjeta llena ahora, el modelo cabe igual en ella"
        );

        // Con un tope de 2 MiB solo caben dos capas de 1 MiB, aunque la tarjeta tenga 16 GiB.
        let mut con_tope = config(CapasGpu::Auto, 0);
        con_tope.vram_maxima_mib = Some(2);
        con_tope.margen_vram_mib = 0;
        assert_eq!(
            encaje_en_tarjeta(&con_tope, &ruta, &ocupada),
            Encaje::Parcial {
                en_gpu: 2,
                total: 4
            }
        );
    }

    #[test]
    fn ni_la_primera_capa_cabe_y_el_aviso_dice_cuanto_pedia() {
        let (_dir, ruta) = gguf_de_juguete();
        // El GGUF de juguete tiene capas de 1 MiB; con 100 bytes libres no cabe ni una.
        let resolucion = resolver(&config(CapasGpu::Auto, 512), &ruta, Some(&gpu(100)), None);

        assert_eq!(resolucion.capas, 0);
        assert!(
            resolucion.aviso.contains("pide") && resolucion.aviso.contains("GiB"),
            "el aviso de «no cabe ninguna» tiene que decir cuánto pedía la capa que no cupo: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn el_aviso_del_reparto_parcial_dice_cuantas_capas_van_a_cada_sitio() {
        let (_dir, ruta) = gguf_de_juguete();
        // Dos capas de 1 MiB caben en 2,5 MiB; las cuatro más los embeddings, no.
        let vram = 2 * 1024 * 1024 + 512 * 1024;

        let resolucion = resolver(&config(CapasGpu::Auto, 512), &ruta, Some(&gpu(vram)), None);

        assert_eq!(resolucion.capas, 2);
        assert!(
            resolucion.aviso.contains("de 4"),
            "el aviso tiene que decir cuántas de cuántas: {}",
            resolucion.aviso
        );
        assert!(
            resolucion.aviso.contains("piden") && resolucion.aviso.contains("GiB"),
            "el aviso de un reparto parcial tiene que decir cuánto pedían esas dos capas: {}",
            resolucion.aviso
        );
    }

    /// La configuración de juguete con un tope de VRAM.
    fn con_tope(capas: CapasGpu, tope_mib: u64) -> MotorConfig {
        let mut configuracion = config(capas, 512);
        configuracion.vram_maxima_mib = Some(tope_mib);
        configuracion
    }

    /// Lo que midió el banco con el modelo de juguete y la configuración de juguete.
    fn medida(capas: u32, vram_ocupada_mib: u64) -> MedidaDelBanco {
        MedidaDelBanco {
            capas,
            vram_ocupada_mib,
            contexto: 512,
            cache_kv: "q8_0".to_string(),
            paralelo: 1,
        }
    }

    #[test]
    fn auto_no_pasa_del_tope_aunque_sobre_memoria() {
        let (_dir, ruta) = gguf_de_juguete();
        // Con 3 MiB caben dos capas de 1 MiB con su caché; la tarjeta tiene 16 GiB libres.
        let resolucion = resolver(
            &con_tope(CapasGpu::Auto, 3),
            &ruta,
            Some(&gpu(16 << 30)),
            None,
        );

        assert_eq!(resolucion.capas, 2);
        assert!(
            resolucion.aviso.contains("tope de 3 MiB"),
            "{}",
            resolucion.aviso
        );
    }

    #[test]
    fn con_medida_y_tope_el_aviso_cuenta_la_libre_de_verdad_y_el_tope_aparte() {
        let (_dir, ruta) = gguf_de_juguete();
        let mut configuracion = config(CapasGpu::Auto, 512);
        configuracion.vram_maxima_mib = Some(1000);

        let resolucion = resolver(
            &configuracion,
            &ruta,
            Some(&gpu(16 << 30)),
            Some(&medida(3, 100)),
        );

        assert_eq!(resolucion.capas, 3);
        assert!(
            resolucion
                .aviso
                .contains("16,0 GiB libres, con el tope de 1000 MiB"),
            "{}",
            resolucion.aviso
        );
    }

    #[test]
    fn un_tope_mayor_que_la_libre_no_cambia_nada() {
        let (_dir, ruta) = gguf_de_juguete();
        let vram = 2 * 1024 * 1024 + 512 * 1024;

        let resolucion = resolver(
            &con_tope(CapasGpu::Auto, 1000),
            &ruta,
            Some(&gpu(vram)),
            None,
        );

        assert_eq!(resolucion.capas, 2, "manda la libre, que es menor");
    }

    #[test]
    fn auto_usa_lo_medido_por_el_banco_si_vale_y_cabe() {
        let (_dir, ruta) = gguf_de_juguete();

        let resolucion = resolver(
            &config(CapasGpu::Auto, 512),
            &ruta,
            Some(&gpu(16 << 30)),
            Some(&medida(3, 100)),
        );

        assert_eq!(
            resolucion.capas, 3,
            "las capas medidas, no las calculadas (5)"
        );
        assert!(
            resolucion.aviso.contains("midió el banco"),
            "{}",
            resolucion.aviso
        );
    }

    #[test]
    fn lo_medido_que_no_cabe_en_el_tope_no_se_usa() {
        let (_dir, ruta) = gguf_de_juguete();

        let resolucion = resolver(
            &con_tope(CapasGpu::Auto, 50),
            &ruta,
            Some(&gpu(16 << 30)),
            Some(&medida(3, 100)),
        );

        assert_eq!(
            resolucion.capas, 5,
            "se calcula: las capas caben enteras en 50 MiB"
        );
    }

    #[test]
    fn lo_medido_con_otro_contexto_otra_cache_o_mas_conversaciones_no_se_usa() {
        let (_dir, ruta) = gguf_de_juguete();
        let tarjeta = gpu(16 << 30);

        let mut otro_contexto = medida(3, 100);
        otro_contexto.contexto = 1024;
        let mut otra_cache = medida(3, 100);
        otra_cache.cache_kv = "f16".to_string();
        for medida in [otro_contexto, otra_cache] {
            let resolucion = resolver(
                &config(CapasGpu::Auto, 512),
                &ruta,
                Some(&tarjeta),
                Some(&medida),
            );
            assert_eq!(resolucion.capas, 5, "{medida:?}");
        }

        let mut dos = config(CapasGpu::Auto, 512);
        dos.paralelo = 2;
        let resolucion = resolver(&dos, &ruta, Some(&tarjeta), Some(&medida(3, 100)));
        assert_ne!(
            resolucion.capas, 3,
            "la medida es de una sola conversación y aquí hay dos"
        );
    }

    /// La medida se compara con el `paralelo` configurado, no con un 1 incrustado: con
    /// `paralelo = 2` vale si el banco midió también con dos.
    #[test]
    fn una_medida_con_las_mismas_conversaciones_en_paralelo_vale() {
        let (_dir, ruta) = gguf_de_juguete();
        let tarjeta = gpu(16 << 30);
        let mut dos = config(CapasGpu::Auto, 512);
        dos.paralelo = 2;
        let medida_con_dos = MedidaDelBanco {
            paralelo: 2,
            ..medida(3, 100)
        };

        let resolucion = resolver(&dos, &ruta, Some(&tarjeta), Some(&medida_con_dos));

        assert_eq!(resolucion.capas, 3, "{}", resolucion.aviso);
    }

    #[test]
    fn un_numero_fijo_que_no_cabe_en_el_tope_se_baja_y_se_avisa() {
        let (_dir, ruta) = gguf_de_juguete();

        let resolucion = resolver(
            &con_tope(CapasGpu::Fijas(4), 3),
            &ruta,
            Some(&gpu(16 << 30)),
            None,
        );

        assert_eq!(resolucion.capas, 2);
        assert!(
            resolucion.aviso.contains("no caben en el tope"),
            "{}",
            resolucion.aviso
        );
    }

    #[test]
    fn un_numero_fijo_que_cabe_en_el_tope_se_respeta_aunque_no_quepa_en_la_libre() {
        let (_dir, ruta) = gguf_de_juguete();

        let dentro = resolver(
            &con_tope(CapasGpu::Fijas(2), 3),
            &ruta,
            Some(&gpu(16 << 30)),
            None,
        );
        assert_eq!(dentro.capas, 2);

        let sin_libre = resolver(
            &con_tope(CapasGpu::Fijas(4), 100),
            &ruta,
            Some(&gpu(1024)),
            None,
        );
        assert_eq!(
            sin_libre.capas, 4,
            "el tope no se pasa; la libre la decide quien fija"
        );

        let todas = resolver(
            &con_tope(CapasGpu::Fijas(99), 100),
            &ruta,
            Some(&gpu(16 << 30)),
            None,
        );
        assert_eq!(todas.capas, 99, "si cabe entero en el tope no se toca");
    }

    #[test]
    fn sin_tarjeta_medible_el_tope_no_se_aplica_y_se_dice() {
        let (_dir, ruta) = gguf_de_juguete();

        let fijas = resolver(&con_tope(CapasGpu::Fijas(4), 3), &ruta, None, None);
        assert_eq!(fijas.capas, 4);
        assert!(
            fijas.aviso.contains("no se ha podido aplicar"),
            "{}",
            fijas.aviso
        );

        let auto = resolver(&con_tope(CapasGpu::Auto, 3), &ruta, None, None);
        assert_eq!(auto.capas, 99);
        assert!(
            auto.aviso.contains("no se ha podido aplicar"),
            "{}",
            auto.aviso
        );
    }

    #[test]
    fn un_tipo_de_cache_que_llama_server_no_admite_no_es_calculable_y_lo_nombra() {
        let (_dir, ruta) = gguf_de_juguete();
        let mut configuracion = config(CapasGpu::Fijas(11), 512);
        // «q3_k» existe en ggml pero no está entre los tipos que `llama-server` admite para
        // `--cache-type-k`/`--cache-type-v` (ver `tipo_de_cache`): es el mismo caso que ya cubre
        // `un_tipo_de_cache_inventado_no_se_acepta` en `cache`, aquí visto desde `resolver`.
        configuracion.cache_kv = "q3_k".to_string();

        let resolucion = resolver(&configuracion, &ruta, Some(&gpu(16 << 30)), None);

        assert_eq!(
            resolucion.capas, 11,
            "sin cálculo se respeta lo configurado"
        );
        assert!(
            resolucion.aviso.contains("q3_k"),
            "el aviso tiene que nombrar el tipo de caché que rechazó: {}",
            resolucion.aviso
        );
        assert!(
            !resolucion.aviso.to_lowercase().contains("gpu dedicada"),
            "no puede confundirse con el motivo de falta de GPU, que es otra rama: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn cero_cabezas_de_atencion_no_es_calculable_y_no_entra_en_panico() {
        // La guarda de `calculo` está antes de `embedding / cabezas`: sin ella, esta cabecera
        // dividiría por cero. Copia los pares del GGUF de juguete cambiando solo
        // `attention.head_count` a 0; sin tensores, porque la guarda actúa antes de pesar nada.
        let pares = vec![
            ("general.architecture", ValorPrueba::Cadena("llama".into())),
            ("llama.block_count", ValorPrueba::U32(4)),
            ("llama.embedding_length", ValorPrueba::U32(512)),
            ("llama.attention.head_count", ValorPrueba::U32(0)),
            ("llama.attention.head_count_kv", ValorPrueba::U32(2)),
            ("llama.context_length", ValorPrueba::U32(32_768)),
        ];
        let (_dir, ruta) = escribir_temporal(&fichero_con_tensores(&pares, &[], 32));

        let resolucion = resolver(
            &config(CapasGpu::Fijas(7), 512),
            &ruta,
            Some(&gpu(16 << 30)),
            None,
        );

        assert_eq!(resolucion.capas, 7, "sin cálculo se respeta lo configurado");
        assert!(
            resolucion.aviso.contains("cero cabezas"),
            "el aviso tiene que decir que las cabezas de atención no valen: {}",
            resolucion.aviso
        );
        assert!(
            !resolucion.aviso.to_lowercase().contains("gpu dedicada"),
            "no puede confundirse con el motivo de falta de GPU, que es otra rama: {}",
            resolucion.aviso
        );
    }
}
