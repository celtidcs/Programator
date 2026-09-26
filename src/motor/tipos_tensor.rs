//! Tabla de cuantización de ggml: cuántos valores lleva un bloque de cada tipo de tensor y cuántos
//! bytes ocupa ese bloque.
//!
//! Vive fuera de `gguf` porque **no es propia del formato GGUF**: la usan tanto el peso de los
//! tensores como el caché KV, y un caché KV no es un fichero GGUF.

/// Cuántos valores lleva un bloque de un tipo de tensor y cuántos bytes ocupa ese bloque. En ggml
/// los tipos cuantizados no guardan valores sueltos sino bloques con sus escalas, así que el peso
/// de un tensor no es «elementos × algo»: es «bloques × tamaño de bloque».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TipoTensor {
    pub bloque: u64,
    pub tamano: u64,
}

/// Traduce el código de tipo que trae la tabla de tensores del GGUF.
///
/// Los pares salen de `ggml.h` (el `enum ggml_type`, que fija los códigos) y de los `static_assert`
/// de `ggml-common.h` (que fijan el tamaño de cada bloque), consultados el 15/09/2026. Los códigos
/// 4, 5, 31, 32, 33 y 36-38 existieron y están retirados: devolver `None` para ellos es correcto.
///
/// **Un código desconocido devuelve `None` y no se estima.** Un modelo con una cuantización nueva
/// hará que el encaje se declare incalculable, que es la respuesta honesta; inventar un tamaño
/// produciría un número creíble y falso, que es mucho peor que no tener número.
pub fn tipo_tensor(codigo: u32) -> Option<TipoTensor> {
    let (bloque, tamano) = match codigo {
        0 => (1, 4),      // F32
        1 => (1, 2),      // F16
        2 => (32, 18),    // Q4_0
        3 => (32, 20),    // Q4_1
        6 => (32, 22),    // Q5_0
        7 => (32, 24),    // Q5_1
        8 => (32, 34),    // Q8_0
        9 => (32, 36),    // Q8_1
        10 => (256, 84),  // Q2_K
        11 => (256, 110), // Q3_K
        12 => (256, 144), // Q4_K
        13 => (256, 176), // Q5_K
        14 => (256, 210), // Q6_K
        15 => (256, 292), // Q8_K
        16 => (256, 66),  // IQ2_XXS
        17 => (256, 74),  // IQ2_XS
        18 => (256, 98),  // IQ3_XXS
        19 => (256, 50),  // IQ1_S
        20 => (32, 18),   // IQ4_NL
        21 => (256, 110), // IQ3_S
        22 => (256, 82),  // IQ2_S
        23 => (256, 136), // IQ4_XS
        24 => (1, 1),     // I8
        25 => (1, 2),     // I16
        26 => (1, 4),     // I32
        27 => (1, 8),     // I64
        28 => (1, 8),     // F64
        29 => (256, 56),  // IQ1_M
        30 => (1, 2),     // BF16
        34 => (256, 54),  // TQ1_0
        35 => (256, 66),  // TQ2_0
        39 => (32, 17),   // MXFP4
        40 => (64, 36),   // NVFP4
        41 => (128, 18),  // Q1_0
        42 => (64, 18),   // Q2_0
        _ => return None,
    };
    Some(TipoTensor { bloque, tamano })
}

/// Bytes que ocupa un tensor de esas dimensiones y ese tipo.
///
/// Devuelve `None` si el producto de las dimensiones desborda un `u64`: un GGUF corrupto puede
/// declarar dimensiones absurdas, y multiplicarlas con envoltura daría un peso pequeño y falso que
/// haría creer al encaje que un modelo imposible cabe de sobra.
///
/// También devuelve `None` si el bloque es cero, que aunque no ocurre en la tabla estándar, podría
/// construirse a mano dado que los campos de `TipoTensor` son públicos, y una resta que desborda
/// entraría en pánico en modo depuración.
pub fn bytes_de_tensor(dimensiones: &[u64], tipo: TipoTensor) -> Option<u64> {
    if tipo.bloque == 0 {
        return None;
    }
    let mut elementos: u64 = 1;
    for dimension in dimensiones {
        elementos = elementos.checked_mul(*dimension)?;
    }
    // Redondeo hacia arriba: un bloque a medias sigue ocupando un bloque entero en el fichero.
    let bloques = elementos.checked_add(tipo.bloque - 1)? / tipo.bloque;
    bloques.checked_mul(tipo.tamano)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Por qué existe esta prueba, aunque ya haya otra («la_suma_de_los_tensores_cuadra_con_el_
    /// tamano_del_fichero», en `motor::gguf::tensores`) que también verifica la tabla contra un
    /// fichero: esa otra prueba se comprueba contra sí misma. Su GGUF sintético lo escribe
    /// `fichero_con_tensores`, que calcula el tamaño de cada tensor llamando a las mismas
    /// `tipo_tensor`/`bytes_de_tensor` que usa después el lector; si aquí `Q6_K` fuese 211 en vez
    /// de 210, el escritor escribiría 211, el lector leería 211 y esa prueba pasaría igual de
    /// contenta. Lo que falta es una segunda fuente independiente para comparar, y la única que
    /// hay es la propia especificación de ggml.
    ///
    /// Esta prueba es esa segunda fuente: compara cada entrada de `tipo_tensor` contra la fórmula
    /// **literal** de la que sale, tomada de los `static_assert` de `ggml-common.h` (repositorio
    /// `ggml-org/ggml`, consultados el 15/09/2026), escrita con sus constantes con nombre —
    /// `QK_K`, `K_SCALE_SIZE`, `ggml_half`, etc.— y no con el número que ya está copiado en la
    /// tabla. Es la única prueba de las dos que ataría un error de transcripción al copiar esa
    /// tabla a mano.
    ///
    /// Límite honesto: si la fórmula de aquí abajo y la entrada de la tabla estuvieran mal **de
    /// la misma manera** —las dos con un 209 en vez de un 210, por ejemplo—, esta prueba tampoco
    /// lo vería: solo compara dos textos, no mide nada del mundo. Lo que descarta es un error de
    /// transcripción entre la fuente y la tabla, que es el riesgo real: la tabla se transcribió
    /// una vez a mano y nada más la ha vuelto a comparar con la especificación.
    #[test]
    fn la_tabla_de_tipos_cuadra_con_las_formulas_de_ggml_common_h() {
        const GGML_HALF: u64 = 2; // sizeof(ggml_half): un f16 de 16 bits.
        const QK_K: u64 = 256;
        const K_SCALE_SIZE: u64 = 12;
        const IQ3S_N_SCALE: u64 = QK_K / 64;
        const QK4_0: u64 = 32;
        const QK4_1: u64 = 32;
        const QK5_0: u64 = 32;
        const QK5_1: u64 = 32;
        const QK8_0: u64 = 32;
        const QK8_1: u64 = 32;
        const QK4_NL: u64 = 32;
        const QK_MXFP4: u64 = 32;
        const QK_NVFP4: u64 = 64;
        const QK_NVFP4_SUB: u64 = 16;
        const QK1_0: u64 = 128;
        const QK2_0: u64 = 64;

        // (código ggml, bloque esperado, tamaño esperado según la fórmula del static_assert).
        let casos: &[(u32, u64, u64)] = &[
            (2, QK4_0, GGML_HALF + QK4_0 / 2),                   // Q4_0
            (3, QK4_1, 2 * GGML_HALF + QK4_1 / 2),               // Q4_1
            (6, QK5_0, GGML_HALF + 4 + QK5_0 / 2),               // Q5_0: el 4 es sizeof(uint32_t)
            (7, QK5_1, 2 * GGML_HALF + 4 + QK5_1 / 2),           // Q5_1
            (8, QK8_0, GGML_HALF + QK8_0),                       // Q8_0
            (9, QK8_1, 2 * GGML_HALF + QK8_1),                   // Q8_1
            (10, QK_K, 2 * GGML_HALF + QK_K / 16 + QK_K / 4),    // Q2_K
            (11, QK_K, GGML_HALF + QK_K / 4 + QK_K / 8 + 12),    // Q3_K
            (12, QK_K, 2 * GGML_HALF + K_SCALE_SIZE + QK_K / 2), // Q4_K
            (13, QK_K, 2 * GGML_HALF + K_SCALE_SIZE + QK_K / 2 + QK_K / 8), // Q5_K
            (14, QK_K, GGML_HALF + QK_K / 16 + 3 * QK_K / 4),    // Q6_K
            (15, QK_K, 4 + QK_K + (QK_K / 16) * 2), // Q8_K: 4 es sizeof(float), *2 es sizeof(int16_t)
            (16, QK_K, GGML_HALF + (QK_K / 8) * 2), // IQ2_XXS
            (17, QK_K, GGML_HALF + (QK_K / 8) * 2 + QK_K / 32), // IQ2_XS
            (18, QK_K, GGML_HALF + 3 * (QK_K / 8)), // IQ3_XXS
            (19, QK_K, GGML_HALF + QK_K / 8 + QK_K / 16), // IQ1_S
            (20, QK4_NL, GGML_HALF + QK4_NL / 2),   // IQ4_NL
            (21, QK_K, GGML_HALF + 13 * (QK_K / 32) + IQ3S_N_SCALE), // IQ3_S
            (22, QK_K, GGML_HALF + QK_K / 4 + QK_K / 16), // IQ2_S
            (23, QK_K, GGML_HALF + 2 + QK_K / 64 + QK_K / 2), // IQ4_XS: el 2 es sizeof(uint16_t)
            (29, QK_K, QK_K / 8 + QK_K / 16 + QK_K / 32), // IQ1_M
            (34, QK_K, GGML_HALF + QK_K / 64 + (QK_K - 4 * QK_K / 64) / 5), // TQ1_0
            (35, QK_K, GGML_HALF + QK_K / 4),       // TQ2_0
            (39, QK_MXFP4, 1 + QK_MXFP4 / 2),       // MXFP4
            (40, QK_NVFP4, (QK_NVFP4 / QK_NVFP4_SUB) + QK_NVFP4 / 2), // NVFP4
            (41, QK1_0, GGML_HALF + QK1_0 / 8),     // Q1_0
            (42, QK2_0, GGML_HALF + QK2_0 / 4),     // Q2_0
            // Tipos sin bloque: un valor suelto por elemento, bloque 1.
            (0, 1, 4),  // F32
            (1, 1, 2),  // F16
            (30, 1, 2), // BF16
            (28, 1, 8), // F64
            (24, 1, 1), // I8
            (25, 1, 2), // I16
            (26, 1, 4), // I32
            (27, 1, 8), // I64
        ];

        for &(codigo, bloque_esperado, tamano_esperado) in casos {
            let tipo = tipo_tensor(codigo)
                .unwrap_or_else(|| panic!("el código {codigo} debería ser un tipo conocido"));
            assert_eq!(
                tipo.bloque, bloque_esperado,
                "código {codigo}: el bloque de la tabla no cuadra con la fórmula de \
                 ggml-common.h"
            );
            assert_eq!(
                tipo.tamano, tamano_esperado,
                "código {codigo}: el tamaño de la tabla no cuadra con la fórmula de \
                 ggml-common.h"
            );
        }

        // Guarda de exhaustividad: sin esto, la lista de `casos` de arriba y la tabla de
        // `tipo_tensor` podrían desincronizarse en silencio. Si alguien añade un código nuevo a
        // `tipo_tensor` y olvida añadir su caso aquí, esta prueba seguiría en verde sin haber
        // comprobado nada del código nuevo. Se recorren todos los códigos de 0 a 64 —más que
        // suficiente para cubrir el rango que usa ggml hoy y el margen de códigos futuros
        // cercanos— y cualquiera que `tipo_tensor` reconozca tiene que estar en `casos`.
        let codigos_comprobados: std::collections::HashSet<u32> =
            casos.iter().map(|&(codigo, _, _)| codigo).collect();
        for codigo in 0..=64u32 {
            if tipo_tensor(codigo).is_some() {
                assert!(
                    codigos_comprobados.contains(&codigo),
                    "el código {codigo} se ha añadido a `tipo_tensor` pero no a los `casos` de \
                     esta prueba: su tamaño tiene que salir de la fórmula del `static_assert` \
                     correspondiente en ggml-common.h, no de copiar el número ya escrito en la \
                     tabla"
                );
            }
        }
    }

    #[test]
    fn un_bloque_de_q4_k_ocupa_144_bytes_cada_256_valores() {
        let tipo = tipo_tensor(12).expect("Q4_K es un tipo conocido");
        assert_eq!(tipo.bloque, 256);
        assert_eq!(tipo.tamano, 144);
        assert_eq!(bytes_de_tensor(&[256], tipo), Some(144));
    }

    #[test]
    fn f32_ocupa_cuatro_bytes_por_elemento() {
        let tipo = tipo_tensor(0).expect("F32 es un tipo conocido");
        assert_eq!(bytes_de_tensor(&[1000], tipo), Some(4000));
    }

    #[test]
    fn una_matriz_pesa_el_producto_de_sus_dimensiones() {
        let tipo = tipo_tensor(0).expect("F32");
        // 5120 × 32 = 163 840 elementos × 4 bytes.
        assert_eq!(bytes_de_tensor(&[5120, 32], tipo), Some(655_360));
    }

    #[test]
    fn un_tipo_desconocido_no_se_estima_sino_que_se_rinde() {
        assert!(tipo_tensor(200).is_none(), "código inventado");
        for retirado in [4u32, 5, 31, 32, 33, 36, 37, 38] {
            assert!(
                tipo_tensor(retirado).is_none(),
                "el código {retirado} está retirado de ggml y no debe aceptarse"
            );
        }
    }

    #[test]
    fn las_dimensiones_que_desbordan_no_entran_en_panico() {
        let tipo = tipo_tensor(0).expect("F32");
        assert_eq!(
            bytes_de_tensor(&[u64::MAX, 2], tipo),
            None,
            "el producto desborda: hay que rendirse, no envolver en silencio"
        );
    }

    #[test]
    fn un_recuento_que_no_llena_el_bloque_redondea_hacia_arriba() {
        let tipo = tipo_tensor(12).expect("Q4_K");
        // 257 valores no caben en un bloque de 256: hacen falta dos.
        assert_eq!(bytes_de_tensor(&[257], tipo), Some(288));
    }

    #[test]
    fn un_tipo_con_bloque_de_cero_no_entra_en_panico() {
        // No sale de `tipo_tensor`, pero los campos de `TipoTensor` son públicos y nada impide
        // construirlo a mano. Una resta que desborda sería un pánico en modo depuración.
        let absurdo = TipoTensor {
            bloque: 0,
            tamano: 4,
        };
        assert_eq!(bytes_de_tensor(&[256], absurdo), None);
    }
}
