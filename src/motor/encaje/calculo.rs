//! El encaje de un modelo ya leído: con la cabecera y los pesos del GGUF en la mano, la
//! configuración y la memoria libre, calcula el caché KV de todas las conversaciones y decide el
//! reparto. No abre ningún fichero: la lectura del GGUF la hace quien llama.

use super::cache::{kv_por_capa, tipo_de_cache};
use super::reparto::{coste_de_la_decision, decidir};
use super::Encaje;
use crate::config::MotorConfig;
use crate::motor::gguf::{InformeGguf, MetadatosModelo};
use crate::motor::mensajes;
use crate::unidades::BYTES_POR_MIB;

/// Calcula el caché y decide. Cada dato que falte produce un `NoCalculable` con **ese** motivo
/// escrito, no uno genérico: quien lea el aviso tiene que saber qué arreglar. Devuelve, junto con
/// la decisión, el coste en bytes que le costó a `decidir` tomarla —`None` cuando no hubo pesos con
/// los que calcular ninguno—, porque `redactar_aviso` necesita esa cifra y no tiene acceso a
/// `informe.pesos` ni a `kv`.
pub(super) fn calcular_con_el_informe(
    config: &MotorConfig,
    informe: &InformeGguf,
    vram_libre: u64,
) -> (Encaje, Option<u64>) {
    let kv = match kv_de_todas_las_conversaciones(config, &informe.metadatos) {
        Ok(kv) => kv,
        Err(motivo) => return (Encaje::NoCalculable { motivo }, None),
    };

    let margen = config.margen_vram_mib.saturating_mul(BYTES_POR_MIB);
    let encaje = decidir(&informe.pesos, kv, vram_libre, margen);
    let coste = coste_de_la_decision(&encaje, &informe.pesos, kv);
    (encaje, coste)
}

/// Bytes de caché KV por capa, sumadas todas las conversaciones en paralelo. `Err` trae el motivo,
/// ya redactado, de por qué no se pudo calcular.
fn kv_de_todas_las_conversaciones(
    config: &MotorConfig,
    metadatos: &MetadatosModelo,
) -> Result<u64, String> {
    let cache = tipo_de_cache(&config.cache_kv)
        .ok_or_else(|| mensajes::cache_desconocida(&config.cache_kv))?;
    let (embedding, cabezas, cabezas_kv) = geometria_de_atencion(metadatos)?;

    let kv_de_una_secuencia = kv_por_capa(
        config.contexto as u64,
        cabezas_kv,
        embedding / cabezas,
        cache,
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
