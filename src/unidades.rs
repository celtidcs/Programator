//! Las unidades binarias de memoria, en un solo sitio.
//!
//! Así ningún módulo tiene su propio `1024 * 1024`. La VRAM se configura e informa en MiB,
//! llega de la tarjeta y de los ficheros en bytes, y a las personas se les cuenta en GiB.

/// Bytes en un mebibyte (2^20).
pub const BYTES_POR_MIB: u64 = 1024 * 1024;

/// Lo mismo en `usize`, para tamaños de memoria del propio proceso (trozos de lectura, el límite
/// de memoria de un Job de Windows). Cabe en cualquier `usize` de 32 bits o más.
pub const BYTES_POR_MIB_USIZE: usize = 1024 * 1024;

/// Bytes en un gibibyte (2^30), en coma flotante para dividir.
pub const BYTES_POR_GIB_F64: f64 = 1024.0 * 1024.0 * 1024.0;

/// Segundos en un minuto, para contar a una persona un tiempo en minutos.
pub const SEGUNDOS_POR_MINUTO: u64 = 60;

/// Minutos en una hora, para contar a una persona un tiempo en horas y minutos.
pub const MINUTOS_POR_HORA: u64 = 60;

/// Milisegundos en un segundo, en coma flotante para dividir (velocidades en tokens por segundo).
pub const MS_POR_SEGUNDO: f64 = 1000.0;

/// El entero de un porcentaje: un umbral del 80 % es 80 / `CIEN_POR_CIEN` de la cifra.
pub const CIEN_POR_CIEN: u32 = 100;

/// Lo que se escribe en lugar de una cifra que no se pudo medir. Uno solo para el registro, el
/// encaje, el banco y la revisión.
pub const SIN_MEDIR: &str = "sin medir";

/// Bytes a GiB con un decimal y coma decimal, que es como se escribe en español. Es la única: toda
/// cifra de GiB la lee una persona (diagnóstico, encaje, historial de encaje y revisión), y antes
/// había dos, una con punto y otra con coma, que salían juntas en el mismo arranque.
pub fn en_gib(bytes: u64) -> String {
    let gib = bytes as f64 / BYTES_POR_GIB_F64;
    format!("{gib:.1} GiB").replace('.', ",")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_gib_salen_con_un_decimal_y_coma() {
        assert_eq!(en_gib(8 * 1024 * BYTES_POR_MIB), "8,0 GiB");
        assert_eq!(en_gib(2_576_980_378), "2,4 GiB");
    }

    #[test]
    fn las_tres_formas_dicen_lo_mismo() {
        assert_eq!(BYTES_POR_MIB, 1 << 20);
        assert_eq!(BYTES_POR_MIB_USIZE as u64, BYTES_POR_MIB);
        assert_eq!(BYTES_POR_GIB_F64, (BYTES_POR_MIB * 1024) as f64);
    }
}
