//! El historial de encaje: una línea por arranque con las capas que se pidieron a la tarjeta,
//! anotada en el espacio del agente en cuanto se conoce la carpeta de trabajo.

use super::informe_de_arranque::EncajeDelArranque;
use crate::VERSION;
use programator::arnes::espacio;
use programator::config::Config;
use programator::motor::encaje_historico;
use std::path::Path;

/// Una línea más al historial de encaje (visto en uso real), ahora que la carpeta de trabajo
/// ya se conoce. `reunir_datos_de_arranque` corre antes de resolverla a propósito (para que
/// `--diagnostico` funcione sin ella), así que este registro va aquí y no allí. Sin modelo
/// declarado no hay nada que registrar; un fallo al escribir solo avisa, nunca aborta.
pub(super) fn registrar_el_encaje(carpeta: &Path, config: &Config, encaje: &EncajeDelArranque) {
    let Some(capas_en_gpu) = encaje.capas_en_gpu else {
        return;
    };
    let marca_tiempo = programator::protocolo::marca_de_tiempo_actual();
    let linea = encaje_historico::linea_historico(&encaje_historico::DatosDeEncaje {
        marca_tiempo: &marca_tiempo,
        gpu: encaje.gpu.as_ref(),
        contexto: config.motor.contexto,
        capas_en_gpu,
        capas_totales: encaje.capas_totales,
        version: VERSION,
    });
    let ruta = espacio::ruta_del_encaje_historico(carpeta, &config.agente.nombre);
    if let Err(fallo) = encaje_historico::registrar(&ruta, &linea) {
        eprintln!(
            "{}",
            encaje_historico::mensaje_fallo_registrar(&ruta, &fallo)
        );
    }
}
