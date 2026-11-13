//! La ficha de un encargo del banco: leerla, validarla y cargar todos los encargos de una carpeta.
//!
//! El esquema está documentado en `evaluacion/banco/encargos/README.md`. Aquí se decide, una sola
//! vez y para todo el banco, cómo se puntúa cada encargo ([`FormaDePuntuar`]); la validación y la
//! puntuación se apoyan en esa misma decisión, para que no puedan contradecirse.
//!
//! La validación es una función pura sobre la ficha y lo que hay en su carpeta
//! ([`FicherosPresentes`]); leer el disco es una capa fina aparte.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::mensajes;
use crate::error::{Error, Resultado};
use crate::ficheros::{entradas, leer};

/// El fichero con la ficha, dentro de la carpeta de cada encargo.
pub const NOMBRE_FICHA: &str = "ficha.toml";
/// El cuerpo del encargo que se publica en el canal.
pub const NOMBRE_ENUNCIADO: &str = "enunciado.md";
/// Lo que vale cada nota, en los encargos que puntúa el agente que dirige.
pub const NOMBRE_RUBRICA: &str = "rubrica.md";
/// Los ficheros que se copian a la carpeta de trabajo antes de publicar el encargo.
pub const CARPETA_PARTIDA: &str = "partida";

/// Las tres familias de encargos de la especificación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Familia {
    /// Escribir código que pasa unas pruebas ocultas.
    Oficio,
    /// Protocolo, diseño, depuración y honestidad: lo puntúa el agente que dirige.
    Criterio,
    /// Las clases de trabajo de las filas de desempeño (`FilaDeDesempeno`).
    Uso,
}

/// Las clases de trabajo de las filas de desempeño: solo en la familia uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaseDeUso {
    Formas,
    Validaciones,
    FuncionAcotada,
    Diagnostico,
    FicheroLimpio,
}

/// Lo que tiene que haber instalado en el equipo para comprobar una entrega: un lenguaje o un
/// programa, por su nombre (`rust`, `python`, `php`...).
///
/// **Es un nombre abierto**: cada usuario prueba los lenguajes
/// que le interesan sin tocar el código. Los que el banco conoce sin configurar nada están en
/// [`crate::banco::config::HERRAMIENTAS_POR_DEFECTO`]; cualquier otro se declara en
/// `[comprobacion_local.herramientas]` de `banco.toml`, con la orden que mira si está instalado.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct Herramienta(String);

impl Herramienta {
    /// Un nombre válido: no vacío, en minúsculas, con letras, dígitos, guiones y guiones bajos. Es
    /// lo que se escribe como clave de TOML y lo que se lee en los mensajes, así que no admite
    /// espacios ni signos que obliguen a entrecomillarlo.
    pub fn nueva(nombre: &str) -> Result<Self, String> {
        let valido = !nombre.is_empty()
            && nombre
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if valido {
            Ok(Self(nombre.to_string()))
        } else {
            Err(mensajes::motivo_nombre_de_herramienta_no_valido(nombre))
        }
    }

    /// El nombre, tal como se escribe en las fichas y en `banco.toml`.
    pub fn nombre(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Herramienta {
    type Error = String;

    fn try_from(nombre: String) -> Result<Self, Self::Error> {
        Self::nueva(&nombre)
    }
}

impl std::fmt::Display for Herramienta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Una orden que se ejecuta desde la raíz de `pruebas/`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Orden {
    pub orden: String,
}

/// Una prueba oculta: pasa si su orden termina con código 0.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comprobacion {
    pub nombre: String,
    pub orden: String,
}

/// Un requisito que se comprueba buscando un patrón en la entrega.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequisitoDeTexto {
    pub nombre: String,
    pub patron: String,
    pub maximo: Option<usize>,
    pub minimo: Option<usize>,
    /// Si está, solo se busca en el texto anterior a esta marca.
    pub antes_de: Option<String>,
    #[serde(default)]
    pub ignorar_mayusculas: bool,
}

impl RequisitoDeTexto {
    /// Si `contenido` cumple el requisito: cuenta las apariciones del patrón (en la parte anterior
    /// a `antes_de`, si está) y las compara con el límite. Sin límite no se cumple; la validación
    /// de la ficha ya lo impide.
    pub fn se_cumple_en(&self, contenido: &str) -> bool {
        let parte = match &self.antes_de {
            Some(marca) => contenido.split(marca.as_str()).next().unwrap_or(contenido),
            None => contenido,
        };
        let apariciones = if self.ignorar_mayusculas {
            parte
                .to_lowercase()
                .matches(&self.patron.to_lowercase())
                .count()
        } else {
            parte.matches(&self.patron).count()
        };
        match (self.maximo, self.minimo) {
            (Some(maximo), None) => apariciones <= maximo,
            (None, Some(minimo)) => apariciones >= minimo,
            _ => false,
        }
    }
}

/// Un defecto puesto a propósito en el fichero auditado.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefectoSembrado {
    /// Cualquier línea donde se pueda señalar el defecto.
    pub lineas: Vec<usize>,
    /// Se guarda para el informe; no cuenta para el acierto.
    pub tipo: String,
    pub descripcion: String,
}

/// La ficha de un encargo, tal como se escribe en `ficha.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FichaEncargo {
    pub id: String,
    pub titulo: String,
    pub familia: Familia,
    pub clase: Option<ClaseDeUso>,
    /// El fichero que el modelo entrega con `escribir_propuesta`.
    pub entrega: Option<String>,
    /// Dónde se copia la entrega dentro de `pruebas/`.
    pub colocar_en: Option<String>,
    #[serde(default)]
    pub herramientas: Vec<Herramienta>,
    /// Lo que se hace antes de compilar y no toca el código del modelo, como descargar las
    /// dependencias: es lo único que tiene red en la comprobación local.
    pub preparar: Option<Orden>,
    pub compilar: Option<Orden>,
    #[serde(default)]
    pub comprobaciones: Vec<Comprobacion>,
    #[serde(default)]
    pub texto: Vec<RequisitoDeTexto>,
    /// En las auditorías, la ruta del fichero revisado dentro de la carpeta de trabajo.
    pub auditado: Option<String>,
    #[serde(default)]
    pub defectos: Vec<DefectoSembrado>,
    /// Si es una de varias variantes de un mismo encargo, el encargo del que lo es.
    pub variante_de: Option<String>,
}

/// Cómo se puntúa un encargo. Es la única decisión sobre eso en todo el banco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormaDePuntuar {
    /// Pruebas ocultas sobre la entrega: oficio y función acotada.
    Comprobaciones,
    /// Hallazgos contra defectos sembrados: formas y validaciones.
    Auditoria,
    /// Auditoría de un fichero sin defectos: todo hallazgo es inventado.
    FicheroLimpio,
    /// La puntúa el agente que dirige con `rubrica.md`: criterio y diagnóstico.
    Rubrica,
}

/// Cómo se puntúa un encargo de esa familia y esa clase, o `None` si no casan. Va aparte de la
/// ficha porque también lo preguntan quienes solo tienen los resultados de una tanda.
pub fn forma_de_puntuar(familia: Familia, clase: Option<ClaseDeUso>) -> Option<FormaDePuntuar> {
    match (familia, clase) {
        (Familia::Oficio, None) => Some(FormaDePuntuar::Comprobaciones),
        (Familia::Criterio, None) => Some(FormaDePuntuar::Rubrica),
        (Familia::Uso, Some(ClaseDeUso::FuncionAcotada)) => Some(FormaDePuntuar::Comprobaciones),
        (Familia::Uso, Some(ClaseDeUso::Formas | ClaseDeUso::Validaciones)) => {
            Some(FormaDePuntuar::Auditoria)
        }
        (Familia::Uso, Some(ClaseDeUso::FicheroLimpio)) => Some(FormaDePuntuar::FicheroLimpio),
        (Familia::Uso, Some(ClaseDeUso::Diagnostico)) => Some(FormaDePuntuar::Rubrica),
        (Familia::Oficio | Familia::Criterio, Some(_)) | (Familia::Uso, None) => None,
    }
}

impl FichaEncargo {
    /// Cómo se puntúa, o `None` si familia y clase no casan (se rechaza al validar).
    pub fn forma_de_puntuar(&self) -> Option<FormaDePuntuar> {
        forma_de_puntuar(self.familia, self.clase)
    }

    /// El encargo que cuenta en la tanda: el propio, o aquel del que es variante.
    pub fn encargo_que_cuenta(&self) -> &str {
        self.variante_de.as_deref().unwrap_or(&self.id)
    }
}

/// Un encargo cargado: su ficha y la carpeta donde están el enunciado y las pruebas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encargo {
    pub ficha: FichaEncargo,
    pub carpeta: PathBuf,
}

/// Lo que hay en la carpeta de un encargo, para validar sin tocar el disco.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FicherosPresentes {
    pub enunciado: bool,
    pub rubrica: bool,
    /// Rutas relativas a `partida/`, con `/` como separador.
    pub partida: BTreeSet<String>,
}

/// Por qué una ficha no vale. Cada mensaje dice en qué encargo y qué falta.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProblemaDeFicha {
    #[error("no hay ningún encargo en «{0}»")]
    CarpetaSinEncargos(PathBuf),
    #[error("la ficha de «{carpeta}» no se puede leer: {causa}")]
    Ilegible { carpeta: String, causa: String },
    #[error("el identificador {0} está repetido")]
    IdRepetido(String),
    #[error("{0}: falta {NOMBRE_ENUNCIADO}")]
    SinEnunciado(String),
    #[error(
        "{0}: la familia y la clase no casan (la clase solo va, y siempre, en la familia uso)"
    )]
    ClaseQueNoCasa(String),
    #[error("{0}: no tiene comprobaciones")]
    SinComprobaciones(String),
    #[error("{id}: falta la clave «{clave}»")]
    FaltaClave { id: String, clave: &'static str },
    #[error("{0}: es una auditoría y no tiene defectos sembrados")]
    SinDefectos(String),
    #[error("{0}: es un fichero limpio y tiene defectos sembrados")]
    LimpioConDefectos(String),
    #[error("{id}: un defecto sembrado no tiene líneas: «{descripcion}»")]
    DefectoSinLineas { id: String, descripcion: String },
    #[error("{id}: el fichero de partida «{fichero}» no está en {CARPETA_PARTIDA}/")]
    FaltaFicheroDePartida { id: String, fichero: String },
    #[error("{0}: falta {NOMBRE_RUBRICA}")]
    SinRubrica(String),
    #[error("{id}: el requisito de texto «{nombre}» necesita uno, y solo uno, de maximo y minimo")]
    TextoSinLimite { id: String, nombre: String },
}

impl From<ProblemaDeFicha> for Error {
    fn from(problema: ProblemaDeFicha) -> Self {
        Error::Banco(problema.to_string())
    }
}

/// Comprueba que la ficha tiene todo lo que su forma de puntuar necesita.
pub fn validar_ficha(
    ficha: &FichaEncargo,
    presentes: &FicherosPresentes,
) -> Result<(), ProblemaDeFicha> {
    let id = || ficha.id.clone();
    if !presentes.enunciado {
        return Err(ProblemaDeFicha::SinEnunciado(id()));
    }
    let forma = ficha
        .forma_de_puntuar()
        .ok_or_else(|| ProblemaDeFicha::ClaseQueNoCasa(id()))?;
    for requisito in &ficha.texto {
        if requisito.maximo.is_some() == requisito.minimo.is_some() {
            return Err(ProblemaDeFicha::TextoSinLimite {
                id: id(),
                nombre: requisito.nombre.clone(),
            });
        }
    }
    match forma {
        FormaDePuntuar::Comprobaciones => validar_comprobaciones(ficha),
        FormaDePuntuar::Auditoria => validar_auditoria(ficha, presentes),
        FormaDePuntuar::FicheroLimpio => validar_fichero_limpio(ficha, presentes),
        FormaDePuntuar::Rubrica if presentes.rubrica => Ok(()),
        FormaDePuntuar::Rubrica => Err(ProblemaDeFicha::SinRubrica(id())),
    }
}

fn exigir(ficha: &FichaEncargo, valor: bool, clave: &'static str) -> Result<(), ProblemaDeFicha> {
    if valor {
        Ok(())
    } else {
        Err(ProblemaDeFicha::FaltaClave {
            id: ficha.id.clone(),
            clave,
        })
    }
}

fn validar_comprobaciones(ficha: &FichaEncargo) -> Result<(), ProblemaDeFicha> {
    exigir(ficha, ficha.entrega.is_some(), "entrega")?;
    exigir(ficha, ficha.colocar_en.is_some(), "colocar_en")?;
    exigir(ficha, !ficha.herramientas.is_empty(), "herramientas")?;
    if ficha.comprobaciones.is_empty() {
        return Err(ProblemaDeFicha::SinComprobaciones(ficha.id.clone()));
    }
    Ok(())
}

fn validar_auditado(
    ficha: &FichaEncargo,
    presentes: &FicherosPresentes,
) -> Result<(), ProblemaDeFicha> {
    let Some(auditado) = &ficha.auditado else {
        return exigir(ficha, false, "auditado");
    };
    if presentes.partida.contains(auditado) {
        Ok(())
    } else {
        Err(ProblemaDeFicha::FaltaFicheroDePartida {
            id: ficha.id.clone(),
            fichero: auditado.clone(),
        })
    }
}

fn validar_auditoria(
    ficha: &FichaEncargo,
    presentes: &FicherosPresentes,
) -> Result<(), ProblemaDeFicha> {
    validar_auditado(ficha, presentes)?;
    if ficha.defectos.is_empty() {
        return Err(ProblemaDeFicha::SinDefectos(ficha.id.clone()));
    }
    match ficha.defectos.iter().find(|d| d.lineas.is_empty()) {
        Some(defecto) => Err(ProblemaDeFicha::DefectoSinLineas {
            id: ficha.id.clone(),
            descripcion: defecto.descripcion.clone(),
        }),
        None => Ok(()),
    }
}

fn validar_fichero_limpio(
    ficha: &FichaEncargo,
    presentes: &FicherosPresentes,
) -> Result<(), ProblemaDeFicha> {
    validar_auditado(ficha, presentes)?;
    if ficha.defectos.is_empty() {
        Ok(())
    } else {
        Err(ProblemaDeFicha::LimpioConDefectos(ficha.id.clone()))
    }
}

/// Rechaza identificadores repetidos y ordena los encargos por identificador.
pub fn ordenar_sin_repetidos(mut encargos: Vec<Encargo>) -> Result<Vec<Encargo>, ProblemaDeFicha> {
    encargos.sort_by(|a, b| a.ficha.id.cmp(&b.ficha.id));
    if let Some(par) = encargos
        .windows(2)
        .find(|par| par[0].ficha.id == par[1].ficha.id)
    {
        return Err(ProblemaDeFicha::IdRepetido(par[0].ficha.id.clone()));
    }
    Ok(encargos)
}

/// Carga y valida todos los encargos de `carpeta`: cada subcarpeta con un `ficha.toml`.
///
/// Una carpeta sin encargos es un error, no una tanda vacía.
pub fn cargar_encargos(carpeta: &Path) -> Resultado<Vec<Encargo>> {
    let mut encargos = Vec::new();
    for subcarpeta in subcarpetas(carpeta)? {
        let ruta_ficha = subcarpeta.join(NOMBRE_FICHA);
        if ruta_ficha.is_file() {
            encargos.push(cargar_encargo(&subcarpeta, &ruta_ficha)?);
        }
    }
    if encargos.is_empty() {
        return Err(ProblemaDeFicha::CarpetaSinEncargos(carpeta.to_path_buf()).into());
    }
    Ok(ordenar_sin_repetidos(encargos)?)
}

fn cargar_encargo(carpeta: &Path, ruta_ficha: &Path) -> Resultado<Encargo> {
    let texto = leer(ruta_ficha)?;
    let ficha: FichaEncargo =
        toml::from_str(&texto).map_err(|causa| ProblemaDeFicha::Ilegible {
            carpeta: nombre_de(carpeta),
            causa: causa.to_string(),
        })?;
    validar_ficha(&ficha, &ficheros_presentes(carpeta)?)?;
    Ok(Encargo {
        ficha,
        carpeta: carpeta.to_path_buf(),
    })
}

fn ficheros_presentes(carpeta: &Path) -> Resultado<FicherosPresentes> {
    let partida = carpeta.join(CARPETA_PARTIDA);
    let mut ficheros = BTreeSet::new();
    if partida.is_dir() {
        recorrer(&partida, &partida, &mut ficheros)?;
    }
    Ok(FicherosPresentes {
        enunciado: carpeta.join(NOMBRE_ENUNCIADO).is_file(),
        rubrica: carpeta.join(NOMBRE_RUBRICA).is_file(),
        partida: ficheros,
    })
}

/// Añade a `ficheros` la ruta, relativa a `raiz` y con `/`, de cada fichero bajo `carpeta`.
fn recorrer(raiz: &Path, carpeta: &Path, ficheros: &mut BTreeSet<String>) -> Resultado<()> {
    for ruta in entradas(carpeta)? {
        if ruta.is_dir() {
            recorrer(raiz, &ruta, ficheros)?;
        } else if let Ok(relativa) = ruta.strip_prefix(raiz) {
            let partes: Vec<String> = relativa
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            ficheros.insert(partes.join("/"));
        }
    }
    Ok(())
}

fn subcarpetas(carpeta: &Path) -> Resultado<Vec<PathBuf>> {
    Ok(entradas(carpeta)?
        .into_iter()
        .filter(|ruta| ruta.is_dir())
        .collect())
}

fn nombre_de(carpeta: &Path) -> String {
    carpeta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| carpeta.display().to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const OFICIO: &str = r##"
id = "E03"
titulo = "Rust"
familia = "oficio"
entrega = "rangos.rs"
colocar_en = "src/rangos.rs"
herramientas = ["rust"]
[compilar]
orden = "cargo build"
[[comprobaciones]]
nombre = "caso"
orden = "cargo test caso"
[[texto]]
nombre = "sin unwrap"
patron = ".unwrap()"
maximo = 0
antes_de = "#[cfg(test)]"
"##;

    const CRITERIO: &str = r#"
id = "E10a"
titulo = "Honestidad"
familia = "criterio"
variante_de = "E10"
"#;

    const AUDITORIA: &str = r#"
id = "U01"
titulo = "Formas"
familia = "uso"
clase = "formas"
auditado = "Pedidos.cs"
[[defectos]]
lineas = [41, 62]
tipo = "DUPLICADO"
descripcion = "copiado"
"#;

    const LIMPIO: &str = r#"
id = "U05"
titulo = "Limpio"
familia = "uso"
clase = "fichero_limpio"
auditado = "texto.rs"
"#;

    fn ficha(texto: &str) -> FichaEncargo {
        toml::from_str(texto).unwrap()
    }

    fn presentes(partida: &[&str]) -> FicherosPresentes {
        FicherosPresentes {
            enunciado: true,
            rubrica: true,
            partida: partida.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn una_ficha_de_oficio_se_lee_y_valida() {
        let f = ficha(OFICIO);

        assert_eq!(f.forma_de_puntuar(), Some(FormaDePuntuar::Comprobaciones));
        assert_eq!(f.herramientas, vec![Herramienta::nueva("rust").unwrap()]);
        assert_eq!(f.texto[0].maximo, Some(0));
        assert!(!f.texto[0].ignorar_mayusculas);
        assert_eq!(validar_ficha(&f, &presentes(&[])), Ok(()));
    }

    #[test]
    fn una_ficha_puede_pedir_un_lenguaje_que_el_banco_no_trae_de_serie() {
        // Los lenguajes son abiertos. Que el banco sepa
        // comprobarlo se decide al cargarlo, con banco.toml; la ficha solo lo nombra.
        let f = ficha(&OFICIO.replace(r#"herramientas = ["rust"]"#, r#"herramientas = ["php"]"#));

        assert_eq!(f.herramientas, vec![Herramienta::nueva("php").unwrap()]);
        assert_eq!(f.herramientas[0].nombre(), "php");
        assert_eq!(f.herramientas[0].to_string(), "php");
    }

    #[test]
    fn un_nombre_de_herramienta_con_mayusculas_o_espacios_no_vale_y_dice_por_que() {
        for malo in ["", "PHP", "php 8", "c#", "pó"] {
            let fallo = Herramienta::nueva(malo).unwrap_err();
            assert!(fallo.contains("minúsculas"), "{malo:?}: {fallo}");
        }
        for bueno in ["php", "php8", "c-sharp", "lua5_4"] {
            assert!(Herramienta::nueva(bueno).is_ok(), "{bueno}");
        }

        let texto = OFICIO.replace(r#"herramientas = ["rust"]"#, r#"herramientas = ["PHP"]"#);
        let fallo = toml::from_str::<FichaEncargo>(&texto).unwrap_err();
        assert!(fallo.to_string().contains("«PHP»"), "{fallo}");
    }

    #[test]
    fn una_ficha_de_criterio_se_lee_y_cuenta_como_su_encargo() {
        let f = ficha(CRITERIO);

        assert_eq!(f.forma_de_puntuar(), Some(FormaDePuntuar::Rubrica));
        assert_eq!(f.encargo_que_cuenta(), "E10");
        assert_eq!(validar_ficha(&f, &presentes(&[])), Ok(()));
    }

    #[test]
    fn una_ficha_de_uso_se_lee_y_valida() {
        let f = ficha(AUDITORIA);

        assert_eq!(f.forma_de_puntuar(), Some(FormaDePuntuar::Auditoria));
        assert_eq!(f.defectos[0].lineas, vec![41, 62]);
        assert_eq!(f.encargo_que_cuenta(), "U01");
        assert_eq!(validar_ficha(&f, &presentes(&["Pedidos.cs"])), Ok(()));
    }

    #[test]
    fn un_fichero_limpio_se_valida_sin_defectos() {
        assert_eq!(
            validar_ficha(&ficha(LIMPIO), &presentes(&["texto.rs"])),
            Ok(())
        );
    }

    #[test]
    fn una_familia_desconocida_no_se_lee() {
        let texto = CRITERIO.replace("\"criterio\"", "\"poesia\"");

        assert!(toml::from_str::<FichaEncargo>(&texto).is_err());
    }

    #[test]
    fn una_clave_con_errata_no_se_ignora() {
        let texto = format!("{OFICIO}\ncolocar_em = \"x\"\n");

        assert!(toml::from_str::<FichaEncargo>(&texto).is_err());
    }

    #[test]
    fn un_encargo_de_oficio_sin_comprobaciones_se_rechaza() {
        let mut f = ficha(OFICIO);
        f.comprobaciones.clear();

        let error = validar_ficha(&f, &presentes(&[])).unwrap_err();

        assert_eq!(error, ProblemaDeFicha::SinComprobaciones("E03".into()));
        assert_eq!(error.to_string(), "E03: no tiene comprobaciones");
    }

    #[test]
    fn un_encargo_de_oficio_sin_entrega_dice_que_clave_falta() {
        let mut f = ficha(OFICIO);
        f.entrega = None;

        let error = validar_ficha(&f, &presentes(&[])).unwrap_err();

        assert_eq!(error.to_string(), "E03: falta la clave «entrega»");
    }

    #[test]
    fn una_auditoria_sin_defectos_se_rechaza() {
        let mut f = ficha(AUDITORIA);
        f.defectos.clear();

        let error = validar_ficha(&f, &presentes(&["Pedidos.cs"])).unwrap_err();

        assert_eq!(
            error.to_string(),
            "U01: es una auditoría y no tiene defectos sembrados"
        );
    }

    #[test]
    fn un_defecto_sin_lineas_se_rechaza() {
        let mut f = ficha(AUDITORIA);
        f.defectos[0].lineas.clear();

        let error = validar_ficha(&f, &presentes(&["Pedidos.cs"])).unwrap_err();

        assert!(error
            .to_string()
            .contains("U01: un defecto sembrado no tiene líneas"));
    }

    #[test]
    fn un_fichero_de_partida_que_no_esta_se_rechaza() {
        let error = validar_ficha(&ficha(AUDITORIA), &presentes(&["Otro.cs"])).unwrap_err();

        assert_eq!(
            error.to_string(),
            "U01: el fichero de partida «Pedidos.cs» no está en partida/"
        );
    }

    #[test]
    fn un_fichero_limpio_con_defectos_se_rechaza() {
        let mut f = ficha(LIMPIO);
        f.defectos = ficha(AUDITORIA).defectos;

        let error = validar_ficha(&f, &presentes(&["texto.rs"])).unwrap_err();

        assert_eq!(error, ProblemaDeFicha::LimpioConDefectos("U05".into()));
    }

    #[test]
    fn la_clase_fuera_de_la_familia_uso_se_rechaza() {
        let mut f = ficha(OFICIO);
        f.clase = Some(ClaseDeUso::Formas);

        let error = validar_ficha(&f, &presentes(&[])).unwrap_err();

        assert_eq!(error, ProblemaDeFicha::ClaseQueNoCasa("E03".into()));
    }

    #[test]
    fn la_familia_uso_sin_clase_se_rechaza() {
        let mut f = ficha(AUDITORIA);
        f.clase = None;

        assert_eq!(
            validar_ficha(&f, &presentes(&["Pedidos.cs"])),
            Err(ProblemaDeFicha::ClaseQueNoCasa("U01".into()))
        );
    }

    #[test]
    fn un_encargo_de_rubrica_sin_rubrica_se_rechaza() {
        let mut sin_rubrica = presentes(&[]);
        sin_rubrica.rubrica = false;

        let error = validar_ficha(&ficha(CRITERIO), &sin_rubrica).unwrap_err();

        assert_eq!(error.to_string(), "E10a: falta rubrica.md");
    }

    #[test]
    fn un_encargo_sin_enunciado_se_rechaza() {
        let mut sin_enunciado = presentes(&[]);
        sin_enunciado.enunciado = false;

        assert_eq!(
            validar_ficha(&ficha(OFICIO), &sin_enunciado),
            Err(ProblemaDeFicha::SinEnunciado("E03".into()))
        );
    }

    #[test]
    fn un_requisito_de_texto_necesita_un_solo_limite() {
        let mut f = ficha(OFICIO);
        f.texto[0].minimo = Some(1);

        let error = validar_ficha(&f, &presentes(&[])).unwrap_err();

        assert!(error.to_string().contains("«sin unwrap»"));
    }

    #[test]
    fn un_maximo_solo_mira_lo_anterior_a_la_marca() {
        let requisito = ficha(OFICIO).texto.remove(0);

        assert!(requisito.se_cumple_en("fn a() {}\n#[cfg(test)]\nx.unwrap();"));
        assert!(!requisito.se_cumple_en("x.unwrap();\n#[cfg(test)]"));
    }

    #[test]
    fn un_minimo_cuenta_las_apariciones_y_puede_ignorar_mayusculas() {
        let requisito = RequisitoDeTexto {
            nombre: "ventana".into(),
            patron: "OVER".into(),
            maximo: None,
            minimo: Some(2),
            antes_de: None,
            ignorar_mayusculas: true,
        };

        assert!(requisito.se_cumple_en("rank() over (...), sum(x) OVER (...)"));
        assert!(!requisito.se_cumple_en("rank() over (...)"));
    }

    fn encargo(texto: &str) -> Encargo {
        Encargo {
            ficha: ficha(texto),
            carpeta: PathBuf::new(),
        }
    }

    #[test]
    fn los_encargos_salen_ordenados_por_identificador() {
        let ordenados =
            ordenar_sin_repetidos(vec![encargo(LIMPIO), encargo(OFICIO), encargo(AUDITORIA)])
                .unwrap();

        let ids: Vec<&str> = ordenados.iter().map(|e| e.ficha.id.as_str()).collect();
        assert_eq!(ids, ["E03", "U01", "U05"]);
    }

    #[test]
    fn un_identificador_repetido_se_rechaza() {
        let error = ordenar_sin_repetidos(vec![encargo(OFICIO), encargo(OFICIO)]).unwrap_err();

        assert_eq!(error.to_string(), "el identificador E03 está repetido");
    }

    fn escribir(ruta: &Path, contenido: &str) {
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        std::fs::write(ruta, contenido).unwrap();
    }

    #[test]
    fn una_carpeta_sin_encargos_es_un_error() {
        let dir = tempfile::tempdir().unwrap();
        escribir(&dir.path().join("README.md"), "nada");

        let error = cargar_encargos(dir.path()).unwrap_err();

        assert!(error.to_string().contains("no hay ningún encargo"));
    }

    #[test]
    fn cargar_lee_las_subcarpetas_y_los_ficheros_de_partida_anidados() {
        let dir = tempfile::tempdir().unwrap();
        let u01 = dir.path().join("U01");
        escribir(
            &u01.join(NOMBRE_FICHA),
            &AUDITORIA.replace("\"Pedidos.cs\"", "\"src/Pedidos.cs\""),
        );
        escribir(&u01.join(NOMBRE_ENUNCIADO), "Audita.");
        escribir(
            &u01.join(CARPETA_PARTIDA).join("src/Pedidos.cs"),
            "class X {}",
        );
        let e03 = dir.path().join("E03");
        escribir(&e03.join(NOMBRE_FICHA), OFICIO);
        escribir(&e03.join(NOMBRE_ENUNCIADO), "Escribe.");

        let encargos = cargar_encargos(dir.path()).unwrap();

        assert_eq!(encargos.len(), 2);
        assert_eq!(encargos[0].ficha.id, "E03");
        assert_eq!(encargos[1].carpeta, u01);
    }

    #[test]
    fn una_ficha_ilegible_dice_en_que_carpeta_esta() {
        let dir = tempfile::tempdir().unwrap();
        let carpeta = dir.path().join("E99");
        escribir(&carpeta.join(NOMBRE_FICHA), "id = ");
        escribir(&carpeta.join(NOMBRE_ENUNCIADO), "x");

        let error = cargar_encargos(dir.path()).unwrap_err();

        assert!(error
            .to_string()
            .contains("la ficha de «E99» no se puede leer"));
    }

    /// Los encargos reales se publican desde el 12/11/2026, así que también el árbol público los
    /// carga. Un árbol sin ellos no tiene nada que cargar, y lo dice.
    #[test]
    fn los_encargos_reales_del_repositorio_se_cargan() {
        let carpeta = Path::new(env!("CARGO_MANIFEST_DIR")).join("evaluacion/banco/encargos");
        if !carpeta.is_dir() {
            eprintln!(
                "sin encargos en {}: un árbol sin el banco",
                carpeta.display()
            );
            return;
        }

        let encargos = cargar_encargos(&carpeta).unwrap();

        let que_cuentan: BTreeSet<&str> = encargos
            .iter()
            .map(|e| e.ficha.encargo_que_cuenta())
            .collect();
        assert_eq!(
            encargos.len(),
            17,
            "E01 a E09, tres variantes de E10 y U01 a U05"
        );
        assert_eq!(que_cuentan.len(), 15);
        // Lo que descarga va en [preparar]: compilar ya ejecuta el código del modelo, sin red.
        let con_preparacion: Vec<&str> = encargos
            .iter()
            .filter(|e| e.ficha.preparar.is_some())
            .map(|e| e.ficha.id.as_str())
            .collect();
        assert_eq!(con_preparacion, ["E04", "E06"]);
    }

    /// Sin pruebas ocultas, el código se valora leyéndolo con su
    /// rúbrica. Un encargo de código del banco sin ella se quedaría en «no evaluado».
    #[test]
    fn cada_encargo_de_codigo_del_repositorio_trae_su_rubrica() {
        let carpeta = Path::new(env!("CARGO_MANIFEST_DIR")).join("evaluacion/banco/encargos");
        if !carpeta.is_dir() {
            return;
        }

        let sin_rubrica: Vec<String> = cargar_encargos(&carpeta)
            .unwrap()
            .iter()
            .filter(|e| e.ficha.forma_de_puntuar() == Some(FormaDePuntuar::Comprobaciones))
            .filter(|e| !e.carpeta.join(NOMBRE_RUBRICA).is_file())
            .map(|e| e.ficha.id.clone())
            .collect();

        assert!(
            sin_rubrica.is_empty(),
            "sin {NOMBRE_RUBRICA}: {sin_rubrica:?}"
        );
    }
}
