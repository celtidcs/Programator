//! Decide cuánto del modelo cabe en la GPU.
//!
//! La política de decisión —`kv_por_capa`, `decidir` y `capas_para_llama`— sigue siendo aritmética
//! pura: no toca nada del mundo, así que se prueba entera con números inventados, sin una GPU
//! delante. `resolver` y `calcular`, en cambio, orquestan: juntan la configuración, la cabecera del
//! GGUF y la medida de la GPU para poder aplicar esa política a un modelo y una tarjeta concretos, y
//! para eso sí abren el fichero de pesos —solo leen su cabecera y su tabla de tensores, nunca los
//! pesos en sí— a través de `gguf::leer_modelo`. Lo que este módulo **no** hace en ningún punto:
//! preguntar al sistema gráfico (la GPU llega por parámetro, nunca por `hardware::describir_gpu()`)
//! ni lanzar ningún proceso.

use super::gguf;
use super::gguf::tensores::PesoDelModelo;
use super::hardware::Gpu;
use super::tipos_tensor::{bytes_de_tensor, tipo_tensor, TipoTensor};
use crate::config::{CapasGpu, MotorConfig};
use std::path::Path;

/// Traduce el nombre que se escribe en `[motor] cache_kv` al tipo de ggml correspondiente.
///
/// Solo están los tipos que `llama-server` admite en `--cache-type-k` y `--cache-type-v`: el caché
/// no acepta las cuantizaciones de bloque grande que sí valen para los pesos.
///
/// Los códigos numéricos proceden de la especificación técnica de GGML (`enum ggml_type` en `ggml.h`):
const GGML_TIPO_F32: u32 = 0;
const GGML_TIPO_F16: u32 = 1;
const GGML_TIPO_Q4_0: u32 = 2;
const GGML_TIPO_Q4_1: u32 = 3;
const GGML_TIPO_Q5_0: u32 = 6;
const GGML_TIPO_Q5_1: u32 = 7;
const GGML_TIPO_Q8_0: u32 = 8;
/// Cuantización no lineal de 4 bits admitida para caché KV en GGML.
const GGML_TIPO_IQ4_NL: u32 = 20;
/// Formato de punto flotante Brain de 16 bits admitido para caché KV en GGML.
const GGML_TIPO_BF16: u32 = 30;

pub fn tipo_de_cache(nombre: &str) -> Option<TipoTensor> {
    let codigo = match nombre.to_ascii_lowercase().as_str() {
        "f32" => GGML_TIPO_F32,
        "f16" => GGML_TIPO_F16,
        "q4_0" => GGML_TIPO_Q4_0,
        "q4_1" => GGML_TIPO_Q4_1,
        "q5_0" => GGML_TIPO_Q5_0,
        "q5_1" => GGML_TIPO_Q5_1,
        "q8_0" => GGML_TIPO_Q8_0,
        "iq4_nl" => GGML_TIPO_IQ4_NL,
        "bf16" => GGML_TIPO_BF16,
        _ => return None,
    };
    tipo_tensor(codigo)
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
pub fn kv_por_capa(
    contexto: u64,
    cabezas_kv: u64,
    dim_cabeza: u64,
    cache: TipoTensor,
) -> Option<u64> {
    bytes_de_tensor(&[2, contexto, cabezas_kv, dim_cabeza], cache)
}

/// Cuánto del modelo cabe en la GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Encaje {
    /// Cabe entero, capa de salida incluida. `total` son las capas que tiene el modelo.
    Completo { total: u32 },
    /// `en_gpu` capas de las `total` del modelo; el resto irá por CPU. `0 < en_gpu <= total`:
    /// cuando `en_gpu == total`, las capas caben todas pero `fuera_de_capa` —embeddings, salida y
    /// normalización final— no cupo con ellas, así que la salida se queda en CPU y por eso este
    /// desenlace no es `Completo`.
    Parcial { en_gpu: u32, total: u32 },
    /// No cabe ni la primera capa.
    NadaEnGpu,
    /// Falta un dato para calcular. Quien llame usa el valor configurado.
    NoCalculable { motivo: String },
}

/// Decide cuántas capas caben en `libre` bytes de VRAM, reservando `margen`.
///
/// El coste de `n` capas es la suma de las `n` **últimas** más `n × kv_por_capa`, porque son las
/// últimas las que `llama.cpp` sube a la GPU. Como el coste crece con `n`, basta con buscar el
/// mayor que quepa; con cuarenta capas la búsqueda lineal sobra y se lee mejor que una binaria.
pub fn decidir(pesos: &PesoDelModelo, kv_por_capa: u64, libre: u64, margen: u64) -> Encaje {
    let total = pesos.por_capa.len();
    if total == 0 {
        return Encaje::NoCalculable {
            motivo: "el GGUF no declara ninguna capa".to_string(),
        };
    }
    let Ok(total_u32) = u32::try_from(total) else {
        return Encaje::NoCalculable {
            motivo: format!("un modelo de {total} capas no es creíble"),
        };
    };

    let disponible = libre.saturating_sub(margen);

    // El modelo entero incluye lo que no es de ninguna capa: embeddings, salida y normalización.
    // `token_embd.weight` se cuenta aquí aunque llama.cpp suela dejarlo en RAM: sobreestimar cuesta
    // como mucho una capa menos de las posibles, y subestimar cuesta un fallo de reserva.
    let coste_entero = coste_de_las_ultimas(pesos, total, kv_por_capa)
        .and_then(|c| c.checked_add(pesos.fuera_de_capa));
    if coste_entero.is_some_and(|coste| coste <= disponible) {
        return Encaje::Completo { total: total_u32 };
    }

    // Todas las capas caben, pero no junto con `fuera_de_capa`: es el desenlace `Parcial { en_gpu:
    // total, .. }`, el único que el bucle de más abajo no puede alcanzar porque `(1..total)`
    // excluye `n == total` por construcción. Se prueba aquí, entre descartar el modelo entero y
    // bajar a `total - 1`, porque es exactamente ese paso el que faltaba: en la tarjeta del
    // Director es el desenlace más probable —cuarenta capas que caben pero no junto a `token_embd`
    // y `output`— y sin este caso se contestaría 39 en vez de 40.
    if coste_de_las_ultimas(pesos, total, kv_por_capa).is_some_and(|c| c <= disponible) {
        return Encaje::Parcial {
            en_gpu: total_u32,
            total: total_u32,
        };
    }

    for n in (1..total).rev() {
        if coste_de_las_ultimas(pesos, n, kv_por_capa).is_some_and(|c| c <= disponible) {
            // `n < total` y `total` cabe en u32, así que esta conversión no puede fallar.
            return Encaje::Parcial {
                en_gpu: n as u32,
                total: total_u32,
            };
        }
    }

    Encaje::NadaEnGpu
}

/// Bytes que cuestan las `n` últimas capas, pesos y caché incluidos. `None` si la suma desborda.
fn coste_de_las_ultimas(pesos: &PesoDelModelo, n: usize, kv_por_capa: u64) -> Option<u64> {
    let desde = pesos.por_capa.len().checked_sub(n)?;
    let mut total: u64 = 0;
    for peso in &pesos.por_capa[desde..] {
        total = total.checked_add(*peso)?.checked_add(kv_por_capa)?;
    }
    Some(total)
}

/// Traduce la decisión al número que espera `--n-gpu-layers`.
///
/// El `+ 1` de `Completo` no es un redondeo: es lo que hace que `llama.cpp` suba también la capa de
/// salida, que solo se descarga cuando el número **supera** al de capas del modelo.
pub fn capas_para_llama(encaje: &Encaje, configuradas: u32) -> u32 {
    match encaje {
        Encaje::Completo { total } => total.saturating_add(1),
        Encaje::Parcial { en_gpu, .. } => *en_gpu,
        Encaje::NadaEnGpu => 0,
        Encaje::NoCalculable { .. } => configuradas,
    }
}

/// Lo que Programator pedía antes de que existiera el encaje. Es el valor al que se vuelve cuando
/// `"auto"` no puede calcular nada: degradar a lo de siempre es mejor que negarse a arrancar.
const CAPAS_SI_NO_SE_SABE: u32 = 99;

/// El número de capas que va a `--n-gpu-layers` y la explicación de por qué.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolucion {
    pub capas: u32,
    /// Qué contar al Director. Nunca vacío: si no hay nada que avisar, dice que cabe entero.
    pub aviso: String,
}

/// Decide el número de capas para este modelo, esta GPU y esta configuración.
///
/// Recibe la GPU en vez de preguntarla para poder probarse con tarjetas inventadas; `main` le pasa
/// la de verdad. El contexto configurado **no se toca nunca**: es una garantía funcional del arnés,
/// y recortarlo trunca encargos sin que nadie se entere, mientras que unas capas en CPU solo cuestan
/// lentitud, que se ve.
pub fn resolver(config: &MotorConfig, modelo: &Path, gpu: Option<&Gpu>) -> Resolucion {
    let configuradas = match config.capas_gpu {
        CapasGpu::Fijas(n) => n,
        CapasGpu::Auto => CAPAS_SI_NO_SE_SABE,
    };

    let (encaje, coste) = calcular(config, modelo, gpu);
    let aviso = redactar_aviso(&encaje, coste, gpu);

    match config.capas_gpu {
        // Un número escrito en el TOML se transcribe tal cual: como mucho se avisa.
        CapasGpu::Fijas(n) => Resolucion { capas: n, aviso },
        CapasGpu::Auto => Resolucion {
            capas: capas_para_llama(&encaje, configuradas),
            aviso,
        },
    }
}

/// Junta las piezas: pesa el modelo, calcula el caché y decide. Cada dato que falte produce un
/// `NoCalculable` con **ese** motivo escrito, no uno genérico: quien lea el aviso tiene que saber
/// qué arreglar. Devuelve, junto con la decisión, el coste en bytes que le costó a `decidir`
/// tomarla —`None` cuando no hubo pesos con los que calcular ninguno—, porque `redactar_aviso`
/// necesita esa cifra y no tiene acceso a `informe.pesos` ni a `kv`.
fn calcular(config: &MotorConfig, modelo: &Path, gpu: Option<&Gpu>) -> (Encaje, Option<u64>) {
    let Some(gpu) = gpu else {
        return (
            Encaje::NoCalculable {
                motivo: "no se ha podido medir ninguna GPU dedicada".to_string(),
            },
            None,
        );
    };

    let informe = match gguf::leer_modelo(modelo) {
        Ok(informe) => informe,
        Err(fallo) => {
            return (
                Encaje::NoCalculable {
                    motivo: format!("el modelo no se ha podido pesar: {fallo}"),
                },
                None,
            )
        }
    };

    let Some(cache) = tipo_de_cache(&config.cache_kv) else {
        return (
            Encaje::NoCalculable {
                motivo: format!(
                    "«{}» no es un tipo de caché que llama-server admita",
                    config.cache_kv
                ),
            },
            None,
        );
    };

    // Separadas en tres comprobaciones y no en una desestructuración conjunta: un único texto
    // para las tres claves no le dice al Director cuál de ellas falta, y esa es la única pista
    // que tiene para saber qué mirar en el GGUF.
    let metadatos = &informe.metadatos;
    let Some(embedding) = metadatos.longitud_embedding else {
        return (
            Encaje::NoCalculable {
                motivo: "a la cabecera del GGUF le falta «embedding_length»".to_string(),
            },
            None,
        );
    };
    let Some(cabezas) = metadatos.cabezas_atencion else {
        return (
            Encaje::NoCalculable {
                motivo: "a la cabecera del GGUF le falta «attention.head_count»".to_string(),
            },
            None,
        );
    };
    let Some(cabezas_kv) = metadatos.cabezas_atencion_kv else {
        return (
            Encaje::NoCalculable {
                motivo: "a la cabecera del GGUF le falta «attention.head_count_kv»".to_string(),
            },
            None,
        );
    };
    if cabezas == 0 {
        return (
            Encaje::NoCalculable {
                motivo: "la cabecera del GGUF declara cero cabezas de atención".to_string(),
            },
            None,
        );
    }

    let Some(kv_de_una_secuencia) = kv_por_capa(
        config.contexto as u64,
        cabezas_kv,
        embedding / cabezas,
        cache,
    ) else {
        return (
            Encaje::NoCalculable {
                motivo: "el caché de ese contexto no cabe ni en un u64".to_string(),
            },
            None,
        );
    };

    // La caché se reserva **una vez por secuencia**, no una por modelo: `llama-server` da a cada
    // slot de `--parallel` su propio espacio de contexto. Contarla una sola vez era decidir el
    // reparto sobre un presupuesto que no existía, y es lo que hacía que con los cuatro slots por
    // defecto del motor se ofrecieran 35 capas de 40 donde en realidad no cabían ni esas.
    let Some(kv) = kv_de_una_secuencia.checked_mul(config.paralelo.max(1) as u64) else {
        return (
            Encaje::NoCalculable {
                motivo: format!(
                    "el caché de {} conversaciones en paralelo no cabe ni en un u64",
                    config.paralelo
                ),
            },
            None,
        );
    };

    let margen = config.margen_vram_mib.saturating_mul(1024 * 1024);
    let encaje = decidir(&informe.pesos, kv, gpu.vram_libre, margen);
    let coste = coste_de_la_decision(&encaje, &informe.pesos, kv);
    (encaje, coste)
}

/// El coste en bytes de la decisión ya tomada: lo mismo que comparó `decidir` contra `disponible`
/// para llegar a ese resultado, y no un cálculo nuevo. Separado de `decidir` porque esa función es
/// la política pura del §7 y no le hace falta este número para decidir nada; solo lo necesita el
/// aviso para poder decir cuánto pedía el intento que ganó.
fn coste_de_la_decision(encaje: &Encaje, pesos: &PesoDelModelo, kv_por_capa: u64) -> Option<u64> {
    match encaje {
        Encaje::Completo { total } => coste_de_las_ultimas(pesos, *total as usize, kv_por_capa)
            .and_then(|c| c.checked_add(pesos.fuera_de_capa)),
        Encaje::Parcial { en_gpu, .. } => {
            coste_de_las_ultimas(pesos, *en_gpu as usize, kv_por_capa)
        }
        // Ninguna capa cupo, ni siquiera la última —la más barata de intentar, porque `decidir`
        // prueba de más capas a menos—: es la cifra de «pide tanto y no había ni eso» del §8.
        Encaje::NadaEnGpu => coste_de_las_ultimas(pesos, 1, kv_por_capa),
        Encaje::NoCalculable { .. } => None,
    }
}

/// Qué contarle al Director. Los cuatro textos del §8 de la especificación, con las cifras en GiB
/// para que se puedan comparar de un vistazo con lo que dice el administrador de tareas. `coste`
/// trae lo que pedía la decisión tomada —`None` en `NoCalculable`, que no llegó a calcular nada—:
/// es el número con el que el Director decidiría si tocar `margen_vram_mib`, y sin él el aviso solo
/// decía cuánto había libre, nunca cuánto se pidió.
fn redactar_aviso(encaje: &Encaje, coste: Option<u64>, gpu: Option<&Gpu>) -> String {
    let donde = match gpu {
        Some(g) => format!("{} ({} libres)", g.nombre, en_gib(g.vram_libre)),
        None => "sin GPU medible".to_string(),
    };
    // Solo los tres desenlaces que sí calcularon algo tienen un coste que contar; `NoCalculable`
    // no pasa por aquí porque su rama de `match` no lo consulta.
    let pide = coste.map(en_gib).unwrap_or_default();
    match encaje {
        Encaje::Completo { total } => format!(
            "el modelo entero cabe en {donde}: sus {total} capas piden {pide} y van a la GPU"
        ),
        Encaje::Parcial { en_gpu, total } => {
            let fuera = total - en_gpu;
            // Concordancia: con una sola capa fuera, «las 1 restantes irán» es exactamente lo que
            // el Director vio impreso el 22/09/2026 al bajar el contexto a 16k. Un informe que se
            // lee en cada arranque no puede estar mal escrito.
            let cola = if fuera == 1 {
                "la restante irá por CPU".to_string()
            } else {
                format!("las {fuera} restantes irán por CPU")
            };
            format!(
                "en {donde} caben {en_gpu} de {total} capas (piden {pide}); {cola} y el ciclo \
                 será más lento"
            )
        }
        Encaje::NadaEnGpu => format!(
            "ni la primera capa cabe en {donde}: pide {pide} y el modelo irá entero por CPU"
        ),
        Encaje::NoCalculable { motivo } => format!(
            "no se ha podido calcular el encaje ({motivo}), así que se piden \
             {CAPAS_SI_NO_SE_SABE} capas como hasta ahora"
        ),
    }
}

/// Bytes a GiB con un decimal, que es la unidad en la que se habla de VRAM.
fn en_gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
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

    /// Cuatro capas de 100 bytes cada una y 50 fuera de capa: números pequeños para que las cuentas se
    /// puedan hacer de cabeza al leer la prueba.
    fn pesos_de_juguete() -> PesoDelModelo {
        PesoDelModelo {
            por_capa: vec![100, 100, 100, 100],
            fuera_de_capa: 50,
            inicio_datos: 0,
            fin_datos: 450,
        }
    }

    #[test]
    fn cuando_sobra_vram_cabe_el_modelo_entero() {
        // 4 capas × (100 + 10) + 50 = 490.
        let encaje = decidir(&pesos_de_juguete(), 10, 10_000, 0);
        assert_eq!(encaje, Encaje::Completo { total: 4 });
        assert_eq!(
            capas_para_llama(&encaje, 99),
            5,
            "una más que las capas, que es lo que hace a llama.cpp subir también la salida"
        );
    }

    #[test]
    fn cuando_falta_poco_se_quedan_capas_en_cpu() {
        // 490 no cabe en 400; 3 capas son 3 × 110 = 330, que sí.
        let encaje = decidir(&pesos_de_juguete(), 10, 400, 0);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 3,
                total: 4
            }
        );
        assert_eq!(capas_para_llama(&encaje, 99), 3);
    }

    #[test]
    fn cuando_no_cabe_ni_una_capa_no_se_descarga_ninguna() {
        let encaje = decidir(&pesos_de_juguete(), 10, 50, 0);
        assert_eq!(encaje, Encaje::NadaEnGpu);
        assert_eq!(capas_para_llama(&encaje, 99), 0);
    }

    #[test]
    fn el_margen_se_descuenta_de_lo_disponible() {
        // Con 490 de VRAM cabría entero; con 200 de margen solo caben dos capas.
        let encaje = decidir(&pesos_de_juguete(), 10, 490, 200);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 2,
                total: 4
            }
        );
    }

    #[test]
    fn un_margen_mayor_que_la_vram_no_desborda() {
        let encaje = decidir(&pesos_de_juguete(), 10, 100, 100_000);
        assert_eq!(encaje, Encaje::NadaEnGpu, "saturating, no envoltura");
    }

    // Los dos casos de frontera que pedía el §10 de la especificación y que faltaban: «cabe
    // justo» y «cabe una capa menos que justo». Son los que pinzan la comparación `<=` de
    // `decidir`, donde viviría un `<` puesto por descuido: con un margen de sobra o de menos, un
    // `<` en vez de un `<=` no se nota, porque casi ningún número real cae justo en la frontera.

    #[test]
    fn cuando_el_coste_exacto_iguala_lo_disponible_cabe_justo() {
        // El modelo entero pide 4 × (100 + 10) + 50 = 490. Con exactamente 490 libres y margen 0,
        // «cabe» tiene que seguir siendo cierto: es `coste <= disponible`, no `coste < disponible`.
        let encaje = decidir(&pesos_de_juguete(), 10, 490, 0);
        assert_eq!(
            encaje,
            Encaje::Completo { total: 4 },
            "490 libres tienen que bastar para un coste de 490 exactos"
        );
    }

    #[test]
    fn cuando_solo_las_tres_ultimas_igualan_lo_disponible_cabe_una_capa_menos_que_justo() {
        // El modelo entero (490) no cabe en 330. Las tres últimas capas piden exactamente
        // 3 × (100 + 10) = 330: la misma frontera de antes, pero un escalón más abajo, en el
        // bucle que prueba `total - 1` capas.
        let encaje = decidir(&pesos_de_juguete(), 10, 330, 0);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 3,
                total: 4
            },
            "330 libres tienen que bastar para las tres últimas capas, que piden 330 exactos"
        );
    }

    #[test]
    fn cuando_todas_las_capas_caben_pero_no_lo_de_fuera_es_parcial_con_en_gpu_igual_a_total() {
        // Las cuatro capas piden 4 × (100 + 10) = 440 sin contar `fuera_de_capa` (50): con 440
        // libres caben todas, pero el modelo entero —que sí suma esos 50— no (440 + 50 = 490 >
        // 440). El bucle `(1..total).rev()` excluye `n == total` por construcción, así que sin
        // este caso el resultado sería `Parcial { en_gpu: 3, .. }`, una capa menos de las que en
        // realidad caben: es la tarjeta del Director, donde las 40 capas de Devstral caben pero
        // sumarles `token_embd` y `output` ya no.
        let encaje = decidir(&pesos_de_juguete(), 10, 440, 0);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 4,
                total: 4
            },
            "las cuatro capas caben; solo lo de fuera de capa no. En_gpu tiene que llegar a 4, \
             no quedarse en 3"
        );
        assert_eq!(
            capas_para_llama(&encaje, 99),
            4,
            "Parcial {{ en_gpu: 4, total: 4 }} tiene que traducirse a 4, no a 5: a diferencia de \
             Completo, aquí no hay «+1» porque la salida no viaja con las capas"
        );
    }

    #[test]
    fn un_modelo_sin_capas_no_es_calculable() {
        let vacio = PesoDelModelo::default();
        let encaje = decidir(&vacio, 10, 10_000, 0);
        assert!(matches!(encaje, Encaje::NoCalculable { .. }));
        assert_eq!(
            capas_para_llama(&encaje, 99),
            99,
            "sin cálculo se respeta lo configurado, que es lo que Programator hacía antes"
        );
    }

    #[test]
    fn se_descargan_las_ultimas_capas_que_son_las_que_llama_pone_en_gpu() {
        // Capas de peso desigual: las últimas son las caras. Si el cálculo sumara las primeras,
        // creería que caben tres.
        let pesos = PesoDelModelo {
            por_capa: vec![10, 10, 1_000, 1_000],
            fuera_de_capa: 0,
            inicio_datos: 0,
            fin_datos: 2_020,
        };
        let encaje = decidir(&pesos, 0, 1_500, 0);
        assert_eq!(
            encaje,
            Encaje::Parcial {
                en_gpu: 1,
                total: 4
            }
        );
    }

    #[test]
    fn el_caso_real_de_devstral_en_una_4070_ti_super() {
        // 40 capas de 350 MiB, 1,2 GiB fuera de capa, 85 MiB de caché por capa y 14,93 GiB de
        // presupuesto menos 1 GiB de margen. Comprueba que la decisión es parcial y razonable, no un
        // número exacto: los pesos reales los dará el GGUF cuando el Director lo descargue.
        let pesos = PesoDelModelo {
            por_capa: vec![350 * 1024 * 1024; 40],
            fuera_de_capa: 1_200 * 1024 * 1024,
            inicio_datos: 0,
            fin_datos: 0,
        };
        let encaje = decidir(&pesos, 85 * 1024 * 1024, 16_035_872_768, 1024 * 1024 * 1024);
        match encaje {
            Encaje::Parcial { en_gpu, total } => {
                assert_eq!(total, 40);
                assert!(
                    (30..40).contains(&en_gpu),
                    "con estos números deben caber casi todas, pero no todas: {en_gpu}"
                );
            }
            otro => panic!("se esperaba un encaje parcial, y salió {otro:?}"),
        }
    }
}

#[cfg(test)]
mod pruebas_resolver {
    use super::*;
    use crate::motor::gguf::tensores::pruebas_apoyo::{
        escribir_temporal, fichero_con_tensores, gguf_de_juguete, ValorPrueba,
    };

    /// El GGUF de juguete tiene cuatro capas de 1 MiB, 256 KiB fuera de capa, 512 de embedding,
    /// 8 cabezas y 2 de KV. Con `contexto` bajo, el caché es despreciable y manda el peso.
    fn config(capas_gpu: CapasGpu, contexto: u32) -> MotorConfig {
        MotorConfig {
            binario: None,
            modelo: None,
            puerto: 8080,
            contexto,
            capas_gpu,
            cache_kv: "q8_0".to_string(),
            paralelo: 1,
            margen_vram_mib: 0,
            umbral_aviso_contexto: 70,
            bytes_por_token: 4,
            tiempo_lectura_segundos: crate::config::tiempo_lectura_motor_segundos_por_defecto(),
            tiempo_escritura_segundos: crate::config::tiempo_escritura_motor_segundos_por_defecto(),
        }
    }

    fn gpu(vram: u64) -> Gpu {
        Gpu {
            nombre: "inventada".to_string(),
            vram_total: vram,
            vram_libre: vram,
        }
    }

    #[test]
    fn con_una_sola_capa_fuera_el_aviso_esta_bien_escrito() {
        // «las 1 restantes irán por CPU» es lo que se imprimió de verdad el 22/09/2026 al bajar el
        // contexto a 16k y pasar de 35 capas a 39. El informe de arranque lo lee una persona en
        // cada arranque.
        let aviso = redactar_aviso(
            &Encaje::Parcial {
                en_gpu: 39,
                total: 40,
            },
            Some(1_000),
            None,
        );

        assert!(
            !aviso.contains("las 1 restantes"),
            "concordancia rota: {aviso}"
        );
        assert!(aviso.contains("la restante irá por CPU"), "{aviso}");
    }

    #[test]
    fn mas_conversaciones_en_paralelo_dejan_menos_capas_en_la_gpu() {
        // El defecto que esta prueba cierra, medido el 22/09/2026: `llama-server` reserva la caché
        // KV **una vez por secuencia**, y el encaje la contaba una sola vez. Con los cuatro slots
        // que el motor trae por defecto, el consumo real era cuatro veces el calculado y el
        // reparto se decidía sobre un presupuesto que no existía.
        // El modelo de juguete son cuatro capas de 1 MiB. Con contexto 2048 y caché q8_0, cada
        // capa carga además unos 0,53 MiB de caché por secuencia. Con estos 5 MB de tarjeta salen
        // tres capas si hay una sola secuencia y una sola si hay cuatro: la diferencia se ve.
        let (_dir, ruta) = gguf_de_juguete();
        let tarjeta = gpu(5_000_000);

        let mut una = config(CapasGpu::Auto, 2048);
        una.paralelo = 1;
        let mut cuatro = config(CapasGpu::Auto, 2048);
        cuatro.paralelo = 4;

        let con_una = resolver(&una, &ruta, Some(&tarjeta)).capas;
        let con_cuatro = resolver(&cuatro, &ruta, Some(&tarjeta)).capas;

        assert!(
            con_cuatro < con_una,
            "cuatro secuencias reservan cuatro cachés y deben caber menos capas: \
             con paralelo=1 salieron {con_una} y con paralelo=4, {con_cuatro}"
        );
    }

    #[test]
    fn un_numero_configurado_se_respeta_aunque_el_calculo_diga_que_no_cabe() {
        let (_dir, ruta) = gguf_de_juguete();

        let resolucion = resolver(&config(CapasGpu::Fijas(99), 512), &ruta, Some(&gpu(1_000)));

        assert_eq!(resolucion.capas, 99, "lo escrito en el TOML manda");
        assert!(
            !resolucion.aviso.is_empty(),
            "pero el aviso tiene que contar que con eso no cabe"
        );
    }

    #[test]
    fn sin_gpu_medible_auto_cae_al_99_de_siempre_diciendo_por_que() {
        let (_dir, ruta) = gguf_de_juguete();

        let resolucion = resolver(&config(CapasGpu::Auto, 512), &ruta, None);

        assert_eq!(resolucion.capas, 99);
        assert!(
            resolucion.aviso.to_lowercase().contains("gpu"),
            "tiene que decir que el dato que faltó es la GPU: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn con_un_modelo_ilegible_auto_cae_al_99_sin_tumbar_el_arranque() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("no-existe.gguf");

        let resolucion = resolver(&config(CapasGpu::Auto, 512), &ruta, Some(&gpu(16 << 30)));

        assert_eq!(resolucion.capas, 99);
        assert!(!resolucion.aviso.is_empty());
    }

    #[test]
    fn con_vram_de_sobra_auto_sube_el_modelo_entero_y_la_salida() {
        let (_dir, ruta) = gguf_de_juguete();

        let resolucion = resolver(&config(CapasGpu::Auto, 512), &ruta, Some(&gpu(16 << 30)));

        assert_eq!(resolucion.capas, 5, "cuatro capas más la salida");
        assert!(
            resolucion.aviso.contains("piden") && resolucion.aviso.contains("GiB"),
            "el aviso de «cabe entero» tiene que decir cuánto pide el modelo, no solo cuánto \
             hay libre: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn ni_la_primera_capa_cabe_y_el_aviso_dice_cuanto_pedia() {
        let (_dir, ruta) = gguf_de_juguete();
        // El GGUF de juguete tiene capas de 1 MiB; con 100 bytes libres no cabe ni una.
        let resolucion = resolver(&config(CapasGpu::Auto, 512), &ruta, Some(&gpu(100)));

        assert_eq!(resolucion.capas, 0);
        assert!(
            resolucion.aviso.contains("pide") && resolucion.aviso.contains("GiB"),
            "el aviso de «no cabe ninguna» tiene que decir cuánto pedía la capa que no cupo: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn el_aviso_del_reparto_parcial_dice_cuantas_capas_van_a_cada_sitio() {
        let (_dir, ruta) = gguf_de_juguete();
        // Dos capas de 1 MiB caben en 2,5 MiB; las cuatro más los embeddings, no.
        let vram = 2 * 1024 * 1024 + 512 * 1024;

        let resolucion = resolver(&config(CapasGpu::Auto, 512), &ruta, Some(&gpu(vram)));

        assert_eq!(resolucion.capas, 2);
        assert!(
            resolucion.aviso.contains("de 4"),
            "el aviso tiene que decir cuántas de cuántas: {}",
            resolucion.aviso
        );
        assert!(
            resolucion.aviso.contains("piden") && resolucion.aviso.contains("GiB"),
            "el aviso de un reparto parcial tiene que decir cuánto pedían esas dos capas: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn un_tipo_de_cache_que_llama_server_no_admite_no_es_calculable_y_lo_nombra() {
        let (_dir, ruta) = gguf_de_juguete();
        let mut cfg = config(CapasGpu::Fijas(11), 512);
        // «q3_k» existe en ggml pero no está entre los tipos que `llama-server` admite para
        // `--cache-type-k`/`--cache-type-v` (ver `tipo_de_cache`): es el mismo caso que ya cubre
        // `un_tipo_de_cache_inventado_no_se_acepta` en `mod pruebas`, aquí visto desde `resolver`.
        cfg.cache_kv = "q3_k".to_string();

        let resolucion = resolver(&cfg, &ruta, Some(&gpu(16 << 30)));

        assert_eq!(
            resolucion.capas, 11,
            "sin cálculo se respeta lo configurado"
        );
        assert!(
            resolucion.aviso.contains("q3_k"),
            "el aviso tiene que nombrar el tipo de caché que rechazó: {}",
            resolucion.aviso
        );
        assert!(
            !resolucion.aviso.to_lowercase().contains("gpu dedicada"),
            "no puede confundirse con el motivo de falta de GPU, que es otra rama: {}",
            resolucion.aviso
        );
    }

    #[test]
    fn cero_cabezas_de_atencion_no_es_calculable_y_no_entra_en_panico() {
        // La guarda de `calcular` está antes de `embedding / cabezas`: sin ella, esta cabecera
        // dividiría por cero. Copia los pares del GGUF de juguete cambiando solo
        // `attention.head_count` a 0; sin tensores, porque la guarda actúa antes de pesar nada.
        let pares = vec![
            ("general.architecture", ValorPrueba::Cadena("llama".into())),
            ("llama.block_count", ValorPrueba::U32(4)),
            ("llama.embedding_length", ValorPrueba::U32(512)),
            ("llama.attention.head_count", ValorPrueba::U32(0)),
            ("llama.attention.head_count_kv", ValorPrueba::U32(2)),
            ("llama.context_length", ValorPrueba::U32(32_768)),
        ];
        let (_dir, ruta) = escribir_temporal(&fichero_con_tensores(&pares, &[], 32));

        let resolucion = resolver(
            &config(CapasGpu::Fijas(7), 512),
            &ruta,
            Some(&gpu(16 << 30)),
        );

        assert_eq!(resolucion.capas, 7, "sin cálculo se respeta lo configurado");
        assert!(
            resolucion.aviso.contains("cero cabezas"),
            "el aviso tiene que decir que las cabezas de atención no valen: {}",
            resolucion.aviso
        );
        assert!(
            !resolucion.aviso.to_lowercase().contains("gpu dedicada"),
            "no puede confundirse con el motivo de falta de GPU, que es otra rama: {}",
            resolucion.aviso
        );
    }
}
