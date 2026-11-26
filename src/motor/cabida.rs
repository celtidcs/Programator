//! Si un modelo cabe en la tarjeta lo bastante para trabajar con él.
//!
//! Lógica pura sobre el [`Encaje`]: no abre ficheros ni mide la tarjeta.

use std::num::NonZeroU32;

use super::encaje::Encaje;

/// Lo mínimo de las capas de un modelo que tiene que quedar en la tarjeta para que cuente como que
/// cabe: tres cuartos. Origen: el Manual de uso, medido con Devstral en una tarjeta de 16 GB: con
/// 39 de 40 capas, 23 tokens por segundo; con 35, 12; con 30 (tres cuartos), 7; por debajo, un
/// encargo de minutos pasa a ser de muchos minutos y ya no es usable.
pub const PROPORCION_MINIMA_DE_CAPAS_EN_TARJETA: Proporcion = Proporcion {
    numerador: 3,
    // `new(4)` nunca es `None`; el `match` es solo porque una constante no puede usar `unwrap`.
    denominador: match NonZeroU32::new(4) {
        Some(cuatro) => cuatro,
        None => NonZeroU32::MIN,
    },
};

/// Una fracción de las capas del modelo. El denominador nunca es cero, y lo dice el tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Proporcion {
    pub numerador: u32,
    pub denominador: NonZeroU32,
}

impl Proporcion {
    /// Si `parte` de `total` llega a esta proporción. Sin desbordes: se compara en 64 bits.
    pub fn alcanzada(self, parte: u32, total: u32) -> bool {
        u64::from(parte) * u64::from(self.denominador.get())
            >= u64::from(total) * u64::from(self.numerador)
    }

    /// Cuántas capas de `total` hacen falta como mínimo, redondeando hacia arriba.
    pub fn minimo_de(self, total: u32) -> u32 {
        let necesarias = (u64::from(total) * u64::from(self.numerador))
            .div_ceil(u64::from(self.denominador.get()));
        u32::try_from(necesarias).unwrap_or(total)
    }
}

/// Cuánto de un modelo cabe en la tarjeta, visto para decidir si se puede trabajar con él.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cabida {
    /// Entero, con la capa de salida.
    Entero,
    /// En parte, por encima de la proporción mínima.
    EnParte { en_tarjeta: u32, total: u32 },
    /// En parte, por debajo de la proporción mínima: sería demasiado lento.
    Insuficiente { en_tarjeta: u32, total: u32 },
    /// Ni una capa.
    Nada,
    /// No se pudo calcular, con el motivo.
    NoSeSabe(String),
}

impl Cabida {
    /// Si se puede trabajar con el modelo en esta tarjeta.
    pub fn cabe(&self) -> bool {
        matches!(self, Cabida::Entero | Cabida::EnParte { .. })
    }
}

/// La cabida que corresponde a un encaje.
pub fn cabida(encaje: &Encaje) -> Cabida {
    match encaje {
        Encaje::Completo { .. } => Cabida::Entero,
        Encaje::Parcial { en_gpu, total }
            if PROPORCION_MINIMA_DE_CAPAS_EN_TARJETA.alcanzada(*en_gpu, *total) =>
        {
            Cabida::EnParte {
                en_tarjeta: *en_gpu,
                total: *total,
            }
        }
        Encaje::Parcial { en_gpu, total } => Cabida::Insuficiente {
            en_tarjeta: *en_gpu,
            total: *total,
        },
        Encaje::NadaEnGpu => Cabida::Nada,
        Encaje::NoCalculable { motivo } => Cabida::NoSeSabe(motivo.clone()),
    }
}

/// Si el fichero del modelo está en el disco. Lo mira quien llama: este módulo no abre ficheros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FicheroDelModelo {
    Esta,
    Falta,
}

/// Por qué no se puede trabajar con un modelo, si se sabe antes de lanzar el motor: falta su
/// fichero, o no cabe en la tarjeta. `None` si se puede, o si no se sabe (sin tarjeta medida, o
/// sin poder calcular el encaje): en la duda se intenta.
pub fn por_que_no_se_puede_usar(
    modelo: &std::path::Path,
    fichero: FicheroDelModelo,
    cabida: Option<&Cabida>,
) -> Option<String> {
    if fichero == FicheroDelModelo::Falta {
        return Some(super::mensajes::falta_el_fichero_del_modelo(modelo));
    }
    match cabida {
        Some(Cabida::Insuficiente { en_tarjeta, total }) => {
            Some(motivo_no_cabe(modelo, Some((*en_tarjeta, *total))))
        }
        Some(Cabida::Nada) => Some(motivo_no_cabe(modelo, None)),
        _ => None,
    }
}

fn motivo_no_cabe(modelo: &std::path::Path, capas: Option<(u32, u32)>) -> String {
    let nombre = modelo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| modelo.display().to_string());
    super::mensajes::modelo_que_no_cabe(&nombre, capas)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_modelo_que_falta_o_no_cabe_no_se_puede_usar_y_en_la_duda_si() {
        const ESTA: FicheroDelModelo = FicheroDelModelo::Esta;
        let ruta = std::path::Path::new("modelos/m.gguf");
        assert!(
            por_que_no_se_puede_usar(ruta, FicheroDelModelo::Falta, None)
                .unwrap()
                .contains("falta el fichero")
        );
        assert!(por_que_no_se_puede_usar(ruta, ESTA, Some(&parcial(10, 40)))
            .unwrap()
            .contains("10 de sus 40"));
        assert!(por_que_no_se_puede_usar(ruta, ESTA, Some(&Cabida::Nada)).is_some());
        assert_eq!(
            por_que_no_se_puede_usar(ruta, ESTA, Some(&parcial(30, 40))),
            None
        );
        assert_eq!(
            por_que_no_se_puede_usar(ruta, ESTA, Some(&Cabida::Entero)),
            None
        );
        assert_eq!(
            por_que_no_se_puede_usar(ruta, ESTA, Some(&Cabida::NoSeSabe("x".into()))),
            None
        );
        assert_eq!(por_que_no_se_puede_usar(ruta, ESTA, None), None);
    }

    fn parcial(en_gpu: u32, total: u32) -> Cabida {
        cabida(&Encaje::Parcial { en_gpu, total })
    }

    #[test]
    fn entero_y_desde_tres_cuartos_cabe_y_por_debajo_no() {
        assert_eq!(cabida(&Encaje::Completo { total: 40 }), Cabida::Entero);
        assert!(parcial(40, 40).cabe());
        assert!(parcial(30, 40).cabe());
        assert_eq!(
            parcial(29, 40),
            Cabida::Insuficiente {
                en_tarjeta: 29,
                total: 40
            }
        );
        assert!(!parcial(29, 40).cabe());
        assert!(!cabida(&Encaje::NadaEnGpu).cabe());
        assert!(!cabida(&Encaje::NoCalculable {
            motivo: "x".to_string()
        })
        .cabe());
    }

    #[test]
    fn la_proporcion_redondea_hacia_arriba_y_no_desborda() {
        assert_eq!(PROPORCION_MINIMA_DE_CAPAS_EN_TARJETA.minimo_de(40), 30);
        assert_eq!(PROPORCION_MINIMA_DE_CAPAS_EN_TARJETA.minimo_de(41), 31);
        assert!(PROPORCION_MINIMA_DE_CAPAS_EN_TARJETA.alcanzada(u32::MAX, u32::MAX));
        assert!(!PROPORCION_MINIMA_DE_CAPAS_EN_TARJETA.alcanzada(30, 41));
        assert!(PROPORCION_MINIMA_DE_CAPAS_EN_TARJETA.alcanzada(31, 41));
    }
}
