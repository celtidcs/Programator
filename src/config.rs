//! Carga de `programator.toml`. Las rutas relativas se resuelven contra el directorio del fichero
//! de configuración, nunca contra el directorio de trabajo del proceso: una aplicación portable que
//! depende de desde dónde la lanzas no es portable.

use crate::error::{Error, Resultado};
use claves_retiradas::interpretar_sin_claves_retiradas;
use mensajes::{aviso_de_envoltorio_vacio, aviso_de_techo, falta_el_modelo, toml_ininteligible};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub mod atencion_rapida;
mod auto_o_numero;
pub mod clave_del_motor;
mod claves_retiradas;
mod mensajes;
pub mod modelo_por_defecto;
pub mod preparacion;
pub mod reparto;
pub mod soltar_modelo;

pub use atencion_rapida::AtencionRapida;
pub use clave_del_motor::ClaveConfigurada;
pub use preparacion::Preparacion;
pub use reparto::{CapasEnCpu, ModoDeCarga, RepartoDeMemoria};
pub use soltar_modelo::SoltarModelo;

/// Cuántas capas del modelo se piden a la GPU.
///
/// `"auto"` deja que Programator lo calcule a partir de la VRAM libre y del peso real del modelo;
/// un número se transcribe a `--n-gpu-layers` **sin discutir**. Quien lo configura tiene el máximo
/// control sobre el motor, y eso incluye el derecho a equivocarse a sabiendas sobre su propia tarjeta: lo
/// que está escrito en el TOML es lo que recibe `llama-server`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapasGpu {
    #[default]
    Auto,
    Fijas(u32),
}

impl<'de> Deserialize<'de> for CapasGpu {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(auto_o_numero::leer(d, "capas_gpu", "capas")?.map_or(CapasGpu::Auto, CapasGpu::Fijas))
    }
}

/// El fichero de configuración, que vive junto al ejecutable.
pub const NOMBRE_DEL_FICHERO_DE_CONFIGURACION: &str = "programator.toml";

#[derive(Debug, Clone, Deserialize)]
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
    pub muestreo: Muestreo,
    #[serde(default)]
    pub actualizaciones: Actualizaciones,
    /// El registro de lo que Programator propone y en qué queda.
    #[serde(default)]
    pub entregas: Entregas,
    /// Cómo se prepara una propuesta en una copia del proyecto (`preparar:`).
    #[serde(default)]
    pub preparacion: Preparacion,
    /// La revisión mensual de modelos.
    #[serde(default)]
    pub revision: crate::revision::config::ConfigRevision,
    #[serde(skip)]
    base: PathBuf,
    /// Lo que conviene decir al arrancar sobre el propio fichero: claves que ya no valen.
    #[serde(skip)]
    avisos: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct Agente {
    /// Con él se firma y de él sale su buzón: el nombre en minúsculas con «.md».
    #[serde(default = "nombre_por_defecto")]
    pub nombre: String,
    #[serde(default = "plantilla_normas_por_defecto")]
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
    /// El registro de modelos: lo que el banco midió de cada modelo, que la guía cuenta al agente
    /// de pago. Lo escribe `--registro-anotar`. Por defecto: "registro-de-modelos.toml".
    #[serde(default = "registro_de_modelos_por_defecto")]
    pub registro_de_modelos: String,
    /// Si cada encargo deja en `.gestor/<agente>/` la conversación con el modelo, vuelta a vuelta.
    /// Por defecto, no: lleva el contenido de los ficheros del proyecto. El banco de modelos lo
    /// activa siempre, porque sin él no se puede saber por qué un modelo hizo lo que hizo.
    #[serde(default)]
    pub registro_de_conversacion: bool,
    /// El vigilante del canal de Gestor Agentes que trae el paquete. Si el proyecto no tiene uno,
    /// Programator lo deja en `.gestor/vigilante/` al arrancar. Por defecto:
    /// "vigilante/gestor-vigilante.exe" en Windows y "vigilante/gestor-vigilante" en Linux.
    #[serde(default = "vigilante_por_defecto")]
    pub vigilante: String,
}

fn vigilante_por_defecto() -> String {
    format!(
        "{}/{}",
        crate::arranque::vigilante::CARPETA_DEL_VIGILANTE,
        crate::plataforma::NOMBRE_DEL_VIGILANTE
    )
}

/// La plantilla de la guía para el equipo, junto al ejecutable (desde la 0.6.0, 22/09/2026).
pub const GUIA_POR_DEFECTO: &str = "plantillas/como-encargar-a-programator.md";

fn guia_por_defecto() -> String {
    GUIA_POR_DEFECTO.to_string()
}

/// El registro de lo que midió el banco, junto al ejecutable (desde el 12/11/2026).
pub const REGISTRO_DE_MODELOS_POR_DEFECTO: &str = "registro-de-modelos.toml";

fn registro_de_modelos_por_defecto() -> String {
    REGISTRO_DE_MODELOS_POR_DEFECTO.to_string()
}

/// La plantilla del recordatorio que va pegado a cada encargo (desde la 0.6.0, 22/09/2026).
pub const RECORDATORIO_POR_DEFECTO: &str = "plantillas/recordatorio-de-encargo.md";

fn recordatorio_por_defecto() -> String {
    RECORDATORIO_POR_DEFECTO.to_string()
}

/// La plantilla de las instrucciones que ve el modelo (desde la 0.6.0, 22/09/2026).
pub const PREAMBULO_POR_DEFECTO: &str = "plantillas/preambulo-del-modelo.md";

fn preambulo_por_defecto() -> String {
    PREAMBULO_POR_DEFECTO.to_string()
}

/// El nombre con el que Programator se identifica en el canal.
pub const NOMBRE_POR_DEFECTO: &str = "Programator";

fn nombre_por_defecto() -> String {
    NOMBRE_POR_DEFECTO.to_string()
}

/// La plantilla de instrucciones que se instala en un proyecto que no tiene las suyas.
pub const PLANTILLA_NORMAS_POR_DEFECTO: &str = "plantillas/instrucciones-del-proyecto.md";

fn plantilla_normas_por_defecto() -> String {
    PLANTILLA_NORMAS_POR_DEFECTO.to_string()
}

impl Default for Agente {
    fn default() -> Self {
        Self {
            nombre: nombre_por_defecto(),
            plantilla_normas: plantilla_normas_por_defecto(),
            preambulo: preambulo_por_defecto(),
            recordatorio: recordatorio_por_defecto(),
            guia: guia_por_defecto(),
            registro_de_modelos: registro_de_modelos_por_defecto(),
            registro_de_conversacion: false,
            vigilante: vigilante_por_defecto(),
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
#[derive(Debug, Clone, Deserialize)]
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
    /// Si no hay comprobador de `py`, buscar un Python y comprobar con él. Por defecto, sí.
    #[serde(default = "comprobar_python_por_defecto")]
    pub comprobar_python: bool,
    /// La ruta del Python con que comprobar; vacía, se busca en el PATH.
    #[serde(default)]
    pub python: String,
    /// Cuántos caracteres de lo que dice un comprobador que rechaza se le pasan al modelo.
    #[serde(default = "maximo_salida_al_modelo_por_defecto")]
    pub maximo_salida_al_modelo: usize,
}

/// Lo que ve el modelo de un comprobador que rechaza, si el TOML no dice otra cosa: 1200
/// caracteres. Un error de `rustc` puede ocupar pantallas enteras y el contexto es finito; con esto
/// sobra para saber qué falló y dónde. Es la cifra que el arnés usaba fija hasta la 1.1.0
/// y está en el TOML para quien use otro modelo u otro contexto.
pub const MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO: usize = 1200;

const fn maximo_salida_al_modelo_por_defecto() -> usize {
    MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO
}

/// Por defecto se busca Python.
const fn comprobar_python_por_defecto() -> bool {
    true
}

/// Cuánto se espera por defecto a un comprobador antes de cortarlo: 30 segundos.
/// Única fuente de verdad compartida con el arnés (`TIEMPO_VERIFICACION_POR_DEFECTO`).
pub const TIEMPO_VERIFICACION_POR_DEFECTO_SEGUNDOS: u64 = 30;

pub(crate) const fn tiempo_verificacion_por_defecto() -> u64 {
    TIEMPO_VERIFICACION_POR_DEFECTO_SEGUNDOS
}

impl Default for Verificacion {
    /// Sin comprobadores, pero con el tiempo máximo puesto: un `Default` derivado dejaría el tope
    /// en cero segundos, que cortaría cualquier comprobación nada más lanzarla.
    fn default() -> Self {
        Self {
            comprobadores: std::collections::BTreeMap::new(),
            tiempo_maximo_segundos: tiempo_verificacion_por_defecto(),
            comprobar_python: comprobar_python_por_defecto(),
            python: String::new(),
            maximo_salida_al_modelo: MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MotorConfig {
    pub binario: Option<String>,
    pub modelo: Option<String>,
    #[serde(default = "puerto_por_defecto")]
    pub puerto: u16,
    #[serde(default = "contexto_por_defecto")]
    pub contexto: u32,
    #[serde(default)]
    pub capas_gpu: CapasGpu,
    /// VRAM que **no** se reparte en capas: cubre el contexto de CUDA, los búferes de cómputo y lo
    /// que cambie en la tarjeta entre la medida y la carga. Por defecto,
    /// [`MARGEN_VRAM_POR_DEFECTO_MIB`]. Quedarse corto cuesta unas capas en CPU, que se nota como
    /// lentitud; pasarse cuesta un arranque abortado o una paginación de VRAM que parece un cuelgue.
    #[serde(default = "margen_por_defecto")]
    pub margen_vram_mib: u64,
    #[serde(default = "cache_por_defecto")]
    pub cache_kv: String,
    /// La atención rápida del motor (`--flash-attn`). Por defecto, `"on"`.
    #[serde(default, rename = "flash_attn")]
    pub atencion_rapida: AtencionRapida,
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
    /// Si cada conversación nueva se calcula sin la caché de prefijos de `llama-server`. Con la
    /// caché, la respuesta depende de lo que se le pidió antes al servidor, y la misma petición deja
    /// de dar lo mismo (`motor::continuidad`). Dentro de una conversación se sigue aprovechando.
    #[serde(default = "empezar_en_frio_por_defecto")]
    pub empezar_en_frio: bool,
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
    /// Cuándo se suelta el modelo de la tarjeta entre encargos. Por defecto, `"auto"`.
    #[serde(default)]
    pub soltar_modelo: SoltarModelo,
    /// Tope de VRAM, en MiB, que el encaje no supera nunca. Sin él, el encaje usa toda la libre.
    #[serde(default)]
    pub vram_maxima_mib: Option<u64>,
    /// Segundos que se espera a que cargue un modelo pedido a mitad de una pasada. Por defecto,
    /// [`ESPERA_DE_CARGA_POR_DEFECTO_SEGUNDOS`].
    #[serde(default = "espera_de_carga_por_defecto")]
    pub espera_de_carga_segundos: u64,
    /// La clave con que se protege el motor. Por defecto, `"auto"`: una nueva en cada arranque.
    #[serde(default)]
    pub clave: ClaveConfigurada,
    /// Qué parte del modelo y de la caché va a la memoria del equipo. Sus claves se escriben
    /// sueltas en `[motor]`, como las demás.
    #[serde(flatten)]
    pub reparto: RepartoDeMemoria,
}

/// Espera por defecto a que cargue un modelo pedido a mitad de una pasada: 300 segundos. Holgado
/// frente a los 20 a 60 segundos que tarda el Devstral de 24B en una 4070 Ti SUPER.
pub const ESPERA_DE_CARGA_POR_DEFECTO_SEGUNDOS: u64 = 300;

pub(crate) const fn espera_de_carga_por_defecto() -> u64 {
    ESPERA_DE_CARGA_POR_DEFECTO_SEGUNDOS
}

impl MotorConfig {
    /// El nombre del modelo configurado: el de su fichero sin carpeta ni extensión, que es como
    /// lo nombran la tanda y el registro.
    pub fn nombre_del_modelo(&self) -> Option<String> {
        self.modelo
            .as_deref()
            .and_then(|ruta| crate::motor::fichero_de_modelo::nombre_de_modelo(Path::new(ruta)))
    }

    /// El tipo de la caché de claves: `cache_k` si se escribió, y si no, `cache_kv`, que es la
    /// clave de la 1.1.0 y sigue dando los dos.
    pub fn cache_k(&self) -> &str {
        self.reparto.cache_k.as_deref().unwrap_or(&self.cache_kv)
    }

    /// El tipo de la caché de valores, con la misma regla que [`MotorConfig::cache_k`].
    pub fn cache_v(&self) -> &str {
        self.reparto.cache_v.as_deref().unwrap_or(&self.cache_kv)
    }
}

/// El puerto de `llama-server` (desde el andamiaje del proyecto, 13/09/2026).
pub const PUERTO_POR_DEFECTO: u16 = 8080;

fn puerto_por_defecto() -> u16 {
    PUERTO_POR_DEFECTO
}
/// La ventana de contexto. 16384 y no 32768: medido el 22/09/2026, con 16k caben las cuarenta capas
/// del Devstral y la generación sube a 23,0 tokens por segundo, contra 15,4 con el contexto doble.
/// Las normas de la casa ocupan unos ocho mil tokens, así que sigue sobrando sitio para un encargo y
/// su respuesta. Quien necesite más lo sube sabiendo lo que cuesta.
pub const CONTEXTO_POR_DEFECTO: u32 = 16384;

fn contexto_por_defecto() -> u32 {
    CONTEXTO_POR_DEFECTO
}
/// Conversaciones en paralelo: una, con la ventana de contexto entera para ella (desde la 0.6.0, 22/09/2026).
pub const PARALELO_POR_DEFECTO: u32 = 1;

fn paralelo_por_defecto() -> u32 {
    PARALELO_POR_DEFECTO
}
/// Activado: es lo que hace que la misma prueba dé el mismo resultado. Cuesta reprocesar las instrucciones y el encargo una vez por conversación.
pub fn empezar_en_frio_por_defecto() -> bool {
    true
}
/// Margen de VRAM por defecto, en MiB. Fue 1024 mientras la medida de Windows (DXGI) exageraba la
/// memoria libre en alrededor de 1 GiB. Desde el 12/11/2026 se mide toda la tarjeta, y el encaje
/// queda solo entre 100 y 170 MiB por debajo de lo que ocupa de verdad `llama-server` (tanda del
/// banco 2026-11-12-1219). Por eso se bajó a 512 ese mismo día.
pub const MARGEN_VRAM_POR_DEFECTO_MIB: u64 = 512;

fn margen_por_defecto() -> u64 {
    MARGEN_VRAM_POR_DEFECTO_MIB
}
/// El tipo del caché KV: la mitad de memoria que `f16` (desde el andamiaje del proyecto, 13/09/2026).
pub const CACHE_KV_POR_DEFECTO: &str = "q8_0";

fn cache_por_defecto() -> String {
    CACHE_KV_POR_DEFECTO.to_string()
}
/// Porcentaje de la ventana de contexto a partir del que se avisa de que se va a desbordar (25/09/2026).
pub const UMBRAL_AVISO_CONTEXTO_POR_DEFECTO_PORCENTAJE: u32 = 70;

fn umbral_contexto_por_defecto() -> u32 {
    UMBRAL_AVISO_CONTEXTO_POR_DEFECTO_PORCENTAJE
}
/// Bytes que se cuentan por token al estimar la ocupación: lo habitual en texto latino (25/09/2026).
pub const BYTES_POR_TOKEN_POR_DEFECTO: u32 = 4;

fn bytes_por_token_por_defecto() -> u32 {
    BYTES_POR_TOKEN_POR_DEFECTO
}
/// Tiempo máximo de lectura por defecto al comunicarse con el motor: 600 segundos (10 minutos).
pub const TIEMPO_LECTURA_MOTOR_POR_DEFECTO_SEGUNDOS: u64 = 600;

pub(crate) const fn tiempo_lectura_motor_segundos_por_defecto() -> u64 {
    TIEMPO_LECTURA_MOTOR_POR_DEFECTO_SEGUNDOS
}
/// Tiempo máximo de escritura por defecto al enviar la petición al motor: 60 segundos.
pub const TIEMPO_ESCRITURA_MOTOR_POR_DEFECTO_SEGUNDOS: u64 = 60;

pub(crate) const fn tiempo_escritura_motor_segundos_por_defecto() -> u64 {
    TIEMPO_ESCRITURA_MOTOR_POR_DEFECTO_SEGUNDOS
}

#[derive(Debug, Clone, Deserialize)]
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
    /// Por defecto: 2 (Regla de los Dos Intentos).
    #[serde(default = "max_denegaciones_seguidas_por_defecto")]
    pub max_denegaciones_seguidas: u32,
    /// Tras cuántos intentos fallidos se cierra un encargo de la cola de pendientes y se le
    /// responde a quien lo pidió con el motivo. Por defecto, [`INTENTOS_POR_ENCARGO_POR_DEFECTO`].
    #[serde(default = "intentos_por_encargo_por_defecto")]
    pub intentos_por_encargo: u32,
    /// Cuántas propuestas anteriores se le cuentan al modelo en cada encargo, en el apartado «Lo
    /// que ya entregaste en este proyecto». Con 0 no sale. Por defecto,
    /// [`ENTREGAS_EN_CONTEXTO_POR_DEFECTO`].
    #[serde(default = "entregas_en_contexto_por_defecto")]
    pub entregas_en_contexto: usize,
    /// Cuántos minutos tiene que faltar de su buzón un encargo ya atendido para que Programator lo
    /// olvide. Por defecto, [`OLVIDAR_LO_CERRADO_POR_DEFECTO_MINUTOS`].
    #[serde(default = "olvidar_lo_cerrado_por_defecto")]
    pub olvidar_lo_cerrado_tras_minutos: u64,
    /// Cuántos milisegundos tiene que llevar quieto un buzón con novedades para leerlas. Por
    /// defecto, [`BUZON_ESTABLE_POR_DEFECTO_MS`].
    #[serde(default = "buzon_estable_por_defecto")]
    pub buzon_estable_tras_ms: u64,
}

/// Un segundo y medio: quien escribe su buzón por trozos (un editor que guarda, una
/// sincronización) termina en mucho menos, y es poco al lado del intervalo entre vueltas. Sin
/// esta espera, un buzón leído a medio escribir daba un encargo cortado que se atendía como
/// nuevo.
pub const BUZON_ESTABLE_POR_DEFECTO_MS: u64 = 1500;

pub(crate) const fn buzon_estable_por_defecto() -> u64 {
    BUZON_ESTABLE_POR_DEFECTO_MS
}

/// Una hora: de sobra para que un buzón leído vacío o a medias mientras su dueño lo reescribe se
/// lea entero otra vez, y poco para lo que pesa una ficha.
pub const OLVIDAR_LO_CERRADO_POR_DEFECTO_MINUTOS: u64 = 60;

pub(crate) const fn olvidar_lo_cerrado_por_defecto() -> u64 {
    OLVIDAR_LO_CERRADO_POR_DEFECTO_MINUTOS
}

/// Veinte líneas cortas: unos dos mil bytes, poco frente a la ventana, y bastan para lo de las
/// últimas semanas de un proyecto.
pub const ENTREGAS_EN_CONTEXTO_POR_DEFECTO: usize = 20;

pub(crate) const fn entregas_en_contexto_por_defecto() -> usize {
    ENTREGAS_EN_CONTEXTO_POR_DEFECTO
}

/// Intentos fallidos tras los que se cierra un encargo: 2. Uno puede ser mala suerte; el segundo
/// ya es algo que alguien tiene que mirar.
pub const INTENTOS_POR_ENCARGO_POR_DEFECTO: u32 = 2;

pub(crate) const fn intentos_por_encargo_por_defecto() -> u32 {
    INTENTOS_POR_ENCARGO_POR_DEFECTO
}

/// Segundos mínimos entre dos sondeos del canal, se ponga lo que se ponga en el TOML.
const INTERVALO_MINIMO_SEGUNDOS: u64 = 1;

/// Techo de los tiempos en segundos del TOML que acaban sumados a la hora: un día. Más no tiene
/// uso, y un valor enorme haría entrar en pánico esas sumas.
const TECHO_DE_SEGUNDOS: u64 = 86_400;

/// Nombre por defecto del fichero donde el arnés escribe su estado en cada sondeo.
pub const FICHERO_LATIDO_POR_DEFECTO: &str = "latido.json";

/// Segundos entre dos sondeos del canal si el TOML no dice otra cosa. Leer el canal no gasta
/// tokens de nadie y es barato; con tres minutos, quien encargaba esperaba de media minuto y medio
/// antes de que Programator viera el encargo.
const INTERVALO_POR_DEFECTO_SEGUNDOS: u64 = 30;

pub(crate) const fn intervalo_por_defecto() -> u64 {
    INTERVALO_POR_DEFECTO_SEGUNDOS
}
/// Cupo máximo de herramientas por encargo por defecto: 12.
/// Única fuente de verdad compartida con el arnés (`Limites::default`).
pub const MAX_HERRAMIENTAS_POR_DEFECTO: u32 = 12;

pub(crate) const fn max_herramientas_por_defecto() -> u32 {
    MAX_HERRAMIENTAS_POR_DEFECTO
}

/// Única fuente del tope de repeticiones recordadas por defecto: 12 (coincide con el cupo de
/// herramientas por encargo).
pub const MAX_REPETICIONES_RECORDADAS_POR_DEFECTO: usize = 12;

pub(crate) const fn max_repeticiones_recordadas_por_defecto() -> usize {
    MAX_REPETICIONES_RECORDADAS_POR_DEFECTO
}
/// Reintentos por defecto ante un fallo del motor: 2 (coherente con la Regla de los Dos Intentos).
pub const MAX_REINTENTOS_MOTOR_POR_DEFECTO: u32 = 2;

pub(crate) const fn max_reintentos_motor_por_defecto() -> u32 {
    MAX_REINTENTOS_MOTOR_POR_DEFECTO
}
/// Espera por defecto entre reintentos del motor: 5 segundos. Proporciona una ventana suficiente
/// para que remita una contención transitoria de GPU (p. ej. renderizado o captura gráfica).
pub const ESPERA_REINTENTO_MOTOR_POR_DEFECTO_SEGUNDOS: u64 = 5;

pub(crate) const fn espera_reintento_motor_segundos_por_defecto() -> u64 {
    ESPERA_REINTENTO_MOTOR_POR_DEFECTO_SEGUNDOS
}
/// Denegaciones seguidas toleradas por defecto antes de abortar: 2 (Regla de los Dos Intentos).
/// Única fuente de verdad compartida con el arnés (`Limites::default`).
pub const MAX_DENEGACIONES_SEGUIDAS_POR_DEFECTO: u32 = 2;

pub(crate) const fn max_denegaciones_seguidas_por_defecto() -> u32 {
    MAX_DENEGACIONES_SEGUIDAS_POR_DEFECTO
}
/// Única fuente del tope de lectura por defecto: 16 KiB, medido (ver
/// `arnes::herramientas::TOPE_LECTURA_POR_DEFECTO`, que se deriva de esta constante en vez de
/// repetir la cifra).
pub const TOPE_LECTURA_POR_DEFECTO_BYTES: usize = 16 * 1024;

pub(crate) const fn tope_lectura_por_defecto() -> usize {
    TOPE_LECTURA_POR_DEFECTO_BYTES
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
            intentos_por_encargo: intentos_por_encargo_por_defecto(),
            entregas_en_contexto: entregas_en_contexto_por_defecto(),
            olvidar_lo_cerrado_tras_minutos: olvidar_lo_cerrado_por_defecto(),
            buzon_estable_tras_ms: buzon_estable_por_defecto(),
        }
    }
}

/// Parámetros de muestreo que se mandan al modelo en cada petición.
///
/// Lo que figura aquí es lo que recibe el modelo: no se omite ninguno, para que no queden
/// valores ocultos decididos por el servidor. Tampoco la semilla: va siempre fija, y por eso la
/// misma petición da siempre la misma respuesta.
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
    /// Semilla del sorteo de tokens. **Siempre fija**: sin ella, el servidor elige una al azar en
    /// cada petición y pedir dos veces lo mismo da dos respuestas distintas.
    /// La variación solo entra cuando alguien la pide, cambiando este número.
    #[serde(default = "semilla_por_defecto")]
    pub semilla: i64,
}

/// La temperatura de fábrica de `[muestreo]`: la de la evaluación del 22/09/2026. El banco usa la
/// misma constante, no una copia propia.
pub const TEMPERATURA_POR_DEFECTO: f32 = 0.2;

fn temperatura_por_defecto() -> f32 {
    TEMPERATURA_POR_DEFECTO
}
/// Fijado el 14/09/2026, al sacar el muestreo al TOML para que nada quede a valores ocultos.
pub const TOP_K_POR_DEFECTO: u32 = 40;

fn top_k_por_defecto() -> u32 {
    TOP_K_POR_DEFECTO
}
/// Fijado el 14/09/2026, al sacar el muestreo al TOML para que nada quede a valores ocultos.
pub const TOP_P_POR_DEFECTO: f32 = 0.95;

fn top_p_por_defecto() -> f32 {
    TOP_P_POR_DEFECTO
}
/// Fijado el 14/09/2026, al sacar el muestreo al TOML para que nada quede a valores ocultos.
pub const MIN_P_POR_DEFECTO: f32 = 0.05;

fn min_p_por_defecto() -> f32 {
    MIN_P_POR_DEFECTO
}
/// Sin penalización: 1,0 deja las probabilidades como están. Fijado el 14/09/2026.
pub const PENALIZACION_REPETICION_POR_DEFECTO: f32 = 1.0;

fn penalizacion_repeticion_por_defecto() -> f32 {
    PENALIZACION_REPETICION_POR_DEFECTO
}

/// La semilla de la evaluación del 22/09/2026, que es también la primera del banco: así una
/// tanda con la semilla por defecto se puede comparar con el uso normal.
pub const SEMILLA_POR_DEFECTO: i64 = 42;

fn semilla_por_defecto() -> i64 {
    SEMILLA_POR_DEFECTO
}

impl Default for Muestreo {
    fn default() -> Self {
        Self {
            temperatura: temperatura_por_defecto(),
            top_k: top_k_por_defecto(),
            top_p: top_p_por_defecto(),
            min_p: min_p_por_defecto(),
            penalizacion_repeticion: penalizacion_repeticion_por_defecto(),
            semilla: semilla_por_defecto(),
        }
    }
}

/// Comprobación, puramente informativa, de si hay una versión más nueva publicada en GitHub.
///
/// No descarga nada, no actualiza nada y no puede fallar el arranque: solo avisa en consola si hay
/// una versión más nueva que la instalada.
#[derive(Debug, Clone, Deserialize)]
pub struct Actualizaciones {
    /// Si Programator comprueba, al arrancar, si hay una versión más nueva en GitHub. Por
    /// defecto: `true`.
    #[serde(default = "comprobar_actualizaciones_por_defecto")]
    pub comprobar: bool,
    /// Repositorio de GitHub (`propietario/nombre`) contra el que se compara la versión instalada.
    /// Por defecto: "celtidcs/Programator".
    #[serde(default = "repositorio_actualizaciones_por_defecto")]
    pub repositorio: String,
    /// Cuánto se espera a GitHub antes de rendirse, en segundos. Por defecto: 3.
    ///
    /// Corto a propósito: es una comprobación informativa sobre una máquina que ya va a tardar
    /// minutos en atender un encargo. No tiene sentido que el arranque se note más lento por ella.
    #[serde(default = "tiempo_espera_actualizaciones_por_defecto")]
    pub tiempo_espera_segundos: u64,
}

fn comprobar_actualizaciones_por_defecto() -> bool {
    true
}
fn repositorio_actualizaciones_por_defecto() -> String {
    crate::aviso_de_version::REPOSITORIO_DE_VERSIONES_POR_DEFECTO.to_string()
}
/// Lo que puede tardar como mucho la consulta a GitHub: el arranque no se calienta esperándola (01/10/2026).
pub const TIEMPO_ESPERA_ACTUALIZACIONES_POR_DEFECTO_SEGUNDOS: u64 = 3;

fn tiempo_espera_actualizaciones_por_defecto() -> u64 {
    TIEMPO_ESPERA_ACTUALIZACIONES_POR_DEFECTO_SEGUNDOS
}

impl Default for Actualizaciones {
    fn default() -> Self {
        Self {
            comprobar: comprobar_actualizaciones_por_defecto(),
            repositorio: repositorio_actualizaciones_por_defecto(),
            tiempo_espera_segundos: tiempo_espera_actualizaciones_por_defecto(),
        }
    }
}

/// `[entregas]`: cada cuánto se deduce en qué quedó cada propuesta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Entregas {
    /// Cada cuántos minutos, como mucho, se busca en el proyecto si una propuesta se copió tal
    /// cual. Por defecto: 60. Con 0, en cada pasada.
    #[serde(default = "deducir_cada_minutos_por_defecto")]
    pub deducir_cada_minutos: u64,
    /// Cuánto se espera a git al listar los ficheros del proyecto, en segundos. Por defecto: 30.
    #[serde(default = "tiempo_de_busqueda_por_defecto")]
    pub tiempo_de_busqueda_segundos: u64,
    /// Tras cuántos días sin que nadie diga ni se deduzca nada, una propuesta queda «sin
    /// confirmar» y sale al archivo. Por defecto: 30.
    #[serde(default = "dias_sin_confirmar_por_defecto")]
    pub dias_sin_confirmar: u64,
}

/// Recorrer el proyecto cuesta en uno grande; una vez por hora basta para lo que se mide con
/// ello.
pub const DEDUCIR_CADA_MINUTOS_POR_DEFECTO: u64 = 60;
fn deducir_cada_minutos_por_defecto() -> u64 {
    DEDUCIR_CADA_MINUTOS_POR_DEFECTO
}
/// `git ls-files` contesta en milisegundos; medio minuto solo corta un git colgado.
pub const TIEMPO_DE_BUSQUEDA_POR_DEFECTO_SEGUNDOS: u64 = 30;
fn tiempo_de_busqueda_por_defecto() -> u64 {
    TIEMPO_DE_BUSQUEDA_POR_DEFECTO_SEGUNDOS
}

/// Un mes: lo que no se aplicó ni se descartó en ese tiempo ya no espera revisión.
pub const DIAS_SIN_CONFIRMAR_POR_DEFECTO: u64 = 30;
fn dias_sin_confirmar_por_defecto() -> u64 {
    DIAS_SIN_CONFIRMAR_POR_DEFECTO
}

impl Default for Entregas {
    fn default() -> Self {
        Self {
            deducir_cada_minutos: deducir_cada_minutos_por_defecto(),
            tiempo_de_busqueda_segundos: tiempo_de_busqueda_por_defecto(),
            dias_sin_confirmar: dias_sin_confirmar_por_defecto(),
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

        let (mut config, avisos) = interpretar_sin_claves_retiradas(&texto)
            .map_err(|e| Error::Configuracion(toml_ininteligible(ruta, &e)))?;

        if config.motor.modelo.is_none() {
            return Err(Error::ConfiguracionDelMotor(falta_el_modelo()));
        }

        // Suelo del intervalo de sondeo. Un `0` explícito en el TOML pasaría la carga sin quejarse
        // y el bucle de `main` giraría a máxima velocidad: un núcleo al 100% releyendo el canal
        // entero sin pausa, quizá sobre una unidad de red o sincronizada. El fichero lo edita el usuario a
        // mano, así que el error es plausible y el arnés no puede permitirse confiar en que no
        // ocurra. Un segundo es tan absurdamente corto como se quiera, pero no quema la máquina.
        if config.ciclo.intervalo_segundos < INTERVALO_MINIMO_SEGUNDOS {
            config.ciclo.intervalo_segundos = INTERVALO_MINIMO_SEGUNDOS;
        }
        let mut avisos = avisos;
        for (clave, valor) in [
            (
                "[ciclo] intervalo_segundos",
                &mut config.ciclo.intervalo_segundos,
            ),
            (
                "[verificacion] tiempo_maximo_segundos",
                &mut config.verificacion.tiempo_maximo_segundos,
            ),
            (
                "[entregas] tiempo_de_busqueda_segundos",
                &mut config.entregas.tiempo_de_busqueda_segundos,
            ),
            (
                "[preparacion] tiempo_maximo_segundos",
                &mut config.preparacion.tiempo_maximo_segundos,
            ),
        ] {
            if *valor > TECHO_DE_SEGUNDOS {
                avisos.push(aviso_de_techo(clave, *valor));
                *valor = TECHO_DE_SEGUNDOS;
            }
        }

        // Mismo criterio con el paralelismo del motor: un `0` en el TOML es un motor que no
        // atiende a nadie, y `llama-server` lo rechazaría al arrancar con un error que no dice de
        // dónde viene. Se corrige aquí, donde sí se sabe.
        if config.motor.paralelo == 0 {
            config.motor.paralelo = 1;
        }

        // Un envoltorio vacío lanzaría las órdenes aisladas sin aislar: se vuelve al de fábrica.
        if config
            .preparacion
            .envoltorio
            .as_ref()
            .is_some_and(Vec::is_empty)
        {
            config.preparacion.envoltorio = None;
            avisos.push(aviso_de_envoltorio_vacio());
        }

        // Un `0` dejaría al modelo sin saber por qué se rechazó su propuesta: se vuelve al de
        // fábrica.
        if config.verificacion.maximo_salida_al_modelo == 0 {
            config.verificacion.maximo_salida_al_modelo = MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO;
        }

        config.avisos = avisos;

        config.base = ruta
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        Ok(config)
    }

    /// Lo que conviene decir al arrancar sobre el fichero: claves que ya no valen.
    pub fn avisos(&self) -> &[String] {
        &self.avisos
    }

    /// La misma configuración con `modelo` en lugar de `[motor] modelo`, si se pide alguno. Es lo
    /// que hace `--modelo`: cambia el modelo de una ejecución sin tocar el TOML, y se resuelve
    /// igual que el del TOML, contra la carpeta de este.
    pub fn con_modelo(mut self, modelo: Option<&str>) -> Self {
        if let Some(modelo) = modelo {
            self.motor.modelo = Some(modelo.to_string());
        }
        self
    }

    /// La carpeta donde está instalado Programator: la del fichero de configuración en uso. Ahí se
    /// resuelven sus rutas relativas y ahí se hace la revisión de modelos.
    pub fn carpeta_de_instalacion(&self) -> &Path {
        &self.base
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

    /// La ruta sale tal cual: con `{:?}` saldría entre comillas y, en Windows, con las barras
    /// dobladas.
    #[test]
    fn el_toml_ininteligible_nombra_la_ruta_tal_cual() {
        let ruta = Path::new("carpeta").join("programator.toml");
        let texto = toml_ininteligible(&ruta, &"falta un igual");
        assert_eq!(
            texto,
            format!(
                "«{}» no se pudo interpretar: falta un igual",
                ruta.display()
            )
        );
        assert!(!texto.contains('"'), "{texto}");
    }

    fn escribir(carpeta: &std::path::Path, contenido: &str) -> std::path::PathBuf {
        let ruta = carpeta.join("programator.toml");
        let mut f = std::fs::File::create(&ruta).unwrap();
        f.write_all(contenido.as_bytes()).unwrap();
        ruta
    }

    #[test]
    fn carga_una_configuracion_completa() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta_temporal.path(),
            r#"
[carpeta]
ruta = "C:/proyectos/mi-proyecto"

[agente]
nombre = "Programator"
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
        assert_eq!(config.ciclo.intervalo_segundos, 30);
        assert_eq!(
            crate::arranque::cadencia_sondeo(config.ciclo.intervalo_segundos),
            "cada 30 segundos"
        );
        assert_eq!(config.ciclo.max_reintentos_motor, 2);
        assert_eq!(config.ciclo.espera_reintento_motor_segundos, 5);
        assert_eq!(config.ciclo.max_denegaciones_seguidas, 2);
    }

    #[test]
    fn la_preparacion_del_ejemplo_es_la_de_por_defecto() {
        let ejemplo: Config = toml::from_str(include_str!("../programator.ejemplo.toml")).unwrap();
        assert_eq!(ejemplo.preparacion, Preparacion::default());
    }

    /// Los ejemplos comentados de `[preparacion.ordenes.rs]`, sin el comentario, se leen.
    #[test]
    fn las_ordenes_comentadas_del_ejemplo_se_leen() {
        let ejemplo = include_str!("../programator.ejemplo.toml");
        let inicio = ejemplo.find("# [preparacion.ordenes.rs]").unwrap();
        let tabla: String = ejemplo[inicio..]
            .lines()
            .take_while(|l| l.starts_with("# "))
            .map(|l| format!("{}\n", &l[2..]))
            .collect();
        let leida: Preparacion =
            toml::from_str(&tabla.replace("[preparacion.ordenes.rs]", "[ordenes.rs]")).unwrap();
        let rs = leida.ordenes_de("rs").unwrap();
        assert_eq!(rs.previas, [["cargo", "fetch"]]);
        assert_eq!(rs.formatear, ["cargo", "fmt"]);
        assert_eq!(rs.aisladas.len(), 2);
    }

    #[test]
    fn un_tiempo_de_preparacion_de_mas_de_un_dia_se_acota() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("programator.toml");
        std::fs::write(
            &ruta,
            "[motor]\nmodelo = \"m.gguf\"\n[preparacion]\ntiempo_maximo_segundos = 999999\n\
             [entregas]\ntiempo_de_busqueda_segundos = 999999\n",
        )
        .unwrap();
        let config = Config::cargar(&ruta).unwrap();
        assert_eq!(config.preparacion.tiempo_maximo_segundos, TECHO_DE_SEGUNDOS);
        assert_eq!(
            config.entregas.tiempo_de_busqueda_segundos,
            TECHO_DE_SEGUNDOS
        );
        assert_eq!(config.avisos().len(), 2, "{:?}", config.avisos());
    }

    #[test]
    fn el_envoltorio_de_preparar_se_lee_y_vacio_no_vale() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("programator.toml");
        let cargar = |envoltorio: &str| {
            std::fs::write(
                &ruta,
                format!("[motor]\nmodelo = \"m.gguf\"\n[preparacion]\nenvoltorio = {envoltorio}\n"),
            )
            .unwrap();
            Config::cargar(&ruta).unwrap()
        };

        let propio = cargar("[\"bwrap\", \"--unshare-net\"]");
        assert_eq!(
            propio.preparacion.envoltorio,
            Some(vec!["bwrap".to_string(), "--unshare-net".to_string()])
        );
        assert!(propio.avisos().is_empty());

        let vacio = cargar("[]");
        assert_eq!(vacio.preparacion.envoltorio, None);
        assert_eq!(vacio.avisos(), [aviso_de_envoltorio_vacio()]);
    }

    #[test]
    fn las_entregas_del_ejemplo_son_las_de_por_defecto() {
        let ejemplo: Config = toml::from_str(include_str!("../programator.ejemplo.toml")).unwrap();
        assert_eq!(ejemplo.entregas, Entregas::default());
        assert_eq!(ejemplo.entregas.deducir_cada_minutos, 60);
        assert_eq!(ejemplo.entregas.dias_sin_confirmar, 30);
    }

    #[test]
    fn la_revision_del_ejemplo_es_la_de_por_defecto() {
        let ejemplo: Config = toml::from_str(include_str!("../programator.ejemplo.toml")).unwrap();
        assert_eq!(
            ejemplo.revision,
            crate::revision::config::ConfigRevision::default()
        );
    }

    #[test]
    fn sin_seccion_de_revision_valen_los_valores_por_defecto() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(carpeta.path(), "[motor]\nmodelo = \"m.gguf\"\n");
        let config = Config::cargar(&ruta).unwrap();
        assert_eq!(
            config.revision,
            crate::revision::config::ConfigRevision::default()
        );
    }

    #[test]
    fn el_modelo_pedido_sustituye_al_del_toml_y_se_resuelve_igual() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(carpeta.path(), "[motor]\nmodelo = \"m.gguf\"\n");
        let config = Config::cargar(&ruta).unwrap();

        let igual = config.clone().con_modelo(None);
        assert_eq!(igual.motor.modelo.as_deref(), Some("m.gguf"));

        let otra = config.con_modelo(Some("otros/n.gguf"));
        assert_eq!(otra.motor.modelo.as_deref(), Some("otros/n.gguf"));
        assert_eq!(
            otra.resolver("otros/n.gguf"),
            carpeta.path().join("otros/n.gguf")
        );
    }

    #[test]
    fn el_nombre_del_modelo_es_el_del_fichero_sin_carpeta_ni_extension() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            "[motor]\nmodelo = \"modelos/devstral-Q4_K_M.gguf\"\n",
        );
        let config = Config::cargar(&ruta).unwrap();
        assert_eq!(
            config.motor.nombre_del_modelo().as_deref(),
            Some("devstral-Q4_K_M")
        );
        let mut sin_modelo = config.motor.clone();
        sin_modelo.modelo = None;
        assert_eq!(sin_modelo.nombre_del_modelo(), None);
    }

    /// Lo que `programator.ejemplo.toml` dice que vale cada clave sin tocar es lo que vale de
    /// verdad sin escribirla. Fija los valores de fábrica antes de pasarlos a constantes.
    #[test]
    fn los_valores_por_defecto_son_los_que_documenta_el_ejemplo() {
        let ejemplo: Config = toml::from_str(include_str!("../programator.ejemplo.toml")).unwrap();
        let fabrica: Config = toml::from_str("[motor]\nmodelo = \"m.gguf\"\n").unwrap();

        macro_rules! iguales {
            ($($campo:ident . $clave:ident),+ $(,)?) => {
                $(assert_eq!(
                    ejemplo.$campo.$clave, fabrica.$campo.$clave,
                    concat!(stringify!($campo), ".", stringify!($clave))
                );)+
            };
        }
        iguales!(
            motor.puerto,
            motor.contexto,
            motor.capas_gpu,
            motor.margen_vram_mib,
            motor.cache_kv,
            motor.reparto,
            motor.atencion_rapida,
            motor.paralelo,
            motor.empezar_en_frio,
            motor.umbral_aviso_contexto,
            motor.bytes_por_token,
            motor.tiempo_lectura_segundos,
            motor.tiempo_escritura_segundos,
            motor.soltar_modelo,
            motor.espera_de_carga_segundos,
            motor.clave,
            ciclo.intervalo_segundos,
            ciclo.max_herramientas_por_encargo,
            ciclo.tope_lectura_bytes,
            ciclo.fichero_latido,
            ciclo.max_repeticiones_recordadas,
            ciclo.max_reintentos_motor,
            ciclo.espera_reintento_motor_segundos,
            ciclo.max_denegaciones_seguidas,
            ciclo.intentos_por_encargo,
            muestreo.temperatura,
            muestreo.top_k,
            muestreo.top_p,
            muestreo.min_p,
            muestreo.penalizacion_repeticion,
            muestreo.semilla,
            ciclo.entregas_en_contexto,
            ciclo.olvidar_lo_cerrado_tras_minutos,
            ciclo.buzon_estable_tras_ms,
            actualizaciones.comprobar,
            actualizaciones.repositorio,
            actualizaciones.tiempo_espera_segundos,
            verificacion.tiempo_maximo_segundos,
            verificacion.comprobar_python,
            agente.nombre,
            agente.plantilla_normas,
            agente.preambulo,
            agente.recordatorio,
        );
        // Las cifras de fábrica, para que pasarlas a constantes no cambie ninguna.
        assert_eq!(
            (
                fabrica.motor.puerto,
                fabrica.motor.contexto,
                fabrica.motor.umbral_aviso_contexto,
                fabrica.motor.bytes_por_token,
                fabrica.motor.tiempo_lectura_segundos,
                fabrica.motor.tiempo_escritura_segundos,
            ),
            (8080, 16384, 70, 4, 600, 60)
        );
        assert_eq!(
            (
                fabrica.ciclo.max_herramientas_por_encargo,
                fabrica.ciclo.tope_lectura_bytes,
                fabrica.ciclo.max_repeticiones_recordadas,
                fabrica.ciclo.max_reintentos_motor,
                fabrica.ciclo.espera_reintento_motor_segundos,
                fabrica.ciclo.max_denegaciones_seguidas,
            ),
            (12, 16384, 12, 2, 5, 2)
        );
        assert_eq!(
            (
                fabrica.muestreo.top_k,
                fabrica.muestreo.top_p,
                fabrica.muestreo.min_p,
                fabrica.muestreo.penalizacion_repeticion,
            ),
            (40, 0.95, 0.05, 1.0)
        );
        assert_eq!(fabrica.motor.cache_kv, "q8_0");
        assert_eq!(fabrica.motor.paralelo, 1);
        assert_eq!(fabrica.verificacion.tiempo_maximo_segundos, 30);
        assert_eq!(fabrica.actualizaciones.tiempo_espera_segundos, 3);
        assert_eq!(fabrica.actualizaciones.repositorio, "celtidcs/Programator");
    }

    #[test]
    fn la_cache_se_lee_con_cache_kv_con_cache_k_y_cache_v_o_mezcladas() {
        // G3.
        let motor = |texto: &str| -> MotorConfig {
            toml::from_str::<Config>(&format!("[motor]\nmodelo = \"m.gguf\"\n{texto}"))
                .unwrap()
                .motor
        };
        let caches = |m: &MotorConfig| (m.cache_k().to_string(), m.cache_v().to_string());

        assert_eq!(caches(&motor("")), ("q8_0".into(), "q8_0".into()));
        assert_eq!(
            caches(&motor("cache_kv = \"f16\"")),
            ("f16".into(), "f16".into()),
            "cache_kv solo, como en la 1.1.0, da las dos"
        );
        assert_eq!(
            caches(&motor("cache_k = \"f16\"\ncache_v = \"q4_0\"")),
            ("f16".into(), "q4_0".into())
        );
        assert_eq!(
            caches(&motor("cache_kv = \"q4_0\"\ncache_k = \"f16\"")),
            ("f16".into(), "q4_0".into()),
            "la que falta sale de cache_kv"
        );
        assert_eq!(
            caches(&motor("cache_v = \"f16\"")),
            ("q8_0".into(), "f16".into()),
            "sin cache_kv, la que falta es la de fábrica"
        );
    }

    #[test]
    fn las_claves_del_reparto_se_leen_sueltas_en_la_seccion_del_motor() {
        let motor = toml::from_str::<Config>(
            "[motor]\nmodelo = \"m.gguf\"\ncache_en_cpu = true\nffn_en_cpu = \"auto\"\n\
             tensores_en_cpu = [\"output\"]\n",
        )
        .unwrap()
        .motor;
        assert!(motor.reparto.cache_en_cpu);
        assert_eq!(motor.reparto.ffn_en_cpu, CapasEnCpu::Auto);
        assert_eq!(motor.reparto.tensores_en_cpu, vec!["output".to_string()]);
        let fallo = toml::from_str::<Config>("[motor]\nmodelo = \"m.gguf\"\nffn_en_cpu = 1.5\n")
            .unwrap_err()
            .to_string();
        assert!(fallo.contains("ffn_en_cpu solo admite"), "{fallo}");
    }

    #[test]
    fn reintentos_motor_se_leen_del_toml() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
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
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
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
    fn python_se_busca_por_defecto_y_se_puede_apagar_o_fijar() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("programator.toml");
        std::fs::write(&ruta, "[motor]\nmodelo = \"x.gguf\"\n").unwrap();
        let por_defecto = Config::cargar(&ruta).unwrap().verificacion;
        std::fs::write(
            &ruta,
            "[motor]\nmodelo = \"x.gguf\"\n\n[verificacion]\ncomprobar_python = false\n\
             python = \"C:/Python312/python.exe\"\n",
        )
        .unwrap();
        let fijado = Config::cargar(&ruta).unwrap().verificacion;

        assert!(por_defecto.comprobar_python);
        assert!(por_defecto.python.is_empty());
        assert!(!fijado.comprobar_python);
        assert_eq!(fijado.python, "C:/Python312/python.exe");
        assert!(Verificacion::default().comprobar_python);
    }

    #[test]
    fn sin_seccion_de_verificacion_no_hay_comprobadores_pero_si_tiempo_maximo() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(carpeta.path(), "[motor]\nmodelo = \"m.gguf\"\n");

        let config = Config::cargar(&ruta).unwrap();

        assert!(config.verificacion.comprobadores.is_empty());
        assert_eq!(
            config.verificacion.tiempo_maximo_segundos, 30,
            "un tope de cero segundos cortaría cualquier comprobación nada más lanzarla"
        );
    }

    #[test]
    fn los_comprobadores_se_leen_del_toml_con_sus_argumentos() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            r#"
[motor]
modelo = "m.gguf"

[verificacion]
tiempo_maximo_segundos = 15
maximo_salida_al_modelo = 500

[verificacion.comprobadores]
py = ["python", "-m", "py_compile", "{fichero}"]
lua = ["luac", "-p", "{fichero}"]
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.verificacion.tiempo_maximo_segundos, 15);
        assert_eq!(config.verificacion.maximo_salida_al_modelo, 500);
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
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
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
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
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
    fn una_salida_al_modelo_de_cero_vuelve_a_la_de_fabrica() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"

[verificacion]
maximo_salida_al_modelo = 0
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(
            config.verificacion.maximo_salida_al_modelo,
            MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO
        );
    }

    /// Sin techo, un valor enorme llegaría a sumas de tiempo que entran en pánico.
    #[test]
    fn los_tiempos_de_mas_de_un_dia_se_bajan_a_un_dia_y_se_avisa() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            "[motor]\nmodelo = \"x.gguf\"\n\n[ciclo]\nintervalo_segundos = 9223372036854775807\n\n\
             [verificacion]\ntiempo_maximo_segundos = 99999999999\n",
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.ciclo.intervalo_segundos, TECHO_DE_SEGUNDOS);
        assert_eq!(
            config.verificacion.tiempo_maximo_segundos,
            TECHO_DE_SEGUNDOS
        );
        assert_eq!(config.avisos().len(), 2, "{:?}", config.avisos());
        assert!(config.avisos()[0].contains("[ciclo] intervalo_segundos"));
        assert!(config.avisos()[1].contains("[verificacion] tiempo_maximo_segundos"));
    }

    /// Un `[agente]` con una sola clave carga y toma el resto por defecto.
    #[test]
    fn un_agente_con_una_sola_clave_toma_el_resto_por_defecto() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            "[motor]\nmodelo = \"x.gguf\"\n\n[agente]\nregistro_de_conversacion = true\n",
        );

        let config = Config::cargar(&ruta).unwrap();

        assert!(config.agente.registro_de_conversacion);
        assert_eq!(config.agente.nombre, Agente::default().nombre);
        assert_eq!(
            config.agente.plantilla_normas,
            Agente::default().plantilla_normas
        );
    }

    /// Dos claves que no se leían en ningún sitio.
    #[test]
    fn el_buzon_y_el_umbral_de_poda_ya_no_valen_y_lo_avisan() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            "[motor]\nmodelo = \"x.gguf\"\n\n[agente]\nbuzon = \"otro.md\"\n\n\
             [poda]\numbral_lineas = 80\n",
        );

        let config = Config::cargar(&ruta).unwrap();

        let avisos = config.avisos().join("\n");
        assert!(avisos.contains("[agente] buzon"), "{avisos}");
        assert!(avisos.contains("[poda] umbral_lineas"), "{avisos}");
    }

    #[test]
    fn un_intervalo_de_cero_se_eleva_al_suelo_para_no_quemar_un_nucleo() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
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
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            "[motor]\nmodelo = \"x.gguf\"\n\n[ciclo]\nintervalo_segundos = 5\n",
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(
            config.ciclo.intervalo_segundos, 5,
            "el suelo no puede pisar un valor que el usuario eligió a conciencia"
        );
    }

    #[test]
    fn rechaza_una_configuracion_sin_modelo() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(carpeta.path(), "[motor]\nbinario = \"x.exe\"\n");

        let fallo = Config::cargar(&ruta).unwrap_err();

        assert!(
            fallo.to_string().contains("modelo"),
            "el error debe nombrar el campo que falta, y decía: {fallo}"
        );
        // Es un fallo del motor, para que el arranque dé el consejo del motor, y se lee como
        // cualquier configuración inválida.
        assert!(
            matches!(fallo, Error::ConfiguracionDelMotor(_)),
            "{fallo:?}"
        );
        assert!(
            fallo.to_string().starts_with("configuración inválida: "),
            "{fallo}"
        );
    }

    #[test]
    fn resuelve_las_rutas_relativas_contra_la_base_y_no_contra_el_directorio_actual() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(carpeta.path(), "[motor]\nmodelo = \"modelos/x.gguf\"\n");
        let config = Config::cargar(&ruta).unwrap();

        let resuelta = config.resolver("modelos/x.gguf");

        assert_eq!(resuelta, carpeta.path().join("modelos/x.gguf"));
    }

    #[test]
    fn el_muestreo_ausente_toma_los_valores_por_defecto() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(carpeta.path(), "[motor]\nmodelo = \"modelos/x.gguf\"\n");

        let config = Config::cargar(&ruta).unwrap();

        assert!((config.muestreo.temperatura - 0.2).abs() < f32::EPSILON);
        assert_eq!(config.muestreo.top_k, 40);
        assert!((config.muestreo.top_p - 0.95).abs() < f32::EPSILON);
        assert!((config.muestreo.min_p - 0.05).abs() < f32::EPSILON);
        assert!((config.muestreo.penalizacion_repeticion - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn el_muestreo_se_lee_del_fichero() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            r#"
[motor]
modelo = "modelos/x.gguf"

[muestreo]
temperatura = 0.7
top_k = 64
semilla = 7
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert!((config.muestreo.temperatura - 0.7).abs() < f32::EPSILON);
        assert_eq!(config.muestreo.top_k, 64);
        assert_eq!(config.muestreo.semilla, 7);
        assert!((config.muestreo.top_p - 0.95).abs() < f32::EPSILON);
        assert!((config.muestreo.min_p - 0.05).abs() < f32::EPSILON);
        assert!((config.muestreo.penalizacion_repeticion - 1.0).abs() < f32::EPSILON);
    }

    // Sustituye a la aserción `semilla == None` del contrato anterior, retirado el
    // 12/11/2026: sin semilla en el TOML ya no decide el servidor, se usa la fija.
    #[test]
    fn sin_semilla_en_el_toml_se_usa_la_semilla_por_defecto() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            "[motor]\n\
             modelo = \"modelos/x.gguf\"\n\
             [muestreo]\n",
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.muestreo.semilla, SEMILLA_POR_DEFECTO);
    }

    #[test]
    fn sin_ruta_del_registro_se_usa_la_de_junto_al_toml() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            "[motor]\n\
             modelo = \"modelos/x.gguf\"\n",
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(
            config.agente.registro_de_modelos,
            "registro-de-modelos.toml"
        );
    }

    #[test]
    fn una_semilla_que_no_es_entera_no_se_acepta() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            "[motor]\n\
             modelo = \"modelos/x.gguf\"\n\
             [muestreo]\n\
             semilla = \"aleatoria\"\n",
        );

        assert!(Config::cargar(&ruta).is_err());
    }

    #[test]
    fn las_claves_de_la_1_0_0_tienen_su_valor_por_defecto() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.motor.soltar_modelo, SoltarModelo::Auto);
        assert_eq!(config.motor.vram_maxima_mib, None, "sin tope por defecto");
        assert_eq!(config.motor.espera_de_carga_segundos, 300);
        assert_eq!(config.ciclo.intentos_por_encargo, 2);
        assert_eq!(Ciclo::default().intentos_por_encargo, 2);
    }

    #[test]
    fn las_claves_de_la_1_0_0_se_leen_del_toml() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
soltar_modelo = 0
vram_maxima_mib = 8192
espera_de_carga_segundos = 120

[ciclo]
intentos_por_encargo = 5
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.motor.soltar_modelo, SoltarModelo::TrasMinutos(0));
        assert_eq!(config.motor.vram_maxima_mib, Some(8192));
        assert_eq!(config.motor.espera_de_carga_segundos, 120);
        assert_eq!(config.ciclo.intentos_por_encargo, 5);
    }

    #[test]
    fn soltar_modelo_invalido_no_carga() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
soltar_modelo = "nunca"
"#,
        );

        assert!(Config::cargar(&ruta).is_err());
    }

    #[test]
    fn capas_gpu_acepta_la_palabra_auto() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            r#"
[motor]
modelo = "modelos/devstral.gguf"
capas_gpu = "auto"
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.motor.capas_gpu, CapasGpu::Auto);
        assert_eq!(
            config.motor.margen_vram_mib, MARGEN_VRAM_POR_DEFECTO_MIB,
            "margen por defecto"
        );
    }

    #[test]
    fn capas_gpu_acepta_un_numero_y_lo_respeta() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
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
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
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
        let carpeta = tempfile::tempdir().unwrap();
        // «maximas» no puede contener la subcadena «auto»: si la palabra rechazada la llevara
        // (como pasaba con «automatico»), la aserción de más abajo se cumpliría sola —el mensaje
        // haría eco de la entrada— aunque alguien borrara la sugerencia final y el error dejara de
        // decir cuál es la única palabra válida. Con «maximas», el «auto» del mensaje solo puede
        // venir de la sugerencia.
        let ruta = escribir(
            carpeta.path(),
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
        // inglés, en un fichero que el usuario edita a mano.
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
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
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
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

    /// Sin el `rename` del campo, la clave del TOML se ignoraría en silencio y todo seguiría en
    /// verde.
    #[test]
    fn flash_attn_del_toml_llega_a_la_configuracion_del_motor() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            r#"
[motor]
modelo = "m.gguf"
flash_attn = "off"
"#,
        );

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.motor.atencion_rapida, AtencionRapida::Desactivada);
    }

    #[test]
    fn fichero_latido_toma_valor_por_defecto_o_configurado() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let ruta_defecto = escribir(
            carpeta_temporal.path(),
            r#"
[motor]
modelo = "m.gguf"
"#,
        );
        let config_defecto = Config::cargar(&ruta_defecto).unwrap();
        assert_eq!(config_defecto.ciclo.fichero_latido, "latido.json");

        let ruta_personalizada = escribir(
            carpeta_temporal.path(),
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

    const MINIMA: &str = r#"
[carpeta]
ruta = "C:/proyectos/mi-proyecto"

[agente]
nombre = "Programator"
plantilla_normas = "plantillas/instrucciones-del-proyecto.md"

[motor]
binario = "herramientas/llama-server.exe"
modelo = "modelos/devstral.gguf"
"#;

    #[test]
    fn la_clave_de_los_prefijos_ya_no_vale_y_lo_avisa() {
        let carpeta = tempfile::tempdir().unwrap();
        let con_la_clave = MINIMA.replace(
            "nombre = \"Programator\"",
            "nombre = \"Programator\"\nprefijos_ignorados = [\"archivado-\"]",
        );
        let ruta = escribir(carpeta.path(), &con_la_clave);

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.avisos().len(), 1);
        assert!(config.avisos()[0].contains("[agente] prefijos_ignorados"));
        assert!(config.avisos()[0].contains("historico-"));
    }

    /// La identidad sale solo de la cabecera del buzón, así que la lista de nombres de fichero
    /// deja de valer. Una configuración anterior puede traerla todavía.
    #[test]
    fn la_lista_de_buzones_que_ordenan_ya_no_vale_y_dice_la_nueva() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            &format!("{MINIMA}\n[revision]\nbuzones_que_ordenan = [\"claude\"]\n"),
        );

        let config = Config::cargar(&ruta).unwrap();

        assert!(config.revision.identidades_que_ordenan.is_empty());
        assert_eq!(config.avisos().len(), 1);
        assert!(config.avisos()[0].contains("[revision] buzones_que_ordenan"));
        assert!(config.avisos()[0].contains("identidades_que_ordenan"));
    }

    #[test]
    fn las_claves_de_la_maquina_de_la_revision_ya_no_valen_y_no_impiden_arrancar() {
        // [revision] rechaza las claves desconocidas: sin quitarlas antes, un programator.toml
        // de antes del 13/11/2026 no arrancaría.
        let carpeta = tempfile::tempdir().unwrap();
        let con_las_claves = format!(
            "{MINIMA}\n[revision]\nmaquina_autorizada = \"maquina-linux\"\n\
             reintentar_maquina_cada_segundos = 300\n\
             esperar_maquina_como_mucho_minutos = 480\ncandidatos_por_revision = 2\n"
        );
        let ruta = escribir(carpeta.path(), &con_las_claves);

        let config = Config::cargar(&ruta).unwrap();

        assert_eq!(config.revision.candidatos_por_revision, 2);
        let avisos = config.avisos().join("\n");
        assert_eq!(config.avisos().len(), 3, "{avisos}");
        for clave in [
            "[revision] maquina_autorizada",
            "[revision] reintentar_maquina_cada_segundos",
            "[revision] esperar_maquina_como_mucho_minutos",
        ] {
            assert!(avisos.contains(clave), "falta «{clave}»:\n{avisos}");
        }
        assert!(avisos.contains("[comprobacion_local]"), "{avisos}");
    }

    #[test]
    fn una_clave_desconocida_de_la_revision_sigue_siendo_un_error() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(
            carpeta.path(),
            &format!("{MINIMA}\n[revision]\nmaquina = \"x\"\n"),
        );

        assert!(Config::cargar(&ruta).is_err());
    }

    #[test]
    fn sin_claves_retiradas_no_hay_avisos() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = escribir(carpeta.path(), MINIMA);

        assert!(Config::cargar(&ruta).unwrap().avisos().is_empty());
    }
}
