//! La puerta, contra las entregas de verdad de la evaluación del 22/09/2026.
//!
//! Las pruebas del módulo usan comprobadores de juguete, que es lo correcto para probar la
//! mecánica. Esta usa **Python de verdad**, con el mismo comprobador que pone Programator al
//! arrancar, sobre **los ficheros que el modelo entregó aquel día**, porque la pregunta que importa
//! no es si el mecanismo funciona: es si habría cazado lo que se nos coló.
//!
//! Se salta sola si no hay Python en la máquina: una prueba que depende de lo que esté instalado
//! no puede romper la suite de otro.
//!
//! Es hermética: las rutas del repositorio se anclan en
//! `CARGO_MANIFEST_DIR`, los temporales van a `CARGO_TARGET_TMPDIR` y se borran solos, y comprobar
//! la entrega real no deja `__pycache__` en el árbol.

use programator::arnes::verificacion::python::{completar, Python};
use programator::arnes::verificacion::{verificar, Comprobadores, Veredicto};
use programator::config::{Verificacion, MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO};
use programator::orden_acotada::{ejecutar_con_tope, DesenlaceDeOrden};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Tiempo máximo de cada orden de Python: sobra para `--version` y para compilar un fichero corto.
const TIEMPO_MAXIMO: Duration = Duration::from_secs(60);

/// La entrega original del candidato, tal como quedó archivada en la evaluación.
const ENTREGA_REAL: &str = "evaluacion/banco/.gestor/candidatos/programator/reintentos.py";

/// Los comprobadores que pondría Programator al arrancar en esta máquina, o `None` si no encuentra
/// Python. Busca igual que el producto: `python` y luego `python3`, el primero que responde.
fn comprobadores_del_producto() -> Option<Comprobadores> {
    let mut verificacion = Verificacion::default();
    let aqui = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let python = completar(&mut verificacion, |programa| {
        matches!(
            ejecutar_con_tope(programa, &["--version".to_owned()], aqui, TIEMPO_MAXIMO),
            DesenlaceDeOrden::Terminada { exito: true, .. }
        )
    });
    match python {
        Python::Encontrado(_) => Some(verificacion.comprobadores),
        _ => None,
    }
}

fn del_repositorio(ruta: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(ruta)
}

fn temporal() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("temporal en target")
}

#[test]
fn el_python_que_el_modelo_entrego_de_verdad_pasa_la_puerta() {
    let Some(comprobadores) = comprobadores_del_producto() else {
        eprintln!("sin Python en esta máquina: prueba omitida");
        return;
    };

    // Sin copias: si algún día se toca ese fichero, esta prueba tiene que enterarse.
    let fichero = del_repositorio(ENTREGA_REAL);
    assert!(
        fichero.is_file(),
        "falta la entrega real del candidato en {}",
        fichero.display()
    );

    let trabajo = temporal();
    let veredicto = verificar(
        &fichero,
        &comprobadores,
        TIEMPO_MAXIMO,
        trabajo.path(),
        MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO,
    );

    assert!(
        matches!(veredicto, Veredicto::Pasa { .. }),
        "su decorador de reintentos sí compilaba, y la puerta no puede decir lo contrario: \
         {veredicto:?}"
    );
    let cache = fichero.with_file_name("__pycache__");
    assert!(
        !cache.exists(),
        "comprobar no puede ensuciar el repositorio: quedó {}",
        cache.display()
    );
}

#[test]
fn un_fichero_con_el_mismo_defecto_que_el_lua_no_pasa_la_puerta() {
    let Some(comprobadores) = comprobadores_del_producto() else {
        eprintln!("sin Python en esta máquina: prueba omitida");
        return;
    };

    // El fallo real fue un identificador con «ñ» en Lua, que Lua no admite. Aquí se reproduce el
    // mismo tipo de defecto —sintaxis que el lenguaje rechaza— en el lenguaje que esta máquina sí
    // puede comprobar: lo que se prueba es que la puerta lo caza y devuelve el motivo.
    let carpeta = temporal();
    let fichero = carpeta.path().join("roto.py");
    std::fs::write(&fichero, "def sin_cerrar(:\n    pass\n").unwrap();

    let veredicto = verificar(
        &fichero,
        &comprobadores,
        TIEMPO_MAXIMO,
        carpeta.path(),
        MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO,
    );

    let Veredicto::NoPasa { salida, .. } = &veredicto else {
        panic!("la puerta tenía que rechazarlo: {veredicto:?}");
    };
    assert!(
        salida.to_lowercase().contains("syntax"),
        "el motivo del rechazo es lo único que le sirve al modelo para corregir: {salida}"
    );
}
