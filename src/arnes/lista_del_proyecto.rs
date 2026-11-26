//! Qué ficheros tiene el proyecto, sin lo que ignora su `.gitignore` ni las carpetas que se piden
//! saltar. Solo lee. Lo usan la deducción de las entregas (`entregas::busqueda`) y la copia de
//! `preparar:` (`preparacion::copia`).
//!
//! El proyecto no tiene un lector de `.gitignore`: con git, la lista sale de una sola orden,
//! `git ls-files --cached --others --exclude-standard`, que lo respeta; sin git, o si la carpeta no
//! es un repositorio, se recorre la carpeta sin seguir enlaces. En los dos casos se quitan las rutas
//! que pasan por una carpeta saltada, a cualquier profundidad: git puede listar `.gestor/` si el
//! proyecto no la ignora.

use crate::error::Resultado;
use crate::orden_acotada::{ejecutar_con_tope, DesenlaceDeOrden};
use std::path::Path;
use std::time::Duration;

/// El programa con que se lista lo que no ignora el `.gitignore`.
const PROGRAMA_GIT: &str = "git";

/// Los argumentos de la lista: lo versionado y lo nuevo, sin lo ignorado, separado por NUL.
const ARGUMENTOS_DE_LA_LISTA: [&str; 5] = [
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "-z",
];

/// Los ficheros de `raiz`, como rutas relativas con `/`, fuera de las carpetas `saltadas`. `tiempo`
/// es lo que se espera a git.
pub fn listar(raiz: &Path, saltadas: &[&str], tiempo: Duration) -> Resultado<Vec<String>> {
    let rutas = match listar_con_git(raiz, tiempo) {
        Some(rutas) => rutas,
        None => listar_recorriendo(raiz, saltadas)?,
    };
    Ok(sin_las_saltadas(rutas, saltadas))
}

/// Las rutas que no pasan por ninguna carpeta saltada. Puro.
pub fn sin_las_saltadas(rutas: Vec<String>, saltadas: &[&str]) -> Vec<String> {
    rutas
        .into_iter()
        .map(|ruta| ruta.replace('\\', "/"))
        .filter(|ruta| !pasa_por(ruta, saltadas))
        .collect()
}

fn pasa_por(ruta: &str, saltadas: &[&str]) -> bool {
    let tramos: Vec<&str> = ruta.split('/').collect();
    let carpetas = &tramos[..tramos.len().saturating_sub(1)];
    carpetas.iter().any(|tramo| saltadas.contains(tramo))
}

/// La lista de git, o `None` si git no está o la carpeta no es un repositorio.
fn listar_con_git(raiz: &Path, tiempo: Duration) -> Option<Vec<String>> {
    let argumentos: Vec<String> = ARGUMENTOS_DE_LA_LISTA
        .iter()
        .map(|a| a.to_string())
        .collect();
    match ejecutar_con_tope(PROGRAMA_GIT, &argumentos, raiz, tiempo) {
        DesenlaceDeOrden::Terminada {
            exito: true,
            salida,
            ..
        } => Some(
            salida
                .split('\0')
                .filter(|r| !r.is_empty())
                .map(str::to_string)
                .collect(),
        ),
        _ => None,
    }
}

/// Recorre la carpeta sin entrar en las saltadas ni seguir enlaces. Rutas relativas, con `/`.
fn listar_recorriendo(raiz: &Path, saltadas: &[&str]) -> Resultado<Vec<String>> {
    let mut rutas = Vec::new();
    let mut pendientes = vec![String::new()];
    while let Some(relativa) = pendientes.pop() {
        let carpeta = if relativa.is_empty() {
            raiz.to_path_buf()
        } else {
            raiz.join(&relativa)
        };
        let Ok(entradas) = std::fs::read_dir(&carpeta) else {
            continue;
        };
        for entrada in entradas.flatten() {
            let nombre = entrada.file_name().to_string_lossy().to_string();
            let ruta = if relativa.is_empty() {
                nombre.clone()
            } else {
                format!("{relativa}/{nombre}")
            };
            let Ok(tipo) = entrada.file_type() else {
                continue;
            };
            if tipo.is_dir() && !saltadas.contains(&nombre.as_str()) {
                pendientes.push(ruta);
            } else if tipo.is_file() {
                rutas.push(ruta);
            }
        }
    }
    Ok(rutas)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const SALTADAS: [&str; 2] = [".gestor", ".git"];

    #[test]
    fn sin_git_recorre_la_carpeta_y_salta_las_pedidas() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let raiz = carpeta_temporal.path();
        std::fs::create_dir_all(raiz.join("src/hondo")).unwrap();
        std::fs::create_dir_all(raiz.join(".gestor/candidatos")).unwrap();
        std::fs::create_dir_all(raiz.join(".git")).unwrap();
        std::fs::write(raiz.join("src/hondo/x.py"), "a").unwrap();
        std::fs::write(raiz.join(".gestor/candidatos/x.py"), "a").unwrap();
        std::fs::write(raiz.join(".git/x.py"), "a").unwrap();

        let mut rutas = listar_recorriendo(raiz, &SALTADAS).unwrap();
        rutas.sort();

        assert_eq!(rutas, ["src/hondo/x.py"]);
    }

    /// Lo que liste git dentro de una carpeta saltada no cuenta, esté donde esté; un fichero que
    /// se llame como ella, sí.
    #[test]
    fn se_quitan_las_rutas_que_pasan_por_una_saltada() {
        let lista = vec![
            ".gestor/candidatos/programator/x.py".to_string(),
            "sub\\.git\\x.py".to_string(),
            "x.py".to_string(),
            "doc/.gestor".to_string(),
        ];

        assert_eq!(sin_las_saltadas(lista, &SALTADAS), ["x.py", "doc/.gestor"]);
    }

    /// Con git, lo que ignora el `.gitignore` no sale en la lista y lo nuevo sí, aunque no esté
    /// añadido. Sin git, que es un caso real en Windows (un equipo sin git instalado), se
    /// comprueba lo que pasa entonces: no hay lista de git y se recorre la carpeta entera, con lo
    /// ignorado dentro, porque nadie lee el `.gitignore`. La salida de la prueba dice qué caso vio.
    #[test]
    fn con_git_lo_ignorado_no_sale_y_lo_nuevo_si() {
        let carpeta = tempfile::tempdir().unwrap();
        let raiz = carpeta.path();
        std::fs::write(raiz.join(".gitignore"), "ignorado.txt\n").unwrap();
        std::fs::write(raiz.join("ignorado.txt"), "a").unwrap();
        std::fs::write(raiz.join("nuevo.rs"), "a").unwrap();
        let iniciado = std::process::Command::new(PROGRAMA_GIT)
            .args(["init", "-q"])
            .current_dir(raiz)
            .status();

        let Ok(iniciado) = iniciado else {
            eprintln!("sin git en este equipo: se comprueba el recorrido, no la lista de git");
            assert_eq!(listar_con_git(raiz, Duration::from_secs(30)), None);
            let mut rutas = listar(raiz, &[], Duration::from_secs(30)).unwrap();
            rutas.sort();
            assert_eq!(rutas, [".gitignore", "ignorado.txt", "nuevo.rs"]);
            return;
        };
        assert!(iniciado.success());

        let mut rutas = listar_con_git(raiz, Duration::from_secs(30)).unwrap();
        rutas.sort();

        assert_eq!(rutas, [".gitignore", "nuevo.rs"]);
    }
}
