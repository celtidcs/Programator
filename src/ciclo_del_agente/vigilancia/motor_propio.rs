//! El motor que Programator arrancó por su cuenta, visto desde el bucle: qué modelo tiene cargado
//! y cuándo se suelta por llevar tiempo sin encargos (`[motor] soltar_modelo`).

use super::decisiones::Pendiente;
use programator::config::Config;
use programator::motor::fichero_de_modelo::nombre_de_modelo;
use programator::motor::mensajes::mensaje_modelo_soltado;
use programator::motor::proceso;
use programator::motor::soltar::{motor_para_soltar, toca_soltar, MotorPropio};
// Segundos de un minuto, para pasar el tiempo sin encargos a los minutos de `soltar_modelo`.
use programator::unidades::SEGUNDOS_POR_MINUTO;
use std::cell::RefCell;

/// El modelo del motor propio, por el nombre de su fichero; `None` si no hay propio o si su
/// proceso ya terminó, para que el latido no nombre un modelo que el motor no llegó a cargar.
pub(super) fn modelo_cargado(servidor: &RefCell<Option<proceso::ServidorLocal>>) -> Option<String> {
    servidor
        .borrow()
        .as_ref()
        .filter(|propio| propio.sigue_vivo())
        .map(|propio| {
            nombre_de_modelo(propio.modelo())
                .unwrap_or_else(|| propio.modelo().display().to_string())
        })
}

/// Suelta el motor propio si toca (`[motor] soltar_modelo`). Con algo `pendiente` (aplazado o en la
/// cola) no se suelta.
pub(super) fn soltar_si_toca(
    config: &Config,
    servidor: &RefCell<Option<proceso::ServidorLocal>>,
    ultimo_encargo: std::time::Instant,
    pendiente: Pendiente,
) {
    let motor = motor_para_soltar(MotorPropio {
        hay: servidor.borrow().is_some(),
        ocupado: pendiente != Pendiente::Nada,
    });
    let minutos = ultimo_encargo.elapsed().as_secs() / SEGUNDOS_POR_MINUTO;
    if !toca_soltar(config.motor.soltar_modelo, minutos, motor) {
        return;
    }
    let modelo = modelo_cargado(servidor).unwrap_or_default();
    // Al soltarlo se para el proceso y se espera a que salga: así queda libre la tarjeta.
    drop(servidor.borrow_mut().take());
    println!("{}", mensaje_modelo_soltado(&modelo, minutos));
}
