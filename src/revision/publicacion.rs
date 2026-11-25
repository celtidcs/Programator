//! Lo que Programator publica en su buzón al atender un pedido de revisión: lo que hizo y lo que
//! toca después. Lo lee el modelo de pago que dirige la revisión, así que cada respuesta acaba
//! diciendo cuál es el paso siguiente.

use std::fmt::Write as _;
use std::path::PathBuf;

use super::buscar::BusquedaHecha;
use super::cerrar::{Cerrado, Desenlace, EnUsoAusente, Regla};
use super::mensajes;
use super::probados::{Estado, Probado};
use super::probar::Prueba;
use super::reponer::Reposicion;
use crate::banco::tanda::{CARPETA_DE_RUBRICAS, FICHERO_DE_MOTIVOS};
use crate::concordancia::{cantidad, segun};
use crate::error::Error;
use crate::informe::en_gib;
use crate::motor::fichero_de_modelo::con_extension;

/// Las rúbricas que faltan de cada tanda.
pub type Pendientes = Vec<(PathBuf, Vec<PathBuf>)>;

pub fn busqueda(hecha: &BusquedaHecha) -> String {
    format!(
        "## Revisión: búsqueda\n\n{}{}\n\nSiguiente paso: elige qué candidatos probar y pídelo \
         con «revisión: probar», un candidato por línea («<repositorio> <fichero>»).",
        reposicion(&hecha.reposicion),
        mensajes::busqueda_hecha(hecha)
    )
}

/// Lo que se hizo con los modelos del registro que faltaban, antes de los candidatos. Sin
/// ausentes, nada.
fn reposicion(reposicion: &Reposicion) -> String {
    if !reposicion.hubo_ausentes() {
        return String::new();
    }
    let mut texto = String::from("Reposición:\n");
    for repuesto in &reposicion.repuestos {
        let _ = writeln!(
            texto,
            "- repuesto: «{}» ({}, huella comprobada)",
            con_extension(&repuesto.nombre),
            en_gib(repuesto.bytes)
        );
    }
    for sin in &reposicion.sin_reponer {
        let _ = writeln!(
            texto,
            "- sin reponer: «{}»: {}",
            con_extension(&sin.nombre),
            mensajes::motivo_sin_reponer(&sin.motivo)
        );
    }
    for nombre in &reposicion.fuera_del_registro {
        let _ = writeln!(
            texto,
            "- fuera del registro: «{nombre}»: falta y no gana nada"
        );
    }
    for nombre in &reposicion.sin_decidir {
        let _ = writeln!(
            texto,
            "- sin decidir: «{nombre}»: falta y no se sabe si es útil"
        );
    }
    texto + "\n"
}

pub fn prueba(prueba: &Prueba, pendientes: &Pendientes) -> String {
    let mut texto = String::from("## Revisión: prueba\n\n");
    for probado in &prueba.probados {
        texto.push_str(&linea_de_probado(probado));
        texto.push('\n');
    }
    if !prueba.fuera_del_tope.is_empty() {
        let fuera: Vec<String> = prueba
            .fuera_del_tope
            .iter()
            .map(|c| format!("«{} {}»", c.repositorio, c.fichero))
            .collect();
        let _ = writeln!(
            texto,
            "\nNo se han probado, porque pasan del tope de «[revision] candidatos_por_revision»: {}.",
            fuera.join(", ")
        );
    }
    texto.push_str(&rubricas_que_faltan(pendientes));
    texto
}

fn linea_de_probado(probado: &Probado) -> String {
    let nombre = format!("«{}» de «{}»", probado.fichero, probado.repositorio);
    let tanda = probado
        .tanda
        .as_ref()
        .map_or(String::new(), |t| format!(" Tanda: «{}».", t.display()));
    let motivo = probado.motivo.as_deref().unwrap_or("");
    match probado.estado {
        Estado::Comprobada => format!("- ✅ {nombre}: tanda pasada y comprobada.{tanda}"),
        Estado::SinComprobar => format!(
            "- ⏳ {nombre}: las pasadas están guardadas, pero no se pudieron comprobar \
             ({motivo}). Se vuelve a intentar al cerrar.{tanda}"
        ),
        Estado::Fallido => format!("- 🔴 {nombre}: no se llegó a pasar el banco: {motivo}."),
    }
}

/// «esta rúbrica» o «estas 3 rúbricas».
fn estas_rubricas(n: usize) -> String {
    match n {
        1 => "esta rúbrica".to_string(),
        n => format!("estas {n} rúbricas"),
    }
}

fn rubricas_que_faltan(pendientes: &Pendientes) -> String {
    let todas: Vec<&PathBuf> = pendientes.iter().flat_map(|(_, r)| r.iter()).collect();
    if todas.is_empty() {
        return "\nSiguiente paso: no queda ninguna rúbrica por puntuar; pide «revisión: cerrar»."
            .to_string();
    }
    let mut texto = format!(
        "\nSiguiente paso: puntúa {}, con la nota en su línea «Nota:» y los motivos en \
         «{CARPETA_DE_RUBRICAS}/{FICHERO_DE_MOTIVOS}» de cada tanda, y después pide \
         «revisión: cerrar».\n",
        estas_rubricas(todas.len())
    );
    for rubrica in todas {
        let _ = writeln!(texto, "- «{}»", rubrica.display());
    }
    texto
}

pub fn cierre(cerrado: &Cerrado) -> String {
    let mut texto = String::from("## Revisión: cierre\n\n");
    for tanda in &cerrado.comprobadas_ahora {
        let _ = writeln!(texto, "- ✅ Comprobada ahora: «{}».", tanda.display());
    }
    for (tanda, motivo) in &cerrado.sin_comprobar {
        let _ = writeln!(
            texto,
            "- ⏳ Sigue sin comprobar, y no cuenta: «{}» ({motivo}).",
            tanda.display()
        );
    }
    match &cerrado.desenlace {
        Desenlace::FaltanRubricas(rubricas) => {
            let _ = writeln!(
                texto,
                "\nNo se ha cerrado nada: {} {} sin nota. {} y vuelve a pedir «revisión: cerrar».",
                segun(rubricas.len(), "queda", "quedan"),
                cantidad(rubricas.len(), "rúbrica", "rúbricas"),
                segun(rubricas.len(), "Puntúala", "Puntúalas")
            );
            for rubrica in rubricas {
                let _ = writeln!(texto, "- «{}»", rubrica.display());
            }
        }
        Desenlace::Aplicada(regla) => texto.push_str(&regla_aplicada(regla)),
    }
    texto
}

fn regla_aplicada(regla: &Regla) -> String {
    let mut texto = String::new();
    for (tanda, motivo) in &regla.sin_anotar {
        let _ = writeln!(
            texto,
            "- ⚠️ No se anotó en el registro «{}»: {motivo}",
            tanda.display()
        );
    }
    let _ = writeln!(texto, "\nSe quedan: {}.", lista(&regla.se_quedan));
    match (&regla.cambio_de_modelo, &regla.en_uso_sin_sustituto) {
        (Some((viejo, nuevo)), _) => {
            let _ = writeln!(
                texto,
                "🔁 Modelo por defecto cambiado: «{viejo}» no ganaba nada y lo sustituye «{nuevo}». \
                 El siguiente encargo ya se atiende con él."
            );
        }
        (None, Some(nombre)) => {
            let _ = writeln!(
                texto,
                "⚠️ El modelo por defecto, «{nombre}», no gana nada, pero no hay quien lo sustituya: \
                 se queda."
            );
        }
        (None, None) => {}
    }
    texto.push_str(&en_uso_ausente(regla.en_uso_ausente.as_ref()));
    if regla.retirados.is_empty() {
        texto.push_str("No se retira ninguno.\n");
    }
    for (nombre, liberados) in &regla.retirados {
        let _ = writeln!(
            texto,
            "🗑️ Retirado «{nombre}»: se han liberado {}.",
            en_gib(*liberados)
        );
    }
    for (nombre, motivo) in &regla.sin_retirar {
        let _ = writeln!(texto, "🔴 Tocaba retirar «{nombre}» y no se pudo: {motivo}");
    }
    texto.push_str(&ausentes_del_cierre(regla));
    texto.push_str(
        "\nSiguiente paso: escribe el informe para quien usa Programator en la carpeta del mes \
         de «revisiones/», como «informe.md».",
    );
    texto
}

/// El modelo por defecto que falta en la carpeta de modelos y no se pudo reponer.
fn en_uso_ausente(ausente: Option<&EnUsoAusente>) -> String {
    match ausente {
        Some(EnUsoAusente {
            nombre,
            sustituto: Some(nuevo),
        }) => format!(
            "🔁 Modelo por defecto cambiado: el fichero de «{nombre}» falta en «modelos» y no se \
             pudo reponer, así que lo sustituye «{nuevo}», el que más gana. «{nombre}» sigue en el \
             registro, y la siguiente «revisión: buscar» intentará reponerlo. El siguiente encargo \
             ya se atiende con «{nuevo}».\n"
        ),
        Some(EnUsoAusente {
            nombre,
            sustituto: None,
        }) => format!(
            "⚠️ El fichero del modelo por defecto, «{nombre}», falta en «modelos» y no se pudo \
             reponer, y ningún otro gana nada para sustituirlo. Pide «revisión: buscar» para \
             reponerlo, o déjalo a mano en «modelos».\n"
        ),
        None => String::new(),
    }
}

/// Los ausentes que salen del registro y los útiles que siguen faltando.
fn ausentes_del_cierre(regla: &Regla) -> String {
    let mut texto = String::new();
    if !regla.fuera_del_registro.is_empty() {
        let _ = writeln!(
            texto,
            "📤 Salen del registro, porque falta su fichero en «modelos» y no ganan nada: {}.",
            lista(&regla.fuera_del_registro)
        );
    }
    if !regla.faltan_utiles.is_empty() {
        let _ = writeln!(
            texto,
            "📦 Faltan en «modelos» y siguen en el registro, porque ganan algo o son el modelo por \
             defecto: {}. La siguiente «revisión: buscar» intentará reponerlos.",
            lista(&regla.faltan_utiles)
        );
    }
    texto
}

fn lista(nombres: &[String]) -> String {
    if nombres.is_empty() {
        return "ninguno".to_string();
    }
    nombres
        .iter()
        .map(|n| format!("«{n}»"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// El pedido no se entiende o quien lo pide no puede.
pub fn rechazado(motivo: &str) -> String {
    format!("## Revisión: no se atiende\n\n🔴 {motivo}.")
}

/// El pedido se entendió, pero falló al hacerlo.
pub fn fallido(pedido: &str, fallo: &Error) -> String {
    format!("## Revisión: {pedido}\n\n{}", mensajes::fallo(fallo))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::revision::encargo::CandidatoPedido;

    /// «Puntúa esta rúbrica» con una sola, no «estas 1 rúbricas».
    #[test]
    fn el_siguiente_paso_con_una_rubrica_va_en_singular() {
        let una: Pendientes = vec![(PathBuf::from("t1"), vec![PathBuf::from("t1/rubricas/a.md")])];
        assert!(rubricas_que_faltan(&una).contains("puntúa esta rúbrica,"));
        let dos: Pendientes = vec![(
            PathBuf::from("t1"),
            vec![
                PathBuf::from("t1/rubricas/a.md"),
                PathBuf::from("t1/rubricas/b.md"),
            ],
        )];
        assert!(rubricas_que_faltan(&dos).contains("puntúa estas 2 rúbricas,"));
    }

    fn probado(estado: Estado, motivo: Option<&str>) -> Probado {
        Probado {
            repositorio: "autor/M-GGUF".into(),
            fichero: "m.gguf".into(),
            estado,
            tanda: Some(PathBuf::from("banco/resultados/t1")),
            motivo: motivo.map(str::to_string),
        }
    }

    #[test]
    fn la_prueba_dice_cada_candidato_y_las_rubricas_que_puntuar() {
        let texto = prueba(
            &Prueba {
                probados: vec![
                    probado(Estado::Comprobada, None),
                    probado(Estado::Fallido, Some("la huella no coincide")),
                ],
                fuera_del_tope: vec![CandidatoPedido {
                    repositorio: "otro/R".into(),
                    fichero: "r.gguf".into(),
                }],
            },
            &vec![(
                PathBuf::from("t1"),
                vec![PathBuf::from("t1/rubricas/E01-42.md")],
            )],
        );

        assert!(texto.contains("✅ «m.gguf»"), "{texto}");
        assert!(texto.contains("la huella no coincide"), "{texto}");
        assert!(texto.contains("«otro/R r.gguf»"), "{texto}");
        assert!(texto.contains("t1/rubricas/E01-42.md"), "{texto}");
        assert!(texto.contains("revisión: cerrar"), "{texto}");
    }

    #[test]
    fn el_cierre_dice_el_cambio_de_modelo_y_lo_retirado() {
        let texto = cierre(&Cerrado {
            comprobadas_ahora: Vec::new(),
            sin_comprobar: Vec::new(),
            desenlace: Desenlace::Aplicada(Box::new(Regla {
                se_quedan: vec!["nuevo".into()],
                cambio_de_modelo: Some(("devstral".into(), "nuevo".into())),
                retirados: vec![("devstral".into(), 14 * 1024 * 1024 * 1024)],
                ..Regla::default()
            })),
        });

        assert!(texto.contains("Se quedan: «nuevo»."), "{texto}");
        assert!(texto.contains("«devstral» no ganaba nada"), "{texto}");
        assert!(texto.contains("🗑️ Retirado «devstral»"), "{texto}");
        assert!(texto.contains("informe.md"), "{texto}");
        assert!(
            texto.contains("escribe el informe para quien usa Programator"),
            "{texto}"
        );
    }

    #[test]
    fn el_cierre_nombra_los_ausentes_y_el_modelo_por_defecto_que_falta() {
        let con = |en_uso_ausente| {
            cierre(&Cerrado {
                comprobadas_ahora: Vec::new(),
                sin_comprobar: Vec::new(),
                desenlace: Desenlace::Aplicada(Box::new(Regla {
                    se_quedan: vec!["nuevo".into()],
                    en_uso_ausente,
                    fuera_del_registro: vec!["perdido".into()],
                    faltan_utiles: vec!["devstral".into(), "bueno".into()],
                    ..Regla::default()
                })),
            })
        };
        let cambiado = con(Some(EnUsoAusente {
            nombre: "devstral".into(),
            sustituto: Some("nuevo".into()),
        }));
        assert!(
            cambiado.contains("el fichero de «devstral» falta en «modelos» y no se pudo reponer, así que lo sustituye «nuevo»"),
            "{cambiado}"
        );
        assert!(
            cambiado.contains("Salen del registro, porque falta su fichero en «modelos» y no ganan nada: «perdido»."),
            "{cambiado}"
        );
        assert!(
            cambiado.contains("son el modelo por defecto: «devstral», «bueno». La siguiente «revisión: buscar» intentará reponerlos."),
            "{cambiado}"
        );
        let sin_sustituto = con(Some(EnUsoAusente {
            nombre: "devstral".into(),
            sustituto: None,
        }));
        assert!(
            sin_sustituto.contains("ningún otro gana nada para sustituirlo"),
            "{sin_sustituto}"
        );
        let sin_ausentes = cierre(&Cerrado {
            comprobadas_ahora: Vec::new(),
            sin_comprobar: Vec::new(),
            desenlace: Desenlace::Aplicada(Box::default()),
        });
        assert!(!sin_ausentes.contains("«modelos»"), "{sin_ausentes}");
    }

    #[test]
    fn el_cierre_con_rubricas_sin_nota_no_cierra_y_las_lista() {
        let texto = cierre(&Cerrado {
            comprobadas_ahora: Vec::new(),
            sin_comprobar: vec![(PathBuf::from("t2"), "el intérprete no responde".into())],
            desenlace: Desenlace::FaltanRubricas(vec![PathBuf::from("t1/rubricas/E01-42.md")]),
        });

        assert!(texto.contains("No se ha cerrado nada"), "{texto}");
        // Con una sola, en singular.
        assert!(
            texto.contains("queda 1 rúbrica sin nota. Puntúala y"),
            "{texto}"
        );
        assert!(texto.contains("t1/rubricas/E01-42.md"), "{texto}");
        assert!(texto.contains("el intérprete no responde"), "{texto}");
    }

    fn busqueda_con(reposicion: Reposicion) -> String {
        busqueda(&BusquedaHecha {
            carpeta: PathBuf::from("revisiones/2026-11"),
            candidatos: 2,
            ya_descargados: 0,
            descartados: 5,
            anterior: None,
            reposicion,
        })
    }

    #[test]
    fn la_respuesta_lleva_la_reposicion_solo_si_hubo_ausentes() {
        use crate::revision::reponer::{MotivoSinReponer, Repuesto, SinReponer};

        let sin_ausentes = busqueda_con(Reposicion::default());
        assert!(!sin_ausentes.contains("Reposición"), "{sin_ausentes}");

        let con_ausentes = busqueda_con(Reposicion {
            repuestos: vec![Repuesto {
                nombre: "JackOD-9B-Coder-CD-Q4_K_M".into(),
                repositorio: "r/JackOD-GGUF".into(),
                bytes: 5 * 1024 * 1024 * 1024,
            }],
            sin_reponer: vec![SinReponer {
                nombre: "X".into(),
                motivo: MotivoSinReponer::SinOrigen,
            }],
            fuera_del_registro: vec!["devstral-small-2-24b-Q4_K_M".into()],
            sin_decidir: vec!["Y".into()],
            ..Reposicion::default()
        });
        let bloque = con_ausentes.find("Reposición:\n").unwrap();
        let candidatos = con_ausentes.find("candidatos").unwrap();
        assert!(bloque < candidatos, "{con_ausentes}");
        for linea in [
            "- repuesto: «JackOD-9B-Coder-CD-Q4_K_M.gguf» (5,0 GiB, huella comprobada)\n",
            "- sin reponer: «X.gguf»: no se sabe de dónde salió",
            "- fuera del registro: «devstral-small-2-24b-Q4_K_M»: falta y no gana nada\n",
            "- sin decidir: «Y»: falta y no se sabe si es útil\n",
        ] {
            assert!(con_ausentes.contains(linea), "{linea}\n{con_ausentes}");
        }
    }
}
