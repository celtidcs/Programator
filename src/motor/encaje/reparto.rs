//! La política del reparto de capas: cuántas caben en unos bytes de VRAM, cuánto cuesta lo
//! decidido y qué número se le pasa a `--n-gpu-layers`. Aritmética pura sobre los pesos ya medidos:
//! se prueba entera con números inventados, sin disco ni GPU delante.

use super::Encaje;
use crate::motor::gguf::tensores::PesoDelModelo;
use crate::motor::mensajes;

/// Decide cuántas capas caben en `libre` bytes de VRAM, reservando `margen`.
///
/// El coste de `n` capas es la suma de las `n` **últimas** más `n × kv_por_capa`, porque son las
/// últimas las que `llama.cpp` sube a la GPU. Como el coste crece con `n`, basta con buscar el
/// mayor que quepa; con cuarenta capas la búsqueda lineal sobra y se lee mejor que una binaria.
pub(super) fn decidir(pesos: &PesoDelModelo, kv_por_capa: u64, libre: u64, margen: u64) -> Encaje {
    let total = pesos.por_capa.len();
    if total == 0 {
        return Encaje::NoCalculable {
            motivo: mensajes::SIN_CAPAS.to_string(),
        };
    }
    let Ok(total_u32) = u32::try_from(total) else {
        return Encaje::NoCalculable {
            motivo: mensajes::capas_increibles(total),
        };
    };

    let disponible = libre.saturating_sub(margen);

    // El modelo entero incluye lo que no es de ninguna capa: embeddings, salida y normalización.
    // `token_embd.weight` se cuenta aquí aunque llama.cpp suela dejarlo en RAM: sobreestimar cuesta
    // como mucho una capa menos de las posibles, y subestimar cuesta un fallo de reserva.
    let coste_entero = coste_de_las_ultimas(pesos, total, kv_por_capa)
        .and_then(|c| c.checked_add(pesos.fuera_de_capa));
    if coste_entero.is_some_and(|coste| coste <= disponible) {
        return Encaje::Completo { total: total_u32 };
    }

    // Todas las capas caben, pero no junto con `fuera_de_capa`: es el desenlace `Parcial { en_gpu:
    // total, .. }`, el único que el bucle de más abajo no puede alcanzar porque `(1..total)`
    // excluye `n == total` por construcción. Se prueba aquí, entre descartar el modelo entero y
    // bajar a `total - 1`, porque es exactamente ese paso el que faltaba: en la tarjeta en
    // que se midió es el desenlace más probable —cuarenta capas que caben pero no junto a `token_embd`
    // y `output`— y sin este caso se contestaría 39 en vez de 40.
    if coste_de_las_ultimas(pesos, total, kv_por_capa).is_some_and(|c| c <= disponible) {
        return Encaje::Parcial {
            en_gpu: total_u32,
            total: total_u32,
        };
    }

    for n in (1..total).rev() {
        if coste_de_las_ultimas(pesos, n, kv_por_capa).is_some_and(|c| c <= disponible) {
            // `n < total` y `total` cabe en u32: la conversión no falla, pero no se trunca a ciegas.
            return Encaje::Parcial {
                en_gpu: u32::try_from(n).unwrap_or(total_u32),
                total: total_u32,
            };
        }
    }

    Encaje::NadaEnGpu
}

/// Bytes que cuestan las `n` últimas capas, pesos y caché incluidos. `None` si la suma desborda.
fn coste_de_las_ultimas(pesos: &PesoDelModelo, n: usize, kv_por_capa: u64) -> Option<u64> {
    let desde = pesos.por_capa.len().checked_sub(n)?;
    let mut total: u64 = 0;
    for peso in &pesos.por_capa[desde..] {
        total = total.checked_add(*peso)?.checked_add(kv_por_capa)?;
    }
    Some(total)
}

/// Traduce la decisión al número que espera `--n-gpu-layers`.
///
/// El `+ 1` de `Completo` no es un redondeo: es lo que hace que `llama.cpp` suba también la capa de
/// salida, que solo se descarga cuando el número **supera** al de capas del modelo.
pub(super) fn capas_para_llama(encaje: &Encaje, configuradas: u32) -> u32 {
    match encaje {
        Encaje::Completo { total } => total.saturating_add(1),
        Encaje::Parcial { en_gpu, .. } => *en_gpu,
        Encaje::NadaEnGpu => 0,
        Encaje::NoCalculable { .. } => configuradas,
    }
}

/// Cuántas capas caben como mucho según el encaje, o `None` si caben todas (no hay que bajar
/// nada, se pida el número que se pida) o no se pudo calcular.
pub(super) fn capas_que_caben(encaje: &Encaje) -> Option<u32> {
    match encaje {
        Encaje::Completo { .. } | Encaje::NoCalculable { .. } => None,
        Encaje::Parcial { en_gpu, .. } => Some(*en_gpu),
        Encaje::NadaEnGpu => Some(0),
    }
}

/// El coste en bytes de la decisión ya tomada: lo mismo que comparó `decidir` contra `disponible`
/// para llegar a ese resultado, y no un cálculo nuevo. Separado de `decidir` porque esa función es
/// la política pura del reparto y no le hace falta este número para decidir nada; solo lo necesita el
/// aviso para poder decir cuánto pedía el intento que ganó.
pub(super) fn coste_de_la_decision(
    encaje: &Encaje,
    pesos: &PesoDelModelo,
    kv_por_capa: u64,
) -> Option<u64> {
    match encaje {
        Encaje::Completo { total } => coste_de_las_ultimas(pesos, *total as usize, kv_por_capa)
            .and_then(|c| c.checked_add(pesos.fuera_de_capa)),
        Encaje::Parcial { en_gpu, .. } => {
            coste_de_las_ultimas(pesos, *en_gpu as usize, kv_por_capa)
        }
        // Ninguna capa cupo, ni siquiera la última —la más barata de intentar, porque `decidir`
        // prueba de más capas a menos—: es la cifra del aviso «pide tanto y no había ni eso».
        Encaje::NadaEnGpu => coste_de_las_ultimas(pesos, 1, kv_por_capa),
        Encaje::NoCalculable { .. } => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Cuatro capas de 100 bytes cada una y 50 fuera de capa: números pequeños para que las cuentas se
    /// puedan hacer de cabeza al leer la prueba.
    fn pesos_de_juguete() -> PesoDelModelo {
        PesoDelModelo {
            por_capa: vec![100, 100, 100, 100],
            fuera_de_capa: 50,
            inicio_datos: 0,
            fin_datos: 450,
        }
    }

    #[test]
    fn cuando_sobra_vram_cabe_el_modelo_entero() {
        // 4 capas × (100 + 10) + 50 = 490.
        let encaje = decidir(&pesos_de_juguete(), 10, 10_000, 0);
        assert_eq!(encaje, Encaje::Completo { total: 4 });
        assert_eq!(
            capas_para_llama(&encaje, 99),
            5,
            "una más que las capas, que es lo que hace a llama.cpp subir también la salida"
        );
    }

    #[test]
    fn cuando_falta_poco_se_quedan_capas_en_cpu() {
        // 490 no cabe en 400; 3 capas son 3 × 110 = 330, que sí.
        let encaje = decidir(&pesos_de_juguete(), 10, 400, 0);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 3,
                total: 4
            }
        );
        assert_eq!(capas_para_llama(&encaje, 99), 3);
    }

    #[test]
    fn cuando_no_cabe_ni_una_capa_no_se_descarga_ninguna() {
        let encaje = decidir(&pesos_de_juguete(), 10, 50, 0);
        assert_eq!(encaje, Encaje::NadaEnGpu);
        assert_eq!(capas_para_llama(&encaje, 99), 0);
    }

    #[test]
    fn el_margen_se_descuenta_de_lo_disponible() {
        // Con 490 de VRAM cabría entero; con 200 de margen solo caben dos capas.
        let encaje = decidir(&pesos_de_juguete(), 10, 490, 200);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 2,
                total: 4
            }
        );
    }

    #[test]
    fn un_margen_mayor_que_la_vram_no_desborda() {
        let encaje = decidir(&pesos_de_juguete(), 10, 100, 100_000);
        assert_eq!(encaje, Encaje::NadaEnGpu, "saturating, no envoltura");
    }

    // Los dos casos de frontera: «cabe justo» y «cabe una capa menos que justo». Son los que
    // pinzan la comparación `<=` de `decidir`, donde viviría un `<` puesto por descuido: con un
    // margen de sobra o de menos, un `<` en vez de un `<=` no se nota, porque casi ningún número
    // real cae justo en la frontera.

    #[test]
    fn cuando_el_coste_exacto_iguala_lo_disponible_cabe_justo() {
        // El modelo entero pide 4 × (100 + 10) + 50 = 490. Con exactamente 490 libres y margen 0,
        // «cabe» tiene que seguir siendo cierto: es `coste <= disponible`, no `coste < disponible`.
        let encaje = decidir(&pesos_de_juguete(), 10, 490, 0);
        assert_eq!(
            encaje,
            Encaje::Completo { total: 4 },
            "490 libres tienen que bastar para un coste de 490 exactos"
        );
    }

    #[test]
    fn cuando_solo_las_tres_ultimas_igualan_lo_disponible_cabe_una_capa_menos_que_justo() {
        // El modelo entero (490) no cabe en 330. Las tres últimas capas piden exactamente
        // 3 × (100 + 10) = 330: la misma frontera de antes, pero un escalón más abajo, en el
        // bucle que prueba `total - 1` capas.
        let encaje = decidir(&pesos_de_juguete(), 10, 330, 0);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 3,
                total: 4
            },
            "330 libres tienen que bastar para las tres últimas capas, que piden 330 exactos"
        );
    }

    #[test]
    fn cuando_todas_las_capas_caben_pero_no_lo_de_fuera_es_parcial_con_en_gpu_igual_a_total() {
        // Las cuatro capas piden 4 × (100 + 10) = 440 sin contar `fuera_de_capa` (50): con 440
        // libres caben todas, pero el modelo entero —que sí suma esos 50— no (440 + 50 = 490 >
        // 440). El bucle `(1..total).rev()` excluye `n == total` por construcción, así que sin
        // este caso el resultado sería `Parcial { en_gpu: 3, .. }`, una capa menos de las que en
        // realidad caben: es la tarjeta en que se midió, donde las 40 capas de Devstral caben pero
        // sumarles `token_embd` y `output` ya no.
        let encaje = decidir(&pesos_de_juguete(), 10, 440, 0);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 4,
                total: 4
            },
            "las cuatro capas caben; solo lo de fuera de capa no. En_gpu tiene que llegar a 4, \
             no quedarse en 3"
        );
        assert_eq!(
            capas_para_llama(&encaje, 99),
            4,
            "Parcial {{ en_gpu: 4, total: 4 }} tiene que traducirse a 4, no a 5: a diferencia de \
             Completo, aquí no hay «+1» porque la salida no viaja con las capas"
        );
    }

    #[test]
    fn un_modelo_sin_capas_no_es_calculable() {
        let vacio = PesoDelModelo::default();
        let encaje = decidir(&vacio, 10, 10_000, 0);
        assert!(matches!(encaje, Encaje::NoCalculable { .. }));
        assert_eq!(
            capas_para_llama(&encaje, 99),
            99,
            "sin cálculo se respeta lo configurado, que es lo que Programator hacía antes"
        );
    }

    #[test]
    fn se_descargan_las_ultimas_capas_que_son_las_que_llama_pone_en_gpu() {
        // Capas de peso desigual: las últimas son las caras. Si el cálculo sumara las primeras,
        // creería que caben tres.
        let pesos = PesoDelModelo {
            por_capa: vec![10, 10, 1_000, 1_000],
            fuera_de_capa: 0,
            inicio_datos: 0,
            fin_datos: 2_020,
        };
        let encaje = decidir(&pesos, 0, 1_500, 0);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 1,
                total: 4
            }
        );
    }

    #[test]
    fn el_caso_real_de_devstral_en_una_4070_ti_super() {
        // 40 capas de 350 MiB, 1,2 GiB fuera de capa, 85 MiB de caché por capa y 14,93 GiB de
        // presupuesto menos 1 GiB de margen. Comprueba que la decisión es parcial y razonable, no un
        // número exacto: los pesos reales los dará el GGUF cuando se descargue.
        let pesos = PesoDelModelo {
            por_capa: vec![350 * 1024 * 1024; 40],
            fuera_de_capa: 1_200 * 1024 * 1024,
            inicio_datos: 0,
            fin_datos: 0,
        };
        let encaje = decidir(&pesos, 85 * 1024 * 1024, 16_035_872_768, 1024 * 1024 * 1024);
        match encaje {
            Encaje::Parcial { en_gpu, total } => {
                assert_eq!(total, 40);
                assert!(
                    (30..40).contains(&en_gpu),
                    "con estos números deben caber casi todas, pero no todas: {en_gpu}"
                );
            }
            otro => panic!("se esperaba un encaje parcial, y salió {otro:?}"),
        }
    }
}
