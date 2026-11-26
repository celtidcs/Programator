//! Dónde se guarda la clave del motor mientras está vivo, para reconocerlo si Programator se cae y
//! vuelve a arrancar con su `llama-server` todavía escuchando.
//!
//! El fichero vive junto a la configuración de la instalación, no en el proyecto: el motor es de la
//! instalación y del puerto, y así la clave nunca acaba en el repositorio de quien encarga.

use super::ClaveDelMotor;
use crate::config::{ClaveConfigurada, Config};
use crate::error::Resultado;
use std::path::PathBuf;

/// El fichero de la clave, relativo a la carpeta de la configuración.
pub const FICHERO_DE_LA_CLAVE: &str = ".clave-del-motor";

/// La ruta del fichero de la clave de esta instalación.
pub fn ruta(config: &Config) -> PathBuf {
    config.resolver(FICHERO_DE_LA_CLAVE)
}

/// La clave con que hay que hablar ahora con el motor del puerto: la fija del TOML o, con
/// `"auto"`, la que se guardó al arrancarlo. `None` si no hay ninguna guardada (o no se deja leer):
/// entonces se pregunta sin clave y, si el motor la pide, se sabrá por su 401.
pub fn vigente(config: &Config) -> Option<ClaveDelMotor> {
    match &config.motor.clave {
        ClaveConfigurada::Fija(texto) => Some(ClaveDelMotor::de_texto(texto)),
        ClaveConfigurada::Auto => crate::ficheros::leer_si_existe(&ruta(config))
            .ok()
            .and_then(|texto| clave_del_texto_guardado(&texto)),
    }
}

/// La clave de un arranque nuevo. Con `"auto"`, una recién inventada, que se guarda antes de lanzar
/// el motor: si se guardara después, una caída entre medias dejaría un motor que nadie reconoce.
pub fn para_arrancar(config: &Config) -> Resultado<ClaveDelMotor> {
    let clave = ClaveDelMotor::nueva(&config.motor.clave)?;
    if config.motor.clave == ClaveConfigurada::Auto {
        crate::ficheros::escribir_solo_para_el_dueno(&ruta(config), clave.texto())?;
    }
    Ok(clave)
}

/// Lo que vale como clave en el texto del fichero: la primera línea, sin espacios. Vacío no vale.
fn clave_del_texto_guardado(texto: &str) -> Option<ClaveDelMotor> {
    let limpio = texto.lines().next().unwrap_or_default().trim();
    (!limpio.is_empty()).then(|| ClaveDelMotor::de_texto(limpio))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn config_en(carpeta: &std::path::Path, clave: &str) -> Config {
        let ruta = carpeta.join("programator.toml");
        std::fs::write(
            &ruta,
            format!("[motor]\nmodelo = \"m.gguf\"\nclave = \"{clave}\"\n"),
        )
        .unwrap();
        Config::cargar(&ruta).unwrap()
    }

    #[test]
    fn con_auto_la_clave_del_arranque_se_guarda_y_es_la_vigente() {
        let carpeta = tempfile::tempdir().unwrap();
        let config = config_en(carpeta.path(), "auto");
        assert_eq!(vigente(&config), None);

        let clave = para_arrancar(&config).unwrap();

        assert_eq!(vigente(&config), Some(clave.clone()));
        let otra = para_arrancar(&config).unwrap();
        assert_ne!(otra, clave, "cada arranque inventa una nueva");
        assert_eq!(vigente(&config), Some(otra));
    }

    /// La clave va en el entorno y no en la línea de órdenes para que otro usuario del equipo no
    /// la vea, y en Unix el fichero que la guarda tampoco puede leerlo nadie más.
    #[cfg(unix)]
    #[test]
    fn en_unix_el_fichero_de_la_clave_solo_lo_lee_su_dueno() {
        use std::os::unix::fs::PermissionsExt;
        let carpeta = tempfile::tempdir().unwrap();
        let config = config_en(carpeta.path(), "auto");

        para_arrancar(&config).unwrap();

        let modo = std::fs::metadata(ruta(&config))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(modo & 0o777, 0o600, "{modo:o}");
    }

    #[test]
    fn con_una_fija_no_se_guarda_nada() {
        let carpeta = tempfile::tempdir().unwrap();
        let config = config_en(carpeta.path(), "mia");

        let clave = para_arrancar(&config).unwrap();

        assert_eq!(clave.texto(), "mia");
        assert_eq!(vigente(&config), Some(ClaveDelMotor::de_texto("mia")));
        assert!(!ruta(&config).exists());
    }

    #[test]
    fn un_fichero_vacio_o_con_salto_final_se_lee_bien() {
        assert_eq!(clave_del_texto_guardado(""), None);
        assert_eq!(clave_del_texto_guardado("  \n"), None);
        assert_eq!(
            clave_del_texto_guardado("abc\r\n"),
            Some(ClaveDelMotor::de_texto("abc"))
        );
    }
}
