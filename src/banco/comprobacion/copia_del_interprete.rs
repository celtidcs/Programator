//! La copia de Git con que se comprueba en Windows con `integridad_baja`.
//!
//! El bash de Git (msys) crea, al arrancar, un directorio de objetos de Windows que comparten
//! todos los procesos de esa instalación, con un nombre que sale de su ruta. Si lo creó un bash
//! normal, de integridad media, uno de integridad baja no puede abrirlo y muere con
//! «NtCreateDirectoryObject … 0xC0000022». Y en un equipo donde se trabaja con agentes casi siempre
//! hay un bash normal abierto. Una copia de Git en otra carpeta tiene su propio directorio de
//! objetos, y como solo la usa la comprobación, nadie lo crea con integridad media (medido en una
//! máquina Windows de pruebas el 13/11/2026: con un bash normal abierto, la copia termina la tanda
//! y la instalación de siempre no).
//!
//! Se copia una vez y se vuelve a copiar solo si cambia el Git de origen. Lo que decide es puro
//! ([`raiz_de_git`], [`hay_que_copiar`], [`marca_de`]); la copia y la lectura del disco van aparte.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::banco::config::{Aislamiento, ComprobacionLocal};
use crate::banco::mensajes;
use crate::error::{Error, Resultado};
use crate::ficheros::{
    borrar_si_existe, copiar_arbol, crear_carpeta, escribir, leer_si_existe, renombrar,
};

/// Dónde está el bash dentro de una instalación de Git for Windows.
const BASH_EN_GIT: [&str; 2] = ["bin", "bash.exe"];
/// Las carpetas de Git que hacen falta para que su bash y sus utilidades funcionen: el lanzador
/// (`bin`), la configuración de msys (`etc`) y el propio msys con sus programas (`usr`). `ucrt64` o
/// `mingw64`, que traen `git` y no hacen falta para ejecutar las fichas, no se copian: son cientos
/// de MB.
const CARPETAS_QUE_SE_COPIAN: [&str; 3] = ["bin", "etc", "usr"];
/// Las que msys espera encontrar en su raíz aunque estén vacías.
const CARPETAS_QUE_SE_CREAN: [&str; 2] = ["tmp", "dev"];
/// Los entornos de MinGW que puede traer Git for Windows. Su lanzador `bin\bash.exe` reconoce la
/// raíz de la instalación porque existe `<entorno>\bin`; sin ella dice «Top-level not found» (medido
/// el 13/11/2026 con Git 2.5x en `ucrt64`: con la carpeta vacía arranca y pone `MSYSTEM=UCRT64`).
/// En la copia se crea vacía la de cada entorno que tenga el origen; su contenido no hace falta.
const ENTORNOS_DE_MINGW: [&str; 5] = ["ucrt64", "mingw64", "clang64", "clangarm64", "mingw32"];
/// La carpeta de programas de cada entorno de MinGW.
const CARPETA_DE_PROGRAMAS: &str = "bin";
/// La forma de la copia. Sube cuando cambia lo que se copia o se crea, para que una copia hecha
/// con una forma anterior se rehaga sola.
const FORMA_DE_LA_COPIA: u32 = 2;
/// La biblioteca de msys: si cambia, cambió el Git de origen y hay que volver a copiar.
const BIBLIOTECA_DE_MSYS: [&str; 3] = ["usr", "bin", "msys-2.0.dll"];
/// El fichero de la copia que dice de qué Git se sacó.
const NOMBRE_DE_LA_MARCA: &str = "origen-de-la-copia.txt";
/// La carpeta provisional donde se copia antes de dar la copia por buena.
const SUFIJO_PROVISIONAL: &str = ".a-medias";

/// La raíz de la instalación de Git a la que pertenece `bash`, si `bash` es su `bin\bash.exe`.
pub fn raiz_de_git(bash: &Path) -> Option<PathBuf> {
    let nombre = bash.file_name()?.to_str()?;
    let carpeta = bash.parent()?;
    let nombre_de_la_carpeta = carpeta.file_name()?.to_str()?;
    let es_el_de_git = nombre.eq_ignore_ascii_case(BASH_EN_GIT[1])
        && nombre_de_la_carpeta.eq_ignore_ascii_case(BASH_EN_GIT[0]);
    es_el_de_git
        .then(|| carpeta.parent().map(Path::to_path_buf))
        .flatten()
}

/// La marca de un Git: la forma de la copia, su ruta y el tamaño y la fecha de su biblioteca de
/// msys.
pub fn marca_de(raiz: &Path, tamano: u64, segundos: u64) -> String {
    format!(
        "forma = {FORMA_DE_LA_COPIA}\norigen = {}\ntamano = {tamano}\nmodificado = {segundos}\n",
        raiz.display()
    )
}

/// Los entornos de MinGW de `raiz` cuya carpeta de programas hay que crear vacía en la copia.
fn entornos_de(raiz: &Path) -> Vec<PathBuf> {
    ENTORNOS_DE_MINGW
        .iter()
        .map(|entorno| Path::new(entorno).join(CARPETA_DE_PROGRAMAS))
        .filter(|carpeta| raiz.join(carpeta).is_dir())
        .collect()
}

/// La carpeta de la copia, absoluta: el bash se lanza desde otra carpeta de trabajo y una ruta
/// relativa no lo encontraría.
fn carpeta_de_la_copia(carpeta_del_banco: &Path, copia: &str) -> Resultado<PathBuf> {
    let ruta = carpeta_del_banco.join(copia);
    std::path::absolute(&ruta).map_err(|causa| Error::Lectura { ruta, causa })
}

/// Si hay que copiar: no hay copia, o la que hay se sacó de otro Git o de otra versión.
pub fn hay_que_copiar(marca_actual: &str, marca_guardada: &str) -> bool {
    marca_actual != marca_guardada
}

/// El bash de una copia hecha en `copia`.
pub fn bash_de_la_copia(copia: &Path) -> PathBuf {
    copia.join(BASH_EN_GIT[0]).join(BASH_EN_GIT[1])
}

/// Dónde está la biblioteca de msys en la instalación de Git de `raiz`.
fn biblioteca_de_msys(raiz: &Path) -> PathBuf {
    BIBLIOTECA_DE_MSYS
        .iter()
        .fold(raiz.to_path_buf(), |ruta, parte| ruta.join(parte))
}

/// La marca del Git que hay en `raiz`, leyendo su biblioteca de msys.
fn marca_en_disco(raiz: &Path) -> Resultado<String> {
    let biblioteca = biblioteca_de_msys(raiz);
    let datos = std::fs::metadata(&biblioteca).map_err(|causa| Error::Lectura {
        ruta: biblioteca.clone(),
        causa,
    })?;
    let segundos = datos
        .modified()
        .ok()
        .and_then(|fecha| fecha.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duracion| duracion.as_secs());
    Ok(marca_de(raiz, datos.len(), segundos))
}

/// Copia lo necesario de `raiz` en `copia`. Primero a una carpeta provisional, que se renombra al
/// final: si algo falla a medias, no queda una copia incompleta que parezca buena.
fn copiar(raiz: &Path, copia: &Path, marca: &str) -> Resultado<()> {
    let mut provisional = copia.as_os_str().to_owned();
    provisional.push(SUFIJO_PROVISIONAL);
    let provisional = PathBuf::from(provisional);
    borrar_si_existe(&provisional)?;
    for carpeta in CARPETAS_QUE_SE_COPIAN {
        copiar_arbol(&raiz.join(carpeta), &provisional.join(carpeta))?;
    }
    for carpeta in CARPETAS_QUE_SE_CREAN {
        crear_carpeta(&provisional.join(carpeta))?;
    }
    for carpeta in entornos_de(raiz) {
        crear_carpeta(&provisional.join(carpeta))?;
    }
    escribir(&provisional.join(NOMBRE_DE_LA_MARCA), marca)?;
    borrar_si_existe(copia)?;
    renombrar(&provisional, copia)
}

/// Deja lista la copia de Git de `raiz` en `copia` y devuelve si hubo que copiar.
pub fn asegurar_la_copia(raiz: &Path, copia: &Path) -> Resultado<bool> {
    let marca = marca_en_disco(raiz)?;
    let guardada = leer_si_existe(&copia.join(NOMBRE_DE_LA_MARCA))?;
    if !hay_que_copiar(&marca, &guardada) {
        return Ok(false);
    }
    println!("{}", mensajes::mensaje_copiando_el_interprete(raiz, copia));
    copiar(raiz, copia, &marca)?;
    Ok(true)
}

/// `local` con el bash de la copia, si comprueba con `integridad_baja` y tiene dónde copiar; si
/// no, tal cual. Con el aviso que haya que dar.
pub fn con_la_copia(
    local: &ComprobacionLocal,
    carpeta_del_banco: &Path,
) -> Resultado<(ComprobacionLocal, Option<String>)> {
    if local.aislamiento != Aislamiento::IntegridadBaja || local.copia_del_interprete.is_empty() {
        return Ok((local.clone(), None));
    }
    let interprete = local.interprete_efectivo();
    let Some(programa) = interprete.first() else {
        return Ok((local.clone(), None));
    };
    // Que la ruta acabe en `bin/bash.exe` no basta: el de MSYS2 está en `usr/bin/bash.exe`, y
    // su `usr` no es una instalación de Git. Lo que la distingue es la biblioteca de msys.
    let raiz = raiz_de_git(Path::new(programa)).filter(|raiz| biblioteca_de_msys(raiz).is_file());
    let Some(raiz) = raiz else {
        let aviso = mensajes::mensaje_interprete_sin_copia(programa);
        return Ok((local.clone(), Some(aviso)));
    };
    let copia = carpeta_de_la_copia(carpeta_del_banco, &local.copia_del_interprete)?;
    asegurar_la_copia(&raiz, &copia)?;
    let mut con_copia = local.clone();
    let mut orden = interprete.clone();
    orden[0] = bash_de_la_copia(&copia).display().to_string();
    con_copia.interprete = Some(orden);
    Ok((con_copia, None))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un Git de mentira: lo que se copia, lo que no y la biblioteca de msys.
    fn git_de_mentira(raiz: &Path) {
        for (fichero, contenido) in [
            ("bin/bash.exe", "lanzador"),
            ("etc/profile", "perfil"),
            ("usr/bin/bash.exe", "bash"),
            ("usr/bin/msys-2.0.dll", "msys"),
            ("usr/lib/algo.so", "biblioteca"),
            ("ucrt64/bin/git.exe", "git"),
            ("git-bash.exe", "otro"),
        ] {
            let ruta = raiz.join(fichero);
            std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
            std::fs::write(ruta, contenido).unwrap();
        }
    }

    fn local_aislado(interprete: &Path) -> ComprobacionLocal {
        ComprobacionLocal {
            aislamiento: Aislamiento::IntegridadBaja,
            interprete: Some(vec![interprete.display().to_string(), "-c".into()]),
            ..ComprobacionLocal::default()
        }
    }

    #[test]
    fn la_raiz_de_git_es_la_carpeta_de_encima_de_bin() {
        assert_eq!(
            raiz_de_git(Path::new("C:/Program Files/Git/bin/bash.exe")),
            Some(PathBuf::from("C:/Program Files/Git"))
        );
        assert_eq!(
            raiz_de_git(Path::new("D:/git/BIN/Bash.EXE")),
            Some(PathBuf::from("D:/git"))
        );
        assert_eq!(raiz_de_git(Path::new("C:/Git/usr/bin/sh.exe")), None);
        assert_eq!(raiz_de_git(Path::new("bash")), None);
    }

    #[test]
    fn se_copia_si_no_hay_copia_o_si_cambio_el_git() {
        let marca = marca_de(Path::new("C:/Git"), 10, 20);
        assert!(hay_que_copiar(&marca, ""));
        assert!(!hay_que_copiar(&marca, &marca));
        assert!(hay_que_copiar(
            &marca,
            &marca_de(Path::new("C:/Git"), 11, 20)
        ));
        assert!(hay_que_copiar(
            &marca,
            &marca_de(Path::new("C:/Git"), 10, 21)
        ));
        assert!(hay_que_copiar(
            &marca,
            &marca_de(Path::new("D:/Git"), 10, 20)
        ));
    }

    #[test]
    fn copia_bin_etc_y_usr_crea_tmp_y_dev_y_deja_fuera_lo_demas() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().join("Git");
        git_de_mentira(&raiz);
        let copia = dir.path().join("banco").join("interprete-aislado");

        assert!(asegurar_la_copia(&raiz, &copia).unwrap());

        for presente in [
            "bin/bash.exe",
            "etc/profile",
            "usr/bin/msys-2.0.dll",
            "usr/lib/algo.so",
        ] {
            assert!(copia.join(presente).is_file(), "falta {presente}");
        }
        assert!(copia.join("tmp").is_dir() && copia.join("dev").is_dir());
        // De `ucrt64` solo su carpeta de programas, vacía, para que el lanzador reconozca la raíz;
        // de los entornos que el origen no tiene, nada.
        assert!(copia.join("ucrt64/bin").is_dir());
        assert!(!copia.join("ucrt64/bin/git.exe").exists());
        assert!(!copia.join("mingw64").exists());
        assert!(!copia.join("git-bash.exe").exists());
        let mut provisional = copia.as_os_str().to_owned();
        provisional.push(SUFIJO_PROVISIONAL);
        assert!(!PathBuf::from(provisional).exists());
    }

    #[test]
    fn sin_cambios_no_se_vuelve_a_copiar_y_con_cambios_si() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().join("Git");
        git_de_mentira(&raiz);
        let copia = dir.path().join("copia");
        asegurar_la_copia(&raiz, &copia).unwrap();
        std::fs::write(copia.join("etc/profile"), "tocado").unwrap();

        assert!(!asegurar_la_copia(&raiz, &copia).unwrap());
        assert_eq!(
            std::fs::read_to_string(copia.join("etc/profile")).unwrap(),
            "tocado"
        );

        std::fs::write(raiz.join("usr/bin/msys-2.0.dll"), "msys de otra version").unwrap();
        assert!(asegurar_la_copia(&raiz, &copia).unwrap());
        assert_eq!(
            std::fs::read_to_string(copia.join("etc/profile")).unwrap(),
            "perfil"
        );
    }

    #[test]
    fn con_integridad_baja_se_usa_el_bash_de_la_copia_con_el_resto_de_la_orden() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().join("Git");
        git_de_mentira(&raiz);
        let banco = dir.path().join("banco");

        let (local, aviso) =
            con_la_copia(&local_aislado(&raiz.join("bin/bash.exe")), &banco).unwrap();

        let copia = banco.join("interprete-aislado");
        assert_eq!(aviso, None);
        assert_eq!(
            local.interprete,
            Some(vec![
                bash_de_la_copia(&copia).display().to_string(),
                "-c".into()
            ])
        );
        assert!(bash_de_la_copia(&copia).is_file());
    }

    #[test]
    fn el_bash_de_la_copia_va_en_ruta_absoluta_aunque_el_banco_venga_en_relativa() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().join("Git");
        git_de_mentira(&raiz);
        let local = local_aislado(&raiz.join("bin/bash.exe"));
        let relativa = Path::new("banco-de-una-prueba-de-ruta-relativa");

        let resultado = con_la_copia(&local, relativa);
        let quedo = relativa.exists();
        borrar_si_existe(relativa).unwrap();

        assert!(quedo);
        let (local, _) = resultado.unwrap();
        let bash = PathBuf::from(&local.interprete.unwrap()[0]);
        assert!(bash.is_absolute(), "{}", bash.display());
        assert!(
            bash.ends_with("banco-de-una-prueba-de-ruta-relativa/interprete-aislado/bin/bash.exe")
        );
    }

    #[test]
    fn una_copia_de_una_forma_anterior_se_rehace() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().join("Git");
        git_de_mentira(&raiz);
        let copia = dir.path().join("copia");
        asegurar_la_copia(&raiz, &copia).unwrap();
        let actual = std::fs::read_to_string(copia.join(NOMBRE_DE_LA_MARCA)).unwrap();
        let sin_forma: String = actual
            .lines()
            .filter(|linea| !linea.starts_with("forma"))
            .map(|linea| format!("{linea}\n"))
            .collect();
        std::fs::write(copia.join(NOMBRE_DE_LA_MARCA), sin_forma).unwrap();
        std::fs::remove_dir_all(copia.join("ucrt64")).unwrap();

        assert!(asegurar_la_copia(&raiz, &copia).unwrap());
        assert!(copia.join("ucrt64/bin").is_dir());
    }

    #[test]
    fn con_otro_aislamiento_o_sin_carpeta_no_se_copia_nada() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().join("Git");
        git_de_mentira(&raiz);
        let banco = dir.path().join("banco");
        let aislado = local_aislado(&raiz.join("bin/bash.exe"));
        let sin_aislar = ComprobacionLocal {
            aislamiento: Aislamiento::Ninguno,
            ..aislado.clone()
        };
        let sin_carpeta = ComprobacionLocal {
            copia_del_interprete: String::new(),
            ..aislado
        };

        for local in [sin_aislar, sin_carpeta] {
            let (resultado, aviso) = con_la_copia(&local, &banco).unwrap();
            assert_eq!(resultado, local);
            assert_eq!(aviso, None);
        }
        assert!(!banco.exists());
    }

    #[test]
    fn un_interprete_que_no_es_el_bash_de_git_se_usa_tal_cual_y_se_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let local = local_aislado(Path::new("C:/msys64/usr/bin/bash.exe"));

        let (resultado, aviso) = con_la_copia(&local, dir.path()).unwrap();

        assert_eq!(resultado, local);
        let aviso = aviso.unwrap();
        assert!(aviso.contains("C:/msys64/usr/bin/bash.exe"), "{aviso}");
        assert!(aviso.contains("copia_del_interprete"), "{aviso}");
    }
}
