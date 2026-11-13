//! Reconocer, en el texto de un encargo del canal, un pedido de revisión.
//!
//! Lo reconoce el arnés, no el modelo (especificación de la revisión autónoma, §4): un encargo cuya
//! primera línea es `revisión: buscar`, `revisión: probar` o `revisión: cerrar` lo atiende el
//! código, y cualquier otro va al modelo como siempre. Todo aquí es puro.

use super::config::ConfigRevision;
use super::mensajes;

/// La palabra con que empieza un pedido de revisión, con y sin tilde: quien escribe el encargo es
/// otro modelo, y no siempre la pone.
const PALABRAS_DE_REVISION: [&str; 2] = ["revisión:", "revision:"];

/// Un candidato pedido en `revisión: probar`, con los nombres exactos de `candidatos.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatoPedido {
    pub repositorio: String,
    pub fichero: String,
}

/// Lo que se pide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pedido {
    Buscar,
    Probar(Vec<CandidatoPedido>),
    Cerrar,
}

/// Lo que es el texto de un encargo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconocido {
    /// Un encargo normal: va al modelo.
    NoEsDeRevision,
    Pedido(Pedido),
    /// Empieza como un pedido de revisión pero no se entiende. Lleva el motivo, que se publica.
    MalEscrito(String),
}

/// Lee el texto de un encargo.
pub fn reconocer(texto: &str) -> Reconocido {
    let mut lineas = texto.lines().map(str::trim).filter(|l| !l.is_empty());
    let Some(primera) = lineas.next() else {
        return Reconocido::NoEsDeRevision;
    };
    let minusculas = primera.to_lowercase();
    let Some(resto) = PALABRAS_DE_REVISION
        .iter()
        .find_map(|palabra| minusculas.strip_prefix(palabra))
    else {
        return Reconocido::NoEsDeRevision;
    };
    match resto.trim() {
        "buscar" => Reconocido::Pedido(Pedido::Buscar),
        "cerrar" => Reconocido::Pedido(Pedido::Cerrar),
        "probar" => leer_candidatos(lineas),
        otro => Reconocido::MalEscrito(mensajes::pedido_desconocido(otro)),
    }
}

/// Las líneas de `revisión: probar`, una por candidato: `<repositorio> <fichero>`. Se admite que
/// vengan como lista de Markdown o entre comillas invertidas.
fn leer_candidatos<'a>(lineas: impl Iterator<Item = &'a str>) -> Reconocido {
    let mut candidatos = Vec::new();
    for linea in lineas {
        let limpia = linea
            .trim_start_matches(['-', '*'])
            .replace('`', "")
            .trim()
            .to_string();
        let partes: Vec<&str> = limpia.split_whitespace().collect();
        let [repositorio, fichero] = partes.as_slice() else {
            return Reconocido::MalEscrito(mensajes::linea_de_candidato_mal_escrita(linea));
        };
        candidatos.push(CandidatoPedido {
            repositorio: repositorio.to_string(),
            fichero: fichero.to_string(),
        });
    }
    if candidatos.is_empty() {
        return Reconocido::MalEscrito(mensajes::PROBAR_SIN_CANDIDATOS.to_string());
    }
    Reconocido::Pedido(Pedido::Probar(candidatos))
}

/// Si el buzón `de` (un fichero del canal, como `claude.md`) puede pedir una revisión, o el motivo
/// por el que no, que se publica.
pub fn puede_pedirla(de: &str, config: &ConfigRevision) -> Result<(), String> {
    if config.buzones_que_ordenan.is_empty() {
        return Err(mensajes::REVISION_DESHABILITADA.to_string());
    }
    let buzon = de.trim_end_matches(".md");
    if config
        .buzones_que_ordenan
        .iter()
        .any(|permitido| permitido.eq_ignore_ascii_case(buzon))
    {
        Ok(())
    } else {
        Err(mensajes::buzon_sin_permiso(
            buzon,
            &config.buzones_que_ordenan,
        ))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn candidato(repositorio: &str, fichero: &str) -> CandidatoPedido {
        CandidatoPedido {
            repositorio: repositorio.into(),
            fichero: fichero.into(),
        }
    }

    #[test]
    fn los_tres_pedidos_se_reconocen_con_y_sin_tilde() {
        assert_eq!(
            reconocer("revisión: buscar"),
            Reconocido::Pedido(Pedido::Buscar)
        );
        assert_eq!(
            reconocer("Revision:   cerrar\n"),
            Reconocido::Pedido(Pedido::Cerrar)
        );
    }

    #[test]
    fn probar_lee_un_candidato_por_linea_con_sus_mayusculas() {
        let texto = "revisión: probar\n\
                     - `ManniX-ITA/JackOD-9B-Coder-MTP-GGUF JackOD-9B-Coder-CD-Q4_K_M.gguf`\n\
                     \n\
                     nyan-ame/Qwen2.5-Coder-7B-GGUF Qwen2.5-Coder-7B-Q4_K_M.gguf\n";
        assert_eq!(
            reconocer(texto),
            Reconocido::Pedido(Pedido::Probar(vec![
                candidato(
                    "ManniX-ITA/JackOD-9B-Coder-MTP-GGUF",
                    "JackOD-9B-Coder-CD-Q4_K_M.gguf"
                ),
                candidato(
                    "nyan-ame/Qwen2.5-Coder-7B-GGUF",
                    "Qwen2.5-Coder-7B-Q4_K_M.gguf"
                ),
            ]))
        );
    }

    #[test]
    fn un_encargo_normal_no_es_de_revision() {
        assert_eq!(
            reconocer("Escribe una función que sume.\nrevisión: buscar"),
            Reconocido::NoEsDeRevision
        );
        assert_eq!(reconocer(""), Reconocido::NoEsDeRevision);
    }

    #[test]
    fn lo_que_parece_un_pedido_pero_no_se_entiende_dice_por_que() {
        assert!(matches!(
            reconocer("revisión: descargar todo"),
            Reconocido::MalEscrito(ref m) if m.contains("«descargar todo»")
        ));
        assert!(matches!(
            reconocer("revisión: probar"),
            Reconocido::MalEscrito(ref m) if m.contains("ningún candidato")
        ));
        assert!(matches!(
            reconocer("revisión: probar\nsolo-el-repositorio"),
            Reconocido::MalEscrito(ref m) if m.contains("«solo-el-repositorio»")
        ));
    }

    #[test]
    fn solo_pide_revision_quien_esta_en_la_lista() {
        let mut config = ConfigRevision::default();
        assert!(puede_pedirla("claude.md", &config)
            .unwrap_err()
            .contains("buzones_que_ordenan"));

        config.buzones_que_ordenan = vec!["Claude".into()];
        assert_eq!(puede_pedirla("claude.md", &config), Ok(()));
        let negativa = puede_pedirla("gemini.md", &config).unwrap_err();
        assert!(negativa.contains("«gemini»"), "{negativa}");
        assert!(negativa.contains("Claude"), "{negativa}");
    }
}
