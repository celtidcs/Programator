//! Cuándo sobra la copia de un `preparar:`: en la pasada
//! siguiente a que el agente diga que aceptó o rechazó la propuesta, o pasados
//! `[preparacion] dias_de_vida`. Cada copia lleva una marca con la propuesta y la fecha.

use crate::arnes::entregas::{EstadoDeLaPropuesta, RegistroDeEntregas};
use crate::error::{Error, Resultado};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// El fichero de la marca, en la carpeta de cada copia.
pub const FICHERO_DE_LA_MARCA: &str = "preparacion.toml";

/// El formato de [`Marca::creada`]: el del latido, con segundos.
pub const FORMATO_DE_LA_FECHA: &str = crate::protocolo::canal::FORMATO_DE_LA_HORA_DEL_LATIDO;

/// De qué propuesta es una copia y cuándo se hizo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marca {
    pub propuesta: String,
    pub creada: String,
}

/// Si la copia con esta marca sobra. Sin marca, o con una que no se entiende, sobra: es lo que
/// quedó de una preparación cortada, y lo de dentro no lo pidió nadie.
pub fn sobra(
    marca: Option<&Marca>,
    registro: &RegistroDeEntregas,
    ahora: NaiveDateTime,
    dias_de_vida: u64,
) -> bool {
    let Some(marca) = marca else {
        return true;
    };
    let Ok(creada) = NaiveDateTime::parse_from_str(&marca.creada, FORMATO_DE_LA_FECHA) else {
        return true;
    };
    let vida = i64::try_from(dias_de_vida)
        .ok()
        .and_then(chrono::Duration::try_days)
        .unwrap_or(chrono::Duration::MAX);
    if ahora.signed_duration_since(creada) >= vida {
        return true;
    }
    registro
        .entregas
        .iter()
        .rev()
        .find(|entrada| entrada.propuesta == marca.propuesta)
        .is_some_and(|entrada| {
            matches!(
                entrada.estado,
                EstadoDeLaPropuesta::Aceptada | EstadoDeLaPropuesta::Rechazada
            )
        })
}

/// Escribe la marca de la copia que vive en `carpeta`.
pub fn marcar(carpeta: &Path, marca: &Marca) -> Resultado<()> {
    let texto = toml::to_string(marca).map_err(|causa| Error::Configuracion(causa.to_string()))?;
    crate::ficheros::escribir(&carpeta.join(FICHERO_DE_LA_MARCA), &texto)
}

/// Borra de `carpeta_de_las_copias` las copias que sobran y devuelve sus nombres. Lo que no es
/// una carpeta se deja.
pub fn limpiar(
    carpeta_de_las_copias: &Path,
    registro: &RegistroDeEntregas,
    ahora: NaiveDateTime,
    dias_de_vida: u64,
) -> Resultado<Vec<String>> {
    if !carpeta_de_las_copias.is_dir() {
        return Ok(Vec::new());
    }
    let error_de_lectura = |causa| Error::Lectura {
        ruta: carpeta_de_las_copias.to_path_buf(),
        causa,
    };
    let mut borradas = Vec::new();
    for entrada in std::fs::read_dir(carpeta_de_las_copias).map_err(error_de_lectura)? {
        let ruta = entrada.map_err(error_de_lectura)?.path();
        if !ruta.is_dir() {
            continue;
        }
        let marca = std::fs::read_to_string(ruta.join(FICHERO_DE_LA_MARCA))
            .ok()
            .and_then(|texto| toml::from_str::<Marca>(&texto).ok());
        if sobra(marca.as_ref(), registro, ahora, dias_de_vida) {
            crate::ficheros::borrar_si_existe(&ruta)?;
            if let Some(nombre) = ruta.file_name() {
                borradas.push(nombre.to_string_lossy().into_owned());
            }
        }
    }
    borradas.sort();
    Ok(borradas)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::arnes::entregas::{
        huella, EntradaDeEntrega, VeredictoDeLaEntrega, LO_DIO_PROGRAMATOR,
    };

    fn hora(texto: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(texto, FORMATO_DE_LA_FECHA).unwrap()
    }

    fn marca(propuesta: &str) -> Marca {
        Marca {
            propuesta: propuesta.into(),
            creada: "2026-11-25 10:00:00".into(),
        }
    }

    fn registro_con(estados: &[(&str, EstadoDeLaPropuesta)]) -> RegistroDeEntregas {
        RegistroDeEntregas {
            entregas: estados
                .iter()
                .map(|(propuesta, estado)| EntradaDeEntrega {
                    propuesta: propuesta.to_string(),
                    encargo: "id".to_string(),
                    nombre_del_encargo: "n".to_string(),
                    de: "claude.md".to_string(),
                    modelo: None,
                    fecha: "2026-11-25 10:00".to_string(),
                    veredicto: VeredictoDeLaEntrega::Pasa,
                    huella: huella("x"),
                    estado: *estado,
                    estado_lo_dio: LO_DIO_PROGRAMATOR.to_string(),
                    destino: None,
                    motivo: None,
                    archivada: None,
                    ya_en_el_proyecto: Vec::new(),
                })
                .collect(),
            ..RegistroDeEntregas::default()
        }
    }

    /// F5: la copia caduca a los días de vida.
    #[test]
    fn una_copia_sobra_pasados_sus_dias() {
        let registro = RegistroDeEntregas::default();
        let m = marca("x.rs");
        assert!(!sobra(Some(&m), &registro, hora("2026-11-27 09:59:59"), 2));
        assert!(sobra(Some(&m), &registro, hora("2026-11-27 10:00:00"), 2));
    }

    /// F5: y en cuanto el agente dice que aceptó o rechazó la propuesta; la última entrada con ese
    /// nombre es la que cuenta.
    #[test]
    fn una_copia_sobra_cuando_su_propuesta_se_resuelve() {
        let ahora = hora("2026-11-25 11:00:00");
        let m = marca("x.rs");
        for (estado, sobra_ya) in [
            (EstadoDeLaPropuesta::Propuesta, false),
            (EstadoDeLaPropuesta::Aceptada, true),
            (EstadoDeLaPropuesta::Rechazada, true),
            (EstadoDeLaPropuesta::Sustituida, false),
            (EstadoDeLaPropuesta::SinConfirmar, false),
        ] {
            let registro =
                registro_con(&[("otra.rs", EstadoDeLaPropuesta::Aceptada), ("x.rs", estado)]);
            assert_eq!(sobra(Some(&m), &registro, ahora, 2), sobra_ya, "{estado:?}");
        }
        let registro = registro_con(&[
            ("x.rs", EstadoDeLaPropuesta::Aceptada),
            ("x.rs", EstadoDeLaPropuesta::Propuesta),
        ]);
        assert!(!sobra(Some(&m), &registro, ahora, 2));
    }

    #[test]
    fn sin_marca_o_con_una_rota_sobra() {
        let registro = RegistroDeEntregas::default();
        let ahora = hora("2026-11-25 11:00:00");
        assert!(sobra(None, &registro, ahora, 2));
        let rota = Marca {
            creada: "ayer".into(),
            ..marca("x.rs")
        };
        assert!(sobra(Some(&rota), &registro, ahora, 2));
    }

    #[test]
    fn limpiar_borra_solo_las_copias_que_sobran() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let carpeta = carpeta_temporal.path();
        for (nombre, propuesta) in [("viva", "x.rs"), ("resuelta", "y.rs")] {
            std::fs::create_dir_all(carpeta.join(nombre).join("proyecto")).unwrap();
            marcar(&carpeta.join(nombre), &marca(propuesta)).unwrap();
        }
        std::fs::create_dir(carpeta.join("sin-marca")).unwrap();
        std::fs::write(carpeta.join("suelto.txt"), "x").unwrap();
        let registro = registro_con(&[("y.rs", EstadoDeLaPropuesta::Rechazada)]);

        let borradas = limpiar(carpeta, &registro, hora("2026-11-25 11:00:00"), 2).unwrap();

        assert_eq!(borradas, ["resuelta", "sin-marca"]);
        assert!(carpeta.join("viva/proyecto").is_dir());
        assert!(carpeta.join("suelto.txt").exists());
        assert!(limpiar(
            &carpeta.join("no-existe"),
            &registro,
            hora("2026-11-25 11:00:00"),
            2
        )
        .unwrap()
        .is_empty());
    }
}
