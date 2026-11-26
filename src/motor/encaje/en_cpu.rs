//! Qué parte de las capas se queda en la memoria del equipo: los expertos de `--n-cpu-moe` y las
//! FFN de `--n-cpu-ffn`, que llama.cpp deja en la CPU en las capas 0 a N-1
//! (`llm_add_n_cpu_ffn_overrides`, `common/common.h` de b10993). Aritmética pura sobre los
//! tensores ya leídos.
//!
//! Los patrones de `tensores_en_cpu` no se restan: son expresiones regulares de C++ y Programator
//! no tiene con qué evaluarlas igual que llama-server. Se cuentan como si fueran a la tarjeta, que
//! es contar de más: como mucho cuesta alguna capa que habría cabido, nunca una carga que no cabe.

use super::Encaje;
use crate::config::{CapasEnCpu, RepartoDeMemoria};
use crate::motor::gguf::tensores::{ClaseDeTensor, PesoDelModelo};

/// Cuántas capas dejan en la CPU sus expertos y cuántas sus FFN: los números que van a
/// `--n-cpu-moe` y a `--n-cpu-ffn`. Cero es ninguna, y entonces la opción no se pasa.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnCpu {
    pub expertos: u32,
    pub ffn: u32,
}

impl EnCpu {
    /// Lo que se escribió en el TOML, con `"auto"` como cero: lo que se usa cuando no hay con qué
    /// calcular.
    pub fn sin_calcular(reparto: &RepartoDeMemoria) -> Self {
        EnCpu {
            expertos: fijas_o_cero(reparto.expertos_en_cpu),
            ffn: fijas_o_cero(reparto.ffn_en_cpu),
        }
    }
}

fn fijas_o_cero(capas: CapasEnCpu) -> u32 {
    match capas {
        CapasEnCpu::Fijas(n) => n,
        CapasEnCpu::Auto => 0,
    }
}

/// El peso que queda en la tarjeta por capa, descontado lo que va a la CPU. Solo trae `por_capa` y
/// `fuera_de_capa`, que es lo que mira el reparto.
pub(super) fn pesos_en_la_tarjeta(pesos: &PesoDelModelo, en_cpu: EnCpu) -> PesoDelModelo {
    let mut por_capa = pesos.por_capa.clone();
    for tensor in &pesos.tensores {
        let Some(capa) = tensor.capa else {
            continue;
        };
        let limite = match tensor.clase() {
            ClaseDeTensor::Experto => en_cpu.expertos,
            ClaseDeTensor::FfnDensa => en_cpu.ffn,
            ClaseDeTensor::Otro => continue,
        };
        if (capa as u64) < u64::from(limite) {
            if let Some(peso) = por_capa.get_mut(capa) {
                *peso = peso.saturating_sub(tensor.bytes);
            }
        }
    }
    PesoDelModelo {
        tensores: Vec::new(),
        por_capa,
        fuera_de_capa: pesos.fuera_de_capa,
        inicio_datos: pesos.inicio_datos,
        fin_datos: pesos.fin_datos,
    }
}

/// Resuelve `"auto"`: el menor número de capas que, mandando esa parte a la CPU, deja el modelo
/// entero en la tarjeta. Si ninguno lo consigue, todas, que es lo que más memoria libera. Un modelo
/// sin tensores de esa clase da cero: no hay nada que mover. Con los dos en `"auto"` se resuelven
/// primero los expertos y después las FFN, con los expertos ya resueltos.
pub(super) fn resolver(
    reparto: &RepartoDeMemoria,
    pesos: &PesoDelModelo,
    decidir: impl Fn(&PesoDelModelo) -> Encaje,
) -> EnCpu {
    let mut en_cpu = EnCpu::sin_calcular(reparto);
    if reparto.expertos_en_cpu == CapasEnCpu::Auto {
        let base = en_cpu;
        en_cpu.expertos = menor_que_cabe(pesos, ClaseDeTensor::Experto, &decidir, move |n| EnCpu {
            expertos: n,
            ..base
        });
    }
    if reparto.ffn_en_cpu == CapasEnCpu::Auto {
        let base = en_cpu;
        en_cpu.ffn = menor_que_cabe(pesos, ClaseDeTensor::FfnDensa, &decidir, move |n| EnCpu {
            ffn: n,
            ..base
        });
    }
    en_cpu
}

fn menor_que_cabe(
    pesos: &PesoDelModelo,
    clase: ClaseDeTensor,
    decidir: &impl Fn(&PesoDelModelo) -> Encaje,
    con: impl Fn(u32) -> EnCpu,
) -> u32 {
    if !pesos.tensores.iter().any(|t| t.clase() == clase) {
        return 0;
    }
    let total = u32::try_from(pesos.por_capa.len()).unwrap_or(u32::MAX);
    (0..=total)
        .find(|n| {
            matches!(
                decidir(&pesos_en_la_tarjeta(pesos, con(*n))),
                Encaje::Completo { .. }
            )
        })
        .unwrap_or(total)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::gguf::tensores::TensorDelModelo;

    fn tensor(nombre: &str, bytes: u64, capa: usize) -> TensorDelModelo {
        TensorDelModelo {
            nombre: nombre.to_string(),
            bytes,
            capa: Some(capa),
        }
    }

    /// Tres capas: cada una con 10 de atención y 100 de expertos o de FFN, según `clase`.
    fn tres_capas(ffn: &str) -> PesoDelModelo {
        let tensores: Vec<TensorDelModelo> = (0..3)
            .flat_map(|capa| {
                [
                    tensor(&format!("blk.{capa}.attn_q.weight"), 10, capa),
                    tensor(&format!("blk.{capa}.{ffn}.weight"), 100, capa),
                ]
            })
            .collect();
        PesoDelModelo {
            tensores,
            por_capa: vec![110; 3],
            fuera_de_capa: 5,
            inicio_datos: 0,
            fin_datos: 0,
        }
    }

    /// Cabe entero si las capas y lo de fuera suman como mucho `presupuesto`.
    fn con_presupuesto(presupuesto: u64) -> impl Fn(&PesoDelModelo) -> Encaje {
        move |pesos: &PesoDelModelo| {
            let total: u64 = pesos.por_capa.iter().sum::<u64>() + pesos.fuera_de_capa;
            if total <= presupuesto {
                Encaje::Completo { total: 3 }
            } else {
                Encaje::NadaEnGpu
            }
        }
    }

    #[test]
    fn lo_que_va_a_la_cpu_se_resta_de_las_primeras_capas() {
        let pesos = tres_capas("ffn_up_exps");
        let vista = pesos_en_la_tarjeta(
            &pesos,
            EnCpu {
                expertos: 2,
                ffn: 0,
            },
        );
        assert_eq!(vista.por_capa, vec![10, 10, 110]);
        assert_eq!(vista.fuera_de_capa, 5);

        let densa = tres_capas("ffn_down");
        let vista = pesos_en_la_tarjeta(
            &densa,
            EnCpu {
                expertos: 3,
                ffn: 1,
            },
        );
        assert_eq!(
            vista.por_capa,
            vec![10, 110, 110],
            "los expertos no tocan una FFN densa"
        );
    }

    #[test]
    fn auto_busca_el_menor_numero_de_capas_que_cabe() {
        // 3 × 110 + 5 = 335. Con 135 de presupuesto hay que quitar 200: dos capas de expertos.
        let pesos = tres_capas("ffn_gate_exps");
        let reparto = RepartoDeMemoria {
            expertos_en_cpu: CapasEnCpu::Auto,
            ..Default::default()
        };
        let en_cpu = resolver(&reparto, &pesos, con_presupuesto(135));
        assert_eq!(
            en_cpu,
            EnCpu {
                expertos: 2,
                ffn: 0
            }
        );
        let en_cpu = resolver(&reparto, &pesos, con_presupuesto(335));
        assert_eq!(en_cpu.expertos, 0, "si ya cabe, no se mueve nada");
        let en_cpu = resolver(&reparto, &pesos, con_presupuesto(1));
        assert_eq!(en_cpu.expertos, 3, "si no cabe nunca, todas");
    }

    #[test]
    fn auto_en_ffn_vale_para_los_modelos_densos() {
        let pesos = tres_capas("ffn_up");
        let reparto = RepartoDeMemoria {
            ffn_en_cpu: CapasEnCpu::Auto,
            expertos_en_cpu: CapasEnCpu::Auto,
            ..Default::default()
        };
        let en_cpu = resolver(&reparto, &pesos, con_presupuesto(235));
        assert_eq!(
            en_cpu,
            EnCpu {
                expertos: 0,
                ffn: 1
            },
            "sin expertos que mover, auto en expertos da cero"
        );
    }

    /// Con las dos claves en `"auto"`, las FFN se deciden con los expertos ya en la CPU: si no,
    /// pedirían capas de más.
    #[test]
    fn con_las_dos_en_auto_las_ffn_cuentan_con_los_expertos_ya_resueltos() {
        // Cada capa: 10 de atención, 100 de expertos y 50 de FFN densa; 3 × 160 + 5 = 485.
        let tensores: Vec<TensorDelModelo> = (0..3)
            .flat_map(|capa| {
                [
                    tensor(&format!("blk.{capa}.attn_q.weight"), 10, capa),
                    tensor(&format!("blk.{capa}.ffn_up_exps.weight"), 100, capa),
                    tensor(&format!("blk.{capa}.ffn_down.weight"), 50, capa),
                ]
            })
            .collect();
        let pesos = PesoDelModelo {
            tensores,
            por_capa: vec![160; 3],
            fuera_de_capa: 5,
            inicio_datos: 0,
            fin_datos: 0,
        };
        let reparto = RepartoDeMemoria {
            ffn_en_cpu: CapasEnCpu::Auto,
            expertos_en_cpu: CapasEnCpu::Auto,
            ..Default::default()
        };
        // Sin ningún experto cabe 185 como mucho: con 135 van los tres a la CPU, y queda 60 por
        // capa. Una FFN más deja 10 + 60 + 60 + 5 = 135. Sin contar los expertos harían falta las
        // tres FFN.
        let en_cpu = resolver(&reparto, &pesos, con_presupuesto(135));
        assert_eq!(
            en_cpu,
            EnCpu {
                expertos: 3,
                ffn: 1
            }
        );
    }

    #[test]
    fn un_numero_fijo_no_se_recalcula() {
        let pesos = tres_capas("ffn_up");
        let reparto = RepartoDeMemoria {
            ffn_en_cpu: CapasEnCpu::Fijas(2),
            ..Default::default()
        };
        let en_cpu = resolver(&reparto, &pesos, con_presupuesto(10_000));
        assert_eq!(en_cpu.ffn, 2);
        assert_eq!(EnCpu::sin_calcular(&reparto).ffn, 2);
        let auto = RepartoDeMemoria {
            ffn_en_cpu: CapasEnCpu::Auto,
            ..Default::default()
        };
        assert_eq!(EnCpu::sin_calcular(&auto).ffn, 0);
    }
}
