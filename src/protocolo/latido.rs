//! Estado comprobable del arnés en cada sondeo.
//!
//! **Por qué este módulo.** El informe de NatureLand (incidencia C1) documentó el coste de un
//! arnés mudo: a las 10:35 quien dirigía concluyó que Programator estaba parado porque `lectura.json`
//! no se tocaba desde las 09:34. Era falso: los ficheros solo cambiaban cuando el arnés actuaba, no
//! cuando sondeaba. El duplicado resultante hubo que retirarlo a mano doce minutos después.
//!
//! Este módulo define el [`Latido`], un fichero que se escribe en **cada sondeo** —actúe el arnés o
//! no— declarando su estado (`reposo`, `atendiendo` o `error`), el encargo en curso si lo hay, y
//! cuándo será el próximo sondeo.

use crate::error::{Error, Resultado};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// El estado en el que se encuentra el arnés en el momento del sondeo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// Longitud máxima de la línea de resumen del encargo en el latido para mantenerlo legible.
const TOPE_CARACTERES_RESUMEN: usize = 120;
/// Caracteres a conservar antes de añadir los puntos suspensivos ("…").
const CARACTERES_RECORTE_RESUMEN: usize = TOPE_CARACTERES_RESUMEN - 3;
/// Texto por defecto cuando el encargo no contiene líneas legibles.
const TEXTO_ENCARGO_VACIO: &str = "encargo sin texto";

impl DetalleEncargo {
    /// Construye el detalle a partir del buzón y del texto íntegro del encargo.
    pub fn nuevo(buzon: &str, texto_encargo: &str) -> Self {
        let primera_linea = texto_encargo
            .lines()
            .map(|l| l.trim())
            .find(|l| !l.is_empty())
            .unwrap_or(TEXTO_ENCARGO_VACIO);

        // Si la primera línea supera el tope, se trunca con puntos suspensivos para
        // mantener el fichero de latido legible de un vistazo por una persona o agente.
        let resumen = if primera_linea.chars().count() > TOPE_CARACTERES_RESUMEN {
            let acotada: String = primera_linea
                .chars()
                .take(CARACTERES_RECORTE_RESUMEN)
                .collect();
            format!("{acotada}…")
        } else {
            primera_linea.to_string()
        };

        Self {
            buzon: buzon.to_string(),
            resumen,
        }
    }
}

/// Foto del estado del arnés en un momento concreto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Latido {
    /// Marca de tiempo de este latido.
    pub marca_tiempo: String,
    /// Estado actual del arnés.
    pub estado: EstadoLatido,
    /// Encargo que se está atendiendo, si lo hay.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encargo: Option<DetalleEncargo>,
    /// Momento estimado en que tendrá lugar el próximo sondeo.
    pub proximo_sondeo: String,
    /// Detalle adicional del error, si el estado es `Error`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detalle_error: Option<String>,
}

impl Latido {
    /// Crea un latido en estado de reposo, sin encargo activo.
    fn reposo(marca_tiempo: impl Into<String>, proximo_sondeo: impl Into<String>) -> Self {
        Self {
            marca_tiempo: marca_tiempo.into(),
            estado: EstadoLatido::Reposo,
            encargo: None,
            proximo_sondeo: proximo_sondeo.into(),
            detalle_error: None,
        }
    }

    /// Crea un latido en estado de atención, con el encargo en curso.
    fn atendiendo(
        marca_tiempo: impl Into<String>,
        proximo_sondeo: impl Into<String>,
        encargo: DetalleEncargo,
    ) -> Self {
        Self {
            marca_tiempo: marca_tiempo.into(),
            estado: EstadoLatido::Atendiendo,
            encargo: Some(encargo),
            proximo_sondeo: proximo_sondeo.into(),
            detalle_error: None,
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
            proximo_sondeo: proximo_sondeo.into(),
            detalle_error: Some(detalle_error.into()),
        }
    }

    /// Crea un latido en estado de reposo calculando los tiempos a partir del momento actual
    /// y del intervalo en segundos del ciclo.
    pub fn en_reposo(ahora: DateTime<Local>, intervalo_segundos: u64) -> Self {
        let (inicio, proximo) = calcular_tiempos_sondeo(ahora, intervalo_segundos);
        Self::reposo(inicio, proximo)
    }

    /// Crea un latido en estado de atención calculando los tiempos a partir del momento actual
    /// y del intervalo en segundos del ciclo.
    pub fn en_atencion(
        ahora: DateTime<Local>,
        intervalo_segundos: u64,
        encargo: DetalleEncargo,
    ) -> Self {
        let (inicio, proximo) = calcular_tiempos_sondeo(ahora, intervalo_segundos);
        Self::atendiendo(inicio, proximo, encargo)
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
        let latido = serde_json::from_str(&texto)
            .map_err(|e| Error::Configuracion(format!("{ruta:?} está corrupto: {e}")))?;
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
            .map_err(|e| Error::Configuracion(format!("no se pudo serializar el latido: {e}")))?;
        std::fs::write(ruta, texto).map_err(|causa| Error::Escritura {
            ruta: ruta.to_path_buf(),
            causa,
        })
    }

    /// Intenta guardar el latido en disco; si falla, emite por `stderr` la advertencia unificada
    /// explicando qué falló, la consecuencia y qué hacer.
    pub fn guardar_con_aviso(&self, ruta: &Path) {
        if let Err(e) = self.guardar(ruta) {
            eprintln!("{}", advertencia_escritura_fallida(&e));
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
         distinguir desde fuera si Programator está vivo, ocupado o detenido.\n\
         Qué hacer: comprueba que la carpeta del arnés exista y tenga permisos de escritura, o que \
         ningún otro proceso mantenga bloqueado «latido.json»."
    )
}

/// Formatea una fecha y hora local con el patrón estándar ISO sin microsegundos.
fn formatear_marca_tiempo(dt: DateTime<Local>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Calcula el par (marca de tiempo actual, próximo sondeo) a partir de una marca de tiempo base
/// y un intervalo en segundos.
///
/// **Función pura:** no llama al reloj del sistema, lo que permite probar los cálculos de sondeo
/// con valores deterministas.
fn calcular_tiempos_sondeo(ahora: DateTime<Local>, intervalo_segundos: u64) -> (String, String) {
    let proximo = ahora + chrono::Duration::seconds(intervalo_segundos as i64);
    (
        formatear_marca_tiempo(ahora),
        formatear_marca_tiempo(proximo),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::TimeZone;

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
        let latido_atendiendo = Latido::atendiendo(t1, t2, encargo);
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
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("latido.json");

        let primer_latido = Latido::atendiendo(
            "2026-09-25 10:00:00",
            "2026-09-25 10:03:00",
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
        let dir = tempfile::tempdir().unwrap();
        let ruta_profunda = dir
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
        let dir = tempfile::tempdir().unwrap();
        let resultado = Latido::cargar(&dir.path().join("fantasma.json")).unwrap();
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
        assert_eq!(latido_reposo.proximo_sondeo, "2026-09-25 10:32:00");

        let detalle = DetalleEncargo::nuevo("claude.md", "encargo");
        let latido_atendiendo = Latido::en_atencion(ahora, 60, detalle);
        assert_eq!(latido_atendiendo.estado, EstadoLatido::Atendiendo);
        assert_eq!(latido_atendiendo.proximo_sondeo, "2026-09-25 10:31:00");

        let latido_error = Latido::en_error(ahora, 300, None, "fallo de red");
        assert_eq!(latido_error.estado, EstadoLatido::Error);
        assert_eq!(latido_error.proximo_sondeo, "2026-09-25 10:35:00");
        assert_eq!(latido_error.detalle_error.as_deref(), Some("fallo de red"));
    }

    #[test]
    fn advertencia_escritura_fallida_explica_fallo_consecuencia_y_accion() {
        let err = Error::Escritura {
            ruta: std::path::PathBuf::from(".gestor/agente/latido.json"),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "acceso denegado"),
        };
        let texto = advertencia_escritura_fallida(&err);
        assert!(texto.starts_with("⚠️ No se pudo escribir el latido"));
        assert!(texto.contains("Consecuencia:"));
        assert!(texto.contains("no podrán distinguir desde fuera si Programator está vivo"));
        assert!(texto.contains("Qué hacer:"));
        assert!(texto.contains("permisos de escritura"));
    }
}
