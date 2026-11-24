//! La configuración del ciclo: se carga del TOML, se cuentan sus avisos de claves retiradas y se
//! completa el comprobador de Python si el TOML no lo trae.

use programator::argumentos::Orden;
use programator::arranque;
use programator::config::Config;
use programator::error::Resultado;
use std::path::Path;

/// La configuración, con el modelo de `--diagnostico --modelo` si se pidió, y sus avisos de claves
/// retiradas impresos.
pub(super) fn cargar_la_configuracion(ruta_config: &Path, orden: &Orden) -> Resultado<Config> {
    let mut config = Config::cargar(ruta_config)?.con_modelo(modelo_pedido(orden));
    for aviso in config.avisos() {
        eprintln!("{}", arranque::bloque(aviso));
    }
    completar_el_comprobador_de_python(&mut config);
    Ok(config)
}

/// El modelo pedido en la línea de órdenes: solo `--diagnostico --modelo` lo cambia.
fn modelo_pedido(orden: &Orden) -> Option<&str> {
    match orden {
        Orden::Diagnostico { modelo } => modelo.as_deref(),
        _ => None,
    }
}

/// Si no hay comprobador de `.py` en el TOML, busca un Python que responda y lo pone. Antes de
/// componer las normas y la guía, que cuentan los comprobadores.
pub(super) fn completar_el_comprobador_de_python(config: &mut Config) {
    let instalacion = config.carpeta_de_instalacion().to_path_buf();
    let tiempo = std::time::Duration::from_secs(config.verificacion.tiempo_maximo_segundos);
    let python =
        programator::arnes::verificacion::python::completar(&mut config.verificacion, |programa| {
            matches!(
                programator::orden_acotada::ejecutar_con_tope(
                    programa,
                    &["--version".to_owned()],
                    &instalacion,
                    tiempo,
                ),
                programator::orden_acotada::DesenlaceDeOrden::Terminada { exito: true, .. }
            )
        });
    if let Some(aviso) = programator::arnes::verificacion::python::mensaje(&python) {
        eprintln!("{}", arranque::bloque(&aviso));
    }
}
