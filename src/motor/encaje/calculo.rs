//! El encaje de un modelo ya leído: con la cabecera y los pesos del GGUF en la mano, la
//! configuración y la memoria libre, calcula el caché KV de todas las conversaciones y decide el
//! reparto. No abre ningún fichero: la lectura del GGUF la hace quien llama.

use super::cache::{kv_por_capa, tipo_de_cache};
use super::en_cpu::{self, pesos_en_la_tarjeta, EnCpu};
use super::reparto::{coste_de_la_decision, decidir};
use super::Encaje;
use crate::config::MotorConfig;
use crate::motor::gguf::tensores::PesoDelModelo;
use crate::motor::gguf::{InformeGguf, MetadatosModelo};
use crate::motor::mensajes;
use crate::unidades::BYTES_POR_MIB;

/// Lo que sale del cálculo: la decisión, lo que costó tomarla y qué parte de las capas se deja en
/// la CPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Calculo {
    pub(super) encaje: Encaje,
    /// El coste en bytes que le costó a `decidir` tomar la decisión, `None` cuando no hubo pesos
    /// con los que calcular ninguno: `redactar_aviso` necesita esa cifra y no tiene acceso a los
    /// pesos ni a la caché.
    pub(super) coste: Option<u64>,
    pub(super) en_cpu: EnCpu,
}

/// Calcula el caché y decide. Cada dato que falte produce un `NoCalculable` con **ese** motivo
/// escrito, no uno genérico: quien lea el aviso tiene que saber qué arreglar.
///
/// Lo que va a la CPU se descuenta antes de decidir: la caché entera con `cache_en_cpu`, y de cada
/// capa los expertos o las FFN que digan `expertos_en_cpu` y `ffn_en_cpu`, con `"auto"` ya
/// resuelto.
pub(super) fn calcular_con_el_informe(
    config: &MotorConfig,
    informe: &InformeGguf,
    vram_libre: u64,
) -> Calculo {
    let kv = match kv_de_todas_las_conversaciones(config, &informe.metadatos) {
        Ok(kv) => kv,
        Err(motivo) => {
            return Calculo {
                encaje: Encaje::NoCalculable { motivo },
                coste: None,
                en_cpu: EnCpu::sin_calcular(&config.reparto),
            }
        }
    };
    // Con la caché en la memoria del equipo, las capas de la tarjeta no reservan nada para ella.
    let kv_en_la_tarjeta = if config.reparto.cache_en_cpu { 0 } else { kv };

    let margen = config.margen_vram_mib.saturating_mul(BYTES_POR_MIB);
    let decide = |pesos: &PesoDelModelo| decidir(pesos, kv_en_la_tarjeta, vram_libre, margen);
    let en_cpu = en_cpu::resolver(&config.reparto, &informe.pesos, decide);
    let pesos = pesos_en_la_tarjeta(&informe.pesos, en_cpu);
    let encaje = decide(&pesos);
    let coste = coste_de_la_decision(&encaje, &pesos, kv_en_la_tarjeta);
    Calculo {
        encaje,
        coste,
        en_cpu,
    }
}

/// Los bytes de tarjeta que el cálculo atribuye a una carga ya hecha: `capas` capas en la tarjeta,
/// como se pasan a `--n-gpu-layers`, y `en_cpu` en la CPU. Es la cifra con la que se compara lo
/// que ocupó de verdad. `None` si falta algún dato para calcularla.
pub(super) fn vram_de_la_carga(
    config: &MotorConfig,
    informe: &InformeGguf,
    capas: u32,
    en_cpu: EnCpu,
) -> Option<u64> {
    let kv = kv_de_todas_las_conversaciones(config, &informe.metadatos).ok()?;
    let kv_en_la_tarjeta = if config.reparto.cache_en_cpu { 0 } else { kv };
    let pesos = pesos_en_la_tarjeta(&informe.pesos, en_cpu);
    let total = u32::try_from(pesos.por_capa.len()).ok()?;
    // El mismo convenio que `capas_para_llama`: pasar del número de capas sube también la salida.
    let encaje = match capas {
        0 => return Some(0),
        n if n > total => Encaje::Completo { total },
        en_gpu => Encaje::Parcial { en_gpu, total },
    };
    coste_de_la_decision(&encaje, &pesos, kv_en_la_tarjeta)
}

/// Bytes de caché KV por capa, sumadas todas las conversaciones en paralelo. `Err` trae el motivo,
/// ya redactado, de por qué no se pudo calcular.
fn kv_de_todas_las_conversaciones(
    config: &MotorConfig,
    metadatos: &MetadatosModelo,
) -> Result<u64, String> {
    let claves = tipo_de_cache(config.cache_k())
        .ok_or_else(|| mensajes::cache_desconocida(config.cache_k()))?;
    let valores = tipo_de_cache(config.cache_v())
        .ok_or_else(|| mensajes::cache_desconocida(config.cache_v()))?;
    let (embedding, cabezas, cabezas_kv) = geometria_de_atencion(metadatos)?;

    let kv_de_una_secuencia = kv_por_capa(
        config.contexto as u64,
        cabezas_kv,
        embedding / cabezas,
        claves,
        valores,
    )
    .ok_or_else(|| mensajes::CACHE_DESBORDADA.to_string())?;

    // La caché se reserva **una vez por secuencia**, no una por modelo: `llama-server` da a cada
    // slot de `--parallel` su propio espacio de contexto. Contarla una sola vez era decidir el
    // reparto sobre un presupuesto que no existía, y es lo que hacía que con los cuatro slots por
    // defecto del motor se ofrecieran 35 capas de 40 donde en realidad no cabían ni esas.
    kv_de_una_secuencia
        .checked_mul(config.paralelo.max(1) as u64)
        .ok_or_else(|| mensajes::cache_paralela_desbordada(config.paralelo))
}

/// La longitud de embedding, las cabezas de atención y las de KV que declara la cabecera, con las
/// cabezas de atención distintas de cero (se divide por ellas).
///
/// Separadas en tres comprobaciones y no en una desestructuración conjunta: un único texto
/// para las tres claves no le dice al usuario cuál de ellas falta, y esa es la única pista
/// que tiene para saber qué mirar en el GGUF.
fn geometria_de_atencion(metadatos: &MetadatosModelo) -> Result<(u64, u64, u64), String> {
    let embedding = metadatos
        .longitud_embedding
        .ok_or_else(|| mensajes::falta_en_la_cabecera("embedding_length"))?;
    let cabezas = metadatos
        .cabezas_atencion
        .ok_or_else(|| mensajes::falta_en_la_cabecera("attention.head_count"))?;
    let cabezas_kv = metadatos
        .cabezas_atencion_kv
        .ok_or_else(|| mensajes::falta_en_la_cabecera("attention.head_count_kv"))?;
    if cabezas == 0 {
        return Err(mensajes::CERO_CABEZAS.to_string());
    }
    Ok((embedding, cabezas, cabezas_kv))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La clave que falta se nombra, y se nombra la primera que falta en el orden de la cabecera.
    #[test]
    fn la_geometria_nombra_la_primera_clave_que_falta() {
        let mut metadatos = MetadatosModelo::default();
        assert_eq!(
            geometria_de_atencion(&metadatos),
            Err(mensajes::falta_en_la_cabecera("embedding_length"))
        );
        metadatos.longitud_embedding = Some(512);
        assert_eq!(
            geometria_de_atencion(&metadatos),
            Err(mensajes::falta_en_la_cabecera("attention.head_count"))
        );
        metadatos.cabezas_atencion = Some(0);
        assert_eq!(
            geometria_de_atencion(&metadatos),
            Err(mensajes::falta_en_la_cabecera("attention.head_count_kv"))
        );
        metadatos.cabezas_atencion_kv = Some(2);
        assert_eq!(
            geometria_de_atencion(&metadatos),
            Err(mensajes::CERO_CABEZAS.to_string())
        );
        metadatos.cabezas_atencion = Some(8);
        assert_eq!(geometria_de_atencion(&metadatos), Ok((512, 8, 2)));
    }
}
