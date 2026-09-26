//! Carga de `programator.toml`. Las rutas relativas se resuelven contra el directorio del fichero
//! de configuración, nunca contra el directorio de trabajo del proceso: una aplicación portable que
//! depende de desde dónde la lanzas no es portable.

use crate::error::{Error, Resultado};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Cuántas capas del modelo se piden a la GPU.
///
/// `"auto"` deja que Programator lo calcule a partir de la VRAM libre y del peso real del modelo;
/// un número se transcribe a `--n-gpu-layers` **sin discutir**. El Director pidió máximo control
/// sobre el motor, y eso incluye el derecho a equivocarse a sabiendas sobre su propia tarjeta: lo
/// que está escrito en el TOML es lo que recibe `llama-server`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapasGpu {
    #[default]
    Auto,
    Fijas(u32),
}

impl<'de> Deserialize<'de> for CapasGpu {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Crudo {
            Numero(u32),
            Palabra(String),
        }

        // Solo intercepta el error de aquí: cuando el valor no es ni número ni cadena (un
        // decimal, por ejemplo), serde agota las variantes de Crudo y contesta con el nombre de
        // este enum auxiliar, en inglés, en un mensaje que llega tal cual al Director cuando
        // edita el TOML a mano. El caso de la palabra que no es «auto» tiene su propio mensaje
        // más abajo, y no pasa por este `map_err`: Crudo sí se deserializa con éxito como
        // `Palabra`, así que el error no ocurre aquí.
        let crudo = Crudo::deserialize(d).map_err(|_| {
            serde::de::Error::custom(
                "capas_gpu solo admite un número entero de capas o la palabra «auto»",
            )
        })?;

        match crudo {
            Crudo::Numero(n) => Ok(CapasGpu::Fijas(n)),
            Crudo::Palabra(p) if p.eq_ignore_ascii_case("auto") => Ok(CapasGpu::Auto),
            Crudo::Palabra(p) => Err(serde::de::Error::custom(format!(
                "«{p}» no vale para capas_gpu: escribe un número o la palabra «auto»"
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub carpeta: Carpeta,
    #[serde(default)]
    pub agente: Agente,
    #[serde(default)]
    pub verificacion: Verificacion,
    pub motor: MotorConfig,
    #[serde(default)]
    pub ciclo: Ciclo,
    #[serde(default)]
    pub poda: Poda,
    #[serde(default)]
    pub muestreo: Muestreo,
    #[serde(skip)]
    base: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct Carpeta {
    /// Si falta, Programator la pide al arrancar con un diálogo nativo.
    pub ruta: Option<String>,
    /// Si se pregunta la carpeta aunque se recuerde en cuál se trabajó la última vez.
    /// Por defecto: `true`.
    #[serde(default = "preguntar_siempre_por_defecto")]
    pub preguntar_siempre: bool,
}

impl Default for Carpeta {
    fn default() -> Self {
        Self {
            ruta: None,
            preguntar_siempre: preguntar_siempre_por_defecto(),
        }
    }
}

/// Única fuente de si se pregunta la carpeta habiendo memoria: `true`.
///
/// Se elige preguntar porque es lo que menos sorprende a quien está delante: abrir el diálogo y
/// equivocarse de carpeta se arregla en un clic, mientras que arrancar en silencio sobre el proyecto
/// de ayer se descubre tarde y mal. Quien automatice el arranque no pierde nada: `--ruta` y la clave
/// `ruta` de este mismo apartado siguen mandando sobre esto, y son las dos formas de arrancar sin
/// nadie delante.
pub(crate) const fn preguntar_siempre_por_defecto() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct Agente {
    pub nombre: String,
    pub buzon: String,
    pub plantilla_normas: String,
    /// El texto que se le da al modelo como instrucciones de sistema, en cada encargo.
    ///
    /// **No son las instrucciones completas, y esa es la gracia.** Un documento de protocolo de
    /// equipo ocupa unos 8.000 tokens, que en la ventana por defecto es la mitad: un texto que el
    /// modelo relee entero cada vez y del que la mayor parte no le aplica porque no dispone de esos
    /// verbos. El preámbulo trae lo que necesita para actuar en unos 1.000 tokens, y las
    /// instrucciones completas se quedan en `PROGRAMATOR.md`, que el modelo puede leer con
    /// `leer_fichero` cuando le haga falta.
    ///
    /// Si el fichero no existe, se usan las normas completas: quien no lo tenga no nota nada.
    #[serde(default = "preambulo_por_defecto")]
    pub preambulo: String,
    /// Lo que se le recuerda **pegado al encargo**, no en las instrucciones de sistema.
    ///
    /// **La diferencia no es de estilo: se midió el 22/09/2026 y es enorme.** Con las normas de
    /// código en el mensaje de sistema, el modelo las lee —se comprobó preguntándole por ellas— y
    /// escribe igual que si no las tuviera: 1 de 6 comprobaciones mecánicas. Con el mismo texto
    /// pegado al final del encargo, 5 de 6; y pidiéndole además que declare cómo cumplió cada
    /// punto, 6 de 6.
    ///
    /// Si el fichero no existe, no se añade nada y el encargo va como iba.
    #[serde(default = "recordatorio_por_defecto")]
    pub recordatorio: String,
    /// La guía que el arnés deja en el canal para que los demás agentes sepan qué encargarle.
    ///
    /// La publica el arnés, no el modelo: si dependiera de que el modelo se acordara de declarar
    /// sus límites, no sería fiable, que es justo lo que se necesita de un aviso de límites.
    #[serde(default = "guia_por_defecto")]
    pub guia: String,
    /// El fragmento con las cifras de desempeño medidas que se interpola en la guía. Por defecto:
    /// "plantillas/desempeno-medido.md".
    #[serde(default = "desempeno_por_defecto")]
    pub desempeno: String,
    /// Prefijos de ficheros del canal que no son buzones de nadie: los históricos que genera este
    /// proyecto al archivar. Vacío deja los de por defecto de Programator: `"historico-"` y
    /// `"histórico-"`.
    ///
    /// **Va a configuración y no incrustado porque el nombre del histórico lo elige el proyecto
    /// anfitrión, no el arnés.** Medido el 23/09/2026: sin este filtro, archivar un encargo lo
    /// vuelve a encargar, porque el histórico se queda dentro de la carpeta que el arnés sondea y
    /// conserva su encabezado `## Para Programator`.
    #[serde(default)]
    pub prefijos_ignorados: Vec<String>,
}

fn guia_por_defecto() -> String {
    "plantillas/como-encargar-a-programator.md".to_string()
}

fn desempeno_por_defecto() -> String {
    "plantillas/desempeno-medido.md".to_string()
}

fn recordatorio_por_defecto() -> String {
    "plantillas/recordatorio-de-encargo.md".to_string()
}

fn preambulo_por_defecto() -> String {
    "plantillas/preambulo-del-modelo.md".to_string()
}

impl Default for Agente {
    fn default() -> Self {
        Self {
            nombre: "Programator".to_string(),
            buzon: ".gestor/canal/programator.md".to_string(),
            plantilla_normas: "plantillas/instrucciones-del-proyecto.md".to_string(),
            preambulo: preambulo_por_defecto(),
            recordatorio: recordatorio_por_defecto(),
            guia: guia_por_defecto(),
            desempeno: desempeno_por_defecto(),
            prefijos_ignorados: Vec::new(),
        }
    }
}

/// Con qué se comprueba cada propuesta que escribe el modelo, antes de que nadie se fíe de ella.
///
/// **Por qué es configuración y no código.** En la evaluación del 22/09/2026, dos de las siete
/// entregas del modelo no ejecutaban —un Lua con una `ñ` en un identificador, un SQL con un alias
/// de ventana mal usado— y en los dos casos el modelo publicó que había entregado la solución.
/// Comprobarlo exige un compilador, y cada máquina tiene los que tiene: incrustar una lista en el
/// código la rompería en la máquina siguiente. Además, así **el modelo no elige qué se ejecuta**:
/// las órdenes las escribe una persona, y él solo pone el nombre del fichero, que ya está
/// confinado.
///
/// Sin esta sección no se verifica nada y el comportamiento es el de siempre.
#[derive(Debug, Deserialize)]
pub struct Verificacion {
    /// Extensión (sin punto) → la orden que la comprueba. `{fichero}` se sustituye por la ruta.
    ///
    /// En su propia tabla y no al mismo nivel que `tiempo_maximo_segundos`: mezclar un número
    /// suelto con un mapa de listas obliga a serde a adivinar, y el TOML se lee mejor con la
    /// separación explícita.
    #[serde(default)]
    pub comprobadores: std::collections::BTreeMap<String, Vec<String>>,
    /// Cuánto se espera a un comprobador antes de cortarlo. Un comprobador colgado no puede colgar
    /// un arnés pensado para vivir días.
    #[serde(default = "tiempo_verificacion_por_defecto")]
    pub tiempo_maximo_segundos: u64,
}

/// Cuánto se espera por defecto a un comprobador antes de cortarlo: 30 segundos.
/// Única fuente de verdad compartida con el arnés (`TIEMPO_VERIFICACION_POR_DEFECTO`).
pub(crate) const fn tiempo_verificacion_por_defecto() -> u64 {
    30
}

impl Default for Verificacion {
    /// Sin comprobadores, pero con el tiempo máximo puesto: un `Default` derivado dejaría el tope
    /// en cero segundos, que cortaría cualquier comprobación nada más lanzarla.
    fn default() -> Self {
        Self {
            comprobadores: std::collections::BTreeMap::new(),
            tiempo_maximo_segundos: tiempo_verificacion_por_defecto(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MotorConfig {
    pub binario: Option<String>,
    pub modelo: Option<String>,
    #[serde(default = "puerto_por_defecto")]
    pub puerto: u16,
    #[serde(default = "contexto_por_defecto")]
    pub contexto: u32,
    #[serde(default)]
    pub capas_gpu: CapasGpu,
    /// VRAM que **no** se reparte en capas: cubre el contexto de CUDA, los búferes de cómputo y el
    /// desfase entre el presupuesto que ve Programator y el que verá `llama-server`, que es otro
    /// proceso. Conservador a propósito: quedarse corto cuesta unas capas en CPU, que se nota como
    /// lentitud; pasarse cuesta un arranque abortado o una paginación de VRAM que parece un cuelgue.
    #[serde(default = "margen_por_defecto")]
    pub margen_vram_mib: u64,
    #[serde(default = "cache_por_defecto")]
    pub cache_kv: String,
    /// Cuántas conversaciones simultáneas reserva el motor (`--parallel`).
    ///
    /// **Uno, y no el valor por defecto del motor.** Programator atiende los encargos en serie:
    /// `ejecutar_pasada` los recorre de uno en uno y nunca lanza dos peticiones a la vez. Dejar que
    /// `llama-server` aplique su valor por defecto —cuatro— reserva cuatro veces la caché KV que se
    /// va a usar, y esa VRAM es justo la que echa las últimas capas del modelo a la CPU. Medido el
    /// 22/09/2026 sobre el Devstral: 9,8 tokens por segundo con cuatro secuencias contra 23,0 con
    /// una.
    #[serde(default = "paralelo_por_defecto")]
    pub paralelo: u32,
    /// A partir de qué porcentaje de la ventana se avisa de que la conversación va justa. Por
    /// defecto: 70.
    #[serde(default = "umbral_contexto_por_defecto")]
    pub umbral_aviso_contexto: u32,
    /// Cuántos bytes se cuentan por token al estimar la ocupación. Por defecto: 4, que es lo
    /// habitual en texto latino con este tokenizador. Es una aproximación y el arnés lo dice.
    #[serde(default = "bytes_por_token_por_defecto")]
    pub bytes_por_token: u32,
    /// Tiempo máximo en segundos esperando los primeros bytes de respuesta del motor.
    /// Por defecto: 600 segundos (10 minutos), holgado para no cortar respuestas legítimas.
    #[serde(default = "tiempo_lectura_motor_segundos_por_defecto")]
    pub tiempo_lectura_segundos: u64,
    /// Tiempo máximo en segundos para escribir la petición HTTP en el socket del motor.
    /// Por defecto: 60 segundos.
    #[serde(default = "tiempo_escritura_motor_segundos_por_defecto")]
    pub tiempo_escritura_segundos: u64,
}

fn puerto_por_defecto() -> u16 {
    8080
}
fn contexto_por_defecto() -> u32 {
    // 16384 y no 32768: medido el 22/09/2026, con 16k caben las cuarenta capas del Devstral y la
    // generación sube a 23,0 tokens por segundo, contra 15,4 con el contexto doble. Las normas de
    // la casa ocupan unos ocho mil tokens, así que sigue sobrando sitio para un encargo y su
    // respuesta. Quien necesite más lo sube sabiendo lo que cuesta.
    16384
}
fn paralelo_por_defecto() -> u32 {
    1
}
fn margen_por_defecto() -> u64 {
    1024
}
fn cache_por_defecto() -> String {
    "q8_0".to_string()
}
fn umbral_contexto_por_defecto() -> u32 {
    70
}
fn bytes_por_token_por_defecto() -> u32 {
    4
}
/// Tiempo máximo de lectura por defecto al comunicarse con el motor: 600 segundos (10 minutos).
pub(crate) const fn tiempo_lectura_motor_segundos_por_defecto() -> u64 {
    600
}
/// Tiempo máximo de escritura por defecto al enviar la petición al motor: 60 segundos.
pub(crate) const fn tiempo_escritura_motor_segundos_por_defecto() -> u64 {
    60
}

#[derive(Debug, Deserialize)]
pub struct Ciclo {
    #[serde(default = "intervalo_por_defecto")]
    pub intervalo_segundos: u64,
    #[serde(default = "max_herramientas_por_defecto")]
    pub max_herramientas_por_encargo: u32,
    #[serde(default = "tope_lectura_por_defecto")]
    pub tope_lectura_bytes: usize,
    /// Nombre del fichero donde se escribe el estado del arnés en cada sondeo.
    /// Por defecto: "latido.json".
    #[serde(default = "fichero_latido_por_defecto")]
    pub fichero_latido: String,
    /// Cuántas solicitudes anteriores se recuerdan para colapsar llamadas idénticas repetidas.
    /// Por defecto: 12.
    #[serde(default = "max_repeticiones_recordadas_por_defecto")]
    pub max_repeticiones_recordadas: usize,
    /// Número máximo de reintentos ante un motor que no responde antes de abortar.
    /// Por defecto: 2.
    #[serde(default = "max_reintentos_motor_por_defecto")]
    pub max_reintentos_motor: u32,
    /// Segundos de espera entre reintentos tras comprobar que el motor sigue vivo.
    /// Por defecto: 5.
    #[serde(default = "espera_reintento_motor_segundos_por_defecto")]
    pub espera_reintento_motor_segundos: u64,
    /// Número máximo de veces seguidas que el modelo puede insistir en una misma herramienta
    /// denegada antes de abortar el encargo.
    /// Por defecto: 2 (Regla de los Dos Intentos del §2.1).
    #[serde(default = "max_denegaciones_seguidas_por_defecto")]
    pub max_denegaciones_seguidas: u32,
}

/// Segundos mínimos entre dos sondeos del canal, se ponga lo que se ponga en el TOML.
const INTERVALO_MINIMO_SEGUNDOS: u64 = 1;

/// Nombre por defecto del fichero donde el arnés escribe su estado en cada sondeo.
pub const FICHERO_LATIDO_POR_DEFECTO: &str = "latido.json";

pub(crate) const fn intervalo_por_defecto() -> u64 {
    180
}
/// Cupo máximo de herramientas por encargo por defecto: 12.
/// Única fuente de verdad compartida con el arnés (`Limites::default`).
pub(crate) const fn max_herramientas_por_defecto() -> u32 {
    12
}

/// Única fuente del tope de repeticiones recordadas por defecto: 12 (coincide con el cupo de
/// herramientas por encargo).
pub(crate) const fn max_repeticiones_recordadas_por_defecto() -> usize {
    12
}
/// Reintentos por defecto ante un fallo del motor: 2 (coherente con la Regla de los Dos Intentos del §2.1).
pub(crate) const fn max_reintentos_motor_por_defecto() -> u32 {
    2
}
/// Espera por defecto entre reintentos del motor: 5 segundos. Proporciona una ventana suficiente
/// para que remita una contención transitoria de GPU (p. ej. renderizado o captura gráfica).
pub(crate) const fn espera_reintento_motor_segundos_por_defecto() -> u64 {
    5
}
/// Denegaciones seguidas toleradas por defecto antes de abortar: 2 (Regla de los Dos Intentos del §2.1).
/// Única fuente de verdad compartida con el arnés (`Limites::default`).
pub(crate) const fn max_denegaciones_seguidas_por_defecto() -> u32 {
    2
}
/// Única fuente del tope de lectura por defecto: 16 KiB, medido (ver
/// `arnes::herramientas::TOPE_LECTURA_POR_DEFECTO`, que se deriva de esta constante en vez de
/// repetir la cifra).
pub(crate) const fn tope_lectura_por_defecto() -> usize {
    16 * 1024
}
fn fichero_latido_por_defecto() -> String {
    FICHERO_LATIDO_POR_DEFECTO.to_string()
}

impl Default for Ciclo {
    fn default() -> Self {
        Self {
            intervalo_segundos: intervalo_por_defecto(),
            max_herramientas_por_encargo: max_herramientas_por_defecto(),
            tope_lectura_bytes: tope_lectura_por_defecto(),
            fichero_latido: fichero_latido_por_defecto(),
            max_repeticiones_recordadas: max_repeticiones_recordadas_por_defecto(),
            max_reintentos_motor: max_reintentos_motor_por_defecto(),
            espera_reintento_motor_segundos: espera_reintento_motor_segundos_por_defecto(),
            max_denegaciones_seguidas: max_denegaciones_seguidas_por_defecto(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Poda {
    /// A partir de cuántas líneas activas conviene proponer la poda de un buzón.
    ///
    /// **Se mide en líneas y no en bytes porque así lo mide el equipo.** El §3.2 del protocolo fija
    /// «un tope orientativo de 80 líneas activas» para las bandejas; contar bytes obligaría a cada
    /// agente a traducir el criterio del otro, y un buzón se lee y se poda por líneas, no por peso.
    #[serde(default = "umbral_por_defecto")]
    pub umbral_lineas: usize,
}

fn umbral_por_defecto() -> usize {
    80
}

impl Default for Poda {
    fn default() -> Self {
        Self {
            umbral_lineas: umbral_por_defecto(),
        }
    }
}

/// Parámetros de muestreo que se mandan al modelo en cada petición.
///
/// Lo que figura aquí es lo que recibe el modelo: no se omite ninguno, para que no queden
/// valores ocultos decididos por el servidor. La semilla es la excepción, y su ausencia
/// significa «que el servidor elija una al azar».
#[derive(Debug, Clone, Deserialize)]
pub struct Muestreo {
    #[serde(default = "temperatura_por_defecto")]
    pub temperatura: f32,
    #[serde(default = "top_k_por_defecto")]
    pub top_k: u32,
    #[serde(default = "top_p_por_defecto")]
    pub top_p: f32,
    #[serde(default = "min_p_por_defecto")]
    pub min_p: f32,
    #[serde(default = "penalizacion_repeticion_por_defecto")]
    pub penalizacion_repeticion: f32,
    /// Semilla fija, para respuestas reproducibles. Ausente: la elige el servidor.
    #[serde(default)]
    pub semilla: Option<i64>,
}

fn temperatura_por_defecto() -> f32 {
    0.2
}
fn top_k_por_defecto() -> u32 {
    40
}
fn top_p_por_defecto() -> f32 {
    0.95
}
fn min_p_por_defecto() -> f32 {
    0.05
}
fn penalizacion_repeticion_por_defecto() -> f32 {
    1.0
}

impl Default for Muestreo {
    fn default() -> Self {
        Self {
            temperatura: temperatura_por_defecto(),
            top_k: top_k_por_defecto(),
            top_p: top_p_por_defecto(),
            min_p: min_p_por_defecto(),
            penalizacion_repeticion: penalizacion_repeticion_por_defecto(),
            semilla: None,
        }
    }
}

impl Config {
    /// Carga la configuración y valida que estén los campos sin valor razonable por defecto.
    pub fn cargar(ruta: &Path) -> Resultado<Self> {
        let texto = std::fs::read_to_string(ruta).map_err(|causa| Error::Lectura {
            ruta: ruta.to_path_buf(),
            causa,
        })?;

        let mut config: Config = toml::from_str(&texto)
            .map_err(|e| Error::Configuracion(format!("{ruta:?} no se pudo interpretar: {e}")))?;

        if config.motor.modelo.is_none() {
            return Err(Error::Configuracion(
                "falta «modelo» en la sección [motor]: sin fichero GGUF no hay nada que arrancar"
                    .to_string(),
            ));
        }

        // Suelo del intervalo de sondeo. Un `0` explícito en el TOML pasaría la carga sin quejarse
        // y el bucle de `main` giraría a máxima velocidad: un núcleo al 100% releyendo el canal
        // entero sin pausa, sobre una unidad de Google Drive. El fichero lo edita el Director a
        // mano, así que el error es plausible y el arnés no puede permitirse confiar en que no
        // ocurra. Un segundo es tan absurdamente corto como se quiera, pero no quema la máquina.
        if config.ciclo.intervalo_segundos < INTERVALO_MINIMO_SEGUNDOS {
            config.ciclo.intervalo_segundos = INTERVALO_MINIMO_SEGUNDOS;
        }

        // Mismo criterio con el paralelismo del motor: un `0` en el TOML es un motor que no
        // atiende a nadie, y `llama-server` lo rechazaría al arrancar con un error que no dice de
        // dónde viene. Se corrige aquí, donde sí se sabe.
        if config.motor.paralelo == 0 {
            config.motor.paralelo = 1;
        }

        config.base = ruta
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        Ok(config)
    }

    /// Resuelve una ruta relativa contra el directorio del fichero de configuración.
    pub fn resolver(&self, relativa: &str) -> PathBuf {
        let p = Path::new(relativa);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.base.join(p)
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Write;

    fn escribir(dir: &std::path::Path, contenido: &str) -> std::path::PathBuf {
        let ruta = dir.join("programator.toml");
        let mut f = std::fs::File::create(&ruta).unwrap();
        f.write_all(contenido.as_bytes()).unwrap();
        ruta
    }

    #[test]
    fn carga_una_configuracion_completa() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[carpeta]
ruta = "C:/proyectos/mmcelt"

[agente]
nombre = "Programator"
buzon = ".gestor/canal/programator.md"
plantilla_normas = "plantillas/instrucciones-del-proyecto.md"

[motor]
binario = "herramientas/llama-server.exe"
modelo = "modelos/devstral.gguf"
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.agente.nombre, "Programator");
        assert_eq!(
            config.motor.puerto, 8080,
            "el puerto debe tener valor por defecto"
        );
        assert_eq!(
            config.motor.contexto, 16384,
            "el contexto por defecto es el que rinde: medido el 22/09/2026, 16k da 23,0 tokens por \
             segundo contra 15,4 con 32k, porque deja sitio para las cuarenta capas"
        );
        assert_eq!(
            config.motor.paralelo, 1,
            "Programator atiende los encargos de uno en uno: reservar caché para más secuencias \
             es VRAM que no se usa y capas que se van a la CPU"
        );
        assert_eq!(config.ciclo.intervalo_segundos, 180);
        assert_eq!(config.ciclo.max_reintentos_motor, 2);
        assert_eq!(config.ciclo.espera_reintento_motor_segundos, 5);
        assert_eq!(config.ciclo.max_denegaciones_seguidas, 2);
        assert_eq!(config.poda.umbral_lineas, 80);
    }

    #[test]
    fn reintentos_motor_se_leen_del_toml() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "m.gguf"

[ciclo]
max_reintentos_motor = 4
espera_reintento_motor_segundos = 10
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.ciclo.max_reintentos_motor, 4);
        assert_eq!(config.ciclo.espera_reintento_motor_segundos, 10);
    }

    #[test]
    fn max_denegaciones_seguidas_se_lee_del_toml() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "m.gguf"

[ciclo]
max_denegaciones_seguidas = 5
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.ciclo.max_denegaciones_seguidas, 5);
    }

    #[test]
    fn sin_seccion_de_verificacion_no_hay_comprobadores_pero_si_tiempo_maximo() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(dir.path(), "[motor]\nmodelo = \"m.gguf\"\n");

        let config = Config::cargar(&ruta).unwrap();

        assert!(config.verificacion.comprobadores.is_empty());
        assert_eq!(
            config.verificacion.tiempo_maximo_segundos, 30,
            "un tope de cero segundos cortaría cualquier comprobación nada más lanzarla"
        );
    }

    #[test]
    fn los_comprobadores_se_leen_del_toml_con_sus_argumentos() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "m.gguf"

[verificacion]
tiempo_maximo_segundos = 15

[verificacion.comprobadores]
py = ["python", "-m", "py_compile", "{fichero}"]
lua = ["luac", "-p", "{fichero}"]
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.verificacion.tiempo_maximo_segundos, 15);
        assert_eq!(
            config.verificacion.comprobadores.get("py").unwrap(),
            &vec![
                "python".to_string(),
                "-m".to_string(),
                "py_compile".to_string(),
                "{fichero}".to_string()
            ]
        );
        assert!(config.verificacion.comprobadores.contains_key("lua"));
    }

    #[test]
    fn el_paralelismo_se_puede_subir_desde_el_toml() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
paralelo = 4
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.motor.paralelo, 4);
    }

    #[test]
    fn un_paralelismo_de_cero_se_eleva_a_uno() {
        // Cero secuencias no es una configuración: es un motor que no atiende a nadie. Se corrige
        // en vez de rechazar el arranque, igual que se hace con el intervalo del ciclo.
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
paralelo = 0
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.motor.paralelo, 1);
    }

    #[test]
    fn un_intervalo_de_cero_se_eleva_al_suelo_para_no_quemar_un_nucleo() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            "[motor]\nmodelo = \"x.gguf\"\n\n[ciclo]\nintervalo_segundos = 0\n",
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(
            config.ciclo.intervalo_segundos, INTERVALO_MINIMO_SEGUNDOS,
            "un 0 en el TOML haría girar el bucle de sondeo a máxima velocidad"
        );
    }

    #[test]
    fn un_intervalo_por_encima_del_suelo_se_respeta_tal_cual() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            "[motor]\nmodelo = \"x.gguf\"\n\n[ciclo]\nintervalo_segundos = 5\n",
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(
            config.ciclo.intervalo_segundos, 5,
            "el suelo no puede pisar un valor que el Director eligió a conciencia"
        );
    }

    #[test]
    fn rechaza_una_configuracion_sin_modelo() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(dir.path(), "[motor]\nbinario = \"x.exe\"\n");

        let fallo = Config::cargar(&ruta).unwrap_err();

        assert!(
            fallo.to_string().contains("modelo"),
            "el error debe nombrar el campo que falta, y decía: {fallo}"
        );
    }

    #[test]
    fn resuelve_las_rutas_relativas_contra_la_base_y_no_contra_el_directorio_actual() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(dir.path(), "[motor]\nmodelo = \"modelos/x.gguf\"\n");
        let config = Config::cargar(&ruta).unwrap();

        let resuelta = config.resolver("modelos/x.gguf");

        assert_eq!(resuelta, dir.path().join("modelos/x.gguf"));
    }

    #[test]
    fn el_muestreo_ausente_toma_los_valores_por_defecto() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(dir.path(), "[motor]\nmodelo = \"modelos/x.gguf\"\n");

        let config = Config::cargar(&ruta).unwrap();

        assert!((config.muestreo.temperatura - 0.2).abs() < f32::EPSILON);
        assert_eq!(config.muestreo.top_k, 40);
        assert!((config.muestreo.top_p - 0.95).abs() < f32::EPSILON);
        assert!((config.muestreo.min_p - 0.05).abs() < f32::EPSILON);
        assert!((config.muestreo.penalizacion_repeticion - 1.0).abs() < f32::EPSILON);
        assert_eq!(config.muestreo.semilla, None);
    }

    #[test]
    fn el_muestreo_se_lee_del_fichero() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "modelos/x.gguf"

[muestreo]
temperatura = 0.7
top_k = 64
semilla = 42
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert!((config.muestreo.temperatura - 0.7).abs() < f32::EPSILON);
        assert_eq!(config.muestreo.top_k, 64);
        assert_eq!(config.muestreo.semilla, Some(42));
        assert!((config.muestreo.top_p - 0.95).abs() < f32::EPSILON);
        assert!((config.muestreo.min_p - 0.05).abs() < f32::EPSILON);
        assert!((config.muestreo.penalizacion_repeticion - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn capas_gpu_acepta_la_palabra_auto() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
capas_gpu = "auto"
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.motor.capas_gpu, CapasGpu::Auto);
        assert_eq!(config.motor.margen_vram_mib, 1024, "margen por defecto");
    }

    #[test]
    fn capas_gpu_acepta_un_numero_y_lo_respeta() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
capas_gpu = 40
margen_vram_mib = 2048
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.motor.capas_gpu, CapasGpu::Fijas(40));
        assert_eq!(config.motor.margen_vram_mib, 2048);
    }

    #[test]
    fn sin_la_clave_el_defecto_es_auto() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
"#,
        );

        assert_eq!(
            Config::cargar(&ruta).unwrap().motor.capas_gpu,
            CapasGpu::Auto
        );
    }

    #[test]
    fn una_palabra_que_no_es_auto_se_rechaza_diciendo_cual_es_la_buena() {
        let dir = tempfile::tempdir().unwrap();
        // «maximas» no puede contener la subcadena «auto»: si la palabra rechazada la llevara
        // (como pasaba con «automatico»), la aserción de más abajo se cumpliría sola —el mensaje
        // haría eco de la entrada— aunque alguien borrara la sugerencia final y el error dejara de
        // decir cuál es la única palabra válida. Con «maximas», el «auto» del mensaje solo puede
        // venir de la sugerencia.
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
capas_gpu = "maximas"
"#,
        );

        let fallo = Config::cargar(&ruta).unwrap_err();

        assert!(
            fallo.to_string().contains("auto"),
            "el error tiene que decir cuál es la única palabra válida: {fallo}"
        );
        assert!(
            fallo.to_string().contains("maximas"),
            "el error también tiene que nombrar la palabra que rechazó: {fallo}"
        );
    }

    #[test]
    fn un_decimal_en_capas_gpu_no_publica_el_nombre_del_enum_auxiliar() {
        // Un decimal no es ni número entero (Crudo::Numero) ni cadena (Crudo::Palabra): serde
        // agota las variantes de Crudo antes de llegar a nuestro match, así que sin intercepción
        // el mensaje sería el genérico de serde con el nombre del enum interno («Crudo») en
        // inglés, en un fichero que el Director edita a mano.
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
capas_gpu = 99.0
"#,
        );

        let fallo = Config::cargar(&ruta).unwrap_err();

        assert!(
            fallo.to_string().contains("auto"),
            "el error tiene que decir las dos formas válidas: {fallo}"
        );
        assert!(
            !fallo.to_string().contains("Crudo"),
            "el error no puede nombrar el enum interno: {fallo}"
        );
    }

    #[test]
    fn un_programator_toml_de_la_version_anterior_sigue_cargando_igual() {
        // La 0.3.0 escribía `capas_gpu = 99`. No puede romperse al actualizar.
        let dir = tempfile::tempdir().unwrap();
        let ruta = escribir(
            dir.path(),
            r#"
[motor]
binario = "herramientas/llama-server.exe"
modelo = "modelos/devstral.gguf"
capas_gpu = 99
cache_kv = "q8_0"
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.motor.capas_gpu, CapasGpu::Fijas(99));
    }

    #[test]
    fn fichero_latido_toma_valor_por_defecto_o_configurado() {
        let dir = tempfile::tempdir().unwrap();
        let ruta_defecto = escribir(
            dir.path(),
            r#"
[motor]
modelo = "m.gguf"
"#,
        );
        let config_defecto = Config::cargar(&ruta_defecto).unwrap();
        assert_eq!(config_defecto.ciclo.fichero_latido, "latido.json");

        let ruta_personalizada = escribir(
            dir.path(),
            r#"
[motor]
modelo = "m.gguf"

[ciclo]
fichero_latido = "estado_arnes.json"
"#,
        );
        let config_personalizada = Config::cargar(&ruta_personalizada).unwrap();
        assert_eq!(
            config_personalizada.ciclo.fichero_latido,
            "estado_arnes.json"
        );
    }
}
