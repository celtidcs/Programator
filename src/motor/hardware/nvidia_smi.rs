//! La medición de una GPU de NVIDIA, preguntándole a `nvidia-smi`, en Linux y en Windows.
//!
//! **Por qué `nvidia-smi` y no una biblioteca.** Lo que da es la memoria de **toda la tarjeta**,
//! que es lo que hace falta: quien la ocupa es `llama-server`, que es otro proceso. La alternativa
//! sería NVML, una biblioteca de NVIDIA a la que hay que llamar por FFI. `nvidia-smi` es un
//! programa que ya viene con el controlador, en Linux y en Windows, se invoca como cualquier otro,
//! devuelve texto y no añade ni una dependencia al proyecto. Si el controlador está instalado, el
//! programa está; y si no lo está, tampoco habría GPU de NVIDIA que medir.
//!
//! **Por qué también en Windows (12/11/2026).** Hasta entonces Windows medía con DXGI, que solo ve
//! el presupuesto del proceso que pregunta. Con el modelo cargado, `nvidia-smi` daba 14 195 MiB
//! ocupados y DXGI 344.
//!
//! **Lo que decide está separado de lo que ejecuta.** `describir_gpu` lanza el proceso y no
//! interpreta nada; `interpretar` recibe texto y no toca el sistema. Así la parte que puede
//! equivocarse se prueba entera sin tarjeta, que es lo que exige la norma de la casa.
//!
//! **Y por eso el análisis se compila en todas las plataformas.** Lo que lanza el proceso va
//! marcado para Linux y Windows, que son donde hay forma de lanzarlo; el análisis se compila
//! siempre, y la suite lo verifica en cada tanda, en cualquier máquina.

use super::Gpu;
use crate::orden_acotada::DesenlaceDeOrden;
#[cfg(any(target_os = "linux", windows))]
use std::time::Duration;

/// El programa que trae el controlador de NVIDIA, y que se busca en el `PATH`.
#[cfg(any(target_os = "linux", windows))]
const PROGRAMA: &str = "nvidia-smi";

/// Tope de la consulta. En el equipo de desarrollo (RTX 4070 Ti SUPER, Windows) contesta en unos
/// 60 ms; el primer arranque del controlador puede tardar algunos segundos. Diez segundos lo
/// cubren con margen, y un `nvidia-smi` colgado no cuelga a Programator. Es un tope de seguridad,
/// no un ajuste: por eso no está en el TOML.
#[cfg(any(target_os = "linux", windows))]
const TIEMPO_MAXIMO: Duration = Duration::from_secs(10);

/// Lo que se le pide: nombre, memoria total y memoria libre, en ese orden.
///
/// `csv,noheader,nounits` deja la salida en la forma más simple que admite: una línea por tarjeta,
/// tres campos separados por comas y las cifras desnudas, sin la coletilla « MiB» detrás.
#[cfg(any(target_os = "linux", windows))]
const CONSULTA: [&str; 2] = [
    "--query-gpu=name,memory.total,memory.free",
    "--format=csv,noheader,nounits",
];

use crate::unidades::BYTES_POR_MIB;

/// La GPU de NVIDIA con más memoria, o `None` si no se puede saber.
///
/// Que `nvidia-smi` no esté, falle o conteste algo ininteligible es siempre lo mismo para quien
/// pregunta: no se sabe. No se distingue entre esos casos a propósito, porque la respuesta útil es
/// idéntica y separarlos solo añadiría ramas que nadie mira.
///
/// **Limitación declarada:** la memoria libre que informa `nvidia-smi` es la del momento de la
/// consulta, y quien va a ocuparla es `llama-server`, que arranca después. Esa distancia la cubre
/// el margen de seguridad de `motor::encaje`.
#[cfg(any(target_os = "linux", windows))]
pub fn describir_gpu() -> Option<Gpu> {
    let argumentos = CONSULTA.map(str::to_owned);
    gpu_de_la_respuesta(crate::orden_acotada::ejecutar_con_tope(
        PROGRAMA,
        &argumentos,
        std::path::Path::new("."),
        TIEMPO_MAXIMO,
    ))
}

/// La tarjeta que da la respuesta de `nvidia-smi`. Si no terminó bien —no está, falló o se agotó
/// el tiempo—, no se sabe.
fn gpu_de_la_respuesta(desenlace: DesenlaceDeOrden) -> Option<Gpu> {
    match desenlace {
        DesenlaceDeOrden::Terminada {
            exito: true,
            salida,
            ..
        } => interpretar(&salida),
        _ => None,
    }
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
    // La salida es de un programa ajeno: una cifra que no cabe en bytes no es una tarjeta.
    let vram_total = campos
        .next()?
        .trim()
        .parse::<u64>()
        .ok()?
        .checked_mul(BYTES_POR_MIB)?;
    let vram_libre = campos
        .next()?
        .trim()
        .parse::<u64>()
        .ok()?
        .checked_mul(BYTES_POR_MIB)?;

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

    fn terminada(exito: bool, salida: &str) -> DesenlaceDeOrden {
        DesenlaceDeOrden::Terminada {
            exito,
            codigo: Some(if exito { 0 } else { 1 }),
            estado: String::new(),
            salida: salida.into(),
            errores: String::new(),
        }
    }

    #[test]
    fn solo_una_respuesta_terminada_con_exito_da_tarjeta() {
        assert!(gpu_de_la_respuesta(terminada(true, UNA_TARJETA)).is_some());
        assert_eq!(gpu_de_la_respuesta(terminada(false, UNA_TARJETA)), None);
    }

    /// Un `nvidia-smi` colgado se corta y se toma como «no se sabe».
    #[test]
    fn un_nvidia_smi_cortado_por_tiempo_no_da_tarjeta() {
        assert_eq!(gpu_de_la_respuesta(DesenlaceDeOrden::Agotada), None);
        assert_eq!(
            gpu_de_la_respuesta(DesenlaceDeOrden::NoSeLanzo("no está".into())),
            None
        );
    }

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

    /// La cifra en MiB por los bytes de un MiB se multiplica comprobando el desbordamiento. Sin
    /// comprobar, una salida absurda desbordaba: en depuración, pánico; en la compilación final,
    /// una tarjeta con una memoria cualquiera.
    #[test]
    fn una_memoria_que_no_cabe_en_bytes_no_es_una_tarjeta() {
        let enorme = u64::MAX / 2;
        assert_eq!(interpretar(&format!("Tarjeta, {enorme}, 1000\n")), None);
        assert_eq!(interpretar(&format!("Tarjeta, 24564, {enorme}\n")), None);
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
