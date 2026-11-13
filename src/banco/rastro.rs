//! Lo que la comprobación local deja fuera de su carpeta.
//!
//! Las variables de `[comprobacion_local]` llevan las cachés a la carpeta de la tanda, pero una
//! herramienta puede no hacerles caso. Por eso, antes y después de las pruebas ocultas se toma
//! una foto de las carpetas donde esas herramientas escriben si nadie las redirige, y lo que
//! aparece o cambia entre las dos se cuenta en el informe. No se borra nada: se dice.
//!
//! Lo que decide —qué cambió, qué carpeta resulta de una plantilla— es puro y se prueba sin
//! disco. Solo [`tomar_foto`] lee.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Una carpeta que se vigila, ya con su ruta de este equipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarpetaVigilada {
    pub ruta: PathBuf,
    /// Es el temporal del sistema, que también usan otros programas.
    pub es_temporal: bool,
}

/// Hasta dónde se mira dentro de cada carpeta vigilada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alcance {
    /// Cuántos niveles: 1 es solo lo que cuelga directamente de la carpeta.
    pub profundidad: usize,
    /// Cuántas entradas como mucho por carpeta. Pasado el tope, la carpeta se da por no vigilada
    /// entera, en vez de tardar minutos en una caché enorme.
    pub entradas_maximas: usize,
}

/// Una entrada de una carpeta vigilada.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entrada {
    /// `None` si el sistema no dio la fecha.
    modificada: Option<SystemTime>,
    /// `None` en las carpetas, cuyo tamaño no dice nada.
    tamano: Option<u64>,
    en_temporal: bool,
}

/// Cómo estaban las carpetas vigiladas en un momento.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Foto {
    entradas: BTreeMap<PathBuf, Entrada>,
    /// Las carpetas que pasaron del tope de entradas.
    incompletas: BTreeSet<PathBuf>,
}

impl Foto {
    /// Anota una entrada. Lo usan [`tomar_foto`] y las pruebas.
    fn anotar(
        &mut self,
        ruta: PathBuf,
        modificada: Option<SystemTime>,
        tamano: Option<u64>,
        en_temporal: bool,
    ) {
        self.entradas.insert(
            ruta,
            Entrada {
                modificada,
                tamano,
                en_temporal,
            },
        );
    }
}

/// De qué clase es lo que se cuenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaseDeRastro {
    /// Apareció o cambió fuera de la carpeta de la tanda.
    Fuera,
    /// Apareció o cambió en el temporal del sistema: puede ser de otro programa.
    EnTemporal,
    /// La carpeta pasó del tope de entradas y no se pudo vigilar entera.
    SinVigilarEntera,
}

/// Algo que contar sobre lo que pasó fuera de la carpeta de la tanda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rastro {
    pub ruta: PathBuf,
    pub clase: ClaseDeRastro,
}

/// Las carpetas que resultan de las plantillas de `vigilar` y `temporales_del_sistema`. Las que
/// nombran una variable que no existe en `entorno` se quedan fuera.
pub fn carpetas_vigiladas(
    vigilar: &[String],
    temporales: &[String],
    entorno: &BTreeMap<String, String>,
) -> Vec<CarpetaVigilada> {
    let de = |plantillas: &[String], es_temporal: bool| -> Vec<CarpetaVigilada> {
        plantillas
            .iter()
            .filter_map(|plantilla| expandir(plantilla, entorno))
            .map(|ruta| CarpetaVigilada {
                ruta: con_un_solo_separador(&ruta),
                es_temporal,
            })
            .collect()
    };
    let mut carpetas = de(vigilar, false);
    carpetas.extend(de(temporales, true));
    carpetas
}

/// La ruta rehecha pieza a pieza, con el separador de la plataforma. Las plantillas van con «/» y
/// las variables de Windows traen «\»; sin esto, el informe mezclaba las dos.
fn con_un_solo_separador(ruta: &str) -> PathBuf {
    Path::new(ruta).components().collect()
}

/// Sustituye cada `{VARIABLE}` por su valor en `entorno`. `None` si alguna no existe: una
/// carpeta a medio componer no es ninguna carpeta.
pub fn expandir(plantilla: &str, entorno: &BTreeMap<String, String>) -> Option<String> {
    let mut resultado = String::new();
    let mut resto = plantilla;
    while let Some(inicio) = resto.find('{') {
        let fin = inicio + resto[inicio..].find('}')?;
        resultado.push_str(&resto[..inicio]);
        resultado.push_str(entorno.get(&resto[inicio + 1..fin])?);
        resto = &resto[fin + 1..];
    }
    resultado.push_str(resto);
    Some(resultado)
}

/// Lee del disco cada carpeta hasta la profundidad de `alcance`. Una carpeta que no existe o no se
/// deja leer no aporta nada; si aparece después, todo lo suyo contará como nuevo. Los enlaces no
/// se siguen: llevarían a mirar fuera de lo vigilado.
pub fn tomar_foto(carpetas: &[CarpetaVigilada], alcance: Alcance) -> Foto {
    let mut foto = Foto::default();
    for carpeta in carpetas {
        if !recorrer(carpeta, alcance, &mut foto) {
            foto.incompletas.insert(carpeta.ruta.clone());
        }
    }
    foto
}

/// Anota las entradas de `carpeta` nivel a nivel. `false` si pasó del tope de entradas.
fn recorrer(carpeta: &CarpetaVigilada, alcance: Alcance, foto: &mut Foto) -> bool {
    let mut pendientes = vec![(carpeta.ruta.clone(), 1)];
    let mut vistas = 0;
    while let Some((directorio, nivel)) = pendientes.pop() {
        let Ok(entradas) = std::fs::read_dir(&directorio) else {
            continue;
        };
        for entrada in entradas.flatten() {
            vistas += 1;
            if vistas > alcance.entradas_maximas {
                return false;
            }
            // De la propia entrada y no del listado: en Windows, `DirEntry::metadata` da la copia
            // de las fechas que NTFS guarda en la carpeta padre y refresca cuando le parece. En
            // una máquina Windows de pruebas (12/11/2026) esa copia de `~\.cargo\registry\cache` pasó de
            // `.9485852` a `.9505999` sin que nadie escribiera, y salía como rastro.
            let metadatos = std::fs::symlink_metadata(entrada.path()).ok();
            let es_carpeta = entrada.file_type().is_ok_and(|tipo| tipo.is_dir());
            let modificada = metadatos.as_ref().and_then(|m| m.modified().ok());
            let tamano = metadatos.filter(|_| !es_carpeta).map(|m| m.len());
            foto.anotar(entrada.path(), modificada, tamano, carpeta.es_temporal);
            if es_carpeta && nivel < alcance.profundidad {
                pendientes.push((entrada.path(), nivel + 1));
            }
        }
    }
    true
}

/// Lo que está en `despues` y no estaba en `antes`, o estaba con otra fecha u otro tamaño, más las
/// carpetas que no se pudieron vigilar enteras en alguna de las dos fotos.
pub fn diferencia(antes: &Foto, despues: &Foto) -> Vec<Rastro> {
    let cambios = despues
        .entradas
        .iter()
        .filter(|(ruta, entrada)| {
            antes.entradas.get(*ruta).is_none_or(|previa| {
                previa.modificada != entrada.modificada || previa.tamano != entrada.tamano
            })
        })
        .map(|(ruta, entrada)| Rastro {
            ruta: ruta.clone(),
            clase: if entrada.en_temporal {
                ClaseDeRastro::EnTemporal
            } else {
                ClaseDeRastro::Fuera
            },
        });
    let incompletas = antes
        .incompletas
        .union(&despues.incompletas)
        .map(|ruta| Rastro {
            ruta: ruta.clone(),
            clase: ClaseDeRastro::SinVigilarEntera,
        });
    cambios.chain(incompletas).collect()
}

/// La ruta de una entrada de rastro tal como se cuenta en el informe y se guarda en la tanda.
pub fn describir(rastro: &Rastro) -> String {
    let ruta = rastro.ruta.display();
    match rastro.clase {
        ClaseDeRastro::Fuera => ruta.to_string(),
        ClaseDeRastro::EnTemporal => {
            format!("{ruta} (temporal del sistema: puede ser de otro programa)")
        }
        ClaseDeRastro::SinVigilarEntera => {
            format!("{ruta} (no se pudo vigilar entera: pasa del tope de entradas)")
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::time::Duration;

    const ALCANCE: Alcance = Alcance {
        profundidad: 4,
        entradas_maximas: 1000,
    };

    fn instante(segundos: u64) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(segundos))
    }

    fn entorno(pares: &[(&str, &str)]) -> BTreeMap<String, String> {
        pares
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn si_nada_cambia_no_hay_rastro() {
        let mut foto = Foto::default();
        foto.anotar("c/a".into(), instante(1), Some(1), false);

        assert!(diferencia(&foto, &foto.clone()).is_empty());
    }

    #[test]
    fn una_entrada_nueva_es_rastro() {
        let antes = Foto::default();
        let mut despues = Foto::default();
        despues.anotar("c/nueva".into(), instante(1), Some(1), false);

        let rastro = diferencia(&antes, &despues);

        assert_eq!(
            rastro,
            [Rastro {
                ruta: "c/nueva".into(),
                clase: ClaseDeRastro::Fuera
            }]
        );
    }

    #[test]
    fn una_entrada_modificada_es_rastro_y_una_borrada_no() {
        let mut antes = Foto::default();
        antes.anotar("c/cambia".into(), instante(1), Some(1), false);
        antes.anotar("c/se_va".into(), instante(1), Some(1), false);
        let mut despues = Foto::default();
        despues.anotar("c/cambia".into(), instante(2), Some(1), false);

        let rastro = diferencia(&antes, &despues);

        assert_eq!(rastro.len(), 1);
        assert_eq!(rastro[0].ruta, PathBuf::from("c/cambia"));
    }

    #[test]
    fn lo_del_temporal_del_sistema_se_marca() {
        let mut despues = Foto::default();
        despues.anotar("tmp/x".into(), instante(1), Some(1), true);

        let rastro = diferencia(&Foto::default(), &despues);

        assert_eq!(rastro[0].clase, ClaseDeRastro::EnTemporal);
        assert!(describir(&rastro[0]).contains("puede ser de otro programa"));
    }

    #[test]
    fn una_carpeta_que_aparece_despues_cuenta_entera() {
        let dir = tempfile::tempdir().unwrap();
        let vigilada = CarpetaVigilada {
            ruta: dir.path().join("cache"),
            es_temporal: false,
        };
        let antes = tomar_foto(std::slice::from_ref(&vigilada), ALCANCE);
        std::fs::create_dir(&vigilada.ruta).unwrap();
        std::fs::write(vigilada.ruta.join("paquete"), "x").unwrap();

        let rastro = diferencia(
            &antes,
            &tomar_foto(std::slice::from_ref(&vigilada), ALCANCE),
        );

        assert_eq!(rastro.len(), 1);
        assert_eq!(rastro[0].ruta, vigilada.ruta.join("paquete"));
    }

    fn vigilada_en(dir: &Path) -> CarpetaVigilada {
        CarpetaVigilada {
            ruta: dir.join("cache"),
            es_temporal: false,
        }
    }

    /// npm añade sus paquetes dentro de subcarpetas que ya existían: con solo el primer nivel no
    /// se veían.
    #[test]
    fn un_fichero_nuevo_a_dos_niveles_se_ve() {
        let dir = tempfile::tempdir().unwrap();
        let vigilada = vigilada_en(dir.path());
        std::fs::create_dir_all(vigilada.ruta.join("a")).unwrap();
        let antes = tomar_foto(std::slice::from_ref(&vigilada), ALCANCE);
        std::fs::write(vigilada.ruta.join("a").join("nuevo"), "x").unwrap();

        let rastro = diferencia(
            &antes,
            &tomar_foto(std::slice::from_ref(&vigilada), ALCANCE),
        );

        assert!(rastro
            .iter()
            .any(|r| r.ruta == vigilada.ruta.join("a").join("nuevo")));
    }

    #[test]
    fn lo_que_pasa_de_la_profundidad_no_se_mira() {
        let dir = tempfile::tempdir().unwrap();
        let vigilada = vigilada_en(dir.path());
        let hondo = vigilada.ruta.join("a").join("b");
        std::fs::create_dir_all(&hondo).unwrap();
        std::fs::write(hondo.join("f"), "x").unwrap();
        let alcance = Alcance {
            profundidad: 2,
            ..ALCANCE
        };

        let foto = tomar_foto(std::slice::from_ref(&vigilada), alcance);

        assert!(foto.entradas.contains_key(&hondo));
        assert!(!foto.entradas.contains_key(&hondo.join("f")));
    }

    #[test]
    fn el_mismo_instante_con_otro_tamano_es_rastro() {
        let mut antes = Foto::default();
        antes.anotar("c/f".into(), instante(1), Some(1), false);
        let mut despues = Foto::default();
        despues.anotar("c/f".into(), instante(1), Some(2), false);

        assert_eq!(diferencia(&antes, &despues).len(), 1);
    }

    #[test]
    fn la_carpeta_que_pasa_del_tope_se_dice_sin_vigilar_entera() {
        let dir = tempfile::tempdir().unwrap();
        let vigilada = vigilada_en(dir.path());
        std::fs::create_dir_all(&vigilada.ruta).unwrap();
        for nombre in ["a", "b", "c"] {
            std::fs::write(vigilada.ruta.join(nombre), "x").unwrap();
        }
        let alcance = Alcance {
            entradas_maximas: 2,
            ..ALCANCE
        };

        let foto = tomar_foto(std::slice::from_ref(&vigilada), alcance);
        let rastro = diferencia(&foto, &foto);

        assert_eq!(
            rastro,
            [Rastro {
                ruta: vigilada.ruta.clone(),
                clase: ClaseDeRastro::SinVigilarEntera
            }]
        );
        assert!(describir(&rastro[0]).contains("no se pudo vigilar entera"));
    }

    /// La plantilla va con «/» y la variable de Windows trae «\»: salía
    /// `C:\Users\x/.nuget/packages` en el informe (máquina Windows de pruebas, 12/11/2026).
    #[cfg(windows)]
    #[test]
    fn la_carpeta_vigilada_sale_con_un_solo_separador() {
        let entorno = entorno(&[("USERPROFILE", r"C:\Users\x")]);

        let carpetas = carpetas_vigiladas(&["{USERPROFILE}/.nuget/packages".into()], &[], &entorno);

        assert_eq!(
            carpetas[0].ruta.display().to_string(),
            r"C:\Users\x\.nuget\packages"
        );
    }

    #[test]
    fn expandir_sustituye_las_variables() {
        let entorno = entorno(&[("HOME", "/home/x"), ("SUB", "npm")]);

        assert_eq!(
            expandir("{HOME}/.{SUB}", &entorno).as_deref(),
            Some("/home/x/.npm")
        );
        assert_eq!(expandir("/tmp", &entorno).as_deref(), Some("/tmp"));
    }

    #[test]
    fn sin_la_variable_la_carpeta_se_queda_fuera() {
        let entorno = entorno(&[("HOME", "/home/x")]);

        assert_eq!(expandir("{LOCALAPPDATA}/npm-cache", &entorno), None);
        assert_eq!(expandir("{HOME", &entorno), None);
        let carpetas = carpetas_vigiladas(
            &["{LOCALAPPDATA}/npm-cache".into(), "{HOME}/.npm".into()],
            &["/tmp".into()],
            &entorno,
        );
        assert_eq!(
            carpetas,
            [
                CarpetaVigilada {
                    ruta: "/home/x/.npm".into(),
                    es_temporal: false
                },
                CarpetaVigilada {
                    ruta: "/tmp".into(),
                    es_temporal: true
                },
            ]
        );
    }
}
