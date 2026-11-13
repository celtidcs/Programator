//! Lo que Programator publica en su buzón al atender un pedido de revisión: lo que hizo y lo que
//! toca después. Lo lee el modelo de pago que dirige la revisión, así que cada respuesta acaba
//! diciendo cuál es el paso siguiente (especificación de la revisión autónoma, §3).

use std::fmt::Write as _;
use std::path::PathBuf;

use super::buscar::BusquedaHecha;
use super::cerrar::{Cerrado, Desenlace, Regla};
use super::mensajes;
use super::probados::{Estado, Probado};
use super::probar::Prueba;
use crate::error::Error;
use crate::informe::en_gib;

/// Las rúbricas que faltan de cada tanda.
pub type Pendientes = Vec<(PathBuf, Vec<PathBuf>)>;

pub fn busqueda(hecha: &BusquedaHecha) -> String {
    format!(
        "## Revisión: búsqueda\n\n{}\n\nSiguiente paso: elige qué candidatos probar y pídelo con \
         «revisión: probar», un candidato por línea («<repositorio> <fichero>»).",
        mensajes::busqueda_hecha(hecha)
    )
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

fn rubricas_que_faltan(pendientes: &Pendientes) -> String {
    let todas: Vec<&PathBuf> = pendientes.iter().flat_map(|(_, r)| r.iter()).collect();
    if todas.is_empty() {
        return "\nSiguiente paso: no queda ninguna rúbrica por puntuar; pide «revisión: cerrar»."
            .to_string();
    }
    let mut texto = format!(
        "\nSiguiente paso: puntúa estas {} rúbricas, con la nota en su línea «Nota:» y los motivos \
         en «rubricas/motivos-de-las-notas.md» de cada tanda, y después pide «revisión: cerrar».\n",
        todas.len()
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
                "\nNo se ha cerrado nada: quedan {} rúbricas sin nota. Puntúalas y vuelve a pedir \
                 «revisión: cerrar».",
                rubricas.len()
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
    texto.push_str(
        "\nSiguiente paso: escribe el informe para el Director en la carpeta del mes de \
         «revisiones/», como «informe.md».",
    );
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
            desenlace: Desenlace::Aplicada(Regla {
                se_quedan: vec!["nuevo".into()],
                cambio_de_modelo: Some(("devstral".into(), "nuevo".into())),
                retirados: vec![("devstral".into(), 14 * 1024 * 1024 * 1024)],
                ..Regla::default()
            }),
        });

        assert!(texto.contains("Se quedan: «nuevo»."), "{texto}");
        assert!(texto.contains("«devstral» no ganaba nada"), "{texto}");
        assert!(texto.contains("🗑️ Retirado «devstral»"), "{texto}");
        assert!(texto.contains("informe.md"), "{texto}");
    }

    #[test]
    fn el_cierre_con_rubricas_sin_nota_no_cierra_y_las_lista() {
        let texto = cierre(&Cerrado {
            comprobadas_ahora: Vec::new(),
            sin_comprobar: vec![(PathBuf::from("t2"), "el intérprete no responde".into())],
            desenlace: Desenlace::FaltanRubricas(vec![PathBuf::from("t1/rubricas/E01-42.md")]),
        });

        assert!(texto.contains("No se ha cerrado nada"), "{texto}");
        assert!(texto.contains("t1/rubricas/E01-42.md"), "{texto}");
        assert!(texto.contains("el intérprete no responde"), "{texto}");
    }
}
