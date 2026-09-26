//! Lectura y composición de las instrucciones de sistema para el modelo.
//!
//! Separa la carga del preámbulo y las normas completas, aplicando la sustitución
//! de marcadores de repertorio y comprobadores con respaldo garantizado si el fichero
//! configurado no está disponible.

use crate::arnes::espacio::normas_en_uso;
use crate::arnes::verificacion::Comprobadores;
use crate::error::{Error, Resultado};
use std::path::Path;

/// Lee las normas de la casa desde `<carpeta>/PROGRAMATOR.md`.
/// Las instrucciones de sistema que ve el modelo en cada encargo.
///
/// Con preámbulo configurado y legible, se usa ese: unos 1.000 tokens con lo que necesita para
/// actuar, en vez de los 8.000 del protocolo completo. Las normas enteras siguen en
/// `PROGRAMATOR.md`, dentro de la carpeta de trabajo, y el modelo puede leerlas con `leer_fichero`
/// cuando necesite consultar algo concreto.
///
/// **Si el preámbulo no está o no se deja leer, se cae a las normas completas.** Quedarse sin
/// instrucciones sería mucho peor que gastar contexto de más, y quien no tenga el fichero no nota
/// ningún cambio.
///
/// El preámbulo se compone aquí, al leerlo, porque es la plantilla cruda: a diferencia de
/// `PROGRAMATOR.md`, que ya sale compuesto de `normas_en_uso` (se instala así en el arranque), a
/// este camino no lo compone nadie más. Componerlo en los dos sitios dejaría el marcador
/// sustituido dos veces si algún día ambos textos compartieran uno; componerlo en ninguno es lo
/// que le hizo prometer al modelo, en NatureLand, una compilación de `.cs` que no iba a ocurrir
/// porque esa extensión no tenía comprobador configurado.
pub(super) fn leer_instrucciones(
    carpeta: &Path,
    agente: &str,
    preambulo: Option<&Path>,
    comprobadores: &Comprobadores,
) -> Resultado<String> {
    if let Some(ruta) = preambulo {
        if let Ok(texto) = std::fs::read_to_string(ruta) {
            if !texto.trim().is_empty() {
                let texto = crate::arnes::redaccion::componer(&texto);
                let texto = crate::arnes::normas::componer(&texto, comprobadores);
                return Ok(texto);
            }
        }
    }
    let ruta = normas_en_uso(carpeta, agente);
    std::fs::read_to_string(&ruta).map_err(|causa| Error::Lectura { ruta, causa })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta_de_prueba() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let carpeta = dir.path().to_path_buf();
        std::fs::create_dir_all(carpeta.join(".gestor/canal")).unwrap();
        std::fs::write(carpeta.join("PROGRAMATOR.md"), "NORMAS DE LA CASA\n").unwrap();
        (dir, carpeta)
    }

    #[test]
    fn con_preambulo_el_modelo_ve_el_preambulo_y_no_las_normas_enteras() {
        // El porqué: las normas completas ocupan unos 8.000 tokens de los 16.384 del contexto, y
        // la mayor parte no le aplica al modelo porque no dispone de esos verbos. El preámbulo
        // trae lo que necesita para actuar en unos 1.000.
        let (dir, carpeta) = carpeta_de_prueba();
        let preambulo = dir.path().join("preambulo.md");
        std::fs::write(&preambulo, "ERES PROGRAMATOR Y TIENES CINCO HERRAMIENTAS\n").unwrap();

        let instrucciones = leer_instrucciones(
            &carpeta,
            "Programator",
            Some(&preambulo),
            &Comprobadores::new(),
        )
        .unwrap();

        assert!(
            instrucciones.contains("CINCO HERRAMIENTAS"),
            "{instrucciones}"
        );
        assert!(
            !instrucciones.contains("NORMAS DE LA CASA"),
            "el protocolo entero no debe colarse en el contexto: {instrucciones}"
        );
    }

    #[test]
    fn el_preambulo_se_compone_con_el_repertorio_y_los_comprobadores_configurados() {
        // La plantilla cruda trae los dos marcadores sin sustituir: a este camino, a diferencia
        // de PROGRAMATOR.md, no lo compone nadie más antes de leerlo.
        let (dir, carpeta) = carpeta_de_prueba();
        let preambulo = dir.path().join("preambulo.md");
        std::fs::write(&preambulo, "{repertorio}\n\n{comprobadores}\n").unwrap();
        let comprobadores: Comprobadores = [(
            "rs".to_string(),
            vec!["cargo".to_string(), "check".to_string()],
        )]
        .into_iter()
        .collect();

        let instrucciones =
            leer_instrucciones(&carpeta, "Programator", Some(&preambulo), &comprobadores).unwrap();

        assert!(
            instrucciones.contains("leer_fichero"),
            "el repertorio tiene que salir de la ficha: {instrucciones}"
        );
        assert!(
            instrucciones.contains("`.rs` → `cargo check`"),
            "los comprobadores configurados tienen que aparecer: {instrucciones}"
        );
        assert!(
            !instrucciones.contains("{repertorio}") && !instrucciones.contains("{comprobadores}"),
            "ningún marcador puede sobrevivir a la composición: {instrucciones}"
        );
    }

    #[test]
    fn las_instrucciones_se_leen_de_la_carpeta_del_agente_cuando_no_estan_en_la_raiz() {
        // La ubicación que estrenan las instalaciones nuevas desde la 0.9.0.
        let carpeta = tempfile::tempdir().unwrap();
        let destino = crate::arnes::espacio::ruta_de_las_normas(carpeta.path(), "Programator");
        std::fs::create_dir_all(destino.parent().unwrap()).unwrap();
        std::fs::write(&destino, "INSTRUCCIONES NUEVAS\n").unwrap();

        let leidas =
            leer_instrucciones(carpeta.path(), "Programator", None, &Comprobadores::new()).unwrap();

        assert_eq!(leidas, "INSTRUCCIONES NUEVAS\n");
    }

    #[test]
    fn sin_preambulo_se_siguen_usando_las_normas_enteras() {
        // Retrocompatibilidad: quien no tenga el fichero no nota ningún cambio.
        let (_dir, carpeta) = carpeta_de_prueba();

        let instrucciones =
            leer_instrucciones(&carpeta, "Programator", None, &Comprobadores::new()).unwrap();

        assert!(
            instrucciones.contains("NORMAS DE LA CASA"),
            "{instrucciones}"
        );
    }

    #[test]
    fn un_preambulo_que_no_esta_no_deja_al_modelo_sin_instrucciones() {
        // Quedarse mudo sería mucho peor que gastar contexto de más: se cae a las normas enteras.
        let (dir, carpeta) = carpeta_de_prueba();
        let fantasma = dir.path().join("no-existe.md");

        let instrucciones = leer_instrucciones(
            &carpeta,
            "Programator",
            Some(&fantasma),
            &Comprobadores::new(),
        )
        .unwrap();

        assert!(
            instrucciones.contains("NORMAS DE LA CASA"),
            "{instrucciones}"
        );
    }

    #[test]
    fn un_preambulo_vacio_tampoco_deja_al_modelo_mudo() {
        let (dir, carpeta) = carpeta_de_prueba();
        let vacio = dir.path().join("vacio.md");
        std::fs::write(&vacio, "   \n\n").unwrap();

        let instrucciones =
            leer_instrucciones(&carpeta, "Programator", Some(&vacio), &Comprobadores::new())
                .unwrap();

        assert!(
            instrucciones.contains("NORMAS DE LA CASA"),
            "{instrucciones}"
        );
    }
}
