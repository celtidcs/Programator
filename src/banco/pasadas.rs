//! La fase 1 del banco, en disco: lo que se midió al pasar cada encargo.
//!
//! **Por qué existe.** La primera tanda real (11/11/2026) pasó 51 encargos en casi cinco horas de
//! GPU y se perdió entera porque falló la preparación de la máquina de pruebas: la duración, el
//! consumo y si cada pasada llegó a tiempo solo vivían en memoria. Ahora se escriben en
//! `pasadas.toml` en cuanto termina cada pasada, y `--banco-comprobar` hace la fase 2 a partir de
//! ahí, sin volver a cargar el modelo.
//!
//! Lo que ya está en la carpeta de trabajo de cada pasada (la respuesta en el canal y las entregas)
//! no se copia aquí: se vuelve a leer de allí con [`Recogida::de_la_carpeta`], igual que al pasarla.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::ejecucion::{carpeta_del_encargo, ConsumoAcumulado, Recogida};
use super::ficha::Encargo;
use super::resultados::DatosDeTanda;
use super::tanda::Pasado;
use crate::error::{Error, Resultado};
use crate::ficheros::{escribir, leer};
use crate::motor::Consumo;

/// El fichero, dentro de la carpeta de la tanda.
pub const NOMBRE_PASADAS: &str = "pasadas.toml";

/// Los datos de la tanda y las pasadas hechas hasta ahora, en el orden en que se pasaron.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasadasDeTanda {
    pub tanda: DatosDeTanda,
    #[serde(default)]
    pub pasadas: Vec<PasadaGuardada>,
}

/// Lo que se midió de una pasada y no queda en su carpeta de trabajo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasadaGuardada {
    pub encargo: String,
    pub semilla: i64,
    pub a_tiempo: bool,
    pub segundos: f64,
    pub respuestas: usize,
    pub respuestas_sin_consumo: usize,
    pub tokens_entrada: Option<u64>,
    pub tokens_en_cache: Option<u64>,
    pub tokens_generados: Option<u64>,
    pub ms_entrada: Option<f64>,
    pub ms_generacion: Option<f64>,
    /// Los nombres de los ficheros entregados. Solo para quien lea el fichero: al recuperar la
    /// pasada se vuelven a buscar en su carpeta.
    #[serde(default)]
    pub entregas: Vec<String>,
}

impl PasadaGuardada {
    pub fn de(pasado: &Pasado) -> Self {
        let recogida = &pasado.recogida;
        let total = &recogida.consumo.total;
        Self {
            encargo: pasado.encargo.ficha.id.clone(),
            semilla: pasado.semilla,
            a_tiempo: recogida.a_tiempo,
            segundos: recogida.duracion.as_secs_f64(),
            respuestas: recogida.consumo.respuestas,
            respuestas_sin_consumo: recogida.consumo.respuestas_sin_consumo,
            tokens_entrada: total.tokens_entrada,
            tokens_en_cache: total.tokens_en_cache,
            tokens_generados: total.tokens_generados,
            ms_entrada: total.ms_entrada,
            ms_generacion: total.ms_generacion,
            entregas: recogida
                .entregas
                .iter()
                .filter_map(|r| r.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect(),
        }
    }

    fn consumo(&self) -> ConsumoAcumulado {
        ConsumoAcumulado {
            total: Consumo {
                tokens_entrada: self.tokens_entrada,
                tokens_en_cache: self.tokens_en_cache,
                tokens_generados: self.tokens_generados,
                ms_entrada: self.ms_entrada,
                ms_generacion: self.ms_generacion,
            },
            respuestas: self.respuestas,
            respuestas_sin_consumo: self.respuestas_sin_consumo,
        }
    }
}

pub fn componer_toml(pasadas: &PasadasDeTanda) -> Resultado<String> {
    toml::to_string(pasadas)
        .map_err(|e| Error::Banco(format!("no se pudieron componer las pasadas: {e}")))
}

pub fn leer_toml(texto: &str) -> Resultado<PasadasDeTanda> {
    toml::from_str(texto).map_err(|e| Error::Banco(format!("las pasadas no se pueden leer: {e}")))
}

/// Reescribe `pasadas.toml` entero. Se llama tras cada pasada: si el banco se para, queda todo lo
/// pasado hasta ese momento.
pub fn guardar(pasadas: &PasadasDeTanda, carpeta_de_la_tanda: &Path) -> Resultado<()> {
    escribir(
        &carpeta_de_la_tanda.join(NOMBRE_PASADAS),
        &componer_toml(pasadas)?,
    )
}

/// Lee `pasadas.toml` de una tanda. Si no está, lo dice: las tandas anteriores al 12/11/2026 no lo
/// tienen y no se pueden comprobar aparte.
pub fn cargar(carpeta_de_la_tanda: &Path) -> Resultado<PasadasDeTanda> {
    let ruta = carpeta_de_la_tanda.join(NOMBRE_PASADAS);
    if !ruta.is_file() {
        return Err(Error::Banco(format!(
            "no hay {} en «{}»: no es la carpeta de una tanda, o es de antes de que el banco \
             guardara cada pasada (12/11/2026) y hay que repetirla entera",
            NOMBRE_PASADAS,
            carpeta_de_la_tanda.display()
        )));
    }
    leer_toml(&leer(&ruta)?)
}

/// Rehace las pasadas guardadas a partir de sus carpetas de trabajo, tal como las dejó la fase 1.
pub fn recuperar<'a>(
    guardadas: &[PasadaGuardada],
    encargos: &'a [Encargo],
    carpeta_de_trabajo: &Path,
    agente: &str,
) -> Resultado<Vec<Pasado<'a>>> {
    guardadas
        .iter()
        .map(|guardada| {
            let encargo = encargos
                .iter()
                .find(|e| e.ficha.id == guardada.encargo)
                .ok_or_else(|| {
                    Error::Banco(format!(
                        "la pasada de {} no tiene encargo en el banco: ¿se ha borrado o renombrado \
                         su carpeta?",
                        guardada.encargo
                    ))
                })?;
            let carpeta =
                carpeta_del_encargo(carpeta_de_trabajo, &encargo.ficha.id, guardada.semilla);
            if !carpeta.is_dir() {
                return Err(Error::Banco(format!(
                    "falta la carpeta de trabajo «{}» de una pasada guardada",
                    carpeta.display()
                )));
            }
            // El fichero se puede tocar a mano: un valor imposible es un error, no un pánico.
            let duracion = Duration::try_from_secs_f64(guardada.segundos).map_err(|_| {
                Error::Banco(format!(
                    "la pasada de {} con la semilla {} tiene una duración imposible: {}",
                    guardada.encargo, guardada.semilla, guardada.segundos
                ))
            })?;
            let recogida = Recogida::de_la_carpeta(
                &carpeta,
                agente,
                guardada.a_tiempo,
                guardada.consumo(),
                duracion,
            )?;
            Ok(Pasado {
                encargo,
                semilla: guardada.semilla,
                carpeta,
                recogida,
            })
        })
        .collect()
}
