//! Los encargos que no llegan a atenderse con el modelo: quedan pendientes en el registro de
//! encargos (o suman un intento), o se cierran respondiendo con el motivo a quien los pidió.

use super::pasada::Pasada;
use super::publicacion;
use super::respuesta::responder;
use super::ResumenPasada;
use crate::arnes::registro_de_encargos::{EstadoDelEncargo, RegistroDeEncargos};
use crate::protocolo::Encargo;
use chrono::Local;

/// Un encargo sin modelo con el que atenderlo: se responde en el acto con el motivo y cómo
/// conseguir uno, y sale de la cola. Si la respuesta no se publica, queda pendiente.
pub(super) fn cerrar_sin_modelo(
    pasada: &Pasada,
    fichas: &mut RegistroDeEncargos,
    encargo: &Encargo,
    motivo: &str,
    resumen: &mut ResumenPasada,
) {
    let terminal = pasada.ajustes.terminal;
    terminal.avisar(&publicacion::mensaje_encargo_sin_modelo(
        &encargo.de,
        motivo,
    ));
    match responder(
        pasada.canal,
        &publicacion::aviso_sin_modelo(motivo),
        pasada.leido,
        encargo,
        pasada.recibido_a,
        terminal,
    ) {
        Ok(()) => {
            super::guardado::terminar_y_guardar(
                pasada,
                fichas,
                encargo,
                EstadoDelEncargo::Atendido,
                &fecha_de_ahora(),
            );
        }
        Err(fallo) => dejar_pendiente(
            pasada,
            fichas,
            encargo,
            &publicacion::motivo_sin_publicar(&fallo),
            resumen,
        ),
    }
}

/// Mete el encargo en la cola de pendientes (o le suma un intento). Al llegar a los intentos
/// que se admiten, se cierra: se responde a quien lo pidió con el motivo y sale de la cola. Si esa
/// respuesta no se publica, sigue en la cola y se vuelve a intentar cerrar en la vuelta siguiente.
pub(super) fn dejar_pendiente(
    pasada: &Pasada,
    fichas: &mut RegistroDeEncargos,
    encargo: &Encargo,
    motivo: &str,
    resumen: &mut ResumenPasada,
) {
    let fecha = fecha_de_ahora();
    let cierre = fichas
        .anotar_fallo(encargo, &fecha, motivo, pasada.ajustes.intentos_por_encargo)
        .map(|ficha| publicacion::aviso_encargo_cerrado(ficha.intentos, &ficha.motivo));
    super::guardado::guardar_al_momento(pasada, fichas);
    let terminal = pasada.ajustes.terminal;
    let Some(cierre) = cierre else {
        resumen.pendientes += 1;
        terminal.avisar(&publicacion::mensaje_encargo_queda_pendiente(
            &encargo.de,
            motivo,
        ));
        return;
    };
    match responder(
        pasada.canal,
        &cierre,
        pasada.leido,
        encargo,
        pasada.recibido_a,
        terminal,
    ) {
        Ok(()) => {
            super::guardado::terminar_y_guardar(
                pasada,
                fichas,
                encargo,
                EstadoDelEncargo::Cerrado,
                &fecha,
            );
            terminal.avisar(&publicacion::mensaje_encargo_cerrado(&encargo.de, motivo));
        }
        Err(fallo) => {
            resumen.pendientes += 1;
            terminal.avisar(&publicacion::mensaje_fallo_publicar_aviso_pendiente(&fallo));
        }
    }
}

/// La fecha de ahora, como la marca de tiempo del canal: la de las fichas del registro.
pub(super) fn fecha_de_ahora() -> String {
    Local::now()
        .format(crate::protocolo::canal::FORMATO_DE_LA_MARCA_DE_TIEMPO)
        .to_string()
}
