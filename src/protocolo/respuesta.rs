//! La cabecera de una respuesta de Programator, para que despierte a quien encargó y a nadie más.
//!
//! Por compatibilidad con Gestor Agentes, cada respuesta lleva, tras su `LATIDO:`, la línea
//! `Para <identidad>: respuesta a <nombre corto>`, y debajo `RECIBIDO <hora>: <ETIQUETA>` si el
//! encargo trae etiqueta. El resto del texto publicado no puede despertar a nadie: cada «Para» que
//! Gestor leería como encargo se escribe en minúscula ([`neutralizar_para`]).
//!
//! Lógica pura: los textos llegan como argumentos.

use super::destinatarios::{self, Identidad, FIN_DE_LISTA, PALABRA_DEL_ENCARGO};
use super::encargo::{Encargo, Forma};
use std::borrow::Cow;

/// Lo que precede a la identidad en la primera línea de una bandeja de Gestor Agentes
/// (`# Bandeja de Claude 2`, plantilla de bandeja de `crates/nucleo/src/plantillas.rs`).
pub const CABECERA_DE_BANDEJA: &str = "# Bandeja de ";

/// Identidades de ejemplo al pedirle a un agente que diga quién es: varias herramientas, porque
/// Programator no sabe cuál le va a encargar, y la identidad la elige el agente.
pub const IDENTIDADES_DE_EJEMPLO: [&str; 3] = ["Claude 1", "Codex 2", "Gemini 1"];

/// Los ejemplos de identidad tal como se escriben en un texto: «Claude 1», «Codex 2» o «Gemini 1».
pub fn ejemplos_de_identidad() -> String {
    let citados: Vec<String> = IDENTIDADES_DE_EJEMPLO
        .iter()
        .map(|identidad| format!("«{identidad}»"))
        .collect();
    match citados.split_last() {
        Some((ultimo, resto)) if !resto.is_empty() => format!("{} o {ultimo}", resto.join(", ")),
        _ => citados.concat(),
    }
}
/// Palabra del acuse que lee Gestor Agentes (`acuses.rs`, en cualquier mayúscula).
const PALABRA_DE_ACUSE: &str = "RECIBIDO";
/// Largo mínimo de una etiqueta, como en `acuses.rs` de Gestor: «M1» o «T4» son demasiado comunes.
const LARGO_MINIMO_DE_ETIQUETA: usize = 3;
/// Cómo se nombra el encargo en la línea `Para` si no trae nada que lo nombre.
const NOMBRE_SIN_TEXTO: &str = "tu encargo";
/// «Para» en minúscula, que no despierta a nadie.
const PARA_NEUTRO: &str = "para ";

/// Lo que va tras el `LATIDO:` de una respuesta: a quién despierta, cómo se llama el encargo y,
/// si lo trae, su etiqueta para el acuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cabecera {
    identidad: Identidad,
    nombre: String,
    etiqueta: Option<String>,
}

impl Cabecera {
    /// Quién encargó.
    pub fn identidad(&self) -> &Identidad {
        &self.identidad
    }

    /// La cabecera de la respuesta a `encargo`, que `agente` leyó en un buzón con
    /// `contenido_del_buzon`. `None` si no se sabe quién lo escribió: no hay a quién despertar.
    pub fn de_encargo(encargo: &Encargo, contenido_del_buzon: &str, agente: &str) -> Option<Self> {
        let identidad = identidad_del_buzon(contenido_del_buzon)?;
        // Bajo un encabezado o con un marcador, el texto guardado ya no lleva la línea `Para`: se
        // rehace para leer el nombre y la etiqueta con la misma regla.
        let texto = if encargo.forma == Forma::Linea {
            Cow::Borrowed(encargo.texto.as_str())
        } else {
            Cow::Owned(format!("{PALABRA_DEL_ENCARGO}{agente}: {}", encargo.texto))
        };
        let primera = texto.lines().next().unwrap_or_default();
        Some(Self {
            identidad,
            nombre: nombre_corto(&texto, agente),
            etiqueta: etiqueta_para(primera, agente),
        })
    }

    /// Las líneas de la cabecera: la línea `Para` y, si hay etiqueta, el acuse con `hora`.
    pub fn lineas(&self, hora: &str) -> String {
        let mut lineas = linea_para(&self.identidad, &self.nombre);
        if let Some(etiqueta) = &self.etiqueta {
            lineas.push('\n');
            lineas.push_str(&linea_de_acuse(hora, etiqueta));
        }
        lineas
    }
}

/// Etiqueta del encargo en su primera línea, con la regla de `acuses::etiqueta` de Gestor Agentes
/// 3.7.0: la primera palabra, tras la lista de destinatarios, en mayúsculas y cifras con al menos
/// una cifra o un guion (`PING-1`, `VERIF-360D`).
pub fn etiqueta(primera_linea: &str) -> Option<String> {
    let tras_para = primera_linea.split_once(PALABRA_DEL_ENCARGO)?.1;
    let resto = tras_para.split_once(|c| FIN_DE_LISTA.contains(&c))?.1;
    palabras(resto)
        .find(|palabra| es_etiqueta(palabra))
        .map(str::to_owned)
}

/// La etiqueta del encargo para `agente`: si la línea trae su «Para» con dos puntos, la primera
/// etiqueta tras ellos; si no, la de [`etiqueta`]. No sale del primer «Para» de la línea, que
/// puede ser de otro agente.
pub fn etiqueta_para(primera_linea: &str, agente: &str) -> Option<String> {
    match destinatarios::tras_los_dos_puntos(primera_linea, &Identidad::de(agente)) {
        Some(resto) => palabras(resto)
            .find(|palabra| es_etiqueta(palabra))
            .map(str::to_owned),
        None => etiqueta(primera_linea),
    }
}

/// Cómo se nombra el encargo en la respuesta: lo que va tras los dos puntos del «Para» que nombra
/// a `agente`, hasta el final de esa línea; si no trae nada, la primera palabra de la línea
/// siguiente con texto. Sin «Para» que despierte a nadie.
pub fn nombre_corto(texto_del_encargo: &str, agente: &str) -> String {
    let mut lineas = texto_del_encargo.lines();
    let primera = lineas.next().unwrap_or_default();
    let la_etiqueta = etiqueta_para(primera, agente);
    let en_la_linea = destinatarios::tras_los_dos_puntos(primera, &Identidad::de(agente))
        .map(|resto| resto.trim().trim_matches(['*', '_']).trim())
        .map(|resto| sin_la_etiqueta_delante(resto, la_etiqueta.as_deref()))
        .filter(|resto| !resto.is_empty());
    let nombre = en_la_linea.map(str::to_owned).or_else(|| {
        lineas
            .find_map(|linea| palabras(linea).next())
            .map(str::to_owned)
    });
    neutralizar_para(nombre.as_deref().unwrap_or(NOMBRE_SIN_TEXTO))
}

/// `texto` sin la etiqueta, si es su primera palabra: ya va en el acuse, y en el nombre se
/// repetía.
fn sin_la_etiqueta_delante<'a>(texto: &'a str, etiqueta: Option<&str>) -> &'a str {
    etiqueta
        .and_then(|etiqueta| texto.strip_prefix(etiqueta))
        .filter(|resto| !resto.starts_with(sigue_la_palabra))
        .map_or(texto, |resto| {
            resto.trim_start_matches(|c: char| c.is_whitespace() || TRAS_LA_ETIQUETA.contains(&c))
        })
}

/// Si el carácter continuaría la palabra: entonces lo que empieza por la etiqueta es otra palabra.
fn sigue_la_palabra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Lo que se suele escribir entre la etiqueta y el nombre del encargo (`PR-1: escribe…`).
const TRAS_LA_ETIQUETA: [char; 4] = [':', '.', ',', ';'];

/// Quién escribió la bandeja: la identidad de su primera línea (`# Bandeja de Claude 2`), leída
/// con la regla de Gestor. `None` si no la trae. Solo la cabecera; el nombre del fichero no
/// dice nada, porque la regla que lo traduce solo vale en los proyectos de Gestor Agentes.
pub fn identidad_del_buzon(contenido: &str) -> Option<Identidad> {
    contenido
        .lines()
        .next()
        .and_then(|linea| linea.trim().strip_prefix(CABECERA_DE_BANDEJA))
        .and_then(Identidad::leer)
}

/// Lo que se añade a una respuesta cuando el buzón `buzon` no dice quién lo escribe: sin eso, la
/// respuesta no puede despertar a nadie. La identidad la declara el agente en su buzón;
/// Programator no la adivina por el nombre del fichero.
pub fn aviso_sin_identidad(buzon: &str) -> String {
    let ejemplos = ejemplos_de_identidad();
    format!(
        "⚠️ No sé quién ha escrito este encargo: la primera línea de «{buzon}» no es \
         «{CABECERA_DE_BANDEJA}<tu identidad>» (tu herramienta y tu número, como {ejemplos}), \
         así que esta respuesta no te avisará. Escribe esa línea al principio de tu buzón y lanza el \
         vigilante con la misma identidad."
    )
}

/// La línea que despierta a quien encargó.
pub fn linea_para(identidad: &Identidad, nombre_corto: &str) -> String {
    format!("{PALABRA_DEL_ENCARGO}{identidad}: respuesta a {nombre_corto}")
}

/// Una línea `Para` a varios, separados por «y» como lee Gestor Agentes, con un asunto tras los
/// dos puntos. `None` sin nadie a quien dirigirla.
pub fn linea_para_varios(identidades: &[Identidad], asunto: &str) -> Option<String> {
    if identidades.is_empty() {
        return None;
    }
    let nombres: Vec<String> = identidades.iter().map(ToString::to_string).collect();
    Some(format!(
        "{PALABRA_DEL_ENCARGO}{}: {}",
        nombres.join(" y "),
        neutralizar_para(asunto)
    ))
}

/// La línea con la que Gestor Agentes da el encargo por recibido.
pub fn linea_de_acuse(hora: &str, etiqueta: &str) -> String {
    format!("{PALABRA_DE_ACUSE} {hora}: {etiqueta}")
}

/// El texto con cada «Para» que Gestor leería como encargo (palabra suelta con mayúscula) escrito
/// en minúscula: se lee igual y no despierta a nadie. «Parámetro» o «SepáraPara» no se tocan.
pub fn neutralizar_para(texto: &str) -> String {
    let mut resultado = String::with_capacity(texto.len());
    let mut copiado_hasta = 0;
    for (posicion, suelto) in destinatarios::apariciones_del_para(texto) {
        resultado.push_str(&texto[copiado_hasta..posicion]);
        resultado.push_str(if suelto {
            PARA_NEUTRO
        } else {
            PALABRA_DEL_ENCARGO
        });
        copiado_hasta = posicion + PALABRA_DEL_ENCARGO.len();
    }
    resultado.push_str(&texto[copiado_hasta..]);
    resultado
}

/// Palabras de una línea: tramos de letras, cifras y guiones, como en `acuses.rs` de Gestor.
fn palabras(texto: &str) -> impl Iterator<Item = &str> {
    texto
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .filter(|palabra| !palabra.is_empty())
}

/// Si una palabra tiene forma de etiqueta, como en `acuses.rs` de Gestor.
fn es_etiqueta(palabra: &str) -> bool {
    palabra.chars().count() >= LARGO_MINIMO_DE_ETIQUETA
        && palabra.chars().any(char::is_uppercase)
        && palabra
            .chars()
            .all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '-')
        && palabra.chars().any(|c| c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn encargo(de: &str, texto: &str, forma: Forma) -> Encargo {
        Encargo {
            latido: None,
            de: de.to_string(),
            texto: texto.to_string(),
            forma,
        }
    }

    #[test]
    fn la_cabecera_de_un_encargo_en_linea_lleva_para_y_acuse() {
        let en_linea = encargo(
            "claude2.md",
            "Para Programator: PG-A contar\nmás",
            Forma::Linea,
        );

        let cabecera =
            Cabecera::de_encargo(&en_linea, "# Bandeja de Claude 2\n", "Programator").unwrap();

        assert_eq!(
            cabecera.lineas("15:00:00"),
            "Para Claude 2: respuesta a contar\nRECIBIDO 15:00:00: PG-A"
        );
    }

    #[test]
    fn sin_etiqueta_no_hay_acuse_y_sin_identidad_no_hay_cabecera() {
        let sin_etiqueta = encargo("codex.md", "Para Programator: revisa", Forma::Linea);
        assert_eq!(
            Cabecera::de_encargo(&sin_etiqueta, "# Bandeja de Codex 1\n", "Programator")
                .unwrap()
                .lineas("15:00:00"),
            "Para Codex 1: respuesta a revisa"
        );

        for con_nombre_de_gestor in ["claude.md", "claude2.md", "codex.md"] {
            let sin_cabecera = encargo(con_nombre_de_gestor, "Para Programator: x", Forma::Linea);
            assert_eq!(
                Cabecera::de_encargo(&sin_cabecera, "LATIDO: x\n", "Programator"),
                None,
                "{con_nombre_de_gestor}"
            );
        }
        let sin_identidad = encargo("2026.md", "Para Programator: x", Forma::Linea);
        assert_eq!(
            Cabecera::de_encargo(&sin_identidad, "", "Programator"),
            None
        );
    }

    #[test]
    fn bajo_un_encabezado_el_nombre_y_la_etiqueta_salen_de_su_primera_linea() {
        let bajo = encargo("claude.md", "E03-B prueba\n\nHaz esto.", Forma::Encabezado);

        assert_eq!(
            Cabecera::de_encargo(&bajo, "# Bandeja de Claude 1\n", "Programator")
                .unwrap()
                .lineas("09:00:00"),
            "Para Claude 1: respuesta a prueba\nRECIBIDO 09:00:00: E03-B"
        );
    }

    /// La etiqueta no sale del primer «Para» de la línea si es de otro.
    #[test]
    fn la_etiqueta_sale_del_para_del_agente() {
        let linea = "Para Gemini 1: X1-A. Para Programator: PG-A cuenta";
        assert_eq!(etiqueta_para(linea, "Programator").as_deref(), Some("PG-A"));
        assert_eq!(
            Cabecera::de_encargo(
                &encargo("claude2.md", linea, Forma::Linea),
                "# Bandeja de Claude 2\n",
                "Programator"
            )
            .unwrap()
            .lineas("15:00:00"),
            "Para Claude 2: respuesta a cuenta\nRECIBIDO 15:00:00: PG-A"
        );
        // Sin «Para Programator:» con dos puntos, la regla de siempre.
        assert_eq!(
            etiqueta_para("Para Programator. Petición VERIF-1: x", "Programator").as_deref(),
            Some("VERIF-1")
        );
    }

    #[test]
    fn la_etiqueta_con_los_casos_de_gestor() {
        // Los de `crates/canal/tests/acuses.rs` de Gestor Agentes 3.7.0.
        assert_eq!(etiqueta("Para Codex 1: PING-1").as_deref(), Some("PING-1"));
        assert_eq!(
            etiqueta("Para Gemini 1. Petición VERIF-360D: verifica F17").as_deref(),
            Some("VERIF-360D")
        );
        assert_eq!(etiqueta("Para Codex 1: VC6-1").as_deref(), Some("VC6-1"));
        assert_eq!(etiqueta("Para Codex 1: revisa esto"), None);
        assert_eq!(etiqueta("Para Codex 1: T4"), None);
        assert_eq!(etiqueta("Para Codex 1: URGENTE revisa"), None);
        // Sin mayúscula o con minúsculas no es etiqueta.
        assert_eq!(etiqueta("Para Codex 1: 2026-11-22 revisa"), None);
        assert_eq!(etiqueta("Para Codex 1: 100 pruebas"), None);
        assert_eq!(etiqueta("Para Codex 1: Revision-1 y v1-beta"), None);
        assert_eq!(
            etiqueta("Para Programator: PG-A contar").as_deref(),
            Some("PG-A")
        );
        assert_eq!(etiqueta("Sin destinatarios PG-A"), None);
    }

    #[test]
    fn el_nombre_corto_sale_de_la_linea_del_para_o_de_la_siguiente() {
        assert_eq!(
            nombre_corto("Para Programator: X1 prueba\nmás texto", "Programator"),
            "X1 prueba"
        );
        assert_eq!(
            nombre_corto(
                "Para Gemini 1: otra. **Para Programator:** revisión: buscar",
                "Programator"
            ),
            "revisión: buscar"
        );
        assert_eq!(
            nombre_corto("Para Programator:\n\nrevisión: probar\n", "Programator"),
            "revisión"
        );
        assert_eq!(
            nombre_corto("## Para Programator", "Programator"),
            NOMBRE_SIN_TEXTO
        );
        // La etiqueta ya va en el acuse; en el nombre se repetía.
        assert_eq!(
            nombre_corto("Para Programator: PR-1 escribe sat.py", "Programator"),
            "escribe sat.py"
        );
        assert_eq!(
            nombre_corto("Para Programator: PR-1\nRevisa el pago", "Programator"),
            "Revisa"
        );
        assert_eq!(
            nombre_corto("Para Programator: PR-1", "Programator"),
            NOMBRE_SIN_TEXTO
        );
        assert_eq!(
            nombre_corto("Para Programator: revisa PR-1 hoy", "Programator"),
            "revisa PR-1 hoy"
        );
        assert_eq!(
            nombre_corto("Para Programator: PR-1: escribe sat.py", "Programator"),
            "escribe sat.py"
        );
        assert_eq!(
            nombre_corto("Para Programator: PR-1_x escribe", "Programator"),
            "PR-1_x escribe"
        );
        assert_eq!(
            nombre_corto("Para Programator: díselo Para Gemini 1: ya", "Programator"),
            "díselo para Gemini 1: ya"
        );
    }

    #[test]
    fn la_identidad_sale_solo_de_la_cabecera() {
        // Solo la cabecera: deducirla del nombre del fichero con la regla de Gestor solo valdría
        // en los proyectos de Gestor Agentes.
        let de = |contenido: &str| identidad_del_buzon(contenido).map(|id| id.to_string());
        assert_eq!(
            de("# Bandeja de Claude 2\n\nLATIDO: …").as_deref(),
            Some("Claude 2")
        );
        assert_eq!(de("# Bandeja de Codex 3").as_deref(), Some("Codex 3"));
        assert_eq!(de(""), None);
        assert_eq!(de("# Otra cosa\n"), None);
        assert_eq!(de("# Bandeja de ???\n"), None);
        assert_eq!(de("LATIDO: x\n# Bandeja de Claude 2\n"), None);
    }

    #[test]
    fn la_linea_para_nombra_a_quien_encargo_y_a_nadie_mas() {
        let claude2 = Identidad::leer("Claude 2").unwrap();
        let nombre = nombre_corto(
            "Para Programator: avisa Para Gemini 1, Codex: x",
            "Programator",
        );
        let linea = linea_para(&claude2, &nombre);

        assert!(linea.starts_with("Para Claude 2: respuesta a "), "{linea}");
        assert_eq!(destinatarios::destinatarios(&linea), ["Claude 2"]);
        for otro in ["Gemini 1", "Codex", "Programator", "Claude 1"] {
            assert!(!Identidad::de(otro).es_destinatario_de(&linea), "{otro}");
        }
    }

    #[test]
    fn el_aviso_sin_identidad_dice_el_buzon_y_la_linea_que_falta_sin_despertar_a_nadie() {
        let aviso = aviso_sin_identidad("claude-1.md");

        assert!(aviso.contains("«claude-1.md»"), "{aviso}");
        assert!(aviso.contains("«# Bandeja de <tu identidad>»"), "{aviso}");
        assert!(
            aviso.contains("«Claude 1», «Codex 2» o «Gemini 1»"),
            "{aviso}"
        );
        assert!(destinatarios::destinatarios(&aviso).is_empty(), "{aviso}");
    }

    #[test]
    fn la_linea_a_varios_los_nombra_a_todos_y_a_nadie_mas() {
        let ids: Vec<Identidad> = ["Claude 1", "Codex 2"]
            .iter()
            .filter_map(|id| Identidad::leer(id))
            .collect();

        let linea = linea_para_varios(&ids, "ninguno cabe").unwrap();

        assert_eq!(linea, "Para Claude 1 y Codex 2: ninguno cabe");
        assert_eq!(
            destinatarios::destinatarios(&linea),
            ["Claude 1", "Codex 2"]
        );
        assert_eq!(linea_para_varios(&[], "x"), None);

        let con_para = linea_para_varios(&ids, "ninguno cabe. Para Gemini 1: nada").unwrap();
        assert_eq!(
            destinatarios::destinatarios(&con_para),
            ["Claude 1", "Codex 2"],
            "{con_para}"
        );
        assert!(!con_para.contains("Para Gemini"), "{con_para}");
    }

    #[test]
    fn el_acuse_lleva_la_palabra_y_la_etiqueta_como_lo_lee_gestor() {
        let linea = linea_de_acuse("15:48:44", "PG-A");

        assert_eq!(linea, "RECIBIDO 15:48:44: PG-A");
        // La regla de `acuses::acusado` de Gestor: «recibido» en cualquier mayúscula y la etiqueta
        // como palabra.
        assert!(linea.to_lowercase().contains("recibido"));
        assert!(palabras(&linea).any(|palabra| palabra == "PG-A"));
        assert!(destinatarios::destinatarios(&linea).is_empty());
    }

    #[test]
    fn neutralizar_para_solo_toca_el_para_que_despierta() {
        let casos = [
            ("Para Gemini 1: haz", "para Gemini 1: haz"),
            ("**Para Gemini 1:** haz", "**para Gemini 1:** haz"),
            ("Hoy. Para Codex: x", "Hoy. para Codex: x"),
            ("> Para Claude 1: x", "> para Claude 1: x"),
            ("Para Para Codex", "para para Codex"),
            ("Parámetro y para Codex", "Parámetro y para Codex"),
            ("SepáraPara Codex", "SepáraPara Codex"),
            ("Sin nada", "Sin nada"),
        ];
        for (texto, esperado) in casos {
            let neutro = neutralizar_para(texto);
            assert_eq!(neutro, esperado, "{texto}");
            assert!(destinatarios::destinatarios(&neutro).is_empty(), "{texto}");
        }
    }
}
