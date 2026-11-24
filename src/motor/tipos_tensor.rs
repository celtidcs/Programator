//! Tabla de cuantización de ggml: cuántos valores lleva un bloque de cada tipo de tensor y cuántos
//! bytes ocupa ese bloque.
//!
//! Vive fuera de `gguf` porque **no es propia del formato GGUF**: la usan tanto el peso de los
//! tensores como el caché KV, y un caché KV no es un fichero GGUF.

use std::num::NonZeroU64;

/// Cuántos valores lleva un bloque de un tipo de tensor y cuántos bytes ocupa ese bloque. En ggml
/// los tipos cuantizados no guardan valores sueltos sino bloques con sus escalas, así que el peso
/// de un tensor no es «elementos × algo»: es «bloques × tamaño de bloque».
///
/// El bloque nunca es cero, y lo dice el tipo: antes era un `u64` público y
/// `bytes_de_tensor` tenía que vigilar a mano un bloque de cero construido fuera de la tabla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TipoTensor {
    pub bloque: NonZeroU64,
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
    let fila = TABLA.iter().find(|tipo| tipo.codigo == codigo)?;
    // Ninguna fila de la tabla tiene bloque cero (lo fija una prueba); si alguna lo tuviera, ese
    // tipo no se sabría pesar, que es lo honesto.
    Some(TipoTensor {
        bloque: NonZeroU64::new(fila.bloque)?,
        tamano: fila.tamano,
    })
}

/// El código de ggml de un tipo, por su nombre (`q8_0`, `bf16`…), sin distinguir mayúsculas.
/// `None` si no es ninguno de la tabla.
pub fn codigo_por_nombre(nombre: &str) -> Option<u32> {
    TABLA
        .iter()
        .find(|tipo| tipo.nombre.eq_ignore_ascii_case(nombre))
        .map(|tipo| tipo.codigo)
}

/// Una fila de la tabla: el código del `enum ggml_type`, el nombre con que lo escribe ggml, los
/// valores por bloque y los bytes por bloque.
struct FilaDeTipo {
    codigo: u32,
    nombre: &'static str,
    bloque: u64,
    tamano: u64,
}

const fn fila(codigo: u32, nombre: &'static str, bloque: u64, tamano: u64) -> FilaDeTipo {
    FilaDeTipo {
        codigo,
        nombre,
        bloque,
        tamano,
    }
}

/// La tabla única de tipos: también el caché KV la usa, sin copia propia de los códigos en
/// `motor::encaje`.
const TABLA: [FilaDeTipo; 35] = [
    fila(0, "f32", 1, 4),
    fila(1, "f16", 1, 2),
    fila(2, "q4_0", 32, 18),
    fila(3, "q4_1", 32, 20),
    fila(6, "q5_0", 32, 22),
    fila(7, "q5_1", 32, 24),
    fila(8, "q8_0", 32, 34),
    fila(9, "q8_1", 32, 36),
    fila(10, "q2_k", 256, 84),
    fila(11, "q3_k", 256, 110),
    fila(12, "q4_k", 256, 144),
    fila(13, "q5_k", 256, 176),
    fila(14, "q6_k", 256, 210),
    fila(15, "q8_k", 256, 292),
    fila(16, "iq2_xxs", 256, 66),
    fila(17, "iq2_xs", 256, 74),
    fila(18, "iq3_xxs", 256, 98),
    fila(19, "iq1_s", 256, 50),
    fila(20, "iq4_nl", 32, 18),
    fila(21, "iq3_s", 256, 110),
    fila(22, "iq2_s", 256, 82),
    fila(23, "iq4_xs", 256, 136),
    fila(24, "i8", 1, 1),
    fila(25, "i16", 1, 2),
    fila(26, "i32", 1, 4),
    fila(27, "i64", 1, 8),
    fila(28, "f64", 1, 8),
    fila(29, "iq1_m", 256, 56),
    fila(30, "bf16", 1, 2),
    fila(34, "tq1_0", 256, 54),
    fila(35, "tq2_0", 256, 66),
    fila(39, "mxfp4", 32, 17),
    fila(40, "nvfp4", 64, 36),
    fila(41, "q1_0", 128, 18),
    fila(42, "q2_0", 64, 18),
];

/// Bytes que ocupa un tensor de esas dimensiones y ese tipo.
///
/// Devuelve `None` si el producto de las dimensiones desborda un `u64`: un GGUF corrupto puede
/// declarar dimensiones absurdas, y multiplicarlas con envoltura daría un peso pequeño y falso que
/// haría creer al encaje que un modelo imposible cabe de sobra.
///
pub fn bytes_de_tensor(dimensiones: &[u64], tipo: TipoTensor) -> Option<u64> {
    let mut elementos: u64 = 1;
    for dimension in dimensiones {
        elementos = elementos.checked_mul(*dimension)?;
    }
    // Redondeo hacia arriba: un bloque a medias sigue ocupando un bloque entero en el fichero.
    let bloques = elementos.div_ceil(tipo.bloque.get());
    bloques.checked_mul(tipo.tamano)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Cada código y cada nombre aparecen una sola vez, y el nombre lleva a su código.
    #[test]
    fn la_tabla_no_repite_codigos_ni_nombres_y_el_nombre_lleva_al_codigo() {
        for (i, tipo) in TABLA.iter().enumerate() {
            assert!(
                TABLA[i + 1..]
                    .iter()
                    .all(|otro| otro.codigo != tipo.codigo && otro.nombre != tipo.nombre),
                "{} ({}) repetido",
                tipo.nombre,
                tipo.codigo
            );
            assert_eq!(codigo_por_nombre(tipo.nombre), Some(tipo.codigo));
            assert_eq!(
                codigo_por_nombre(&tipo.nombre.to_ascii_uppercase()),
                Some(tipo.codigo)
            );
        }
        assert_eq!(codigo_por_nombre("q3_k_m"), None);
    }

    /// Los códigos del caché que antes tenía `motor::encaje` por su cuenta (`ggml.h`).
    #[test]
    fn los_codigos_del_cache_son_los_de_ggml() {
        let esperados = [
            ("f32", 0),
            ("f16", 1),
            ("q4_0", 2),
            ("q4_1", 3),
            ("q5_0", 6),
            ("q5_1", 7),
            ("q8_0", 8),
            ("iq4_nl", 20),
            ("bf16", 30),
        ];
        for (nombre, codigo) in esperados {
            assert_eq!(codigo_por_nombre(nombre), Some(codigo), "{nombre}");
        }
    }

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
                tipo.bloque.get(),
                bloque_esperado,
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
        assert_eq!(tipo.bloque.get(), 256);
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

    /// Sustituye a `un_tipo_con_bloque_de_cero_no_entra_en_panico`: un `TipoTensor`
    /// con bloque cero no se puede construir. Lo que queda por vigilar es la tabla: si una fila
    /// trajera bloque cero, su tipo dejaría de pesarse sin avisar.
    #[test]
    fn ninguna_fila_de_la_tabla_tiene_bloque_cero() {
        for fila in TABLA {
            assert!(
                tipo_tensor(fila.codigo).is_some(),
                "la fila {} ({}) no da un tipo",
                fila.codigo,
                fila.nombre
            );
        }
    }

    /// Cerca del final de un u64 el redondeo sumaba `bloque - 1` y desbordaba: un tensor que sí
    /// se puede pesar salía como imposible. Con `div_ceil` no hay suma.
    #[test]
    fn el_redondeo_no_da_por_imposible_un_tensor_que_se_puede_pesar() {
        let tipo = tipo_tensor(12).expect("Q4_K es un tipo conocido");
        let bloques = 1u64 << 56; // u64::MAX / 256, redondeado hacia arriba
        assert_eq!(bytes_de_tensor(&[u64::MAX], tipo), Some(bloques * 144));
        // Y lo que de verdad no cabe en bytes sigue saliendo como `None`.
        let f32 = tipo_tensor(0).expect("F32 es un tipo conocido");
        assert_eq!(bytes_de_tensor(&[u64::MAX], f32), None);
    }
}
