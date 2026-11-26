//! Compone los argumentos con que se lanza `llama-server`. Solo calcula: no lanza nada.

use crate::config::{ModoDeCarga, MotorConfig, RepartoDeMemoria};
use crate::motor::encaje::EnCpu;
use std::path::Path;

/// El nombre del tipo de búfer de la memoria del equipo en `--override-tensor`:
/// `ggml_backend_cpu_buffer_type_get_name` en `ggml/src/ggml-backend.cpp:2439` de b10993.
const BUFER_DE_CPU: &str = "CPU";

/// El valor especial de `--cors-origins` que solo responde a `localhost`, `127.0.0.1` y `::1`
/// (`origin_is_localhost` en
/// https://raw.githubusercontent.com/ggml-org/llama.cpp/b10993/tools/server/server-http.cpp). Sin
/// él, `llama-server` da la cabecera a cualquier origen.
const ORIGENES_PERMITIDOS: &str = "localhost";

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
    en_cpu: EnCpu,
) -> Vec<String> {
    vec![
        binario.display().to_string(),
        "--model".into(),
        modelo.display().to_string(),
        // Fijo en el código, no configurable: si se pudiera fijar en la configuración alguien
        // podría ponerlo a "0.0.0.0" y dejar el modelo escuchando a toda la red local.
        "--host".into(),
        crate::motor::ANFITRION_LOCAL.into(),
        // Fijo por lo mismo: la cabecera de CORS solo se da a páginas servidas desde este equipo.
        // La protección de verdad es la clave (`motor::clave`); esto evita además que una página
        // ajena pueda leer siquiera las respuestas de rechazo.
        "--cors-origins".into(),
        ORIGENES_PERMITIDOS.into(),
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
        config.cache_k().to_string(),
        "--cache-type-v".into(),
        config.cache_v().to_string(),
        // Sin esta bandera, la API compatible-OpenAI no emite `tool_calls`: el modelo solo
        // devolvería texto suelto y el repertorio confinado no vería una sola solicitud.
        "--jinja".into(),
    ]
    .into_iter()
    .chain(opciones_del_reparto(&config.reparto, en_cpu))
    .collect()
}

/// Las opciones del reparto de memoria. Con los valores por defecto no hay ninguna, y la línea es
/// la de la 1.1.0. Los nombres son los de `common/arg.cpp` de b10993: `--no-kv-offload` (2412),
/// `--load-mode` (2687), `--override-tensor` (2750), `--n-cpu-moe` (2763) y `--n-cpu-ffn` (2773).
///
/// Las capas de `--n-cpu-moe` y `--n-cpu-ffn` llegan en `en_cpu`, como `capas_gpu`: con `"auto"`
/// las calcula el encaje, que es quien sabe pesar el modelo.
fn opciones_del_reparto(reparto: &RepartoDeMemoria, en_cpu: EnCpu) -> Vec<String> {
    let mut opciones = Vec::new();
    if reparto.cache_en_cpu {
        opciones.push("--no-kv-offload".to_string());
    }
    if reparto.modo_de_carga != ModoDeCarga::Auto {
        opciones.push("--load-mode".to_string());
        opciones.push(reparto.modo_de_carga.palabra().to_string());
    }
    for (opcion, capas) in [
        ("--n-cpu-moe", en_cpu.expertos),
        ("--n-cpu-ffn", en_cpu.ffn),
    ] {
        if capas > 0 {
            opciones.push(opcion.to_string());
            opciones.push(capas.to_string());
        }
    }
    for patron in &reparto.tensores_en_cpu {
        opciones.push("--override-tensor".to_string());
        opciones.push(format!("{patron}={BUFER_DE_CPU}"));
    }
    opciones
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::config::CapasEnCpu;
    use crate::motor::proceso::pruebas::config_de_prueba;

    /// Lo que el encaje resolvería con `config_con_todo_el_reparto`, que fija tres y cinco.
    const EN_CPU_DE_TODO_EL_REPARTO: EnCpu = EnCpu {
        expertos: 3,
        ffn: 5,
    };

    #[test]
    fn la_atencion_rapida_sale_de_la_configuracion() {
        let mut config = config_de_prueba();
        config.atencion_rapida = crate::config::AtencionRapida::Desactivada;
        let linea = linea_de_arranque(
            &config,
            std::path::Path::new("llama-server.exe"),
            std::path::Path::new("devstral.gguf"),
            99,
            EnCpu::default(),
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
            EnCpu::default(),
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
    fn la_linea_cierra_cors_y_no_lleva_ninguna_clave() {
        let linea = linea_de_arranque(
            &config_de_prueba(),
            std::path::Path::new("llama-server.exe"),
            std::path::Path::new("devstral.gguf"),
            99,
            EnCpu::default(),
        );

        let unida = linea.join(" ");
        assert!(unida.contains("--cors-origins localhost"), "{unida}");
        assert!(!unida.contains("--api-key"), "{unida}");
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
            EnCpu::default(),
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
            EnCpu::default(),
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
            EnCpu::default(),
        );
        comprobar_que_ninguna_opcion_se_queda_sin_valor(&linea);
        // Y con todas las opciones del reparto puestas, que añaden la única otra bandera suelta.
        let linea = linea_de_arranque(
            &config_con_todo_el_reparto(),
            std::path::Path::new("llama.exe"),
            std::path::Path::new("devstral.gguf"),
            35,
            EN_CPU_DE_TODO_EL_REPARTO,
        );
        comprobar_que_ninguna_opcion_se_queda_sin_valor(&linea);
    }

    /// Una configuración con cada clave del reparto distinta de su valor por defecto.
    fn config_con_todo_el_reparto() -> MotorConfig {
        let mut config = config_de_prueba();
        config.reparto = RepartoDeMemoria {
            cache_k: Some("f16".into()),
            cache_v: Some("q4_0".into()),
            cache_en_cpu: true,
            modo_de_carga: ModoDeCarga::MmapMlock,
            expertos_en_cpu: CapasEnCpu::Fijas(3),
            ffn_en_cpu: CapasEnCpu::Fijas(5),
            tensores_en_cpu: vec!["blk\\.0\\.attn_q".into(), "output".into()],
        };
        config
    }

    fn comprobar_que_ninguna_opcion_se_queda_sin_valor(linea: &[String]) {
        // Las únicas banderas que de verdad van sin valor. La lista es explícita a propósito:
        // añadir una obliga a pasar por aquí y a comprobar que esa opción sigue siendo booleana
        // en la versión del motor que se empaqueta. `--no-kv-offload` lo es en b10993: es la
        // negación de `--kv-offload` (`common/arg.cpp:2412`).
        const SIN_VALOR: &[&str] = &["--jinja", "--no-kv-offload"];

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
    fn con_el_reparto_por_defecto_la_linea_es_la_de_la_version_anterior() {
        // G1: la línea que componía la 1.1.0 con esta misma configuración, argumento a argumento.
        let linea = linea_de_arranque(
            &config_de_prueba(),
            std::path::Path::new("llama.exe"),
            std::path::Path::new("m.gguf"),
            40,
            EnCpu::default(),
        );
        let de_la_1_1_0: Vec<String> = [
            "llama.exe",
            "--model",
            "m.gguf",
            "--host",
            "127.0.0.1",
            "--cors-origins",
            "localhost",
            "--port",
            "8080",
            "--n-gpu-layers",
            "40",
            "--ctx-size",
            "32768",
            "--parallel",
            "1",
            "--flash-attn",
            "on",
            "--cache-type-k",
            "q8_0",
            "--cache-type-v",
            "q8_0",
            "--jinja",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(linea, de_la_1_1_0);
    }

    #[test]
    fn cada_clave_del_reparto_da_su_opcion() {
        // G2.
        let linea = linea_de_arranque(
            &config_con_todo_el_reparto(),
            std::path::Path::new("llama.exe"),
            std::path::Path::new("m.gguf"),
            40,
            EN_CPU_DE_TODO_EL_REPARTO,
        );
        let resto: Vec<&str> = linea
            .iter()
            .skip_while(|a| *a != "--jinja")
            .skip(1)
            .map(String::as_str)
            .collect();
        assert_eq!(
            resto,
            vec![
                "--no-kv-offload",
                "--load-mode",
                "mmap+mlock",
                "--n-cpu-moe",
                "3",
                "--n-cpu-ffn",
                "5",
                "--override-tensor",
                "blk\\.0\\.attn_q=CPU",
                "--override-tensor",
                "output=CPU",
            ]
        );
        let unida = linea.join(" ");
        assert!(unida.contains("--cache-type-k f16"), "{unida}");
        assert!(unida.contains("--cache-type-v q4_0"), "{unida}");
    }

    #[test]
    fn las_capas_en_cpu_son_las_que_le_pasan_no_las_del_toml() {
        // Como `--n-gpu-layers`: "auto" lo resuelve el encaje y la línea solo transcribe. Cero es no
        // mandar nada.
        let mut config = config_de_prueba();
        config.reparto.expertos_en_cpu = CapasEnCpu::Auto;
        config.reparto.ffn_en_cpu = CapasEnCpu::Fijas(7);
        let sin_nada = linea_de_arranque(
            &config,
            std::path::Path::new("llama.exe"),
            std::path::Path::new("m.gguf"),
            40,
            EnCpu::default(),
        )
        .join(" ");
        assert!(!sin_nada.contains("--n-cpu-moe"), "{sin_nada}");
        assert!(!sin_nada.contains("--n-cpu-ffn"), "{sin_nada}");

        let con_una = linea_de_arranque(
            &config,
            std::path::Path::new("llama.exe"),
            std::path::Path::new("m.gguf"),
            40,
            EnCpu {
                expertos: 0,
                ffn: 1,
            },
        )
        .join(" ");
        assert!(con_una.contains("--n-cpu-ffn 1"), "{con_una}");
        assert!(!con_una.contains("--n-cpu-moe"), "{con_una}");
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
            EnCpu::default(),
        )
        .join(" ");

        assert!(unida.contains("--n-gpu-layers 34"), "{unida}");
        assert!(
            !unida.contains("--n-gpu-layers 99"),
            "el 99 del TOML no puede colarse por detrás: {unida}"
        );
    }
}
