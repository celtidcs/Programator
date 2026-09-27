//! La medición de la GPU en Linux, preguntándole a `nvidia-smi`.
//!
//! **Por qué `nvidia-smi` y no una biblioteca.** En Windows la cifra la da DXGI, que forma parte
//! del sistema. En Linux el equivalente sería NVML, que es una biblioteca de NVIDIA a la que hay
//! que llamar por FFI. `nvidia-smi` es un programa que ya viene con el controlador, se invoca
//! como cualquier otro, devuelve texto y no añade ni una dependencia al proyecto. Si el
//! controlador está instalado, el programa está; y si no lo está, tampoco habría GPU que medir.
//!
//! **Lo que decide está separado de lo que ejecuta.** `describir_gpu` lanza el proceso y no
//! interpreta nada; `interpretar` recibe texto y no toca el sistema. Así la parte que puede
//! equivocarse se prueba entera sin tarjeta, que es lo que exige la norma de la casa.
//!
//! **Y por eso el análisis se compila en todas las plataformas, no solo en Linux.** Lo que lanza
//! el proceso sí es de Linux y va marcado como tal, pero si el análisis solo existiera allí, sus
//! pruebas solo correrían allí: se escribiría a ciegas desde Windows y nadie lo vería fallar hasta
//! tener la máquina delante. Compilándolo siempre, la suite lo verifica en cada tanda.

use super::Gpu;
#[cfg(target_os = "linux")]
use std::process::Command;

/// El programa que trae el controlador de NVIDIA, y que se busca en el `PATH`.
#[cfg(target_os = "linux")]
const PROGRAMA: &str = "nvidia-smi";

/// Lo que se le pide: nombre, memoria total y memoria libre, en ese orden.
///
/// `csv,noheader,nounits` deja la salida en la forma más simple que admite: una línea por tarjeta,
/// tres campos separados por comas y las cifras desnudas, sin la coletilla « MiB» detrás.
#[cfg(target_os = "linux")]
const CONSULTA: [&str; 2] = [
    "--query-gpu=name,memory.total,memory.free",
    "--format=csv,noheader,nounits",
];

/// `nvidia-smi` da la memoria en mebibytes; el resto de Programator la cuenta en bytes.
const BYTES_POR_MIB: u64 = 1024 * 1024;

/// La GPU de NVIDIA con más memoria, o `None` si no se puede saber.
///
/// Que `nvidia-smi` no esté, falle o conteste algo ininteligible es siempre lo mismo para quien
/// pregunta: no se sabe. No se distingue entre esos casos a propósito, porque la respuesta útil es
/// idéntica y separarlos solo añadiría ramas que nadie mira.
///
/// **Limitación declarada:** la memoria libre que informa `nvidia-smi` es la del momento de la
/// consulta, y quien va a ocuparla es `llama-server`, que arranca después. Es la misma distancia
/// que hay en Windows entre el presupuesto medido y el que verá el motor, y la cubre el mismo
/// margen de seguridad de `motor::encaje`.
#[cfg(target_os = "linux")]
pub fn describir_gpu() -> Option<Gpu> {
    let salida = Command::new(PROGRAMA).args(CONSULTA).output().ok()?;
    if !salida.status.success() {
        return None;
    }
    interpretar(&String::from_utf8_lossy(&salida.stdout))
}

/// Saca de la salida de `nvidia-smi` la tarjeta con más memoria total.
///
/// Se queda con la mayor, y no con la primera, por el mismo motivo que en Windows: en una máquina
/// con dos tarjetas la que interesa es la grande, y el orden en que las enumere el sistema no es
/// una promesa de nadie.
///
/// Una línea que no se entienda se descarta en silencio en vez de invalidar el resto. Si una de
/// las tarjetas informa `[N/A]` —pasa con algunas virtualizadas— eso no debería dejar ciega a la
/// máquina entera respecto de las demás.
fn interpretar(salida: &str) -> Option<Gpu> {
    salida
        .lines()
        .filter_map(interpretar_linea)
        .fold(None, |mejor: Option<Gpu>, gpu| match mejor {
            Some(previa) if previa.vram_total >= gpu.vram_total => Some(previa),
            _ => Some(gpu),
        })
}

/// Una línea de la salida, que es «nombre, total, libre», a `Gpu`.
///
/// Devuelve `None` en cuanto algo no cuadra: campos de menos, cifras que no son cifras, un nombre
/// vacío, o una tarjeta que dice tener cero memoria. Ese último caso es el equivalente al filtro
/// del adaptador software de Windows: algo que declara cero memoria no es una tarjeta en la que
/// cargar un modelo.
fn interpretar_linea(linea: &str) -> Option<Gpu> {
    let mut campos = linea.split(',');
    let nombre = campos.next()?.trim().to_string();
    let vram_total = campos.next()?.trim().parse::<u64>().ok()? * BYTES_POR_MIB;
    let vram_libre = campos.next()?.trim().parse::<u64>().ok()? * BYTES_POR_MIB;

    if nombre.is_empty() || vram_total == 0 {
        return None;
    }

    Some(Gpu {
        nombre,
        vram_total,
        // Que informe más libre que total no debería pasar, pero si pasa se recorta en vez de
        // propagarse: `motor::encaje` reparte capas con esta cifra y una mentira al alza acabaría
        // pidiendo a la tarjeta más de lo que cabe.
        vram_libre: vram_libre.min(vram_total),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La salida real de una máquina con una sola tarjeta, tal como la escribe `nvidia-smi`.
    const UNA_TARJETA: &str = "NVIDIA GeForce RTX 4070 Ti SUPER, 16376, 15234\n";

    #[test]
    fn una_tarjeta_se_lee_entera_y_en_bytes() {
        let gpu = interpretar(UNA_TARJETA).expect("la línea es válida");
        assert_eq!(gpu.nombre, "NVIDIA GeForce RTX 4070 Ti SUPER");
        assert_eq!(gpu.vram_total, 16376 * 1024 * 1024);
        assert_eq!(gpu.vram_libre, 15234 * 1024 * 1024);
    }

    #[test]
    fn con_dos_tarjetas_gana_la_de_mas_memoria_aunque_no_sea_la_primera() {
        let salida = "NVIDIA T400, 4096, 4000\nNVIDIA GeForce RTX 4090, 24564, 20000\n";
        let gpu = interpretar(salida).expect("hay dos líneas válidas");
        assert_eq!(gpu.nombre, "NVIDIA GeForce RTX 4090");
        assert_eq!(gpu.vram_total, 24564 * 1024 * 1024);
    }

    #[test]
    fn una_linea_ilegible_no_invalida_a_las_demas() {
        let salida = "esto no tiene comas ni cifras\nNVIDIA GeForce RTX 4090, 24564, 20000\n";
        let gpu = interpretar(salida).expect("la segunda línea sí es válida");
        assert_eq!(gpu.nombre, "NVIDIA GeForce RTX 4090");
    }

    #[test]
    fn una_tarjeta_que_dice_no_saber_su_memoria_se_descarta() {
        // Pasa en tarjetas virtualizadas: `nvidia-smi` contesta «[N/A]» donde iba la cifra.
        assert_eq!(interpretar("NVIDIA A100-SXM, [N/A], [N/A]\n"), None);
    }

    #[test]
    fn una_tarjeta_con_cero_memoria_no_cuenta() {
        // El equivalente al adaptador software que Windows filtra: no es un sitio donde cargar
        // nada, y dejarla pasar la convertiría en la elegida en cuanto la buena estuviera ocupada.
        assert_eq!(interpretar("Dispositivo raro, 0, 0\n"), None);
    }

    #[test]
    fn sin_salida_no_se_inventa_una_tarjeta() {
        assert_eq!(interpretar(""), None);
        assert_eq!(interpretar("\n\n"), None);
    }

    #[test]
    fn libre_nunca_supera_al_total() {
        // Si el sistema contesta algo incoherente, se recorta: `motor::encaje` reparte capas con
        // esta cifra, y pasarle una mentira al alza haría que pidiera a la tarjeta más de lo que
        // cabe, que es el fallo que este módulo existe para no cometer.
        let gpu = interpretar("Tarjeta confundida, 8192, 99999\n").expect("la línea es válida");
        assert_eq!(gpu.vram_libre, gpu.vram_total);
    }

    #[test]
    fn los_espacios_sobrantes_no_estorban() {
        let gpu = interpretar("  NVIDIA GeForce RTX 4090 ,  24564 ,  20000  \n")
            .expect("la línea es válida salvo por los espacios");
        assert_eq!(gpu.nombre, "NVIDIA GeForce RTX 4090");
        assert_eq!(gpu.vram_total, 24564 * 1024 * 1024);
    }
}
