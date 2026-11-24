//! El fichero de un modelo: su extensión y el nombre con que lo conocen la tanda, el registro y
//! la revisión, en un solo sitio.
//!
//! Antes, seis módulos tenían su propia `".gguf"` o `"gguf"` y siete sacaban el nombre con su
//! propio `file_stem`. La extensión no distingue mayúsculas: un `Modelo.GGUF` es un modelo.

use std::path::Path;

/// La extensión de los modelos, sin el punto.
pub const EXTENSION: &str = "gguf";

/// `modelo.gguf` → `modelo`, sin distinguir mayúsculas en la extensión y conservando las del
/// nombre; lo que no es un GGUF, nada.
pub fn nombre_sin_extension(fichero: &str) -> Option<&str> {
    let (nombre, extension) = fichero.rsplit_once('.')?;
    extension.eq_ignore_ascii_case(EXTENSION).then_some(nombre)
}

/// Si el nombre de fichero es el de un modelo.
pub fn es_gguf(fichero: &str) -> bool {
    nombre_sin_extension(fichero).is_some()
}

/// Si la ruta es la de un modelo, por su extensión.
pub fn es_ruta_gguf(ruta: &Path) -> bool {
    ruta.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION))
}

/// El fichero de un modelo a partir de su nombre: `modelo` → `modelo.gguf`.
pub fn con_extension(nombre: &str) -> String {
    format!("{nombre}.{EXTENSION}")
}

/// El nombre con que el registro conoce un modelo: el de su fichero, sin carpeta ni extensión.
/// `None` si la ruta no tiene nombre de fichero.
pub fn nombre_de_modelo(ruta: &Path) -> Option<String> {
    ruta.file_stem()
        .map(|nombre| nombre.to_string_lossy().into_owned())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_extension_no_distingue_mayusculas_y_el_nombre_se_conserva() {
        assert_eq!(nombre_sin_extension("Modelo-Q4.GGUF"), Some("Modelo-Q4"));
        assert_eq!(nombre_sin_extension("modelo.gguf"), Some("modelo"));
        assert_eq!(nombre_sin_extension("modelo.bin"), None);
        assert_eq!(nombre_sin_extension("gguf"), None);
        assert!(es_gguf("a.Gguf"));
        assert!(!es_gguf("a.gguf.parcial"));
    }

    #[test]
    fn la_ruta_y_el_nombre_salen_igual_que_del_texto() {
        let ruta = Path::new("modelos/Devstral.GGUF");

        assert!(es_ruta_gguf(ruta));
        assert!(!es_ruta_gguf(Path::new("modelos/Devstral.txt")));
        assert_eq!(nombre_de_modelo(ruta).as_deref(), Some("Devstral"));
        assert_eq!(con_extension("Devstral"), "Devstral.gguf");
    }
}
