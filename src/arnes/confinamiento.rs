//! Confinamiento de rutas. El modelo nunca escribe un fichero: solicita una ruta, y aquí se decide
//! si esa ruta cae dentro del ámbito permitido. Un fallo en este módulo anula todas las garantías
//! del arnés, así que la comprobación se hace en dos pasos: normalización textual y canonización
//! real del ancestro existente más profundo (que resuelve enlaces simbólicos y uniones de
//! directorio), siempre, exista o no el fichero final. Condicionar la canonización a que la ruta
//! completa exista dejaría un hueco: un enlace intermedio con el fichero hoja todavía inexistente
//! escaparía sin comprobación real de sistema de ficheros.

use crate::error::{Error, Resultado};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Carpeta dentro de la cual el modelo puede operar.
pub struct Ambito {
    raiz: PathBuf,
}

impl Ambito {
    /// Crea un ámbito sobre una carpeta que debe existir.
    pub fn nuevo(raiz: &Path) -> Resultado<Self> {
        let raiz = raiz.canonicalize().map_err(|causa| Error::Lectura {
            ruta: raiz.to_path_buf(),
            causa,
        })?;
        Ok(Self { raiz })
    }

    /// La carpeta de trabajo, ya canónica.
    pub fn raiz(&self) -> &Path {
        &self.raiz
    }

    /// Resuelve una ruta solicitada por el modelo, o la deniega.
    pub fn resolver(&self, relativa: &str) -> Resultado<PathBuf> {
        let solicitada = Path::new(relativa);
        let unida = if solicitada.is_absolute() {
            solicitada.to_path_buf()
        } else {
            self.raiz.join(solicitada)
        };

        let normalizada = normalizar(&unida);
        if !normalizada.starts_with(&self.raiz) {
            return Err(Error::FueraDeAmbito(unida));
        }

        // La canonización decide siempre, exista o no el fichero final: se aplica sobre el
        // ancestro existente más profundo (en el peor caso, `self.raiz`, que ya existe porque
        // `Ambito::nuevo` la canonizó) y luego se reconstruye la cola que todavía no existe. Así
        // no hay ruta, con o sin fichero hoja, que evite pasar por un sistema de ficheros real.
        let (ancestro, restantes) = ancestro_mas_profundo(&normalizada);
        let ancestro_real = ancestro.canonicalize().map_err(|causa| Error::Lectura {
            ruta: ancestro.clone(),
            causa,
        })?;
        if !ancestro_real.starts_with(&self.raiz) {
            return Err(Error::FueraDeAmbito(ancestro_real));
        }

        let mut real = ancestro_real;
        for componente in restantes {
            real.push(componente);
        }
        Ok(real)
    }
}

/// Elimina `.` y resuelve `..` de forma puramente textual, sin tocar el disco.
fn normalizar(ruta: &Path) -> PathBuf {
    let mut salida = PathBuf::new();
    for componente in ruta.components() {
        match componente {
            Component::CurDir => {}
            Component::ParentDir => {
                salida.pop();
            }
            otro => salida.push(otro.as_os_str()),
        }
    }
    salida
}

/// Si hay algo en disco con ese nombre, aunque sea un enlace roto. `exists()` sigue los enlaces y
/// da falso ante uno roto: el ascenso lo saltaría y la escritura lo seguiría luego fuera del
/// ámbito. Parar en él hace que la canonización falle y la ruta se deniegue.
pub(crate) fn hay_algo_en(ruta: &Path) -> bool {
    ruta.symlink_metadata().is_ok()
}

/// Recorre `ruta` hacia arriba hasta el primer ancestro que existe en disco. Devuelve ese
/// ancestro junto con los componentes finales que todavía no existen, en el orden en que hay
/// que volver a añadirlos. Termina como muy tarde en la raíz del sistema de ficheros, pero en la
/// práctica siempre para antes: el ámbito ya comprobó que `ruta` cae bajo `self.raiz`, y esa raíz
/// existe porque `Ambito::nuevo` la canonizó al crear el ámbito.
fn ancestro_mas_profundo(ruta: &Path) -> (PathBuf, Vec<OsString>) {
    let mut restantes = Vec::new();
    let mut actual = ruta.to_path_buf();
    while !hay_algo_en(&actual) {
        match actual.file_name() {
            Some(nombre) => {
                restantes.push(nombre.to_os_string());
                actual.pop();
            }
            None => break,
        }
    }
    restantes.reverse();
    (actual, restantes)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn concede_una_ruta_dentro_del_ambito() {
        let carpeta = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(carpeta.path().join("src")).unwrap();
        std::fs::write(carpeta.path().join("src/main.rs"), "fn main() {}").unwrap();
        let ambito = Ambito::nuevo(carpeta.path()).unwrap();

        let resuelta = ambito.resolver("src/main.rs").unwrap();

        assert!(resuelta.ends_with("main.rs"));
    }

    #[test]
    fn deniega_el_ascenso_con_dos_puntos() {
        let carpeta = tempfile::tempdir().unwrap();
        let ambito = Ambito::nuevo(carpeta.path()).unwrap();

        let fallo = ambito.resolver("../../secreto.txt").unwrap_err();

        assert!(matches!(fallo, crate::error::Error::FueraDeAmbito(_)));
    }

    #[test]
    fn deniega_el_ascenso_disimulado_en_medio_de_la_ruta() {
        let carpeta = tempfile::tempdir().unwrap();
        let ambito = Ambito::nuevo(carpeta.path()).unwrap();

        let fallo = ambito.resolver("src/../../fuera.txt").unwrap_err();

        assert!(matches!(fallo, crate::error::Error::FueraDeAmbito(_)));
    }

    #[test]
    fn deniega_una_ruta_absoluta_ajena() {
        let carpeta = tempfile::tempdir().unwrap();
        let otro = tempfile::tempdir().unwrap();
        let ambito = Ambito::nuevo(carpeta.path()).unwrap();

        let fallo = ambito
            .resolver(otro.path().join("x.txt").to_str().unwrap())
            .unwrap_err();

        assert!(matches!(fallo, crate::error::Error::FueraDeAmbito(_)));
    }

    #[test]
    fn concede_un_fichero_que_todavia_no_existe_pero_cuyo_destino_esta_dentro() {
        let carpeta = tempfile::tempdir().unwrap();
        let ambito = Ambito::nuevo(carpeta.path()).unwrap();

        let resuelta = ambito.resolver(".gestor/candidatos/nuevo.md").unwrap();

        assert!(resuelta.ends_with("nuevo.md"));
    }

    #[cfg(windows)]
    #[test]
    fn deniega_el_ascenso_a_traves_de_una_union_de_directorio_con_fichero_nuevo() {
        let carpeta = tempfile::tempdir().unwrap();
        let otro = tempfile::tempdir().unwrap();
        let enlace = carpeta.path().join("enlace");

        // Las uniones de directorio (`mklink /J`), a diferencia de los enlaces
        // simbólicos, no exigen privilegios elevados en Windows.
        let estado = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                enlace.to_str().unwrap(),
                otro.path().to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(
            estado.success(),
            "no se pudo crear la unión de directorio de prueba"
        );

        let ambito = Ambito::nuevo(carpeta.path()).unwrap();

        let fallo = ambito.resolver("enlace/fichero-nuevo.txt").unwrap_err();

        assert!(matches!(fallo, crate::error::Error::FueraDeAmbito(_)));
    }

    /// `exists()` da falso ante un enlace roto: si el ascenso lo saltara como si no hubiera nada,
    /// la escritura seguiría luego el enlace fuera del proyecto.
    #[cfg(windows)]
    #[test]
    fn deniega_una_union_de_directorio_rota() {
        let carpeta = tempfile::tempdir().unwrap();
        let otro = tempfile::tempdir().unwrap();
        let destino = otro.path().join("borrado");
        std::fs::create_dir(&destino).unwrap();
        let enlace = carpeta.path().join("enlace");
        let estado = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                enlace.to_str().unwrap(),
                destino.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(estado.success(), "no se pudo crear la unión de prueba");
        std::fs::remove_dir(&destino).unwrap();
        let ambito = Ambito::nuevo(carpeta.path()).unwrap();

        assert!(ambito.resolver("enlace/nuevo.txt").is_err());
        assert!(ambito.resolver("enlace").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn deniega_un_enlace_simbolico_roto_hacia_fuera() {
        let carpeta = tempfile::tempdir().unwrap();
        let otro = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(
            otro.path().join("nuevo.rs"),
            carpeta.path().join("enlace.rs"),
        )
        .unwrap();
        let ambito = Ambito::nuevo(carpeta.path()).unwrap();

        assert!(ambito.resolver("enlace.rs").is_err());
    }
}
