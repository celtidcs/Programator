//! El comprobador de `.py` que se pone solo si hay Python en el equipo: sin él, las propuestas
//! `.py` quedarían «SIN COMPROBAR» mientras nadie configure un comprobador.
//!
//! Si `[verificacion.comprobadores]` ya trae uno para `py`, manda ese. Si no, y no está apagado,
//! se prueba la ruta de `[verificacion] python` o, sin ruta, los nombres de
//! [`CANDIDATOS_EN_EL_PATH`]; el primero que responde a `--version` es el comprobador. Probar que
//! responde, y no solo que existe, deja fuera el `python.exe` de la tienda de Windows, que está en
//! el PATH pero solo abre la tienda.

use crate::config::Verificacion;

/// Extensión (sin punto) de los ficheros de Python.
const EXTENSION: &str = "py";

/// Los nombres con que se busca Python en el PATH, en este orden: `python` en Windows y en muchas
/// distribuciones, `python3` en las que no instalan el otro.
pub const CANDIDATOS_EN_EL_PATH: [&str; 2] = ["python", "python3"];

/// Lo que hace el comprobador: compilar el fichero a bytecode en memoria, sin guardarlo ni
/// ejecutarlo. Detecta los mismos errores que `py_compile`, pero no deja un `__pycache__` junto a
/// la propuesta. Lee bytes para que Python respete la codificación declarada y la marca BOM.
const COMPROBACION: &str =
    "import sys; compile(open(sys.argv[1], 'rb').read(), sys.argv[1], 'exec')";

/// Qué pasó al buscar Python.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Python {
    /// El TOML ya trae un comprobador de `.py`: no se toca.
    YaConfigurado,
    /// `[verificacion] comprobar_python = false`.
    Apagado,
    /// Se encontró y se puso como comprobador.
    Encontrado(String),
    /// No respondió ninguno de los probados.
    NoEncontrado { probados: Vec<String> },
}

/// Pone el comprobador de `.py` en `verificacion` si hace falta y hay un Python que responda.
/// `responde` dice si un programa contesta a `--version`.
pub fn completar(
    verificacion: &mut Verificacion,
    mut responde: impl FnMut(&str) -> bool,
) -> Python {
    if verificacion.comprobadores.contains_key(EXTENSION) {
        return Python::YaConfigurado;
    }
    if !verificacion.comprobar_python {
        return Python::Apagado;
    }
    let probados: Vec<String> = if verificacion.python.trim().is_empty() {
        CANDIDATOS_EN_EL_PATH.map(str::to_owned).to_vec()
    } else {
        vec![verificacion.python.trim().to_owned()]
    };
    let Some(encontrado) = probados.iter().find(|programa| responde(programa)).cloned() else {
        return Python::NoEncontrado { probados };
    };
    verificacion
        .comprobadores
        .insert(EXTENSION.to_owned(), orden_de_comprobacion(&encontrado));
    Python::Encontrado(encontrado)
}

/// La orden que comprueba un `.py` con `python`. `-I` aísla a Python de la carpeta donde corre y de
/// las variables de entorno de Python.
fn orden_de_comprobacion(python: &str) -> Vec<String> {
    [python, "-I", "-c", COMPROBACION, "{fichero}"]
        .map(str::to_owned)
        .to_vec()
}

/// El aviso de la terminal.
pub fn mensaje(python: &Python) -> Option<String> {
    match python {
        Python::YaConfigurado | Python::Apagado => None,
        Python::Encontrado(programa) => Some(format!(
            "🐍 Las propuestas .py se comprueban con {programa}: se compila su sintaxis, sin \
             ejecutarlas."
        )),
        Python::NoEncontrado { probados } => Some(format!(
            "⚠️ No se encontró Python (se probó {}): las propuestas .py quedarán SIN COMPROBAR. \
             Si lo tienes instalado, pon su ruta en [verificacion] python de programator.toml.",
            probados.join(", ")
        )),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn verificacion(comprobar: bool, ruta: &str) -> Verificacion {
        Verificacion {
            comprobar_python: comprobar,
            python: ruta.into(),
            ..Verificacion::default()
        }
    }

    #[test]
    fn el_primero_que_responde_es_el_comprobador() {
        let mut v = verificacion(true, "");
        let mut probados = Vec::new();

        let python = completar(&mut v, |programa| {
            probados.push(programa.to_owned());
            programa == "python3"
        });

        assert_eq!(python, Python::Encontrado("python3".into()));
        assert_eq!(probados, ["python", "python3"]);
        let orden = &v.comprobadores[EXTENSION];
        assert_eq!(orden[0], "python3");
        assert_eq!(orden.last().map(String::as_str), Some("{fichero}"));
        assert!(orden.iter().any(|a| a.contains("compile(")), "{orden:?}");
    }

    #[test]
    fn con_ruta_en_el_toml_solo_se_prueba_esa() {
        let mut v = verificacion(true, "C:/Python312/python.exe");
        let mut probados = Vec::new();

        let python = completar(&mut v, |programa| {
            probados.push(programa.to_owned());
            false
        });

        assert_eq!(
            python,
            Python::NoEncontrado {
                probados: vec!["C:/Python312/python.exe".into()]
            }
        );
        assert_eq!(probados, ["C:/Python312/python.exe"]);
        assert!(v.comprobadores.is_empty());
    }

    #[test]
    fn sin_python_no_se_pone_nada_y_se_dice_que_se_probo() {
        let mut v = verificacion(true, "");

        let python = completar(&mut v, |_| false);

        assert_eq!(
            python,
            Python::NoEncontrado {
                probados: vec!["python".into(), "python3".into()]
            }
        );
        assert!(v.comprobadores.is_empty());
    }

    #[test]
    fn el_comprobador_del_toml_manda_y_apagado_no_se_busca() {
        let mut configurado = verificacion(true, "");
        configurado
            .comprobadores
            .insert(EXTENSION.into(), vec!["mi-python".into()]);
        let mut apagado = verificacion(false, "");
        let mut se_probo = false;

        assert_eq!(
            completar(&mut configurado, |_| {
                se_probo = true;
                true
            }),
            Python::YaConfigurado
        );
        assert_eq!(
            completar(&mut apagado, |_| {
                se_probo = true;
                true
            }),
            Python::Apagado
        );

        assert!(!se_probo);
        assert_eq!(configurado.comprobadores[EXTENSION], ["mi-python"]);
        assert!(apagado.comprobadores.is_empty());
    }

    #[test]
    fn los_avisos_dicen_con_que_se_comprueba_o_que_falta() {
        let encontrado = mensaje(&Python::Encontrado("python".into())).unwrap();
        let falta = mensaje(&Python::NoEncontrado {
            probados: vec!["python".into(), "python3".into()],
        })
        .unwrap();

        assert!(encontrado.contains("python"), "{encontrado}");
        assert!(falta.contains("SIN COMPROBAR"), "{falta}");
        assert!(falta.contains("python, python3"), "{falta}");
        assert!(falta.contains("[verificacion] python"), "{falta}");
        assert_eq!(mensaje(&Python::YaConfigurado), None);
        assert_eq!(mensaje(&Python::Apagado), None);
    }
}
