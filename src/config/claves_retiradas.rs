//! Las claves que `programator.toml` admitía y ya no valen: cuáles son y cómo se quitan del
//! fichero antes de interpretarlo, con un aviso por cada una.

use super::mensajes::aviso_de_clave_retirada;
use super::Config;

/// Una clave que `programator.toml` admitía y ya no vale. Si el fichero la trae, no se lee: se
/// quita antes de interpretarlo y se avisa al arrancar. Un TOML con una clave que nadie lee se
/// cargaría sin decir nada, y quien la puso creería que vale; y en una sección que rechaza las
/// claves desconocidas, el programa no arrancaría por una línea que ya no significa nada.
pub(super) struct ClaveRetirada {
    pub(super) seccion: &'static str,
    pub(super) clave: &'static str,
    /// Por qué ya no vale y qué pasa ahora en su lugar.
    pub(super) porque: &'static str,
    pub(super) que_hacer: &'static str,
}

/// Las claves retiradas, cada una con la orden que la quitó.
const CLAVES_RETIRADAS: [ClaveRetirada; 7] = [
    // Retiradas el 22/11/2026: no se leían en ningún sitio.
    ClaveRetirada {
        seccion: "agente",
        clave: "buzon",
        porque: "Programator escribe siempre en el buzón que sale de su nombre, «[agente] nombre» \
                 en minúsculas y con «.md»; esta clave no cambiaba nada.",
        que_hacer: "bórrala; para cambiar de buzón, cambia el nombre.",
    },
    ClaveRetirada {
        seccion: "poda",
        clave: "umbral_lineas",
        porque: "La poda se propone al cerrar cada encargo, sin contar líneas; esta clave no \
                 cambiaba nada.",
        que_hacer: "bórrala, y la sección [poda] si se queda vacía.",
    },
    // Retirada el 22/11/2026: la identidad sale solo de la cabecera del buzón y la lista era de
    // nombres de fichero.
    ClaveRetirada {
        seccion: "revision",
        clave: "buzones_que_ordenan",
        porque: "Quien puede pedir una revisión se reconoce por la identidad que declara en la \
                 primera línea de su buzón («# Bandeja de Claude 1»), no por el nombre del \
                 fichero. Hasta que la cambies, nadie puede pedir una revisión por el canal.",
        que_hacer: "cámbiala por «identidades_que_ordenan = [\"Claude 1\"]», con las identidades \
                    de quienes pueden pedirla.",
    },
    // Retirada el 13/11/2026: los históricos del canal son dos prefijos fijos.
    ClaveRetirada {
        seccion: "agente",
        clave: "prefijos_ignorados",
        porque: "Los históricos del canal son los ficheros que empiezan por «historico-» o \
                 «histórico-», y ningún otro.",
        que_hacer: "quita esa línea; si tu proyecto archiva con otro prefijo, renómbralo a uno de \
                    esos dos.",
    },
    // Retirada el 13/11/2026: Programator nunca usa otra máquina, solo la GPU del
    // equipo donde corre, y ejecutar ahí no necesita autorización.
    ClaveRetirada {
        seccion: "revision",
        clave: "maquina_autorizada",
        porque: "Programator ya no usa otra máquina: las tandas de la revisión se comprueban en \
                 este equipo, sin autorización, salvo que el banco.toml de la carpeta del banco \
                 apague las pruebas ocultas con «activa = false» en [comprobacion_local].",
        que_hacer: "quita esa línea; las tandas se comprueban solas en este equipo.",
    },
    ClaveRetirada {
        seccion: "revision",
        clave: "reintentar_maquina_cada_segundos",
        porque: "Ya no hay una máquina de comprobación a la que esperar: se comprueba en este \
                 equipo.",
        que_hacer: "quita esa línea.",
    },
    ClaveRetirada {
        seccion: "revision",
        clave: "esperar_maquina_como_mucho_minutos",
        porque: "Ya no hay una máquina de comprobación a la que esperar: se comprueba en este \
                 equipo.",
        que_hacer: "quita esa línea.",
    },
];

/// Quita de `tabla` las claves retiradas que traiga y devuelve un aviso por cada una.
fn quitar_claves_retiradas(tabla: &mut toml::Table) -> Vec<String> {
    CLAVES_RETIRADAS
        .iter()
        .filter(|retirada| {
            tabla
                .get_mut(retirada.seccion)
                .and_then(toml::Value::as_table_mut)
                .and_then(|seccion| seccion.remove(retirada.clave))
                .is_some()
        })
        .map(aviso_de_clave_retirada)
        .collect()
}

/// Interpreta el texto de `programator.toml` sin las claves retiradas, con sus avisos.
///
/// Sin claves retiradas se interpreta el texto tal cual, que es lo que da los errores con su línea;
/// solo si hay alguna se interpreta la tabla ya sin ella.
pub(super) fn interpretar_sin_claves_retiradas(
    texto: &str,
) -> Result<(Config, Vec<String>), toml::de::Error> {
    let mut tabla: toml::Table = texto.parse()?;
    let avisos = quitar_claves_retiradas(&mut tabla);
    let config = if avisos.is_empty() {
        toml::from_str(texto)?
    } else {
        toml::Value::Table(tabla).try_into()?
    };
    Ok((config, avisos))
}
