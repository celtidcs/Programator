//! Dónde viven los ficheros propios del agente dentro de la carpeta de trabajo.
//!
//! **Por qué un módulo para esto.** Decisión 1: el espacio del agente es `.gestor/<agente>/`, y no
//! `.gestor/canal/`, que es territorio compartido y solo debe contener lo que el protocolo dice.
//! Ese cálculo lo necesitan ya dos sitios —el registro de lectura y el fichero de instrucciones—,
//! y escrito dos veces se desincronizaría el día que alguien cambie `[agente] nombre`.

use std::path::{Path, PathBuf};

/// El acuse de hasta dónde se leyó cada buzón.
const NOMBRE_DEL_REGISTRO: &str = "lectura.json";

/// El fichero de instrucciones del proyecto que lee el modelo.
///
/// **El nombre no se deriva del agente a propósito.** Es el contrato publicado con las
/// instalaciones que ya existen —la guía del canal lo nombra así, y hay proyectos con el suyo ya
/// adaptado—; cambiarlo por un cálculo dejaría huérfanos esos ficheros sin avisar a nadie.
const NOMBRE_DE_LAS_NORMAS: &str = "PROGRAMATOR.md";

/// La carpeta propia de este agente dentro de la carpeta de trabajo.
///
/// En minúsculas porque el nombre configurado es para leerlo («Programator») y una carpeta con
/// mayúsculas en medio de `.gestor/` desentona con el resto del árbol.
pub fn carpeta_del_agente(carpeta: &Path, agente: &str) -> PathBuf {
    carpeta.join(".gestor").join(agente.to_lowercase())
}

/// Dónde se guarda el registro de lectura del canal.
pub fn ruta_del_registro(carpeta: &Path, agente: &str) -> PathBuf {
    carpeta_del_agente(carpeta, agente).join(NOMBRE_DEL_REGISTRO)
}

/// Dónde se guarda el latido del arnés en cada sondeo.
pub fn ruta_del_latido(carpeta: &Path, agente: &str, nombre_fichero: &str) -> PathBuf {
    carpeta_del_agente(carpeta, agente).join(nombre_fichero)
}

/// Dónde se instala el fichero de instrucciones en una instalación nueva.
pub fn ruta_de_las_normas(carpeta: &Path, agente: &str) -> PathBuf {
    carpeta_del_agente(carpeta, agente).join(NOMBRE_DE_LAS_NORMAS)
}

/// Cuál de los dos sitios manda.
///
/// **La raíz gana, y por compatibilidad, no por gusto.** Hasta la 0.9.0 el fichero se instalaba en
/// la raíz del proyecto anfitrión, y hay instalaciones con el suyo adaptado a mano. Mover el sitio
/// no puede dejarlas sin instrucciones ni duplicarles el fichero: si hay uno arriba, ese es el que
/// se lee y el que no se toca. Las instalaciones nuevas estrenan la ubicación limpia.
pub fn normas_en_uso(carpeta: &Path, agente: &str) -> PathBuf {
    let en_la_raiz = carpeta.join(NOMBRE_DE_LAS_NORMAS);
    if en_la_raiz.exists() {
        en_la_raiz
    } else {
        ruta_de_las_normas(carpeta, agente)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_carpeta_del_agente_va_en_minusculas_bajo_gestor() {
        let calculada = carpeta_del_agente(Path::new("/trabajo"), "Programator");
        assert_eq!(calculada, Path::new("/trabajo/.gestor/programator"));
    }

    #[test]
    fn el_registro_vive_en_la_carpeta_del_agente() {
        let calculada = ruta_del_registro(Path::new("/trabajo"), "Programator");
        assert_eq!(
            calculada,
            Path::new("/trabajo/.gestor/programator/lectura.json")
        );
    }

    #[test]
    fn el_latido_vive_en_la_carpeta_del_agente() {
        let calculada = ruta_del_latido(Path::new("/trabajo"), "Programator", "latido.json");
        assert_eq!(
            calculada,
            Path::new("/trabajo/.gestor/programator/latido.json")
        );
    }

    #[test]
    fn las_normas_nuevas_van_a_la_carpeta_del_agente() {
        let calculada = ruta_de_las_normas(Path::new("/trabajo"), "Programator");
        assert_eq!(
            calculada,
            Path::new("/trabajo/.gestor/programator/PROGRAMATOR.md")
        );
    }

    #[test]
    fn el_registro_y_las_instrucciones_comparten_carpeta_sea_cual_sea_el_agente() {
        // Si esto se rompiera, cambiar «[agente] nombre» dejaría el registro en una carpeta y las
        // instrucciones en otra, y nadie lo notaría hasta que faltara una de las dos.
        for agente in ["Programator", "Ayudante"] {
            let registro = ruta_del_registro(Path::new("/trabajo"), agente);
            let normas = ruta_de_las_normas(Path::new("/trabajo"), agente);
            assert_eq!(registro.parent(), normas.parent(), "agente: {agente}");
        }
    }

    #[test]
    fn sin_fichero_en_la_raiz_manda_el_de_la_carpeta_del_agente() {
        let temporal = tempfile::tempdir().unwrap();
        let carpeta = temporal.path();

        assert_eq!(
            normas_en_uso(carpeta, "Programator"),
            ruta_de_las_normas(carpeta, "Programator")
        );
    }

    #[test]
    fn un_fichero_en_la_raiz_sigue_mandando() {
        // La instalación de MMCelt tiene su PROGRAMATOR.md adaptado en la raíz desde antes de
        // esta versión. Mover el sitio no puede dejarla sin instrucciones.
        let temporal = tempfile::tempdir().unwrap();
        let carpeta = temporal.path();
        std::fs::write(carpeta.join("PROGRAMATOR.md"), "LAS DE ANTES").unwrap();

        assert_eq!(
            normas_en_uso(carpeta, "Programator"),
            carpeta.join("PROGRAMATOR.md")
        );
    }

    #[test]
    fn un_nombre_de_agente_distinto_cambia_la_carpeta() {
        // Si esto dejara de cumplirse, el registro y las instrucciones acabarían en carpetas
        // distintas en cuanto alguien tocara «[agente] nombre».
        let calculada = ruta_del_registro(Path::new("/trabajo"), "Ayudante");
        assert_eq!(
            calculada,
            Path::new("/trabajo/.gestor/ayudante/lectura.json")
        );
    }
}
