//! Ciclo de vida de `llama-server`.
//!
//! Si ya hay un servidor escuchando en el puerto, se reutiliza y no se mata al terminar: puede ser
//! del Director. El que arranca Programator sí es suyo, y lo cierra al salir.

use crate::config::MotorConfig;
use crate::error::{Error, Resultado};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

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
        super::HOST_LOCAL.into(),
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
        "on".into(),
        "--cache-type-k".into(),
        config.cache_kv.clone(),
        "--cache-type-v".into(),
        config.cache_kv.clone(),
        // Sin esta bandera, la API compatible-OpenAI no emite `tool_calls`: el modelo solo
        // devolvería texto suelto y el repertorio confinado no vería una sola solicitud.
        "--jinja".into(),
    ]
}

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
}

impl std::fmt::Display for EstadoServidor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Listo => write!(f, "el servidor está listo y responde (Listo)"),
            Self::Cargando => write!(f, "el servidor sigue cargando el modelo (Cargando)"),
            Self::NoDisponible => write!(f, "el servidor no está disponible (NoDisponible)"),
        }
    }
}

/// Tiempo máximo de la comprobación de `/health` completa (conexión + envío + respuesta). Corto a
/// propósito: esta función corre en cada arranque del ciclo, así que un `llama-server` que no
/// responda no puede colgar Programator antes de que haya arrancado nada. También cubre, con el
/// mismo plazo, el caso de que no haya nadie escuchando: `ureq` falla ahí mucho antes, al fallar la
/// propia conexión, así que este tiempo en la práctica solo se agota de verdad si algo acepta la
/// conexión y luego se queda callado.
const TIEMPO_SALUD: Duration = Duration::from_secs(2);

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

/// ¿Es `agente` capaz de hablar con un `llama-server` de verdad en `base_url`?
///
/// Solo se llama cuando `/health` ya contestó 200: hace falta descartar todavía el caso de un
/// healthcheck genérico (ver la documentación de `hay_servidor`), así que se pide `GET /props` y se
/// exige que conteste 200 con un cuerpo JSON que traiga `CAMPO_PROPS_LLAMA_SERVER`. Cualquier otra
/// cosa —sin `/props`, con otro código, o con JSON que no sea el de `llama.cpp`— no es un
/// `llama-server`.
fn responde_props_de_llama_server_url(agente: &ureq::Agent, base_url: &str) -> bool {
    let respuesta = match agente.get(&format!("{base_url}/props")).call() {
        Ok(respuesta) => respuesta,
        Err(_) => return false,
    };

    match respuesta.into_json::<serde_json::Value>() {
        Ok(json) => CAMPOS_PROPS_LLAMA_SERVER
            .iter()
            .any(|campo| json.get(campo).is_some()),
        Err(_) => false,
    }
}

/// Comprueba el estado de un servidor en una URL base dada.
pub fn hay_servidor_url(base_url: &str) -> EstadoServidor {
    let base = base_url.trim_end_matches('/');
    let agente = ureq::AgentBuilder::new().timeout(TIEMPO_SALUD).build();

    match agente.get(&format!("{base}/health")).call() {
        Ok(_respuesta) => {
            if responde_props_de_llama_server_url(&agente, base) {
                EstadoServidor::Listo
            } else {
                EstadoServidor::NoDisponible
            }
        }
        Err(ureq::Error::Status(503, _respuesta)) => EstadoServidor::Cargando,
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
pub fn hay_servidor(puerto: u16) -> EstadoServidor {
    hay_servidor_url(&format!("http://127.0.0.1:{puerto}"))
}

/// El hueco donde vive el `Child` mientras dura el servidor: lo comparten `ServidorLocal`, que es
/// su dueño, y el registro global de cierre ordenado, que solo necesita poder alcanzarlo.
type Compartido = Arc<Mutex<Option<Child>>>;

/// Qué servidor arrancado por Programator hay que matar si llega un Ctrl+C antes de que
/// `ServidorLocal` se suelte por las buenas.
///
/// Guarda un `Weak`, nunca un `Arc` fuerte, y esa elección es la que hace que la garantía de tipos
/// de este módulo («un servidor que Programator no arrancó, no se toca nunca») siga intacta pase lo
/// que pase con Ctrl+C: un `Arc` fuerte aquí mantendría vivo el `Mutex<Option<Child>>` de un
/// servidor que ya se soltó por las buenas, y si esta ranura conservara una referencia a él después
/// de que `ServidorLocal::drop` ya lo hubiera matado y vaciado, en el mejor de los casos no haría
/// nada (el hueco ya está vacío) pero seguiría habiendo una lectura de estado ajeno a la instancia
/// que lo posee. Con `Weak`, en cuanto el único `Arc` fuerte —el que vive dentro de
/// `ServidorLocal`— se suelta, `upgrade()` empieza a devolver `None` sin que nadie tenga que
/// acordarse de limpiar esta ranura a mano.
///
/// Vive como estático porque el manejador de Ctrl+C (instalado una sola vez, en
/// `registrar_cierre_ordenado`) se registra antes de que exista ningún `ServidorLocal`, y necesita
/// un sitio fijo desde el que alcanzar al que se arranque después, sin que nadie tenga que
/// pasárselo a mano por en medio de código que no tiene por qué saber que existe.
static REGISTRO_CIERRE: Mutex<Option<Weak<Mutex<Option<Child>>>>> = Mutex::new(None);

/// Apunta el registro de cierre al servidor recién arrancado. Privada a propósito: la única forma
/// de llegar hasta aquí desde fuera del módulo es `ServidorLocal::arrancar`, así que un servidor
/// que Programator no arrancó jamás puede acabar en esta ranura.
fn registrar_para_cierre_ordenado(compartido: &Compartido) {
    let debil = Arc::downgrade(compartido);
    match REGISTRO_CIERRE.lock() {
        Ok(mut ranura) => *ranura = Some(debil),
        Err(envenenado) => *envenenado.into_inner() = Some(debil),
    }
}

/// Saca el `Child`, si sigue ahí, del hueco compartido con quien lo posee. Usada tanto por
/// `ServidorLocal::drop` (el camino normal) como por `manejar_cierre_ordenado` (Ctrl+C): sea cual
/// sea el que llegue primero, deja el hueco vacío y el otro no encuentra nada que matar dos veces.
fn tomar_hijo(compartido: &Compartido) -> Option<Child> {
    match compartido.lock() {
        Ok(mut guardia) => guardia.take(),
        Err(envenenado) => envenenado.into_inner().take(),
    }
}

/// Mata y espera a `hijo`. Si ya había muerto por su cuenta, matarlo falla y no importa: el
/// objetivo es no dejarlo huérfano, no que la llamada a `kill` tenga éxito.
fn matar_y_esperar(mut hijo: Child) {
    let _ = hijo.kill();
    let _ = hijo.wait();
}

/// Lo que ocurre de verdad al recibir Ctrl+C: si Programator arrancó un servidor y sigue vivo
/// (no se soltó ya por las buenas), lo mata y espera a que termine antes de devolver el control.
///
/// Separada de `registrar_cierre_ordenado` para poder probarla llamándola directamente, sin
/// necesidad de disparar una señal real ni de depender de que un proceso solo pueda instalar un
/// manejador de `ctrlc` una vez: eso haría que, en la suite de pruebas, la primera prueba que lo
/// registrara dejara sin poder registrarlo a todas las demás.
fn manejar_cierre_ordenado() {
    let debil = match REGISTRO_CIERRE.lock() {
        Ok(guardia) => guardia.clone(),
        Err(envenenado) => envenenado.into_inner().clone(),
    };

    let Some(debil) = debil else { return };
    let Some(compartido) = debil.upgrade() else {
        return;
    };

    if let Some(hijo) = tomar_hijo(&compartido) {
        matar_y_esperar(hijo);
    }
}

/// Instala el manejador de Ctrl+C, una sola vez por proceso (una segunda llamada falla: así lo hace
/// `ctrlc`, y es la señal de que algo en el arranque de Programator lo está llamando dos veces).
///
/// **Por qué `ctrlc` y no `SetConsoleCtrlHandler` a mano:** esta última es una API de Win32 que solo
/// se puede llamar con `unsafe`, prohibido en todo el proyecto salvo dentro de dependencias ya
/// auditadas; `ctrlc` hace exactamente esa llamada por dentro, con una interfaz seguro y además
/// portable a Unix, por si Programator corre alguna vez fuera de Windows.
///
/// **Qué no cubre esto:** `ctrlc` solo engancha Ctrl+C (y Ctrl+Break) en la consola. Cerrar la
/// ventana de la consola con el ratón, matar el proceso desde el Administrador de tareas, o un
/// apagado/cierre de sesión de Windows mandan `CTRL_CLOSE_EVENT`, `CTRL_LOGOFF_EVENT` o
/// `CTRL_SHUTDOWN_EVENT` (o, en el caso del Administrador de tareas, ninguna señal en absoluto:
/// termina el proceso directamente). `ctrlc` 3.x no engancha esos otros eventos, así que por esos
/// caminos el servidor arrancado por Programator seguiría sobreviviendo con el modelo cargado en
/// VRAM. Queda dicho aquí y en el informe de la tarea: esta solución cierra el camino más común
/// (Ctrl+C en la terminal), no todos los caminos posibles.
pub fn registrar_cierre_ordenado() -> Resultado<()> {
    ctrlc::set_handler(|| {
        manejar_cierre_ordenado();
        // El manejador de `ctrlc` sustituye al de Windows, que por defecto sí termina el proceso;
        // sin esta llamada, Ctrl+C dejaría de cerrar Programator y solo correría esta limpieza.
        std::process::exit(130);
    })
    .map_err(|causa| {
        Error::Configuracion(format!(
            "no se pudo instalar el manejador de Ctrl+C: {causa}"
        ))
    })
}

/// Qué hay instalado del motor en el disco. No dice si está corriendo —eso es `EstadoServidor`—
/// ni si hay uno disponible para esta pasada —eso es `arnes::sesion::EstadoMotor`—: dice qué
/// ficheros hay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotorInstalado {
    /// El ejecutable está y hay una biblioteca de CUDA junto a él.
    ListoConCuda,
    /// El ejecutable está, pero no se ve ninguna biblioteca de CUDA: descargar capas a la GPU no
    /// tendrá efecto, y el síntoma sería «va lentísimo» sin ninguna causa visible.
    SoloCpu,
    /// No está el ejecutable.
    Falta { ruta: PathBuf },
    /// La configuración no declara ningún motor. No lo devuelve `revisar_motor`, que solo mira el
    /// disco: lo fija quien lee la configuración y ve que no hay nada que mirar.
    NoDeclarado,
}

/// Mira qué hay instalado junto al ejecutable del motor.
///
/// **No comprueba las 55 bibliotecas que trae una distribución de `llama.cpp`**: eso ataría el
/// arnés a una versión concreta y se rompería con la siguiente. Comprueba lo que se puede afirmar
/// con certeza y cambia el comportamiento: que el ejecutable exista, y si hay CUDA a su lado.
pub fn revisar_motor(binario: &Path) -> MotorInstalado {
    if !binario.is_file() {
        return MotorInstalado::Falta {
            ruta: binario.to_path_buf(),
        };
    }

    let Some(carpeta) = binario.parent() else {
        return MotorInstalado::SoloCpu;
    };
    let Ok(entradas) = std::fs::read_dir(carpeta) else {
        // La carpeta no se deja listar: no se puede afirmar que haya CUDA, y decir que no la hay
        // es lo conservador.
        return MotorInstalado::SoloCpu;
    };

    for entrada in entradas.flatten() {
        let nombre = entrada.file_name();
        let nombre = nombre.to_string_lossy().to_ascii_lowercase();
        if nombre.starts_with("ggml-cuda") {
            return MotorInstalado::ListoConCuda;
        }
    }
    MotorInstalado::SoloCpu
}

/// Un `llama-server` lanzado por Programator, que se cierra al soltarlo.
#[derive(Debug)]
pub struct ServidorLocal {
    hijo: Compartido,
}

impl ServidorLocal {
    /// Lanza el servidor. Si el binario no existe, falla nombrando la ruta buscada.
    ///
    /// `capas_gpu` es la resolución ya calculada por `encaje::resolver`, no `config.capas_gpu`:
    /// ver la documentación de `linea_de_arranque`.
    pub fn arrancar(
        config: &MotorConfig,
        binario: &Path,
        modelo: &Path,
        capas_gpu: u32,
    ) -> Resultado<Self> {
        if !binario.exists() {
            return Err(Error::Configuracion(format!(
                "no se encontró el binario de llama-server en «{}»",
                binario.display()
            )));
        }

        let argumentos = linea_de_arranque(config, binario, modelo, capas_gpu);
        let hijo = Command::new(&argumentos[0])
            .args(&argumentos[1..])
            .spawn()
            .map_err(|e| {
                Error::Configuracion(format!("no se pudo lanzar «{}»: {e}", binario.display()))
            })?;

        let compartido: Compartido = Arc::new(Mutex::new(Some(hijo)));
        registrar_para_cierre_ordenado(&compartido);

        Ok(Self { hijo: compartido })
    }
}

impl Drop for ServidorLocal {
    fn drop(&mut self) {
        if let Some(hijo) = tomar_hijo(&self.hijo) {
            matar_y_esperar(hijo);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_ejecutable_el_motor_falta() {
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");

        assert_eq!(
            revisar_motor(&binario),
            MotorInstalado::Falta { ruta: binario }
        );
    }

    #[test]
    fn con_ejecutable_y_sin_biblioteca_de_cuda_el_motor_es_solo_cpu() {
        // No es hipotético: el llama-server que instala Docker trae veinticuatro bibliotecas y
        // todas son ggml-cpu-*. Con él, --n-gpu-layers se acepta sin protestar y no carga una
        // sola capa.
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");
        std::fs::write(&binario, b"no importa el contenido").unwrap();
        std::fs::write(dir.path().join("ggml-cpu-haswell.dll"), b"x").unwrap();

        assert_eq!(revisar_motor(&binario), MotorInstalado::SoloCpu);
    }

    #[test]
    fn con_biblioteca_de_cuda_al_lado_el_motor_esta_listo() {
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");
        std::fs::write(&binario, b"x").unwrap();
        std::fs::write(dir.path().join("ggml-cuda.dll"), b"x").unwrap();

        assert_eq!(revisar_motor(&binario), MotorInstalado::ListoConCuda);
    }

    #[test]
    fn el_nombre_de_la_biblioteca_no_distingue_mayusculas() {
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");
        std::fs::write(&binario, b"x").unwrap();
        std::fs::write(dir.path().join("GGML-CUDA.DLL"), b"x").unwrap();

        assert_eq!(revisar_motor(&binario), MotorInstalado::ListoConCuda);
    }

    #[test]
    fn una_ruta_que_es_un_directorio_no_es_el_ejecutable() {
        // `is_file()` distingue de verdad: una carpeta llamada como el ejecutable existe, pero no
        // se puede lanzar. Decir «falta» es lo correcto.
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");
        std::fs::create_dir(&binario).unwrap();

        assert_eq!(
            revisar_motor(&binario),
            MotorInstalado::Falta { ruta: binario }
        );
    }

    fn config_de_prueba() -> crate::config::MotorConfig {
        crate::config::MotorConfig {
            binario: Some("herramientas/llama-server.exe".into()),
            modelo: Some("modelos/devstral.gguf".into()),
            puerto: 8080,
            contexto: 32768,
            capas_gpu: crate::config::CapasGpu::Fijas(99),
            margen_vram_mib: 1024,
            cache_kv: "q8_0".into(),
            paralelo: 1,
            umbral_aviso_contexto: 70,
            bytes_por_token: 4,
            tiempo_lectura_segundos: crate::config::tiempo_lectura_motor_segundos_por_defecto(),
            tiempo_escritura_segundos: crate::config::tiempo_escritura_motor_segundos_por_defecto(),
        }
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
    ) -> (u16, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};

        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();

        let hilo = std::thread::spawn(move || {
            let (mut conexion, _) = escucha.accept().unwrap();
            let mut buffer = [0u8; 4096];

            for respuesta in respuestas {
                let _ = conexion.read(&mut buffer);
                let _ = conexion.write_all(respuesta.as_bytes());
            }

            loop {
                match conexion.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        });

        (puerto, hilo)
    }

    /// Atajo para el caso, muy frecuente en estas pruebas, de una sola conexión con una sola
    /// respuesta.
    fn servidor_de_prueba_que_responde(
        respuesta_http: String,
    ) -> (u16, std::thread::JoinHandle<()>) {
        servidor_de_prueba_con_secuencia(vec![respuesta_http])
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

        assert_eq!(hay_servidor(puerto), EstadoServidor::Listo);

        hilo.join().unwrap();
    }

    #[test]
    fn detecta_que_el_servidor_esta_cargando_el_modelo() {
        let cuerpo = "{\"status\":\"loading model\"}";
        let respuesta = respuesta_http(503, "Service Unavailable", cuerpo);
        let (puerto, hilo) = servidor_de_prueba_que_responde(respuesta);

        // Un 503 en `/health` basta para decir «Cargando»: no hace falta ni tiene sentido llamar
        // a `/props` para confirmar nada mientras el modelo sigue subiendo.
        assert_eq!(hay_servidor(puerto), EstadoServidor::Cargando);

        hilo.join().unwrap();
    }

    #[test]
    fn detecta_que_el_puerto_esta_ocupado_por_algo_que_no_es_llama_server() {
        let respuesta = "esto no es HTTP en absoluto\r\n\r\n".to_string();
        let (puerto, hilo) = servidor_de_prueba_que_responde(respuesta);

        assert_eq!(hay_servidor(puerto), EstadoServidor::NoDisponible);

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
                hay_servidor(puerto),
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

        assert_eq!(hay_servidor(puerto), EstadoServidor::NoDisponible);

        hilo.join().unwrap();
    }

    /// Variante del caso anterior: `/props` existe y contesta 200, pero con un JSON cualquiera que
    /// no trae ninguno de los campos propios de `llama.cpp`. Sigue sin ser un `llama-server`.
    #[test]
    fn un_200_en_salud_con_props_de_otro_json_no_es_llama_server() {
        let salud = respuesta_http(200, "OK", "{\"status\":\"ok\"}");
        let props_ajeno = respuesta_http(200, "OK", "{\"status\":\"healthy\",\"version\":\"1.0\"}");
        let (puerto, hilo) = servidor_de_prueba_con_secuencia(vec![salud, props_ajeno]);

        assert_eq!(hay_servidor(puerto), EstadoServidor::NoDisponible);

        hilo.join().unwrap();
    }

    #[test]
    fn detecta_que_no_hay_nadie_escuchando() {
        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        drop(escucha);

        assert_eq!(hay_servidor(puerto), EstadoServidor::NoDisponible);
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
        let estado = hay_servidor(puerto);
        let transcurrido = inicio.elapsed();

        assert_eq!(estado, EstadoServidor::NoDisponible);
        assert!(
            transcurrido < std::time::Duration::from_secs(5),
            "hay_servidor tardó {transcurrido:?}: el plazo corto de /health no se respetó"
        );

        hilo.join().unwrap();
    }

    #[test]
    fn arrancar_con_un_binario_inexistente_da_un_error_que_nombra_la_ruta() {
        let config = config_de_prueba();

        let fallo = ServidorLocal::arrancar(
            &config,
            std::path::Path::new("no/existe/llama-server.exe"),
            std::path::Path::new("x.gguf"),
            99,
        )
        .unwrap_err();

        assert!(
            fallo.to_string().contains("no/existe"),
            "el error decía: {fallo}"
        );
    }

    /// Best-effort: mata el proceso de prueba por PID aunque la prueba entre en pánico antes de
    /// llegar al final. Sin esto, un `assert!` fallido a mitad de la prueba dejaría un `ping.exe`
    /// de sobra colgado en la máquina.
    struct MataAlSoltar(u32);

    impl Drop for MataAlSoltar {
        fn drop(&mut self) {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &self.0.to_string()])
                .output();
        }
    }

    /// ¿Sigue vivo el proceso con este PID? Se apoya en `tasklist` (siempre presente en Windows)
    /// en vez de en una API insegura, para no tener que escribir FFI a mano.
    #[cfg(windows)]
    fn proceso_vive(pid: u32) -> bool {
        let filtro = format!("PID eq {pid}");
        let salida = std::process::Command::new("tasklist")
            .args(["/FI", &filtro, "/NH"])
            .output();

        match salida {
            Ok(salida) => String::from_utf8_lossy(&salida.stdout).contains(&pid.to_string()),
            Err(_) => false,
        }
    }

    // Los tres casos de `manejar_cierre_ordenado` viven en una sola prueba, a propósito: la
    // función toca `REGISTRO_CIERRE`, que es un estático compartido por todo el binario de
    // pruebas. `cargo test` corre las pruebas en paralelo, así que dos pruebas separadas que
    // registraran ahí cada una lo suyo podrían pisarse la una a la otra (la segunda sobrescribe
    // la ranura antes de que la primera la lea) y producir un fallo que no depende del código,
    // sino de qué hilo gane la carrera. Con todo secuencial en una única prueba, esa carrera no
    // puede darse.
    #[test]
    #[cfg(windows)]
    fn el_cierre_ordenado_distingue_nada_registrado_servidor_ya_suelto_y_servidor_vivo() {
        // Caso 1: nada registrado todavía. No debe entrar en pánico ni fallar.
        manejar_cierre_ordenado();

        // Caso 2: un servidor que ya se soltó por las buenas (su `Drop` ya corrió). El `Weak` no
        // puede subir a `Arc`, así que no hay nada que matar.
        {
            let compartido: Compartido = std::sync::Arc::new(std::sync::Mutex::new(None));
            registrar_para_cierre_ordenado(&compartido);
            drop(compartido);

            manejar_cierre_ordenado();
        }

        // Caso 3: un servidor de verdad, todavía vivo. `ping` a sí mismo 30 veces vive de sobra
        // para dar tiempo a que el cierre ordenado actúe antes de que termine solo.
        let hijo = std::process::Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .spawn()
            .unwrap();
        let pid = hijo.id();
        let _guardia = MataAlSoltar(pid);

        let compartido: Compartido = std::sync::Arc::new(std::sync::Mutex::new(Some(hijo)));
        registrar_para_cierre_ordenado(&compartido);

        manejar_cierre_ordenado();

        assert!(
            compartido.lock().unwrap().is_none(),
            "el hueco compartido debía quedar vacío tras el cierre ordenado"
        );
        assert!(
            !proceso_vive(pid),
            "el proceso registrado debía estar muerto tras el cierre ordenado"
        );
    }
}
