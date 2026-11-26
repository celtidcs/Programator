//! Leer, escribir, copiar y borrar ficheros, con el error que dice qué ruta falló. Lo usan el banco,
//! el registro y la revisión de modelos.
//!
//! Capa fina sobre `std::fs`: aquí no se decide nada.

use std::path::{Path, PathBuf};

use crate::error::{Error, Resultado};

/// Cuánto se lee de una vez al calcular una huella: 1 MiB, para no cargar un modelo en memoria.
pub(crate) const TAMANO_DEL_TROZO: usize = crate::unidades::BYTES_POR_MIB_USIZE;

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
    let mut calculador = Sha256::new();
    let mut trozo = vec![0u8; TAMANO_DEL_TROZO];
    loop {
        let leidos = fichero.read(&mut trozo).map_err(error)?;
        if leidos == 0 {
            break;
        }
        calculador.update(&trozo[..leidos]);
    }
    Ok(format!("{:x}", calculador.finalize()))
}

/// La huella SHA-256 de unos bytes ya leídos, en hexadecimal: la misma que daría
/// [`huella_sha256`] con un fichero que los contuviera.
pub fn huella_sha256_de_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
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

/// Los permisos de un fichero que solo debe leer su dueño, en Unix: lectura y escritura para él y
/// nada para el grupo ni los demás.
#[cfg(unix)]
const PERMISOS_SOLO_DEL_DUENO: u32 = 0o600;

/// Como [`escribir`], pero en Unix el fichero queda con permisos 0600 aunque ya existiera con
/// otros: es para secretos, como la clave del motor. En Windows hereda la lista de control de
/// acceso de su carpeta, que en la instalación es del usuario.
pub fn escribir_solo_para_el_dueno(ruta: &Path, contenido: &str) -> Resultado<()> {
    let error = |causa| Error::Escritura {
        ruta: ruta.to_path_buf(),
        causa,
    };
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut fichero = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(PERMISOS_SOLO_DEL_DUENO)
            .open(ruta)
            .map_err(error)?;
        // `mode` solo vale al crearlo: si ya existía, sus permisos se ponen aquí.
        fichero
            .set_permissions(std::fs::Permissions::from_mode(PERMISOS_SOLO_DEL_DUENO))
            .map_err(error)?;
        fichero.write_all(contenido.as_bytes()).map_err(error)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(ruta, contenido).map_err(error)
    }
}

/// Lo que se añade al nombre de un fichero para escribirlo antes de renombrarlo con
/// [`escribir_de_una_vez`]: si el programa se corta a mitad, queda a medias el temporal y no el
/// fichero.
pub const SUFIJO_DEL_TEMPORAL: &str = ".escribiendo";

/// Escribe el fichero entero de una vez: primero en un temporal al lado y luego, con un cambio de
/// nombre, con el suyo. Crea la carpeta si falta. Si el cambio de nombre falla, el temporal se
/// borra y se devuelve ese fallo, que es el que importa.
pub fn escribir_de_una_vez(ruta: &Path, contenido: &str) -> Resultado<()> {
    if let Some(carpeta) = ruta.parent() {
        crear_carpeta(carpeta)?;
    }
    let temporal = ruta_del_temporal(ruta);
    escribir(&temporal, contenido)?;
    renombrar(&temporal, ruta).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporal);
    })
}

/// El temporal con que [`escribir_de_una_vez`] escribe `ruta`.
pub fn ruta_del_temporal(ruta: &Path) -> PathBuf {
    let mut nombre = ruta.as_os_str().to_owned();
    nombre.push(SUFIJO_DEL_TEMPORAL);
    PathBuf::from(nombre)
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
    fn escribir_solo_para_el_dueno_escribe_y_sobrescribe() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("secreto");
        escribir_solo_para_el_dueno(&ruta, "uno largo").unwrap();
        escribir_solo_para_el_dueno(&ruta, "dos").unwrap();
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), "dos");
    }

    /// También si el fichero ya existía con permisos abiertos.
    #[cfg(unix)]
    #[test]
    fn escribir_solo_para_el_dueno_deja_0600_en_unix() {
        use std::os::unix::fs::PermissionsExt;
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("secreto");
        std::fs::write(&ruta, "viejo").unwrap();
        std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o644)).unwrap();

        escribir_solo_para_el_dueno(&ruta, "nuevo").unwrap();

        let modo = std::fs::metadata(&ruta).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o600);
    }

    #[test]
    fn escribir_de_una_vez_deja_el_fichero_sin_el_temporal() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let ruta = carpeta_temporal.path().join("sub").join("f.toml");

        escribir_de_una_vez(&ruta, "hola").unwrap();

        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), "hola");
        assert!(!ruta_del_temporal(&ruta).exists());
    }

    /// Una carpeta con el nombre del fichero: ningún sistema deja poner un fichero encima.
    #[test]
    fn si_no_se_puede_poner_su_nombre_no_queda_el_temporal() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("f.toml");
        std::fs::create_dir(&ruta).unwrap();
        std::fs::write(ruta.join("dentro"), "x").unwrap();

        assert!(escribir_de_una_vez(&ruta, "hola").is_err());
        assert!(!ruta_del_temporal(&ruta).exists());
    }

    #[test]
    fn la_huella_de_unos_bytes_es_la_de_un_fichero_con_ellos() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("abc.txt");
        std::fs::write(&ruta, b"abc").unwrap();

        // El vector de prueba de «abc» de FIPS 180-2.
        let esperada = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(huella_sha256_de_bytes(b"abc"), esperada);
        assert_eq!(huella_sha256(&ruta).unwrap(), esperada);
    }

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
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("abc.txt");
        std::fs::write(&ruta, "abc").unwrap();

        assert_eq!(
            huella_sha256(&ruta).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
