//! Las órdenes del banco de modelos (`--banco`, `--banco-informe`, `--banco-comprobar`,
//! `--banco-comparar`) y la que anota una tanda en el registro de modelos (`--registro-anotar`).
//! Un fallo se cuenta y termina el proceso con el código de fallo.

use crate::{ruta_de_la_configuracion, CODIGO_DE_FALLO, VERSION};
use programator::config::Config;
use programator::error::Resultado;
use programator::motor::proceso;
use programator::reloj::RelojReal;
use programator::terminal::TerminalReal;
use std::path::Path;

/// `--banco-informe`: rehace el informe de una tanda ya pasada.
pub(super) fn rehacer_el_informe(resultados: &Path) {
    salir_si_falla_el_banco(programator::banco::tanda::rehacer(
        resultados,
        &TerminalReal,
    ));
}

/// `--banco`: la misma configuración que el ciclo, con el modelo de `--modelo` si se pidió, y el
/// mismo cierre ordenado del motor.
pub(super) fn ejecutar(carpeta: &Path, modelo: Option<&str>) {
    salir_si_falla_el_banco(ejecutar_banco(carpeta, modelo));
}

/// `--banco-comprobar`: solo las pruebas ocultas, sin motor.
pub(super) fn comprobar(tanda: &Path) {
    salir_si_falla_el_banco(comprobar_banco(tanda));
}

/// `--banco-comparar`: dos tandas, sin motor.
pub(super) fn comparar(tanda_a: &Path, tanda_b: &Path) {
    salir_si_falla_el_banco(comparar_tandas(tanda_a, tanda_b));
}

fn salir_si_falla_el_banco(resultado: Resultado<()>) {
    if let Err(fallo) = resultado {
        eprintln!("{}", programator::banco::mensajes::mensaje_fallo(&fallo));
        std::process::exit(CODIGO_DE_FALLO);
    }
}

fn ejecutar_banco(carpeta: &Path, modelo: Option<&str>) -> Resultado<()> {
    let config = Config::cargar(&ruta_de_la_configuracion())?.con_modelo(modelo);
    proceso::registrar_cierre_ordenado()?;
    programator::banco::tanda::ejecutar(
        &config,
        carpeta,
        VERSION,
        &mut RelojReal::desde_ahora(),
        &TerminalReal,
    )
}

/// Del TOML solo hace falta el nombre del agente, para leer su respuesta en el canal de cada
/// pasada.
fn comprobar_banco(tanda: &Path) -> Resultado<()> {
    let config = Config::cargar(&ruta_de_la_configuracion())?;
    programator::banco::tanda::comprobar(&config, tanda, &TerminalReal)
}

/// Del TOML solo hace falta el nombre del agente, para encontrar la conversación de cada pasada.
fn comparar_tandas(tanda_a: &Path, tanda_b: &Path) -> Resultado<()> {
    let config = Config::cargar(&ruta_de_la_configuracion())?;
    programator::banco::comparar::comparar(tanda_a, tanda_b, &config.agente.nombre, &TerminalReal)
}

/// `--registro-anotar`: sin motor ni GPU. Del TOML solo hace falta dónde está el registro y
/// dónde se guardan las revisiones, de cuyas listas sale el origen del modelo.
pub(super) fn anotar_en_el_registro(tanda: &Path) {
    use programator::registro::{anotar_tanda, mensajes};
    use programator::revision::origenes::anotar_con_su_origen;
    let desenlace = Config::cargar(&ruta_de_la_configuracion()).and_then(|config| {
        let registro = config.resolver(&config.agente.registro_de_modelos);
        let revisiones = config.resolver(&config.revision.carpeta);
        anotar_con_su_origen(tanda, &registro, &revisiones).map(|desenlace| (desenlace, registro))
    });
    match desenlace {
        Ok((anotar_tanda::Desenlace::Anotada(anotacion, entrada), registro)) => {
            println!(
                "{}",
                mensajes::mensaje_anotada(&anotacion, &entrada, &registro)
            );
        }
        Ok((anotar_tanda::Desenlace::NoSeAnota(motivo), _)) => {
            eprintln!("{}", mensajes::mensaje_no_se_anota(&motivo, tanda));
            std::process::exit(CODIGO_DE_FALLO);
        }
        Err(fallo) => {
            eprintln!("{}", mensajes::mensaje_fallo(&fallo));
            std::process::exit(CODIGO_DE_FALLO);
        }
    }
}
