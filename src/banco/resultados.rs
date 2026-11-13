//! Los resultados de una tanda: un TOML con todas las cifras y un informe en Markdown para leer.
//!
//! El TOML lleva lo necesario para repetir la tanda (modelo y su huella, motor, versión,
//! configuración, fecha) y, por cada encargo y semilla, la nota, el consumo y en las auditorías los
//! aciertos y los inventados por separado. También guarda, por cada fila de desempeño
//! (`FilaDeDesempeno`), la nota, la velocidad y los inventados: es lo que copia el registro de
//! modelos para saber en qué trabajo puntúa mejor cada modelo, sin volver a medir.
//!
//! Componer y leer son funciones puras. [`rehacer_informe`] es la capa fina que vuelve a leer las
//! rúbricas que ha rellenado el agente que dirige y reescribe los dos ficheros.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::ficha::{ClaseDeUso, Familia};
use super::mensajes;
use super::puntuacion::{resumir, Nota, Resumen, NOTA_MAXIMA};
use super::rubrica::{leer_fichero, NotaDeRubrica};
use crate::error::{Error, Resultado};
use crate::ficheros::{escribir, leer};

/// El fichero con todas las cifras, dentro de la carpeta de la tanda.
pub const NOMBRE_RESULTADOS: &str = "resultados.toml";
/// El informe para leer, junto al anterior.
pub const NOMBRE_INFORME: &str = "informe.md";
const MS_POR_SEGUNDO: f64 = 1000.0;
/// Cuántos caracteres de la huella SHA-256 se muestran en el informe; el TOML la guarda entera.
const CARACTERES_DE_HUELLA: usize = 12;

/// Todo lo de una tanda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultadosDeTanda {
    pub tanda: DatosDeTanda,
    /// Las filas de desempeño, en su orden. Se recalculan con
    /// [`calcular_clases`].
    #[serde(default)]
    pub clases: Vec<ResumenDeClase>,
    #[serde(default)]
    pub encargos: Vec<ResultadoDeEncargo>,
}

/// Lo que hace falta para repetir la tanda, y lo que costó cargar el modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatosDeTanda {
    pub modelo: String,
    pub huella_sha256: String,
    pub compilacion_motor: String,
    pub version_programator: String,
    pub fecha: String,
    pub semillas: Vec<i64>,
    pub temperatura: f64,
    pub margen_lineas: usize,
    pub tiempo_maximo_encargo_segundos: u64,
    /// Dónde se ejecutaron las pruebas ocultas: `local`, este equipo. Las tandas anteriores al
    /// 13/11/2026 pueden nombrar la máquina aparte que se usaba entonces. Sin ella, no se
    /// ejecutó código.
    pub maquina_de_comprobacion: Option<String>,
    pub capas_en_gpu: Option<u32>,
    pub vram_ocupada_mib: Option<u64>,
    pub segundos_de_carga: Option<f64>,
    /// Lo del motor que cambia la respuesta. Las tandas pasadas antes del 12/11/2026 no lo
    /// guardan.
    #[serde(default)]
    pub ajustes_del_motor: Option<AjustesDelMotor>,
    /// La tarjeta gráfica en que se midió, con el nombre que da el sistema. Con otra tarjeta el
    /// mismo modelo da otras cifras. Las tandas anteriores al 12/11/2026 no la guardan.
    #[serde(default)]
    pub tarjeta: Option<String>,
    /// El sistema operativo donde se comprobó, cuando se comprobó en este equipo: las notas de
    /// oficio en otro sistema pueden salir distintas. Las tandas anteriores al 12/11/2026 no lo
    /// guardan.
    #[serde(default)]
    pub sistema_de_comprobacion: Option<String>,
    /// Lo que la comprobación local dejó fuera de la carpeta de la tanda, ya redactado. Las tandas
    /// anteriores al 12/11/2026 no lo guardan.
    #[serde(default)]
    pub rastro_fuera: Vec<String>,
    /// Cómo se aisló la comprobación local, como se escribe en `banco.toml`. Las tandas
    /// anteriores al 12/11/2026 no lo guardan.
    #[serde(default)]
    pub aislamiento: Option<String>,
}

/// Lo de `programator.toml` que cambia lo que contesta el modelo, y que por eso tiene que
/// coincidir para que dos tandas sean repetición una de otra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AjustesDelMotor {
    pub contexto: u32,
    pub cache_kv: String,
    pub paralelo: u32,
    pub empezar_en_frio: bool,
    pub top_k: u32,
    pub top_p: f64,
    pub min_p: f64,
    pub penalizacion_repeticion: f64,
}

/// Un encargo, con todas sus semillas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultadoDeEncargo {
    pub id: String,
    /// El encargo con el que cuenta: el propio, o aquel del que es variante.
    pub cuenta_como: String,
    pub titulo: String,
    pub familia: Familia,
    pub clase: Option<ClaseDeUso>,
    #[serde(default)]
    pub semillas: Vec<ResultadoDeSemilla>,
}

/// En qué quedó la nota de una semilla.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoDeNota {
    Evaluada,
    /// No se pudo comprobar. Nunca cuenta como 0.
    #[default]
    NoEvaluado,
    /// La puntúa el agente que dirige y aún no lo ha hecho.
    Pendiente,
}

/// Un encargo pasado con una semilla. Los campos que no aplican a su forma de puntuar faltan.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResultadoDeSemilla {
    pub semilla: i64,
    pub estado: EstadoDeNota,
    pub nota: Option<f64>,
    /// Por qué no se evaluó, o lo que dijo la compilación que falló.
    pub detalle: Option<String>,
    /// El fichero de rúbrica, relativo a la carpeta de la tanda.
    pub rubrica: Option<String>,
    pub comprobaciones_superadas: Option<usize>,
    pub comprobaciones_total: Option<usize>,
    pub sembrados: Option<usize>,
    pub aciertos: Option<usize>,
    pub inventados: Option<usize>,
    pub repetidos: Option<usize>,
    pub sin_formato: Option<usize>,
    pub segundos: f64,
    pub tokens_entrada: Option<u64>,
    pub tokens_en_cache: Option<u64>,
    pub tokens_generados: Option<u64>,
    pub ms_entrada: Option<f64>,
    pub ms_generacion: Option<f64>,
    /// Respuestas del motor que no trajeron su consumo: si hay alguna, las cifras se quedan cortas.
    pub respuestas_sin_consumo: usize,
    /// Los destinos de red que intentó alcanzar el código del modelo al comprobarlo. Las tandas de
    /// antes del detector de red no lo traen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intentos_de_red: Vec<String>,
    /// Con qué se puntuó un encargo de código: sus pruebas ocultas, o su rúbrica leyendo el código
    /// porque las pruebas no dieron nota. Las dos notas no son comparables. Si no entregó, lo dice
    /// también. Falta en los demás encargos, en los de código que no se evaluaron y en las tandas
    /// de antes del 13/11/2026 (en estas, también en los que no entregaron).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub puntuada_con: Option<PuntuadaCon>,
}

/// Con qué se puntuó la semilla de un encargo de código.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PuntuadaCon {
    /// Ejecutando sus pruebas ocultas: una nota objetiva.
    PruebasOcultas,
    /// Con la rúbrica, leyendo el código sin ejecutarlo: la nota de quien dirige.
    Rubrica,
    /// No entregó a tiempo: un 0, el mismo con pruebas o con rúbrica, así que se compara con
    /// cualquiera de las dos.
    SinEntrega,
}

impl ResultadoDeSemilla {
    /// La nota para resumir; `None` si está pendiente.
    pub fn nota_para_resumir(&self) -> Option<Nota> {
        match (self.estado, self.nota) {
            (EstadoDeNota::Evaluada, Some(nota)) => Some(Nota::Evaluada(nota)),
            (EstadoDeNota::Pendiente, _) => None,
            _ => Some(Nota::NoEvaluado),
        }
    }

    /// Pone la nota que dice el fichero de rúbrica.
    pub(crate) fn aplicar_rubrica(&mut self, nota: NotaDeRubrica) {
        match nota {
            NotaDeRubrica::Puntuada(Nota::Evaluada(valor)) => {
                self.estado = EstadoDeNota::Evaluada;
                self.nota = Some(valor);
            }
            NotaDeRubrica::Puntuada(Nota::NoEvaluado) => {
                self.estado = EstadoDeNota::NoEvaluado;
                self.nota = None;
            }
            NotaDeRubrica::Pendiente => {
                self.estado = EstadoDeNota::Pendiente;
                self.nota = None;
            }
        }
    }
}

/// Las filas de desempeño: las clases de encargo que el registro guarda y la guía cuenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilaDeDesempeno {
    Formas,
    /// Las auditorías con defectos sembrados, tomadas juntas.
    AuditoriasJuntas,
    FuncionAcotada,
    Validaciones,
    Diagnostico,
    FicheroLimpio,
}

/// Las filas, en el orden en que salen en el informe, el registro y la guía.
pub const FILAS_DE_DESEMPENO: [FilaDeDesempeno; 6] = [
    FilaDeDesempeno::Formas,
    FilaDeDesempeno::AuditoriasJuntas,
    FilaDeDesempeno::FuncionAcotada,
    FilaDeDesempeno::Validaciones,
    FilaDeDesempeno::Diagnostico,
    FilaDeDesempeno::FicheroLimpio,
];

impl FilaDeDesempeno {
    /// Las clases de encargo que entran en la fila.
    pub fn clases(self) -> &'static [ClaseDeUso] {
        match self {
            Self::Formas => &[ClaseDeUso::Formas],
            Self::AuditoriasJuntas => &[ClaseDeUso::Formas, ClaseDeUso::Validaciones],
            Self::FuncionAcotada => &[ClaseDeUso::FuncionAcotada],
            Self::Validaciones => &[ClaseDeUso::Validaciones],
            Self::Diagnostico => &[ClaseDeUso::Diagnostico],
            Self::FicheroLimpio => &[ClaseDeUso::FicheroLimpio],
        }
    }

    /// El texto de la fila, el mismo en el informe y en la guía.
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Formas => "Auditar buscando formas",
            Self::AuditoriasJuntas => "Auditar código ajeno, tomando las auditorías juntas",
            Self::FuncionAcotada => "Función acotada",
            Self::Validaciones => "Auditar buscando validaciones ausentes o errores de significado",
            Self::Diagnostico => "Diagnosticar la causa de un fallo",
            Self::FicheroLimpio => "Auditar un fichero que crees limpio, para confirmarlo",
        }
    }
}

/// Una fila de desempeño medida en esta tanda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumenDeClase {
    pub fila: FilaDeDesempeno,
    /// La media de las notas de sus encargos; falta si no hay ninguna o queda alguna pendiente.
    pub nota_media: Option<f64>,
    pub tokens_por_segundo: Option<f64>,
    pub inventados: usize,
    pub pendientes: usize,
}

/// Cómo quedó un encargo, juntando sus variantes y sus semillas.
#[derive(Debug, Clone, PartialEq)]
pub struct ResumenDeEncargo<'a> {
    pub id: &'a str,
    pub titulo: &'a str,
    pub familia: Familia,
    pub clase: Option<ClaseDeUso>,
    /// `None` mientras quede alguna semilla pendiente.
    pub resumen: Option<Resumen>,
    pub pendientes: usize,
    pub superadas: Option<(usize, usize)>,
    pub aciertos: Option<(usize, usize)>,
    pub inventados: usize,
    /// Por qué no se evaluaron las semillas que no se evaluaron, sin repetir. Sin esto, un
    /// «no evaluado» no dice si la entrega faltó, no compiló o faltaba algo en el equipo.
    pub motivos_no_evaluado: Vec<&'a str>,
    /// Cada semilla cuyo código intentó usar la red, con los destinos.
    pub intentos_de_red: Vec<(i64, Vec<String>)>,
    /// Las semillas de código puntuadas con la rúbrica, leyendo el código, y no con las pruebas.
    pub por_rubrica: usize,
}

fn sumar_opciones(valores: impl Iterator<Item = Option<usize>>) -> Option<usize> {
    valores.fold(None, |total, valor| match (total, valor) {
        (None, None) => None,
        (total, valor) => Some(total.unwrap_or(0) + valor.unwrap_or(0)),
    })
}

/// Los encargos agrupados por el que cuentan, en el orden en que aparecen.
pub fn resumir_encargos(encargos: &[ResultadoDeEncargo]) -> Vec<ResumenDeEncargo<'_>> {
    let mut grupos: Vec<(&str, Vec<&ResultadoDeEncargo>)> = Vec::new();
    for encargo in encargos {
        match grupos.iter_mut().find(|(id, _)| *id == encargo.cuenta_como) {
            Some((_, grupo)) => grupo.push(encargo),
            None => grupos.push((&encargo.cuenta_como, vec![encargo])),
        }
    }
    grupos
        .into_iter()
        .map(|(id, grupo)| {
            let semillas: Vec<&ResultadoDeSemilla> =
                grupo.iter().flat_map(|e| e.semillas.iter()).collect();
            let notas: Vec<Option<Nota>> = semillas.iter().map(|s| s.nota_para_resumir()).collect();
            let pendientes = notas.iter().filter(|n| n.is_none()).count();
            let resumen = (pendientes == 0)
                .then(|| resumir(&notas.iter().flatten().copied().collect::<Vec<_>>()));
            let par = |a: Option<usize>, b: Option<usize>| a.zip(b);
            let primero = grupo[0];
            ResumenDeEncargo {
                id,
                titulo: &primero.titulo,
                familia: primero.familia,
                clase: primero.clase,
                resumen,
                pendientes,
                superadas: par(
                    sumar_opciones(semillas.iter().map(|s| s.comprobaciones_superadas)),
                    sumar_opciones(semillas.iter().map(|s| s.comprobaciones_total)),
                ),
                aciertos: par(
                    sumar_opciones(semillas.iter().map(|s| s.aciertos)),
                    sumar_opciones(semillas.iter().map(|s| s.sembrados)),
                ),
                inventados: semillas.iter().filter_map(|s| s.inventados).sum(),
                motivos_no_evaluado: motivos_no_evaluado(&semillas),
                intentos_de_red: semillas
                    .iter()
                    .filter(|s| !s.intentos_de_red.is_empty())
                    .map(|s| (s.semilla, s.intentos_de_red.clone()))
                    .collect(),
                por_rubrica: semillas
                    .iter()
                    .filter(|s| s.puntuada_con == Some(PuntuadaCon::Rubrica))
                    .count(),
            }
        })
        .collect()
}

fn motivos_no_evaluado<'a>(semillas: &[&'a ResultadoDeSemilla]) -> Vec<&'a str> {
    let mut motivos: Vec<&str> = Vec::new();
    for semilla in semillas {
        if semilla.estado != EstadoDeNota::NoEvaluado {
            continue;
        }
        if let Some(motivo) = semilla.detalle.as_deref() {
            if !motivos.contains(&motivo) {
                motivos.push(motivo);
            }
        }
    }
    motivos
}

/// La media de las medias de unos encargos; `None` si no hay ninguna o queda alguno pendiente.
fn media_de_encargos<'a>(resumenes: impl Iterator<Item = &'a ResumenDeEncargo<'a>>) -> Option<f64> {
    let mut medias = Vec::new();
    for resumen in resumenes {
        match resumen.resumen {
            None => return None,
            Some(Resumen { media: Some(m), .. }) => medias.push(m),
            Some(_) => {}
        }
    }
    (!medias.is_empty()).then(|| medias.iter().sum::<f64>() / medias.len() as f64)
}

/// Las familias del banco, en el orden del informe.
pub const FAMILIAS: [Familia; 3] = [Familia::Oficio, Familia::Criterio, Familia::Uso];

/// La nota media de una familia: la media de las de sus encargos. `None` si no tiene ninguno
/// evaluado o le queda alguno pendiente.
pub fn nota_de_familia(resumenes: &[ResumenDeEncargo], familia: Familia) -> Option<f64> {
    media_de_encargos(resumenes.iter().filter(|r| r.familia == familia))
}

/// Tokens generados por segundo de generación, sumando todas las semillas que lo midieron.
pub(crate) fn velocidad_de_generacion<'a>(
    semillas: impl Iterator<Item = &'a ResultadoDeSemilla>,
) -> Option<f64> {
    velocidad(semillas.filter_map(|s| Some((s.tokens_generados?, s.ms_generacion?))))
}

/// Tokens de entrada procesados (sin los que estaban en caché) por segundo.
pub(crate) fn velocidad_de_lectura<'a>(
    semillas: impl Iterator<Item = &'a ResultadoDeSemilla>,
) -> Option<f64> {
    velocidad(semillas.filter_map(|s| {
        let procesados = s
            .tokens_entrada?
            .saturating_sub(s.tokens_en_cache.unwrap_or(0));
        Some((procesados, s.ms_entrada?))
    }))
}

fn velocidad(medidas: impl Iterator<Item = (u64, f64)>) -> Option<f64> {
    let (tokens, ms) = medidas.fold((0u64, 0.0f64), |(t, m), (tokens, ms)| (t + tokens, m + ms));
    (ms > 0.0).then(|| tokens as f64 * MS_POR_SEGUNDO / ms)
}

/// Las filas de desempeño, calculadas a partir de los encargos.
pub fn calcular_clases(encargos: &[ResultadoDeEncargo]) -> Vec<ResumenDeClase> {
    let resumenes = resumir_encargos(encargos);
    FILAS_DE_DESEMPENO
        .iter()
        .map(|&fila| {
            let en_fila =
                |clase: Option<ClaseDeUso>| clase.is_some_and(|c| fila.clases().contains(&c));
            let de_la_fila: Vec<&ResumenDeEncargo> =
                resumenes.iter().filter(|r| en_fila(r.clase)).collect();
            ResumenDeClase {
                fila,
                nota_media: media_de_encargos(de_la_fila.iter().copied()),
                tokens_por_segundo: velocidad_de_generacion(
                    encargos
                        .iter()
                        .filter(|e| en_fila(e.clase))
                        .flat_map(|e| e.semillas.iter()),
                ),
                inventados: de_la_fila.iter().map(|r| r.inventados).sum(),
                pendientes: de_la_fila.iter().map(|r| r.pendientes).sum(),
            }
        })
        .collect()
}

impl ResultadosDeTanda {
    /// Cuántas semillas quedan por puntuar.
    pub fn pendientes(&self) -> usize {
        self.encargos
            .iter()
            .flat_map(|e| e.semillas.iter())
            .filter(|s| s.estado == EstadoDeNota::Pendiente)
            .count()
    }
}

pub fn componer_toml(resultados: &ResultadosDeTanda) -> Resultado<String> {
    toml::to_string(resultados)
        .map_err(|e| Error::Banco(format!("no se pudieron componer los resultados: {e}")))
}

pub fn leer_toml(texto: &str) -> Resultado<ResultadosDeTanda> {
    toml::from_str(texto)
        .map_err(|e| Error::Banco(format!("los resultados no se pueden leer: {e}")))
}

/// Un número para leer, con coma decimal.
pub(crate) fn decimal(valor: f64, decimales: usize) -> String {
    format!("{valor:.decimales$}").replace('.', ",")
}

pub(crate) fn nombre_de_familia(familia: Familia) -> &'static str {
    match familia {
        Familia::Oficio => "oficio",
        Familia::Criterio => "criterio",
        Familia::Uso => "uso",
    }
}

/// La cabecera de una columna de notas, con la escala: «2,8» no dice nada sin saber sobre cuánto.
fn cabecera_de_nota(nombre: &str) -> String {
    format!("{nombre} (sobre {NOTA_MAXIMA})")
}

fn texto_de_nota(nota: Option<f64>, pendiente: bool) -> String {
    match (pendiente, nota) {
        (true, _) => "pendiente".into(),
        (false, Some(n)) => decimal(n, 1),
        (false, None) => "no evaluado".into(),
    }
}

/// El informe en Markdown.
/// Dónde se ejecutaron las pruebas ocultas y, si fue en este equipo, lo que quedó fuera.
/// Cómo se aisló la comprobación local, para el informe. Sin dato, nada: la tanda es anterior.
fn linea_de_aislamiento(aislamiento: Option<&str>) -> String {
    match aislamiento {
        None => String::new(),
        Some("integridad_baja") => {
            "- Aislada con integridad baja y en un Job de Windows: no podía \
                                    escribir fuera de su carpeta, pero sí usar la red\n"
                .into()
        }
        Some("espacio_de_nombres") => "- Aislada en un espacio de nombres sin red\n".into(),
        Some("ninguno") => "- **Sin aislar**: el código del modelo corrió con todos los permisos \
                            del usuario\n"
            .into(),
        Some(otro) => format!("- Aislamiento: {otro}\n"),
    }
}

fn linea_de_pruebas_ocultas(datos: &DatosDeTanda) -> String {
    let Some(maquina) = &datos.maquina_de_comprobacion else {
        return "- Pruebas ocultas: no se ejecutó código del modelo; lo que se puntúa con ellas \
                sale «no evaluado»\n"
            .into();
    };
    // Solo la comprobación local anota el sistema.
    let Some(sistema) = &datos.sistema_de_comprobacion else {
        return format!("- Pruebas ocultas: en {maquina}\n");
    };
    let donde = format!(
        "- Pruebas ocultas: en el equipo que pasó la tanda ({sistema}), dentro de su carpeta\n{}",
        linea_de_aislamiento(datos.aislamiento.as_deref())
    );
    if datos.rastro_fuera.is_empty() {
        return format!("{donde}- Fuera de la carpeta de la tanda no quedó rastro\n");
    }
    let lineas: String = datos
        .rastro_fuera
        .iter()
        .map(|ruta| format!("  - {ruta}\n"))
        .collect();
    format!(
        "{donde}- **Fuera de la carpeta de la tanda apareció esto** (no se ha borrado nada):\n\
         {lineas}"
    )
}

pub fn componer_informe(resultados: &ResultadosDeTanda) -> String {
    let resumenes = resumir_encargos(&resultados.encargos);
    let mut texto = cabecera_del_informe(resultados);
    texto.push_str(&seccion_por_encargo(&resumenes));
    texto.push_str(&seccion_por_familia(&resumenes));
    texto.push_str(&seccion_por_clase(resultados));
    texto.push_str(&seccion_de_velocidad(resultados));
    texto
}

/// El título, el aviso de rúbricas pendientes y los datos de la tanda.
fn cabecera_del_informe(resultados: &ResultadosDeTanda) -> String {
    let datos = &resultados.tanda;
    let pendientes = resultados.pendientes();
    let mut texto = format!("# Banco de modelos: {}\n\n", datos.modelo);

    if pendientes > 0 {
        texto.push_str(&format!(
            "> **Quedan {pendientes} rúbricas por puntuar.** Rellena la línea «Nota:» de cada una y \
             rehaz el informe con `programator --banco-informe {NOMBRE_RESULTADOS}`. Hasta \
             entonces no se da la media de criterio.\n\n"
        ));
    }

    let huella: String = datos
        .huella_sha256
        .chars()
        .take(CARACTERES_DE_HUELLA)
        .collect();
    texto.push_str(&format!(
        "- Fecha: {}\n- Modelo: {} (SHA-256 {huella}…)\n- Motor: {}\n- Programator: {}\n\
         - Semillas: {:?}, temperatura {}\n",
        datos.fecha,
        datos.modelo,
        datos.compilacion_motor,
        datos.version_programator,
        datos.semillas,
        decimal(datos.temperatura, 2),
    ));
    texto.push_str(&linea_de_pruebas_ocultas(datos));
    if let (Some(capas), Some(vram), Some(carga)) = (
        datos.capas_en_gpu,
        datos.vram_ocupada_mib,
        datos.segundos_de_carga,
    ) {
        texto.push_str(&format!(
            "- Carga: {capas} capas en la GPU, {vram} MiB de VRAM, {} s\n",
            decimal(carga, 1)
        ));
    }
    texto
}

/// La tabla con una fila por encargo.
fn seccion_por_encargo(resumenes: &[ResumenDeEncargo]) -> String {
    let mut texto = format!(
        "\n## Por encargo\n\n| Encargo | Familia | {} | Dispersión | Semillas | Detalle |\n\
         |---|---|---|---|---|---|\n",
        cabecera_de_nota("Nota")
    );
    for r in resumenes {
        let media = r.resumen.and_then(|s| s.media);
        let dispersion = r
            .resumen
            .and_then(|s| s.dispersion)
            .map(|d| decimal(d, 2))
            .unwrap_or_default();
        let semillas = r
            .resumen
            .map(|s| format!("{} de {}", s.evaluadas, s.evaluadas + s.no_evaluadas))
            .unwrap_or_else(|| format!("{} pendientes", r.pendientes));
        texto.push_str(&format!(
            "| {} {} | {} | {} | {dispersion} | {semillas} | {} |\n",
            r.id,
            r.titulo,
            nombre_de_familia(r.familia),
            texto_de_nota(media, r.pendientes > 0),
            detalle_del_encargo(r),
        ));
    }
    texto
}

/// La columna «Detalle» de un encargo: lo que se contó para ponerle la nota.
fn detalle_del_encargo(r: &ResumenDeEncargo) -> String {
    let mut detalle = Vec::new();
    if let Some((superadas, total)) = r.superadas {
        detalle.push(format!("{superadas} de {total} comprobaciones"));
    }
    if let Some((aciertos, sembrados)) = r.aciertos {
        detalle.push(format!("{aciertos} de {sembrados} defectos"));
    }
    if r.aciertos.is_some() || r.clase == Some(ClaseDeUso::FicheroLimpio) {
        detalle.push(format!("{} inventados", r.inventados));
    }
    if r.por_rubrica > 0 {
        detalle.push(mensajes::detalle_puntuada_con_rubrica(r.por_rubrica));
    }
    if !r.motivos_no_evaluado.is_empty() {
        detalle.push(format!("no evaluado: {}", r.motivos_no_evaluado.join("; ")));
    }
    if !r.intentos_de_red.is_empty() {
        detalle.push(mensajes::detalle_intentos_de_red(&r.intentos_de_red));
    }
    detalle.join(", ")
}

/// La nota media de cada familia que tiene encargos.
fn seccion_por_familia(resumenes: &[ResumenDeEncargo]) -> String {
    let mut texto = format!(
        "\n## Por familia\n\n| Familia | {} |\n|---|---|\n",
        cabecera_de_nota("Nota media")
    );
    for familia in FAMILIAS {
        let de_la_familia: Vec<&ResumenDeEncargo> =
            resumenes.iter().filter(|r| r.familia == familia).collect();
        if de_la_familia.is_empty() {
            continue;
        }
        let pendiente = de_la_familia.iter().any(|r| r.pendientes > 0);
        texto.push_str(&format!(
            "| {} | {} |\n",
            nombre_de_familia(familia),
            texto_de_nota(nota_de_familia(resumenes, familia), pendiente),
        ));
    }
    texto
}

/// Las filas que se anotan en el registro de modelos.
fn seccion_por_clase(resultados: &ResultadosDeTanda) -> String {
    let mut texto = format!(
        "\n## Por clase de trabajo\n\nLas filas que se anotan en el registro de modelos y que la \
         guía cuenta al agente de pago.\n\n\
         | Clase de encargo | {} | Inventados | Tokens/s generando |\n|---|---|---|---|\n",
        cabecera_de_nota("Nota")
    );
    for clase in calcular_clases(&resultados.encargos) {
        texto.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            clase.fila.titulo(),
            texto_de_nota(clase.nota_media, clase.pendientes > 0),
            clase.inventados,
            clase
                .tokens_por_segundo
                .map(|v| decimal(v, 1))
                .unwrap_or_default(),
        ));
    }
    texto
}

/// El tiempo medio y la velocidad del motor en la tanda.
fn seccion_de_velocidad(resultados: &ResultadosDeTanda) -> String {
    let semillas: Vec<&ResultadoDeSemilla> = resultados
        .encargos
        .iter()
        .flat_map(|e| e.semillas.iter())
        .collect();
    let mut texto = String::from("\n## Velocidad\n\n");
    if !semillas.is_empty() {
        let media = semillas.iter().map(|s| s.segundos).sum::<f64>() / semillas.len() as f64;
        texto.push_str(&format!(
            "- Tiempo medio por encargo: {} s\n",
            decimal(media, 0)
        ));
    }
    let sin_cifra = String::from("sin medir");
    texto.push_str(&format!(
        "- Leyendo: {} tokens/s\n- Generando: {} tokens/s\n",
        velocidad_de_lectura(semillas.iter().copied())
            .map(|v| decimal(v, 1))
            .unwrap_or_else(|| sin_cifra.clone()),
        velocidad_de_generacion(semillas.iter().copied())
            .map(|v| decimal(v, 1))
            .unwrap_or(sin_cifra),
    ));
    let sin_consumo: usize = semillas.iter().map(|s| s.respuestas_sin_consumo).sum();
    if sin_consumo > 0 {
        texto.push_str(&format!(
            "- {sin_consumo} respuestas del motor no trajeron su consumo: las cifras de tokens se \
             quedan cortas.\n"
        ));
    }
    texto
}

/// Escribe el TOML y el informe en `carpeta`.
pub fn guardar(resultados: &ResultadosDeTanda, carpeta: &Path) -> Resultado<()> {
    escribir(
        &carpeta.join(NOMBRE_RESULTADOS),
        &componer_toml(resultados)?,
    )?;
    escribir(&carpeta.join(NOMBRE_INFORME), &componer_informe(resultados))
}

/// Vuelve a leer las rúbricas de una tanda ya pasada y reescribe sus resultados e informe, sin
/// pasar ningún encargo. Devuelve cuántas rúbricas siguen pendientes.
pub fn rehacer_informe(ruta_resultados: &Path) -> Resultado<usize> {
    let carpeta = ruta_resultados.parent().unwrap_or_else(|| Path::new("."));
    let resultados = leer_con_las_rubricas(ruta_resultados)?;
    guardar(&resultados, carpeta)?;
    Ok(resultados.pendientes())
}

/// Lee los resultados de una tanda con las notas que haya en sus rúbricas ahora mismo, aunque no
/// se haya rehecho el informe desde que se puntuaron.
pub fn leer_con_las_rubricas(ruta_resultados: &Path) -> Resultado<ResultadosDeTanda> {
    let carpeta = ruta_resultados.parent().unwrap_or_else(|| Path::new("."));
    let mut resultados = leer_toml(&leer(ruta_resultados)?)?;
    for semilla in resultados
        .encargos
        .iter_mut()
        .flat_map(|e| e.semillas.iter_mut())
    {
        if let Some(rubrica) = &semilla.rubrica {
            let nota = leer_fichero(&carpeta.join(rubrica))?;
            semilla.aplicar_rubrica(nota);
        }
    }
    resultados.clases = calcular_clases(&resultados.encargos);
    Ok(resultados)
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;
    use crate::banco::rubrica::{componer_fichero, nombre_del_fichero};

    fn datos() -> DatosDeTanda {
        DatosDeTanda {
            modelo: "Devstral-Small-2-24B".into(),
            huella_sha256: "ab".repeat(32),
            compilacion_motor: "b10993".into(),
            version_programator: "0.11.0".into(),
            fecha: "2026-11-11".into(),
            semillas: vec![42, 43],
            temperatura: 0.2,
            margen_lineas: 2,
            tiempo_maximo_encargo_segundos: 900,
            maquina_de_comprobacion: None,
            capas_en_gpu: Some(41),
            vram_ocupada_mib: Some(13_100),
            segundos_de_carga: Some(21.5),
            tarjeta: None,
            sistema_de_comprobacion: None,
            rastro_fuera: Vec::new(),
            aislamiento: None,
            ajustes_del_motor: None,
        }
    }

    fn semilla(semilla: i64, estado: EstadoDeNota, nota: Option<f64>) -> ResultadoDeSemilla {
        ResultadoDeSemilla {
            semilla,
            estado,
            nota,
            segundos: 120.0,
            tokens_generados: Some(500),
            ms_generacion: Some(25_000.0),
            ..ResultadoDeSemilla::default()
        }
    }

    fn encargo(id: &str, familia: Familia, clase: Option<ClaseDeUso>) -> ResultadoDeEncargo {
        ResultadoDeEncargo {
            id: id.into(),
            cuenta_como: id.into(),
            titulo: format!("título de {id}"),
            familia,
            clase,
            semillas: Vec::new(),
        }
    }

    pub(crate) fn tanda_de_ejemplo() -> ResultadosDeTanda {
        let mut oficio = encargo("E03", Familia::Oficio, None);
        oficio.semillas = vec![
            ResultadoDeSemilla {
                comprobaciones_superadas: Some(7),
                comprobaciones_total: Some(10),
                ..semilla(42, EstadoDeNota::Evaluada, Some(3.5))
            },
            ResultadoDeSemilla {
                detalle: Some("falta en este equipo: rust".into()),
                ..semilla(43, EstadoDeNota::NoEvaluado, None)
            },
        ];
        let mut formas = encargo("U01", Familia::Uso, Some(ClaseDeUso::Formas));
        formas.semillas = vec![ResultadoDeSemilla {
            sembrados: Some(6),
            aciertos: Some(5),
            inventados: Some(1),
            repetidos: Some(0),
            sin_formato: Some(0),
            ..semilla(42, EstadoDeNota::Evaluada, Some(4.5))
        }];
        let mut criterio = encargo("E01", Familia::Criterio, None);
        criterio.semillas = vec![ResultadoDeSemilla {
            rubrica: Some(format!("rubricas/{}", nombre_del_fichero("E01", 42))),
            ..semilla(42, EstadoDeNota::Pendiente, None)
        }];
        let encargos = vec![oficio, formas, criterio];
        ResultadosDeTanda {
            tanda: datos(),
            clases: calcular_clases(&encargos),
            encargos,
        }
    }

    #[test]
    fn el_toml_compuesto_se_vuelve_a_leer_igual() {
        let resultados = tanda_de_ejemplo();

        let texto = componer_toml(&resultados).unwrap();

        assert_eq!(leer_toml(&texto).unwrap(), resultados);
    }

    #[test]
    fn la_tarjeta_se_guarda_y_una_tanda_antigua_sin_ella_se_sigue_leyendo() {
        let mut resultados = tanda_de_ejemplo();
        resultados.tanda.tarjeta = Some("NVIDIA GeForce RTX 4070 Ti SUPER".into());
        let texto = componer_toml(&resultados).unwrap();
        assert_eq!(leer_toml(&texto).unwrap(), resultados);

        let sin_tarjeta: String = texto
            .lines()
            .filter(|linea| !linea.starts_with("tarjeta"))
            .map(|linea| format!("{linea}\n"))
            .collect();
        assert_eq!(leer_toml(&sin_tarjeta).unwrap().tanda.tarjeta, None);
    }

    #[test]
    fn una_tanda_antigua_sin_sistema_ni_rastro_se_sigue_leyendo() {
        let mut resultados = tanda_de_ejemplo();
        resultados.tanda.sistema_de_comprobacion = Some("windows".into());
        resultados.tanda.rastro_fuera = vec!["C:/Users/x/.nuget/packages/algo".into()];
        let texto = componer_toml(&resultados).unwrap();
        assert_eq!(leer_toml(&texto).unwrap(), resultados);

        let antigua: String = texto
            .lines()
            .filter(|l| !l.starts_with("sistema_de_comprobacion") && !l.starts_with("rastro_fuera"))
            .map(|linea| format!("{linea}\n"))
            .collect();
        let leida = leer_toml(&antigua).unwrap();
        assert_eq!(leida.tanda.sistema_de_comprobacion, None);
        assert!(leida.tanda.rastro_fuera.is_empty());
    }

    #[test]
    fn el_informe_dice_donde_se_comprobo_y_lo_que_quedo_fuera() {
        let mut resultados = tanda_de_ejemplo();
        resultados.tanda.maquina_de_comprobacion = Some("maquina-linux".into());
        assert!(componer_informe(&resultados).contains("- Pruebas ocultas: en maquina-linux\n"));

        resultados.tanda.maquina_de_comprobacion = Some("local".into());
        resultados.tanda.sistema_de_comprobacion = Some("windows".into());
        let sin_rastro = componer_informe(&resultados);
        assert!(sin_rastro.contains("en el equipo que pasó la tanda (windows)"));
        assert!(sin_rastro.contains("no quedó rastro"));

        resultados.tanda.rastro_fuera = vec!["C:/Users/x/.nuget/packages/algo".into()];
        let con_rastro = componer_informe(&resultados);
        assert!(con_rastro.contains("apareció esto"), "{con_rastro}");
        assert!(con_rastro.contains("  - C:/Users/x/.nuget/packages/algo\n"));
    }

    #[test]
    fn el_informe_marca_el_encargo_y_la_semilla_que_intentaron_usar_la_red() {
        let mut resultados = tanda_de_ejemplo();
        assert!(!componer_informe(&resultados).contains("intentó conectarse"));

        resultados.encargos[0].semillas[0].intentos_de_red = vec!["pypi.org:443".into()];
        let informe = componer_informe(&resultados);

        let fila = informe
            .lines()
            .find(|linea| linea.starts_with("| E03"))
            .unwrap();
        assert!(
            fila.contains(
                "⚠️ intentó conectarse a: pypi.org:443 (semilla 42); cuenta como fallida"
            ),
            "{fila}"
        );
        // Se guarda y se vuelve a leer; una tanda sin intentos no escribe la clave.
        let texto = componer_toml(&resultados).unwrap();
        assert_eq!(leer_toml(&texto).unwrap(), resultados);
        assert_eq!(texto.matches("intentos_de_red").count(), 1);
    }

    #[test]
    fn el_informe_dice_que_semillas_de_codigo_se_puntuaron_con_la_rubrica() {
        // Las dos notas no son comparables: el informe lo dice.
        let mut resultados = tanda_de_ejemplo();
        resultados.encargos[0].semillas[0].puntuada_con = Some(PuntuadaCon::PruebasOcultas);
        assert!(!componer_informe(&resultados).contains("con la rúbrica, leyendo el código"));

        resultados.encargos[0].semillas[1] = ResultadoDeSemilla {
            puntuada_con: Some(PuntuadaCon::Rubrica),
            rubrica: Some(format!("rubricas/{}", nombre_del_fichero("E03", 43))),
            ..semilla(43, EstadoDeNota::Evaluada, Some(2.0))
        };
        let informe = componer_informe(&resultados);

        let fila = informe
            .lines()
            .find(|linea| linea.starts_with("| E03"))
            .unwrap();
        assert!(
            fila.contains("1 semilla puntuada con la rúbrica, leyendo el código sin ejecutarlo"),
            "{fila}"
        );
        assert!(fila.contains("no es comparable"), "{fila}");
    }

    #[test]
    fn con_que_se_puntuo_se_guarda_y_una_tanda_antigua_sin_el_dato_se_sigue_leyendo() {
        let mut resultados = tanda_de_ejemplo();
        assert!(!componer_toml(&resultados).unwrap().contains("puntuada_con"));

        resultados.encargos[0].semillas[0].puntuada_con = Some(PuntuadaCon::PruebasOcultas);
        resultados.encargos[0].semillas[1].puntuada_con = Some(PuntuadaCon::Rubrica);
        let texto = componer_toml(&resultados).unwrap();
        assert!(
            texto.contains("puntuada_con = \"pruebas_ocultas\""),
            "{texto}"
        );
        assert!(texto.contains("puntuada_con = \"rubrica\""), "{texto}");
        assert_eq!(leer_toml(&texto).unwrap(), resultados);

        let antigua: String = texto
            .lines()
            .filter(|linea| !linea.starts_with("puntuada_con"))
            .map(|linea| format!("{linea}\n"))
            .collect();
        let leida = leer_toml(&antigua).unwrap();
        assert!(leida
            .encargos
            .iter()
            .flat_map(|e| e.semillas.iter())
            .all(|s| s.puntuada_con.is_none()));
    }

    #[test]
    fn el_informe_dice_como_se_aislo_y_avisa_si_no_se_aislo() {
        let mut resultados = tanda_de_ejemplo();
        resultados.tanda.maquina_de_comprobacion = Some("local".into());
        resultados.tanda.sistema_de_comprobacion = Some("windows".into());
        let con = |aislamiento: Option<&str>, resultados: &mut ResultadosDeTanda| {
            resultados.tanda.aislamiento = aislamiento.map(String::from);
            componer_informe(resultados)
        };

        assert!(con(Some("integridad_baja"), &mut resultados).contains("sí usar la red"));
        assert!(con(Some("espacio_de_nombres"), &mut resultados).contains("sin red"));
        assert!(con(Some("ninguno"), &mut resultados).contains("**Sin aislar**"));
        assert!(!con(None, &mut resultados).contains("islad"));
    }

    #[test]
    fn con_rubricas_pendientes_el_informe_lo_avisa_y_no_da_la_media_de_criterio() {
        let informe = componer_informe(&tanda_de_ejemplo());

        assert!(informe.contains("Quedan 1 rúbricas por puntuar"));
        assert!(informe.contains("| criterio | pendiente |"));
        assert!(informe.contains("| oficio | 3,5 |"));
    }

    #[test]
    fn las_tres_tablas_de_notas_dicen_sobre_cuanto_se_puntua() {
        // No es lo mismo 2,8/10 que 2,8/5.
        let informe = componer_informe(&tanda_de_ejemplo());

        assert!(
            informe.contains("| Familia | Nota (sobre 5) |"),
            "{informe}"
        );
        assert!(informe.contains("| Familia | Nota media (sobre 5) |"));
        assert!(informe.contains("| Clase de encargo | Nota (sobre 5) |"));
    }

    #[test]
    fn el_informe_dice_por_que_no_se_evaluo_una_semilla() {
        // Tanda del 12/11/2026: todo el código salió «no evaluado» y el informe no decía por qué.
        let informe = componer_informe(&tanda_de_ejemplo());

        assert!(
            informe.contains("no evaluado: falta en este equipo: rust"),
            "{informe}"
        );
    }

    #[test]
    fn sin_pendientes_no_hay_aviso() {
        let mut resultados = tanda_de_ejemplo();
        resultados
            .encargos
            .retain(|e| e.familia != Familia::Criterio);

        assert!(!componer_informe(&resultados).contains("Quedan"));
    }

    #[test]
    fn la_tabla_de_clases_tiene_las_seis_filas_en_su_orden() {
        // Hasta el 12/11/2026 el orden lo fijaba plantillas/desempeno-medido.md, escrita a mano.
        // Se retiró con el registro de modelos: ahora lo fija FILAS_DE_DESEMPENO, que comparten
        // el informe, el registro y la guía.
        let informe = componer_informe(&tanda_de_ejemplo());
        let filas: Vec<usize> = FILAS_DE_DESEMPENO
            .iter()
            .map(|f| informe.find(&format!("| {} |", f.titulo())).unwrap())
            .collect();
        assert_eq!(filas.len(), 6);
        assert!(filas.windows(2).all(|par| par[0] < par[1]));
    }

    #[test]
    fn las_variantes_cuentan_como_un_solo_encargo() {
        let mut a = encargo("E10a", Familia::Oficio, None);
        a.cuenta_como = "E10".into();
        a.semillas = vec![semilla(42, EstadoDeNota::Evaluada, Some(2.0))];
        let mut b = a.clone();
        b.id = "E10b".into();
        b.semillas = vec![semilla(42, EstadoDeNota::Evaluada, Some(4.0))];

        let encargos = [a, b];
        let resumenes = resumir_encargos(&encargos);

        assert_eq!(resumenes.len(), 1);
        assert_eq!(resumenes[0].id, "E10");
        assert_eq!(resumenes[0].resumen.unwrap().media, Some(3.0));
    }

    #[test]
    fn la_clase_guarda_nota_velocidad_e_inventados() {
        let clases = tanda_de_ejemplo().clases;
        let formas = clases
            .iter()
            .find(|c| c.fila == FilaDeDesempeno::Formas)
            .unwrap();

        assert_eq!(formas.nota_media, Some(4.5));
        assert_eq!(formas.inventados, 1);
        assert_eq!(formas.tokens_por_segundo, Some(20.0));
        let diagnostico = clases
            .iter()
            .find(|c| c.fila == FilaDeDesempeno::Diagnostico)
            .unwrap();
        assert_eq!(diagnostico.nota_media, None);
    }

    #[test]
    fn rehacer_el_informe_recoge_la_nota_de_la_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let resultados = tanda_de_ejemplo();
        guardar(&resultados, dir.path()).unwrap();
        std::fs::create_dir_all(dir.path().join("rubricas")).unwrap();
        let rubrica =
            componer_fichero("E01", 42, "respuesta", "rúbrica").replace("Nota: \n", "Nota: 4\n");
        std::fs::write(
            dir.path()
                .join("rubricas")
                .join(nombre_del_fichero("E01", 42)),
            rubrica,
        )
        .unwrap();

        let pendientes = rehacer_informe(&dir.path().join(NOMBRE_RESULTADOS)).unwrap();

        assert_eq!(pendientes, 0);
        let informe = std::fs::read_to_string(dir.path().join(NOMBRE_INFORME)).unwrap();
        assert!(informe.contains("| criterio | 4,0 |"));
        assert!(!informe.contains("Quedan"));
    }
}
