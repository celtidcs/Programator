//! Lo único de Programator que habla con la tarjeta gráfica. Todo lo que sabe de la GPU sale de
//! aquí, y quien decide qué hacer con esos números vive en `motor::encaje`, que no incluye este
//! módulo: así la política se prueba sin tarjeta y la medición se puede cambiar sin tocarla.
//!
//! **Hay una forma de medir por plataforma, y este módulo solo elige.** En Windows la cifra sale
//! de DXGI; en Linux, de preguntarle a `nvidia-smi`. Cada una vive en su submódulo, porque son dos
//! mecanismos distintos para responder la misma pregunta y mezclarlos aquí haría que ninguno se
//! pudiera leer entero. Lo común —el tipo `Gpu` y el contrato de abajo— se queda en este fichero.
//!
//! **`None` no es un error, es «no lo sé».** Una máquina sin GPU dedicada, un controlador que no
//! contesta o una plataforma sin forma de medir devuelven `None`, y quien llama tiene que poder
//! distinguir eso de «hay 15 GiB libres». Es el mismo motivo por el que `hay_novedades` devuelve
//! `Sondeo` y no un `bool`: confundir la ignorancia con un dato es justo el error que hace
//! diagnosticar mal.
//!
//! **La cifra de memoria libre es una foto, no una reserva.** Quien va a ocupar la tarjeta es
//! `llama-server`, que es otro proceso y arranca después. Entre la medida y la carga puede cambiar
//! lo que haya abierto en la máquina, y por eso existe el margen de seguridad de `motor::encaje`.

/// Lo que se puede saber de la GPU sin cargar nada en ella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    pub nombre: String,
    /// Memoria que tiene la tarjeta, según su descriptor.
    pub vram_total: u64,
    /// Memoria que se puede pedir **ahora mismo**, ya descontada la presión del resto del sistema.
    pub vram_libre: u64,
}

#[cfg(windows)]
mod dxgi;
#[cfg(windows)]
pub use dxgi::describir_gpu;

// El submódulo se compila donde se usa —Linux— y además siempre que se compilan las pruebas, en
// cualquier plataforma. Así su análisis de texto, que es la parte que puede equivocarse, se
// verifica en cada tanda aunque la tanda se haga desde Windows, y no queda código muerto en el
// binario de quien no lo necesita.
#[cfg(any(target_os = "linux", test))]
mod nvidia_smi;
#[cfg(target_os = "linux")]
pub use nvidia_smi::describir_gpu;

/// En una plataforma sin forma de medir, la respuesta honesta es «no lo sé».
///
/// No es una carencia disimulada: `None` significa exactamente eso, y quien llama ya sabe
/// distinguirlo de una cifra.
#[cfg(not(any(windows, target_os = "linux")))]
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
