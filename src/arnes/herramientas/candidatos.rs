//! Versionado y rotación de nombres de candidatos en disco.
//!
//! Garantiza que una nueva propuesta nunca sobrescriba una entrega previa: si el nombre ya
//! está ocupado, busca la siguiente variante libre (`nombre-002.ext`, `nombre-003.ext`...).

use std::path::{Path, PathBuf};

/// Cuántas variantes de un mismo nombre se admiten antes de rendirse.
///
/// El tope existe para no barrer un directorio entero si algo va mal; noventa y nueve entregas
/// en total (el original más las variantes 002 a 099) son ya un fallo distinto que conviene que se vea.
const MAX_VARIANTES_DE_UN_NOMBRE: u32 = 99;

/// La primera ruta libre de la forma `nombre.ext`, `nombre-002.ext`, `nombre-003.ext`…
///
/// Si no queda ninguna, devuelve la última probada: escribir encima es el mal menor frente a
/// perder la entrega, y el veredicto dirá el nombre real.
pub(super) fn siguiente_libre(ruta: &Path) -> PathBuf {
    if !ruta.exists() {
        return ruta.to_path_buf();
    }
    let tronco = ruta
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let extension = ruta.extension().map(|e| e.to_string_lossy().to_string());
    let mut candidata = ruta.to_path_buf();
    for variante in 2..=MAX_VARIANTES_DE_UN_NOMBRE {
        let nombre = match &extension {
            Some(ext) => format!("{tronco}-{variante:03}.{ext}"),
            None => format!("{tronco}-{variante:03}"),
        };
        candidata = ruta.with_file_name(nombre);
        if !candidata.exists() {
            return candidata;
        }
    }
    candidata
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn siguiente_libre_mantiene_el_directorio_padre_con_cualquier_nombre() {
        // `siguiente_libre` es la pieza nueva que calcula nombres libres cuando el fichero ya existe.
        // Su propiedad crítica para la seguridad: toma file_stem() y extension() de una ruta ya
        // confinada —que son por definición el último componente, sin /, \ ni ..— y aplica
        // with_file_name(), que solo sustituye ese componente y conserva el directorio padre.
        // Por construcción, no hay nombre que lo rompa.
        let dir = tempfile::tempdir().expect("crear directorio temporal");
        let candidatos = dir.path().join(".gestor/candidatos/programator");
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

            let siguiente = siguiente_libre(&ruta_original);

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
            siguiente_nuevo, no_existe,
            "cuando el fichero no existe, siguiente_libre devuelve la ruta sin cambios"
        );
    }
}
