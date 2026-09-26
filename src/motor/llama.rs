//! Cliente contra la API compatible-OpenAI de `llama-server`.
//!
//! Sin runtime asíncrono a propósito: el arnés espera al modelo de todas formas, y `ureq` bloqueante
//! deja un ejecutable mucho más pequeño y una pila de llamadas que se lee de un vistazo.

use std::time::Duration;

use super::{Mensaje, Motor, Papel, Respuesta, SolicitudHerramienta};
use crate::error::{Error, Resultado};

/// Tiempo máximo por defecto esperando los primeros bytes de la respuesta. Sin streaming, `llama-server` no
/// manda ni un byte hasta terminar de generar, así que este plazo es en la práctica el de la
/// generación entera: ha de ser holgado para no cortar respuestas legítimas. Lo único que
/// importa de verdad es que sea finito, para que el arnés nunca se quede colgado esperando al
/// modelo si el servidor acepta la conexión y luego no contesta.
///
/// Se deriva de `config::tiempo_lectura_motor_segundos_por_defecto()`.
const TIEMPO_LECTURA_POR_DEFECTO: Duration =
    Duration::from_secs(crate::config::tiempo_lectura_motor_segundos_por_defecto());

/// Tiempo máximo por defecto para escribir la petición en el socket. Mandar el cuerpo JSON es casi
/// instantáneo; un plazo corto basta para detectar una conexión que no admite escritura, sin
/// alargar la espera ante un fallo real de conexión.
///
/// Se deriva de `config::tiempo_escritura_motor_segundos_por_defecto()`.
const TIEMPO_ESCRITURA_POR_DEFECTO: Duration =
    Duration::from_secs(crate::config::tiempo_escritura_motor_segundos_por_defecto());

pub struct MotorLlama {
    base_url: String,
    modelo: String,
    herramientas: Vec<String>,
    muestreo: crate::config::Muestreo,
    agente: ureq::Agent,
}

impl MotorLlama {
    pub fn nuevo(base_url: &str, modelo: &str) -> Self {
        Self::con_agente(
            base_url,
            modelo,
            agente_con_tiempos(TIEMPO_LECTURA_POR_DEFECTO, TIEMPO_ESCRITURA_POR_DEFECTO),
        )
    }

    /// Fija los plazos máximos de lectura y escritura HTTP con el motor. Permite adaptar la espera
    /// a máquinas más lentas o modelos grandes desde la configuración de `[motor]`.
    pub fn con_tiempos_espera(
        mut self,
        tiempo_lectura: Duration,
        tiempo_escritura: Duration,
    ) -> Self {
        self.agente = agente_con_tiempos(tiempo_lectura, tiempo_escritura);
        self
    }

    /// Declara el repertorio que el modelo puede solicitar.
    pub fn con_herramientas(mut self, nombres: &[&str]) -> Self {
        self.herramientas = nombres.iter().map(|n| n.to_string()).collect();
        self
    }

    /// Fija los parámetros de muestreo que se mandan en cada petición. `config.toml` es la única
    /// fuente de verdad para ellos: sin esta llamada, se usan los valores por defecto de
    /// `Muestreo`.
    pub fn con_muestreo(mut self, muestreo: crate::config::Muestreo) -> Self {
        self.muestreo = muestreo;
        self
    }

    fn con_agente(base_url: &str, modelo: &str, agente: ureq::Agent) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            modelo: modelo.to_string(),
            herramientas: Vec::new(),
            muestreo: crate::config::Muestreo::default(),
            agente,
        }
    }

    /// Constructor mínimo para pruebas: permite fijar un tiempo de lectura corto para comprobar
    /// que un servidor que no responde produce un error y no un cuelgue, sin esperar los 600 s
    /// reales. No es parte de la interfaz pública: ese tiempo no se hace configurable aquí, sino
    /// en `config.rs`, que es de otra tarea.
    #[cfg(test)]
    fn con_tiempo_lectura(base_url: &str, modelo: &str, tiempo_lectura: Duration) -> Self {
        Self::con_agente(
            base_url,
            modelo,
            agente_con_tiempos(tiempo_lectura, TIEMPO_ESCRITURA_POR_DEFECTO),
        )
    }
}

fn agente_con_tiempos(tiempo_lectura: Duration, tiempo_escritura: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_read(tiempo_lectura)
        .timeout_write(tiempo_escritura)
        .build()
}

/// Construye el cuerpo JSON de la petición. Función pura: se prueba sin red.
///
/// Todos los parámetros de muestreo se mandan siempre y explícitos, aunque coincidan con su
/// valor por defecto: `config.toml` es la única fuente de verdad, y así no queda ninguno a
/// merced de lo que decida el servidor. La única excepción es la semilla, que solo se manda si
/// está fijada; omitirla es justo lo que significa «que el servidor elija una al azar».
pub fn cuerpo_peticion(
    conversacion: &[Mensaje],
    modelo: &str,
    herramientas: &[&str],
    muestreo: &crate::config::Muestreo,
) -> serde_json::Value {
    let mensajes: Vec<serde_json::Value> = conversacion
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": match m.papel {
                    Papel::Sistema => "system",
                    Papel::Usuario => "user",
                    Papel::Modelo => "assistant",
                },
                "content": m.contenido,
            })
        })
        .collect();

    // **El modelo tiene que recibir los nombres de sus argumentos.** Hasta la 0.10.0 esto mandaba
    // `{"type": "object"}` pelado: los verbos iban con nombre y sin forma, así que el modelo solo
    // podía adivinar cómo se llamaba cada argumento. El 23/09/2026 adivinó cinco veces seguidas y
    // costó un encargo entero. La forma sale de `arnes::ficha`, que es la única declaración.
    let declaradas: Vec<serde_json::Value> = herramientas
        .iter()
        .map(|nombre| {
            let ficha = crate::arnes::ficha::buscar(nombre);
            let mut propiedades = serde_json::Map::new();
            let mut obligatorios: Vec<serde_json::Value> = Vec::new();

            if let Some(ficha) = ficha {
                for argumento in ficha.obligatorios {
                    propiedades.insert(
                        argumento.nombre.to_string(),
                        serde_json::json!({ "type": "string", "description": argumento.para_que }),
                    );
                    obligatorios.push(serde_json::json!(argumento.nombre));
                }
                for argumento in ficha.opcionales {
                    propiedades.insert(
                        argumento.nombre.to_string(),
                        serde_json::json!({ "type": "string", "description": argumento.para_que }),
                    );
                }
            }

            serde_json::json!({
                "type": "function",
                "function": {
                    "name": nombre,
                    "description": ficha.map(|f| f.para_que).unwrap_or_default(),
                    "parameters": {
                        "type": "object",
                        "properties": propiedades,
                        "required": obligatorios,
                    }
                }
            })
        })
        .collect();

    let mut cuerpo = serde_json::json!({
        "model": modelo,
        "messages": mensajes,
        "tools": declaradas,
        "temperature": muestreo.temperatura,
        "top_k": muestreo.top_k,
        "top_p": muestreo.top_p,
        "min_p": muestreo.min_p,
        "repeat_penalty": muestreo.penalizacion_repeticion,
    });

    if let Some(semilla) = muestreo.semilla {
        cuerpo["seed"] = serde_json::json!(semilla);
    }

    cuerpo
}

/// Interpreta la respuesta de la API. Función pura: se prueba sin red.
pub fn interpretar_respuesta(json: &serde_json::Value) -> Resultado<Respuesta> {
    let mensaje = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .ok_or_else(|| {
            Error::MotorSinRespuesta(format!("la respuesta no trae «choices»: {json}"))
        })?;

    if let Some(llamada) = mensaje
        .get("tool_calls")
        .and_then(|t| t.get(0))
        .and_then(|t| t.get("function"))
    {
        let nombre = llamada
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| Error::MotorSinRespuesta("llamada sin nombre".to_string()))?;

        // Distingue los tres casos que puede mandar el servidor en vez de colapsarlos todos en
        // «argumentos vacíos»: un objeto JSON ya listo, una cadena que hay que interpretar (y que
        // puede venir rota), o la ausencia total de argumentos, que sí es legítima.
        let argumentos = match llamada.get("arguments") {
            Some(valor @ serde_json::Value::Object(_)) => valor.clone(),
            Some(serde_json::Value::String(crudos)) => serde_json::from_str(crudos)
                .map_err(|_| {
                    let truncado: String = crudos.chars().take(200).collect();
                    Error::MotorSinRespuesta(format!(
                        "no se pudieron interpretar los argumentos de la llamada a «{nombre}»: {truncado}"
                    ))
                })?,
            _ => serde_json::json!({}),
        };

        return Ok(Respuesta::Herramienta(SolicitudHerramienta {
            nombre: nombre.to_string(),
            argumentos,
        }));
    }

    let texto = mensaje
        .get("content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| Error::MotorSinRespuesta("la respuesta no trae contenido".to_string()))?;

    Ok(Respuesta::Texto(texto.to_string()))
}

/// Lo que el motor devuelve cuando la conversación no le cabe. No hay código propio para esto: es
/// un 400 como cualquier otro, y el cuerpo es lo único que lo distingue.
const SENALES_DE_CONTEXTO: &[&str] = &["context", "n_ctx", "exceed", "too long"];

/// Traduce el rechazo del motor a algo que se pueda leer sin abrir el registro.
///
/// El 23/09/2026 un desbordamiento de ventana se manifestó como `status code 400` repetido en
/// bucle. El código no decía nada y costó una tarde. El cuerpo sí lo decía.
fn motivo_del_rechazo(estado: u16, cuerpo: &str) -> String {
    let minusculas = cuerpo.to_lowercase();
    if estado == 400 && SENALES_DE_CONTEXTO.iter().any(|s| minusculas.contains(s)) {
        return format!(
            "el motor rechazó la petición porque NO CABE EN SU VENTANA DE CONTEXTO (HTTP {estado}). \
             No es una caída: es que la conversación es más larga que la ventana configurada. \
             Acorta el encargo, o pídele que lea los ficheros con «leer_fichero» en vez de \
             pegárselos. Respuesta del motor: {cuerpo}"
        );
    }
    format!("el motor respondió HTTP {estado}: {cuerpo}")
}

impl Motor for MotorLlama {
    fn responder(&mut self, conversacion: &[Mensaje]) -> Resultado<Respuesta> {
        let nombres: Vec<&str> = self.herramientas.iter().map(|s| s.as_str()).collect();
        let cuerpo = cuerpo_peticion(conversacion, &self.modelo, &nombres, &self.muestreo);

        let respuesta = self
            .agente
            .post(&format!("{}/v1/chat/completions", self.base_url))
            .send_json(cuerpo)
            // El rechazo por ventana llega como un 400 más, y su código no dice nada: lo que lo
            // distingue está en el cuerpo. Traducirlo aquí es lo que evita que un desbordamiento
            // se vea desde fuera como «el arnés ha dejado de atender», que es lo que costó una
            // tarde el 23/09/2026.
            .map_err(|fallo| match fallo {
                ureq::Error::Status(estado, respuesta_error) => {
                    let cuerpo_error = respuesta_error.into_string().unwrap_or_default();
                    Error::MotorSinRespuesta(motivo_del_rechazo(estado, &cuerpo_error))
                }
                otro => Error::MotorSinRespuesta(otro.to_string()),
            })?;

        let json: serde_json::Value = respuesta
            .into_json()
            .map_err(|e| Error::MotorSinRespuesta(format!("respuesta ilegible: {e}")))?;

        interpretar_respuesta(&json)
    }

    fn comprobar_salud(&mut self) -> super::proceso::EstadoServidor {
        super::proceso::hay_servidor_url(&self.base_url)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::{Mensaje, Papel};

    #[test]
    fn el_cuerpo_traduce_los_papeles_al_formato_de_la_api() {
        let conversacion = vec![
            Mensaje {
                papel: Papel::Sistema,
                contenido: "normas".into(),
            },
            Mensaje {
                papel: Papel::Usuario,
                contenido: "encargo".into(),
            },
            Mensaje {
                papel: Papel::Modelo,
                contenido: "respuesta".into(),
            },
        ];

        let cuerpo = cuerpo_peticion(
            &conversacion,
            "devstral",
            &["leer_fichero"],
            &crate::config::Muestreo::default(),
        );

        let mensajes = cuerpo["messages"].as_array().unwrap();
        assert_eq!(mensajes[0]["role"], "system");
        assert_eq!(mensajes[1]["role"], "user");
        assert_eq!(mensajes[2]["role"], "assistant");
        assert_eq!(cuerpo["model"], "devstral");
    }

    #[test]
    fn el_cuerpo_declara_el_repertorio_de_herramientas() {
        let cuerpo = cuerpo_peticion(
            &[],
            "devstral",
            &["leer_fichero", "publicar"],
            &crate::config::Muestreo::default(),
        );

        let nombres: Vec<&str> = cuerpo["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["function"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(nombres, vec!["leer_fichero", "publicar"]);
    }

    #[test]
    fn el_cuerpo_declara_los_argumentos_de_cada_herramienta_y_cuales_son_obligatorios() {
        let cuerpo = cuerpo_peticion(
            &[],
            "devstral",
            &["escribir_propuesta"],
            &crate::config::Muestreo::default(),
        );

        let funcion = &cuerpo["tools"][0]["function"];
        assert_eq!(funcion["name"], "escribir_propuesta");

        let propiedades = &funcion["parameters"]["properties"];
        assert!(
            propiedades["nombre"]["type"] == "string"
                && propiedades["contenido"]["type"] == "string",
            "el modelo no recibe los nombres de sus argumentos: {propiedades}"
        );
        assert!(
            !propiedades["contenido"]["description"]
                .as_str()
                .unwrap_or_default()
                .is_empty(),
            "cada argumento lleva su explicación"
        );

        let obligatorios = funcion["parameters"]["required"]
            .as_array()
            .expect("required es una lista");
        assert_eq!(obligatorios.len(), 2);
        assert!(obligatorios.contains(&serde_json::json!("nombre")));
        assert!(obligatorios.contains(&serde_json::json!("contenido")));
    }

    #[test]
    fn una_herramienta_sin_argumentos_declara_properties_vacio_y_no_ausente() {
        let cuerpo = cuerpo_peticion(
            &[],
            "devstral",
            &["proponer_poda"],
            &crate::config::Muestreo::default(),
        );

        let parametros = &cuerpo["tools"][0]["function"]["parameters"];
        assert!(
            parametros["properties"].is_object(),
            "properties tiene que estar, aunque vacío: {parametros}"
        );
        assert_eq!(parametros["required"].as_array().map(|r| r.len()), Some(0));
    }

    #[test]
    fn interpreta_una_respuesta_de_texto() {
        let json = serde_json::json!({
            "choices": [{ "message": { "content": "He terminado." } }]
        });

        let respuesta = interpretar_respuesta(&json).unwrap();

        assert!(matches!(respuesta, Respuesta::Texto(ref t) if t == "He terminado."));
    }

    #[test]
    fn interpreta_una_llamada_a_herramienta() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "function": {
                            "name": "leer_fichero",
                            "arguments": "{\"ruta\":\"src/main.rs\"}"
                        }
                    }]
                }
            }]
        });

        let respuesta = interpretar_respuesta(&json).unwrap();

        match respuesta {
            Respuesta::Herramienta(s) => {
                assert_eq!(s.nombre, "leer_fichero");
                assert_eq!(s.argumentos["ruta"], "src/main.rs");
            }
            otro => panic!("se esperaba una herramienta y llegó {otro:?}"),
        }
    }

    #[test]
    fn una_respuesta_sin_choices_es_un_error_y_no_un_panico() {
        let json = serde_json::json!({ "error": "modelo no cargado" });

        let fallo = interpretar_respuesta(&json).unwrap_err();

        assert!(matches!(fallo, crate::error::Error::MotorSinRespuesta(_)));
    }

    #[test]
    fn habla_con_un_servidor_de_prueba_de_extremo_a_extremo() {
        use std::io::{Read, Write};

        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();

        let hilo = std::thread::spawn(move || {
            let (mut conexion, _) = escucha.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let _ = conexion.read(&mut buffer);
            let cuerpo = r#"{"choices":[{"message":{"content":"hola"}}]}"#;
            let respuesta = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                cuerpo.len(),
                cuerpo
            );
            conexion.write_all(respuesta.as_bytes()).unwrap();

            // Drena la conexión hasta que el cliente termine de leer la respuesta y cierre su
            // lado. Soltar el socket justo después de escribir es una carrera: el sistema puede
            // mandar un RST en vez de un cierre ordenado si todavía queda algo pendiente, y
            // ureq lo interpretaría como un fallo de red en vez de una respuesta completa.
            loop {
                match conexion.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        });

        // El motor se suelta al terminar este bloque, antes de esperar al hilo servidor: su
        // agente mantiene la conexión abierta en su fondo de reutilización (keep-alive de
        // HTTP/1.1), y esa conexión no se cierra hasta que el agente se destruye. Sin soltarlo
        // aquí, el drenado del servidor esperaría un EOF que nunca llegaría y ambos lados se
        // quedarían colgados el uno esperando al otro.
        let respuesta = {
            let mut motor = MotorLlama::nuevo(&format!("http://127.0.0.1:{puerto}"), "devstral");
            motor
                .responder(&[Mensaje {
                    papel: Papel::Usuario,
                    contenido: "hola".into(),
                }])
                .unwrap()
        };

        hilo.join().unwrap();
        assert!(matches!(respuesta, Respuesta::Texto(ref t) if t == "hola"));
    }

    #[test]
    fn un_servidor_que_acepta_y_no_responde_da_error_en_vez_de_colgarse() {
        use std::io::Read;

        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();

        let hilo = std::thread::spawn(move || {
            let (mut conexion, _) = escucha.accept().unwrap();
            // No escribe nunca una respuesta. Se limita a leer hasta que el cliente cierra por
            // su cuenta, al agotarse su tiempo de espera de lectura.
            let mut buffer = [0u8; 4096];
            loop {
                match conexion.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        });

        let mut motor = MotorLlama::con_tiempo_lectura(
            &format!("http://127.0.0.1:{puerto}"),
            "devstral",
            Duration::from_millis(200),
        );
        let resultado = motor.responder(&[Mensaje {
            papel: Papel::Usuario,
            contenido: "hola".into(),
        }]);

        hilo.join().unwrap();
        assert!(matches!(resultado, Err(Error::MotorSinRespuesta(_))));
    }

    #[test]
    fn argumentos_como_objeto_json_se_usan_tal_cual_sin_pasar_por_texto() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "function": {
                            "name": "leer_fichero",
                            "arguments": { "ruta": "src/main.rs" }
                        }
                    }]
                }
            }]
        });

        let respuesta = interpretar_respuesta(&json).unwrap();

        match respuesta {
            Respuesta::Herramienta(s) => {
                assert_eq!(s.nombre, "leer_fichero");
                assert_eq!(s.argumentos["ruta"], "src/main.rs");
            }
            otro => panic!("se esperaba una herramienta y llegó {otro:?}"),
        }
    }

    #[test]
    fn argumentos_como_cadena_con_json_roto_es_un_error_distinto_del_de_argumentos_ausentes() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "function": {
                            "name": "leer_fichero",
                            "arguments": "{ruta: sin comillas}"
                        }
                    }]
                }
            }]
        });

        let fallo = interpretar_respuesta(&json).unwrap_err();

        let mensaje = match fallo {
            Error::MotorSinRespuesta(mensaje) => mensaje,
            otro => panic!("se esperaba MotorSinRespuesta y llegó {otro:?}"),
        };
        assert!(mensaje.contains("leer_fichero"));
        assert_ne!(mensaje, "llamada sin nombre");
    }

    #[test]
    fn argumentos_ausentes_siguen_produciendo_un_objeto_vacio_y_no_un_error() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "function": { "name": "sin_argumentos" }
                    }]
                }
            }]
        });

        let respuesta = interpretar_respuesta(&json).unwrap();

        match respuesta {
            Respuesta::Herramienta(s) => {
                assert_eq!(s.nombre, "sin_argumentos");
                assert_eq!(s.argumentos, serde_json::json!({}));
            }
            otro => panic!("se esperaba una herramienta y llegó {otro:?}"),
        }
    }

    #[test]
    fn el_cuerpo_lleva_todos_los_parametros_de_muestreo() {
        let muestreo = crate::config::Muestreo {
            temperatura: 0.7,
            top_k: 64,
            top_p: 0.9,
            min_p: 0.1,
            penalizacion_repeticion: 1.2,
            semilla: None,
        };

        let cuerpo = cuerpo_peticion(&[], "devstral", &[], &muestreo);

        assert!((cuerpo["temperature"].as_f64().unwrap() - 0.7).abs() < 1e-6);
        assert_eq!(cuerpo["top_k"], 64);
        assert!((cuerpo["top_p"].as_f64().unwrap() - 0.9).abs() < 1e-6);
        assert!((cuerpo["min_p"].as_f64().unwrap() - 0.1).abs() < 1e-6);
        assert!((cuerpo["repeat_penalty"].as_f64().unwrap() - 1.2).abs() < 1e-6);
    }

    #[test]
    fn sin_semilla_no_se_manda_la_clave_seed() {
        let muestreo = crate::config::Muestreo {
            semilla: None,
            ..crate::config::Muestreo::default()
        };

        let cuerpo = cuerpo_peticion(&[], "devstral", &[], &muestreo);

        assert!(
            cuerpo.get("seed").is_none(),
            "no debe haber clave «seed» cuando la semilla es None, y el cuerpo fue: {cuerpo}"
        );
    }

    #[test]
    fn con_semilla_se_manda_la_clave_seed() {
        let muestreo = crate::config::Muestreo {
            semilla: Some(42),
            ..crate::config::Muestreo::default()
        };

        let cuerpo = cuerpo_peticion(&[], "devstral", &[], &muestreo);

        assert_eq!(cuerpo["seed"], 42);
    }

    #[test]
    fn un_400_por_contexto_se_explica_con_palabras_y_no_con_el_codigo() {
        let motivo = motivo_del_rechazo(400, "the request exceeds the available context size");
        assert!(motivo.contains("VENTANA DE CONTEXTO"), "{motivo}");
        assert!(motivo.contains("leer_fichero"), "dice qué hacer: {motivo}");
    }

    #[test]
    fn un_400_que_no_es_de_contexto_no_se_disfraza_de_uno() {
        let motivo = motivo_del_rechazo(400, "invalid json");
        assert!(!motivo.contains("VENTANA DE CONTEXTO"), "{motivo}");
        assert!(motivo.contains("invalid json"), "{motivo}");
    }

    #[test]
    fn un_400_por_contexto_desde_el_servidor_llega_como_error_legible() {
        use std::io::{Read, Write};

        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();

        let hilo = std::thread::spawn(move || {
            let (mut conexion, _) = escucha.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let _ = conexion.read(&mut buffer);
            let cuerpo_error = "the request exceeds the available context size";
            let respuesta = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                cuerpo_error.len(),
                cuerpo_error
            );
            conexion.write_all(respuesta.as_bytes()).unwrap();

            loop {
                match conexion.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        });

        let fallo = {
            let mut motor = MotorLlama::nuevo(&format!("http://127.0.0.1:{puerto}"), "devstral");
            motor
                .responder(&[Mensaje {
                    papel: Papel::Usuario,
                    contenido: "hola".into(),
                }])
                .unwrap_err()
        };

        hilo.join().unwrap();
        let mensaje = match fallo {
            crate::error::Error::MotorSinRespuesta(msg) => msg,
            otro => panic!("se esperaba MotorSinRespuesta y llegó {otro:?}"),
        };

        assert!(
            mensaje.contains("VENTANA DE CONTEXTO"),
            "el error debe explicarse como falta de contexto: {mensaje}"
        );
        assert!(
            mensaje.contains("leer_fichero"),
            "el error debe sugerir qué hacer: {mensaje}"
        );
    }
}
