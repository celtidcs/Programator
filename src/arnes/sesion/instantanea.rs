//! La foto inicial del canal: si no hay registro de lectura, decide si lo que ya hay en los
//! buzones ajenos se da por visto o se atiende, y deja el registro guardado. Lo que da por visto
//! queda anotado como visto en el registro de encargos.

use crate::arnes::espacio::{carpeta_de_los_encargos, ruta_del_registro};
use crate::arnes::registro_de_encargos::{disco, EstadoDelEncargo, RegistroDeEncargos};
use crate::error::Resultado;
use crate::protocolo::{detectar, Canal, RegistroLectura};
use std::path::Path;

/// Lo que hizo [`tomar_instantanea_inicial`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instantanea {
    /// Ya había registro de lectura: no hacía falta.
    YaHabia,
    /// Programator ya respondió alguna vez en este canal: lo que hay se da por visto, para no
    /// volver a responder lo ya respondido.
    Tomada,
    /// Programator nunca ha respondido en este canal: todo encargo para él está sin atender, se
    /// publicara antes o después de arrancar, y se atiende.
    SinRespuestasPrevias,
}

/// La foto inicial del canal, si no hay registro de lectura. Se llama al arrancar, antes de decir
/// que se escucha.
///
/// Si la foto se tomara en la primera pasada, que solo corre cuando el canal cambia, en un
/// proyecto nuevo ese cambio sería el primer encargo, y se perdería. Además, tiene que dar igual
/// abrir antes el agente o Programator: un encargo publicado antes de arrancar, en un canal donde
/// Programator nunca ha respondido, no puede estar respondido, así que se atiende.
pub fn tomar_instantanea_inicial(carpeta: &Path, agente: &str) -> Resultado<Instantanea> {
    let ruta_registro = ruta_del_registro(carpeta, agente);
    if ruta_registro.exists() {
        return Ok(Instantanea::YaHabia);
    }
    let canal = Canal::nuevo(carpeta, agente)?;
    let propio = std::fs::read_to_string(canal.ruta_buzon(agente)).unwrap_or_default();
    let carpeta_encargos = carpeta_de_los_encargos(carpeta, agente);
    let mut registro = RegistroLectura::default();
    let instantanea = if ya_respondio(&propio) {
        let mut fichas = RegistroDeEncargos::de_fichas(disco::leer(&carpeta_encargos)?.fichas);
        let fecha = crate::protocolo::marca_de_tiempo_actual();
        for (nombre, contenido) in canal.buzones_ajenos()? {
            registro.delta(&nombre, &contenido);
            for encargo in detectar(agente, &nombre, &contenido) {
                fichas.anotar(&encargo, &fecha, EstadoDelEncargo::Visto);
            }
        }
        disco::guardar_las_cambiadas(&carpeta_encargos, &mut fichas)?;
        Instantanea::Tomada
    } else {
        Instantanea::SinRespuestasPrevias
    };
    // La carpeta de los encargos dice que este canal ya no viene de la 1.1.0: sin ella, la
    // migración daría por visto lo leído.
    crate::ficheros::crear_carpeta(&carpeta_encargos)?;
    registro.guardar(&ruta_registro)?;
    Ok(instantanea)
}

/// Si el buzón propio tiene alguna intervención: toda empieza por su latido, con la marca de hoy
/// o con la antigua (`**LATIDO:**`, anterior a la 0.12).
fn ya_respondio(buzon_propio: &str) -> bool {
    buzon_propio
        .lines()
        .any(|linea| crate::protocolo::es_linea_de_latido(linea.trim_start()))
}
