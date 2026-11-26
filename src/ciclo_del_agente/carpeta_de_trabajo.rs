//! La carpeta de trabajo del ciclo: se decide por la línea de órdenes, el TOML, la recordada o,
//! si no hay ninguna, el diálogo; se comprueba que existe y se recuerda.

use crate::directorio_del_ejecutable;
use programator::argumentos::Orden;
use programator::arranque;
use programator::carpeta;
use programator::config::Config;
use programator::error::{Error, Resultado};
use programator::terminal::TerminalReal;
use std::path::PathBuf;

/// La carpeta de trabajo del ciclo. Tres fuentes por delante del diálogo, para que un reinicio no
/// dependa de que haya una persona mirando. Se recuerda en cuanto se sabe que es buena, y no al
/// terminar: un ciclo que vive días no puede esperar a su propio final para dejar anotado dónde
/// trabajaba.
pub(super) fn resolver_la_carpeta(orden: &Orden, config: &Config) -> Resultado<PathBuf> {
    let pedida_en_la_linea = match orden {
        Orden::Ejecutar { ruta } => ruta.clone(),
        _ => None,
    };
    let junto_al_toml = directorio_del_ejecutable();
    let recordada = carpeta::recordada(&junto_al_toml);
    let carpeta = match carpeta::decidir(
        pedida_en_la_linea.as_deref(),
        config.carpeta.ruta.as_deref(),
        recordada.as_deref(),
        carpeta::Memoria::segun(config.carpeta.preguntar_siempre),
    ) {
        Some((ruta, origen)) => {
            if !ruta.is_dir() {
                return Err(Error::ConfiguracionDeLaCarpeta(
                    arranque::mensaje_carpeta_inexistente(&ruta, origen.descripcion()),
                ));
            }
            println!(
                "{}",
                arranque::mensaje_carpeta_de_trabajo(&ruta, origen.descripcion())
            );
            ruta
        }
        None => elegir_en_el_dialogo()?,
    };
    carpeta::recordar(&junto_al_toml, &carpeta, &TerminalReal);
    Ok(carpeta)
}

/// Sin ninguna de las tres fuentes, la carpeta la elige una persona en el diálogo.
fn elegir_en_el_dialogo() -> Resultado<PathBuf> {
    let ruta = rfd::FileDialog::new()
        .set_title(arranque::TITULO_DEL_DIALOGO_DE_CARPETA)
        .pick_folder()
        .ok_or_else(|| Error::ConfiguracionDeLaCarpeta(arranque::mensaje_sin_carpeta_elegida()))?;
    println!(
        "{}",
        arranque::mensaje_carpeta_de_trabajo(&ruta, arranque::ORIGEN_VENTANA_EMERGENTE)
    );
    Ok(ruta)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una carpeta pedida que no existe es un fallo de la carpeta, no del motor ni de la
    /// configuración en general: de eso depende el consejo que acompaña al fallo de arranque.
    #[test]
    fn una_carpeta_que_no_existe_es_un_fallo_de_la_carpeta() {
        let carpeta = tempfile::tempdir().unwrap();
        let toml = carpeta.path().join("programator.toml");
        std::fs::write(&toml, "[motor]\nmodelo = \"m.gguf\"\n").unwrap();
        let config = Config::cargar(&toml).unwrap();
        let orden = Orden::Ejecutar {
            ruta: Some(carpeta.path().join("no-existe").display().to_string()),
        };

        let fallo = resolver_la_carpeta(&orden, &config).unwrap_err();

        assert!(
            matches!(fallo, Error::ConfiguracionDeLaCarpeta(_)),
            "{fallo:?}"
        );
        assert!(
            fallo.to_string().starts_with("configuración inválida: "),
            "{fallo}"
        );
    }
}
