//! Leer, escribir, copiar y borrar ficheros, con el error que dice qué ruta falló. Lo usan el banco,
//! el registro y la revisión de modelos.
//!
//! Capa fina sobre `std::fs`: aquí no se decide nada.

use std::path::{Path, PathBuf};

use crate::error::{Error, Resultado};

/// Cuánto se lee de una vez al calcular una huella: 1 MiB, para no cargar un modelo en memoria.
const TAMANO_DEL_TROZO: usize = 1024 * 1024;

/// Las entradas de una carpeta, sin orden garantizado.
pub fn entradas(carpeta: &Path) -> Resultado<Vec<PathBuf>> {
    let error = |causa| Error::Lectura {
        ruta: carpeta.to_path_buf(),
        causa,
    };
    let mut rutas = Vec::new();
    for entrada in std::fs::read_dir(carpeta).map_err(error)? {
        rutas.push(entrada.map_err(error)?.path());
    }
    Ok(rutas)
}

/// Copia el contenido de `origen` dentro de `destino`, conservando las subcarpetas.
pub fn copiar_arbol(origen: &Path, destino: &Path) -> Resultado<()> {
    crear_carpeta(destino)?;
    for ruta in entradas(origen)? {
        let Some(nombre) = ruta.file_name() else {
            continue;
        };
        let objetivo = destino.join(nombre);
        if ruta.is_dir() {
            copiar_arbol(&ruta, &objetivo)?;
        } else {
            std::fs::copy(&ruta, &objetivo).map_err(|causa| Error::Escritura {
                ruta: objetivo.clone(),
                causa,
            })?;
        }
    }
    Ok(())
}

/// La huella SHA-256 de un fichero, en hexadecimal. Lo lee por trozos: un modelo pesa gigas.
pub fn huella_sha256(ruta: &Path) -> Resultado<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let error = |causa| Error::Lectura {
        ruta: ruta.to_path_buf(),
        causa,
    };
    let mut fichero = std::fs::File::open(ruta).map_err(error)?;
    let mut hasher = Sha256::new();
    let mut trozo = vec![0u8; TAMANO_DEL_TROZO];
    loop {
        let leidos = fichero.read(&mut trozo).map_err(error)?;
        if leidos == 0 {
            break;
        }
        hasher.update(&trozo[..leidos]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// La huella SHA-256 de un texto, en hexadecimal.
#[cfg(test)]
pub fn huella_sha256_del_texto(texto: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(texto.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Si alguna palabra de `texto`, en minúsculas, tiene una de las `huellas`. Sirve para vigilar
/// que un texto que se publica no nombre algo que tampoco puede nombrar la prueba que lo vigila.
#[cfg(test)]
pub fn nombra_alguna_huella(texto: &str, huellas: &[&str]) -> bool {
    texto
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|palabra| !palabra.is_empty())
        .any(|palabra| huellas.contains(&huella_sha256_del_texto(palabra).as_str()))
}

/// Borra un fichero; si no existe, no hace nada.
pub fn borrar_fichero_si_existe(ruta: &Path) -> Resultado<()> {
    match std::fs::remove_file(ruta) {
        Err(causa) if causa.kind() != std::io::ErrorKind::NotFound => Err(Error::Escritura {
            ruta: ruta.to_path_buf(),
            causa,
        }),
        _ => Ok(()),
    }
}

/// Borra una carpeta con todo lo que tenga; si no existe, no hace nada.
pub fn borrar_si_existe(carpeta: &Path) -> Resultado<()> {
    if !carpeta.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(carpeta).map_err(|causa| Error::Escritura {
        ruta: carpeta.to_path_buf(),
        causa,
    })
}

pub fn crear_carpeta(carpeta: &Path) -> Resultado<()> {
    std::fs::create_dir_all(carpeta).map_err(|causa| Error::Escritura {
        ruta: carpeta.to_path_buf(),
        causa,
    })
}

pub fn escribir(ruta: &Path, contenido: &str) -> Resultado<()> {
    std::fs::write(ruta, contenido).map_err(|causa| Error::Escritura {
        ruta: ruta.to_path_buf(),
        causa,
    })
}

/// Cambia de nombre un fichero o una carpeta; el error nombra el destino.
pub fn renombrar(origen: &Path, destino: &Path) -> Resultado<()> {
    std::fs::rename(origen, destino).map_err(|causa| Error::Escritura {
        ruta: destino.to_path_buf(),
        causa,
    })
}

pub fn leer(ruta: &Path) -> Resultado<String> {
    std::fs::read_to_string(ruta).map_err(|causa| Error::Lectura {
        ruta: ruta.to_path_buf(),
        causa,
    })
}

/// Como [`leer`], pero un fichero que no existe es un texto vacío.
pub fn leer_si_existe(ruta: &Path) -> Resultado<String> {
    if ruta.exists() {
        leer(ruta)
    } else {
        Ok(String::new())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_huella_de_un_texto_es_su_sha256_y_se_busca_por_palabras_enteras() {
        let huella = "3928d2fb50c11791fed429ae38189f8bb36c8fdcefc57328d04b1381c39de380";
        assert_eq!(huella_sha256_del_texto("encargo"), huella);
        assert!(nombra_alguna_huella("Atiende un ENCARGO.", &[huella]));
        assert!(!nombra_alguna_huella("atiende los encargos", &[huella]));
        assert!(!nombra_alguna_huella("", &[huella]));
    }

    #[test]
    fn la_huella_es_la_sha256_del_contenido() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("abc.txt");
        std::fs::write(&ruta, "abc").unwrap();

        assert_eq!(
            huella_sha256(&ruta).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
