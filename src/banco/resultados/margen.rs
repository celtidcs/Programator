//! El margen de VRAM que hace falta en la máquina donde se pasó la tanda: lo que ocupó el motor de
//! verdad menos lo que calculaba el encaje. Aritmética pura; la redacción va en `redaccion`.

/// La recomendación de `margen_vram_mib` se redondea hacia arriba a un múltiplo de esto: una
/// cifra redonda que se escribe fácil en el TOML y cubre el error de medida.
pub(crate) const MULTIPLO_DEL_MARGEN_MIB: u64 = 128;

/// Lo medido y lo que se recomienda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MargenMedido {
    /// Lo que ocupó de más el motor sobre lo calculado, en MiB. Negativo si ocupó menos.
    pub(crate) medido_mib: i64,
    /// El `margen_vram_mib` que cubre lo medido: el múltiplo de [`MULTIPLO_DEL_MARGEN_MIB`] que
    /// queda por encima, y nunca menos de uno, porque la memoria libre cambia entre la medida y la
    /// carga aunque el cálculo haya acertado.
    pub(crate) recomendado_mib: u64,
}

/// El margen de una carga, si se midió lo que ocupó y se pudo calcular lo que debía ocupar.
pub(crate) fn margen_medido(
    ocupada_mib: Option<u64>,
    calculada_mib: Option<u64>,
) -> Option<MargenMedido> {
    let ocupada = i64::try_from(ocupada_mib?).ok()?;
    let calculada = i64::try_from(calculada_mib?).ok()?;
    let medido_mib = ocupada.checked_sub(calculada)?;
    let de_mas = u64::try_from(medido_mib).unwrap_or(0);
    let recomendado_mib = de_mas
        .div_ceil(MULTIPLO_DEL_MARGEN_MIB)
        .max(1)
        .saturating_mul(MULTIPLO_DEL_MARGEN_MIB);
    Some(MargenMedido {
        medido_mib,
        recomendado_mib,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_margen_es_lo_ocupado_menos_lo_calculado_y_se_redondea_hacia_arriba() {
        assert_eq!(
            margen_medido(Some(13_763), Some(13_300)),
            Some(MargenMedido {
                medido_mib: 463,
                recomendado_mib: 512
            })
        );
        assert_eq!(
            margen_medido(Some(1_256), Some(1_000)).map(|m| m.recomendado_mib),
            Some(256),
            "256 justos son dos múltiplos, no tres"
        );
        assert_eq!(
            margen_medido(Some(1_257), Some(1_000)).map(|m| m.recomendado_mib),
            Some(384)
        );
    }

    #[test]
    fn si_ocupa_menos_de_lo_calculado_se_recomienda_el_minimo() {
        assert_eq!(
            margen_medido(Some(900), Some(1_000)),
            Some(MargenMedido {
                medido_mib: -100,
                recomendado_mib: MULTIPLO_DEL_MARGEN_MIB
            })
        );
        assert_eq!(
            margen_medido(Some(1_000), Some(1_000)).map(|m| m.recomendado_mib),
            Some(MULTIPLO_DEL_MARGEN_MIB)
        );
    }

    #[test]
    fn sin_alguna_de_las_dos_cifras_no_hay_margen() {
        assert_eq!(margen_medido(None, Some(1)), None);
        assert_eq!(margen_medido(Some(1), None), None);
    }
}
