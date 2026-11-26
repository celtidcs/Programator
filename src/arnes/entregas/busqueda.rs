//! Busca en el proyecto los ficheros con el nombre de alguna propuesta y calcula su huella. Solo
//! lee: es la parte de E/S de la deducción (`deduccion`).
//!
//! Se salta `.gestor/` y `.git/`, y lo que ignore el `.gitignore` del proyecto
//! (`arnes::lista_del_proyecto`). Cada fichero encontrado se confina como cualquier lectura:
//! dentro de la carpeta de trabajo y sin seguir enlaces.

use super::deduccion::Encontrado;
use super::huella;
use crate::arnes::lista_del_proyecto::listar;
use crate::error::Resultado;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

/// Las carpetas que nunca se miran: la de los agentes y la de git.
const CARPETAS_SALTADAS: [&str; 2] = [".gestor", ".git"];

/// Los ficheros de `raiz` cuyo nombre está en `nombres`, con su huella. `tiempo` es lo que se
/// espera a git.
pub fn buscar(
    raiz: &Path,
    nombres: &BTreeSet<String>,
    tiempo: Duration,
) -> Resultado<Vec<Encontrado>> {
    if nombres.is_empty() {
        return Ok(Vec::new());
    }
    let rutas = listar(raiz, &CARPETAS_SALTADAS, tiempo)?;
    let raiz_real = raiz.canonicalize().unwrap_or_else(|_| raiz.to_path_buf());
    Ok(de_la_lista(&raiz_real, rutas, nombres))
}

/// De una lista de rutas relativas, los ficheros con alguno de los nombres que se dejan leer.
fn de_la_lista(
    raiz_real: &Path,
    rutas: Vec<String>,
    nombres: &BTreeSet<String>,
) -> Vec<Encontrado> {
    rutas
        .into_iter()
        .filter(|ruta| nombres.contains(fichero_de(ruta)))
        .filter_map(|ruta| encontrado(raiz_real, ruta))
        .collect()
}

/// El fichero, si se deja leer dentro de la carpeta de trabajo: no un enlace, no fuera, texto.
fn encontrado(raiz_real: &Path, ruta: String) -> Option<Encontrado> {
    let completa = raiz_real.join(&ruta);
    let datos = std::fs::symlink_metadata(&completa).ok()?;
    if !datos.is_file() {
        return None;
    }
    if !completa.canonicalize().ok()?.starts_with(raiz_real) {
        return None;
    }
    let contenido = std::fs::read_to_string(&completa).ok()?;
    Some(Encontrado {
        huella: huella(&contenido),
        ruta: ruta.replace('\\', "/"),
    })
}

fn fichero_de(ruta: &str) -> &str {
    ruta.rsplit(['/', '\\']).next().unwrap_or(ruta)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nombres(lista: &[&str]) -> BTreeSet<String> {
        lista.iter().map(|n| n.to_string()).collect()
    }

    /// Lo de `.gestor/` y `.git/` no se encuentra; lo demás, a cualquier profundidad.
    #[test]
    fn salta_gestor_y_git() {
        let carpeta = tempfile::tempdir().unwrap();
        let raiz = carpeta.path();
        std::fs::create_dir_all(raiz.join("src/hondo")).unwrap();
        std::fs::create_dir_all(raiz.join(".gestor/candidatos")).unwrap();
        std::fs::create_dir_all(raiz.join(".git")).unwrap();
        std::fs::write(raiz.join("src/hondo/x.py"), "a\r\n").unwrap();
        std::fs::write(raiz.join(".gestor/candidatos/x.py"), "a\n").unwrap();
        std::fs::write(raiz.join(".git/x.py"), "a\n").unwrap();

        let hallados = buscar(raiz, &nombres(&["x.py"]), Duration::from_secs(5)).unwrap();

        assert_eq!(
            hallados,
            [Encontrado {
                ruta: "src/hondo/x.py".to_string(),
                huella: huella("a\n"),
            }]
        );
    }

    #[test]
    fn sin_nombres_que_buscar_no_mira_nada() {
        let carpeta = tempfile::tempdir().unwrap();
        assert!(
            buscar(carpeta.path(), &BTreeSet::new(), Duration::from_secs(1))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn busca_solo_los_nombres_pedidos() {
        let carpeta = tempfile::tempdir().unwrap();
        std::fs::write(carpeta.path().join("x.py"), "a\n").unwrap();
        std::fs::write(carpeta.path().join("y.py"), "b\n").unwrap();

        let hallados = buscar(carpeta.path(), &nombres(&["x.py"]), Duration::from_secs(5)).unwrap();

        assert_eq!(hallados.len(), 1);
        assert_eq!(hallados[0].ruta, "x.py");
    }

    #[cfg(unix)]
    #[test]
    fn no_sigue_enlaces_que_salen_del_proyecto() {
        let carpeta = tempfile::tempdir().unwrap();
        let fuera = tempfile::tempdir().unwrap();
        std::fs::write(fuera.path().join("x.py"), "a\n").unwrap();
        std::os::unix::fs::symlink(fuera.path().join("x.py"), carpeta.path().join("x.py")).unwrap();

        let raiz = carpeta.path().canonicalize().unwrap();
        assert_eq!(encontrado(&raiz, "x.py".to_string()), None);
    }
}
