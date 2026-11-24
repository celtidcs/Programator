//! Lo que cumple la entrega de un encargo de código: cuenta los requisitos de texto y, si hay
//! dónde, monta la carpeta de pruebas junto a la de trabajo y lanza en ella las pruebas ocultas.

use std::path::{Path, PathBuf};

use super::{Pasado, Pruebas};
use crate::banco::comprobacion::{montar_carpeta_de_pruebas, Comprobado, ResultadoDePruebas};
use crate::banco::mensajes;
use crate::banco::puntuacion::HechosDeComprobaciones;
use crate::banco::resultados::ResultadoDeSemilla;
use crate::error::Resultado;
use crate::ficheros::{borrar_si_existe, leer};
use crate::terminal::Terminal;

/// Detrás del nombre de la carpeta de un encargo, la de sus pruebas con la entrega colocada.
const SUFIJO_DE_PRUEBAS: &str = "pruebas";

/// Lo que cumple una entrega: los requisitos de texto y, si hay dónde, las pruebas ocultas.
pub(super) fn comprobar_la_entrega(
    pasado: &Pasado,
    ruta: &Path,
    pruebas: Option<&mut Pruebas>,
    semilla: &mut ResultadoDeSemilla,
    terminal: &dyn Terminal,
) -> Resultado<HechosDeComprobaciones> {
    let encargo = pasado.encargo;
    let ficha = &encargo.ficha;
    let contenido = leer(ruta)?;
    let cumplidos = ficha
        .texto
        .iter()
        .filter(|r| r.se_cumple_en(&contenido))
        .count();
    let comprobado = match pruebas {
        None => Comprobado::from(ResultadoDePruebas::NoEvaluado(
            mensajes::motivo_pruebas_ocultas_apagadas(),
        )),
        Some(pruebas) => {
            let montada = carpeta_de_pruebas(&pasado.carpeta);
            // Solo existe si ya se intentó comprobar esta tanda: se monta de nuevo para que no
            // quede nada del intento anterior.
            borrar_si_existe(&montada)?;
            montar_carpeta_de_pruebas(encargo, ruta, &montada)?;
            pruebas(encargo, &montada)
        }
    };
    let resultado = comprobado.resultado;
    if !comprobado.intentos_de_red.is_empty() {
        terminal.avisar(&mensajes::mensaje_intento_de_red(
            &ficha.id,
            pasado.semilla,
            &comprobado.intentos_de_red,
        ));
    }
    semilla.intentos_de_red = comprobado.intentos_de_red;
    match &resultado {
        ResultadoDePruebas::NoEvaluado(motivo) | ResultadoDePruebas::NoCompila(motivo) => {
            semilla.detalle = Some(motivo.clone())
        }
        ResultadoDePruebas::Compila { .. } => {}
    }
    Ok(resultado.hechos(cumplidos, ficha.texto.len()))
}

/// La carpeta de pruebas montadas de un encargo, junto a su carpeta de trabajo.
fn carpeta_de_pruebas(carpeta_del_encargo: &Path) -> PathBuf {
    let nombre = carpeta_del_encargo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    carpeta_del_encargo.with_file_name(format!("{nombre}-{SUFIJO_DE_PRUEBAS}"))
}
