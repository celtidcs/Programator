//! Qué hay al otro lado del puerto del motor: sondea `/health` y `/props` y da el veredicto.
//!
//! Solo mide, no decide: si hay que reutilizar, esperar o arrancar otro lo decide quien llama.

use crate::motor::clave::ClaveDelMotor;
use crate::motor::url_local;
use std::time::Duration;

/// Qué hay realmente al otro lado de un puerto donde Programator podría reutilizar o arrancar su
/// `llama-server`. Un `bool` no basta: hace falta distinguir tres situaciones que exigen tres
/// reacciones distintas de quien llame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoServidor {
    /// Hay un `llama-server` de verdad, con el modelo ya cargado: se puede usar tal cual.
    Listo,
    /// Hay un `llama-server` de verdad, pero `/health` todavía contesta 503 porque sigue cargando
    /// el modelo. Lo correcto es esperar a la siguiente comprobación, **no** arrancar otro en el
    /// mismo puerto: acabaría fallando contra un puerto ya ocupado.
    Cargando,
    /// El puerto no tiene un `llama-server` respondiendo: puede que no haya nadie escuchando, que
    /// haya algo que ni siquiera hable HTTP, o que hable HTTP pero no sea `llama-server`.
    /// Programator no debe reutilizar ese puerto, y tampoco debe intentar arrancar el suyo ahí
    /// (fallaría contra un puerto ya ocupado): quien llame decide qué avisar, según sepa si
    /// esperaba encontrar algo ahí o no.
    NoDisponible,
    /// Hay un `llama-server` que pide una clave y no acepta la de Programator: no es suyo, o es uno
    /// suyo cuya clave se perdió. No se puede usar ni arrancar otro encima, porque el puerto está
    /// ocupado.
    RechazaLaClave,
}

impl std::fmt::Display for EstadoServidor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(crate::motor::mensajes::estado_del_servidor(self))
    }
}

/// Tiempo máximo de la comprobación de `/health` completa (conexión + envío + respuesta). Corto a
/// propósito: esta función corre en cada arranque del ciclo, así que un `llama-server` que no
/// responda no puede colgar Programator antes de que haya arrancado nada. También cubre, con el
/// mismo plazo, el caso de que no haya nadie escuchando: `ureq` falla ahí mucho antes, al fallar la
/// propia conexión, así que este tiempo en la práctica solo se agota de verdad si algo acepta la
/// conexión y luego se queda callado.
const TIEMPO_SALUD: Duration = Duration::from_secs(2);

/// El código con que `/health` de `llama-server` dice que el modelo todavía se está cargando
/// (`tools/server/README.md` de llama.cpp: «503 … Loading model»).
const HTTP_SERVICIO_NO_DISPONIBLE: u16 = 503;

/// El código con que `llama-server` rechaza una petición sin clave o con otra
/// (`middleware_validate_api_key` en `tools/server/server-http.cpp` de b10993).
const HTTP_NO_AUTORIZADO: u16 = 401;

/// Lo que contestó `/props`.
enum Propiedades {
    DeLlamaServer,
    Otras,
    ClaveRechazada,
}

/// Campos de la respuesta JSON de `GET /props` que delatan a un `llama-server` de verdad. Basta con
/// **uno cualquiera** de ellos.
///
/// Se aceptan varios a propósito. Ninguno se ha podido confirmar contra un binario real —no hay uno
/// instalado en este entorno—, y exigir un nombre concreto convertiría esa incertidumbre en un
/// fallo silencioso y permanente: si esa versión de `llama-server` no lo trajera, Programator no
/// daría nunca por listo a su propio motor, intentaría arrancar otro en un puerto ya ocupado, y
/// repetiría el error en cada ciclo sin que nada explicara por qué. Reconocer cualquiera de los
/// cuatro no debilita la comprobación: lo que hay que descartar es un healthcheck genérico, y
/// ninguno de estos campos aparece en uno.
const CAMPOS_PROPS_LLAMA_SERVER: &[&str] = &[
    "total_slots",
    "model_path",
    "default_generation_settings",
    "chat_template",
];

/// ¿Es `props`, el JSON de `GET /props`, el de un `llama-server`? Lo es si trae alguno de
/// `CAMPOS_PROPS_LLAMA_SERVER`. Función pura: decide sin tocar la red.
fn son_props_de_llama_server(props: &serde_json::Value) -> bool {
    CAMPOS_PROPS_LLAMA_SERVER
        .iter()
        .any(|campo| props.get(campo).is_some())
}

/// ¿Es `agente` capaz de hablar con un `llama-server` de verdad en `url_base`?
///
/// Solo se llama cuando `/health` ya contestó 200: hace falta descartar todavía el caso de un
/// healthcheck genérico (ver la documentación de `hay_servidor`), así que se pide `GET /props` y se
/// exige que conteste 200 con un cuerpo JSON que traiga `CAMPO_PROPS_LLAMA_SERVER`. Cualquier otra
/// cosa —sin `/props`, con otro código, o con JSON que no sea el de `llama.cpp`— no es un
/// `llama-server`.
///
/// `/props` pide la clave y `/health` no (son públicas solo `/health` y `/v1/health`), así que es
/// aquí donde se sabe si el motor acepta la de Programator.
fn responde_props_de_llama_server_url(
    agente: &ureq::Agent,
    url_base: &str,
    clave: Option<&ClaveDelMotor>,
) -> Propiedades {
    let mut peticion = agente.get(&format!("{url_base}{}", crate::motor::RUTA_PROPIEDADES));
    if let Some(clave) = clave {
        peticion = peticion.header(CABECERA_DE_LA_CLAVE, &clave.cabecera());
    }
    let respuesta = match peticion.call() {
        Ok(respuesta) => respuesta,
        Err(ureq::Error::StatusCode(HTTP_NO_AUTORIZADO)) => return Propiedades::ClaveRechazada,
        Err(_) => return Propiedades::Otras,
    };

    match respuesta.into_body().read_json::<serde_json::Value>() {
        Ok(json) if son_props_de_llama_server(&json) => Propiedades::DeLlamaServer,
        _ => Propiedades::Otras,
    }
}

/// La cabecera en que `llama-server` busca la clave, con el prefijo `Bearer `.
pub const CABECERA_DE_LA_CLAVE: &str = "Authorization";

/// El agente de `/health` y `/props`: `TIEMPO_SALUD` para todo, de la conexión a la respuesta,
/// como `timeout` de `ureq` 2. Un 503 llega como `ureq::Error::StatusCode`.
fn agente_de_salud() -> ureq::Agent {
    crate::red::configuracion()
        .timeout_global(Some(TIEMPO_SALUD))
        .build()
        .into()
}

/// Comprueba el estado de un servidor en una URL base dada, con la clave que se le manda.
pub fn hay_servidor_url(url_base: &str, clave: Option<&ClaveDelMotor>) -> EstadoServidor {
    let base = url_base.trim_end_matches('/');
    let agente = agente_de_salud();

    match agente
        .get(&format!("{base}{}", crate::motor::RUTA_SALUD))
        .call()
    {
        Ok(mut respuesta) => {
            // `ureq` 3 solo devuelve la conexión a su reserva si se lee el cuerpo entero
            // (`ureq-3.5.2/src/body/mod.rs`); sin esto, `/props` abriría otra conexión. Lo que
            // diga el cuerpo de `/health` no importa: el veredicto lo da `/props`.
            let _cuerpo_de_salud = respuesta.body_mut().read_to_vec();
            match responde_props_de_llama_server_url(&agente, base, clave) {
                Propiedades::DeLlamaServer => EstadoServidor::Listo,
                Propiedades::ClaveRechazada => EstadoServidor::RechazaLaClave,
                Propiedades::Otras => EstadoServidor::NoDisponible,
            }
        }
        Err(ureq::Error::StatusCode(HTTP_SERVICIO_NO_DISPONIBLE)) => EstadoServidor::Cargando,
        // Cualquier otro código HTTP, un error de transporte (incluida la ausencia de nadie
        // escuchando), o una respuesta que ni siquiera es HTTP válido: nada de eso es un
        // `llama-server` respondiendo con normalidad.
        Err(_) => EstadoServidor::NoDisponible,
    }
}

/// ¿Qué hay escuchando en `puerto`?
///
/// Antes solo comprobaba que algo aceptara la conexión TCP, y eso basta para confundir cualquier
/// otro servicio que ocupe el puerto (el 8080 es de los más comunes que hay) con el `llama-server`
/// de Programator. El primer paso hacia el veredicto de verdad es la respuesta HTTP de `/health`:
/// devuelve 200 cuando el modelo está listo y 503 mientras está cargando. Sin comprobación TCP
/// previa aparte: `ureq` ya intenta conectar como primer paso de la petición, así que una
/// comprobación separada solo repetiría ese mismo trabajo sin aportar nada.
///
/// **Por qué un 200 en `/health` no basta por sí solo, y hace falta una segunda petición:**
/// `/health` respondiendo `200 {"status":"ok"}` es el formato de healthcheck más extendido que
/// hay —lo usan Kubernetes, Docker, y prácticamente cualquier framework web—, así que un 200 ahí no
/// distingue un `llama-server` de cualquier otro servicio que alguien tenga escuchando en ese
/// puerto. Comprobar el cuerpo de `/health` tampoco arreglaría nada, porque un servicio genérico
/// puede devolver ese mismo JSON. Por eso, ante un 200, se confirma con `GET /props`
/// (`responde_props_de_llama_server_url`), un endpoint que solo tiene `llama.cpp`: sin esa segunda
/// petición, cualquier healthcheck genérico en ese puerto se habría colado como `Listo`, que es
/// exactamente el defecto que esta función vino a corregir, solo que reducido de «cualquier TCP» a
/// «cualquier HTTP con 200 en `/health`». Ambas peticiones comparten el mismo plazo corto
/// (`TIEMPO_SALUD`): dos peticiones de dos segundos siguen siendo baratas para algo que corre en
/// cada arranque.
pub fn hay_servidor(puerto: u16, clave: Option<&ClaveDelMotor>) -> EstadoServidor {
    hay_servidor_url(&url_local(puerto), clave)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El estado llega a la entrega del modelo, y no debe salir con su nombre de Rust.
    #[test]
    fn el_estado_del_servidor_se_cuenta_sin_nombres_de_rust() {
        for (estado, nombre) in [
            (EstadoServidor::Listo, "Listo"),
            (EstadoServidor::Cargando, "Cargando"),
            (EstadoServidor::NoDisponible, "NoDisponible"),
            (EstadoServidor::RechazaLaClave, "RechazaLaClave"),
        ] {
            let texto = estado.to_string();
            assert!(texto.starts_with("el servidor "), "{texto}");
            assert!(!texto.contains(nombre), "{texto}");
        }
    }

    #[test]
    fn las_props_se_reconocen_por_cualquiera_de_sus_campos_propios_y_sin_red() {
        for campo in CAMPOS_PROPS_LLAMA_SERVER {
            let mut mapa = serde_json::Map::new();
            mapa.insert((*campo).to_string(), serde_json::Value::from(1));
            let props = serde_json::Value::Object(mapa);
            assert!(son_props_de_llama_server(&props), "{props}");
        }
        let ajenas = serde_json::json!({"status": "healthy", "version": "1.0"});
        assert!(!son_props_de_llama_server(&ajenas));
    }

    /// Compone una respuesta HTTP completa (línea de estado, `Content-Length` correcto y cuerpo).
    fn respuesta_http(codigo: u16, motivo: &str, cuerpo: &str) -> String {
        format!(
            "HTTP/1.1 {codigo} {motivo}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            cuerpo.len(),
            cuerpo
        )
    }

    /// Levanta un `TcpListener` en un puerto libre y, en un hilo aparte, acepta una única conexión
    /// por la que sirve, en orden y sobre esa misma conexión persistente, una respuesta por cada
    /// entrada de `respuestas`: la primera petición que llegue recibe `respuestas[0]`, la segunda
    /// `respuestas[1]`, etc. Basta con el orden porque `hay_servidor` siempre pide primero
    /// `/health` y, solo si hace falta, `/props` después: no hace falta leer la ruta de la
    /// petición para saber cuál toca contestar.
    ///
    /// **Por qué una sola conexión y no una por respuesta:** `ureq` reutiliza por `keep-alive` la
    /// conexión de `/health` para la petición de `/props` que pueda venir después —es lo que haría
    /// cualquier cliente HTTP/1.1 razonable contra un servidor que no ha dicho `Connection: close`,
    /// y es exactamente lo que haría un `llama-server` real—, así que un fixture que esperara una
    /// conexión nueva por cada respuesta se quedaría con la segunda `accept()` colgada para
    /// siempre: la petición de `/props` llegaría por la conexión ya aceptada, no por una nueva.
    ///
    /// Tras la última respuesta, drena la conexión hasta EOF antes de cerrarla, igual que en
    /// `motor::llama`: soltar el socket justo tras escribir es una carrera que a veces manda un RST
    /// en vez de un cierre ordenado, y `ureq` lo toma por un fallo de red en vez de una respuesta
    /// completa.
    fn servidor_de_prueba_con_secuencia(
        respuestas: Vec<String>,
    ) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};

        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();

        let hilo = std::thread::spawn(move || {
            let (mut conexion, _) = escucha.accept().unwrap();
            let mut buffer = [0u8; 4096];

            let mut lineas = Vec::new();
            for respuesta in respuestas {
                let leidos = conexion.read(&mut buffer).unwrap_or(0);
                let peticion = String::from_utf8_lossy(&buffer[..leidos]).to_string();
                lineas.push(peticion.lines().next().unwrap_or_default().to_string());
                let _ = conexion.write_all(respuesta.as_bytes());
            }

            loop {
                match conexion.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
            lineas
        });

        (puerto, hilo)
    }

    /// Atajo para el caso, muy frecuente en estas pruebas, de una sola conexión con una sola
    /// respuesta.
    fn servidor_de_prueba_que_responde(
        respuesta_http: String,
    ) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        servidor_de_prueba_con_secuencia(vec![respuesta_http])
    }

    #[test]
    fn el_agente_de_salud_tiene_un_plazo_total_y_va_sin_proxy() {
        let configuracion = agente_de_salud().config().clone();
        assert_eq!(configuracion.timeouts().global, Some(TIEMPO_SALUD));
        assert!(configuracion.http_status_as_error());
        assert!(configuracion.proxy().is_none());
    }

    #[test]
    fn detecta_que_el_servidor_esta_listo() {
        let salud = respuesta_http(200, "OK", "{\"status\":\"ok\"}");
        // `total_slots` es un campo propio de `llama-server`, ausente en cualquier healthcheck
        // genérico: es lo que distingue este caso del de `un_200_en_salud_con_props_ausente...`.
        let props = respuesta_http(
            200,
            "OK",
            "{\"total_slots\":1,\"model_path\":\"devstral.gguf\"}",
        );
        let (puerto, hilo) = servidor_de_prueba_con_secuencia(vec![salud, props]);

        assert_eq!(hay_servidor(puerto, None), EstadoServidor::Listo);

        assert_eq!(
            hilo.join().unwrap(),
            ["GET /health HTTP/1.1", "GET /props HTTP/1.1"]
        );
    }

    #[test]
    fn detecta_que_el_servidor_esta_cargando_el_modelo() {
        let cuerpo = "{\"status\":\"loading model\"}";
        let respuesta = respuesta_http(503, "Service Unavailable", cuerpo);
        let (puerto, hilo) = servidor_de_prueba_que_responde(respuesta);

        // Un 503 en `/health` basta para decir «Cargando»: no hace falta ni tiene sentido llamar
        // a `/props` para confirmar nada mientras el modelo sigue subiendo.
        assert_eq!(hay_servidor(puerto, None), EstadoServidor::Cargando);

        hilo.join().unwrap();
    }

    #[test]
    fn detecta_que_el_puerto_esta_ocupado_por_algo_que_no_es_llama_server() {
        let respuesta = "esto no es HTTP en absoluto\r\n\r\n".to_string();
        let (puerto, hilo) = servidor_de_prueba_que_responde(respuesta);

        assert_eq!(hay_servidor(puerto, None), EstadoServidor::NoDisponible);

        hilo.join().unwrap();
    }

    /// El caso que encontró la revisión: un healthcheck genérico (Kubernetes, Docker, cualquier
    /// framework web) que contesta 200 en `/health` con el mismo cuerpo que un `llama-server`, pero
    /// no tiene `/props` en absoluto. Sin la confirmación contra `/props`, esto se habría colado
    /// como `Listo`.
    #[test]
    fn reconoce_llama_server_aunque_props_traiga_solo_uno_de_los_campos_propios() {
        // No se ha podido confirmar contra un binario real cuál de los campos de `/props` trae cada
        // versión de `llama-server`. Exigir uno concreto convierte esa incertidumbre en un fallo
        // silencioso y permanente: Programator nunca daría por listo a su propio motor e intentaría
        // arrancar otro en un puerto ocupado, en cada ciclo, para siempre. Basta con reconocer
        // cualquiera de los campos característicos: ninguno de ellos aparece en un healthcheck
        // genérico, que es de lo que hay que distinguirse.
        for props in [
            "{\"total_slots\":1}",
            "{\"model_path\":\"devstral.gguf\"}",
            "{\"default_generation_settings\":{}}",
            "{\"chat_template\":\"...\"}",
        ] {
            let salud = respuesta_http(200, "OK", "{\"status\":\"ok\"}");
            let (puerto, hilo) =
                servidor_de_prueba_con_secuencia(vec![salud, respuesta_http(200, "OK", props)]);

            assert_eq!(
                hay_servidor(puerto, None),
                EstadoServidor::Listo,
                "props con {props} debería bastar para reconocer a llama-server"
            );

            hilo.join().unwrap();
        }
    }

    #[test]
    fn un_200_en_salud_con_props_ausente_no_es_llama_server() {
        let salud = respuesta_http(200, "OK", "{\"status\":\"ok\"}");
        let props_ausente = respuesta_http(404, "Not Found", "");
        let (puerto, hilo) = servidor_de_prueba_con_secuencia(vec![salud, props_ausente]);

        assert_eq!(hay_servidor(puerto, None), EstadoServidor::NoDisponible);

        hilo.join().unwrap();
    }

    /// Variante del caso anterior: `/props` existe y contesta 200, pero con un JSON cualquiera que
    /// no trae ninguno de los campos propios de `llama.cpp`. Sigue sin ser un `llama-server`.
    #[test]
    fn un_200_en_salud_con_props_de_otro_json_no_es_llama_server() {
        let salud = respuesta_http(200, "OK", "{\"status\":\"ok\"}");
        let props_ajeno = respuesta_http(200, "OK", "{\"status\":\"healthy\",\"version\":\"1.0\"}");
        let (puerto, hilo) = servidor_de_prueba_con_secuencia(vec![salud, props_ajeno]);

        assert_eq!(hay_servidor(puerto, None), EstadoServidor::NoDisponible);

        hilo.join().unwrap();
    }

    /// Como `servidor_de_prueba_con_secuencia`, pero guarda cada petición entera, con sus
    /// cabeceras, para ver qué clave llevó.
    fn servidor_que_guarda_las_peticiones(
        respuestas: Vec<String>,
    ) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};

        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        let hilo = std::thread::spawn(move || {
            let (mut conexion, _) = escucha.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let mut peticiones = Vec::new();
            for respuesta in respuestas {
                let leidos = conexion.read(&mut buffer).unwrap_or(0);
                peticiones.push(String::from_utf8_lossy(&buffer[..leidos]).to_string());
                let _ = conexion.write_all(respuesta.as_bytes());
            }
            while matches!(conexion.read(&mut buffer), Ok(n) if n > 0) {}
            peticiones
        });
        (puerto, hilo)
    }

    #[test]
    fn las_propiedades_mandan_la_clave() {
        let salud = respuesta_http(200, "OK", "{\"status\":\"ok\"}");
        let props = respuesta_http(200, "OK", "{\"total_slots\":1}");
        let (puerto, hilo) = servidor_que_guarda_las_peticiones(vec![salud, props]);

        let clave = ClaveDelMotor::de_texto("clave-de-prueba");
        assert_eq!(hay_servidor(puerto, Some(&clave)), EstadoServidor::Listo);

        let peticiones = hilo.join().unwrap();
        let de_props = peticiones[1].to_lowercase();
        assert!(de_props.starts_with("get /props"), "{de_props}");
        assert!(
            de_props.contains("authorization: bearer clave-de-prueba"),
            "{de_props}"
        );
    }

    /// `llama-server` contesta 401 en `/props` cuando la clave falta o es otra; `/health` es
    /// pública y contesta 200 igual.
    #[test]
    fn un_401_en_las_propiedades_es_que_rechaza_la_clave() {
        let salud = respuesta_http(200, "OK", "{\"status\":\"ok\"}");
        let rechazo = respuesta_http(
            401,
            "Unauthorized",
            "{\"error\":{\"message\":\"Invalid API Key\",\"type\":\"authentication_error\",\"code\":401}}",
        );
        let (puerto, hilo) = servidor_de_prueba_con_secuencia(vec![salud, rechazo]);

        let clave = ClaveDelMotor::de_texto("otra");
        assert_eq!(
            hay_servidor(puerto, Some(&clave)),
            EstadoServidor::RechazaLaClave
        );

        hilo.join().unwrap();
    }

    #[test]
    fn detecta_que_no_hay_nadie_escuchando() {
        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        drop(escucha);

        assert_eq!(hay_servidor(puerto, None), EstadoServidor::NoDisponible);
    }

    #[test]
    fn un_servidor_que_acepta_la_conexion_y_no_contesta_nunca_no_cuelga_hay_servidor() {
        use std::io::Read;

        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();

        let hilo = std::thread::spawn(move || {
            let (mut conexion, _) = escucha.accept().unwrap();
            // No escribe nunca una respuesta: se limita a leer hasta que el cliente cierre por su
            // cuenta, al agotarse su plazo de espera.
            let mut buffer = [0u8; 4096];
            loop {
                match conexion.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        });

        let inicio = std::time::Instant::now();
        let estado = hay_servidor(puerto, None);
        let transcurrido = inicio.elapsed();

        assert_eq!(estado, EstadoServidor::NoDisponible);
        assert!(
            transcurrido < std::time::Duration::from_secs(5),
            "hay_servidor tardó {transcurrido:?}: el plazo corto de /health no se respetó"
        );

        hilo.join().unwrap();
    }
}
