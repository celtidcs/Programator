//! Versiones, respuesta de GitHub y decisión de avisar.
//!
//! Todo lo de este módulo decide sin tocar la red: leer la respuesta, comparar versiones y
//! construir el aviso. Por eso se comprueba con textos fijos.
//!
//! La respuesta es un dato ajeno. De la respuesta de releases/latest solo se toma tag_name, y
//! solo si es exactamente X.Y.Z con una v opcional delante. El enlace del aviso no sale de la
//! respuesta, porque html_url podría apuntar a cualquier sitio: se compone con la dirección web
//! y el repositorio de los ajustes, más la etiqueta ya validada, que solo contiene dígitos,
//! puntos y quizá una v.

use crate::origen::OrigenDeLasVersiones;
use std::fmt;

/// Prefijo opcional con el que GitHub suele nombrar las etiquetas de versión, como en v0.12.0.
const PREFIJO_DE_ETIQUETA: char = 'v';

/// Número de componentes de una versión mayor.menor.parche.
const COMPONENTES_DE_UNA_VERSION: usize = 3;

/// Por qué no se pudo comprobar si hay una versión nueva.
///
/// Ninguno de estos fallos es grave: la comprobación es una cortesía y, si falla, simplemente
/// no se avisa. Quien use la biblioteca decide si los registra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalloDeComprobacion {
    /// No se pudo hablar con el servidor: sin red, tiempo agotado, certificado o estado HTTP
    /// de error, incluido el límite de peticiones.
    Red(String),
    /// La respuesta no es el JSON esperado o no trae tag_name como texto.
    RespuestaIlegible(String),
    /// La etiqueta publicada, o la versión local, no tiene la forma X.Y.Z.
    EtiquetaNoValida(String),
    /// Los ajustes no se pueden usar con seguridad.
    AjustesNoValidos(String),
    /// El nombre con que el programa quiere presentarse en la cabecera User-Agent no es
    /// válido.
    IdentificacionNoValida(String),
}

impl fmt::Display for FalloDeComprobacion {
    /// Texto técnico del fallo, pensado para un registro.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FalloDeComprobacion::Red(detalle) => write!(f, "fallo de red: {detalle}"),
            FalloDeComprobacion::RespuestaIlegible(detalle) => {
                write!(f, "respuesta ilegible: {detalle}")
            }
            FalloDeComprobacion::EtiquetaNoValida(etiqueta) => {
                write!(f, "etiqueta de versión no válida: «{etiqueta}»")
            }
            FalloDeComprobacion::AjustesNoValidos(detalle) => {
                write!(f, "ajustes de comprobación no válidos: {detalle}")
            }
            FalloDeComprobacion::IdentificacionNoValida(nombre) => {
                write!(f, "nombre de programa no válido: «{nombre}»")
            }
        }
    }
}

impl std::error::Error for FalloDeComprobacion {}

/// Versión mayor.menor.parche comparable como números.
///
/// El orden derivado compara campo a campo en el orden de declaración, que es justo el de la
/// numeración semántica: por eso 0.12.0 es mayor que 0.9.9, cosa que una comparación de texto
/// haría al revés.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VersionSemantica {
    /// Primer número.
    mayor: u64,
    /// Segundo número.
    menor: u64,
    /// Tercer número.
    parche: u64,
}

impl VersionSemantica {
    /// Lee una etiqueta como v0.12.0 o 0.12.0.
    ///
    /// Se rechaza todo lo que no sea exactamente tres números separados por puntos, con una v
    /// minúscula opcional delante: sufijos de prepublicación como -rc1, metadatos como
    /// +compilacion, espacios y números que no caben en u64. Una versión que no se puede
    /// comparar con seguridad no puede provocar un aviso.
    ///
    /// Devuelve la versión, o None si la etiqueta no tiene la forma esperada.
    pub fn desde_etiqueta(etiqueta: &str) -> Option<Self> {
        let numeros = etiqueta
            .strip_prefix(PREFIJO_DE_ETIQUETA)
            .unwrap_or(etiqueta);
        let partes: Vec<&str> = numeros.split('.').collect();
        if partes.len() != COMPONENTES_DE_UNA_VERSION {
            return None;
        }
        let mut valores = [0_u64; COMPONENTES_DE_UNA_VERSION];
        for (valor, parte) in valores.iter_mut().zip(&partes) {
            // u64::from_str admite un signo más delante; aquí solo valen dígitos.
            if parte.is_empty() || !parte.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            *valor = parte.parse().ok()?;
        }
        let [mayor, menor, parche] = valores;
        Some(Self {
            mayor,
            menor,
            parche,
        })
    }
}

impl fmt::Display for VersionSemantica {
    /// Escribe la versión como mayor.menor.parche, sin prefijo.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.mayor, self.menor, self.parche)
    }
}

/// Lo que hay que decir al usuario cuando existe una versión más nueva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvisoDeVersionNueva {
    /// La versión publicada, sin prefijo, como 0.13.0.
    pub version: String,
    /// Página web de esa versión, compuesta desde los ajustes y nunca desde la respuesta.
    pub enlace: String,
}

/// Saca tag_name de la respuesta de releases/latest.
///
/// El parámetro cuerpo es el texto de la respuesta, ya acotado en tamaño por quien lo leyó.
///
/// Devuelve FalloDeComprobacion::RespuestaIlegible si no es un objeto JSON con tag_name de
/// texto. Es lo que llega, por ejemplo, cuando GitHub responde con una página de error.
pub fn interpretar_respuesta(cuerpo: &str) -> Result<String, FalloDeComprobacion> {
    /// Lo único que interesa de la respuesta; el resto de campos se ignora.
    #[derive(serde::Deserialize)]
    struct UltimaVersion {
        /// Etiqueta git de la versión publicada.
        tag_name: String,
    }
    serde_json::from_str::<UltimaVersion>(cuerpo)
        .map(|ultima| ultima.tag_name)
        .map_err(|error| FalloDeComprobacion::RespuestaIlegible(error.to_string()))
}

/// Decide si hay que avisar de una versión nueva.
///
/// Recibe la versión que se está ejecutando, la etiqueta de la última versión publicada y el
/// origen ya validado del que salen los enlaces. Devuelve Some con el aviso solo si la
/// publicada es estrictamente mayor que la local, y FalloDeComprobacion::EtiquetaNoValida si
/// alguna de las dos no tiene la forma X.Y.Z.
pub fn decidir_aviso(
    version_local: &str,
    etiqueta_publicada: &str,
    origen: &OrigenDeLasVersiones,
) -> Result<Option<AvisoDeVersionNueva>, FalloDeComprobacion> {
    let no_valida = |etiqueta: &str| FalloDeComprobacion::EtiquetaNoValida(etiqueta.to_string());
    let local =
        VersionSemantica::desde_etiqueta(version_local).ok_or_else(|| no_valida(version_local))?;
    let publicada = VersionSemantica::desde_etiqueta(etiqueta_publicada)
        .ok_or_else(|| no_valida(etiqueta_publicada))?;

    if publicada <= local {
        return Ok(None);
    }
    Ok(Some(AvisoDeVersionNueva {
        version: publicada.to_string(),
        enlace: origen.enlace_de_la_version(etiqueta_publicada),
    }))
}
