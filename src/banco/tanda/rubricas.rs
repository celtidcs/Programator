//! Los ficheros de rúbrica de una tanda: compone el de cada semilla que se valora leyendo, lo deja
//! en la carpeta de rúbricas y, si ya estaba, recoge la nota que le puso quien dirige.

use std::path::Path;

use super::puntuar::NO_RESPONDIO;
use super::{Pasado, CARPETA_DE_RUBRICAS};
use crate::banco::ficha::NOMBRE_RUBRICA;
use crate::banco::mensajes;
use crate::banco::puntuacion::Nota;
use crate::banco::resultados::{PuntuadaCon, ResultadoDeSemilla};
use crate::banco::rubrica::{
    componer_fichero, componer_fichero_de_codigo, leer_fichero as leer_fichero_de_rubrica,
    nombre_del_fichero,
};
use crate::error::Resultado;
use crate::ficheros::{escribir, leer};

/// Un encargo de código cuyas pruebas ocultas no dieron nota: con rúbrica, deja su fichero con el
/// código entregado para que quien dirige lo valore leyéndolo; sin ella, queda «no evaluado» y el
/// motivo lo dice. `semilla.detalle` trae ya por qué las pruebas no dieron nota.
pub(super) fn valorar_leyendo_el_codigo(
    pasado: &Pasado,
    entregado: &Path,
    carpeta_rubricas: &Path,
    semilla: &mut ResultadoDeSemilla,
) -> Resultado<()> {
    let encargo = pasado.encargo;
    let motivo = semilla.detalle.clone().unwrap_or_default();
    let ruta_rubrica = encargo.carpeta.join(NOMBRE_RUBRICA);
    if !ruta_rubrica.is_file() {
        semilla.detalle = Some(mensajes::motivo_sin_rubrica_de_codigo(&motivo));
        semilla.poner_nota(Nota::NoEvaluado);
        return Ok(());
    }
    let nombre_entrega = encargo.ficha.entrega.as_deref().unwrap_or_default();
    let contenido = componer_fichero_de_codigo(
        &encargo.ficha.id,
        pasado.semilla,
        nombre_entrega,
        &leer(entregado)?,
        &motivo,
        &leer(&ruta_rubrica)?,
    );
    dejar_rubrica(pasado, &contenido, carpeta_rubricas, semilla)?;
    semilla.puntuada_con = Some(PuntuadaCon::Rubrica);
    Ok(())
}

/// Un encargo de criterio: deja la rúbrica para que la puntúe una persona, o recoge la nota si ya
/// está puesta.
pub(super) fn puntuar_con_rubrica(
    pasado: &Pasado,
    respuesta: &str,
    carpeta_rubricas: &Path,
    semilla: &mut ResultadoDeSemilla,
) -> Resultado<()> {
    let encargo = pasado.encargo;
    let rubrica = leer(&encargo.carpeta.join(NOMBRE_RUBRICA))?;
    let respuesta = if pasado.recogida.a_tiempo {
        respuesta
    } else {
        NO_RESPONDIO
    };
    let contenido = componer_fichero(&encargo.ficha.id, pasado.semilla, respuesta, &rubrica);
    dejar_rubrica(pasado, &contenido, carpeta_rubricas, semilla)
}

/// Deja el fichero de rúbrica de esta semilla con `contenido`, o recoge su nota si ya estaba.
fn dejar_rubrica(
    pasado: &Pasado,
    contenido: &str,
    carpeta_rubricas: &Path,
    semilla: &mut ResultadoDeSemilla,
) -> Resultado<()> {
    let nombre = nombre_del_fichero(&pasado.encargo.ficha.id, pasado.semilla);
    let ruta = carpeta_rubricas.join(&nombre);
    // Ya existe si se comprueba otra vez una tanda (`--banco-comprobar`), y puede llevar la nota
    // puesta a mano: se respeta. Lo que se valora es lo mismo, lo de `pasadas.toml`.
    if ruta.is_file() {
        semilla.aplicar_rubrica(leer_fichero_de_rubrica(&ruta)?);
    } else {
        escribir(&ruta, contenido)?;
        semilla.dejar_pendiente();
    }
    semilla.rubrica = Some(format!("{CARPETA_DE_RUBRICAS}/{nombre}"));
    Ok(())
}
