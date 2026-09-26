//! Lo único de Programator que habla con el sistema gráfico. Todo lo que sabe de la GPU sale de
//! aquí, y quien decide qué hacer con esos números vive en `motor::encaje`, que no incluye este
//! módulo: así la política se prueba sin tarjeta y la medición se puede cambiar sin tocarla.

/// Lo que se puede saber de la GPU sin cargar nada en ella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    pub nombre: String,
    /// Memoria que tiene la tarjeta, según su descriptor.
    pub vram_total: u64,
    /// Memoria que se puede pedir **ahora mismo**, ya descontada la presión del resto del sistema.
    pub vram_libre: u64,
}

/// La GPU dedicada con más memoria, o `None` si no se puede saber.
///
/// **`None` no es un error.** Una máquina sin GPU dedicada, un driver que no llega a
/// `IDXGIAdapter3` o un Windows que no expone DXGI son «no lo sé», y quien llama tiene que poder
/// distinguirlo de «hay 15 GiB libres», por el mismo motivo por el que `hay_novedades` devuelve
/// `Sondeo` y no un `bool`.
///
/// **Limitación declarada:** `Budget` es el presupuesto del proceso que pregunta, y quien va a
/// reservar la VRAM es `llama-server`, que es otro proceso. Corren en la misma máquina y ven
/// presupuestos equivalentes, así que la medida sirve, pero no es la misma cifra. Esa diferencia es
/// una de las razones de que exista el margen de `motor::encaje`.
#[cfg(windows)]
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

/// Fuera de Windows no hay DXGI. Programator es de Windows, así que esto no es una carencia: es la
/// forma de que el proyecto siga compilando y pasando la suite en otra plataforma.
#[cfg(not(windows))]
pub fn describir_gpu() -> Option<Gpu> {
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn describir_la_gpu_no_revienta_y_lo_que_devuelve_es_coherente() {
        // No se puede afirmar qué GPU hay: depende de la máquina, y en una sin GPU dedicada la
        // respuesta correcta es `None`. Lo que sí tiene que cumplirse siempre es que, si contesta,
        // lo que dice sea coherente.
        if let Some(gpu) = describir_gpu() {
            assert!(
                !gpu.nombre.trim().is_empty(),
                "una GPU sin nombre es sospechosa"
            );
            assert!(
                gpu.vram_total > 0,
                "el adaptador software debería estar filtrado"
            );
            assert!(
                gpu.vram_libre <= gpu.vram_total,
                "libre {} no puede superar al total {}",
                gpu.vram_libre,
                gpu.vram_total
            );
        }
    }

    /// Prueba manual, no de humo: imprime lo que ve `describir_gpu()` en esta máquina para
    /// compararlo a mano con la salida de la sonda desechable que verificó la Tarea 5. No se afirma
    /// nada por assert porque el resultado depende del hardware; el valor de esta prueba es la
    /// inspección visual con `--nocapture`, y se deja puesta para el futuro.
    #[test]
    #[ignore]
    fn imprime_lo_que_ve_describir_gpu_en_esta_maquina() {
        match describir_gpu() {
            Some(gpu) => println!(
                "GPU: {} | vram_total: {} | vram_libre: {}",
                gpu.nombre, gpu.vram_total, gpu.vram_libre
            ),
            None => println!("describir_gpu() devolvió None"),
        }
    }
}
