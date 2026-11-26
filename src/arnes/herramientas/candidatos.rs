//! Versionado y rotación de nombres de candidatos en disco.
//!
//! Garantiza que una nueva propuesta nunca sobrescriba una entrega previa: si el nombre ya
//! está ocupado, busca la siguiente variante libre (`nombre-002.ext`, `nombre-003.ext`...).

use crate::arnes::confinamiento::hay_algo_en;
use std::path::{Path, PathBuf};

/// Cuántas variantes de un mismo nombre se admiten antes de rendirse.
///
/// El tope existe para no barrer un directorio entero si algo va mal; noventa y nueve entregas
/// en total (el original más las variantes 002 a 099) son ya un fallo distinto que conviene que se vea.
const MAX_VARIANTES_DE_UN_NOMBRE: u32 = 99;

/// La primera variante: la 1 es el nombre tal cual, sin sufijo.
pub(crate) const PRIMERA_VARIANTE: u32 = 2;

/// `ruta` con la variante `variante`: `nombre-002.ext`, o `nombre-002` sin extensión. Es la única
/// regla de los sufijos: las propuestas y su archivo los componen igual, y la deducción de las
/// entregas depende de que coincidan.
pub(crate) fn con_variante(ruta: &Path, variante: u32) -> PathBuf {
    let tronco = ruta
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    ruta.with_file_name(match ruta.extension() {
        Some(ext) => format!("{tronco}-{variante:03}.{}", ext.to_string_lossy()),
        None => format!("{tronco}-{variante:03}"),
    })
}

/// La primera ruta libre de la forma `nombre.ext`, `nombre-002.ext`, `nombre-003.ext`…
///
/// Si no queda ninguna, `None`: nunca se escribe encima de lo que hay, porque si lo que hay es un
/// enlace, escribir lo seguiría fuera del proyecto.
pub(super) fn siguiente_libre(ruta: &Path) -> Option<PathBuf> {
    if !hay_algo_en(ruta) {
        return Some(ruta.to_path_buf());
    }
    (PRIMERA_VARIANTE..=MAX_VARIANTES_DE_UN_NOMBRE)
        .map(|variante| con_variante(ruta, variante))
        .find(|candidata| !hay_algo_en(candidata))
}

/// Escribe un fichero que no existía. `create_new` falla si ya hay algo con ese nombre, aunque
/// sea un enlace roto, y no lo sigue.
pub(super) fn escribir_fichero_nuevo(ruta: &Path, contenido: &str) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(ruta)?
        .write_all(contenido.as_bytes())
}

/// Lo que se le dice al modelo si no queda ningún nombre libre para su propuesta.
pub(super) fn motivo_sin_nombre_libre(nombre: &str) -> String {
    format!(
        "no queda ningún nombre libre para «{nombre}» entre sus {MAX_VARIANTES_DE_UN_NOMBRE} variantes: elige otro nombre"
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La regla de los sufijos, que comparten las propuestas y su archivo.
    #[test]
    fn la_variante_va_antes_de_la_extension_con_tres_cifras() {
        let carpeta = Path::new("candidatos").join("src");
        assert_eq!(
            con_variante(&carpeta.join("lib.rs"), 2),
            carpeta.join("lib-002.rs")
        );
        assert_eq!(
            con_variante(&carpeta.join("Makefile"), 15),
            carpeta.join("Makefile-015")
        );
        assert_eq!(
            con_variante(&carpeta.join("datos.tar.gz"), 3),
            carpeta.join("datos.tar-003.gz")
        );
    }

    #[test]
    fn siguiente_libre_mantiene_el_directorio_padre_con_cualquier_nombre() {
        // `siguiente_libre` es la pieza nueva que calcula nombres libres cuando el fichero ya existe.
        // Su propiedad crítica para la seguridad: toma file_stem() y extension() de una ruta ya
        // confinada —que son por definición el último componente, sin /, \ ni ..— y aplica
        // with_file_name(), que solo sustituye ese componente y conserva el directorio padre.
        // Por construcción, no hay nombre que lo rompa.
        let carpeta = tempfile::tempdir().expect("crear directorio temporal");
        let candidatos = carpeta.path().join(".gestor/candidatos/programator");
        std::fs::create_dir_all(&candidatos).expect("crear directorio de candidatos");

        let nombres_retorcidos = [
            "sin_extension", // Sin extensión: file_stem() = "sin_extension", extension() = None
            "a.b.c.rs",      // Varios puntos: file_stem() = "a.b.c", extension() = "rs"
            ".oculto",       // Empieza por punto: file_stem() = ".oculto", extension() = None
            "..raro.rs", // Contiene .. pero es un nombre válido: file_stem() = "..raro", ext = "rs"
        ];

        for nombre_retorcido in &nombres_retorcidos {
            let ruta_original = candidatos.join(nombre_retorcido);
            std::fs::write(&ruta_original, "ocupado").unwrap();

            let siguiente = siguiente_libre(&ruta_original).unwrap();

            // El invariante: directorio padre nunca cambia
            assert_eq!(
                siguiente.parent(),
                ruta_original.parent(),
                "siguiente_libre(«{nombre_retorcido}») debe mantener el directorio padre"
            );

            // Cuando el original existe, siguiente_libre busca variantes: devuelve otro nombre
            assert_ne!(
                siguiente.file_name(),
                ruta_original.file_name(),
                "cuando «{nombre_retorcido}» existe, siguiente_libre debe buscar una variante"
            );

            // El nombre calculado debe estar libre
            assert!(
                !siguiente.exists(),
                "siguiente_libre(«{nombre_retorcido}») devolvió un fichero que ya existe"
            );

            // Limpiar para la siguiente iteración
            std::fs::remove_file(&ruta_original).unwrap();
        }

        // Caso límite: cuando el fichero no existe, siguiente_libre devuelve la ruta original
        let no_existe = candidatos.join("nuevo.rs");
        let siguiente_nuevo = siguiente_libre(&no_existe);
        assert_eq!(
            siguiente_nuevo,
            Some(no_existe),
            "cuando el fichero no existe, siguiente_libre devuelve la ruta sin cambios"
        );
    }

    #[test]
    fn escribir_un_fichero_nuevo_nunca_escribe_encima() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("a.rs");
        escribir_fichero_nuevo(&ruta, "uno").unwrap();

        assert!(escribir_fichero_nuevo(&ruta, "dos").is_err());
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), "uno");
    }

    /// Si no queda ningún nombre libre no se devuelve ninguno. Devolver el último ocupado haría
    /// escribir encima, y si era un enlace, seguirlo.
    #[test]
    fn sin_nombres_libres_no_devuelve_ninguno() {
        let carpeta = tempfile::tempdir().unwrap();
        let original = carpeta.path().join("z.rs");
        std::fs::write(&original, "ocupado").unwrap();
        for variante in 2..=MAX_VARIANTES_DE_UN_NOMBRE {
            std::fs::write(
                carpeta.path().join(format!("z-{variante:03}.rs")),
                "ocupado",
            )
            .unwrap();
        }

        assert_eq!(siguiente_libre(&original), None);
    }

    /// Un enlace roto ocupa su nombre aunque `exists()` diga que no; escribir en él seguiría el
    /// enlace.
    #[cfg(windows)]
    #[test]
    fn un_enlace_roto_ocupa_su_nombre() {
        let carpeta = tempfile::tempdir().unwrap();
        let destino = carpeta.path().join("borrado");
        std::fs::create_dir(&destino).unwrap();
        let enlace = carpeta.path().join("x.rs");
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

        assert_eq!(
            siguiente_libre(&enlace),
            Some(carpeta.path().join("x-002.rs"))
        );

        // Y también cuando el enlace roto es una de las variantes.
        let original = carpeta.path().join("y.rs");
        std::fs::write(&original, "ocupado").unwrap();
        std::fs::create_dir(&destino).unwrap();
        let variante = carpeta.path().join("y-002.rs");
        let estado = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                variante.to_str().unwrap(),
                destino.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(estado.success(), "no se pudo crear la unión de prueba");
        std::fs::remove_dir(&destino).unwrap();

        assert_eq!(
            siguiente_libre(&original),
            Some(carpeta.path().join("y-003.rs"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn un_enlace_roto_ocupa_su_nombre() {
        let carpeta = tempfile::tempdir().unwrap();
        let enlace = carpeta.path().join("x.rs");
        std::os::unix::fs::symlink(carpeta.path().join("no-existe"), &enlace).unwrap();

        assert_eq!(
            siguiente_libre(&enlace),
            Some(carpeta.path().join("x-002.rs"))
        );

        // Y también cuando el enlace roto es una de las variantes.
        let original = carpeta.path().join("y.rs");
        std::fs::write(&original, "ocupado").unwrap();
        std::os::unix::fs::symlink(
            carpeta.path().join("no-existe"),
            carpeta.path().join("y-002.rs"),
        )
        .unwrap();

        assert_eq!(
            siguiente_libre(&original),
            Some(carpeta.path().join("y-003.rs"))
        );
    }
}
