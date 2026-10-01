//! El historial acumulativo de cómo se repartió el modelo entre GPU y CPU en cada arranque.
//!
//! **Por qué existe.** Cada arranque calcula el encaje y lo imprime una vez en consola, pero hasta
//! ahora no quedaba registrado en ningún sitio consultable después (INC-N07 de NatureLand). Si un
//! arranque reparte peor que el anterior —menos capas en GPU, por ejemplo—, no había forma de saber
//! si cambió algo real (el contexto configurado, la versión, el margen de seguridad) o si solo
//! había menos VRAM libre en ese instante por otra razón externa. Una línea por arranque convierte
//! esa pregunta en mirar dos líneas del fichero en vez de no poder saberlo.
//!
//! **Separado de `encaje` a propósito.** `encaje::decidir` y `encaje::resolver` son la política,
//! aritmética pura o la orquestación que pesa el modelo; este módulo solo redacta una línea de
//! texto con lo que ya se decidió y la añade a un fichero. Mezclar las dos cosas haría que ninguna
//! se leyera entera.

use super::encaje::en_gib;
use super::hardware::Gpu;
use crate::error::{Error, Resultado};
use std::io::Write;
use std::path::Path;

/// Lo que necesita una línea del historial de encaje, reunido en un sitio.
///
/// Van agrupados y no sueltos porque `linea_historico` ya pedía seis argumentos posicionales, el
/// mismo umbral a partir del cual `sesion::Ajustes` se introdujo para `ejecutar_pasada`: es donde
/// empiezan a confundirse entre sí al llamar.
pub struct DatosDeEncaje<'a> {
    /// Formato §3.2 del protocolo, igual que las marcas de tiempo del canal.
    pub marca_tiempo: &'a str,
    pub gpu: Option<&'a Gpu>,
    pub contexto: u32,
    pub capas_en_gpu: u32,
    /// Capas totales del modelo, si se conocen. `None` cuando no se pudo pesar el GGUF.
    pub capas_totales: Option<u32>,
    pub version: &'a str,
}

/// Redacta una línea del historial de encaje.
///
/// Función pura: no toca disco, ni reloj, ni GPU. La marca de tiempo entra por parámetro, igual
/// que ya hace `protocolo::latido`, para poder probarla sin esperar ni depender de la hora del
/// sistema.
pub fn linea_historico(datos: &DatosDeEncaje) -> String {
    let vram = match datos.gpu {
        Some(gpu) => format!(
            "{} libres de {}",
            en_gib(gpu.vram_libre),
            en_gib(gpu.vram_total)
        ),
        None => "GPU no detectada".to_string(),
    };
    let capas_en_gpu = datos.capas_en_gpu;
    let capas = match datos.capas_totales {
        Some(total) => format!("{capas_en_gpu} de {total} capas en GPU"),
        None => format!("{capas_en_gpu} capas en GPU"),
    };
    let marca_tiempo = datos.marca_tiempo;
    let version = datos.version;
    let contexto = datos.contexto;
    format!("{marca_tiempo} | v{version} | {vram} | contexto {contexto} | {capas}")
}

/// Añade una línea al historial de encaje, creando el fichero y la carpeta del agente si hace
/// falta.
///
/// Un fallo aquí **no puede tumbar el arranque**: es un registro de transparencia, no una garantía
/// funcional. Quien llame decide qué avisar; esta función solo informa del error.
pub fn registrar(ruta: &Path, linea: &str) -> Resultado<()> {
    if let Some(padre) = ruta.parent() {
        std::fs::create_dir_all(padre).map_err(|causa| Error::Escritura {
            ruta: padre.to_path_buf(),
            causa,
        })?;
    }
    let mut fichero = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ruta)
        .map_err(|causa| Error::Escritura {
            ruta: ruta.to_path_buf(),
            causa,
        })?;
    writeln!(fichero, "{linea}").map_err(|causa| Error::Escritura {
        ruta: ruta.to_path_buf(),
        causa,
    })
}

/// Redacta el aviso emitido cuando no se pudo escribir el historial de encaje.
pub fn mensaje_fallo_registrar(ruta: &Path, error: &Error) -> String {
    format!(
        "⚠️ No se pudo escribir el historial de encaje en {}: {error}.\n\
         Consecuencia: no queda constancia de este arranque en el log acumulativo; el arranque en \
         sí no se ve afectado.\n\
         Qué hacer: comprueba permisos de escritura en esa carpeta. Si el fallo persiste, no hace \
         falta actuar más allá de eso: el historial se retoma solo en el próximo arranque que sí \
         pueda escribir.",
        ruta.display()
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn gpu_de_prueba() -> Gpu {
        Gpu {
            nombre: "RTX 4070 Ti SUPER".to_string(),
            vram_total: 16_376 * 1024 * 1024,
            vram_libre: 14_482 * 1024 * 1024,
        }
    }

    #[test]
    fn una_linea_con_gpu_y_reparto_parcial_trae_todos_los_datos() {
        let gpu = gpu_de_prueba();
        let linea = linea_historico(&DatosDeEncaje {
            marca_tiempo: "2026-10-01 16:24",
            gpu: Some(&gpu),
            contexto: 16384,
            capas_en_gpu: 39,
            capas_totales: Some(40),
            version: "0.10.4",
        });

        assert!(linea.starts_with("2026-10-01 16:24"));
        assert!(linea.contains("v0.10.4"));
        assert!(linea.contains("GiB"));
        assert!(linea.contains("libres de"));
        assert!(linea.contains("contexto 16384"));
        assert!(linea.contains("39 de 40 capas en GPU"));
    }

    #[test]
    fn sin_gpu_detectada_lo_dice_en_vez_de_inventar_una_cifra() {
        let linea = linea_historico(&DatosDeEncaje {
            marca_tiempo: "2026-10-01 16:24",
            gpu: None,
            contexto: 16384,
            capas_en_gpu: 99,
            capas_totales: None,
            version: "0.10.4",
        });

        assert!(linea.contains("GPU no detectada"));
        assert!(linea.contains("99 capas en GPU"));
    }

    #[test]
    fn sin_capas_totales_conocidas_no_las_inventa() {
        let gpu = gpu_de_prueba();
        let linea = linea_historico(&DatosDeEncaje {
            marca_tiempo: "2026-10-01 16:24",
            gpu: Some(&gpu),
            contexto: 16384,
            capas_en_gpu: 99,
            capas_totales: None,
            version: "0.10.4",
        });

        // Sin total conocido, la línea dice «99 capas en GPU» y no «99 de N capas en GPU».
        assert_eq!(linea.matches("99 capas en GPU").count(), 1);
        assert!(!linea.contains("de 99 capas") && !linea.contains("99 de"));
    }

    #[test]
    fn registrar_crea_el_fichero_y_la_carpeta_si_no_existen() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("sub").join("encaje-historico.log");

        registrar(&ruta, "primera línea").unwrap();

        let contenido = std::fs::read_to_string(&ruta).unwrap();
        assert_eq!(contenido, "primera línea\n");
    }

    #[test]
    fn registrar_dos_veces_acumula_sin_sobrescribir() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("encaje-historico.log");

        registrar(&ruta, "primer arranque").unwrap();
        registrar(&ruta, "segundo arranque").unwrap();

        let contenido = std::fs::read_to_string(&ruta).unwrap();
        assert_eq!(contenido, "primer arranque\nsegundo arranque\n");
    }

    #[test]
    fn mensaje_fallo_registrar_explica_fallo_consecuencia_y_accion() {
        let ruta = Path::new("/trabajo/.gestor/programator/encaje-historico.log");
        let error = Error::Escritura {
            ruta: ruta.to_path_buf(),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "bloqueado"),
        };

        let mensaje = mensaje_fallo_registrar(ruta, &error);

        assert!(mensaje.contains("No se pudo escribir el historial de encaje"));
        assert!(mensaje.contains("Consecuencia:"));
        assert!(mensaje.contains("Qué hacer:"));
    }
}
