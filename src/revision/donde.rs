//! Dónde se hace la revisión de modelos: en la carpeta donde está instalado Programator, la del
//! `programator.toml` en uso, con Programator y el agente que la lleva abiertos allí. Ahí están los
//! modelos, el banco y las revisiones, y el agente los puede leer sin salir de su proyecto. Desde
//! otra carpeta solo se explica cómo hacerla.

use std::path::{Path, PathBuf};

/// Si la carpeta de trabajo es la de instalación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Donde {
    EnLaInstalacion,
    /// En otra carpeta; `instalacion` es adonde hay que ir.
    Fuera {
        instalacion: PathBuf,
    },
}

/// Compara la carpeta de trabajo con la de instalación, ya canonizadas si se pudo: una ruta relativa
/// y su absoluta, o una con otras mayúsculas en Windows, son la misma carpeta.
pub fn donde(carpeta_de_trabajo: &Path, instalacion: &Path) -> Donde {
    // Sin el prefijo extendido de Windows que añade `canonicalize`: la ruta se le enseña a una persona.
    let real = |ruta: &Path| {
        crate::plataforma::sin_prefijo_extendido(
            &ruta.canonicalize().unwrap_or_else(|_| ruta.to_path_buf()),
        )
    };
    let instalacion = real(instalacion);
    if real(carpeta_de_trabajo) == instalacion {
        Donde::EnLaInstalacion
    } else {
        Donde::Fuera { instalacion }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_misma_carpeta_aunque_se_escriba_distinto_es_la_instalacion() {
        let carpeta = tempfile::tempdir().unwrap();
        let con_rodeo = carpeta.path().join("sub").join("..");
        std::fs::create_dir(carpeta.path().join("sub")).unwrap();

        assert_eq!(
            donde(carpeta.path(), carpeta.path()),
            Donde::EnLaInstalacion
        );
        assert_eq!(donde(&con_rodeo, carpeta.path()), Donde::EnLaInstalacion);
    }

    /// El aviso no enseña la forma larga de la ruta, con «\\?\» delante.
    #[test]
    fn la_instalacion_se_nombra_sin_el_prefijo_extendido() {
        let instalacion = tempfile::tempdir().unwrap();
        let proyecto = tempfile::tempdir().unwrap();

        let Donde::Fuera { instalacion } = donde(proyecto.path(), instalacion.path()) else {
            panic!("debía estar fuera");
        };

        assert!(
            !instalacion.to_string_lossy().starts_with(r"\\?\"),
            "{}",
            instalacion.display()
        );
    }

    #[test]
    fn otra_carpeta_esta_fuera_y_dice_adonde_ir() {
        let instalacion = tempfile::tempdir().unwrap();
        let proyecto = tempfile::tempdir().unwrap();

        assert_eq!(
            donde(proyecto.path(), instalacion.path()),
            Donde::Fuera {
                instalacion: crate::plataforma::sin_prefijo_extendido(
                    &instalacion.path().canonicalize().unwrap()
                )
            }
        );
    }
}
