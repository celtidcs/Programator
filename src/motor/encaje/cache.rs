//! Cuánto pesa el caché KV: qué tipos de caché admite `llama-server` y cuántos bytes gasta una capa
//! con un contexto y una geometría de atención dados. Aritmética pura, sin disco ni GPU.

use crate::motor::tipos_tensor::{bytes_de_tensor, codigo_por_nombre, tipo_tensor, TipoTensor};

/// Los tipos que `llama-server` admite en `--cache-type-k` y `--cache-type-v`: el caché no acepta
/// las cuantizaciones de bloque grande que sí valen para los pesos. Su código y su tamaño salen de
/// la tabla única de `tipos_tensor`.
const TIPOS_DE_CACHE: [&str; 9] = [
    "f32", "f16", "q4_0", "q4_1", "q5_0", "q5_1", "q8_0", "iq4_nl", "bf16",
];

/// Traduce el nombre que se escribe en `[motor] cache_kv` al tipo de ggml correspondiente, sin
/// distinguir mayúsculas. `None` si no es un tipo de caché.
pub(super) fn tipo_de_cache(nombre: &str) -> Option<TipoTensor> {
    if !TIPOS_DE_CACHE
        .iter()
        .any(|admitido| admitido.eq_ignore_ascii_case(nombre))
    {
        return None;
    }
    tipo_tensor(codigo_por_nombre(nombre)?)
}

/// Bytes de caché KV que consume **una** capa alojada en la GPU.
///
/// Son dos tensores, el de claves y el de valores, de `contexto × cabezas_kv × dim_cabeza`
/// elementos cada uno. Se calcula reutilizando `bytes_de_tensor` en vez de repetir la aritmética de
/// bloques: el caché se cuantiza exactamente igual que los pesos.
///
/// **Por capa y no por modelo, a propósito:** `llama.cpp` aloja el caché de cada capa en el mismo
/// backend donde vive la capa, así que las que se quedan en CPU no gastan VRAM. Sin esto, el
/// reparto parcial no se podría calcular.
pub(super) fn kv_por_capa(
    contexto: u64,
    cabezas_kv: u64,
    dim_cabeza: u64,
    cache: TipoTensor,
) -> Option<u64> {
    bytes_de_tensor(&[2, contexto, cabezas_kv, dim_cabeza], cache)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_cache_de_devstral_a_32k_pide_89_mib_por_capa() {
        // Devstral Small 2 24B: 5120 de embedding entre 32 cabezas son 160 por cabeza, y 8 cabezas
        // de KV. Con q8_0 (34 bytes cada 32 valores) y 32 768 de contexto.
        let cache = tipo_de_cache("q8_0").expect("q8_0 es un tipo de caché válido");
        assert_eq!(kv_por_capa(32_768, 8, 160, cache), Some(89_128_960));
    }

    #[test]
    fn el_cache_en_f16_pesa_casi_el_doble_que_en_q8_0() {
        let en_f16 = kv_por_capa(32_768, 8, 160, tipo_de_cache("f16").unwrap()).unwrap();
        let en_q8 = kv_por_capa(32_768, 8, 160, tipo_de_cache("q8_0").unwrap()).unwrap();
        assert_eq!(en_f16, 167_772_160);
        assert!(en_f16 > en_q8 && en_f16 < en_q8 * 2);
    }

    #[test]
    fn el_cache_crece_en_proporcion_al_contexto() {
        let cache = tipo_de_cache("q8_0").unwrap();
        let corto = kv_por_capa(4_096, 8, 160, cache).unwrap();
        let largo = kv_por_capa(32_768, 8, 160, cache).unwrap();
        assert_eq!(largo, corto * 8);
    }

    #[test]
    fn un_tipo_de_cache_inventado_no_se_acepta() {
        assert!(
            tipo_de_cache("q3_k").is_none(),
            "no vale para caché en llama.cpp"
        );
        assert!(tipo_de_cache("").is_none());
        assert!(
            tipo_de_cache("Q8_0").is_some(),
            "el nombre no distingue mayúsculas"
        );
    }

    #[test]
    fn un_contexto_absurdo_no_entra_en_panico() {
        let cache = tipo_de_cache("f16").unwrap();
        assert_eq!(kv_por_capa(u64::MAX, u64::MAX, u64::MAX, cache), None);
    }
}
