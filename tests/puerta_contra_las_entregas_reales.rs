//! La puerta, contra las entregas de verdad de la evaluación del 22/09/2026.
//!
//! Las pruebas del módulo usan comprobadores de juguete, que es lo correcto para probar la
//! mecánica. Esta usa **Python de verdad** sobre **los ficheros que el modelo entregó aquel día**,
//! porque la pregunta que importa no es si el mecanismo funciona: es si habría cazado lo que se
//! nos coló.
//!
//! Se salta sola si no hay Python en la máquina: una prueba que depende de lo que esté instalado
//! no puede romper la suite de otro.

use programator::arnes::verificacion::{verificar, Comprobadores, Veredicto};
use std::time::Duration;

fn hay_python() -> bool {
    std::process::Command::new("python")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn comprobadores_de_python() -> Comprobadores {
    let mut mapa = Comprobadores::new();
    mapa.insert(
        "py".to_string(),
        vec![
            "python".into(),
            "-m".into(),
            "py_compile".into(),
            "{fichero}".into(),
        ],
    );
    mapa
}

#[test]
fn el_python_que_el_modelo_entrego_de_verdad_pasa_la_puerta() {
    if !hay_python() {
        eprintln!("sin Python en esta máquina: prueba omitida");
        return;
    }

    // La entrega original, tal como quedó archivada en la evaluación. Sin copias: si algún día se
    // toca ese fichero, esta prueba tiene que enterarse.
    let fichero =
        std::path::Path::new("evaluacion/banco/.gestor/candidatos/programator/reintentos.py");
    assert!(
        fichero.is_file(),
        "falta la entrega real del candidato en {}",
        fichero.display()
    );

    let temporal = std::env::temp_dir();
    let veredicto = verificar(
        fichero,
        &comprobadores_de_python(),
        Duration::from_secs(60),
        &temporal,
    );

    assert!(
        matches!(veredicto, Veredicto::Pasa { .. }),
        "su decorador de reintentos sí compilaba, y la puerta no puede decir lo contrario: \
         {veredicto:?}"
    );
}

#[test]
fn un_fichero_con_el_mismo_defecto_que_el_lua_no_pasa_la_puerta() {
    if !hay_python() {
        eprintln!("sin Python en esta máquina: prueba omitida");
        return;
    }

    // El fallo real fue un identificador con «ñ» en Lua, que Lua no admite. Aquí se reproduce el
    // mismo tipo de defecto —sintaxis que el lenguaje rechaza— en el lenguaje que esta máquina sí
    // puede comprobar: lo que se prueba es que la puerta lo caza y devuelve el motivo.
    let dir = std::env::temp_dir().join("programator-prueba-puerta");
    std::fs::create_dir_all(&dir).unwrap();
    let fichero = dir.join("roto.py");
    std::fs::write(&fichero, "def sin_cerrar(:\n    pass\n").unwrap();

    let veredicto = verificar(
        &fichero,
        &comprobadores_de_python(),
        Duration::from_secs(60),
        &dir,
    );

    let Veredicto::NoPasa { salida, .. } = &veredicto else {
        panic!("la puerta tenía que rechazarlo: {veredicto:?}");
    };
    assert!(
        salida.to_lowercase().contains("syntax"),
        "el motivo del rechazo es lo único que le sirve al modelo para corregir: {salida}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
