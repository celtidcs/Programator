//! El ayudante del aislamiento en Windows: `programator --lanzar-aislado -- <orden>`.
//!
//! Lo lanza la comprobación local, nunca una persona. Hace cuatro cosas, en este orden:
//!
//! 1. espera un byte por la entrada estándar: es la señal de que Programator ya lo metió en el
//!    Job, y así ningún proceso de la orden nace fuera de él;
//! 2. baja su propia integridad al nivel bajo;
//! 3. lanza la orden, que hereda la integridad, la entrada y las salidas;
//! 4. sale con el código de la orden.
//!
//! Si algo falla antes de lanzarla, no la lanza: el motivo va a la salida de errores y el código de
//! salida es [`CODIGO_AISLAMIENTO_FALLIDO`].

/// El código con que sale el ayudante si no pudo aislar ni lanzar la orden. 125 es el que usan
/// `env` y `docker run` para «falló el que lanza, no lo lanzado», y no lo devuelve ninguna de las
/// herramientas del banco.
pub const CODIGO_AISLAMIENTO_FALLIDO: i32 = 125;

/// El byte que Programator escribe al ayudante cuando ya está dentro del Job. Su valor no importa,
/// solo que llegue.
pub const SENAL_DE_ARRANQUE: u8 = b'1';

/// Lo que el ayudante escribe en la salida de errores si no pudo lanzar la orden. Llega al
/// informe de la comprobación como la salida de errores de esa orden.
pub fn mensaje_del_fallo(motivo: &str) -> String {
    format!("Programator no pudo aislar la orden, y no la lanzó: {motivo}")
}

/// Hace el trabajo del ayudante y devuelve el código con que debe salir, o el motivo del fallo.
#[cfg(windows)]
pub fn lanzar_aislado(orden: &[String]) -> Result<i32, String> {
    use std::io::Read;

    let Some((programa, argumentos)) = orden.split_first() else {
        return Err("no hay orden que lanzar".into());
    };
    let mut senal = [0u8; 1];
    std::io::stdin()
        .read_exact(&mut senal)
        .map_err(|causa| format!("no llegó la señal de arranque: {causa}"))?;
    crate::aislamiento_windows::bajar_mi_integridad().map_err(|fallo| fallo.to_string())?;
    let estado = std::process::Command::new(programa)
        .args(argumentos)
        .status()
        .map_err(|causa| format!("no se pudo lanzar «{programa}»: {causa}"))?;
    Ok(estado.code().unwrap_or(CODIGO_AISLAMIENTO_FALLIDO))
}

/// Fuera de Windows no hay integridad baja: la configuración ya lo rechaza antes de llegar aquí.
#[cfg(not(windows))]
pub fn lanzar_aislado(_orden: &[String]) -> Result<i32, String> {
    Err("--lanzar-aislado solo existe en Windows".into())
}
