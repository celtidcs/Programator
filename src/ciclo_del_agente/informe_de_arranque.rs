//! El informe de arranque: reúne lo medido de la máquina, lo cuenta en la consola y guarda lo que
//! hace falta después para el historial de encaje.

use crate::VERSION;
use programator::argumentos::Orden;
use programator::arranque;
use programator::carpeta;
use programator::config::Config;
use programator::diagnostico::reunir_datos_de_arranque;
use programator::informe::{componer_informe, DatosDeArranque, ResumenModelo};
use programator::motor::hardware::Gpu;
use programator::terminal::TerminalReal;
use std::path::Path;

/// Lo medido al arrancar que hace falta después, cuando ya se conoce la carpeta de trabajo, para
/// anotar el historial de encaje.
pub(super) struct EncajeDelArranque {
    pub(super) gpu: Option<Gpu>,
    /// Las capas que se pedirán a `llama-server`. `None` sin modelo declarado.
    pub(super) capas_en_gpu: Option<u32>,
    pub(super) capas_totales: Option<u32>,
}

impl EncajeDelArranque {
    /// El nombre de la tarjeta medida al arrancar; `None` si no se pudo medir.
    pub(super) fn nombre_de_la_tarjeta(&self) -> Option<&str> {
        self.gpu.as_ref().map(|gpu| gpu.nombre.as_str())
    }
}

/// La carpeta que nombra el informe: la pedida con `--ruta`, que manda, o la de `[carpeta] ruta`.
/// Antes solo miraba el TOML y, con `--ruta`, decía «NO FIJA» (prueba con la tarjeta, 22/11/2026).
fn carpeta_para_el_informe<'a>(orden: &'a Orden, del_toml: Option<&'a str>) -> Option<&'a Path> {
    let pedida = match orden {
        Orden::Ejecutar { ruta } => ruta.as_deref(),
        _ => None,
    };
    pedida.or(del_toml).map(Path::new)
}

/// Las capas que tiene el modelo pesado, si caben en un `u32`; `None` si no se pesó.
fn capas_totales_del_modelo(modelo: &ResumenModelo) -> Option<u32> {
    match modelo {
        ResumenModelo::Pesado { capas, .. } => capas.and_then(|c| u32::try_from(c).ok()),
        _ => None,
    }
}

/// Compone e imprime el informe de arranque.
///
/// El informe se compone **antes** de resolver la carpeta de trabajo, y a propósito no la
/// necesita resuelta: `--diagnostico` existe justo para poder preguntar «¿esto cómo está hoy?»
/// sin dejar un proceso colgado, que es lo que pasaba cuando el diálogo de carpeta se abría
/// primero. Si `[carpeta] ruta` no está fijada, la última fila lo dice y ya; los otros cuatro
/// datos no dependen de la carpeta para nada. Va por `stderr`, como el resto de los avisos
/// desde la 0.4.0. La fila de carpeta va última a propósito (2026-10-01): si el diálogo decide
/// la carpeta más abajo, la línea que lo confirma se imprime justo después, sin nada entre
/// medias.
pub(super) fn informar_del_arranque(config: &Config, orden: &Orden) -> EncajeDelArranque {
    let datos = reunir_datos_de_arranque(config, &TerminalReal);
    // Capturado antes de que `componer_informe` se lleve `datos.modelo` por valor: lo necesita el
    // registro del historial de encaje, una vez se conoce la carpeta de trabajo.
    let capas_totales = capas_totales_del_modelo(&datos.modelo);
    let informe = componer_informe(&DatosDeArranque {
        version: VERSION,
        carpeta: carpeta_para_el_informe(orden, config.carpeta.ruta.as_deref()),
        gpu: datos.gpu.as_ref(),
        motor: &datos.motor,
        ruta_motor: config.motor.binario.as_deref(),
        modelo: datos.modelo,
        encaje: &datos.encaje,
        memoria: carpeta::Memoria::segun(config.carpeta.preguntar_siempre),
    });
    eprintln!("{}", arranque::bloque(&informe));
    EncajeDelArranque {
        gpu: datos.gpu,
        capas_en_gpu: datos.capas_en_gpu,
        capas_totales,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Con --ruta, el informe no dice «NO FIJA».
    #[test]
    fn el_informe_nombra_la_carpeta_pedida_con_ruta() {
        let pedida = Orden::Ejecutar {
            ruta: Some("M:/proyecto".into()),
        };
        let sin = Orden::Ejecutar { ruta: None };

        assert_eq!(
            carpeta_para_el_informe(&pedida, Some("M:/otra")),
            Some(Path::new("M:/proyecto"))
        );
        assert_eq!(
            carpeta_para_el_informe(&sin, Some("M:/otra")),
            Some(Path::new("M:/otra"))
        );
        assert_eq!(carpeta_para_el_informe(&sin, None), None);
    }
}
