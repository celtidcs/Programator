//! El lugar de este equipo donde se ejecutarán las pruebas ocultas: elige el intérprete que se
//! usará (y su copia, si comprueba con integridad baja) y prueba antes de cargar el modelo que la
//! carpeta de comprobación se puede preparar.

use std::path::Path;

use super::CARPETA_DE_RESULTADOS;
use crate::banco::comprobacion::lanzador::LanzadorLocal;
use crate::banco::comprobacion::prueba_previa::probar_el_lanzador;
use crate::banco::comprobacion::{copia_del_interprete, interprete};
use crate::banco::config::ComprobacionLocal;
use crate::banco::lugar::LugarElegido;
use crate::error::Resultado;
use crate::terminal::Terminal;

/// Si se van a ejecutar pruebas ocultas, deja lista antes de cargar el modelo la copia de Git, si
/// hace falta, y prueba que la carpeta de comprobación se puede preparar en la de resultados del
/// banco.
pub(super) fn probar_el_lugar(
    lugar: &LugarElegido,
    carpeta_banco: &Path,
    terminal: &dyn Terminal,
) -> Resultado<()> {
    match lugar {
        LugarElegido::Ninguno => Ok(()),
        LugarElegido::Local(local) => {
            interprete_de_este_equipo(local, carpeta_banco, terminal)?;
            probar_el_lanzador(
                &mut LanzadorLocal::para(local),
                &carpeta_banco.join(CARPETA_DE_RESULTADOS),
            )
        }
    }
}

/// `local` con el intérprete que se usará en este equipo: el que encuentre junto a Git y, si
/// comprueba con integridad baja, el de la copia. Dice en la terminal lo que haya que avisar.
pub(super) fn interprete_de_este_equipo(
    local: &ComprobacionLocal,
    carpeta_banco: &Path,
    terminal: &dyn Terminal,
) -> Resultado<ComprobacionLocal> {
    let (local, aviso) = interprete::en_este_equipo(local);
    let (local, aviso_de_la_copia) =
        copia_del_interprete::con_la_copia(&local, carpeta_banco, terminal)?;
    for aviso in [aviso, aviso_de_la_copia].into_iter().flatten() {
        terminal.decir(&aviso);
    }
    Ok(local)
}
