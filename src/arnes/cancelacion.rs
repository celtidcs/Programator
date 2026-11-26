//! `cancelar: <referencia>`: un encargo con el que quien encargó algo lo retira antes de que se
//! atienda.
//!
//! Lo reconoce el arnés, no el modelo, como la revisión: la primera línea del cuerpo empieza por
//! `cancelar:`, con los dos puntos. «cancelar la prueba…» sin ellos es un encargo normal y va al
//! modelo. La referencia es la etiqueta del encargo (`PG-12`), su nombre corto (lo que sigue a
//! `Para Programator:`) o su identificador. Solo cancela quien encargó: el mismo buzón, que solo
//! escribe su dueño.

use super::registro_de_encargos::{EstadoDelEncargo, FichaDeEncargo, RegistroDeEncargos};
use crate::protocolo::respuesta::{etiqueta_para, nombre_corto};
use crate::protocolo::{detectar, Encargo};
use std::collections::BTreeSet;

/// La palabra con que empieza una cancelación. Se compara sin distinguir mayúsculas.
const PALABRA_DE_CANCELAR: &str = "cancelar:";

/// La referencia de una cancelación, si el cuerpo del encargo lo es: lo que sigue a `cancelar:` en
/// su primera línea con texto, que puede estar vacío.
pub fn referencia(cuerpo: &str) -> Option<&str> {
    let primera = crate::protocolo::encargo::primera_linea_con_texto(cuerpo)?;
    let largo = PALABRA_DE_CANCELAR.len();
    let palabra = primera.get(..largo)?;
    if !palabra.eq_ignore_ascii_case(PALABRA_DE_CANCELAR) {
        return None;
    }
    Some(primera[largo..].trim().trim_matches(['*', '`']).trim())
}

/// Si el cuerpo de un encargo es una cancelación.
pub fn es_cancelacion(cuerpo: &str) -> bool {
    referencia(cuerpo).is_some()
}

/// Lo que se hizo con una cancelación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolucion {
    /// Se cancelaron estos encargos, por su nombre corto: estaban sin atender.
    Cancelados(Vec<String>),
    /// Los encargos de esa referencia ya tenían respuesta: no había nada que cancelar.
    YaTerminado(EstadoDelEncargo),
    /// Ningún encargo responde a esa referencia.
    NoEncontrado,
    /// La referencia es de encargos de otro buzón: solo los cancela quien los encargó.
    DeOtro,
}

/// Resuelve las cancelaciones pendientes contra el registro: cancela lo que nombran y devuelve, por
/// cada una, el encargo de la cancelación y el texto de su respuesta. Las cancelaciones mismas
/// siguen abiertas: quien publica la respuesta las da por atendidas.
pub fn cancelar(
    cancelaciones: &[Encargo],
    fichas: &mut RegistroDeEncargos,
    agente: &str,
    fecha: &str,
) -> Vec<(Encargo, String)> {
    cancelaciones
        .iter()
        .map(|cancelacion| {
            let referencia = referencia(cancelacion.cuerpo(agente)).unwrap_or_default();
            let resolucion = resolver(referencia, cancelacion, fichas.fichas(), agente);
            if let Resolucion::Cancelados(_) = resolucion {
                for encargo in abiertos_de(referencia, cancelacion, fichas.fichas(), agente) {
                    fichas.terminar(&encargo, EstadoDelEncargo::Cancelado, fecha);
                }
            }
            (cancelacion.clone(), respuesta(referencia, &resolucion))
        })
        .collect()
}

/// Qué pasa con la cancelación `referencia`, pedida por `cancelacion`. Puro.
pub fn resolver(
    referencia: &str,
    cancelacion: &Encargo,
    fichas: &[FichaDeEncargo],
    agente: &str,
) -> Resolucion {
    let nombradas: Vec<&FichaDeEncargo> = nombradas(referencia, fichas, agente);
    let propias: Vec<&&FichaDeEncargo> = nombradas
        .iter()
        .filter(|ficha| ficha.de == cancelacion.de)
        .collect();
    if propias.is_empty() {
        return if nombradas.is_empty() {
            Resolucion::NoEncontrado
        } else {
            Resolucion::DeOtro
        };
    }
    let abiertas: Vec<String> = propias
        .iter()
        .filter(|ficha| ficha.estado.esta_abierto())
        .map(|ficha| nombre_corto(&ficha.texto, agente))
        .collect();
    match propias.last() {
        Some(ultima) if abiertas.is_empty() => Resolucion::YaTerminado(ultima.estado),
        _ => Resolucion::Cancelados(abiertas),
    }
}

/// Los encargos abiertos del mismo buzón que nombra la referencia: los que se cancelan.
fn abiertos_de(
    referencia: &str,
    cancelacion: &Encargo,
    fichas: &[FichaDeEncargo],
    agente: &str,
) -> Vec<Encargo> {
    nombradas(referencia, fichas, agente)
        .into_iter()
        .filter(|ficha| ficha.de == cancelacion.de && ficha.estado.esta_abierto())
        .map(FichaDeEncargo::encargo)
        .collect()
}

/// Las fichas que nombra la referencia, de cualquier buzón, sin las cancelaciones (la misma
/// incluida): una cancelación no se cancela.
fn nombradas<'a>(
    referencia: &str,
    fichas: &'a [FichaDeEncargo],
    agente: &str,
) -> Vec<&'a FichaDeEncargo> {
    if referencia.is_empty() {
        return Vec::new();
    }
    fichas
        .iter()
        .filter(|ficha| !es_cancelacion(ficha.encargo().cuerpo(agente)))
        .filter(|ficha| la_nombra(referencia, ficha, agente))
        .collect()
}

/// Si la referencia es el identificador, la etiqueta o el nombre corto de la ficha.
fn la_nombra(referencia: &str, ficha: &FichaDeEncargo, agente: &str) -> bool {
    nombra(referencia, &ficha.id, &ficha.texto, agente)
}

/// Si la referencia es `id`, o la etiqueta o el nombre corto del encargo de `texto`.
fn nombra(referencia: &str, id: &str, texto: &str, agente: &str) -> bool {
    let primera = texto.lines().next().unwrap_or_default();
    id.eq_ignore_ascii_case(referencia)
        || etiqueta_para(primera, agente).is_some_and(|e| e.eq_ignore_ascii_case(referencia))
        || nombre_corto(texto, agente).to_lowercase() == referencia.to_lowercase()
}

/// Las cancelaciones de `encargo` que hay en `contenido`, su buzón, y que el registro no conoce
/// (`conocidos`): las que llegaron mientras se atendía. Solo se mira su buzón porque solo cancela
/// quien encargó.
pub fn cancelaciones_nuevas_de(
    encargo: &Encargo,
    contenido: &str,
    agente: &str,
    conocidos: &BTreeSet<String>,
) -> Vec<Encargo> {
    let id = encargo.id();
    detectar(agente, &encargo.de, contenido)
        .into_iter()
        .filter(|cancelacion| !conocidos.contains(&cancelacion.id()))
        .filter(|cancelacion| {
            referencia(cancelacion.cuerpo(agente))
                .is_some_and(|r| !r.is_empty() && nombra(r, &id, &encargo.texto, agente))
        })
        .collect()
}

/// El texto de la respuesta a una cancelación.
pub fn respuesta(referencia: &str, resolucion: &Resolucion) -> String {
    match resolucion {
        Resolucion::Cancelados(nombres) => mensaje_cancelados(nombres),
        Resolucion::YaTerminado(estado) => mensaje_ya_terminado(referencia, *estado),
        Resolucion::NoEncontrado => mensaje_no_encontrado(referencia),
        Resolucion::DeOtro => mensaje_de_otro(referencia),
    }
}

fn mensaje_cancelados(nombres: &[String]) -> String {
    let lista: Vec<String> = nombres.iter().map(|n| format!("«{n}»")).collect();
    format!(
        "Cancelado, sin atender: {}. No se ha gastado el modelo en él.",
        lista.join(", ")
    )
}

fn mensaje_ya_terminado(referencia: &str, estado: EstadoDelEncargo) -> String {
    format!(
        "No había nada que cancelar: el encargo «{referencia}» ya no estaba pendiente (está {}).",
        nombre_del_estado(estado)
    )
}

fn mensaje_no_encontrado(referencia: &str) -> String {
    if referencia.is_empty() {
        return "No se ha cancelado nada: tras `cancelar:` falta la etiqueta, el nombre o el \
                identificador del encargo."
            .to_string();
    }
    format!(
        "No se ha cancelado nada: ningún encargo responde a «{referencia}». Vale su etiqueta \
         (`PG-12`), lo que siguió a `Para Programator:` o su identificador."
    )
}

fn mensaje_de_otro(referencia: &str) -> String {
    format!(
        "No se ha cancelado nada: el encargo «{referencia}» es de otro buzón, y solo lo puede \
         cancelar quien lo encargó."
    )
}

/// Cómo se nombra un estado en una respuesta.
fn nombre_del_estado(estado: EstadoDelEncargo) -> &'static str {
    match estado {
        EstadoDelEncargo::Pendiente => "pendiente",
        EstadoDelEncargo::EnCurso => "en curso",
        EstadoDelEncargo::Atendido => "atendido",
        EstadoDelEncargo::Cancelado => "cancelado",
        EstadoDelEncargo::Cerrado => "cerrado sin respuesta del modelo",
        EstadoDelEncargo::Visto => "dado por visto al arrancar",
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::protocolo::encargo::Forma;

    const AGENTE: &str = "Programator";

    fn encargo(de: &str, latido: &str, texto: &str) -> Encargo {
        Encargo {
            de: de.to_string(),
            texto: texto.to_string(),
            forma: Forma::Linea,
            latido: Some(latido.to_string()),
        }
    }

    fn registro(encargos: &[(&Encargo, EstadoDelEncargo)]) -> RegistroDeEncargos {
        let mut registro = RegistroDeEncargos::default();
        for (encargo, estado) in encargos {
            registro.anotar(encargo, "2026-11-25 10:00:00", *estado);
        }
        registro
    }

    fn estado_de(registro: &RegistroDeEncargos, encargo: &Encargo) -> EstadoDelEncargo {
        registro
            .fichas()
            .iter()
            .find(|ficha| ficha.id == encargo.id())
            .map(|ficha| ficha.estado)
            .unwrap()
    }

    #[test]
    fn se_reconoce_con_los_dos_puntos_y_sin_distinguir_mayusculas() {
        assert_eq!(referencia("cancelar: PG-12"), Some("PG-12"));
        assert_eq!(referencia("Cancelar: `PG-12`\nmotivo"), Some("PG-12"));
        assert_eq!(referencia("cancelar:"), Some(""));
    }

    #[test]
    fn sin_los_dos_puntos_es_un_encargo_normal() {
        assert_eq!(referencia("cancelar la prueba de carga"), None);
        assert_eq!(referencia("Revisa cancelar: x"), None);
        assert_eq!(referencia(""), None);
    }

    /// D3, en el registro: lo pendiente pasa a cancelado.
    #[test]
    fn cancela_por_etiqueta_nombre_o_identificador() {
        let objetivo = encargo("claude.md", "1", "Para Programator: PG-12 contar líneas");
        for referencia_escrita in ["PG-12", "pg-12", "contar líneas", &objetivo.id()] {
            let cancelacion = encargo(
                "claude.md",
                "2",
                &format!("Para Programator: cancelar: {referencia_escrita}"),
            );
            let mut fichas = registro(&[
                (&objetivo, EstadoDelEncargo::Pendiente),
                (&cancelacion, EstadoDelEncargo::Pendiente),
            ]);

            let respuestas = cancelar(
                std::slice::from_ref(&cancelacion),
                &mut fichas,
                AGENTE,
                "2026-11-25 10:01:00",
            );

            assert_eq!(
                estado_de(&fichas, &objetivo),
                EstadoDelEncargo::Cancelado,
                "{referencia_escrita}"
            );
            assert_eq!(
                estado_de(&fichas, &cancelacion),
                EstadoDelEncargo::Pendiente,
                "la cancelación la cierra quien publica"
            );
            assert!(respuestas[0].1.starts_with("Cancelado"), "{respuestas:?}");
            assert!(
                respuestas[0].1.contains("«contar líneas»"),
                "{respuestas:?}"
            );
        }
    }

    /// D5.
    #[test]
    fn el_encargo_de_otro_buzon_no_se_cancela() {
        let ajeno = encargo("codex.md", "1", "Para Programator: PG-12 contar líneas");
        let cancelacion = encargo("claude.md", "2", "Para Programator: cancelar: PG-12");
        let mut fichas = registro(&[
            (&ajeno, EstadoDelEncargo::Pendiente),
            (&cancelacion, EstadoDelEncargo::Pendiente),
        ]);

        let respuestas = cancelar(std::slice::from_ref(&cancelacion), &mut fichas, AGENTE, "f");

        assert_eq!(estado_de(&fichas, &ajeno), EstadoDelEncargo::Pendiente);
        assert_eq!(respuestas[0].1, mensaje_de_otro("PG-12"));
    }

    /// D6.
    #[test]
    fn lo_que_no_existe_o_ya_se_atendio_se_contesta_sin_cancelar_nada() {
        let atendido = encargo("claude.md", "1", "Para Programator: PG-12 contar líneas");
        let fichas = registro(&[(&atendido, EstadoDelEncargo::Atendido)]);
        let pide = |referencia: &str| {
            encargo(
                "claude.md",
                "2",
                &format!("Para Programator: cancelar: {referencia}"),
            )
        };

        assert_eq!(
            resolver("PG-99", &pide("PG-99"), fichas.fichas(), AGENTE),
            Resolucion::NoEncontrado
        );
        assert_eq!(
            resolver("PG-12", &pide("PG-12"), fichas.fichas(), AGENTE),
            Resolucion::YaTerminado(EstadoDelEncargo::Atendido)
        );
        assert_eq!(
            resolver("", &pide(""), fichas.fichas(), AGENTE),
            Resolucion::NoEncontrado
        );
        assert!(mensaje_ya_terminado("PG-12", EstadoDelEncargo::Atendido).contains("atendido"));
    }

    #[test]
    fn las_cancelaciones_nuevas_son_las_del_buzon_que_no_conoce_el_registro() {
        let en_curso = encargo("claude.md", "1", "Para Programator: PG-12 contar líneas");
        let vieja = encargo("claude.md", "2", "Para Programator: cancelar: PG-12");
        let contenido = "LATIDO: 1\nPara Programator: PG-12 contar líneas\n\n\
                         LATIDO: 2\nPara Programator: cancelar: PG-12\n\n\
                         LATIDO: 3\nPara Programator: cancelar: PG-13\n\n\
                         LATIDO: 4\nPara Programator: cancelar: contar líneas\n";
        let conocidos: BTreeSet<String> = [en_curso.id(), vieja.id()].into_iter().collect();

        let nuevas = cancelaciones_nuevas_de(&en_curso, contenido, AGENTE, &conocidos);

        assert_eq!(nuevas.len(), 1, "{nuevas:?}");
        assert_eq!(nuevas[0].latido.as_deref(), Some("4"));
    }

    #[test]
    fn una_cancelacion_no_se_nombra_a_si_misma_ni_a_otra_cancelacion() {
        let primera = encargo("claude.md", "1", "Para Programator: cancelar: PG-12");
        let segunda = encargo("claude.md", "2", "Para Programator: cancelar: PG-12");
        let fichas = registro(&[
            (&primera, EstadoDelEncargo::Pendiente),
            (&segunda, EstadoDelEncargo::Pendiente),
        ]);

        assert_eq!(
            resolver("PG-12", &segunda, fichas.fichas(), AGENTE),
            Resolucion::NoEncontrado
        );
    }
}
