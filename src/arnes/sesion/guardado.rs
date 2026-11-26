//! Lo que cambia en el registro de encargos durante la atención va al disco en ese momento.
//!
//! Si solo se guardara al final de la pasada, un Programator que muere entre una respuesta
//! publicada y ese final (ventana cerrada, Administrador de tareas, apagado) volvería a atender el
//! encargo al arrancar: se vio en la prueba manual O, con tres respuestas al mismo encargo.

use super::pasada::Pasada;
use super::publicacion;
use crate::arnes::espacio::carpeta_de_los_encargos;
use crate::arnes::registro_de_encargos::{disco, EstadoDelEncargo, RegistroDeEncargos};
use crate::protocolo::Encargo;

/// Cierra la ficha del encargo con `estado` y la guarda en el acto.
pub(super) fn terminar_y_guardar(
    pasada: &Pasada,
    fichas: &mut RegistroDeEncargos,
    encargo: &Encargo,
    estado: EstadoDelEncargo,
    fecha: &str,
) {
    fichas.terminar(encargo, estado, fecha);
    guardar_al_momento(pasada, fichas);
}

/// Guarda las fichas que han cambiado. Si no se puede, lo dice en la terminal y la pasada sigue:
/// el final de la pasada vuelve a intentarlo.
pub(super) fn guardar_al_momento(pasada: &Pasada, fichas: &mut RegistroDeEncargos) {
    let carpeta = carpeta_de_los_encargos(pasada.carpeta, pasada.agente);
    if let Err(fallo) = disco::guardar_las_cambiadas(&carpeta, fichas) {
        pasada
            .ajustes
            .terminal
            .avisar(&publicacion::mensaje_registro_de_encargos_sin_guardar(
                &fallo,
            ));
    }
}
