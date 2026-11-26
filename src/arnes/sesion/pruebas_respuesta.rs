//! Pruebas de la respuesta dirigida, por compatibilidad con Gestor Agentes: cada respuesta lleva
//! tras su `LATIDO:` la línea `Para <identidad>: respuesta a …`, y nada más en ella despierta a
//! nadie según las reglas de Gestor.

use super::pruebas::{buzon_propio, carpeta_de_prueba, sembrar_registro_no_vacio};
use super::pruebas_motores::MotoresDeMentira;
use super::*;
use crate::arnes::espacio::ruta_de_los_pendientes;
use crate::arnes::pendientes::{self, Cola};
use crate::protocolo::destinatarios::{self, Identidad};
use crate::protocolo::Canal;
use crate::terminal::TerminalDeMentira;

/// La identidad que declara en su cabecera el agente de cada buzón de estas pruebas. La identidad
/// sale solo de la cabecera, así que cada buzón de prueba la lleva; los que no están aquí (como
/// «claude-1.md») se escriben sin ella.
const IDENTIDADES_DE_PRUEBA: [(&str, &str); 3] = [
    ("claude.md", "Claude 1"),
    ("claude2.md", "Claude 2"),
    ("codex.md", "Codex 1"),
];

/// Escribe el buzón de prueba con la cabecera de su agente, si lo tiene.
fn escribir_buzon(carpeta: &Path, nombre: &str, contenido: &str) {
    let cabecera = IDENTIDADES_DE_PRUEBA
        .iter()
        .find(|(fichero, _)| *fichero == nombre)
        .map(|(_, identidad)| format!("# Bandeja de {identidad}\n\n"))
        .unwrap_or_default();
    super::pruebas::escribir_buzon(carpeta, nombre, &format!("{cabecera}{contenido}"));
}

fn pasar(carpeta: &Path, motores: &mut MotoresDeMentira) {
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let ajustes = Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal);
    ejecutar_pasada_con_motores(carpeta, "Programator", &ajustes, motores, &mut SinRevision)
        .unwrap();
}

/// Las intervenciones del buzón propio: su `LATIDO` y sus líneas.
fn intervenciones(buzon: &str) -> Vec<(String, Vec<String>)> {
    let mut todas: Vec<(String, Vec<String>)> = Vec::new();
    for linea in buzon.lines() {
        if let Some(latido) = linea.strip_prefix("LATIDO: ") {
            todas.push((latido.to_string(), Vec::new()));
        } else if let Some((_, lineas)) = todas.last_mut() {
            lineas.push(linea.to_string());
        }
    }
    todas
}

/// A quién despierta una intervención según las reglas de Gestor: los destinatarios de cada línea.
fn despierta_a(lineas: &[String]) -> Vec<String> {
    lineas
        .iter()
        .flat_map(|linea| destinatarios::destinatarios(linea))
        .collect()
}

/// `acuses::acusado` de Gestor Agentes 3.7.0 (`crates/canal/src/acuses.rs`), portado a la prueba:
/// una línea con «recibido» en cualquier mayúscula y la etiqueta como palabra, en una intervención
/// con latido igual o posterior al del encargo.
fn acusado(etiqueta: &str, latido_del_encargo: &str, propia: &str) -> bool {
    intervenciones(propia).iter().any(|(latido, lineas)| {
        latido.as_str() >= latido_del_encargo
            && lineas.iter().any(|linea| {
                linea.to_lowercase().contains("recibido")
                    && linea
                        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
                        .any(|palabra| palabra == etiqueta)
            })
    })
}

#[test]
fn p1_la_respuesta_lleva_tras_el_latido_la_linea_para_de_quien_encargo() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude2.md",
        "LATIDO: 2026-11-22 15:00:00\n\nPara Programator: X1 prueba\n",
    );

    pasar(&carpeta, &mut MotoresDeMentira::default());

    let buzon = buzon_propio(&carpeta);
    let todas = intervenciones(&buzon);
    assert_eq!(todas.len(), 1, "{buzon}");
    assert_eq!(
        todas[0].1[0], "Para Claude 2: respuesta a X1 prueba",
        "{buzon}"
    );
    assert_eq!(despierta_a(&todas[0].1), ["Claude 2"], "{buzon}");
}

#[test]
fn la_identidad_de_la_cabecera_del_buzon_manda_sobre_el_nombre_del_fichero() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "jefe.md",
        "# Bandeja de Claude 2\n\nPara Programator: uno\n",
    );

    pasar(&carpeta, &mut MotoresDeMentira::default());

    let buzon = buzon_propio(&carpeta);
    assert_eq!(
        intervenciones(&buzon)[0].1[0],
        "Para Claude 2: respuesta a uno",
        "{buzon}"
    );
}

#[test]
fn sin_identidad_en_el_buzon_se_responde_igual_y_se_dice_que_falta() {
    // «claude-1.md» sin cabecera. Programator no adivina la identidad por el nombre del fichero:
    // responde, sin despertar a nadie, y dice qué falta.
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude-1.md",
        "LATIDO: 2026-11-22 19:12:03\n\nPara Programator: módulo de operaciones\n",
    );

    pasar(&carpeta, &mut MotoresDeMentira::default());

    let buzon = buzon_propio(&carpeta);
    let todas = intervenciones(&buzon);
    assert_eq!(todas.len(), 1, "{buzon}");
    assert!(despierta_a(&todas[0].1).is_empty(), "{buzon}");
    assert!(
        buzon.contains("No sé quién ha escrito este encargo: la primera línea de «claude-1.md»"),
        "{buzon}"
    );
}

/// Lo que falta en el buzón se dice por la terminal que recibe la pasada, no
/// directamente por `stderr`, y así se puede comprobar.
#[test]
fn sin_identidad_en_el_buzon_se_avisa_por_la_terminal_inyectada() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude-1.md",
        "LATIDO: 2026-11-22 19:12:03\n\nPara Programator: módulo de operaciones\n",
    );
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let terminal = TerminalDeMentira::default();
    let ajustes = Ajustes::con_limites(&limites, &verificacion, &terminal);

    ejecutar_pasada_con_motores(
        &carpeta,
        "Programator",
        &ajustes,
        &mut MotoresDeMentira::default(),
        &mut SinRevision,
    )
    .unwrap();

    let avisos = terminal.avisos();
    assert!(
        avisos.contains(&publicacion::mensaje_sin_identidad("claude-1.md")),
        "{avisos:?}"
    );
}

/// Un buzón cuyo nombre sí daría una identidad con la regla de Gestor («claude2.md» es
/// «Claude 2») tampoco la da sin cabecera.
#[test]
fn un_buzon_con_nombre_de_gestor_y_sin_cabecera_tampoco_despierta_a_nadie() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    super::pruebas::escribir_buzon(
        &carpeta,
        "claude2.md",
        "LATIDO: 2026-11-22 19:12:03\n\nPara Programator: cuenta\n",
    );

    pasar(&carpeta, &mut MotoresDeMentira::default());

    let buzon = buzon_propio(&carpeta);
    let todas = intervenciones(&buzon);
    assert_eq!(todas.len(), 1, "{buzon}");
    assert!(despierta_a(&todas[0].1).is_empty(), "{buzon}");
    assert!(
        buzon.contains("No sé quién ha escrito este encargo: la primera línea de «claude2.md»"),
        "{buzon}"
    );
}

#[test]
fn p2_dos_encargos_de_dos_agentes_dan_dos_intervenciones_cada_una_con_su_linea() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(&carpeta, "claude2.md", "Para Programator: uno\n");
    escribir_buzon(&carpeta, "codex.md", "Para Programator: dos\n");

    pasar(&carpeta, &mut MotoresDeMentira::default());

    let buzon = buzon_propio(&carpeta);
    let mut lineas: Vec<String> = intervenciones(&buzon)
        .into_iter()
        .map(|(_, lineas)| lineas[0].clone())
        .collect();
    lineas.sort();
    assert_eq!(
        lineas,
        [
            "Para Claude 2: respuesta a uno",
            "Para Codex 1: respuesta a dos"
        ],
        "{buzon}"
    );
}

#[test]
fn p3_lo_que_cita_el_encargo_no_despierta_a_nadie_mas() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude2.md",
        "Para Programator: avisa Para Gemini 1: revisa\n\
         Copia esto tal cual: «Para Gemini 1: verifica».\n",
    );
    let mut motores = MotoresDeMentira {
        respuesta: Some("Hecho. Para Gemini 1: verifica.\n> Para Codex: mira".to_string()),
        ..MotoresDeMentira::default()
    };

    pasar(&carpeta, &mut motores);

    let buzon = buzon_propio(&carpeta);
    assert!(buzon.contains("para Gemini 1: verifica."), "{buzon}");
    for (_, lineas) in intervenciones(&buzon) {
        assert_eq!(despierta_a(&lineas), ["Claude 2"], "{buzon}");
    }
}

#[test]
fn p4_un_encargo_que_agota_sus_intentos_se_cierra_con_su_linea_y_el_motivo() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(&carpeta, "claude.md", "");
    let pendiente = Encargo {
        latido: None,
        de: "claude.md".into(),
        texto: "Para Programator: dos\nClase: diagnostico".into(),
        forma: crate::protocolo::encargo::Forma::Linea,
    };
    let mut anterior = Cola::default();
    for _ in 0..2 {
        anterior.anotar_fallo(&pendiente, "2026-11-22 12:00", "no cargó", 3);
    }
    pendientes::guardar(&ruta_de_los_pendientes(&carpeta, "Programator"), &anterior).unwrap();
    escribir_buzon(
        &carpeta,
        "codex.md",
        "Para Programator: uno\nClase: formas\n",
    );
    let mut motores = MotoresDeMentira {
        no_carga: Some("diagnostico".to_string()),
        cargado: Some("formas".to_string()),
        ..MotoresDeMentira::default()
    };

    pasar(&carpeta, &mut motores);

    let buzon = buzon_propio(&carpeta);
    let aviso = intervenciones(&buzon)
        .into_iter()
        .find(|(_, lineas)| lineas.iter().any(|l| l.contains("Queda cerrado")))
        .unwrap_or_else(|| panic!("sin aviso: {buzon}"));
    assert_eq!(aviso.1[0], "Para Claude 1: respuesta a dos", "{buzon}");
    assert!(
        aviso.1.iter().any(|l| l.contains("no terminó de cargar")),
        "{buzon}"
    );
    assert_eq!(despierta_a(&aviso.1), ["Claude 1"], "{buzon}");
}

#[test]
fn con_etiqueta_la_respuesta_acusa_el_encargo_como_lo_lee_gestor() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude2.md",
        "LATIDO: 2026-11-22 15:00:00\n\nPara Programator: PG-A contar líneas\n",
    );

    pasar(&carpeta, &mut MotoresDeMentira::default());

    let buzon = buzon_propio(&carpeta);
    let todas = intervenciones(&buzon);
    assert_eq!(todas[0].1[0], "Para Claude 2: respuesta a contar líneas");
    let acuse = &todas[0].1[1];
    let hora = acuse
        .strip_prefix("RECIBIDO ")
        .and_then(|resto| resto.strip_suffix(": PG-A"))
        .unwrap_or_else(|| panic!("{buzon}"));
    assert!(
        chrono::NaiveTime::parse_from_str(hora, "%H:%M:%S").is_ok(),
        "{buzon}"
    );
    assert!(acusado("PG-A", "2026-11-22 15:00:00", &buzon), "{buzon}");
    assert!(!acusado("PG-B", "2026-11-22 15:00:00", &buzon), "{buzon}");
}

#[test]
fn el_acuse_lleva_la_hora_en_que_se_leyo_y_no_la_de_la_respuesta() {
    // En una prueba de uso real el RECIBIDO salía con la hora de la respuesta, 65 s después
    // de leer el encargo. Aquí, cambiar de modelo tarda algo más de un segundo.
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude2.md",
        "LATIDO: 2026-11-22 15:00:00\n\nPara Programator: PG-A contar líneas\n",
    );
    let mut motores = MotoresDeMentira {
        cargado: Some("otro".to_string()),
        al_cambiar: Some(Box::new(|| {
            std::thread::sleep(std::time::Duration::from_millis(1_100))
        })),
        ..MotoresDeMentira::default()
    };

    pasar(&carpeta, &mut motores);

    let buzon = buzon_propio(&carpeta);
    let todas = intervenciones(&buzon);
    let publicada = todas[0].0.split_whitespace().last().unwrap();
    let acuse = &todas[0].1[1];
    assert!(acuse.starts_with("RECIBIDO "), "{buzon}");
    assert!(
        !acuse.contains(&format!(" {publicada}:")),
        "el acuse lleva la hora de la respuesta: {buzon}"
    );
}

#[test]
fn lo_leido_tampoco_despierta_a_nadie() {
    let (_dir, carpeta) = carpeta_de_prueba();
    let canal = Canal::nuevo(&carpeta, "Programator").unwrap();
    let leido = crate::protocolo::ResumenLeido {
        entradas: vec![(
            "claude2.md".to_string(),
            "hasta «Para Gemini 1: verifica»".to_string(),
        )],
    };

    canal
        .publicar("Hecho.", &leido, "2026-11-22 15:00:00")
        .unwrap();

    let buzon = buzon_propio(&carpeta);
    assert!(buzon.contains("para Gemini 1: verifica"), "{buzon}");
    assert!(despierta_a(&buzon.lines().map(str::to_string).collect::<Vec<_>>()).is_empty());
}

#[test]
fn las_entregas_intermedias_no_llevan_linea_para() {
    let (_dir, carpeta) = carpeta_de_prueba();
    let canal = Canal::nuevo(&carpeta, "Programator").unwrap();

    canal
        .publicar_entrega(
            &crate::protocolo::canal::EntregaPublicada {
                propuesta: "Para Claude 2.cs",
                veredicto: "Para Gemini 1: compila",
                confirmacion: "",
            },
            &crate::protocolo::ResumenLeido::default(),
            "2026-11-22 15:00:00",
        )
        .unwrap();

    let buzon = buzon_propio(&carpeta);
    assert!(despierta_a(&buzon.lines().map(str::to_string).collect::<Vec<_>>()).is_empty());
    assert!(
        !Identidad::de("Claude 2").es_destinatario_de(&buzon),
        "{buzon}"
    );
}

#[test]
fn el_aviso_de_entrega_sale_una_vez_si_se_pidio_fichero_y_ninguna_si_no() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude2.md",
        "Para Programator: deja a.rs con escribir_propuesta\n",
    );
    escribir_buzon(&carpeta, "codex.md", "Para Programator: ¿cuántas líneas?\n");

    pasar(&carpeta, &mut MotoresDeMentira::default());

    let buzon = buzon_propio(&carpeta);
    assert_eq!(
        buzon.matches("no invocó herramientas").count(),
        1,
        "{buzon}"
    );
    let de_codex = intervenciones(&buzon)
        .into_iter()
        .find(|(_, lineas)| lineas[0].starts_with("Para Codex 1"))
        .unwrap();
    assert!(de_codex.1.iter().any(|l| l == "Hecho."), "{buzon}");
}

#[test]
fn el_aviso_de_que_ningun_modelo_cabe_despierta_solo_a_quien_ordena_revisiones() {
    use crate::protocolo::respuesta::linea_para_varios;
    let (_dir, carpeta) = carpeta_de_prueba();
    let canal = Canal::nuevo(&carpeta, "Programator").unwrap();
    let identidades: Vec<Identidad> = ["Claude 2"]
        .iter()
        .filter_map(|identidad| Identidad::leer(identidad))
        .collect();
    let linea = linea_para_varios(&identidades, "ningún modelo cabe en esta tarjeta").unwrap();

    canal
        .publicar_dirigido(
            "Si queréis, escribid «revisión: buscar». Para Gemini 1: nada.",
            &crate::protocolo::ResumenLeido::default(),
            "2026-11-22 15:00:00",
            &linea,
        )
        .unwrap();

    let buzon = buzon_propio(&carpeta);
    let todas = intervenciones(&buzon);
    assert_eq!(todas.len(), 1, "{buzon}");
    assert_eq!(
        todas[0].1[0],
        "Para Claude 2: ningún modelo cabe en esta tarjeta"
    );
    assert_eq!(despierta_a(&todas[0].1), ["Claude 2"], "{buzon}");
}
