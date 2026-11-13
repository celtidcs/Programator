//! El aislamiento de Windows de verdad: el ayudante con integridad baja no puede escribir en una
//! carpeta normal y sí en una marcada.
//!
//! Va con `#[ignore]` porque crea procesos de integridad baja y cambia etiquetas de seguridad: se
//! ejecuta a mano, en una máquina Windows de pruebas y nunca en el equipo de uso diario, con
//! `cargo test --test aislamiento_windows -- --ignored`.
#![cfg(windows)]

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use programator::aislamiento_windows::marcar_carpeta_de_integridad_baja;
use programator::argumentos::orden_del_ayudante;
use programator::ayudante_aislado::SENAL_DE_ARRANQUE;

/// La variable por la que el ayudante recibe dónde escribir, para no meter la ruta, que puede
/// llevar espacios, dentro de la orden de PowerShell.
const VARIABLE_DEL_DESTINO: &str = "PROGRAMATOR_DESTINO";

/// Lanza el ayudante con una orden que escribe en `destino` y dice si lo consiguió.
fn el_ayudante_puede_escribir(destino: &Path) -> bool {
    let orden: Vec<String> = [
        "powershell",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Set-Content -LiteralPath $env:PROGRAMATOR_DESTINO -Value x",
    ]
    .map(String::from)
    .to_vec();
    let completa = orden_del_ayudante(env!("CARGO_BIN_EXE_programator"), &orden);
    let mut ayudante = Command::new(&completa[0])
        .args(&completa[1..])
        .env(VARIABLE_DEL_DESTINO, destino)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    ayudante
        .stdin
        .take()
        .unwrap()
        .write_all(&[SENAL_DE_ARRANQUE])
        .unwrap();
    let estado = ayudante.wait().unwrap();
    estado.success() && destino.exists()
}

/// La vigilancia de la red, sin internet: un proceso del Job se conecta a un servidor de esta
/// prueba en `127.0.0.1`, y la tabla de conexiones tiene que mostrarla. Si falla, la vigilancia no
/// ve nada; si pasa y un `curl` breve no se vio, era la duración.
#[test]
#[ignore = "lee la tabla de conexiones de Windows: se ejecuta a mano en una máquina Windows de pruebas, nunca en el equipo de uso diario"]
fn la_tabla_de_conexiones_ve_la_de_un_proceso_del_job() {
    use programator::aislamiento_windows::{conexiones_de, nombre_del_proceso, Trabajo};
    use std::net::{Ipv4Addr, SocketAddr, TcpListener};

    let servidor = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let puerto = servidor.local_addr().unwrap().port();
    let trabajo = Trabajo::nuevo(8, 512).unwrap();
    let orden = format!(
        "$c = New-Object Net.Sockets.TcpClient('127.0.0.1', {puerto}); Start-Sleep -Seconds 10"
    );
    let mut cliente = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &orden])
        .spawn()
        .unwrap();
    trabajo.meter(&cliente).unwrap();
    let _conexion = servidor.accept().unwrap();

    let procesos = trabajo.procesos().unwrap();
    let remotas = conexiones_de(&procesos).unwrap();
    let programa = nombre_del_proceso(cliente.id());
    let _ = cliente.kill();
    let _ = cliente.wait();

    assert_eq!(programa.as_deref(), Some("powershell.exe"));
    assert!(procesos.contains(&cliente.id()), "{procesos:?}");
    let esperada = SocketAddr::from((Ipv4Addr::LOCALHOST, puerto));
    assert!(remotas.contains(&esperada), "{remotas:?}");
}

#[test]
#[ignore = "crea procesos de integridad baja: se ejecuta a mano en una máquina Windows de pruebas, nunca en el equipo de uso diario"]
fn con_integridad_baja_solo_se_escribe_en_la_carpeta_marcada() {
    let normal = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let marcada = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    marcar_carpeta_de_integridad_baja(marcada.path()).unwrap();

    assert!(!el_ayudante_puede_escribir(
        &normal.path().join("fuera.txt")
    ));
    assert!(el_ayudante_puede_escribir(
        &marcada.path().join("dentro.txt")
    ));
}

/// Deja en `carpeta` solo los permisos que tiene el usuario en una unidad donde solo puede
/// modificar: «Modificar», sin herencia del padre ni «Control total».
fn solo_modificar(carpeta: &Path) {
    let usuario = std::env::var("USERNAME").unwrap();
    let estado = Command::new("icacls")
        .arg(carpeta)
        .args(["/inheritance:r", "/grant:r"])
        .arg(format!("{usuario}:(OI)(CI)M"))
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(estado.success());
}

/// En uso real (13/11/2026) falló en una unidad así con «Acceso denegado» al etiquetar:
/// Modificar no da el permiso de cambiar el propietario, que pide la etiqueta.
#[test]
#[ignore = "crea procesos de integridad baja: se ejecuta a mano en una máquina Windows de pruebas, nunca en el equipo de uso diario"]
fn se_marca_una_carpeta_donde_el_usuario_solo_puede_modificar() {
    let padre = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let marcada = padre.path().join("comprobacion");
    std::fs::create_dir(&marcada).unwrap();
    solo_modificar(&marcada);

    marcar_carpeta_de_integridad_baja(&marcada).unwrap();

    assert!(el_ayudante_puede_escribir(&marcada.join("dentro.txt")));
}
