//! La atención de una vuelta: si toca, dice por qué atiende, pasa por el canal y cuenta en la
//! consola cómo acabó la pasada.

use super::decisiones::{estado_tras_la_pasada, mensaje_al_atender, toca_atender};
use super::latido::EstadoFinVuelta;
use programator::arnes::sesion::{DesenlacePasada, ResumenPasada};
use programator::arranque::{self, Sondeo};
use programator::error::Resultado;

/// Una vuelta con algo que mirar: si toca, pasa por el canal con `pasar` y cuenta cómo acabó.
pub(super) fn atender_si_toca(
    sondeo: Sondeo,
    pendiente_por_motor: &mut bool,
    encargos_en_cola: bool,
    intervalo_segundos: u64,
    pasar: impl FnOnce() -> Resultado<DesenlacePasada>,
) -> EstadoFinVuelta {
    if !toca_atender(sondeo, *pendiente_por_motor || encargos_en_cola) {
        return EstadoFinVuelta::Reposo;
    }
    println!("{}", mensaje_al_atender(sondeo, *pendiente_por_motor));

    match pasar() {
        Ok(desenlace) => {
            // Si el motor sigue cargando, el encargo no se ha atendido y hay que volver por él
            // aunque el canal no vuelva a moverse.
            *pendiente_por_motor = matches!(desenlace, DesenlacePasada::MotorCargando);
            imprimir_desenlace(&desenlace, intervalo_segundos);
            estado_tras_la_pasada(&desenlace)
        }
        // Un fallo atendiendo el ciclo no puede tumbar el proceso: se avisa y se reintenta en la
        // vuelta siguiente, igual que un canal ilegible. La bandera se deja como estaba: si había
        // trabajo pendiente, lo sigue habiendo.
        Err(fallo) => {
            eprintln!("{}", arranque::mensaje_fallo_pasada_ciclo(&fallo));
            EstadoFinVuelta::Error(fallo.to_string())
        }
    }
}

/// El bucle decide qué imprimir a partir del desenlace de la pasada: `ejecutar_pasada` solo
/// informa.
fn imprimir_desenlace(desenlace: &DesenlacePasada, intervalo_segundos: u64) {
    match desenlace {
        DesenlacePasada::PrimeraPasada => {
            println!(
                "{}",
                arranque::mensaje_listo_primer_arranque(intervalo_segundos)
            );
        }
        DesenlacePasada::SinEncargos => {
            println!("{}", arranque::mensaje_desenlace_sin_encargos());
        }
        DesenlacePasada::MotorCargando => {
            println!("{}", arranque::mensaje_desenlace_motor_cargando());
        }
        DesenlacePasada::Atendidos(ResumenPasada {
            atendidos,
            fallidos,
            pendientes,
            ..
        }) => {
            println!(
                "{}",
                arranque::mensaje_desenlace_atendidos(*atendidos, *fallidos)
            );
            if *pendientes > 0 {
                println!("{}", arranque::mensaje_encargos_en_cola(*pendientes));
            }
        }
    }
}
