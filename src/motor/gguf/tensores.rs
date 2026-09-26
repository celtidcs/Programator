//! Tabla de tensores de un GGUF: qué capa pesa cuánto. Vive separado de `gguf/mod.rs` porque la
//! cabecera y la tabla de tensores son dos secciones distintas del fichero, y separarlas deja a
//! cada módulo hablando de una sola cosa.

use super::super::tipos_tensor::{bytes_de_tensor, tipo_tensor};
use super::lector::LectorAcotado;
use super::{error_gguf, Cabecera};
use crate::error::Resultado;
use std::io::Read;
use std::path::Path;

/// Tope de dimensiones por tensor. ggml admite cuatro; ocho da margen a un formato futuro sin
/// aceptar que un fichero corrupto pida reservar un vector enorme.
const LIMITE_DIMENSIONES: u32 = 8;

/// Tope de entradas de la tabla de tensores. Un modelo de 70B ronda el millar; cien mil es
/// generoso sin dejar que un contador inventado obligue a iterar sin fin.
pub(super) const LIMITE_NUMERO_TENSORES: u64 = 100_000;

/// Alineación por defecto del bloque de datos cuando el GGUF no trae `general.alignment`. La fija
/// la especificación del formato.
const ALINEACION_POR_DEFECTO: u64 = 32;

/// El peso del modelo, repartido por donde va a acabar cargándose.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PesoDelModelo {
    /// Bytes de cada capa repetida, indexados por su número.
    pub por_capa: Vec<u64>,
    /// Bytes de todo lo que no pertenece a ninguna capa: embeddings, salida y normalización final.
    pub fuera_de_capa: u64,
    /// Posición del primer byte del bloque de datos.
    pub inicio_datos: u64,
    /// Byte siguiente al último tensor, medido desde `inicio_datos`.
    pub fin_datos: u64,
}

/// A qué capa pertenece un tensor, si es que pertenece a alguna. Los tensores de capa se llaman
/// `blk.<n>.<lo que sea>`; el resto —`token_embd.weight`, `output.weight`, `output_norm.weight`—
/// no son de ninguna.
fn capa_de(nombre: &str) -> Option<usize> {
    let resto = nombre.strip_prefix("blk.")?;
    let (numero, _) = resto.split_once('.')?;
    numero.parse::<usize>().ok()
}

/// Lee la tabla de tensores que sigue a la cabecera y devuelve el peso de cada capa. Sigue sin
/// tocar un solo byte de los pesos: se detiene donde empieza el bloque de datos.
pub(super) fn leer_tensores_de<R: Read>(
    lector: &mut LectorAcotado<R>,
    cabecera: &Cabecera,
    ruta: &Path,
) -> Resultado<PesoDelModelo> {
    let mut pesos = PesoDelModelo::default();

    // `LIMITE_NUMERO_TENSORES` acota cuántos tensores hay, no qué índice de capa declara cada
    // uno: un solo tensor llamado «blk.4000000000.w» pasaría ese límite igual y provocaría un
    // `resize` de miles de millones de posiciones (varios GiB puestos a cero, y el asignador
    // aborta el proceso antes de que haya ocasión de capturar nada). El tope real de capas es
    // `numero_capas` (`block_count`), que la cabecera ya declara; si el GGUF no la trae, no hay
    // ningún dato de la propia cabecera con el que acotar, así que se reutiliza
    // `LIMITE_NUMERO_TENSORES`: no puede haber más capas que tensores.
    //
    // Pero `numero_capas` sale del propio fichero y nadie la acota: un GGUF puede mentir a la vez
    // en `block_count` (poniendo, por ejemplo, 5 000 000 000) y en el índice de un tensor (uno
    // menor que esa mentira pero igualmente descomunal), y ese segundo tensor pasaría la guarda de
    // más abajo sin problema. Por eso el tope declarado se recorta también contra
    // `LIMITE_NUMERO_TENSORES`: un modelo no puede tener más capas que tensores, así que el
    // recorte no rechaza nada legítimo. El recorte tiene además un segundo efecto: con
    // `block_count = u64::MAX` y un tensor `blk.18446744073709551614.w`, `capa + 1` daría
    // exactamente `usize::MAX` sin desbordar, y `Vec::resize` entraría en pánico por
    // desbordamiento de capacidad en vez de que la guarda lo rechace antes; con el recorte, el
    // tope nunca pasa de `LIMITE_NUMERO_TENSORES` y la guarda atrapa el índice mucho antes de
    // llegar al `resize`.
    let tope_capas = cabecera
        .metadatos
        .numero_capas
        .unwrap_or(LIMITE_NUMERO_TENSORES)
        .min(LIMITE_NUMERO_TENSORES);

    for _ in 0..cabecera.numero_tensores {
        let nombre = lector.leer_cadena(ruta)?;
        let numero_dimensiones = lector.leer_u32(ruta)?;
        if numero_dimensiones > LIMITE_DIMENSIONES {
            return Err(error_gguf(
                ruta,
                format!(
                    "el tensor «{nombre}» dice tener {numero_dimensiones} dimensiones, más del \
                     límite de {LIMITE_DIMENSIONES}"
                ),
            ));
        }
        let mut dimensiones = Vec::with_capacity(numero_dimensiones as usize);
        for _ in 0..numero_dimensiones {
            dimensiones.push(lector.leer_u64(ruta)?);
        }
        let codigo = lector.leer_u32(ruta)?;
        let tipo = tipo_tensor(codigo).ok_or_else(|| {
            error_gguf(
                ruta,
                format!("tipo de tensor desconocido en «{nombre}»: código {codigo}"),
            )
        })?;
        let desplazamiento = lector.leer_u64(ruta)?;

        let bytes = bytes_de_tensor(&dimensiones, tipo).ok_or_else(|| {
            error_gguf(
                ruta,
                format!("las dimensiones de «{nombre}» desbordan al multiplicarse"),
            )
        })?;

        let fin = desplazamiento.checked_add(bytes).ok_or_else(|| {
            error_gguf(
                ruta,
                format!("el desplazamiento de «{nombre}» más su tamaño desborda"),
            )
        })?;
        pesos.fin_datos = pesos.fin_datos.max(fin);

        match capa_de(&nombre) {
            Some(capa) => {
                // Acotar antes de reservar: sin esto, `capa + 1` de más abajo desbordaría con
                // `blk.18446744073709551615.w`, y con índices grandes pero no desbordantes el
                // `resize` que sigue reservaría gigabytes o abortaría el proceso directamente.
                if capa as u64 >= tope_capas {
                    return Err(error_gguf(
                        ruta,
                        match cabecera.metadatos.numero_capas {
                            Some(declaradas) => format!(
                                "el tensor «{nombre}» declara la capa {capa}, pero la cabecera \
                                 solo declara {declaradas} capas (`block_count`)"
                            ),
                            None => format!(
                                "el tensor «{nombre}» declara la capa {capa}, más del límite de \
                                 {LIMITE_NUMERO_TENSORES} capas que se usa cuando la cabecera no \
                                 trae `block_count`"
                            ),
                        },
                    ));
                }
                if capa >= pesos.por_capa.len() {
                    pesos.por_capa.resize(capa + 1, 0);
                }
                pesos.por_capa[capa] = pesos.por_capa[capa].saturating_add(bytes);
            }
            None => pesos.fuera_de_capa = pesos.fuera_de_capa.saturating_add(bytes),
        }
    }

    let alineacion = cabecera
        .metadatos
        .alineacion
        .filter(|a| *a > 0)
        .unwrap_or(ALINEACION_POR_DEFECTO);
    pesos.inicio_datos = lector.posicion().div_ceil(alineacion) * alineacion;

    Ok(pesos)
}

/// Constructores de GGUF sintéticos. Existe solo en compilación de pruebas —no es API del
/// proyecto— y está aquí y no dentro de `mod pruebas` porque las pruebas de `motor::encaje`
/// también necesitan un fichero que leer.
#[cfg(test)]
pub(crate) mod pruebas_apoyo {
    use super::{bytes_de_tensor, tipo_tensor};
    use std::io::Write;

    /// Tipos de valor GGUF que la utilidad de pruebas sabe escribir. Cubre los doce tipos de la
    /// especificación (0-12, sin el 9 salvo como wrapper de array) más el caso «cadena con
    /// longitud falsa», que se usa para simular ataques.
    pub(crate) enum ValorPrueba {
        U8(u8),
        I8(i8),
        U16(u16),
        I16(i16),
        U32(u32),
        I32(i32),
        F32(f32),
        Bool(bool),
        Cadena(String),
        U64(u64),
        I64(i64),
        F64(f64),
        ArrayU32(Vec<u32>),
    }

    pub(crate) fn codigo_tipo(valor: &ValorPrueba) -> u32 {
        match valor {
            ValorPrueba::U8(_) => 0,
            ValorPrueba::I8(_) => 1,
            ValorPrueba::U16(_) => 2,
            ValorPrueba::I16(_) => 3,
            ValorPrueba::U32(_) => 4,
            ValorPrueba::I32(_) => 5,
            ValorPrueba::F32(_) => 6,
            ValorPrueba::Bool(_) => 7,
            ValorPrueba::Cadena(_) => 8,
            ValorPrueba::ArrayU32(_) => 9,
            ValorPrueba::U64(_) => 10,
            ValorPrueba::I64(_) => 11,
            ValorPrueba::F64(_) => 12,
        }
    }

    pub(crate) fn escribir_cadena(buf: &mut Vec<u8>, texto: &str) {
        buf.extend_from_slice(&(texto.len() as u64).to_le_bytes());
        buf.extend_from_slice(texto.as_bytes());
    }

    pub(crate) fn escribir_valor(buf: &mut Vec<u8>, valor: &ValorPrueba) {
        match valor {
            ValorPrueba::U8(v) => buf.push(*v),
            ValorPrueba::I8(v) => buf.push(*v as u8),
            ValorPrueba::U16(v) => buf.extend_from_slice(&v.to_le_bytes()),
            ValorPrueba::I16(v) => buf.extend_from_slice(&v.to_le_bytes()),
            ValorPrueba::U32(v) => buf.extend_from_slice(&v.to_le_bytes()),
            ValorPrueba::I32(v) => buf.extend_from_slice(&v.to_le_bytes()),
            ValorPrueba::F32(v) => buf.extend_from_slice(&v.to_le_bytes()),
            ValorPrueba::Bool(v) => buf.push(if *v { 1 } else { 0 }),
            ValorPrueba::Cadena(v) => escribir_cadena(buf, v),
            ValorPrueba::U64(v) => buf.extend_from_slice(&v.to_le_bytes()),
            ValorPrueba::I64(v) => buf.extend_from_slice(&v.to_le_bytes()),
            ValorPrueba::F64(v) => buf.extend_from_slice(&v.to_le_bytes()),
            ValorPrueba::ArrayU32(elementos) => {
                buf.extend_from_slice(&4u32.to_le_bytes()); // tipo de elemento: U32
                buf.extend_from_slice(&(elementos.len() as u64).to_le_bytes());
                for elemento in elementos {
                    buf.extend_from_slice(&elemento.to_le_bytes());
                }
            }
        }
    }

    pub(crate) fn escribir_par(buf: &mut Vec<u8>, clave: &str, valor: &ValorPrueba) {
        escribir_cadena(buf, clave);
        buf.extend_from_slice(&codigo_tipo(valor).to_le_bytes());
        escribir_valor(buf, valor);
    }

    /// Compone una cabecera GGUF válida a partir de la lista de pares clave-valor dada.
    pub(crate) fn cabecera_valida(pares: &[(&str, ValorPrueba)]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"GGUF");
        buf.extend_from_slice(&3u32.to_le_bytes()); // versión 3, la actual
        buf.extend_from_slice(&0u64.to_le_bytes()); // cero tensores: no hace falta ninguno
        buf.extend_from_slice(&(pares.len() as u64).to_le_bytes());
        for (clave, valor) in pares {
            escribir_par(&mut buf, clave, valor);
        }
        buf
    }

    /// Un tensor tal y como lo escribe la utilidad de pruebas.
    pub(crate) struct TensorPrueba {
        pub(crate) nombre: String,
        pub(crate) dimensiones: Vec<u64>,
        pub(crate) tipo: u32,
    }

    pub(crate) fn escribir_tensor(buf: &mut Vec<u8>, tensor: &TensorPrueba, offset: u64) {
        escribir_cadena(buf, &tensor.nombre);
        buf.extend_from_slice(&(tensor.dimensiones.len() as u32).to_le_bytes());
        for dimension in &tensor.dimensiones {
            buf.extend_from_slice(&dimension.to_le_bytes());
        }
        buf.extend_from_slice(&tensor.tipo.to_le_bytes());
        buf.extend_from_slice(&offset.to_le_bytes());
    }

    /// Compone un GGUF completo y coherente: cabecera, tabla de tensores, relleno hasta la
    /// alineación y tantos bytes de datos como digan los tensores. Los desplazamientos se calculan
    /// en orden, cada uno alineado, igual que los escribe un cuantizador real.
    pub(crate) fn fichero_con_tensores(
        pares: &[(&str, ValorPrueba)],
        tensores: &[TensorPrueba],
        alineacion: u64,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"GGUF");
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&(tensores.len() as u64).to_le_bytes());
        buf.extend_from_slice(&(pares.len() as u64).to_le_bytes());
        for (clave, valor) in pares {
            escribir_par(&mut buf, clave, valor);
        }

        let mut offset = 0u64;
        let mut fin_datos = 0u64;
        for tensor in tensores {
            // Un tipo desconocido o unas dimensiones que desbordan cuentan como cero bytes de
            // datos, y no como un pánico: esta utilidad tiene que poder escribir justo los
            // ficheros imposibles con los que se comprueba que el lector los rechaza.
            let bytes = tipo_tensor(tensor.tipo)
                .and_then(|tipo| bytes_de_tensor(&tensor.dimensiones, tipo))
                .unwrap_or(0);
            escribir_tensor(&mut buf, tensor, offset);
            fin_datos = offset + bytes;
            offset = fin_datos.div_ceil(alineacion) * alineacion;
        }

        let relleno = (alineacion - (buf.len() as u64 % alineacion)) % alineacion;
        buf.extend(std::iter::repeat_n(0u8, relleno as usize));
        buf.extend(std::iter::repeat_n(0u8, fin_datos as usize));
        buf
    }

    pub(crate) fn escribir_temporal(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("modelo.gguf");
        let mut fichero = std::fs::File::create(&ruta).unwrap();
        fichero.write_all(bytes).unwrap();
        (dir, ruta)
    }

    pub(crate) fn pares_llama_completos() -> Vec<(&'static str, ValorPrueba)> {
        vec![
            ("general.architecture", ValorPrueba::Cadena("llama".into())),
            ("general.name", ValorPrueba::Cadena("Devstral-24B".into())),
            ("general.file_type", ValorPrueba::U32(2)),
            ("llama.block_count", ValorPrueba::U32(40)),
            ("llama.embedding_length", ValorPrueba::U32(5120)),
            ("llama.attention.head_count", ValorPrueba::U32(32)),
            ("llama.attention.head_count_kv", ValorPrueba::U32(8)),
            ("llama.context_length", ValorPrueba::U32(131072)),
        ]
    }

    /// Un GGUF mínimo pero coherente, pensado para las pruebas de `motor::encaje`: cuatro capas de
    /// 1 MiB, 256 KiB fuera de capa, y una cabecera de llama con 4 capas, 512 de embedding, 8
    /// cabezas de atención y 2 de KV (así `dim_cabeza` sale 64 redondo).
    // Nadie lo llama todavía: `motor::encaje` no existe hasta la Tarea 7. Construirlo aquí, y no
    // esperar a esa tarea, es lo que pide el encargo de la Tarea 2.
    #[allow(dead_code)]
    pub(crate) fn gguf_de_juguete() -> (tempfile::TempDir, std::path::PathBuf) {
        let un_mib = 1024 * 1024 / 4; // en elementos F32, que ocupan cuatro bytes cada uno
        let tensores: Vec<TensorPrueba> = (0..4)
            .map(|capa| TensorPrueba {
                nombre: format!("blk.{capa}.attn_q.weight"),
                dimensiones: vec![un_mib],
                tipo: 0,
            })
            .chain(std::iter::once(TensorPrueba {
                nombre: "token_embd.weight".to_string(),
                dimensiones: vec![256 * 1024 / 4],
                tipo: 0,
            }))
            .collect();

        let pares = vec![
            ("general.architecture", ValorPrueba::Cadena("llama".into())),
            ("llama.block_count", ValorPrueba::U32(4)),
            ("llama.embedding_length", ValorPrueba::U32(512)),
            ("llama.attention.head_count", ValorPrueba::U32(8)),
            ("llama.attention.head_count_kv", ValorPrueba::U32(2)),
            ("llama.context_length", ValorPrueba::U32(32_768)),
        ];

        escribir_temporal(&fichero_con_tensores(&pares, &tensores, 32))
    }
}

#[cfg(test)]
mod pruebas {
    use super::super::leer_modelo;
    use super::pruebas_apoyo::*;
    use crate::error::Error;

    fn tensor(nombre: &str, dimensiones: &[u64], tipo: u32) -> TensorPrueba {
        TensorPrueba {
            nombre: nombre.to_string(),
            dimensiones: dimensiones.to_vec(),
            tipo,
        }
    }

    #[test]
    fn reparte_los_tensores_entre_sus_capas_y_lo_que_no_es_de_ninguna() {
        let bytes = fichero_con_tensores(
            &pares_llama_completos(),
            &[
                tensor("token_embd.weight", &[256, 100], 0), // 102 400 bytes, fuera de capa
                tensor("blk.0.attn_q.weight", &[256, 2], 0), //   2 048 bytes, capa 0
                tensor("blk.0.ffn_up.weight", &[256, 3], 0), //   3 072 bytes, capa 0
                tensor("blk.1.attn_q.weight", &[256, 4], 0), //   4 096 bytes, capa 1
                tensor("output.weight", &[256, 5], 0),       //   5 120 bytes, fuera de capa
            ],
            32,
        );
        let (_dir, ruta) = escribir_temporal(&bytes);

        let informe = leer_modelo(&ruta).unwrap();

        assert_eq!(informe.pesos.por_capa, vec![5_120, 4_096]);
        assert_eq!(informe.pesos.fuera_de_capa, 102_400 + 5_120);
        assert_eq!(
            informe.metadatos.numero_capas,
            Some(40),
            "los metadatos de la cabecera se siguen leyendo igual"
        );
    }

    #[test]
    fn la_suma_de_los_tensores_cuadra_con_el_tamano_del_fichero() {
        let bytes = fichero_con_tensores(
            &pares_llama_completos(),
            &[
                tensor("blk.0.attn_q.weight", &[256, 7], 12), // Q4_K
                tensor("blk.1.ffn_up.weight", &[256, 9], 14), // Q6_K
                tensor("output.weight", &[4096], 0),          // F32
            ],
            32,
        );
        let total = bytes.len() as u64;
        let (_dir, ruta) = escribir_temporal(&bytes);

        let informe = leer_modelo(&ruta).unwrap();

        assert_eq!(
            informe.pesos.inicio_datos + informe.pesos.fin_datos,
            total,
            "si esta igualdad falla, algún tamaño de la tabla de tipos está mal"
        );
    }

    #[test]
    fn un_contador_de_tensores_absurdo_se_rechaza_antes_de_reservar_nada() {
        let mut bytes = fichero_con_tensores(&pares_llama_completos(), &[], 32);
        bytes[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        let (_dir, ruta) = escribir_temporal(&bytes);

        let fallo = leer_modelo(&ruta).unwrap_err();

        assert!(
            fallo.to_string().contains("tensores"),
            "el error debe decir qué contador es el absurdo: {fallo}"
        );
    }

    #[test]
    fn unas_dimensiones_que_desbordan_se_rechazan_sin_panico() {
        let bytes = fichero_con_tensores(
            &pares_llama_completos(),
            &[tensor("blk.0.a.weight", &[u64::MAX, 2], 0)],
            32,
        );
        let (_dir, ruta) = escribir_temporal(&bytes);

        assert!(
            leer_modelo(&ruta).is_err(),
            "un tensor imposible es un fichero inválido, no un pánico"
        );
    }

    #[test]
    fn un_tipo_de_tensor_desconocido_se_rechaza() {
        // El 200 no existe en ggml. Se escribe directamente en la tabla, sin parchear bytes a
        // mano: buscar un patrón dentro del fichero sería frágil, porque está lleno de ceros.
        let bytes = fichero_con_tensores(
            &pares_llama_completos(),
            &[tensor("blk.0.a.weight", &[256], 200)],
            32,
        );
        let (_dir, ruta) = escribir_temporal(&bytes);

        let fallo = leer_modelo(&ruta).unwrap_err();

        assert!(
            fallo.to_string().contains("200"),
            "el error debe nombrar el código desconocido: {fallo}"
        );
    }

    #[test]
    fn un_tipo_retirado_de_ggml_tampoco_se_acepta() {
        // El 31 fue Q4_0_4_4 y está retirado de los GGUF. Un fichero que lo traiga es antiguo o
        // falso.
        let bytes = fichero_con_tensores(
            &pares_llama_completos(),
            &[tensor("blk.0.a.weight", &[256], 31)],
            32,
        );
        let (_dir, ruta) = escribir_temporal(&bytes);

        assert!(leer_modelo(&ruta).is_err());
    }

    #[test]
    fn una_tabla_truncada_a_media_entrada_se_rechaza() {
        let bytes = fichero_con_tensores(
            &pares_llama_completos(),
            &[tensor("blk.0.a.weight", &[256], 0)],
            32,
        );
        let (_dir, ruta) = escribir_temporal(&bytes[..bytes.len() / 2]);

        assert!(leer_modelo(&ruta).is_err());
    }

    #[test]
    fn un_indice_de_capa_descomunal_se_rechaza_sin_reservar_nada() {
        // `blk.4000000000.w`: sin la acotación, esto intentaría un `Vec::resize` de cuatro mil
        // millones de posiciones (32 GiB) y el asignador abortaría el proceso entero, que no es
        // un panic capturable por `unwrap_err`. `pares_llama_completos()` declara
        // `llama.block_count = 40`, así que el índice se rechaza contra ese tope declarado.
        let bytes = fichero_con_tensores(
            &pares_llama_completos(),
            &[tensor("blk.4000000000.w", &[30], 0)],
            32,
        );
        let (_dir, ruta) = escribir_temporal(&bytes);

        let inicio = std::time::Instant::now();
        let fallo = leer_modelo(&ruta).unwrap_err();
        let transcurrido = inicio.elapsed();

        // `contains("40")` no basta: "40" es un prefijo literal de "4000000000", el propio
        // nombre del tensor, así que esa aserción se cumpliría por el eco del nombre incluso si
        // el mensaje no dijera nada del tope de capas. Se exige en cambio la frase completa que
        // solo puede salir de esta guarda: el índice visto con su contexto, y el tope aplicado
        // con el suyo. Ninguna de las dos frases es una subcadena de «blk.4000000000.w».
        let mensaje = fallo.to_string();
        assert!(
            mensaje.contains("declara la capa 4000000000"),
            "el error debe nombrar el índice visto: {fallo}"
        );
        assert!(
            mensaje.contains("solo declara 40 capas"),
            "el error debe nombrar el tope de capas aplicado: {fallo}"
        );
        assert!(
            transcurrido < std::time::Duration::from_secs(2),
            "rechazar un índice descomunal no puede tardar: tardó {transcurrido:?}"
        );
    }

    #[test]
    fn un_indice_que_excede_el_block_count_declarado_se_rechaza() {
        // Con `llama.block_count = 40` (el de `pares_llama_completos()`), las capas válidas son
        // 0..39. `blk.40` es el primer índice fuera de rango: pinza justo el `>=` que separa
        // «cabe» de «no cabe».
        let bytes = fichero_con_tensores(
            &pares_llama_completos(),
            &[tensor("blk.40.attn_q.weight", &[256], 0)],
            32,
        );
        let (_dir, ruta) = escribir_temporal(&bytes);

        let fallo = leer_modelo(&ruta).unwrap_err();

        // Igual que arriba: "40" también es una subcadena literal de "blk.40.attn_q.weight", así
        // que un `contains("40")` desnudo pasaría aunque el mensaje no mencionara el tope. Se
        // exige la frase completa del índice y la del tope, ninguna de las cuales aparece en el
        // nombre del tensor.
        let mensaje = fallo.to_string();
        assert!(
            mensaje.contains("declara la capa 40"),
            "el error debe nombrar el índice visto: {fallo}"
        );
        assert!(
            mensaje.contains("solo declara 40 capas"),
            "el error debe nombrar el tope de capas aplicado: {fallo}"
        );
    }

    #[test]
    fn un_gguf_que_miente_en_block_count_y_en_el_indice_de_capa_se_rechaza_sin_reservar_nada() {
        // El fichero miente dos veces a la vez: `llama.block_count = 5 000 000 000` (una cabecera
        // descomunal) y un único tensor que declara la capa 4 999 999 999 — menor que esa
        // mentira, así que pasaría sin problema una guarda que solo comparase contra
        // `block_count`. Pero un modelo no puede tener más capas que tensores, y este fichero
        // solo trae un tensor: el tope efectivo tiene que recortarse contra
        // `LIMITE_NUMERO_TENSORES`, no solo contra lo que diga la cabecera. Sin ese recorte, esto
        // volvería a intentar un `Vec::resize` de miles de millones de posiciones.
        let pares = vec![
            ("general.architecture", ValorPrueba::Cadena("llama".into())),
            ("llama.block_count", ValorPrueba::U64(5_000_000_000)),
        ];
        let bytes = fichero_con_tensores(&pares, &[tensor("blk.4999999999.w", &[30], 0)], 32);
        let (_dir, ruta) = escribir_temporal(&bytes);

        let inicio = std::time::Instant::now();
        let fallo = leer_modelo(&ruta).unwrap_err();
        let transcurrido = inicio.elapsed();

        assert!(
            matches!(fallo, Error::GgufInvalido { .. }),
            "un fichero que miente dos veces sigue siendo un GGUF inválido, no otra cosa: {fallo}"
        );
        let mensaje = fallo.to_string();
        assert!(
            mensaje.contains("4999999999"),
            "el error debe nombrar el índice de capa visto: {fallo}"
        );
        assert!(
            transcurrido < std::time::Duration::from_secs(2),
            "rechazar el índice no puede tardar ni reservar memoria: tardó {transcurrido:?}"
        );
    }

    #[test]
    fn un_numero_de_dimensiones_absurdo_se_rechaza() {
        let bytes = fichero_con_tensores(
            &pares_llama_completos(),
            &[tensor("blk.0.a.weight", &[2, 2, 2, 2, 2, 2, 2, 2, 2, 2], 0)],
            32,
        );
        let (_dir, ruta) = escribir_temporal(&bytes);

        let fallo = leer_modelo(&ruta).unwrap_err();

        assert!(
            fallo.to_string().contains("dimensiones"),
            "el error debe decir que sobran dimensiones: {fallo}"
        );
    }
}
