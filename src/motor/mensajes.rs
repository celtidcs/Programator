//! Lo que el motor le cuenta a una persona: el encaje de capas en la tarjeta, por qué no se pudo
//! calcular, el arranque y el cierre del motor y el modelo que se suelta. Un solo sitio para estos
//! textos: los módulos del motor deciden y aquí solo se redacta.
//!
//! Las líneas del registro acumulativo del encaje se quedan en `encaje_historico`: son un formato
//! de fichero, no avisos.

use std::path::Path;

use super::encaje::{Encaje, MedidaDelBanco, CAPAS_SI_NO_SE_SABE};
use super::hardware::Gpu;
use crate::unidades::BYTES_POR_GIB_F64;

/// El binario de `llama-server` configurado no existe.
pub(crate) fn sin_binario(binario: &Path) -> String {
    format!(
        "no se encontró el binario de llama-server en «{}»",
        binario.display()
    )
}

/// El sistema no dejó lanzar `llama-server`.
pub(crate) fn no_se_lanzo(binario: &Path, causa: &std::io::Error) -> String {
    format!("no se pudo lanzar «{}»: {causa}", binario.display())
}

/// El GGUF no dice cuántas capas tiene.
pub(crate) const SIN_CAPAS: &str = "el GGUF no declara ninguna capa";

/// No hay tarjeta que medir.
pub(crate) const SIN_GPU: &str = "no se ha podido medir ninguna GPU dedicada";

/// La cabecera dice que no hay cabezas de atención: no se puede repartir el caché.
pub(crate) const CERO_CABEZAS: &str = "la cabecera del GGUF declara cero cabezas de atención";

/// El caché de una secuencia no cabe ni en un `u64`.
pub(crate) const CACHE_DESBORDADA: &str = "el caché de ese contexto no cabe ni en un u64";

/// Un número de capas que no cabe en un `u32`.
pub(crate) fn capas_increibles(total: usize) -> String {
    format!("un modelo de {total} capas no es creíble")
}

/// El modelo no se dejó leer para pesarlo.
pub(crate) fn modelo_sin_pesar(fallo: &dyn std::fmt::Display) -> String {
    format!("el modelo no se ha podido pesar: {fallo}")
}

/// A la cabecera del GGUF le falta `clave`. Se nombra cuál, porque es la única pista que tiene el
/// usuario para saber qué mirar en el fichero.
pub(crate) fn falta_en_la_cabecera(clave: &str) -> String {
    format!("a la cabecera del GGUF le falta «{clave}»")
}

/// `cache_kv` no es un tipo que `llama-server` admita.
pub(crate) fn cache_desconocida(tipo: &str) -> String {
    format!("«{tipo}» no es un tipo de caché que llama-server admita")
}

/// El caché de todas las conversaciones en paralelo no cabe ni en un `u64`.
pub(crate) fn cache_paralela_desbordada(paralelo: u32) -> String {
    format!("el caché de {paralelo} conversaciones en paralelo no cabe ni en un u64")
}

/// El encaje cuando se usan las capas que midió el banco con este modelo en esta tarjeta.
pub(crate) fn aviso_de_medida(medida: &MedidaDelBanco, gpu: &Gpu, tope_mib: Option<u64>) -> String {
    let tope = tope_mib
        .map(|mib| format!(", con el tope de {mib} MiB"))
        .unwrap_or_default();
    format!(
        "se usan {} capas, las que midió el banco con este modelo en {} ({} libres{tope}): \
         ocuparon {} MiB",
        medida.capas,
        gpu.nombre,
        en_gib(gpu.vram_libre),
        medida.vram_ocupada_mib
    )
}

/// El aviso, con el tope de `vram_maxima_mib` que se aplicó.
pub(crate) fn aviso_con_tope(aviso: &str, tope_mib: u64) -> String {
    format!("{aviso} (con el tope de {tope_mib} MiB de vram_maxima_mib)")
}

/// El aviso, con un tope de `vram_maxima_mib` que no se pudo aplicar por no medir la tarjeta.
pub(crate) fn aviso_tope_sin_tarjeta(aviso: &str, tope_mib: u64) -> String {
    format!(
        "{aviso}; el tope de {tope_mib} MiB de vram_maxima_mib no se ha podido aplicar porque no \
         se ha podido medir la tarjeta"
    )
}

/// El aviso, con las capas fijadas en `capas_gpu` bajadas para caber en el tope.
pub(crate) fn aviso_capas_bajadas(aviso: &str, tope_mib: u64, fijadas: u32, caben: u32) -> String {
    format!(
        "{aviso}; las {fijadas} capas de capas_gpu no caben en el tope de {tope_mib} MiB de \
         vram_maxima_mib, así que se piden {caben} capas"
    )
}

/// Qué contarle al usuario. Un texto por cada desenlace del encaje, con las cifras en GiB para que
/// se puedan comparar de un vistazo con lo que dice el administrador de tareas. `coste` trae lo que
/// pedía la decisión tomada —`None` en `NoCalculable`, que no llegó a calcular nada—: es el número
/// con el que el usuario decidiría si tocar `margen_vram_mib`, y sin él el aviso solo diría cuánto
/// había libre, nunca cuánto se pidió.
pub(crate) fn redactar_aviso(encaje: &Encaje, coste: Option<u64>, gpu: Option<&Gpu>) -> String {
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
        // Todas las capas caben, pero no junto con los embeddings y la salida: sin esta rama
        // diría «las 0 restantes irán por CPU».
        Encaje::Parcial { en_gpu, total } if en_gpu >= total => format!(
            "en {donde} caben las {total} capas (piden {pide}), pero no junto con los embeddings y \
             la salida, que irán por CPU; el ciclo será algo más lento"
        ),
        Encaje::Parcial { en_gpu, total } => {
            let fuera = total.saturating_sub(*en_gpu);
            // Concordancia: con una sola capa fuera saldría «las 1 restantes irán», que es lo que
            // pasa al bajar el contexto a 16k. Un informe que se lee en cada arranque no puede
            // estar mal escrito.
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
///
/// `pub(crate)` y no privada: `encaje_historico` la reutiliza para que el log acumulativo hable de
/// VRAM con la misma cifra y el mismo redondeo que el aviso de arranque, en vez de reinventar el
/// formato.
pub(crate) fn en_gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / BYTES_POR_GIB_F64)
}

/// Por qué falla un encargo cuyo motor se cerró solo.
pub fn mensaje_motor_terminado(modelo: Option<&Path>) -> String {
    let modelo = modelo.map(|m| m.display().to_string()).unwrap_or_default();
    format!(
        "el motor local se cerró solo con el modelo «{modelo}» (su registro, en la ventana de \
         Programator, dice por qué)"
    )
}

/// El error cuando no hay motor y no se sabe arrancar uno.
pub fn mensaje_sin_motor_que_arrancar() -> String {
    "no hay ningún motor escuchando en el puerto configurado, y la configuración no trae \
     «motor.binario»/«motor.modelo» con los que arrancar uno propio"
        .to_string()
}

/// Redacta el aviso emitido por terminal al arrancar el motor con el encaje de capas resuelto.
pub fn mensaje_arranque_motor(aviso_encaje: &str) -> String {
    format!("Arrancando el motor — encaje: {aviso_encaje}")
}

/// Lo que se dice al soltar el modelo.
pub fn mensaje_modelo_soltado(modelo: &str, minutos_sin_encargos: u64) -> String {
    format!(
        "Se suelta el modelo «{modelo}» tras {minutos_sin_encargos} minutos sin encargos: la \
         tarjeta queda libre. El próximo encargo lo volverá a cargar."
    )
}

/// La respuesta del motor no trae la elección que se esperaba.
pub(crate) fn respuesta_sin_choices(json: &serde_json::Value) -> String {
    format!("la respuesta no trae «choices»: {json}")
}

/// Los argumentos de una llamada a herramienta no son JSON; se muestran recortados.
pub(crate) fn argumentos_ininterpretables(nombre: &str, truncado: &str) -> String {
    format!("no se pudieron interpretar los argumentos de la llamada a «{nombre}»: {truncado}")
}

/// El cuerpo de la respuesta del motor no se deja leer como JSON.
pub(crate) fn respuesta_ilegible(causa: &dyn std::fmt::Display) -> String {
    format!("respuesta ilegible: {causa}")
}

/// No se pudo instalar el manejador de Ctrl+C que apaga el motor al salir.
pub(crate) fn manejador_de_ctrl_c_sin_instalar(causa: &dyn std::fmt::Display) -> String {
    format!("no se pudo instalar el manejador de Ctrl+C: {causa}")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_motivos_sin_encaje_nombran_lo_que_falta() {
        assert_eq!(
            falta_en_la_cabecera("embedding_length"),
            "a la cabecera del GGUF le falta «embedding_length»"
        );
        assert_eq!(
            cache_desconocida("q9_0"),
            "«q9_0» no es un tipo de caché que llama-server admita"
        );
        assert!(cache_paralela_desbordada(4).contains("de 4 conversaciones"));
        assert!(capas_increibles(5_000_000_000).contains("5000000000 capas"));
    }

    /// Con todas las capas en la tarjeta pero no la salida, el aviso no dice «las 0 restantes irán
    /// por CPU».
    #[test]
    fn con_todas_las_capas_dentro_el_aviso_no_habla_de_cero_restantes() {
        let todas = redactar_aviso(
            &Encaje::Parcial {
                en_gpu: 40,
                total: 40,
            },
            Some(1 << 30),
            None,
        );
        assert!(!todas.contains("las 0"), "{todas}");
        assert!(todas.contains("caben las 40 capas"), "{todas}");
        assert!(todas.contains("la salida"), "{todas}");

        let una = redactar_aviso(
            &Encaje::Parcial {
                en_gpu: 39,
                total: 40,
            },
            Some(1 << 30),
            None,
        );
        assert!(una.contains("la restante irá por CPU"), "{una}");
        let varias = redactar_aviso(
            &Encaje::Parcial {
                en_gpu: 30,
                total: 40,
            },
            Some(1 << 30),
            None,
        );
        assert!(varias.contains("las 10 restantes irán por CPU"), "{varias}");
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
}
