//! La atención de una vuelta: si toca, dice por qué atiende, pasa por el canal y cuenta en la
//! consola cómo acabó la pasada.

use super::decisiones::{
    estado_tras_la_pasada, mensaje_al_atender, pendiente_tras, toca_atender, Cola, Pendiente,
};
use super::latido::EstadoFinVuelta;
use programator::arnes::sesion::{DesenlacePasada, ResumenPasada};
use programator::arranque::{self, Sondeo};
use programator::error::Resultado;

/// Una vuelta con algo que mirar: si toca, pasa por el canal con `pasar` y cuenta cómo acabó.
pub(super) fn atender_si_toca(
    sondeo: Sondeo,
    aplazado: &mut Pendiente,
    cola: Cola,
    intervalo_segundos: u64,
    pasar: impl FnOnce() -> Resultado<DesenlacePasada>,
) -> EstadoFinVuelta {
    let pendiente = aplazado.con_la_cola(cola);
    if !toca_atender(sondeo, pendiente) {
        return EstadoFinVuelta::Reposo;
    }
    println!("{}", mensaje_al_atender(sondeo, pendiente));

    match pasar() {
        Ok(desenlace) => {
            *aplazado = pendiente_tras(&desenlace);
            imprimir_desenlace(&desenlace, intervalo_segundos);
            estado_tras_la_pasada(&desenlace)
        }
        // Un fallo atendiendo el ciclo no puede tumbar el proceso: se avisa y se reintenta en la
        // vuelta siguiente, igual que un canal ilegible. Lo aplazado se deja como estaba: si había
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
        DesenlacePasada::BuzonAMedias => {
            println!("{}", arranque::mensaje_desenlace_buzon_a_medias());
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
