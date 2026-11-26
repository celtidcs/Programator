//! La copia del proyecto donde se prepara una propuesta: lo que lista git (o el árbol, sin git),
//! hasta un tope de tamaño, con la propuesta en la ruta pedida. El proyecto solo se lee.
//!
//! Se salta lo que no es del proyecto o se regenera (`.gestor/`, `.git/`, `target/`,
//! `node_modules/`) y la propia carpeta de las copias, para no copiar copias. Cada fichero se
//! confina como cualquier lectura: un enlace o algo que resuelva fuera del proyecto no se copia.

use crate::arnes::confinamiento::Ambito;
use crate::arnes::lista_del_proyecto::listar;
use crate::error::{Error, Resultado};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Lo que nunca se copia: lo de los agentes, git y lo que se regenera al compilar.
pub const CARPETAS_SALTADAS: [&str; 4] = [".gestor", ".git", "target", "node_modules"];

/// Lo que hace falta para una copia.
#[derive(Debug, Clone, Copy)]
pub struct PedidoDeCopia<'a> {
    /// La carpeta de trabajo.
    pub raiz: &'a Path,
    /// Donde van todas las copias (`[preparacion] carpeta`); sus ficheros no se copian.
    pub carpeta_de_las_copias: &'a Path,
    /// La carpeta de esta copia, dentro de la anterior. Lo que hubiera en ella se borra.
    pub destino: &'a Path,
    /// Dónde va la propuesta, relativa al proyecto.
    pub ruta: &'a str,
    pub contenido: &'a str,
    /// Lo más que puede ocupar lo copiado, en bytes.
    pub tope_bytes: u64,
    /// Lo que se espera a git al listar.
    pub tiempo: Duration,
}

/// En qué quedó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Copia {
    Hecha {
        /// La propuesta, ya en su sitio dentro de la copia.
        fichero: PathBuf,
        /// Lo que había en el proyecto en esa ruta, si había un fichero de texto.
        anterior: Option<String>,
        ficheros: usize,
        bytes: u64,
    },
    /// Lo que se copiaría pasa del tope: no se ha copiado nada.
    PasaDelTope { bytes: u64, tope: u64 },
    /// La ruta pedida sale del proyecto o cae en lo que no se copia.
    FueraDelProyecto,
}

/// Hace la copia, o dice por qué no.
pub fn copiar(pedido: &PedidoDeCopia) -> Resultado<Copia> {
    let ambito = Ambito::nuevo(pedido.raiz)?;
    let raiz = ambito.raiz();
    let en_el_proyecto = match ambito.resolver(pedido.ruta) {
        Ok(ruta) => ruta,
        Err(Error::FueraDeAmbito(_)) => return Ok(Copia::FueraDelProyecto),
        Err(otro) => return Err(otro),
    };
    let Some(relativa) = relativa_si_se_copia(raiz, &en_el_proyecto) else {
        return Ok(Copia::FueraDelProyecto);
    };

    let de_las_copias = relativa_a(raiz, pedido.carpeta_de_las_copias);
    let ficheros: Vec<(String, u64)> = listar(raiz, &CARPETAS_SALTADAS, pedido.tiempo)?
        .into_iter()
        .filter(|ruta| !de_las_copias.as_deref().is_some_and(|c| dentro_de(ruta, c)))
        .filter_map(|ruta| tamano_si_se_copia(raiz, &ruta).map(|bytes| (ruta, bytes)))
        .collect();
    let bytes: u64 = ficheros.iter().map(|(_, bytes)| bytes).sum();
    if bytes > pedido.tope_bytes {
        return Ok(Copia::PasaDelTope {
            bytes,
            tope: pedido.tope_bytes,
        });
    }

    crate::ficheros::borrar_si_existe(pedido.destino)?;
    crate::ficheros::crear_carpeta(pedido.destino)?;
    for (ruta, _) in &ficheros {
        let destino = pedido.destino.join(ruta);
        if let Some(padre) = destino.parent() {
            crate::ficheros::crear_carpeta(padre)?;
        }
        std::fs::copy(raiz.join(ruta), &destino).map_err(|causa| Error::Escritura {
            ruta: destino.clone(),
            causa,
        })?;
    }

    let anterior = std::fs::symlink_metadata(&en_el_proyecto)
        .is_ok_and(|datos| datos.is_file())
        .then(|| std::fs::read_to_string(&en_el_proyecto).ok())
        .flatten();
    let fichero = pedido.destino.join(&relativa);
    if let Some(padre) = fichero.parent() {
        crate::ficheros::crear_carpeta(padre)?;
    }
    crate::ficheros::escribir(&fichero, pedido.contenido)?;
    Ok(Copia::Hecha {
        fichero,
        anterior,
        ficheros: ficheros.len(),
        bytes,
    })
}

/// La ruta relativa, con `/`, si cae en lo que se copia: dentro del proyecto, no en la raíz misma y
/// fuera de las carpetas saltadas.
fn relativa_si_se_copia(raiz: &Path, ruta: &Path) -> Option<String> {
    let relativa = relativa_a(raiz, ruta)?;
    let tramos: Vec<&str> = relativa.split('/').collect();
    let saltada = tramos[..tramos.len().saturating_sub(1)]
        .iter()
        .any(|tramo| CARPETAS_SALTADAS.contains(tramo));
    (!relativa.is_empty() && !saltada).then_some(relativa)
}

fn relativa_a(raiz: &Path, ruta: &Path) -> Option<String> {
    let ruta = ruta.canonicalize().unwrap_or_else(|_| ruta.to_path_buf());
    let resto = ruta.strip_prefix(raiz).ok()?;
    Some(
        resto
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn dentro_de(ruta: &str, carpeta: &str) -> bool {
    !carpeta.is_empty() && (ruta == carpeta || ruta.starts_with(&format!("{carpeta}/")))
}

/// Lo que ocupa, si es un fichero de verdad dentro del proyecto: ni enlace ni algo que resuelva
/// fuera.
fn tamano_si_se_copia(raiz: &Path, ruta: &str) -> Option<u64> {
    let completa = raiz.join(ruta);
    let datos = std::fs::symlink_metadata(&completa).ok()?;
    if !datos.is_file() || !completa.canonicalize().ok()?.starts_with(raiz) {
        return None;
    }
    Some(datos.len())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::collections::BTreeMap;

    const TIEMPO: Duration = Duration::from_secs(10);

    /// Cada fichero del árbol con su huella, `.gestor` incluido: lo que no puede cambiar.
    fn huellas(raiz: &Path) -> BTreeMap<PathBuf, String> {
        let mut huellas = BTreeMap::new();
        let mut pendientes = vec![raiz.to_path_buf()];
        while let Some(carpeta) = pendientes.pop() {
            for entrada in std::fs::read_dir(&carpeta).unwrap().flatten() {
                let ruta = entrada.path();
                if ruta.is_dir() {
                    pendientes.push(ruta);
                } else {
                    let huella = crate::ficheros::huella_sha256(&ruta).unwrap();
                    huellas.insert(ruta.strip_prefix(raiz).unwrap().to_path_buf(), huella);
                }
            }
        }
        huellas
    }

    fn proyecto() -> tempfile::TempDir {
        let carpeta = tempfile::tempdir().unwrap();
        let raiz = carpeta.path();
        std::fs::create_dir_all(raiz.join("src")).unwrap();
        std::fs::create_dir_all(raiz.join("target/debug")).unwrap();
        std::fs::create_dir_all(raiz.join(".gestor/canal")).unwrap();
        std::fs::write(raiz.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        std::fs::write(raiz.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
        std::fs::write(raiz.join("target/debug/x"), "binario").unwrap();
        std::fs::write(raiz.join(".gestor/canal/estado.md"), "# Estado\n").unwrap();
        carpeta
    }

    fn pedido<'a>(
        raiz: &'a Path,
        destino: &'a Path,
        ruta: &'a str,
        tope: u64,
    ) -> PedidoDeCopia<'a> {
        PedidoDeCopia {
            raiz,
            carpeta_de_las_copias: destino.parent().unwrap(),
            destino,
            ruta,
            contenido: "pub fn b() {}\n",
            tope_bytes: tope,
            tiempo: TIEMPO,
        }
    }

    /// F1: tras copiar, el proyecto queda idéntico; la copia tiene lo del proyecto sin lo saltado
    /// y la propuesta en su ruta, y se sabe qué había antes.
    #[test]
    fn la_copia_no_toca_el_proyecto_y_lleva_la_propuesta() {
        let carpeta = proyecto();
        let raiz = carpeta.path().canonicalize().unwrap();
        let destino = raiz.join(".gestor/programator/preparacion/uno");
        let antes = huellas(&raiz);

        let copia = copiar(&pedido(&raiz, &destino, "src/lib.rs", 1 << 20)).unwrap();

        let despues: BTreeMap<_, _> = huellas(&raiz)
            .into_iter()
            .filter(|(ruta, _)| !ruta.starts_with(".gestor/programator"))
            .collect();
        assert_eq!(antes, despues);
        let Copia::Hecha {
            fichero,
            anterior,
            ficheros,
            ..
        } = copia
        else {
            panic!("{copia:?}");
        };
        assert_eq!(ficheros, 2, "Cargo.toml y src/lib.rs");
        assert_eq!(fichero, destino.join("src/lib.rs"));
        assert_eq!(
            std::fs::read_to_string(&fichero).unwrap(),
            "pub fn b() {}\n"
        );
        assert_eq!(anterior.as_deref(), Some("pub fn a() {}\n"));
        assert!(destino.join("Cargo.toml").exists());
        assert!(!destino.join("target").exists());
        assert!(!destino.join(".gestor").exists());
    }

    #[test]
    fn un_fichero_nuevo_no_tiene_anterior() {
        let carpeta = proyecto();
        let raiz = carpeta.path().canonicalize().unwrap();
        let destino = raiz.join(".gestor/programator/preparacion/dos");

        let copia = copiar(&pedido(&raiz, &destino, "src/nuevo.rs", 1 << 20)).unwrap();

        assert!(
            matches!(copia, Copia::Hecha { anterior: None, .. }),
            "{copia:?}"
        );
        assert!(destino.join("src/nuevo.rs").exists());
    }

    /// F2: lo que sale del proyecto, o cae en lo que no se copia, se deniega sin copiar.
    #[test]
    fn una_ruta_fuera_del_proyecto_se_deniega() {
        let carpeta = proyecto();
        let raiz = carpeta.path().canonicalize().unwrap();
        let destino = raiz.join(".gestor/programator/preparacion/tres");
        let fuera = std::env::temp_dir().join("fuera.rs");
        let fuera = fuera.to_string_lossy().to_string();

        for ruta in [
            "../fuera.rs",
            "src/../../fuera.rs",
            fuera.as_str(),
            ".gestor/x.rs",
            ".git/x",
            "",
        ] {
            assert_eq!(
                copiar(&pedido(&raiz, &destino, ruta, 1 << 20)).unwrap(),
                Copia::FueraDelProyecto,
                "{ruta}"
            );
        }
        assert!(!destino.exists());
    }

    /// F4: por encima del tope no se copia nada.
    #[test]
    fn por_encima_del_tope_no_se_copia_nada() {
        let carpeta_temporal = proyecto();
        let raiz = carpeta_temporal.path().canonicalize().unwrap();
        let destino = raiz.join(".gestor/programator/preparacion/cuatro");

        let copia = copiar(&pedido(&raiz, &destino, "src/lib.rs", 10)).unwrap();

        assert!(
            matches!(copia, Copia::PasaDelTope { tope: 10, .. }),
            "{copia:?}"
        );
        assert!(!destino.exists());
    }

    /// Con la carpeta de las copias fuera de `.gestor`, las copias anteriores no se copian.
    #[test]
    fn las_copias_anteriores_no_se_copian() {
        let carpeta = proyecto();
        let raiz = carpeta.path().canonicalize().unwrap();
        let copias = raiz.join("copias");
        std::fs::create_dir_all(copias.join("vieja")).unwrap();
        std::fs::write(copias.join("vieja/grande.bin"), "x".repeat(4096)).unwrap();
        let destino = copias.join("nueva");

        let copia = copiar(&pedido(&raiz, &destino, "src/lib.rs", 1 << 20)).unwrap();

        assert!(
            matches!(copia, Copia::Hecha { ficheros: 2, .. }),
            "{copia:?}"
        );
        assert!(!destino.join("copias").exists());
    }
}
