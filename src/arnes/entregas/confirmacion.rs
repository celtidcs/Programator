//! `aceptada: <propuesta>` y `rechazada: <propuesta> — <motivo>`: el agente que encargó dice en
//! qué quedó una propuesta.
//!
//! Lo reconoce el arnés, como la revisión, y solo con los dos puntos: «aceptada la idea…» sin
//! ellos es un encargo normal. Lo que dice el agente manda sobre lo deducido y ya no se vuelve a
//! deducir. Solo cuenta quien encargó la propuesta: el mismo buzón, que solo escribe su dueño.

use super::{EstadoDeLaPropuesta, RegistroDeEntregas};

/// Las dos palabras, con los dos puntos. Se comparan sin distinguir mayúsculas.
const PALABRA_DE_ACEPTADA: &str = "aceptada:";
const PALABRA_DE_RECHAZADA: &str = "rechazada:";

/// Lo que puede separar el nombre de la propuesta de su motivo: rayas y dos puntos.
const SEPARADORES_DEL_MOTIVO: [char; 4] = ['—', '–', '-', ':'];

/// Lo que dice una confirmación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmacion {
    pub estado: EstadoDeLaPropuesta,
    /// El nombre de la propuesta en la carpeta de candidatos; vacío si no lo trae.
    pub propuesta: String,
    pub motivo: Option<String>,
}

/// La confirmación que es el cuerpo de un encargo, si lo es.
pub fn reconocer(cuerpo: &str) -> Option<Confirmacion> {
    let primera = crate::protocolo::encargo::primera_linea_con_texto(cuerpo)?;
    let (estado, resto) = [
        (EstadoDeLaPropuesta::Aceptada, PALABRA_DE_ACEPTADA),
        (EstadoDeLaPropuesta::Rechazada, PALABRA_DE_RECHAZADA),
    ]
    .into_iter()
    .find_map(|(estado, palabra)| tras_la_palabra(primera, palabra).map(|resto| (estado, resto)))?;
    let resto = resto.trim();
    let (propuesta, motivo) = match resto.split_once(char::is_whitespace) {
        Some((propuesta, motivo)) => (propuesta, motivo),
        None => (resto, ""),
    };
    let motivo = motivo
        .trim()
        .trim_start_matches(SEPARADORES_DEL_MOTIVO)
        .trim();
    Some(Confirmacion {
        estado,
        propuesta: propuesta
            .trim_matches(['`', '*', '«', '»', ':'])
            .to_string(),
        motivo: (!motivo.is_empty()).then(|| motivo.to_string()),
    })
}

/// Si el cuerpo de un encargo es una confirmación.
pub fn es_confirmacion(cuerpo: &str) -> bool {
    reconocer(cuerpo).is_some()
}

fn tras_la_palabra<'a>(linea: &'a str, palabra: &str) -> Option<&'a str> {
    let cabeza = linea.get(..palabra.len())?;
    cabeza
        .eq_ignore_ascii_case(palabra)
        .then(|| &linea[palabra.len()..])
}

/// Aplica la confirmación de `de` (con la identidad `quien`) al registro y devuelve la respuesta.
/// Devuelve también si cambió el registro.
pub fn confirmar(
    confirmacion: &Confirmacion,
    de: &str,
    quien: &str,
    registro: &mut RegistroDeEntregas,
) -> (String, bool) {
    if confirmacion.propuesta.is_empty() {
        return (mensaje_sin_propuesta(), false);
    }
    let nombradas: Vec<usize> = registro
        .entregas
        .iter()
        .enumerate()
        .filter(|(_, entrada)| entrada.propuesta == confirmacion.propuesta)
        .map(|(posicion, _)| posicion)
        .collect();
    let propia = nombradas
        .iter()
        .rev()
        .copied()
        .find(|&posicion| registro.entregas[posicion].de == de);
    let Some(posicion) = propia else {
        let texto = if nombradas.is_empty() {
            mensaje_no_encontrada(&confirmacion.propuesta)
        } else {
            mensaje_de_otro(&confirmacion.propuesta)
        };
        return (texto, false);
    };
    let entrada = &mut registro.entregas[posicion];
    entrada.estado = confirmacion.estado;
    entrada.estado_lo_dio = quien.to_string();
    entrada.motivo = confirmacion.motivo.clone();
    (mensaje_anotada(confirmacion), true)
}

/// La línea que lleva al final la entrega de cada propuesta: cómo decir en qué quedó.
///
/// No escribe «Para Programator» delante de las formas: el canal pone en minúscula todo «Para» de
/// lo que publica Programator, para no despertar a nadie sin querer, y la línea saldría como
/// `para Programator: aceptada: …`, que no despierta a Programator si se copia tal cual.
pub fn linea_de_confirmacion(propuesta: &str) -> String {
    format!(
        "Cuando la apliques o la descartes, dímelo con un encargo nuevo para Programator que diga \
         `aceptada: {propuesta}` o `rechazada: {propuesta} — <motivo>`."
    )
}

fn mensaje_anotada(confirmacion: &Confirmacion) -> String {
    let estado = match confirmacion.estado {
        EstadoDeLaPropuesta::Rechazada => "rechazada",
        _ => "aceptada",
    };
    match &confirmacion.motivo {
        Some(motivo) => format!(
            "Anotado: «{}» queda {estado}, con el motivo «{motivo}».",
            confirmacion.propuesta
        ),
        None => format!("Anotado: «{}» queda {estado}.", confirmacion.propuesta),
    }
}

fn mensaje_sin_propuesta() -> String {
    "No se ha anotado nada: tras `aceptada:` o `rechazada:` falta el nombre de la propuesta, tal \
     como lo dio la entrega."
        .to_string()
}

fn mensaje_no_encontrada(propuesta: &str) -> String {
    format!(
        "No se ha anotado nada: no hay ninguna propuesta «{propuesta}» en el registro de entregas. \
         El nombre es el que dio la entrega, con su `-002` si lo llevaba."
    )
}

fn mensaje_de_otro(propuesta: &str) -> String {
    format!(
        "No se ha anotado nada: la propuesta «{propuesta}» la encargó otro buzón, y solo puede \
         decir en qué quedó quien la encargó."
    )
}

#[cfg(test)]
mod pruebas {
    use super::super::{EntradaDeEntrega, VeredictoDeLaEntrega, LO_DIO_PROGRAMATOR};
    use super::*;

    fn entrada(propuesta: &str, de: &str) -> EntradaDeEntrega {
        EntradaDeEntrega {
            propuesta: propuesta.to_string(),
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: de.to_string(),
            modelo: None,
            fecha: "f".to_string(),
            veredicto: VeredictoDeLaEntrega::Pasa,
            huella: "h".to_string(),
            estado: EstadoDeLaPropuesta::Propuesta,
            estado_lo_dio: LO_DIO_PROGRAMATOR.to_string(),
            destino: None,
            motivo: None,
            archivada: None,
            ya_en_el_proyecto: Vec::new(),
        }
    }

    #[test]
    fn se_reconocen_las_dos_formas_con_su_motivo() {
        assert_eq!(
            reconocer("aceptada: x.py"),
            Some(Confirmacion {
                estado: EstadoDeLaPropuesta::Aceptada,
                propuesta: "x.py".to_string(),
                motivo: None,
            })
        );
        for texto in [
            "rechazada: x.py — no compila",
            "Rechazada: `x.py` - no compila",
            "RECHAZADA: x.py: no compila",
        ] {
            assert_eq!(
                reconocer(texto),
                Some(Confirmacion {
                    estado: EstadoDeLaPropuesta::Rechazada,
                    propuesta: "x.py".to_string(),
                    motivo: Some("no compila".to_string()),
                }),
                "{texto}"
            );
        }
    }

    #[test]
    fn sin_los_dos_puntos_es_un_encargo_normal() {
        assert_eq!(reconocer("aceptada la idea, hazla"), None);
        assert_eq!(reconocer("rechazada x.py: no compila"), None);
        assert_eq!(reconocer("Revisa: aceptada: x"), None);
    }

    /// E4: el agente rechaza con motivo y lo suyo manda; de otro buzón no vale.
    #[test]
    fn el_rechazo_del_que_encargo_se_anota_con_su_motivo() {
        let mut registro = RegistroDeEntregas {
            entregas: vec![entrada("x.py", "claude.md")],
            ..RegistroDeEntregas::default()
        };
        let rechazo = reconocer("rechazada: x.py — no compila").unwrap();

        let (ajeno, cambio_ajeno) = confirmar(&rechazo, "codex.md", "Codex", &mut registro);
        assert!(!cambio_ajeno);
        assert_eq!(ajeno, mensaje_de_otro("x.py"));
        assert_eq!(registro.entregas[0].estado, EstadoDeLaPropuesta::Propuesta);

        let (respuesta, cambio) = confirmar(&rechazo, "claude.md", "Claude 1", &mut registro);
        assert!(cambio);
        assert!(respuesta.contains("rechazada"), "{respuesta}");
        let anotada = &registro.entregas[0];
        assert_eq!(anotada.estado, EstadoDeLaPropuesta::Rechazada);
        assert_eq!(anotada.estado_lo_dio, "Claude 1");
        assert_eq!(anotada.motivo.as_deref(), Some("no compila"));

        // Lo dicho por el agente ya no se deduce.
        let encontrados = [super::super::deduccion::Encontrado {
            ruta: "x.py".to_string(),
            huella: "h".to_string(),
        }];
        assert!(!super::super::deduccion::deducir(
            &mut registro,
            Some(&encontrados)
        ));
        assert_eq!(registro.entregas[0].estado, EstadoDeLaPropuesta::Rechazada);
    }

    #[test]
    fn lo_que_no_esta_en_el_registro_se_contesta_sin_anotar() {
        let mut registro = RegistroDeEntregas::default();
        let (respuesta, cambio) = confirmar(
            &reconocer("aceptada: y.py").unwrap(),
            "claude.md",
            "Claude 1",
            &mut registro,
        );
        assert!(!cambio);
        assert_eq!(respuesta, mensaje_no_encontrada("y.py"));
        let (vacia, _) = confirmar(
            &reconocer("aceptada:").unwrap(),
            "claude.md",
            "Claude 1",
            &mut registro,
        );
        assert_eq!(vacia, mensaje_sin_propuesta());
    }

    /// E7: la línea dice las dos formas exactas con el nombre de la propuesta.
    #[test]
    fn la_linea_de_confirmacion_trae_las_dos_formas() {
        let linea = linea_de_confirmacion("sub/x-002.py");
        assert!(linea.contains("`aceptada: sub/x-002.py`"));
        assert!(linea.contains("`rechazada: sub/x-002.py — <motivo>`"));
        assert_eq!(crate::protocolo::respuesta::neutralizar_para(&linea), linea);
        assert_eq!(
            reconocer("aceptada: sub/x-002.py").unwrap().propuesta,
            "sub/x-002.py"
        );
    }
}
