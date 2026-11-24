//! El vigilante del canal de Gestor Agentes en el proyecto, para que un agente sepa cuándo ha
//! respondido Programator, trabaje con Gestor Agentes o sin él.
//!
//! Hay un solo vigilante, `gestor-vigilante`, que funciona solo con `.gestor/canal/`. Programator
//! lo lleva en su paquete y lo deja en `.gestor/vigilante/` del proyecto **solo si no está**:
//! nunca reemplaza el que haya. En los proyectos con Gestor Agentes manda el suyo, que Gestor
//! sobrescribe en cada lanzamiento. La guía dice lo mismo en los dos casos.
//!
//! [`decidir`] es puro; [`instalar`] copia.

use crate::error::{Error, Resultado};
use crate::plataforma::NOMBRE_DEL_VIGILANTE;
use crate::protocolo::carpetas::{carpeta_del_gestor, CARPETA_DEL_GESTOR};
use std::path::{Path, PathBuf};

/// La carpeta del vigilante dentro de `.gestor/`, la misma en la que lo deja Gestor Agentes
/// (`crates/nucleo/src/proyecto/vigilante.rs`): de ella deduce dónde está `.gestor/`.
pub const CARPETA_DEL_VIGILANTE: &str = "vigilante";

/// Cómo se le dice a un agente que lance el vigilante. Sin extensión: así lo escribe también
/// Gestor Agentes, y en Windows se encuentra igual desde PowerShell y desde bash.
const ORDEN_DEL_VIGILANTE: &str = "gestor-vigilante --hasta-cambio --agente";

/// Lo que hay que hacer con el vigilante del proyecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Ya hay uno: no se toca.
    YaEsta,
    /// No hay ninguno y el paquete trae el suyo: se copia.
    Copiar,
    /// No hay ninguno y el paquete no lo trae.
    FaltaEnElPaquete,
}

/// Qué vigilantes existen. Con nombre, para que no se puedan cruzar como dos `bool` seguidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hallados {
    pub en_el_paquete: bool,
    pub en_el_proyecto: bool,
}

/// Qué hacer, según si existe el vigilante del paquete y el del proyecto.
pub fn decidir(hallados: Hallados) -> Decision {
    match (hallados.en_el_proyecto, hallados.en_el_paquete) {
        (true, _) => Decision::YaEsta,
        (false, true) => Decision::Copiar,
        (false, false) => Decision::FaltaEnElPaquete,
    }
}

/// `<carpeta>/.gestor/vigilante/gestor-vigilante(.exe)`.
pub fn ruta_en_el_proyecto(carpeta: &Path) -> PathBuf {
    carpeta_del_gestor(carpeta)
        .join(CARPETA_DEL_VIGILANTE)
        .join(NOMBRE_DEL_VIGILANTE)
}

/// Deja el vigilante `del_paquete` en el proyecto si no hay ninguno. Devuelve lo que decidió.
pub fn instalar(del_paquete: &Path, carpeta: &Path) -> Resultado<Decision> {
    let destino = ruta_en_el_proyecto(carpeta);
    let decision = decidir(Hallados {
        en_el_paquete: del_paquete.is_file(),
        en_el_proyecto: destino.exists(),
    });
    if decision != Decision::Copiar {
        return Ok(decision);
    }
    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre).map_err(|causa| Error::Escritura {
            ruta: padre.to_path_buf(),
            causa,
        })?;
    }
    std::fs::copy(del_paquete, &destino).map_err(|causa| Error::Escritura {
        ruta: destino.clone(),
        causa,
    })?;
    hacer_ejecutable(&destino)?;
    Ok(decision)
}

/// Fuera de Windows, la copia tiene que poder ejecutarse.
#[cfg(unix)]
fn hacer_ejecutable(ruta: &Path) -> Resultado<()> {
    use std::os::unix::fs::PermissionsExt;
    /// Lectura y ejecución para todos, escritura para el dueño: los permisos de un ejecutable.
    const PERMISOS_DE_EJECUTABLE: u32 = 0o755;
    std::fs::set_permissions(
        ruta,
        std::fs::Permissions::from_mode(PERMISOS_DE_EJECUTABLE),
    )
    .map_err(|causa| Error::Escritura {
        ruta: ruta.to_path_buf(),
        causa,
    })
}

#[cfg(not(unix))]
fn hacer_ejecutable(_ruta: &Path) -> Resultado<()> {
    Ok(())
}

/// La orden que lanza el agente, en segundo plano, tras encargar algo.
pub fn orden_para_el_agente() -> String {
    format!("{CARPETA_DEL_GESTOR}/{CARPETA_DEL_VIGILANTE}/{ORDEN_DEL_VIGILANTE} \"<tu identidad>\"")
}

/// Lo que se dice en la terminal tras mirar el vigilante. `None` si ya estaba: no hay nada que
/// contar.
pub fn mensaje(decision: Decision, carpeta: &Path) -> Option<String> {
    match decision {
        Decision::YaEsta => None,
        Decision::Copiar => Some(format!(
            "Dejado el vigilante del canal en {}: avisa al agente cuando Programator responde.",
            ruta_en_el_proyecto(carpeta).display()
        )),
        Decision::FaltaEnElPaquete => Some(
            "⚠️ Este paquete no trae el vigilante del canal (gestor-vigilante) y el proyecto no \
             tiene uno: el agente que encarga tendrá que mirar el canal por su cuenta para saber \
             cuándo responde Programator."
                .to_string(),
        ),
    }
}

/// Lo que se dice si no se pudo dejar el vigilante en el proyecto.
pub fn mensaje_de_fallo(fallo: &Error) -> String {
    format!("⚠️ No se pudo dejar el vigilante del canal en el proyecto: {fallo}.")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_se_copia_si_el_proyecto_no_tiene_y_el_paquete_si() {
        assert_eq!(
            decidir(Hallados {
                en_el_paquete: true,
                en_el_proyecto: false
            }),
            Decision::Copiar
        );
        assert_eq!(
            decidir(Hallados {
                en_el_paquete: true,
                en_el_proyecto: true
            }),
            Decision::YaEsta
        );
        assert_eq!(
            decidir(Hallados {
                en_el_paquete: false,
                en_el_proyecto: true
            }),
            Decision::YaEsta
        );
        assert_eq!(
            decidir(Hallados {
                en_el_paquete: false,
                en_el_proyecto: false
            }),
            Decision::FaltaEnElPaquete
        );
    }

    #[test]
    fn se_copia_cuando_falta_y_no_se_toca_el_que_hay() {
        let paquete = tempfile::tempdir().unwrap();
        let proyecto = tempfile::tempdir().unwrap();
        let del_paquete = paquete.path().join(NOMBRE_DEL_VIGILANTE);
        std::fs::write(&del_paquete, "nuestro").unwrap();

        assert_eq!(
            instalar(&del_paquete, proyecto.path()).unwrap(),
            Decision::Copiar
        );
        let destino = ruta_en_el_proyecto(proyecto.path());
        assert_eq!(std::fs::read_to_string(&destino).unwrap(), "nuestro");

        std::fs::write(&destino, "el de Gestor").unwrap();
        assert_eq!(
            instalar(&del_paquete, proyecto.path()).unwrap(),
            Decision::YaEsta
        );
        assert_eq!(std::fs::read_to_string(&destino).unwrap(), "el de Gestor");
    }

    #[test]
    fn sin_vigilante_en_el_paquete_se_avisa_y_no_se_crea_nada() {
        let proyecto = tempfile::tempdir().unwrap();

        let decision = instalar(Path::new("no-existe"), proyecto.path()).unwrap();

        assert_eq!(decision, Decision::FaltaEnElPaquete);
        assert!(!proyecto.path().join(CARPETA_DEL_GESTOR).exists());
        assert!(mensaje(decision, proyecto.path())
            .unwrap()
            .contains("gestor-vigilante"));
        assert_eq!(mensaje(Decision::YaEsta, proyecto.path()), None);
    }

    #[cfg(unix)]
    #[test]
    fn la_copia_se_puede_ejecutar() {
        use std::os::unix::fs::PermissionsExt;
        let paquete = tempfile::tempdir().unwrap();
        let proyecto = tempfile::tempdir().unwrap();
        let del_paquete = paquete.path().join(NOMBRE_DEL_VIGILANTE);
        std::fs::write(&del_paquete, "x").unwrap();

        instalar(&del_paquete, proyecto.path()).unwrap();

        let modo = std::fs::metadata(ruta_en_el_proyecto(proyecto.path()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(modo & 0o111, 0o111);
    }

    #[test]
    fn la_guia_pide_la_cabecera_que_lee_programator() {
        let guia = std::fs::read_to_string("plantillas/como-encargar-a-programator.md").unwrap();
        let cabecera = crate::protocolo::respuesta::CABECERA_DE_BANDEJA;
        assert!(guia.contains(&format!("`{cabecera}<tu identidad>`")));
    }

    #[test]
    fn la_guia_da_la_misma_orden() {
        let guia = std::fs::read_to_string("plantillas/como-encargar-a-programator.md").unwrap();
        assert!(
            guia.contains(&orden_para_el_agente()),
            "la guía no da la orden del vigilante"
        );
    }

    #[test]
    fn la_orden_es_la_misma_que_da_gestor_agentes() {
        assert_eq!(
            orden_para_el_agente(),
            ".gestor/vigilante/gestor-vigilante --hasta-cambio --agente \"<tu identidad>\""
        );
    }
}
