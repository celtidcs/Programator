//! Qué modelos hay en la carpeta de modelos, si caben en la tarjeta y qué clases atiende cada uno.
//!
//! Se mira al arrancar. Si ninguno cabe, se avisa y se
//! pide una revisión; nada se descarga sin conformidad.
//!
//! [`reunir`] mira el mundo (la carpeta y la cabecera de cada GGUF, sin cargar nada); el resto es
//! puro.

use crate::arnes::clase_pedida::ClasePedida;
use crate::banco::resultados::FILAS_DE_DESEMPENO;
use crate::config::Config;
use crate::motor::cabida::{cabida, Cabida, PROPORCION_MINIMA_DE_CAPAS_EN_TARJETA};
use crate::motor::encaje::encaje_en_tarjeta;
use crate::motor::hardware::Gpu;
use crate::registro::modelo::Registro;
use crate::revision::eleccion::elegir_modelo;
use crate::revision::proponer::ficheros_con_tamano;

/// El encargo que busca modelos que quepan (`src/revision/encargo.rs`).
pub(crate) const PEDIDO_DE_BUSQUEDA: &str = "revisión: buscar";
/// Lo que se le pone a la línea `Para` del aviso, tras los dos puntos, si hay modelos y ninguno
/// cabe.
const ASUNTO_NINGUNO_CABE: &str = "ningún modelo cabe en esta tarjeta";
/// Lo mismo si la carpeta de modelos está vacía: decir «ninguno cabe» sin modelos confunde.
const ASUNTO_SIN_MODELOS: &str = "no hay ningún modelo en la carpeta de modelos";

/// Qué pasó con el aviso en el canal. No es un booleano porque así un fallo al publicar no se
/// cuenta como si faltara la lista de identidades.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvisoEnElCanal {
    /// Se dejó en el canal para quien puede pedir la revisión.
    Publicado,
    /// No se pudo escribir; el motivo.
    NoPublicado(String),
    /// No hay a quién avisar: `[revision] identidades_que_ordenan` está vacía.
    SinIdentidades,
}

/// Un modelo de la carpeta de modelos, visto al arrancar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeloDisponible {
    pub fichero: String,
    pub cabida: Cabida,
    /// Los nombres cortos de las clases que atiende, según el banco.
    pub clases: Vec<String>,
}

/// Lo visto al arrancar: la tarjeta y los modelos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disponibles {
    pub tarjeta: String,
    pub carpeta: String,
    pub modelos: Vec<ModeloDisponible>,
}

impl Disponibles {
    /// Si no hay ningún modelo con el que se pueda trabajar en esta tarjeta.
    pub fn ninguno_cabe(&self) -> bool {
        !self.modelos.iter().any(|modelo| modelo.cabida.cabe())
    }
}

/// Mira la carpeta de modelos. `None` sin tarjeta medida: sin ella no se sabe qué cabe.
pub fn reunir(config: &Config, gpu: Option<&Gpu>, registro: &Registro) -> Option<Disponibles> {
    let gpu = gpu?;
    let carpeta = config.resolver(&config.revision.carpeta_de_modelos);
    let en_modelos = ficheros_con_tamano(&carpeta).unwrap_or_default();
    let modelos = en_modelos
        .iter()
        .filter(|(fichero, _)| es_modelo(fichero))
        .map(|(fichero, _)| ModeloDisponible {
            fichero: fichero.clone(),
            cabida: cabida(&encaje_en_tarjeta(
                &config.motor,
                &carpeta.join(fichero),
                gpu,
            )),
            clases: clases_que_atiende(registro, fichero, &gpu.nombre, &en_modelos),
        })
        .collect();
    Some(Disponibles {
        tarjeta: gpu.nombre.clone(),
        carpeta: carpeta.display().to_string(),
        modelos,
    })
}

/// Si el fichero es un modelo, por su extensión, sin distinguir mayúsculas.
fn es_modelo(fichero: &str) -> bool {
    crate::motor::fichero_de_modelo::es_gguf(fichero)
}

/// Las clases cuyo modelo elegido, igual que en el ciclo, es este fichero.
fn clases_que_atiende(
    registro: &Registro,
    fichero: &str,
    tarjeta: &str,
    en_modelos: &[(String, u64)],
) -> Vec<String> {
    let nombre = crate::motor::fichero_de_modelo::nombre_sin_extension(fichero).unwrap_or(fichero);
    FILAS_DE_DESEMPENO
        .into_iter()
        .filter(|fila| {
            elegir_modelo(
                registro,
                &ClasePedida::Valida(*fila),
                Some(tarjeta),
                en_modelos,
            )
            .modelo
            .as_deref()
                == Some(nombre)
        })
        .map(|fila| fila.nombre_corto().to_string())
        .collect()
}

/// La lista que se imprime al arrancar y va a la guía.
pub fn componer(disponibles: &Disponibles) -> String {
    let mut texto = format!(
        "Modelos disponibles en {} para {}, contando la tarjeta entera libre (el encaje de arriba \
         cuenta la memoria libre ahora):\n",
        disponibles.carpeta, disponibles.tarjeta
    );
    if disponibles.modelos.is_empty() {
        texto.push_str("- ninguno: la carpeta no tiene modelos\n");
    }
    for modelo in &disponibles.modelos {
        texto.push_str(&format!(
            "- {}: {}{}\n",
            modelo.fichero,
            texto_de_cabida(&modelo.cabida),
            texto_de_clases(&modelo.clases)
        ));
    }
    texto
}

fn texto_de_cabida(cabida: &Cabida) -> String {
    match cabida {
        Cabida::Entero => "cabe entero".to_string(),
        Cabida::EnParte { en_tarjeta, total } if en_tarjeta >= total => {
            format!("caben sus {total} capas en la tarjeta; los embeddings y la salida, en la CPU")
        }
        Cabida::EnParte { en_tarjeta, total } => {
            format!("cabe en parte, {en_tarjeta} de {total} capas en la tarjeta")
        }
        Cabida::Insuficiente { en_tarjeta, total } => format!(
            "no cabe: solo {en_tarjeta} de {total} capas en la tarjeta, y hacen falta {} para \
             que no sea demasiado lento",
            PROPORCION_MINIMA_DE_CAPAS_EN_TARJETA.minimo_de(*total)
        ),
        Cabida::Nada => "no cabe: ni una capa en la tarjeta".to_string(),
        Cabida::NoSeSabe(motivo) => format!("no se sabe si cabe: {motivo}"),
    }
}

fn texto_de_clases(clases: &[String]) -> String {
    if clases.is_empty() {
        String::new()
    } else {
        format!("; atiende {}", clases.join(", "))
    }
}

/// El aviso en la terminal cuando no hay con qué trabajar.
/// `que_hacer` es `revision::mensajes::que_hacer_sin_modelos`, que sabe dónde se hace la revisión.
pub fn aviso_en_la_terminal(
    disponibles: &Disponibles,
    que_hacer: &str,
    canal: &AvisoEnElCanal,
) -> String {
    let que_pasa = if disponibles.modelos.is_empty() {
        format!("⚠️ No hay ningún modelo en {}.", disponibles.carpeta)
    } else {
        format!(
            "⚠️ Ningún modelo de {} cabe en {}: con cada uno quedarían en la tarjeta menos de tres \
             cuartos de sus capas.",
            disponibles.carpeta, disponibles.tarjeta
        )
    };
    format!(
        "{que_pasa}

{que_hacer}

{}",
        texto_del_canal(canal)
    )
}

fn texto_del_canal(canal: &AvisoEnElCanal) -> String {
    match canal {
        AvisoEnElCanal::Publicado => {
            "Se ha avisado en el canal a quien puede pedir la revisión.".to_string()
        }
        AvisoEnElCanal::NoPublicado(fallo) => {
            format!("No se pudo avisar en el canal: {fallo}.")
        }
        AvisoEnElCanal::SinIdentidades => {
            "No se avisa en el canal: falta «[revision] identidades_que_ordenan».".to_string()
        }
    }
}

/// El asunto de la línea `Para` del aviso en el canal.
pub fn asunto_del_aviso(disponibles: &Disponibles) -> &'static str {
    if disponibles.modelos.is_empty() {
        ASUNTO_SIN_MODELOS
    } else {
        ASUNTO_NINGUNO_CABE
    }
}

/// El cuerpo del aviso en el canal cuando no hay con qué trabajar.
pub fn cuerpo_del_aviso(disponibles: &Disponibles, que_hacer: &str) -> String {
    let que_pasa = if disponibles.modelos.is_empty() {
        "No hay ningún modelo en la carpeta de modelos, así que Programator no puede atender \
         ningún encargo."
            .to_string()
    } else {
        format!(
            "Ningún modelo de la carpeta de modelos cabe en {}: con cada uno quedarían en la \
             tarjeta menos de tres cuartos de sus capas, y así un encargo es demasiado lento.",
            disponibles.tarjeta
        )
    };
    format!("{que_pasa} {que_hacer}\n\n{}", componer(disponibles))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Lo que compone `revision::mensajes::que_hacer_sin_modelos`, que se prueba allí.
    const QUE_HACER: &str = "Pide «revisión: buscar».";

    fn modelo(fichero: &str, cabida: Cabida, clases: &[&str]) -> ModeloDisponible {
        ModeloDisponible {
            fichero: fichero.to_string(),
            cabida,
            clases: clases.iter().map(|c| c.to_string()).collect(),
        }
    }

    fn disponibles(modelos: Vec<ModeloDisponible>) -> Disponibles {
        Disponibles {
            tarjeta: "RTX de prueba".to_string(),
            carpeta: "modelos".to_string(),
            modelos,
        }
    }

    #[test]
    fn ninguno_cabe_solo_si_no_hay_uno_que_quepa() {
        let entero = modelo("a.gguf", Cabida::Entero, &[]);
        let en_parte = modelo(
            "b.gguf",
            Cabida::EnParte {
                en_tarjeta: 30,
                total: 40,
            },
            &[],
        );
        let corto = modelo(
            "c.gguf",
            Cabida::Insuficiente {
                en_tarjeta: 29,
                total: 40,
            },
            &[],
        );
        let nada = modelo("d.gguf", Cabida::Nada, &[]);

        assert!(!disponibles(vec![entero, corto.clone()]).ninguno_cabe());
        assert!(!disponibles(vec![en_parte]).ninguno_cabe());
        assert!(disponibles(vec![corto, nada]).ninguno_cabe());
        assert!(disponibles(Vec::new()).ninguno_cabe());
    }

    #[test]
    fn la_lista_dice_de_cada_uno_si_cabe_y_que_clases_atiende() {
        let texto = componer(&disponibles(vec![
            modelo("a.gguf", Cabida::Entero, &["formas", "validaciones"]),
            modelo(
                "b.gguf",
                Cabida::EnParte {
                    en_tarjeta: 35,
                    total: 40,
                },
                &[],
            ),
            modelo(
                "c.gguf",
                Cabida::Insuficiente {
                    en_tarjeta: 20,
                    total: 40,
                },
                &[],
            ),
            modelo("d.gguf", Cabida::NoSeSabe("sin cabecera".to_string()), &[]),
        ]));

        // Con una tarjeta real, la lista decía 39 de 40 y el encaje 33 de 40, sin explicar que la
        // lista cuenta la tarjeta entera libre.
        assert!(texto.starts_with(
            "Modelos disponibles en modelos para RTX de prueba, contando la tarjeta entera libre (el \
             encaje de arriba cuenta la memoria libre ahora):\n"
        ));
        assert!(texto.contains("- a.gguf: cabe entero; atiende formas, validaciones\n"));
        assert!(texto.contains("- b.gguf: cabe en parte, 35 de 40 capas en la tarjeta\n"));
        assert!(texto.contains("solo 20 de 40 capas en la tarjeta, y hacen falta 30"));
        assert!(texto.contains("- d.gguf: no se sabe si cabe: sin cabecera\n"));
        assert!(componer(&disponibles(Vec::new())).contains("ninguno: la carpeta no tiene modelos"));
    }

    /// Con todas sus capas en la tarjeta no sale «cabe en parte, 40 de 40 capas».
    #[test]
    fn con_todas_sus_capas_en_la_tarjeta_no_dice_que_cabe_en_parte() {
        let texto = texto_de_cabida(&Cabida::EnParte {
            en_tarjeta: 40,
            total: 40,
        });
        assert!(!texto.contains("en parte"), "{texto}");
        assert!(texto.contains("sus 40 capas"), "{texto}");
    }

    #[test]
    fn el_aviso_pide_la_revision_y_no_despierta_a_nadie_por_su_cuenta() {
        let ninguno = disponibles(vec![modelo("c.gguf", Cabida::Nada, &[])]);

        let cuerpo = cuerpo_del_aviso(&ninguno, QUE_HACER);
        assert!(cuerpo.contains("«revisión: buscar»"));
        assert!(crate::protocolo::destinatarios::destinatarios(&cuerpo).is_empty());
        assert!(cuerpo.contains("Ningún modelo de la carpeta de modelos cabe"));
        assert_eq!(asunto_del_aviso(&ninguno), ASUNTO_NINGUNO_CABE);
        assert!(
            aviso_en_la_terminal(&ninguno, QUE_HACER, &AvisoEnElCanal::Publicado)
                .contains("Se ha avisado en el canal")
        );
        assert!(
            aviso_en_la_terminal(&ninguno, QUE_HACER, &AvisoEnElCanal::SinIdentidades)
                .contains("identidades_que_ordenan")
        );
    }

    /// Con la carpeta vacía no se dice «ningún modelo cabe».
    #[test]
    fn sin_modelos_el_aviso_dice_que_no_hay_y_no_que_no_caben() {
        let vacia = disponibles(Vec::new());

        let terminal = aviso_en_la_terminal(&vacia, QUE_HACER, &AvisoEnElCanal::Publicado);
        assert!(
            terminal.contains("No hay ningún modelo en modelos"),
            "{terminal}"
        );
        assert!(!terminal.contains("cabe en"), "{terminal}");
        let cuerpo = cuerpo_del_aviso(&vacia, QUE_HACER);
        assert!(cuerpo.starts_with("No hay ningún modelo"), "{cuerpo}");
        assert!(!cuerpo.contains("Ningún modelo"), "{cuerpo}");
        assert_eq!(asunto_del_aviso(&vacia), ASUNTO_SIN_MODELOS);
    }

    /// Un solo párrafo largo costaba leerlo. Qué pasa, qué hacer y qué se ha hecho en el canal
    /// van en párrafos separados por una línea en blanco.
    #[test]
    fn el_aviso_de_la_terminal_va_en_parrafos() {
        let vacia = disponibles(Vec::new());

        let terminal = aviso_en_la_terminal(&vacia, QUE_HACER, &AvisoEnElCanal::SinIdentidades);

        let parrafos: Vec<&str> = terminal.split("\n\n").collect();
        assert_eq!(parrafos.len(), 3, "{terminal}");
        assert!(
            parrafos[0].starts_with("⚠️ No hay ningún modelo"),
            "{terminal}"
        );
        assert_eq!(parrafos[1], QUE_HACER);
        assert!(
            parrafos[2].starts_with("No se avisa en el canal"),
            "{terminal}"
        );
    }

    /// Un fallo al publicar no es lo mismo que no tener a quién avisar.
    #[test]
    fn un_fallo_al_publicar_se_dice_como_fallo_y_no_como_falta_de_identidades() {
        let ninguno = disponibles(vec![modelo("c.gguf", Cabida::Nada, &[])]);

        let fallo = aviso_en_la_terminal(
            &ninguno,
            QUE_HACER,
            &AvisoEnElCanal::NoPublicado("disco lleno".to_string()),
        );
        assert!(
            fallo.contains("No se pudo avisar en el canal: disco lleno."),
            "{fallo}"
        );
        assert!(!fallo.contains("identidades_que_ordenan"), "{fallo}");
    }

    #[test]
    fn cada_modelo_atiende_las_clases_en_que_lo_elige_el_ciclo() {
        use crate::banco::resultados::FilaDeDesempeno;
        use crate::registro::modelo::pruebas::entrada_de_ejemplo;
        use crate::registro::modelo::{CodigoPuntuadoCon, EntradaDeModelo, NotaDeClase};
        const TARJETA: &str = "RTX de prueba";
        let modelo = |nombre: &str, nota: f64, en_diagnostico: f64| -> EntradaDeModelo {
            let mut e = entrada_de_ejemplo();
            e.nombre = nombre.into();
            e.tarjeta = TARJETA.into();
            e.retirado_el = None;
            e.codigo_puntuado_con = Some(CodigoPuntuadoCon::PruebasOcultas);
            e.clase = FILAS_DE_DESEMPENO
                .iter()
                .map(|&clase| NotaDeClase {
                    clase,
                    nota: Some(if clase == FilaDeDesempeno::Diagnostico {
                        en_diagnostico
                    } else {
                        nota
                    }),
                    inventados: 0,
                    tokens_por_segundo: Some(10.0),
                })
                .collect();
            e
        };
        let registro = Registro {
            modelo: vec![modelo("a", 4.0, 1.0), modelo("b", 1.0, 4.5)],
        };
        let en_modelos = vec![("a.gguf".to_string(), 1), ("b.gguf".to_string(), 1)];

        let de_b = clases_que_atiende(&registro, "b.gguf", TARJETA, &en_modelos);
        let de_a = clases_que_atiende(&registro, "a.gguf", TARJETA, &en_modelos);

        assert_eq!(de_b, ["diagnostico"]);
        assert_eq!(de_a.len(), FILAS_DE_DESEMPENO.len() - 1, "{de_a:?}");
        assert!(!de_a.contains(&"diagnostico".to_string()));
        assert!(clases_que_atiende(&registro, "a.gguf", "otra", &en_modelos).is_empty());
    }

    #[test]
    fn solo_los_gguf_son_modelos() {
        assert!(es_modelo("a.gguf"));
        assert!(es_modelo("A.GGUF"));
        assert!(!es_modelo("a.gguf.part"));
        assert!(!es_modelo("probados.toml"));
    }
}
