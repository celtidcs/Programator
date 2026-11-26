//! Por qué no se pudo verificar una propuesta, tal como se cuenta. `verificacion` decide; aquí solo
//! se redacta.

use std::fmt::Display;
use std::time::Duration;

/// El comprobador de esa extensión está en la configuración, pero sin orden.
pub(super) fn comprobador_vacio(extension: &str) -> String {
    format!("el comprobador de «.{extension}» está configurado vacío")
}

/// El comprobador no se pudo lanzar.
pub(super) fn no_se_lanzo(programa: &str, causa: &dyn Display) -> String {
    format!("no se pudo lanzar «{programa}»: {causa}")
}

/// El comprobador pasó del tiempo máximo y se cortó.
pub(super) fn cortado(programa: &str, tope: Duration) -> String {
    format!(
        "«{programa}» seguía trabajando tras {} segundos y se ha cortado",
        tope.as_secs()
    )
}

/// No se pudo esperar a que el comprobador terminara.
pub(super) fn sin_esperar(programa: &str, causa: &dyn Display) -> String {
    format!("no se pudo esperar a «{programa}»: {causa}")
}

/// La salida del comprobador no se pudo leer.
pub(super) fn salida_ilegible(programa: &str, causa: &dyn Display) -> String {
    format!("no se pudo leer la salida de «{programa}»: {causa}")
}

/// El comprobador rechazó sin escribir nada por ninguna de sus dos salidas.
pub(super) fn termino_sin_decir_nada(estado: &dyn Display) -> String {
    format!("terminó con {estado} y sin decir nada")
}
