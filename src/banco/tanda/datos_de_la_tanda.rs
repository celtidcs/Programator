//! Lo que se guarda de una tanda para poder repetirla y compararla, compuesto sin tocar el disco
//! ni el motor: el nombre de su carpeta y sus datos a partir de lo que ya se midió.

use std::path::{Path, PathBuf};

use super::CARPETA_DE_RESULTADOS;
use crate::banco::config::ConfigBanco;
use crate::banco::lugar::LugarElegido;
use crate::banco::resultados::{AjustesDelMotor, DatosDeTanda};
use crate::config::Config;
use crate::motores::preparar::Carga;

/// Cómo se nombra la carpeta de una tanda delante del modelo: ordena por fecha al listarlas y no
/// lleva espacios ni dos puntos, que Windows no admite en un nombre de carpeta.
const FORMATO_DE_LA_CARPETA_DE_LA_TANDA: &str = "%Y-%m-%d-%H%M";

/// Lo que se midió del modelo y del motor antes de pasar los encargos.
pub(super) struct Medido<'a> {
    pub(super) modelo: String,
    pub(super) huella_sha256: String,
    pub(super) compilacion_motor: String,
    /// Lo que costó cargar el motor, si lo arrancó el banco.
    pub(super) carga: Option<&'a Carga>,
}

/// La carpeta de una tanda nueva: en la de resultados del banco, con la fecha delante del modelo.
pub(super) fn carpeta_de_la_tanda(
    carpeta_banco: &Path,
    ahora: &chrono::DateTime<chrono::Local>,
    nombre_modelo: &str,
) -> PathBuf {
    carpeta_banco.join(CARPETA_DE_RESULTADOS).join(format!(
        "{}-{nombre_modelo}",
        ahora.format(FORMATO_DE_LA_CARPETA_DE_LA_TANDA)
    ))
}

/// Los datos de la tanda con lo medido, la configuración del banco y el lugar donde se comprobará.
pub(super) fn datos_de_la_tanda(
    medido: Medido,
    version: &str,
    ahora: &chrono::DateTime<chrono::Local>,
    banco: &ConfigBanco,
    lugar: &LugarElegido,
    config: &Config,
) -> DatosDeTanda {
    let carga = medido.carga;
    DatosDeTanda {
        modelo: medido.modelo,
        huella_sha256: medido.huella_sha256,
        compilacion_motor: medido.compilacion_motor,
        version_programator: version.to_string(),
        fecha: ahora
            .format(crate::protocolo::canal::FORMATO_DE_LA_MARCA_DE_TIEMPO)
            .to_string(),
        semillas: banco.semillas.clone(),
        temperatura: decimal_de_la_configuracion(banco.temperatura),
        margen_lineas: banco.margen_lineas,
        tiempo_maximo_encargo_segundos: banco.tiempo_maximo_encargo_segundos,
        maquina_de_comprobacion: lugar.nombre(),
        capas_en_gpu: carga.map(|c| c.capas),
        vram_ocupada_mib: carga.and_then(|c| c.vram_mib),
        segundos_de_carga: carga.map(|c| c.duracion.as_secs_f64()),
        ajustes_del_motor: Some(ajustes_del_motor(config)),
        tarjeta: carga.and_then(|c| c.tarjeta.clone()),
        sistema_de_comprobacion: lugar.sistema(std::env::consts::OS),
        rastro_fuera: Vec::new(),
        aislamiento: lugar.aislamiento(),
    }
}

/// Un `f32` de la configuración como el `f64` que se escribiría a mano: 0.2, no 0.2000000029.
pub(super) fn decimal_de_la_configuracion(valor: f32) -> f64 {
    valor.to_string().parse().unwrap_or(f64::from(valor))
}

/// Lo de la configuración del motor que se guarda con la tanda para poder compararla.
fn ajustes_del_motor(config: &Config) -> AjustesDelMotor {
    AjustesDelMotor {
        contexto: config.motor.contexto,
        cache_kv: config.motor.cache_kv.clone(),
        paralelo: config.motor.paralelo,
        empezar_en_frio: config.motor.empezar_en_frio,
        top_k: config.muestreo.top_k,
        top_p: decimal_de_la_configuracion(config.muestreo.top_p),
        min_p: decimal_de_la_configuracion(config.muestreo.min_p),
        penalizacion_repeticion: decimal_de_la_configuracion(
            config.muestreo.penalizacion_repeticion,
        ),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn la_temperatura_de_la_configuracion_se_guarda_como_se_escribio() {
        assert_eq!(decimal_de_la_configuracion(0.2), 0.2);
    }

    #[test]
    fn la_carpeta_de_la_tanda_va_en_resultados_con_la_fecha_delante_del_modelo() {
        let ahora = chrono::Local
            .with_ymd_and_hms(2026, 11, 23, 9, 5, 0)
            .unwrap();

        let carpeta = carpeta_de_la_tanda(Path::new("banco"), &ahora, "Devstral");

        assert_eq!(
            carpeta,
            Path::new("banco")
                .join(CARPETA_DE_RESULTADOS)
                .join("2026-11-23-0905-Devstral")
        );
    }
}
