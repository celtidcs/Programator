//! El cierre de un encargo que atendió el modelo: la propuesta de poda del buzón de origen, la
//! publicación del desenlace con lo que pasa en el registro de encargos y en el latido, y la
//! liberación de sus veredictos y sus reservas.

use super::pasada::Pasada;
use super::publicacion::{self, propuesta_de_poda_publicada, relativa_a};
use super::respuesta::responder;
use super::sin_atender::{dejar_pendiente, fecha_de_ahora};
use super::{Motores, ResumenPasada};
use crate::arnes::ciclo::Desenlace;
use crate::arnes::herramientas::Repertorio;
use crate::arnes::registro_de_encargos::{EstadoDelEncargo, RegistroDeEncargos};
use crate::protocolo::encargo::Forma;
use crate::protocolo::{
    guardar_propuesta_poda, nombre_del_buzon, proponer_poda_por_cierre, retirar_reserva, Encargo,
    Latido, ResumenLeido,
};
use crate::terminal::Terminal;
use chrono::Local;
use std::collections::HashMap;
use std::path::Path;

/// Vía A: si el encargo concluyó (con entrega o sin entrega directa), propone la poda del buzón de
/// origen retirando la tarea recién cerrada, y devuelve lo que se añade a la respuesta con las
/// rutas escritas. `contenidos_buzones` guarda cómo quedó cada buzón ya podado en este grupo, para
/// que la poda siguiente parta de ahí.
pub(super) fn proponer_la_poda(
    pasada: &Pasada,
    encargo: &Encargo,
    desenlace: &Desenlace,
    contenidos_buzones: &mut HashMap<String, String>,
) -> Option<String> {
    if !admite_poda(encargo, desenlace) {
        return None;
    }
    let carpeta = pasada.carpeta;
    let agente_origen = nombre_del_buzon(&encargo.de);
    let contenido_origen = contenidos_buzones
        .get(&encargo.de)
        .cloned()
        .or_else(|| std::fs::read_to_string(pasada.canal.ruta_del_fichero(&encargo.de)).ok());

    let texto_origen = contenido_origen?;
    let propuesta = proponer_poda_por_cierre(&texto_origen, &encargo.texto)?;
    let (ruta_podado, ruta_archivo) =
        guardar_propuesta_poda(carpeta, &encargo.de, &propuesta).ok()?;
    // Sin reserva: se anotaría y se quitaría en este mismo cierre, porque las
    // reservas de un encargo se borran al terminarlo.
    contenidos_buzones.insert(encargo.de.clone(), propuesta.podado);
    // Las rutas que se escribieron de verdad.
    Some(propuesta_de_poda_publicada(
        agente_origen,
        &relativa_a(carpeta, &ruta_podado),
        &relativa_a(carpeta, &ruta_archivo),
    ))
}

/// Si al cerrar este encargo se propone podar su buzón de origen. Solo con un encargo bajo
/// encabezado: la poda trocea por `---`, y un buzón del gestor de agentes, donde los encargos
/// van en una línea `Para …`, no lo usa; el bloque sería el buzón entero, con los encargos
/// de los demás. Con el gestor de agentes cada uno poda su propia bandeja.
fn admite_poda(encargo: &Encargo, desenlace: &Desenlace) -> bool {
    encargo.forma == Forma::Encabezado
        && matches!(
            desenlace,
            Desenlace::Publicado(_)
                | Desenlace::SinEntrega(_)
                | Desenlace::SinPublicar { .. }
                | Desenlace::Cortado { .. }
        )
}

/// Un encargo que el modelo ya atendió, con lo que hace falta para cerrarlo.
pub(super) struct Atendido<'e> {
    pub(super) encargo: &'e Encargo,
    pub(super) desenlace: &'e Desenlace,
    /// Cuántas propuestas había en el repertorio antes de este encargo: el repertorio es de toda
    /// la pasada, y sin esto no se sabría si fue este encargo el que entregó algo.
    pub(super) propuestas_antes: usize,
    /// Lo leído que dice su respuesta: el de la pasada o, si leyó una cancelación que llegó
    /// mientras se atendía, el que llega hasta ella.
    pub(super) leido: &'e ResumenLeido,
}

/// Publica el desenlace de un encargo y deja la cola y el latido según cómo fue. Lo publicado
/// sale de la cola; lo que no se pudo publicar entra en ella, salvo que sus propuestas ya hubieran
/// salido al canal.
pub(super) fn publicar_el_desenlace(
    pasada: &Pasada,
    motores: &dyn Motores,
    repertorio: &Repertorio,
    atendido: &Atendido,
    cuerpo: &str,
    fichas: &mut RegistroDeEncargos,
    resumen: &mut ResumenPasada,
) {
    let Atendido {
        encargo,
        desenlace,
        propuestas_antes,
        leido,
    } = *atendido;
    let terminal = pasada.ajustes.terminal;
    // Decisión 6: el desenlace sale al canal únicamente por `Canal::publicar`.
    match responder(
        pasada.canal,
        cuerpo,
        leido,
        encargo,
        pasada.recibido_a,
        terminal,
    ) {
        Ok(()) => {
            resumen.atendidos += 1;
            let estado = match desenlace {
                Desenlace::Cancelado { .. } => EstadoDelEncargo::Cancelado,
                _ => EstadoDelEncargo::Atendido,
            };
            super::guardado::terminar_y_guardar(pasada, fichas, encargo, estado, &fecha_de_ahora());
            // Publicada la respuesta, ya no se está atendiendo: antes seguía en «atendiendo»
            // hasta el final de la vuelta. Si hay otro encargo, su latido de atención lo
            // sobrescribe enseguida. Un encargo abortado deja el latido en error, que es lo que
            // tiene que verse.
            if !matches!(
                desenlace,
                Desenlace::Abortado(_) | Desenlace::FueraDePlazo(_)
            ) {
                Latido::en_reposo(Local::now(), pasada.ajustes.intervalo_segundos)
                    .con_modelo_cargado(motores.cargado())
                    .guardar_con_aviso(pasada.ruta_latido, terminal);
            }
        }
        Err(fallo) => {
            // Un fallo atendiendo un encargo no puede tumbar el proceso: se avisa por stderr
            // y se sigue con el siguiente.
            resumen.fallidos += 1;
            let propuestas_de_este_encargo = repertorio.propuestas().len() - propuestas_antes;
            if super::decisiones::fallo_de_cierre_pierde_el_encargo(
                propuestas_de_este_encargo,
                repertorio.veredictos_no_publicados().len(),
            ) {
                // Decisión 5: es preferible responder dos veces que perder un encargo, porque
                // lo primero se ve y lo segundo no. Queda en la cola y se reatiende.
                terminal.avisar(&publicacion::mensaje_fallo_publicar_desenlace(
                    &encargo.de,
                    &fallo,
                ));
                let motivo = publicacion::motivo_sin_publicar(&fallo);
                dejar_pendiente(pasada, fichas, encargo, &motivo, resumen);
            } else {
                // Las propuestas de este encargo ya habían salido al canal al entregarse
                // (`Canal::publicar_entrega`, incidencia C2): lo único que no se pudo publicar
                // es el resumen de cierre. Reatender el encargo solo produciría una segunda
                // respuesta que contradice a la primera (visto en uso real), así que sale de
                // la cola igual que si el cierre hubiera salido bien.
                terminal.avisar(
                    &publicacion::mensaje_fallo_publicar_cierre_con_entrega_ya_registrada(
                        &encargo.de,
                        &fallo,
                    ),
                );
                super::guardado::terminar_y_guardar(
                    pasada,
                    fichas,
                    encargo,
                    EstadoDelEncargo::Atendido,
                    &fecha_de_ahora(),
                );
            }
        }
    }
}

/// El encargo está cerrado, se haya publicado o no: sus veredictos sin publicar no pasan al
/// siguiente, y sus reservas se quitan.
pub(super) fn liberar_el_encargo(pasada: &Pasada, repertorio: &mut Repertorio) {
    repertorio.cerrar_los_veredictos_del_encargo();
    liberar_reservas(
        &pasada.canal.ruta_estado(),
        repertorio.tomar_reservas(),
        pasada.ajustes.terminal,
    );
}

/// Quita de `estado.md` las reservas de un encargo cerrado. Si alguna no se deja quitar, se avisa
/// y se sigue: el encargo ya está atendido.
fn liberar_reservas(ruta_estado: &Path, lineas: Vec<String>, terminal: &dyn Terminal) {
    for linea in lineas {
        if let Err(fallo) = retirar_reserva(ruta_estado, &linea) {
            terminal.avisar(&publicacion::mensaje_fallo_liberar_reserva(&linea, &fallo));
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn encargo(forma: Forma) -> Encargo {
        Encargo {
            latido: None,
            de: "claude.md".to_string(),
            texto: "## Para Programator\nHaz algo.\n".to_string(),
            forma,
        }
    }

    fn cortado() -> Desenlace {
        Desenlace::Cortado {
            texto: String::new(),
            propuestas: Vec::new(),
        }
    }

    /// Un encargo que el motor cortó también concluye: lo que quedó se publica y el encargo no se
    /// repite, así que su buzón de origen se puede podar como el de uno publicado.
    #[test]
    fn un_encargo_cortado_bajo_encabezado_admite_la_poda() {
        assert!(admite_poda(&encargo(Forma::Encabezado), &cortado()));
    }

    #[test]
    fn un_encargo_abortado_o_en_linea_no_admite_la_poda() {
        assert!(!admite_poda(
            &encargo(Forma::Encabezado),
            &Desenlace::Abortado("motivo".to_string())
        ));
        assert!(!admite_poda(&encargo(Forma::Linea), &cortado()));
    }
}
