//! Qué bash ejecuta las fichas cuando nadie lo ha escrito en `[comprobacion_local]`.
//!
//! El de por defecto en Windows es el de Git for Windows en su carpeta de siempre. Si Git está en
//! otra carpeta, o es el portable, ese bash no existe; pero `git.exe` sí está en el PATH, y el bash
//! va con él. Lo escrito en la configuración manda siempre.
//!
//! La decisión es pura ([`resolver`]) y se prueba sin disco. La búsqueda en el PATH y la
//! comprobación de que existe cada candidato van aparte.

use std::path::{Path, PathBuf};

use crate::banco::config::ComprobacionLocal;
use crate::banco::mensajes;

/// Cómo se llama Git en el PATH.
#[cfg(windows)]
const PROGRAMA_GIT: &str = "git.exe";
#[cfg(not(windows))]
const PROGRAMA_GIT: &str = "git";
/// El bash que trae Git for Windows, en la carpeta `bin` de su instalación.
const BASH_DE_GIT: &str = "bin/bash.exe";
/// Cuántas carpetas por encima de la de `git.exe` puede estar la raíz de Git: `git.exe` está en
/// `cmd\` en la instalación normal y en `mingw64\bin\` en la portable.
const NIVELES_HASTA_LA_RAIZ_DE_GIT: [usize; 2] = [1, 2];

/// De dónde salió el intérprete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origen {
    /// Lo escribió quien configuró el banco.
    Escrito,
    /// El de por defecto, sin buscar nada.
    PorDefecto,
    /// El de por defecto no existía y se encontró junto a `git`.
    JuntoAGit(PathBuf),
}

/// El intérprete que se usa y de dónde sale.
///
/// Solo se busca junto a Git si no hay nada escrito y el programa de por defecto es una ruta
/// absoluta que no existe: un `bash` a secas lo busca el sistema en el PATH.
pub fn resolver(
    escrito: Option<&[String]>,
    por_defecto: &[String],
    existe: impl Fn(&Path) -> bool,
    git: Option<&Path>,
) -> (Vec<String>, Origen) {
    if let Some(escrito) = escrito {
        return (escrito.to_vec(), Origen::Escrito);
    }
    let Some(programa) = por_defecto.first().map(Path::new) else {
        return (por_defecto.to_vec(), Origen::PorDefecto);
    };
    if !programa.is_absolute() || existe(programa) {
        return (por_defecto.to_vec(), Origen::PorDefecto);
    }
    let encontrado = git
        .map(candidatos_junto_a_git)
        .unwrap_or_default()
        .into_iter()
        .find(|candidato| existe(candidato));
    match encontrado {
        Some(bash) => {
            let mut interprete = vec![bash.display().to_string()];
            interprete.extend(por_defecto.iter().skip(1).cloned());
            (interprete, Origen::JuntoAGit(bash))
        }
        None => (por_defecto.to_vec(), Origen::PorDefecto),
    }
}

/// Dónde puede estar el bash de Git según dónde está `git.exe`.
fn candidatos_junto_a_git(git: &Path) -> Vec<PathBuf> {
    NIVELES_HASTA_LA_RAIZ_DE_GIT
        .iter()
        .filter_map(|niveles| git.ancestors().nth(niveles + 1))
        .map(|raiz| raiz.join(BASH_DE_GIT))
        .collect()
}

/// El primer `programa` que aparece en las carpetas del PATH.
fn buscar_en_el_path(programa: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|carpeta| carpeta.join(programa))
        .find(|candidato| candidato.is_file())
}

/// `local` con el intérprete que se usará en este equipo, y el aviso que hay que dar si se
/// encontró junto a Git.
pub fn en_este_equipo(local: &ComprobacionLocal) -> (ComprobacionLocal, Option<String>) {
    let git = buscar_en_el_path(PROGRAMA_GIT);
    let (interprete, origen) = resolver(
        local.interprete.as_deref(),
        &local.interprete_efectivo(),
        |ruta| ruta.is_file(),
        git.as_deref(),
    );
    let aviso = match &origen {
        Origen::JuntoAGit(bash) => Some(mensajes::mensaje_bash_junto_a_git(bash)),
        Origen::Escrito | Origen::PorDefecto => None,
    };
    let mut resuelto = local.clone();
    resuelto.interprete = Some(interprete);
    (resuelto, aviso)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn textos(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Una ruta absoluta en esta plataforma: en Windows, `/x` no lo es sin unidad.
    fn absoluta(ruta: &str) -> PathBuf {
        let raiz = if cfg!(windows) { "C:/" } else { "/" };
        Path::new(raiz).join(ruta)
    }

    fn por_defecto() -> Vec<String> {
        vec![
            absoluta("opt/Git/bin/bash.exe").display().to_string(),
            "-c".into(),
        ]
    }

    #[test]
    fn lo_escrito_manda_aunque_no_exista() {
        let escrito = textos(&["/otro/bash", "-c"]);

        let (interprete, origen) = resolver(Some(&escrito), &por_defecto(), |_| false, None);

        assert_eq!(interprete, escrito);
        assert_eq!(origen, Origen::Escrito);
    }

    #[test]
    fn si_el_de_por_defecto_existe_no_se_busca() {
        let (interprete, origen) = resolver(
            None,
            &por_defecto(),
            |_| true,
            Some(&absoluta("x/cmd/git.exe")),
        );

        assert_eq!(interprete, por_defecto());
        assert_eq!(origen, Origen::PorDefecto);
    }

    #[test]
    fn un_programa_sin_ruta_no_se_busca() {
        let (interprete, origen) = resolver(None, &textos(&["bash", "-c"]), |_| false, None);

        assert_eq!(interprete, textos(&["bash", "-c"]));
        assert_eq!(origen, Origen::PorDefecto);
    }

    #[test]
    fn se_encuentra_junto_al_git_de_una_instalacion_normal() {
        let esperado = absoluta("d/Git").join(BASH_DE_GIT);
        let existe = |ruta: &Path| ruta == esperado;

        let (interprete, origen) = resolver(
            None,
            &por_defecto(),
            existe,
            Some(&absoluta("d/Git/cmd/git.exe")),
        );

        assert_eq!(
            interprete,
            vec![esperado.display().to_string(), "-c".into()]
        );
        assert_eq!(origen, Origen::JuntoAGit(esperado.clone()));
    }

    #[test]
    fn se_encuentra_junto_al_git_portable() {
        let esperado = absoluta("p/Git").join(BASH_DE_GIT);
        let existe = |ruta: &Path| ruta == esperado;

        let (_, origen) = resolver(
            None,
            &por_defecto(),
            existe,
            Some(&absoluta("p/Git/mingw64/bin/git.exe")),
        );

        assert_eq!(origen, Origen::JuntoAGit(esperado.clone()));
    }

    #[test]
    fn sin_git_ni_bash_queda_el_de_por_defecto_y_lo_dira_la_pregunta() {
        let (interprete, origen) = resolver(None, &por_defecto(), |_| false, None);

        assert_eq!(interprete, por_defecto());
        assert_eq!(origen, Origen::PorDefecto);
    }
}
