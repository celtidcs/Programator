//! Los encargos que no llegan a atenderse con el modelo: pasan a la cola de pendientes (o suman
//! un intento), o se cierran respondiendo con el motivo a quien los pidió.

use super::pasada::Pasada;
use super::publicacion;
use super::respuesta::responder;
use super::ResumenPasada;
use crate::arnes::pendientes::Cola;
use crate::protocolo::Encargo;
use chrono::Local;

/// Un encargo sin modelo con el que atenderlo: se responde en el acto con el motivo y cómo
/// conseguir uno, y sale de la cola. Si la respuesta no se publica, queda pendiente.
pub(super) fn cerrar_sin_modelo(
    pasada: &Pasada,
    cola: &mut Cola,
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
            cola.quitar(encargo);
        }
        Err(fallo) => dejar_pendiente(
            pasada,
            cola,
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
    cola: &mut Cola,
    encargo: &Encargo,
    motivo: &str,
    resumen: &mut ResumenPasada,
) {
    let fecha = Local::now()
        .format(crate::protocolo::canal::FORMATO_DE_LA_MARCA_DE_TIEMPO)
        .to_string();
    let cierre = cola
        .anotar_fallo(encargo, &fecha, motivo, pasada.ajustes.intentos_por_encargo)
        .map(|p| publicacion::aviso_encargo_cerrado(p.intentos(), p.motivo()));
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
            cola.quitar(encargo);
            terminal.avisar(&publicacion::mensaje_encargo_cerrado(&encargo.de, motivo));
        }
        Err(fallo) => {
            resumen.pendientes += 1;
            terminal.avisar(&publicacion::mensaje_fallo_publicar_aviso_pendiente(&fallo));
        }
    }
}
