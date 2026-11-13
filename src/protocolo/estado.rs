//! Escritura en `estado.md`, el único fichero compartido del canal.
//!
//! Programator solo toca **sus** reservas: las anota al empezar algo y las quita al terminarlo. No
//! borra las ajenas ni reescribe hechos: `estado.md` lo actualiza quien cambia el hecho.
//!
//! El formato es el del gestor de agentes que coordina el canal, que revisa `estado.md` cada vez
//! que abre un proyecto y avisa de lo que no sigue su plantilla. Lo que lee es:
//! `- <Agente>, <AAAA-MM-DD HH:MM>: <ruta>[, <ruta>…] — <motivo>`, y `Ninguna.` si no hay ninguna.

use super::saneado::{neutralizar_marcas, sanear_saltos};
use crate::error::{Error, Resultado};
use std::path::Path;

/// El encabezado con que se crea la sección si `estado.md` no la tiene.
const SECCION: &str = "## Reservas activas";
/// Con lo que empieza un `estado.md` que no existía.
const ESTADO_NUEVO: &str = "# Estado — hechos comprobables\n";
/// El título de la sección, en cualquier nivel de encabezado o sin él (el gestor de agentes lo
/// escribía antes como una línea suelta).
const TITULO_DE_LA_SECCION: &str = "reservas activas";
/// Lo que dice la sección cuando queda vacía, como la plantilla de `estado.md` del canal.
pub const SIN_RESERVAS: &str = "Ninguna.";
/// Lo que Programator escribía antes en la sección vacía. Se sigue reconociendo para quitarlo:
/// el gestor de agentes lo lee como una reserva ilegible.
const SIN_RESERVAS_ANTIGUO: &str = "- *(ninguna)*";
/// Separa la ruta del motivo, como en la plantilla del canal.
const SEPARADOR_DEL_MOTIVO: &str = " — ";
/// Separa una ruta de otra en la plantilla del canal; dentro de una ruta partiría la reserva
/// en dos.
const SEPARADOR_DE_RUTAS: &str = ", ";

/// Una reserva que anota Programator.
#[derive(Debug, Clone, Copy)]
pub struct Reserva<'a> {
    /// Quién reserva, el nombre del agente tal como está configurado.
    pub agente: &'a str,
    /// La hora del sistema, `AAAA-MM-DD HH:MM`.
    pub cuando: &'a str,
    /// Lo reservado: una ruta relativa, o una máquina.
    pub ruta: &'a str,
    /// Por qué.
    pub motivo: &'a str,
}

/// La línea de la reserva, con el formato del canal.
///
/// Todo lo que entra puede venir del modelo, así que se sanea: ningún salto de línea (inyectaría
/// líneas falsas en un fichero compartido), ninguna marca del arnés, y ni la raya del motivo ni la
/// coma de separar rutas dentro de la ruta, que cambiarían lo que lee el gestor de agentes.
pub fn linea_de_reserva(reserva: &Reserva) -> String {
    let agente = neutralizar_marcas(&sanear_saltos(reserva.agente));
    let cuando = sanear_saltos(reserva.cuando);
    let ruta = sanear_saltos(reserva.ruta)
        .replace('`', "")
        .replace(SEPARADOR_DEL_MOTIVO, " - ")
        .replace(SEPARADOR_DE_RUTAS, ",");
    let motivo = neutralizar_marcas(&sanear_saltos(reserva.motivo));
    format!("- {agente}, {cuando}: {ruta}{SEPARADOR_DEL_MOTIVO}{motivo}")
}

/// Añade la reserva bajo `## Reservas activas`, creando la sección (y `estado.md`) si faltan, y
/// devuelve la línea escrita, que es lo que hay que pasar a [`retirar_reserva`] al terminar.
///
/// Una línea idéntica ya anotada no se repite.
pub fn anotar_reserva(ruta_estado: &Path, reserva: &Reserva) -> Resultado<String> {
    let texto = if ruta_estado.exists() {
        leer(ruta_estado)?
    } else {
        ESTADO_NUEVO.to_string()
    };
    let linea = linea_de_reserva(reserva);
    if !texto.lines().any(|l| l.trim() == linea) {
        escribir(ruta_estado, &con_la_linea(&texto, &linea))?;
    }
    Ok(linea)
}

/// Quita de `estado.md` exactamente esa línea de reserva. Si ya no está, o no hay `estado.md`, no
/// hace nada.
pub fn retirar_reserva(ruta_estado: &Path, linea: &str) -> Resultado<()> {
    if !ruta_estado.exists() {
        return Ok(());
    }
    let texto = leer(ruta_estado)?;
    if !texto.lines().any(|l| l.trim() == linea.trim()) {
        return Ok(());
    }
    escribir(ruta_estado, &sin_la_linea(&texto, linea))
}

fn leer(ruta: &Path) -> Resultado<String> {
    std::fs::read_to_string(ruta).map_err(|causa| Error::Lectura {
        ruta: ruta.to_path_buf(),
        causa,
    })
}

fn escribir(ruta: &Path, texto: &str) -> Resultado<()> {
    std::fs::write(ruta, texto).map_err(|causa| Error::Escritura {
        ruta: ruta.to_path_buf(),
        causa,
    })
}

/// Si `linea` es el título de la sección «Reservas activas».
pub fn es_titulo_de_la_seccion(linea: &str) -> bool {
    linea
        .trim()
        .trim_start_matches('#')
        .trim()
        .trim_matches('*')
        .trim()
        .eq_ignore_ascii_case(TITULO_DE_LA_SECCION)
}

/// Si la línea solo dice que la sección está vacía, en la forma de ahora o en la antigua.
fn dice_que_no_hay_ninguna(linea: &str) -> bool {
    let recortada = linea.trim();
    recortada == SIN_RESERVAS || recortada == SIN_RESERVAS_ANTIGUO
}

/// Las líneas de la sección: tras el título, las de lista y sus continuaciones sangradas, hasta
/// un encabezado o una línea que no es de lista. El «ninguna» no cuenta como reserva.
pub fn lineas_de_la_seccion(estado: &str) -> Vec<&str> {
    let mut lineas = estado.lines().skip_while(|l| !es_titulo_de_la_seccion(l));
    if lineas.next().is_none() {
        return Vec::new();
    }
    lineas
        .take_while(|l| {
            let recortada = l.trim_start();
            recortada.is_empty()
                || dice_que_no_hay_ninguna(l)
                || recortada.starts_with("- ")
                || recortada.starts_with("* ")
                || (l.starts_with(char::is_whitespace) && !recortada.starts_with('#'))
        })
        .filter(|l| !l.trim().is_empty() && !dice_que_no_hay_ninguna(l))
        .collect()
}

/// Añade `linea` al principio de la sección, quitando el «ninguna» de la sección si estaba (solo
/// de esa: el de «Mutaciones vivas» se queda). Si no hay sección, la crea al final.
pub fn con_la_linea(estado: &str, linea: &str) -> String {
    let mut lineas: Vec<String> = estado.lines().map(str::to_string).collect();
    match lineas.iter().position(|l| es_titulo_de_la_seccion(l)) {
        Some(titulo) => {
            let fin = fin_de_la_seccion(&lineas, titulo);
            let mut quitadas = 0;
            for indice in (titulo + 1..fin).rev() {
                if dice_que_no_hay_ninguna(&lineas[indice]) {
                    lineas.remove(indice);
                    quitadas += 1;
                }
            }
            let fin = fin - quitadas;
            let hueco = if lineas.get(titulo + 1).is_some_and(|l| l.trim().is_empty()) {
                titulo + 2
            } else {
                titulo + 1
            };
            lineas.insert(hueco.min(fin.max(titulo + 1)), linea.to_string());
        }
        None => {
            lineas.push(String::new());
            lineas.push(SECCION.to_string());
            lineas.push(String::new());
            lineas.push(linea.to_string());
        }
    }
    format!("{}\n", lineas.join("\n"))
}

/// Dónde acaba la sección que empieza en `titulo`: en el siguiente encabezado o al final.
fn fin_de_la_seccion(lineas: &[String], titulo: usize) -> usize {
    lineas
        .iter()
        .enumerate()
        .skip(titulo + 1)
        .find(|(_, l)| l.trim_start().starts_with('#'))
        .map_or(lineas.len(), |(indice, _)| indice)
}

/// Quita exactamente `linea`; si la sección se queda vacía, vuelve a poner el «ninguna».
pub fn sin_la_linea(estado: &str, linea: &str) -> String {
    let mut lineas: Vec<String> = estado
        .lines()
        .filter(|l| l.trim() != linea.trim())
        .map(str::to_string)
        .collect();
    let texto = format!("{}\n", lineas.join("\n"));
    if lineas_de_la_seccion(&texto).is_empty() {
        if let Some(titulo) = lineas.iter().position(|l| es_titulo_de_la_seccion(l)) {
            let fin = fin_de_la_seccion(&lineas, titulo);
            let ya_lo_dice = lineas[titulo + 1..fin]
                .iter()
                .any(|l| dice_que_no_hay_ninguna(l));
            if !ya_lo_dice {
                if lineas.get(titulo + 1).is_some_and(|l| l.trim().is_empty()) {
                    lineas.insert(titulo + 2, SIN_RESERVAS.to_string());
                } else {
                    lineas.insert(titulo + 1, String::new());
                    lineas.insert(titulo + 2, SIN_RESERVAS.to_string());
                }
            }
        }
        return format!("{}\n", lineas.join("\n"));
    }
    texto
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn reserva<'a>(ruta: &'a str, motivo: &'a str) -> Reserva<'a> {
        Reserva {
            agente: "Programator",
            cuando: "2026-11-13 15:40",
            ruta,
            motivo,
        }
    }

    /// Lo que pide la plantilla del canal: agente
    /// seguido de coma, la hora, dos puntos, las rutas y el motivo tras la raya.
    #[test]
    fn la_linea_sigue_el_formato_del_canal() {
        assert_eq!(
            linea_de_reserva(&reserva("src/main.rs", "refactor del arranque")),
            "- Programator, 2026-11-13 15:40: src/main.rs — refactor del arranque"
        );
    }

    #[test]
    fn anade_la_reserva_bajo_la_seccion_correspondiente() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(
            &ruta,
            "# Estado\n\n## Reservas activas\n\n- Codex, 2026-11-13 10:00: src/main.rs — refactor\n\n## Código\n\n- Rama: main\n",
        )
        .unwrap();

        let linea = anotar_reserva(
            &ruta,
            &reserva(".gestor/candidatos/poda/claude.md", "propuesta de poda"),
        )
        .unwrap();

        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert_eq!(
            linea,
            "- Programator, 2026-11-13 15:40: .gestor/candidatos/poda/claude.md — propuesta de poda"
        );
        assert!(texto.lines().any(|l| l == linea));
        assert!(
            texto.contains("- Codex, 2026-11-13 10:00: src/main.rs — refactor"),
            "no puede borrar reservas ajenas"
        );
        let pos_reserva = texto.find("Programator").unwrap();
        let pos_codigo = texto.find("## Código").unwrap();
        assert!(pos_reserva < pos_codigo, "debe quedar dentro de su sección");
    }

    #[test]
    fn crea_la_seccion_si_el_estado_no_la_tiene() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(&ruta, "# Estado\n\n## Código\n\n- Rama: main\n").unwrap();

        anotar_reserva(&ruta, &reserva("x.md", "motivo")).unwrap();

        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert!(texto.contains("## Reservas activas"));
        assert!(texto.contains("- Programator, 2026-11-13 15:40: x.md — motivo"));
    }

    #[test]
    fn no_duplica_una_reserva_identica() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(&ruta, "# Estado\n\n## Reservas activas\n\n").unwrap();
        anotar_reserva(&ruta, &reserva("x.md", "motivo")).unwrap();

        anotar_reserva(&ruta, &reserva("x.md", "motivo")).unwrap();

        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert_eq!(
            texto
                .matches("- Programator, 2026-11-13 15:40: x.md — motivo")
                .count(),
            1
        );
    }

    #[test]
    fn rechaza_inyeccion_en_motivo() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(&ruta, "# Estado\n\n## Reservas activas\n\n").unwrap();

        // Intento de inyección: un motivo que contiene líneas falsas
        let motivo_inyectado =
            "urgente\n\n- Codex, 2026-11-13 10:00: secreto.rs — inventado por inyección";
        anotar_reserva(&ruta, &reserva("x.md", motivo_inyectado)).unwrap();

        let texto = std::fs::read_to_string(&ruta).unwrap();
        let reserva_programator = texto
            .lines()
            .find(|l| l.contains("Programator") && l.contains("x.md"))
            .expect("debe haber una línea de reserva de Programator");
        assert!(
            reserva_programator.contains("urgente") && reserva_programator.contains("secreto.rs"),
            "el motivo debe quedar entero en una sola línea"
        );
        assert!(
            !texto.lines().any(|l| l.starts_with("- Codex")),
            "no puede crearse una línea separada falsa de Codex"
        );
    }

    #[test]
    fn quita_las_comillas_invertidas_de_la_ruta() {
        // El formato del canal no lleva la ruta entre comillas invertidas: si las trae, se
        // quitan, y la ruta se lee como la escribió quien reserva.
        assert_eq!(
            linea_de_reserva(&reserva("src/`secreto`.rs", "revisar")),
            "- Programator, 2026-11-13 15:40: src/secreto.rs — revisar"
        );
    }

    #[test]
    fn la_ruta_no_puede_partir_la_reserva_ni_robar_el_motivo() {
        // El gestor de agentes parte las rutas por «, » y el motivo por « — ». Una ruta del modelo con
        // cualquiera de los dos le haría leer otra reserva.
        assert_eq!(
            linea_de_reserva(&reserva("a.md, b.md — falso", "de verdad")),
            "- Programator, 2026-11-13 15:40: a.md,b.md - falso — de verdad"
        );
    }

    #[test]
    fn anade_ambas_reservas_si_el_motivo_de_la_segunda_es_prefijo_de_la_primera() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(&ruta, "# Estado\n\n## Reservas activas\n\n").unwrap();

        anotar_reserva(&ruta, &reserva("x.md", "urgente, revisar antes del martes")).unwrap();
        anotar_reserva(&ruta, &reserva("x.md", "urgente")).unwrap();

        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert!(
            texto.contains(
                "- Programator, 2026-11-13 15:40: x.md — urgente, revisar antes del martes"
            ),
            "no puede perder la primera reserva"
        );
        assert!(
            texto
                .lines()
                .any(|l| l == "- Programator, 2026-11-13 15:40: x.md — urgente"),
            "debe añadir la segunda reserva"
        );
        let count = texto
            .lines()
            .filter(|l| l.starts_with("- Programator, 2026-11-13 15:40: x.md"))
            .count();
        assert_eq!(count, 2, "debe haber exactamente 2 reservas distintas");
    }

    #[test]
    fn la_deduplicacion_exacta_sigue_funcionando() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(&ruta, "# Estado\n\n## Reservas activas\n\n").unwrap();

        anotar_reserva(&ruta, &reserva("x.md", "urgente")).unwrap();
        anotar_reserva(&ruta, &reserva("x.md", "urgente")).unwrap();

        let texto = std::fs::read_to_string(&ruta).unwrap();
        let count = texto
            .lines()
            .filter(|l| *l == "- Programator, 2026-11-13 15:40: x.md — urgente")
            .count();
        assert_eq!(count, 1, "la deduplicación exacta debe seguir funcionando");
    }

    const PLANTILLA_DE_GESTOR: &str = "# Estado\n\n## Papeles\n\n- Jefe de proyecto: Claude 1.\n\n\
        ## Reservas activas\n\nNinguna.\n\n## Mutaciones vivas\n\nNinguna.\n";

    #[test]
    fn anotar_quita_el_ninguna_de_su_seccion_y_no_el_de_las_demas() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(&ruta, PLANTILLA_DE_GESTOR).unwrap();

        let linea = anotar_reserva(&ruta, &reserva("x.md", "motivo")).unwrap();

        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert_eq!(
            texto,
            format!(
                "# Estado\n\n## Papeles\n\n- Jefe de proyecto: Claude 1.\n\n\
                 ## Reservas activas\n\n{linea}\n\n## Mutaciones vivas\n\nNinguna.\n"
            )
        );
    }

    #[test]
    fn retirar_deja_la_seccion_como_la_plantilla() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(&ruta, PLANTILLA_DE_GESTOR).unwrap();
        let linea = anotar_reserva(&ruta, &reserva("x.md", "motivo")).unwrap();

        retirar_reserva(&ruta, &linea).unwrap();

        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), PLANTILLA_DE_GESTOR);
    }

    #[test]
    fn retirar_deja_las_reservas_de_otros() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(
            &ruta,
            "## Reservas activas\n\n- Codex, 2026-11-13 10:00: a.rs — algo\n",
        )
        .unwrap();
        let linea = anotar_reserva(&ruta, &reserva("x.md", "motivo")).unwrap();

        retirar_reserva(&ruta, &linea).unwrap();

        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert!(texto.contains("- Codex, 2026-11-13 10:00: a.rs — algo"));
        assert!(!texto.contains("x.md"));
        assert!(!texto.contains(SIN_RESERVAS));
    }

    #[test]
    fn retirar_lo_que_no_esta_no_toca_nada() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("estado.md");
        std::fs::write(&ruta, PLANTILLA_DE_GESTOR).unwrap();

        retirar_reserva(&ruta, "- Programator, 2026-11-13 15:40: x.md — motivo").unwrap();
        retirar_reserva(&dir.path().join("no.md"), "- algo").unwrap();

        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), PLANTILLA_DE_GESTOR);
        assert!(!dir.path().join("no.md").exists());
    }

    #[test]
    fn el_ninguna_antiguo_se_quita_al_anotar() {
        let texto = con_la_linea(
            "## Reservas activas\n\n- *(ninguna)*\n\n_Nota._\n",
            "- yo, 2026-11-13 15:40: a — b",
        );

        assert!(!texto.contains("(ninguna)"));
        assert_eq!(
            lineas_de_la_seccion(&texto),
            ["- yo, 2026-11-13 15:40: a — b"]
        );
    }

    #[test]
    fn el_ninguna_no_es_una_reserva() {
        assert!(lineas_de_la_seccion(PLANTILLA_DE_GESTOR).is_empty());
        assert!(lineas_de_la_seccion("## Reservas activas\n\n- *(ninguna)*\n").is_empty());
    }
}
