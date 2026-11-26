//! Estado comprobable del arnés en cada sondeo.
//!
//! **Por qué este módulo.** El uso real documentó el coste de un
//! arnés mudo: a las 10:35 quien dirigía concluyó que Programator estaba parado porque `lectura.json`
//! no se tocaba desde las 09:34. Era falso: los ficheros solo cambiaban cuando el arnés actuaba, no
//! cuando sondeaba. El duplicado resultante hubo que retirarlo a mano doce minutos después.
//!
//! Este módulo define el [`Latido`], un fichero que se escribe en **cada sondeo** —actúe el arnés o
//! no— declarando su estado (`reposo`, `atendiendo` o `error`), el encargo en curso si lo hay, y,
//! salvo mientras atiende, cuándo será el próximo sondeo.

use crate::error::{Error, Resultado};
use crate::terminal::Terminal;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// El estado en el que se encuentra el arnés en el momento del sondeo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoLatido {
    /// No hay encargos en curso; esperando al siguiente sondeo.
    Reposo,
    /// Atendiendo un encargo del canal.
    Atendiendo,
    /// La última pasada terminó en fallo no recuperable.
    Error,
}

/// Identifica el encargo que el arnés está atendiendo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetalleEncargo {
    /// Fichero del buzón de origen (por ejemplo `claude.md`).
    pub buzon: String,
    /// Primera línea o resumen del encargo para identificarlo sin abrir el buzón.
    pub resumen: String,
    /// El identificador del encargo (`protocolo::identificador`), el mismo que da nombre a su
    /// ficha en el registro de encargos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Longitud máxima de una línea de resumen (la del encargo y la del latido del buzón), para que se
/// lea de un vistazo.
const TOPE_CARACTERES_RESUMEN: usize = 120;
/// Caracteres que se conservan antes de añadir la elipsis «…».
const CARACTERES_RECORTE_RESUMEN: usize = TOPE_CARACTERES_RESUMEN - 3;

/// Una línea de resumen acotada: si pasa del tope, se recorta y se le añade «…». Única para el
/// latido y el buzón, para que no tenga cada uno su copia del tope y del recorte.
pub(crate) fn acotar_resumen(linea: &str) -> String {
    if linea.chars().count() > TOPE_CARACTERES_RESUMEN {
        let acotada: String = linea.chars().take(CARACTERES_RECORTE_RESUMEN).collect();
        format!("{acotada}…")
    } else {
        linea.to_string()
    }
}
/// Texto por defecto cuando el encargo no contiene líneas legibles.
const TEXTO_ENCARGO_VACIO: &str = "encargo sin texto";

impl DetalleEncargo {
    /// Construye el detalle a partir del buzón y del texto íntegro del encargo.
    pub fn nuevo(buzon: &str, texto_encargo: &str) -> Self {
        let primera_linea =
            super::encargo::primera_linea_con_texto(texto_encargo).unwrap_or(TEXTO_ENCARGO_VACIO);

        let resumen = acotar_resumen(primera_linea);

        Self {
            buzon: buzon.to_string(),
            resumen,
            id: None,
        }
    }

    /// El detalle de un encargo detectado, con su identificador.
    pub fn del_encargo(encargo: &crate::protocolo::Encargo) -> Self {
        Self {
            id: Some(encargo.id()),
            ..Self::nuevo(&encargo.de, &encargo.texto)
        }
    }
}

/// Foto del estado del arnés en un momento concreto.
///
/// Los campos son privados: un latido solo se hace con sus constructores
/// (`en_reposo`, `en_atencion`, `en_error`), que no dejan un detalle de error sin estado de error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Latido {
    /// Marca de tiempo de este latido.
    marca_tiempo: String,
    /// Estado actual del arnés.
    estado: EstadoLatido,
    /// Encargo que se está atendiendo, si lo hay.
    #[serde(skip_serializing_if = "Option::is_none")]
    encargo: Option<DetalleEncargo>,
    /// Momento estimado en que tendrá lugar el próximo sondeo. Mientras se atiende no lo hay: el
    /// siguiente sondeo es al terminar la llamada en curso al modelo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    proximo_sondeo: Option<String>,
    /// Detalle adicional del error, si el estado es `Error`.
    #[serde(skip_serializing_if = "Option::is_none")]
    detalle_error: Option<String>,
    /// El modelo que Programator tiene cargado en su motor, si tiene alguno. Opcional, para que
    /// los latidos de antes se sigan leyendo y los de ahora sin modelo no cambien.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    modelo_cargado: Option<String>,
}

impl Latido {
    /// Estado actual del arnés.
    pub fn estado(&self) -> EstadoLatido {
        self.estado
    }

    /// Encargo que se está atendiendo, si lo hay.
    pub fn encargo(&self) -> Option<&DetalleEncargo> {
        self.encargo.as_ref()
    }

    /// Detalle del error, si el estado es `Error`.
    pub fn detalle_error(&self) -> Option<&str> {
        self.detalle_error.as_deref()
    }

    /// El modelo que Programator tiene cargado, si tiene alguno.
    pub fn modelo_cargado(&self) -> Option<&str> {
        self.modelo_cargado.as_deref()
    }

    /// Crea un latido en estado de reposo, sin encargo activo.
    fn reposo(marca_tiempo: impl Into<String>, proximo_sondeo: impl Into<String>) -> Self {
        Self {
            marca_tiempo: marca_tiempo.into(),
            estado: EstadoLatido::Reposo,
            encargo: None,
            proximo_sondeo: Some(proximo_sondeo.into()),
            detalle_error: None,
            modelo_cargado: None,
        }
    }

    /// Crea un latido en estado de atención, con el encargo en curso.
    fn atendiendo(marca_tiempo: impl Into<String>, encargo: DetalleEncargo) -> Self {
        Self {
            proximo_sondeo: None,
            marca_tiempo: marca_tiempo.into(),
            estado: EstadoLatido::Atendiendo,
            encargo: Some(encargo),
            detalle_error: None,
            modelo_cargado: None,
        }
    }

    /// Crea un latido en estado de error, opcionalmente con el encargo que falló y el motivo.
    fn error(
        marca_tiempo: impl Into<String>,
        proximo_sondeo: impl Into<String>,
        encargo: Option<DetalleEncargo>,
        detalle_error: impl Into<String>,
    ) -> Self {
        Self {
            marca_tiempo: marca_tiempo.into(),
            estado: EstadoLatido::Error,
            encargo,
            proximo_sondeo: Some(proximo_sondeo.into()),
            detalle_error: Some(detalle_error.into()),
            modelo_cargado: None,
        }
    }

    /// El mismo latido, diciendo qué modelo tiene cargado el motor propio.
    pub fn con_modelo_cargado(mut self, modelo: Option<String>) -> Self {
        self.modelo_cargado = modelo;
        self
    }

    /// Crea un latido en estado de reposo calculando los tiempos a partir del momento actual
    /// y del intervalo en segundos del ciclo.
    pub fn en_reposo(ahora: DateTime<Local>, intervalo_segundos: u64) -> Self {
        let (inicio, proximo) = calcular_tiempos_sondeo(ahora, intervalo_segundos);
        Self::reposo(inicio, proximo)
    }

    /// Crea un latido en estado de atención en el momento `ahora`. No lleva próximo sondeo.
    pub fn en_atencion(ahora: DateTime<Local>, encargo: DetalleEncargo) -> Self {
        Self::atendiendo(formatear_marca_tiempo(ahora), encargo)
    }

    /// Crea un latido en estado de error calculando los tiempos a partir del momento actual
    /// y del intervalo en segundos del ciclo.
    pub fn en_error(
        ahora: DateTime<Local>,
        intervalo_segundos: u64,
        encargo: Option<DetalleEncargo>,
        detalle_error: impl Into<String>,
    ) -> Self {
        let (inicio, proximo) = calcular_tiempos_sondeo(ahora, intervalo_segundos);
        Self::error(inicio, proximo, encargo, detalle_error)
    }

    /// Carga el latido desde disco. Si el fichero no existe, devuelve `None`.
    pub fn cargar(ruta: &Path) -> Resultado<Option<Self>> {
        if !ruta.exists() {
            return Ok(None);
        }
        let texto = std::fs::read_to_string(ruta).map_err(|causa| Error::Lectura {
            ruta: ruta.to_path_buf(),
            causa,
        })?;
        let latido =
            serde_json::from_str(&texto).map_err(|e| super::lectura::fichero_corrupto(ruta, &e))?;
        Ok(Some(latido))
    }

    /// Guarda el latido en disco en formato JSON con sangría, creando los directorios padre si faltan.
    pub fn guardar(&self, ruta: &Path) -> Resultado<()> {
        if let Some(padre) = ruta.parent() {
            std::fs::create_dir_all(padre).map_err(|causa| Error::Escritura {
                ruta: padre.to_path_buf(),
                causa,
            })?;
        }
        let texto = serde_json::to_string_pretty(self)
            .map_err(|e| Error::Configuracion(mensaje_latido_sin_serializar(&e)))?;
        std::fs::write(ruta, texto).map_err(|causa| Error::Escritura {
            ruta: ruta.to_path_buf(),
            causa,
        })
    }

    /// Intenta guardar el latido en disco; si falla, avisa por la salida de errores de `terminal`
    /// con la advertencia unificada explicando qué falló, la consecuencia y qué hacer.
    pub fn guardar_con_aviso(&self, ruta: &Path, terminal: &dyn Terminal) {
        if let Err(e) = self.guardar(ruta) {
            terminal.avisar(&advertencia_escritura_fallida(&e));
        }
    }
}

/// Redacta la advertencia emitida cuando no se puede persistir el fichero de latido en disco.
///
/// Explica:
/// 1. Qué ha fallado: la escritura en disco del latido.
/// 2. Consecuencia: nadie desde fuera puede saber si el arnés está vivo, ocupado o detenido.
/// 3. Qué hacer: revisar permisos de la carpeta o si el fichero está bloqueado.
pub fn advertencia_escritura_fallida(error: &Error) -> String {
    format!(
        "⚠️ No se pudo escribir el latido del arnés en disco: {error}.\n\
         Consecuencia: mientras no se actualice, los demás agentes y herramientas no podrán \
         distinguir desde fuera si el arnés está vivo, ocupado o detenido.\n\
         Qué hacer: comprueba que la carpeta del arnés exista y tenga permisos de escritura, o que \
         ningún otro proceso mantenga bloqueado el fichero del latido, que es el que nombra el \
         error."
    )
}

/// Formatea una fecha y hora local con el patrón estándar ISO sin microsegundos.
fn formatear_marca_tiempo(dt: DateTime<Local>) -> String {
    dt.format(super::canal::FORMATO_DE_LA_HORA_DEL_LATIDO)
        .to_string()
}

/// Calcula el par (marca de tiempo actual, próximo sondeo) a partir de una marca de tiempo base
/// y un intervalo en segundos.
///
/// **Función pura:** no llama al reloj del sistema, lo que permite probar los cálculos de sondeo
/// con valores deterministas.
fn calcular_tiempos_sondeo(ahora: DateTime<Local>, intervalo_segundos: u64) -> (String, String) {
    // Con sumas comprobadas: un intervalo enorme entraría en pánico, y uno por encima de
    // `i64::MAX` se volvería negativo. Si no se puede sumar, el más lejano posible.
    let proximo = i64::try_from(intervalo_segundos)
        .ok()
        .and_then(chrono::TimeDelta::try_seconds)
        .and_then(|intervalo| ahora.checked_add_signed(intervalo))
        .unwrap_or(DateTime::<Local>::MAX_UTC.with_timezone(&Local));
    (
        formatear_marca_tiempo(ahora),
        formatear_marca_tiempo(proximo),
    )
}

/// El latido no se pudo pasar a JSON.
fn mensaje_latido_sin_serializar(causa: &serde_json::Error) -> String {
    format!("no se pudo serializar el latido: {causa}")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_detalle_de_un_encargo_lleva_su_identificador_y_se_guarda() {
        let encargo = crate::protocolo::Encargo {
            de: "claude.md".into(),
            texto: "Para Programator: uno".into(),
            forma: crate::protocolo::encargo::Forma::Linea,
            latido: Some("2026-11-25 10:00:00".into()),
        };

        let detalle = DetalleEncargo::del_encargo(&encargo);

        assert_eq!(detalle.id.as_deref(), Some(encargo.id().as_str()));
        let json = serde_json::to_value(&detalle).unwrap();
        assert_eq!(json["id"], encargo.id());
        assert_eq!(json["buzon"], "claude.md");
    }

    /// Un latido escrito antes de la 1.2.0, sin identificador, se sigue leyendo.
    #[test]
    fn un_detalle_sin_identificador_se_lee() {
        let detalle: DetalleEncargo =
            serde_json::from_str(r#"{"buzon":"claude.md","resumen":"uno"}"#).unwrap();
        assert_eq!(detalle.id, None);
    }

    #[test]
    fn el_resumen_se_acota_a_ciento_veinte_caracteres_con_elipsis() {
        let justa = "a".repeat(TOPE_CARACTERES_RESUMEN);
        assert_eq!(acotar_resumen(&justa), justa);

        let larga = "á".repeat(TOPE_CARACTERES_RESUMEN + 1);
        let acotada = acotar_resumen(&larga);
        assert_eq!(acotada.chars().count(), TOPE_CARACTERES_RESUMEN - 2);
        assert!(acotada.ends_with("áá…"), "{acotada}");
    }
    use chrono::TimeZone;

    /// Un intervalo enorme en el TOML no hace entrar en pánico la suma de fechas.
    #[test]
    fn un_intervalo_enorme_no_entra_en_panico_ni_va_hacia_atras() {
        for enorme in [100_000_000_000_000_000, u64::MAX] {
            let (inicio, proximo) = calcular_tiempos_sondeo(fecha_fija(10, 0, 0), enorme);
            assert_eq!(inicio, formatear_marca_tiempo(fecha_fija(10, 0, 0)));
            // La fecha más lejana que se puede escribir: el año va con signo, «+262143-…».
            assert!(proximo.starts_with('+'), "{enorme}: {proximo}");
        }
    }

    fn fecha_fija(hora: u32, minuto: u32, segundo: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 9, 25, hora, minuto, segundo)
            .single()
            .expect("fecha válida")
    }

    #[test]
    fn los_tres_estados_se_serializan_y_releen() {
        let t1 = "2026-09-25 10:00:00";
        let t2 = "2026-09-25 10:03:00";

        // 1. Reposo
        let latido_reposo = Latido::reposo(t1, t2);
        let json_reposo = serde_json::to_string(&latido_reposo).unwrap();
        assert!(json_reposo.contains(r#""estado":"reposo""#));
        assert!(!json_reposo.contains("encargo"));
        let recuperado_reposo: Latido = serde_json::from_str(&json_reposo).unwrap();
        assert_eq!(recuperado_reposo, latido_reposo);

        // 2. Atendiendo
        let encargo = DetalleEncargo::nuevo("claude.md", "## Para Programator: E01");
        let latido_atendiendo = Latido::atendiendo(t1, encargo);
        let json_atendiendo = serde_json::to_string(&latido_atendiendo).unwrap();
        assert!(json_atendiendo.contains(r#""estado":"atendiendo""#));
        assert!(json_atendiendo.contains(r#""buzon":"claude.md""#));
        let recuperado_atendiendo: Latido = serde_json::from_str(&json_atendiendo).unwrap();
        assert_eq!(recuperado_atendiendo, latido_atendiendo);

        // 3. Error
        let latido_error = Latido::error(t1, t2, None, "servidor caído");
        let json_error = serde_json::to_string(&latido_error).unwrap();
        assert!(json_error.contains(r#""estado":"error""#));
        assert!(json_error.contains(r#""detalle_error":"servidor caído""#));
        let recuperado_error: Latido = serde_json::from_str(&json_error).unwrap();
        assert_eq!(recuperado_error, latido_error);
    }

    #[test]
    fn el_modelo_cargado_solo_aparece_si_lo_hay_y_los_latidos_de_antes_se_leen() {
        let sin_modelo = Latido::reposo("2026-09-25 10:00:00", "2026-09-25 10:03:00");
        let json = serde_json::to_string(&sin_modelo).unwrap();
        assert!(!json.contains("modelo_cargado"), "{json}");
        let de_antes: Latido = serde_json::from_str(&json).unwrap();
        assert_eq!(de_antes.modelo_cargado, None);

        let con_modelo = sin_modelo.con_modelo_cargado(Some("devstral".into()));
        let json = serde_json::to_string(&con_modelo).unwrap();
        assert!(json.contains(r#""modelo_cargado":"devstral""#), "{json}");
        let releido: Latido = serde_json::from_str(&json).unwrap();
        assert_eq!(releido, con_modelo);
    }

    #[test]
    fn un_latido_sin_encargo_en_curso_es_valido() {
        let latido = Latido::reposo("2026-09-25 10:00:00", "2026-09-25 10:03:00");
        assert!(latido.encargo.is_none());
        assert_eq!(latido.estado, EstadoLatido::Reposo);

        let json = serde_json::to_string_pretty(&latido).unwrap();
        let reparseado: Latido = serde_json::from_str(&json).unwrap();
        assert!(reparseado.encargo.is_none());
    }

    #[test]
    fn escribir_dos_veces_seguidas_deja_el_segundo_y_no_los_concatena() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("latido.json");

        let primer_latido = Latido::atendiendo(
            "2026-09-25 10:00:00",
            DetalleEncargo::nuevo("claude.md", "encargo uno"),
        );
        primer_latido.guardar(&ruta).unwrap();

        let segundo_latido = Latido::reposo("2026-09-25 10:03:00", "2026-09-25 10:06:00");
        segundo_latido.guardar(&ruta).unwrap();

        let leido = Latido::cargar(&ruta).unwrap().expect("debe existir");
        assert_eq!(leido, segundo_latido);
        assert_eq!(leido.estado, EstadoLatido::Reposo);
        assert!(leido.encargo.is_none());
    }

    #[test]
    fn la_ruta_se_crea_si_no_existe() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta_profunda = carpeta
            .path()
            .join(".gestor")
            .join("programator")
            .join("subdirectorio")
            .join("latido.json");

        assert!(!ruta_profunda.parent().unwrap().exists());

        let latido = Latido::reposo("2026-09-25 12:00:00", "2026-09-25 12:03:00");
        latido.guardar(&ruta_profunda).unwrap();

        assert!(ruta_profunda.exists());
        let leido = Latido::cargar(&ruta_profunda).unwrap().expect("cargado");
        assert_eq!(leido, latido);
    }

    #[test]
    fn cargar_inexistente_devuelve_none() {
        let carpeta = tempfile::tempdir().unwrap();
        let resultado = Latido::cargar(&carpeta.path().join("fantasma.json")).unwrap();
        assert!(resultado.is_none());
    }

    #[test]
    fn detalle_encargo_toma_primera_linea_no_vacia() {
        let texto = "\n  \n  ## Para Programator: Tarea 1\nSegunda linea de contexto\n";
        let detalle = DetalleEncargo::nuevo("claude.md", texto);
        assert_eq!(detalle.buzon, "claude.md");
        assert_eq!(detalle.resumen, "## Para Programator: Tarea 1");
    }

    #[test]
    fn detalle_encargo_acota_resumen_si_supera_120_caracteres() {
        let linea_larga = "A".repeat(150);
        let detalle = DetalleEncargo::nuevo("claude.md", &linea_larga);
        assert_eq!(detalle.resumen.chars().count(), 118); // 117 chars + '…'
        assert!(detalle.resumen.ends_with('…'));
    }

    #[test]
    fn calcular_tiempos_sondeo_es_determinista_sin_reloj() {
        let ahora = fecha_fija(14, 0, 0);
        let (inicio, proximo) = calcular_tiempos_sondeo(ahora, 180);
        assert_eq!(inicio, "2026-09-25 14:00:00");
        assert_eq!(proximo, "2026-09-25 14:03:00");
    }

    #[test]
    fn constructores_semanticos_usan_reloj_inyectado_correctamente() {
        let ahora = fecha_fija(10, 30, 0);
        let latido_reposo = Latido::en_reposo(ahora, 120);
        assert_eq!(latido_reposo.estado, EstadoLatido::Reposo);
        assert_eq!(latido_reposo.marca_tiempo, "2026-09-25 10:30:00");
        assert_eq!(
            latido_reposo.proximo_sondeo.as_deref(),
            Some("2026-09-25 10:32:00")
        );

        let detalle = DetalleEncargo::nuevo("claude.md", "encargo");
        let latido_atendiendo = Latido::en_atencion(ahora, detalle);
        assert_eq!(latido_atendiendo.estado, EstadoLatido::Atendiendo);
        // Uso real del 26/11: mientras se atiende no se sondea hasta terminar la llamada en curso,
        // así que el latido no promete ninguna hora.
        assert_eq!(latido_atendiendo.proximo_sondeo, None);
        let json = serde_json::to_string(&latido_atendiendo).unwrap();
        assert!(!json.contains("proximo_sondeo"), "{json}");

        let latido_error = Latido::en_error(ahora, 300, None, "fallo de red");
        assert_eq!(latido_error.estado, EstadoLatido::Error);
        assert_eq!(
            latido_error.proximo_sondeo.as_deref(),
            Some("2026-09-25 10:35:00")
        );
        assert_eq!(latido_error.detalle_error.as_deref(), Some("fallo de red"));
    }

    #[test]
    fn advertencia_escritura_fallida_explica_fallo_consecuencia_y_accion() {
        let fallo = Error::Escritura {
            ruta: std::path::PathBuf::from(".gestor/agente/latido.json"),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "acceso denegado"),
        };
        let texto = advertencia_escritura_fallida(&fallo);
        assert!(texto.starts_with("⚠️ No se pudo escribir el latido"));
        assert!(texto.contains("Consecuencia:"));
        assert!(texto.contains("no podrán distinguir desde fuera si el arnés está vivo"));
        // El fichero lo nombra el error, que lleva la ruta configurada.
        assert!(texto.contains(".gestor/agente/latido.json"), "{texto}");
        assert!(!texto.contains("Programator"), "{texto}");
        assert!(texto.contains("Qué hacer:"));
        assert!(texto.contains("permisos de escritura"));
    }
}
