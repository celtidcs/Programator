//! Compone los argumentos con que se lanza `llama-server`. Solo calcula: no lanza nada.

use crate::config::MotorConfig;
use std::path::Path;

/// Compone la línea de arranque completa. Función pura: se prueba sin lanzar nada.
///
/// `capas_gpu` llega ya resuelto por `encaje::resolver`, no se lee de `config.capas_gpu`: esta
/// función no sabe pesar un modelo ni medir una GPU, así que no puede ser quien decida cuántas
/// capas caben. Es la misma separación que hay entre `hay_servidor` (mide) y quien llama (decide).
pub fn linea_de_arranque(
    config: &MotorConfig,
    binario: &Path,
    modelo: &Path,
    capas_gpu: u32,
) -> Vec<String> {
    vec![
        binario.display().to_string(),
        "--model".into(),
        modelo.display().to_string(),
        // Fijo en el código, no configurable: si se pudiera fijar en la configuración alguien
        // podría ponerlo a "0.0.0.0" y dejar el modelo escuchando a toda la red local.
        "--host".into(),
        crate::motor::ANFITRION_LOCAL.into(),
        "--port".into(),
        config.puerto.to_string(),
        "--n-gpu-layers".into(),
        capas_gpu.to_string(),
        "--ctx-size".into(),
        config.contexto.to_string(),
        // Explícito y no heredado del motor. `llama-server` reserva por defecto caché para cuatro
        // conversaciones simultáneas; Programator atiende los encargos en serie, así que tres
        // cuartas partes de esa reserva no se usan nunca y son las que echan las últimas capas del
        // modelo a la CPU. Medido: 9,8 tokens por segundo con cuatro secuencias, 23,0 con una.
        "--parallel".into(),
        config.paralelo.to_string(),
        // Lleva valor a propósito. En las builds modernas de llama.cpp —la empaquetada es la
        // b10993— «--flash-attn» dejó de ser una bandera suelta y exige «on», «off» o «auto»: sin
        // valor se traga el argumento siguiente y el servidor muere antes de cargar el modelo.
        "--flash-attn".into(),
        config.atencion_rapida.palabra().into(),
        "--cache-type-k".into(),
        config.cache_kv.clone(),
        "--cache-type-v".into(),
        config.cache_kv.clone(),
        // Sin esta bandera, la API compatible-OpenAI no emite `tool_calls`: el modelo solo
        // devolvería texto suelto y el repertorio confinado no vería una sola solicitud.
        "--jinja".into(),
    ]
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::proceso::pruebas::config_de_prueba;

    #[test]
    fn la_atencion_rapida_sale_de_la_configuracion() {
        let mut config = config_de_prueba();
        config.atencion_rapida = crate::config::AtencionRapida::Desactivada;
        let linea = linea_de_arranque(
            &config,
            std::path::Path::new("llama-server.exe"),
            std::path::Path::new("devstral.gguf"),
            99,
        );
        assert!(linea.join(" ").contains("--flash-attn off"), "{linea:?}");
    }

    #[test]
    fn la_linea_de_arranque_lleva_los_parametros_que_fija_la_especificacion() {
        let linea = linea_de_arranque(
            &config_de_prueba(),
            std::path::Path::new("llama-server.exe"),
            std::path::Path::new("devstral.gguf"),
            99,
        );

        let unida = linea.join(" ");
        assert!(unida.contains("--n-gpu-layers 99"));
        // Con su valor, no a secas: una «--flash-attn» suelta se come el argumento siguiente y el
        // servidor muere antes de cargar el modelo. Pasó de verdad el 22/09/2026 contra la
        // b10993, y la aserción anterior no lo cazaba porque solo miraba el nombre de la bandera.
        assert!(unida.contains("--flash-attn on"), "{unida}");
        assert!(unida.contains("--cache-type-k q8_0"));
        assert!(unida.contains("--cache-type-v q8_0"));
        assert!(unida.contains("--ctx-size 32768"));
        assert!(unida.contains("--port 8080"));
        assert!(unida.contains("devstral.gguf"));
        assert!(
            unida.contains("--jinja"),
            "sin --jinja el modelo no emite tool_calls y el repertorio confinado no recibe nada"
        );
        assert!(
            unida.contains("--host 127.0.0.1"),
            "el servidor no debe escuchar en toda la red local"
        );
    }

    #[test]
    fn la_linea_de_arranque_fija_el_paralelismo_en_vez_de_heredarlo() {
        // Medido el 22/09/2026: sin «--parallel», llama-server reserva caché para cuatro
        // conversaciones simultáneas. Programator atiende los encargos de uno en uno, así que tres
        // cuartas partes de esa reserva no se usan nunca — y son las que echan cinco capas del
        // modelo a la CPU, que cuesta pasar de 23,0 a 9,8 tokens por segundo.
        let unida = linea_de_arranque(
            &config_de_prueba(),
            std::path::Path::new("llama.exe"),
            std::path::Path::new("devstral.gguf"),
            40,
        )
        .join(" ");

        assert!(unida.contains("--parallel 1"), "{unida}");
    }

    #[test]
    fn el_paralelismo_sale_de_la_configuracion_no_de_una_constante() {
        let mut config = config_de_prueba();
        config.paralelo = 3;

        let unida = linea_de_arranque(
            &config,
            std::path::Path::new("llama.exe"),
            std::path::Path::new("devstral.gguf"),
            40,
        )
        .join(" ");

        assert!(unida.contains("--parallel 3"), "{unida}");
    }

    #[test]
    fn ninguna_opcion_de_la_linea_de_arranque_se_queda_sin_su_valor() {
        // El 22/09/2026, en el primer arranque real contra el modelo, «--flash-attn» iba suelta y
        // la b10993 la tomó por una opción con valor: se tragó «--cache-type-k» como si fuera su
        // valor y el servidor murió antes de cargar nada. Esta prueba cierra la clase entera del
        // fallo, no solo aquel caso: en esta línea toda opción va seguida de su valor, nunca de
        // otra opción. Si algún día hace falta una bandera sin valor, esta prueba hay que
        // discutirla, no borrarla.
        let linea = linea_de_arranque(
            &config_de_prueba(),
            std::path::Path::new("llama.exe"),
            std::path::Path::new("devstral.gguf"),
            35,
        );

        // Las únicas banderas que de verdad van sin valor. La lista es explícita a propósito:
        // añadir una obliga a pasar por aquí y a comprobar que esa opción sigue siendo booleana
        // en la versión del motor que se empaqueta.
        const SIN_VALOR: &[&str] = &["--jinja"];

        // Se salta el primer elemento, que es el propio ejecutable.
        let argumentos = &linea[1..];
        for (posicion, argumento) in argumentos.iter().enumerate() {
            if !argumento.starts_with("--") || SIN_VALOR.contains(&argumento.as_str()) {
                continue;
            }
            let siguiente = argumentos.get(posicion + 1);
            assert!(
                siguiente.is_some_and(|s| !s.starts_with("--")),
                "«{argumento}» se queda sin valor y se comería la opción siguiente: {linea:?}"
            );
        }
    }

    #[test]
    fn la_linea_de_arranque_lleva_las_capas_que_le_pasan_no_las_del_toml() {
        // `config_de_prueba()` trae `capas_gpu: Fijas(99)` en el TOML, pero eso ya no es lo que
        // decide `--n-gpu-layers`: quien resuelve las capas de verdad es `encaje::resolver`, y esta
        // función solo transcribe el número que le pasan por parámetro.
        let unida = linea_de_arranque(
            &config_de_prueba(),
            std::path::Path::new("llama.exe"),
            std::path::Path::new("m.gguf"),
            34,
        )
        .join(" ");

        assert!(unida.contains("--n-gpu-layers 34"), "{unida}");
        assert!(
            !unida.contains("--n-gpu-layers 99"),
            "el 99 del TOML no puede colarse por detrás: {unida}"
        );
    }
}
