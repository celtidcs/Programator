//! La compilación de `llama-server` que se guarda con la tanda: la pregunta con `--version` y saca
//! de lo que contesta la línea que la dice.

use std::path::Path;
use std::time::Duration;

use crate::orden_acotada::{ejecutar_con_tope, DesenlaceDeOrden};

/// Lo que puede tardar `llama-server --version`. En el equipo de desarrollo (b10993 CUDA 12.4,
/// Windows) contesta en unos 150 ms, y la primera vez tras arrancar el equipo tardó 5,8 s.
/// Treinta segundos lo cubren con margen sin dejar colgado el banco. Es
/// un tope de seguridad, no un ajuste: por eso no está en el TOML.
const TIEMPO_MAXIMO_VERSION_MOTOR: Duration = Duration::from_secs(30);
/// Lo que se guarda como compilación del motor si no se puede preguntar.
pub(super) const COMPILACION_DESCONOCIDA: &str = "desconocida";

/// La compilación de `llama-server`, tal como la cuenta `--version`.
pub(super) fn compilacion_del_motor(binario: Option<&Path>) -> String {
    let Some(binario) = binario else {
        return COMPILACION_DESCONOCIDA.into();
    };
    let directorio = binario.parent().unwrap_or_else(|| Path::new("."));
    let programa = binario.display().to_string();
    match ejecutar_con_tope(
        &programa,
        &["--version".to_string()],
        directorio,
        TIEMPO_MAXIMO_VERSION_MOTOR,
    ) {
        DesenlaceDeOrden::Terminada {
            salida, errores, ..
        } => compilacion_en_la_salida(&salida, &errores),
        _ => COMPILACION_DESCONOCIDA.into(),
    }
}

/// La línea de lo que contestó `--version`, por cualquiera de sus dos salidas, que dice la
/// versión; si ninguna la dice, la compilación es desconocida.
fn compilacion_en_la_salida(salida: &str, errores: &str) -> String {
    format!("{salida}\n{errores}")
        .lines()
        .find(|l| l.contains("version"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| COMPILACION_DESCONOCIDA.into())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_compilacion_es_la_linea_que_dice_la_version_en_cualquiera_de_las_dos_salidas() {
        assert_eq!(
            compilacion_en_la_salida("", "ggml_cuda_init: 1 GPU\n  version: 10993 (abc)  \n"),
            "version: 10993 (abc)"
        );
        assert_eq!(
            compilacion_en_la_salida("version: 1 (x)\n", ""),
            "version: 1 (x)"
        );
        assert_eq!(
            compilacion_en_la_salida("nada\n", "tampoco\n"),
            COMPILACION_DESCONOCIDA
        );
    }
}
