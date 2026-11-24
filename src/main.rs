//! Programator: arnés que convierte un modelo local en un agente del canal.
//!
//! El ejecutable solo reparte: las órdenes que no arrancan el ciclo las atiende
//! [`ordenes_sin_ciclo`], y el ciclo del agente (arranque, preparación de la carpeta y vigilancia
//! del canal) lo lleva [`ciclo_del_agente`]. Sus módulos son del ejecutable, no de la biblioteca:
//! cuentan en la terminal lo que deciden los módulos de `programator`.

mod ciclo_del_agente;
mod ordenes_sin_ciclo;

use programator::argumentos;
use programator::arranque;
use programator::config::NOMBRE_DEL_FICHERO_DE_CONFIGURACION;
use std::path::PathBuf;

/// La versión del paquete, para no escribirla a mano en dos sitios.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// El código de salida de una orden mal escrita: el 2 que usan las herramientas de línea de
/// órdenes para «uso incorrecto», distinto del 1 de un fallo al ejecutar.
const CODIGO_DE_USO_INCORRECTO: i32 = 2;

/// El código de salida de una orden que falló al ejecutarse: el 1 de fallo general, la convención
/// de las herramientas de línea de órdenes que acompaña al 2 de arriba.
const CODIGO_DE_FALLO: i32 = 1;

fn main() {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let Some(orden) = ordenes_sin_ciclo::atender(argumentos::interpretar(&argumentos)) else {
        return;
    };
    if let Err(fallo) = ciclo_del_agente::ejecutar(orden) {
        eprintln!("{}", arranque::mensaje_fallo_arranque(&fallo));
        std::process::exit(CODIGO_DE_FALLO);
    }
}

/// `programator.toml`, junto al ejecutable: el único sitio donde se busca.
fn ruta_de_la_configuracion() -> PathBuf {
    directorio_del_ejecutable().join(NOMBRE_DEL_FICHERO_DE_CONFIGURACION)
}

fn directorio_del_ejecutable() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}
