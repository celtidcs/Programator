//! La medición de la GPU en Windows a través de DXGI, **solo de respaldo**.
//!
//! Desde el 12/11/2026 se usa únicamente cuando `nvidia-smi` no contesta, por ejemplo con una
//! tarjeta que no es de NVIDIA. El motivo es su limitación, abajo: con el modelo cargado por otro
//! proceso, DXGI daba 344 MiB ocupados y la tarjeta tenía 14 195. Lo que los dos medidores
//! comparten —el tipo `Gpu` y la decisión de a quién preguntar— se queda en el módulo padre.

use super::Gpu;

/// La GPU dedicada con más memoria según DXGI, o `None` si no se puede saber.
///
/// **Limitación declarada:** `Budget` y `CurrentUsage` son del proceso que pregunta, no de la
/// tarjeta. Lo que ocupe otro proceso —`llama-server`, un juego, otro modelo— apenas mueve la
/// cifra, y en reposo da alrededor de 1 GiB más libre de lo que hay.
pub fn describir_gpu() -> Option<Gpu> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
        DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
    };

    // SEGURIDAD: las cinco llamadas de este bloque son de solo lectura sobre objetos COM que la
    // propia DXGI gestiona, y ningún puntero sobrevive al final del bloque. `CreateDXGIFactory1`
    // solo pide a DXGI la factoría con la que enumerar adaptadores, sin tocar ningún hardware
    // todavía; `EnumAdapters1` y `cast` solo piden una referencia a un adaptador que DXGI ya
    // conoce; `GetDesc1` copia un descriptor a una estructura por valor; `QueryVideoMemoryInfo`
    // escribe en `info`, que es local a esta función y se libera al salir. Nada de lo que hay aquí
    // reserva memoria de vídeo ni altera el estado del adaptador: son consultas, no órdenes.
    unsafe {
        let factoria: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut mejor: Option<Gpu> = None;

        for indice in 0.. {
            let Ok(adaptador) = factoria.EnumAdapters1(indice) else {
                break; // Se acabaron los adaptadores.
            };
            let Ok(descriptor) = adaptador.GetDesc1() else {
                continue;
            };

            // El «Microsoft Basic Render Driver» declara 16 GB de presupuesto y cero memoria
            // dedicada. Sin este filtro sería el elegido en cuanto la GPU real estuviera ocupada.
            if descriptor.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            let vram_total = descriptor.DedicatedVideoMemory as u64;
            if vram_total == 0 {
                continue;
            }
            if mejor.as_ref().is_some_and(|m| m.vram_total >= vram_total) {
                continue;
            }

            let fin = descriptor
                .Description
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(descriptor.Description.len());
            let nombre = String::from_utf16_lossy(&descriptor.Description[..fin]);

            let vram_libre = match adaptador.cast::<IDXGIAdapter3>() {
                Ok(adaptador3) => {
                    let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                    match adaptador3.QueryVideoMemoryInfo(
                        0,
                        DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
                        &mut info,
                    ) {
                        // El presupuesto puede superar la memoria física, porque incluye lo que el
                        // sistema dejaría desbordar a RAM. Para decidir capas eso sería mentira, así
                        // que se recorta al total de la tarjeta.
                        Ok(()) => info
                            .Budget
                            .saturating_sub(info.CurrentUsage)
                            .min(vram_total),
                        Err(_) => continue,
                    }
                }
                // Sin `IDXGIAdapter3` no hay forma de saber cuánto queda libre, y suponerlo sería
                // justo el error que este módulo existe para evitar.
                Err(_) => continue,
            };

            mejor = Some(Gpu {
                nombre,
                vram_total,
                vram_libre,
            });
        }

        mejor
    }
}
