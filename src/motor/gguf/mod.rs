//! Lector de ficheros GGUF: cabecera, metadatos y tabla de tensores.
//!
//! Formato: <https://github.com/ggml-org/ggml/blob/master/docs/gguf.md> (especificación oficial
//! de `ggml-org`, consultada el 15-09-2026). Un `.gguf` empieza por el número mágico `GGUF`
//! (cuatro bytes), la versión (u32), el número de tensores (u64) y el número de pares
//! clave-valor (u64), todo en little-endian. Después vienen esos pares: cada uno es una cadena
//! (longitud u64 + bytes UTF-8) seguida de un tipo (u32) y un valor cuya codificación depende del
//! tipo. Justo después de los pares viene la tabla de tensores, y después de esa tabla, en la
//! primera posición alineada, el bloque de datos.
//!
//! La maquinaria de bytes —contar, acotar, descodificar— vive en [`lector`]: no sabe qué es un
//! modelo ni una capa. Este módulo sabe qué significan esos bytes; [`tensores`] sabe a qué capa
//! pertenece cada uno.

mod lector;
pub(super) mod tensores;

use crate::error::{Error, Resultado};
use lector::{LectorAcotado, TipoValor, ValorLeido};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use tensores::LIMITE_NUMERO_TENSORES;

/// Número mágico que identifica un fichero GGUF: los cuatro bytes ASCII «GGUF».
const MAGIA_GGUF: u32 = 0x4655_4747; // "GGUF" leído como u32 little-endian.

/// Versiones de GGUF que este lector entiende. El resto de la cabecera no cambia entre 1 y 3; una
/// versión futura sí podría cambiarlo, así que se rechaza en vez de arriesgarse a malinterpretar.
const VERSIONES_SOPORTADAS: std::ops::RangeInclusive<u32> = 1..=3;

/// Tope de una cadena de la cabecera. Los nombres de modelo y plantillas de chat reales miden
/// como mucho unos pocos kilobytes; 1 MiB da margen de sobra sin aceptar la mentira de «esta
/// cadena mide varios exabytes» que cabría en un `u64`.
const LIMITE_LONGITUD_CADENA: u64 = 1024 * 1024;

/// Tope de elementos de un array. El array real más grande de un GGUF es el vocabulario del
/// tokenizador (cientos de miles de entradas); cinco millones da veinte veces ese margen sin
/// quedarse iterando ante un contador inventado.
const LIMITE_ELEMENTOS_ARRAY: u64 = 5_000_000;

/// Tope de pares clave-valor. Los GGUF reales traen de una decena a un par de centenares; cien
/// mil es generoso sin dejar que un contador absurdo obligue a iterar sin fin.
const LIMITE_NUMERO_CLAVES: u64 = 100_000;

/// Profundidad máxima de arrays anidados. Ningún GGUF real anida arrays: el tope solo evita que
/// un fichero adversario fuerce una recursión sin fondo repitiendo array-de-array.
const LIMITE_PROFUNDIDAD_ARRAY: u32 = 8;

/// Tope total de bytes que este lector consume de la cabecera, se guarden o se descarten. La
/// cabecera de un GGUF real se queda en pocos megabytes; 64 MiB da margen amplio y, sobre todo,
/// no depende de acertar con ningún límite individual: por muchas claves y arrays que alguien
/// encadene, el total nunca se acerca a los gigabytes de los pesos del modelo.
const LIMITE_BYTES_METADATOS: u64 = 64 * 1024 * 1024;

/// Lo que se puede extraer de la cabecera de un GGUF. Todo opcional: hay arquitecturas raras que
/// no traen alguna clave, y quien use esto tiene que poder distinguir «no cabe» de «no lo sé».
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetadatosModelo {
    pub arquitectura: Option<String>,
    pub nombre: Option<String>,
    pub tipo_archivo: Option<u64>,
    pub numero_capas: Option<u64>,
    pub longitud_embedding: Option<u64>,
    pub cabezas_atencion: Option<u64>,
    pub cabezas_atencion_kv: Option<u64>,
    pub longitud_contexto: Option<u64>,
    pub alineacion: Option<u64>,
}

fn error_gguf(ruta: &Path, causa: String) -> Error {
    Error::GgufInvalido {
        ruta: ruta.to_path_buf(),
        causa,
    }
}

/// Lo que trae la primera mitad del fichero: el número de tensores que la tabla va a declarar y
/// los pares clave-valor ya interpretados.
struct Cabecera {
    numero_tensores: u64,
    metadatos: MetadatosModelo,
}

fn leer_cabecera_de<R: Read>(lector: &mut LectorAcotado<R>, ruta: &Path) -> Resultado<Cabecera> {
    let magia = lector.leer_u32(ruta)?;
    if magia != MAGIA_GGUF {
        return Err(error_gguf(
            ruta,
            "número mágico inválido: no empieza por «GGUF», no es un fichero GGUF".into(),
        ));
    }

    let version = lector.leer_u32(ruta)?;
    if !VERSIONES_SOPORTADAS.contains(&version) {
        return Err(error_gguf(
            ruta,
            format!("versión de GGUF desconocida: {version}"),
        ));
    }

    // Antes se descartaba. Ahora hace falta, y se acota aquí mismo: la prueba del contador
    // absurdo exige que se rechace antes de empezar a iterar, no dentro del bucle.
    let numero_tensores = lector.leer_u64(ruta)?;
    if numero_tensores > LIMITE_NUMERO_TENSORES {
        return Err(error_gguf(
            ruta,
            format!(
                "la cabecera dice tener {numero_tensores} tensores, más del límite de {LIMITE_NUMERO_TENSORES}"
            ),
        ));
    }

    let numero_claves = lector.leer_u64(ruta)?;
    if numero_claves > LIMITE_NUMERO_CLAVES {
        return Err(error_gguf(
            ruta,
            format!(
                "la cabecera dice tener {numero_claves} pares clave-valor, más del límite de {LIMITE_NUMERO_CLAVES}"
            ),
        ));
    }

    let mut metadatos = MetadatosModelo::default();
    for _ in 0..numero_claves {
        let clave = lector.leer_cadena(ruta)?;
        let codigo_tipo = lector.leer_u32(ruta)?;
        let tipo = TipoValor::desde_codigo(codigo_tipo).ok_or_else(|| {
            error_gguf(
                ruta,
                format!("tipo de valor desconocido para la clave «{clave}»: código {codigo_tipo}"),
            )
        })?;
        let valor = lector.consumir_valor(tipo, 0, ruta)?;
        asignar_campo(&mut metadatos, &clave, valor);
    }

    Ok(Cabecera {
        numero_tensores,
        metadatos,
    })
}

/// Lee solo la cabecera de `ruta` y devuelve los metadatos que hacen falta para el encaje en
/// hardware. Nunca lee los pesos del tensor: se detiene en cuanto termina de recorrer los pares
/// clave-valor, mucho antes de llegar a la sección de tensores.
pub fn leer_metadatos(ruta: &Path) -> Resultado<MetadatosModelo> {
    let fichero = File::open(ruta).map_err(|causa| Error::Lectura {
        ruta: ruta.to_path_buf(),
        causa,
    })?;
    let mut lector = LectorAcotado::nuevo(BufReader::new(fichero));
    leer_metadatos_de(&mut lector, ruta)
}

/// El cuerpo real del lector, separado de `leer_metadatos` para poder probarlo contra cualquier
/// `Read` (un `Vec<u8>` en memoria en las pruebas) sin pasar por el sistema de ficheros.
fn leer_metadatos_de<R: Read>(
    lector: &mut LectorAcotado<R>,
    ruta: &Path,
) -> Resultado<MetadatosModelo> {
    Ok(leer_cabecera_de(lector, ruta)?.metadatos)
}

/// Todo lo que se puede saber de un GGUF sin leer sus pesos.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InformeGguf {
    pub metadatos: MetadatosModelo,
    pub pesos: tensores::PesoDelModelo,
}

/// Lee la cabecera **y** la tabla de tensores en una sola pasada, y devuelve los metadatos junto
/// con el peso de cada capa. Sigue sin tocar un solo byte de los pesos: se detiene donde empieza
/// el bloque de datos, y solo comprueba con `metadata()` que ese bloque cabe en el fichero, sin
/// leer su contenido.
pub fn leer_modelo(ruta: &Path) -> Resultado<InformeGguf> {
    let fichero = File::open(ruta).map_err(|causa| Error::Lectura {
        ruta: ruta.to_path_buf(),
        causa,
    })?;
    let mut lector = LectorAcotado::nuevo(BufReader::new(fichero));
    let cabecera = leer_cabecera_de(&mut lector, ruta)?;
    let pesos = tensores::leer_tensores_de(&mut lector, &cabecera, ruta)?;

    // La tabla puede declarar tensores que no caben en el propio fichero —truncado o mentido—.
    // No hace falta leer ese bloque para saberlo: basta el tamaño que ya conoce el sistema de
    // ficheros. Detectarlo aquí evita que el encaje decida con un peso que no existe de verdad.
    let fin_absoluto = pesos
        .inicio_datos
        .checked_add(pesos.fin_datos)
        .ok_or_else(|| {
            error_gguf(
                ruta,
                "el bloque de datos desborda al sumar su inicio y su tamaño".into(),
            )
        })?;
    let tamano_fichero = std::fs::metadata(ruta)
        .map_err(|causa| Error::Lectura {
            ruta: ruta.to_path_buf(),
            causa,
        })?
        .len();
    if fin_absoluto > tamano_fichero {
        return Err(error_gguf(
            ruta,
            format!(
                "el fichero mide {tamano_fichero} bytes pero la tabla de tensores declara datos \
                 hasta el byte {fin_absoluto}: está truncado"
            ),
        ));
    }

    Ok(InformeGguf {
        metadatos: cabecera.metadatos,
        pesos,
    })
}

/// Reparte el valor recién leído entre los campos de `metadatos` según el nombre de la clave.
/// Se compara por sufijo (`.block_count`, etc.) en vez de exigir el prefijo exacto de la
/// arquitectura: así no importa en qué orden aparezcan las claves, y `general.architecture` puede
/// venir antes o después de las claves prefijadas con su propio nombre.
fn asignar_campo(metadatos: &mut MetadatosModelo, clave: &str, valor: ValorLeido) {
    let terminada_en = |sufijo: &str| clave.ends_with(sufijo);
    match (clave, valor) {
        ("general.architecture", ValorLeido::Cadena(t)) => metadatos.arquitectura = Some(t),
        ("general.name", ValorLeido::Cadena(t)) => metadatos.nombre = Some(t),
        ("general.file_type", ValorLeido::Numero(n)) => metadatos.tipo_archivo = Some(n),
        ("general.alignment", ValorLeido::Numero(n)) => metadatos.alineacion = Some(n),
        (_, ValorLeido::Numero(n)) if terminada_en(".block_count") => {
            metadatos.numero_capas = Some(n)
        }
        (_, ValorLeido::Numero(n)) if terminada_en(".embedding_length") => {
            metadatos.longitud_embedding = Some(n)
        }
        (_, ValorLeido::Numero(n)) if terminada_en(".attention.head_count") => {
            metadatos.cabezas_atencion = Some(n)
        }
        (_, ValorLeido::Numero(n)) if terminada_en(".attention.head_count_kv") => {
            metadatos.cabezas_atencion_kv = Some(n)
        }
        (_, ValorLeido::Numero(n)) if terminada_en(".context_length") => {
            metadatos.longitud_contexto = Some(n)
        }
        _ => {}
    }
}

#[cfg(test)]
mod pruebas {
    use super::tensores::pruebas_apoyo::*;
    use super::*;

    #[test]
    fn lee_todos_los_campos_de_una_cabecera_completa() {
        let (_dir, ruta) = escribir_temporal(&cabecera_valida(&pares_llama_completos()));

        let metadatos = leer_metadatos(&ruta).unwrap();

        assert_eq!(metadatos.arquitectura.as_deref(), Some("llama"));
        assert_eq!(metadatos.nombre.as_deref(), Some("Devstral-24B"));
        assert_eq!(metadatos.tipo_archivo, Some(2));
        assert_eq!(metadatos.numero_capas, Some(40));
        assert_eq!(metadatos.longitud_embedding, Some(5120));
        assert_eq!(metadatos.cabezas_atencion, Some(32));
        assert_eq!(metadatos.cabezas_atencion_kv, Some(8));
        assert_eq!(metadatos.longitud_contexto, Some(131072));
    }

    #[test]
    fn a_una_cabecera_incompleta_le_faltan_solo_los_campos_ausentes() {
        let pares = vec![
            (
                "general.architecture",
                ValorPrueba::Cadena("gptneox".into()),
            ),
            ("gptneox.block_count", ValorPrueba::U32(24)),
        ];
        let (_dir, ruta) = escribir_temporal(&cabecera_valida(&pares));

        let metadatos = leer_metadatos(&ruta).unwrap();

        assert_eq!(metadatos.arquitectura.as_deref(), Some("gptneox"));
        assert_eq!(metadatos.numero_capas, Some(24));
        assert_eq!(metadatos.nombre, None);
        assert_eq!(metadatos.tipo_archivo, None);
        assert_eq!(metadatos.longitud_embedding, None);
        assert_eq!(metadatos.cabezas_atencion, None);
        assert_eq!(metadatos.cabezas_atencion_kv, None);
        assert_eq!(metadatos.longitud_contexto, None);
    }

    #[test]
    fn reconoce_los_doce_tipos_de_valor_y_no_se_descoloca_con_los_que_ignora() {
        let pares = vec![
            ("relleno.u8", ValorPrueba::U8(7)),
            ("relleno.i8", ValorPrueba::I8(-7)),
            ("relleno.u16", ValorPrueba::U16(700)),
            ("relleno.i16", ValorPrueba::I16(-700)),
            ("relleno.i32", ValorPrueba::I32(-70000)),
            ("relleno.f32", ValorPrueba::F32(1.5)),
            ("relleno.bool", ValorPrueba::Bool(true)),
            ("relleno.i64", ValorPrueba::I64(-70000)),
            ("relleno.f64", ValorPrueba::F64(2.5)),
            ("relleno.array", ValorPrueba::ArrayU32(vec![1, 2, 3, 4, 5])),
            ("general.architecture", ValorPrueba::Cadena("llama".into())),
            ("llama.block_count", ValorPrueba::U64(32)),
        ];
        let (_dir, ruta) = escribir_temporal(&cabecera_valida(&pares));

        let metadatos = leer_metadatos(&ruta).unwrap();

        // Si el lector se hubiera descolocado al saltarse cualquiera de los tipos de relleno,
        // estos dos campos —que vienen justo después— habrían leído basura o habrían fallado.
        assert_eq!(metadatos.arquitectura.as_deref(), Some("llama"));
        assert_eq!(metadatos.numero_capas, Some(32));
    }

    #[test]
    fn magia_incorrecta_da_error_legible_sin_panico() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"OTRA");
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes());
        let (_dir, ruta) = escribir_temporal(&buf);

        let error = leer_metadatos(&ruta).unwrap_err();

        assert!(matches!(error, Error::GgufInvalido { .. }));
    }

    #[test]
    fn fichero_truncado_a_mitad_de_una_cadena_da_error_legible() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"GGUF");
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes());
        buf.extend_from_slice(&1u64.to_le_bytes()); // dice que viene un par
        buf.extend_from_slice(&20u64.to_le_bytes()); // la clave mide 20 bytes...
        buf.extend_from_slice(b"corta"); // ...pero el fichero se acaba a los 5
        let (_dir, ruta) = escribir_temporal(&buf);

        let error = leer_metadatos(&ruta).unwrap_err();

        assert!(matches!(error, Error::GgufInvalido { .. }));
    }

    #[test]
    fn longitud_de_cadena_absurda_da_error_en_vez_de_reservar_memoria() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"GGUF");
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes());
        buf.extend_from_slice(&1u64.to_le_bytes());
        buf.extend_from_slice(&u64::MAX.to_le_bytes()); // «esta clave mide 18 exabytes»
        let (_dir, ruta) = escribir_temporal(&buf);

        let error = leer_metadatos(&ruta).unwrap_err();

        assert!(matches!(error, Error::GgufInvalido { .. }));
    }

    #[test]
    fn array_con_longitud_absurda_da_error_en_vez_de_colgarse() {
        let pares = vec![("general.architecture", ValorPrueba::Cadena("llama".into()))];
        let mut buf = cabecera_valida(&pares);
        // Un segundo par a mano, con un array que dice tener u64::MAX elementos de tipo U8.
        escribir_cadena(&mut buf, "otra.clave");
        buf.extend_from_slice(&9u32.to_le_bytes()); // tipo ARRAY
        buf.extend_from_slice(&0u32.to_le_bytes()); // elementos de tipo U8
        buf.extend_from_slice(&u64::MAX.to_le_bytes()); // «con dieciocho trillones de elementos»
                                                        // Hay que corregir el contador de pares: eran 1 y ahora son 2.
        buf[4 + 4 + 8..4 + 4 + 8 + 8].copy_from_slice(&2u64.to_le_bytes());
        let (_dir, ruta) = escribir_temporal(&buf);

        let error = leer_metadatos(&ruta).unwrap_err();

        assert!(matches!(error, Error::GgufInvalido { .. }));
    }

    #[test]
    fn tipo_de_valor_inexistente_da_error_legible() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"GGUF");
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes());
        buf.extend_from_slice(&1u64.to_le_bytes());
        escribir_cadena(&mut buf, "clave.rara");
        buf.extend_from_slice(&99u32.to_le_bytes()); // tipo que no existe en la especificación
        let (_dir, ruta) = escribir_temporal(&buf);

        let error = leer_metadatos(&ruta).unwrap_err();

        assert!(matches!(error, Error::GgufInvalido { .. }));
    }

    #[test]
    fn no_lee_el_fichero_entero() {
        let mut buf = cabecera_valida(&pares_llama_completos());
        let tamano_cabecera = buf.len();
        // Cien megabytes de relleno detrás de la cabecera: si el lector los tocara, esta prueba
        // tardaría segundos en vez de milisegundos, y en un `.gguf` real serían gigabytes de
        // pesos.
        buf.resize(tamano_cabecera + 100 * 1024 * 1024, 0xAA);
        let (_dir, ruta) = escribir_temporal(&buf);

        let inicio = std::time::Instant::now();
        let metadatos = leer_metadatos(&ruta).unwrap();
        let transcurrido = inicio.elapsed();

        assert_eq!(metadatos.numero_capas, Some(40));
        assert!(
            transcurrido < std::time::Duration::from_secs(2),
            "leer la cabecera tardó {transcurrido:?}: parece que se leyó el relleno"
        );
    }

    /// Prueba manual, no de humo: pesa el Devstral real y comprueba que la suma que declara su
    /// propia tabla de tensores es coherente con el tamaño del fichero en disco. Necesita el
    /// modelo descargado en `modelos/devstral-small-2-24b-Q4_K_M.gguf` —el mismo nombre de fichero
    /// que fija `programator.ejemplo.toml`—, relativo a la raíz del paquete, que es desde donde
    /// corre `cargo test`. Se ignora por defecto: sin ese GGUF de varios gigabytes no hay nada que
    /// pesar, y si el fichero no está, no se deja reventar con un pánico de Rust y su traza —el
    /// Director la va a ejecutar a mano, y un pánico ahí es mala guía—: se avisa y se sale. Se deja
    /// puesta para el futuro, igual que la sonda de `motor::hardware`.
    ///
    /// **La igualdad no puede ser exacta.** `fin_datos` es el máximo de `offset + bytes` **sin
    /// alinear**, pero un escritor GGUF real rellena hasta la alineación también después del
    /// último tensor, no solo entre tensores consecutivos. Con un último tensor en Q4_K
    /// (144 mod 32 = 16) o en Q6_K (210 mod 32 = 18) sobran entre 1 y 31 bytes de relleno final que
    /// `inicio_datos + fin_datos` no cuenta. Exigir la igualdad exacta habría dado un falso
    /// negativo con un cálculo correcto; se tolera cualquier diferencia menor que la alineación.
    #[test]
    #[ignore]
    fn el_peso_del_devstral_real_cuadra_con_el_tamano_del_fichero() {
        let ruta = Path::new("modelos/devstral-small-2-24b-Q4_K_M.gguf");
        if !ruta.exists() {
            println!(
                "«{}» no está en este equipo: prueba omitida.",
                ruta.display()
            );
            return;
        }
        let informe = leer_modelo(ruta).expect("no se pudo leer el Devstral real");
        let tamano_fichero = std::fs::metadata(ruta)
            .expect("no se pudo consultar el tamaño del Devstral real")
            .len();
        let suma = informe.pesos.inicio_datos + informe.pesos.fin_datos;
        // Mismo valor por defecto que `ALINEACION_POR_DEFECTO` en `tensores.rs`: no se importa esa
        // constante privada solo para repetir aquí el mismo `32` que ya fija la especificación.
        let alineacion = informe
            .metadatos
            .alineacion
            .filter(|a| *a > 0)
            .unwrap_or(32);
        let relleno = tamano_fichero.saturating_sub(suma);

        println!("inicio_datos + fin_datos = {suma} | tamaño del fichero = {tamano_fichero}");
        assert!(
            suma <= tamano_fichero && relleno < alineacion,
            "inicio_datos + fin_datos ({suma}) debería quedar a menos de {alineacion} bytes por \
             debajo del tamaño del fichero ({tamano_fichero}): la diferencia de {relleno} bytes \
             es el relleno de alineación que un escritor real deja tras el último tensor, y por \
             eso se tolera hasta una alineación entera; una diferencia mayor, o que la suma \
             supere al fichero, sí sería un tamaño mal calculado en la tabla de tipos"
        );
    }
}
