//! Los dos ficheros de una tanda en el disco: escribe el TOML y el informe en su carpeta, y los
//! vuelve a leer con las notas que el agente que dirige haya puesto en las rúbricas. Es la capa
//! fina de entrada y salida: componer y leer el texto lo hacen funciones puras de los módulos
//! hermanos.

use std::path::Path;

use super::estadisticas::calcular_clases;
use super::redaccion::componer_informe;
use super::{componer_toml, leer_toml, ResultadosDeTanda};
use crate::banco::rubrica::leer_fichero;
use crate::error::Resultado;
use crate::ficheros::{escribir, leer};

/// El fichero con todas las cifras, dentro de la carpeta de la tanda.
pub const NOMBRE_RESULTADOS: &str = "resultados.toml";
/// El informe para leer, junto al anterior.
pub const NOMBRE_INFORME: &str = "informe.md";

/// La carpeta de la tanda: la que contiene su fichero de resultados.
fn carpeta_de_la_tanda(ruta_resultados: &Path) -> &Path {
    ruta_resultados.parent().unwrap_or_else(|| Path::new("."))
}

/// Escribe el TOML y el informe en `carpeta`.
pub fn guardar(resultados: &ResultadosDeTanda, carpeta: &Path) -> Resultado<()> {
    escribir(
        &carpeta.join(NOMBRE_RESULTADOS),
        &componer_toml(resultados)?,
    )?;
    escribir(&carpeta.join(NOMBRE_INFORME), &componer_informe(resultados))
}

/// Vuelve a leer las rúbricas de una tanda ya pasada y reescribe sus resultados e informe, sin
/// pasar ningún encargo. Devuelve cuántas rúbricas siguen pendientes.
pub fn rehacer_informe(ruta_resultados: &Path) -> Resultado<usize> {
    let resultados = leer_con_las_rubricas(ruta_resultados)?;
    guardar(&resultados, carpeta_de_la_tanda(ruta_resultados))?;
    Ok(resultados.pendientes())
}

/// Lee los resultados de una tanda con las notas que haya en sus rúbricas ahora mismo, aunque no
/// se haya rehecho el informe desde que se puntuaron.
pub fn leer_con_las_rubricas(ruta_resultados: &Path) -> Resultado<ResultadosDeTanda> {
    let carpeta = carpeta_de_la_tanda(ruta_resultados);
    let mut resultados = leer_toml(&leer(ruta_resultados)?)?;
    for semilla in resultados
        .encargos
        .iter_mut()
        .flat_map(|e| e.semillas.iter_mut())
    {
        if let Some(rubrica) = &semilla.rubrica {
            let nota = leer_fichero(&carpeta.join(rubrica))?;
            semilla.aplicar_rubrica(nota);
        }
    }
    resultados.clases = calcular_clases(&resultados.encargos);
    Ok(resultados)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::pruebas::tanda_de_ejemplo;
    use crate::banco::rubrica::{componer_fichero, nombre_del_fichero};

    #[test]
    fn la_carpeta_de_la_tanda_es_la_que_contiene_los_resultados() {
        let ruta = Path::new("tandas").join("t1").join(NOMBRE_RESULTADOS);

        assert_eq!(
            carpeta_de_la_tanda(&ruta),
            Path::new("tandas").join("t1").as_path()
        );
    }

    #[test]
    fn rehacer_el_informe_recoge_la_nota_de_la_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let resultados = tanda_de_ejemplo();
        guardar(&resultados, dir.path()).unwrap();
        std::fs::create_dir_all(dir.path().join("rubricas")).unwrap();
        let rubrica =
            componer_fichero("E01", 42, "respuesta", "rúbrica").replace("Nota: \n", "Nota: 4\n");
        std::fs::write(
            dir.path()
                .join("rubricas")
                .join(nombre_del_fichero("E01", 42)),
            rubrica,
        )
        .unwrap();

        let pendientes = rehacer_informe(&dir.path().join(NOMBRE_RESULTADOS)).unwrap();

        assert_eq!(pendientes, 0);
        let informe = std::fs::read_to_string(dir.path().join(NOMBRE_INFORME)).unwrap();
        assert!(informe.contains("| criterio | 4,0 |"));
        assert!(!informe.contains("Quedan"));
    }
}
