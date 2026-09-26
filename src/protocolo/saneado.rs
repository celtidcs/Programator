//! Funciones de sanitización para evitar que texto de entrada sea confundido con marcas del arnés.
//!
//! Ningún texto que venga del modelo puede acabar pareciendo generado por Programator. Estas
//! funciones neutralizan dos clases de inyecciones:
//! 1. Líneas que pretenden ser marcas del arnés (`**LATIDO:**`, `**LEÍDO:**`)
//! 2. Saltos de línea que podrían crear líneas adicionales de aspecto legítimo

/// Neutraliza saltos de línea y caracteres de retorno, sustituyéndolos por espacios.
///
/// Se aplica a cualquier texto que provenga del modelo y pueda acabar interpolado en una línea
/// de un fichero compartido (como `estado.md`). Un salto de línea en el texto del usuario
/// es peligroso: permitiría inyectar líneas falsas de aspecto legítimo.
pub fn sanear_saltos(texto: &str) -> String {
    texto.replace(['\n', '\r'], " ")
}

/// Neutraliza líneas que parecen marcas generadas por el arnés, citándolas con `> `.
///
/// Se aplica a cualquier texto que provenga del modelo y pueda contener líneas simulando
/// ser marcas del protocolo. Las líneas de un agente que parecen generadas por Programator
/// son citadas para hacerlas inofensivas sin perder la información.
///
/// No procesa líneas que, tras trim, quedan vacías.
pub fn neutralizar_marcas(texto: &str) -> String {
    texto
        .lines()
        .map(|linea| {
            let recortada = linea.trim();
            if recortada.starts_with("**LATIDO:**") || recortada.starts_with("**LEÍDO:**") {
                format!("> {linea}")
            } else {
                linea.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Marca como cita el texto que escriben otros agentes, prefijando cada línea con `> `.
///
/// Se aplica al texto de los buzones ajenos antes de enseñárselo al modelo, de modo que ninguna
/// de sus líneas pueda confundirse con la voz del arnés. Las líneas vacías se quedan como `>`
/// sin espacio sobrante.
pub fn citar(texto: &str) -> String {
    texto
        .lines()
        .map(|linea| {
            if linea.trim().is_empty() {
                ">".to_string()
            } else {
                format!("> {linea}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cita_texto_de_una_linea() {
        let resultado = citar("contenido");
        assert_eq!(resultado, "> contenido");
    }

    #[test]
    fn cita_texto_multilinea_con_linea_vacia_en_medio() {
        let resultado = citar("linea 1\n\nlinea 3");
        assert_eq!(resultado, "> linea 1\n>\n> linea 3");
    }

    #[test]
    fn texto_vacio_devuelve_cadena_vacia() {
        let resultado = citar("");
        assert_eq!(resultado, "");
    }

    #[test]
    fn normaliza_retorno_de_carro() {
        let resultado = citar("linea 1\r\nlinea 2");
        assert_eq!(resultado, "> linea 1\n> linea 2");
    }
}
