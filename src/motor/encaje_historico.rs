//! El historial acumulativo de cómo se repartió el modelo entre GPU y CPU en cada arranque.
//!
//! **Por qué existe.** Cada arranque calcula el encaje y lo imprime una vez en consola, pero hasta
//! ahora no quedaba registrado en ningún sitio consultable después (visto en uso real). Si un
//! arranque reparte peor que el anterior —menos capas en GPU, por ejemplo—, no había forma de saber
//! si cambió algo real (el contexto configurado, la versión, el margen de seguridad) o si solo
//! había menos VRAM libre en ese instante por otra razón externa. Una línea por arranque convierte
//! esa pregunta en mirar dos líneas del fichero en vez de no poder saberlo.
//!
//! **Separado de `encaje` a propósito.** `encaje/reparto.rs` y `encaje::resolver` son la política,
//! aritmética pura o la orquestación que pesa el modelo; este módulo solo redacta una línea de
//! texto con lo que ya se decidió y la añade a un fichero. Mezclar las dos cosas haría que ninguna
//! se leyera entera.

use super::hardware::Gpu;
use super::mensajes::en_gib;
use crate::error::{Error, Resultado};
use std::io::Write;
use std::path::Path;

/// Lo que necesita una línea del historial de encaje, reunido en un sitio.
///
/// Van agrupados y no sueltos porque `linea_historico` ya pedía seis argumentos posicionales, el
/// mismo umbral a partir del cual `sesion::Ajustes` se introdujo para `ejecutar_pasada`: es donde
/// empiezan a confundirse entre sí al llamar.
pub struct DatosDeEncaje<'a> {
    /// Mismo formato que las marcas de tiempo del canal.
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

/// Lo que se sabe de la memoria al terminar un encargo. Las cifras que no se pudieron medir (sin
/// tarjeta medible, motor ajeno) van como `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memoria {
    pub modelo: String,
    pub capas: u32,
    /// Las que midió el banco con este modelo en esta tarjeta.
    pub capas_medidas: Option<u32>,
    /// Las capas del modelo, de su cabecera GGUF.
    pub capas_totales: Option<u32>,
    pub libre_antes_mib: Option<u64>,
    pub motor_mib: Option<u64>,
    pub libre_despues_mib: Option<u64>,
}

use crate::unidades::SIN_MEDIR;

/// La línea de memoria de una entrega: modelo, capas en la tarjeta, VRAM libre antes de cargar,
/// VRAM que ocupa el motor y VRAM libre al terminar. Se separa con `·` y nunca con `|`: el banco
/// cuenta como fila de tabla toda línea con tres barras.
pub fn linea_de_memoria(memoria: &Memoria) -> String {
    let mib = |cifra: Option<u64>| {
        cifra
            .map(|mib| format!("{mib} MiB"))
            .unwrap_or_else(|| SIN_MEDIR.to_string())
    };
    // El motor cuenta la capa de salida como una más: con todo el modelo en la tarjeta pide
    // `total + 1` capas (o 99, por «todas»), y «34 de 33» parecería un error.
    let capas = match memoria.capas_totales {
        Some(total) if memoria.capas > total => {
            format!("las {total} capas y la de salida en la tarjeta")
        }
        Some(total) => format!("{} de {total} capas en la tarjeta", memoria.capas),
        None => format!("{} capas en la tarjeta", memoria.capas),
    };
    format!(
        "Memoria: {} · {capas} · libre antes de cargar: {} · ocupa el motor: {} · libre al \
         terminar: {}",
        memoria.modelo,
        mib(memoria.libre_antes_mib),
        mib(memoria.motor_mib),
        mib(memoria.libre_despues_mib)
    )
}

/// El aviso de la entrega cuando el encaje dejó menos capas en la tarjeta que las que midió el
/// banco: el encargo se atendió igual, más lento. Las dos cifras se acotan a `total + 1` (las
/// capas más la de salida), porque el banco puede haber guardado 99 por «todas».
pub fn aviso_menos_capas(memoria: &Memoria) -> Option<String> {
    let medidas = memoria.capas_medidas?;
    let acotar = |capas: u32| match memoria.capas_totales {
        Some(total) => capas.min(total.saturating_add(1)),
        None => capas,
    };
    let (usadas, medidas) = (acotar(memoria.capas), acotar(medidas));
    (usadas < medidas).then(|| {
        format!(
            "⚠️ No cabían todas las capas: hay {usadas} en la tarjeta y el banco midió este \
             modelo con {medidas}. El encargo se ha atendido igual, más lento; libera memoria de \
             la tarjeta o baja el contexto para recuperar la velocidad medida."
        )
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

    fn memoria() -> Memoria {
        Memoria {
            modelo: "devstral".to_string(),
            capas: 41,
            capas_medidas: Some(41),
            capas_totales: Some(40),
            libre_antes_mib: Some(15_000),
            motor_mib: Some(13_763),
            libre_despues_mib: Some(1_200),
        }
    }

    #[test]
    fn la_linea_de_memoria_trae_todas_las_cifras_y_ninguna_barra() {
        let linea = linea_de_memoria(&memoria());
        assert_eq!(
            linea,
            "Memoria: devstral · las 40 capas y la de salida en la tarjeta · libre antes de \
             cargar: 15000 MiB · \
             ocupa el motor: 13763 MiB · libre al terminar: 1200 MiB"
        );
        assert!(!linea.contains('|'));
    }

    #[test]
    fn con_parte_del_modelo_en_la_tarjeta_dice_cuantas_de_cuantas() {
        let linea = linea_de_memoria(&Memoria {
            capas: 32,
            ..memoria()
        });
        assert!(
            linea.contains("· 32 de 40 capas en la tarjeta ·"),
            "{linea}"
        );
    }

    #[test]
    fn con_tantas_capas_como_el_total_dice_cuantas_de_cuantas() {
        // Sin la de salida, el modelo no está entero en la tarjeta: «las 40 capas y la de salida»
        // sería mentira.
        let linea = linea_de_memoria(&Memoria {
            capas: 40,
            ..memoria()
        });
        assert!(
            linea.contains("· 40 de 40 capas en la tarjeta ·"),
            "{linea}"
        );
    }

    #[test]
    fn con_todo_el_modelo_en_la_tarjeta_no_dice_mas_capas_que_el_total() {
        for capas in [41, 99] {
            let linea = linea_de_memoria(&Memoria { capas, ..memoria() });
            assert!(
                linea.contains("· las 40 capas y la de salida en la tarjeta ·"),
                "{linea}"
            );
        }
    }

    #[test]
    fn sin_tarjeta_medible_las_cifras_dicen_que_no_se_midieron() {
        let linea = linea_de_memoria(&Memoria {
            capas_totales: None,
            libre_antes_mib: None,
            motor_mib: None,
            libre_despues_mib: None,
            ..memoria()
        });
        assert!(linea.contains("41 capas en la tarjeta"), "{linea}");
        assert_eq!(linea.matches("sin medir").count(), 3, "{linea}");
    }

    #[test]
    fn con_menos_capas_que_las_medidas_se_avisa_y_con_las_mismas_no() {
        assert_eq!(aviso_menos_capas(&memoria()), None);
        let menos = Memoria {
            capas: 30,
            ..memoria()
        };
        let aviso = aviso_menos_capas(&menos).unwrap();
        assert!(
            aviso.contains("hay 30") && aviso.contains("con 41"),
            "{aviso}"
        );
        assert_eq!(
            aviso_menos_capas(&Memoria {
                capas_medidas: None,
                ..menos
            }),
            None
        );
    }

    #[test]
    fn el_99_del_banco_cuenta_como_todas_las_capas() {
        let todas = Memoria {
            capas_medidas: Some(99),
            ..memoria()
        };
        assert_eq!(aviso_menos_capas(&todas), None);
    }
}
