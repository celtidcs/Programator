//! Reconocer, en el texto de un encargo del canal, un pedido de revisión.
//!
//! Lo reconoce el arnés, no el modelo: un encargo cuya primera línea es `revisión: buscar`,
//! `revisión: probar` o `revisión: cerrar` lo atiende el código, y cualquier otro va al modelo como
//! siempre. Todo aquí es puro.

use super::config::ConfigRevision;
use super::mensajes;
use crate::protocolo::destinatarios::Identidad;

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

impl Pedido {
    /// Si atenderlo toca el motor del ciclo. Solo `probar`: suelta el motor para pasar el banco
    /// (`revision::probar`, `soltar_motor` y `pasar_tanda`). `buscar` consulta Hugging Face y
    /// descarga; `cerrar` comprueba y puntúa las tandas ya pasadas, aplica la regla y, si toca,
    /// cambia el modelo de la configuración, que el ciclo relee después. Ninguno de los dos carga
    /// ni suelta el motor, así que pueden atenderse mientras un modelo carga.
    pub fn necesita_el_motor(&self) -> bool {
        matches!(self, Self::Probar(_))
    }
}

/// Si atender el texto de un encargo toca el motor del ciclo: solo un pedido de revisión que lo
/// necesite. Lo mal escrito se contesta con su motivo y no lo toca.
pub fn necesita_el_motor(texto: &str) -> bool {
    match reconocer(texto) {
        Reconocido::Pedido(pedido) => pedido.necesita_el_motor(),
        Reconocido::NoEsDeRevision | Reconocido::MalEscrito(_) => false,
    }
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
        return Reconocido::MalEscrito(mensajes::probar_sin_candidatos());
    }
    Reconocido::Pedido(Pedido::Probar(candidatos))
}

/// Si `quien` (la identidad de la cabecera de su buzón) puede pedir una revisión, o el motivo por
/// el que no, que se publica.
pub fn puede_pedirla(quien: Option<&Identidad>, config: &ConfigRevision) -> Result<(), String> {
    if config.identidades_que_ordenan.is_empty() {
        return Err(mensajes::revision_deshabilitada());
    }
    let Some(quien) = quien else {
        return Err(mensajes::revision_sin_identidad(
            &config.identidades_que_ordenan,
        ));
    };
    if config
        .identidades_que_ordenan
        .iter()
        .filter_map(|permitida| Identidad::leer(permitida))
        .any(|permitida| permitida == *quien)
    {
        Ok(())
    } else {
        Err(mensajes::identidad_sin_permiso(
            &quien.to_string(),
            &config.identidades_que_ordenan,
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
    fn solo_probar_necesita_el_motor() {
        assert!(!Pedido::Buscar.necesita_el_motor());
        assert!(!Pedido::Cerrar.necesita_el_motor());
        assert!(Pedido::Probar(vec![candidato("a/b", "c.gguf")]).necesita_el_motor());
    }

    #[test]
    fn el_texto_dice_si_necesita_el_motor() {
        assert!(!necesita_el_motor("revisión: buscar"));
        assert!(!necesita_el_motor("revision: cerrar"));
        assert!(necesita_el_motor(
            "revisión: probar
a/b c.gguf"
        ));
        assert!(!necesita_el_motor("revisión: lo que sea"));
        assert!(!necesita_el_motor("haz esto"));
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
        let claude1 = Identidad::leer("Claude 1").unwrap();
        let gemini1 = Identidad::leer("Gemini 1").unwrap();
        let mut config = ConfigRevision::default();
        assert!(puede_pedirla(Some(&claude1), &config)
            .unwrap_err()
            .contains("identidades_que_ordenan"));

        config.identidades_que_ordenan = vec!["Claude 1".into()];
        assert_eq!(puede_pedirla(Some(&claude1), &config), Ok(()));
        let negativa = puede_pedirla(Some(&gemini1), &config).unwrap_err();
        assert!(negativa.contains("«Gemini 1»"), "{negativa}");
        assert!(negativa.contains("Claude 1"), "{negativa}");
        let sin_cabecera = puede_pedirla(None, &config).unwrap_err();
        assert!(sin_cabecera.contains("# Bandeja de"), "{sin_cabecera}");
    }
}
