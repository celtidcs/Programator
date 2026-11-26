//! Pruebas de la pasada con el registro de encargos: qué es nuevo y qué no cuando cambia el
//! buzón, un corte a mitad y el paso desde la 1.1.0 (criterios B1 a B4, B6 y B8 de la
//! especificación de la 1.2.0).

use super::pruebas::{carpeta_de_prueba, escribir_buzon, sembrar_registro_no_vacio};
use super::pruebas_motores::{cola, pasar, pasar_sin_unwrap, MotoresDeMentira};
use super::*;
use crate::arnes::espacio::{carpeta_de_los_encargos, ruta_del_registro};
use crate::arnes::registro_de_encargos::disco;
use crate::protocolo::{detectar, RegistroLectura};

/// Cuántos encargos atendió la pasada.
fn atendidos(desenlace: DesenlacePasada) -> usize {
    match desenlace {
        DesenlacePasada::Atendidos(resumen) => resumen.atendidos,
        DesenlacePasada::SinEncargos => 0,
        otro => panic!("se esperaba Atendidos o SinEncargos: {otro:?}"),
    }
}

/// Un canal con la primera pasada ya hecha y un buzón de Codex con `texto`.
fn canal_con(texto: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let (carpeta_temporal, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(&carpeta, "codex.md", texto);
    (carpeta_temporal, carpeta)
}

const UNO: &str = "LATIDO: 2026-11-25 10:00:00\n- Nota previa.\n- Para Programator: uno\n";

/// B1.
#[test]
fn cambiar_lf_por_crlf_en_un_buzon_ya_atendido_no_reatiende_nada() {
    let (_dir, carpeta) = canal_con(UNO);
    let mut motores = MotoresDeMentira::default();
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);

    escribir_buzon(&carpeta, "codex.md", &UNO.replace('\n', "\r\n"));

    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);
}

/// B2: lo de alrededor no cuenta; el texto del encargo sí.
#[test]
fn editar_lo_anterior_no_reatiende_y_editar_el_encargo_lo_hace_nuevo() {
    let (_dir, carpeta) = canal_con(UNO);
    let mut motores = MotoresDeMentira::default();
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);

    escribir_buzon(
        &carpeta,
        "codex.md",
        &UNO.replace("Nota previa", "Nota corregida a mano"),
    );
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);

    escribir_buzon(
        &carpeta,
        "codex.md",
        &UNO.replace(": uno", ": uno, corregido"),
    );
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);
}

/// Uso real del 26/11: la segunda respuesta a un encargo retocado dice que lo es. La primera, no.
#[test]
fn la_respuesta_a_un_encargo_retocado_dice_que_es_la_segunda_atencion() {
    let encargo = "LATIDO: 2026-11-25 10:00:00\nPara Programator: FECHAS-1 módulo\n\
                   escribe una línea por requisito\n";
    let (_dir, carpeta) = canal_con(encargo);
    let mut motores = MotoresDeMentira::default();
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);
    let primera = super::pruebas::buzon_propio(&carpeta);
    assert!(!primera.contains("Segunda atención"), "{primera}");

    escribir_buzon(
        &carpeta,
        "codex.md",
        &encargo.replace("una línea", "una frase"),
    );
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);

    let segunda = super::pruebas::buzon_propio(&carpeta);
    assert_eq!(segunda.matches("Segunda atención").count(), 1, "{segunda}");
}

/// B3: en la 1.1.0, lo añadido por encima del final ya leído se perdía.
#[test]
fn una_linea_para_programator_dentro_de_una_intervencion_ya_leida_se_atiende() {
    let antes = "LATIDO: 2026-11-25 10:00:00\n- Para Claude 1: revisa esto.\n\n\
                 LATIDO: 2026-11-25 11:00:00\n- Hecho lo mío.\n";
    let (_dir, carpeta) = canal_con(antes);
    let mut motores = MotoresDeMentira::default();
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);

    let despues = antes.replace(
        "- Para Claude 1: revisa esto.\n",
        "- Para Claude 1: revisa esto.\n- Para Programator: y tú esto otro.\n",
    );
    escribir_buzon(&carpeta, "codex.md", &despues);

    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);
}

/// B4.
#[test]
fn dos_encargos_iguales_en_intervenciones_distintas_se_atienden_los_dos() {
    let texto = "LATIDO: 2026-11-25 10:00:00\n- Para Programator: pasa las pruebas\n\n\
                 LATIDO: 2026-11-25 12:00:00\n- Para Programator: pasa las pruebas\n";
    let (_dir, carpeta) = canal_con(texto);
    let mut motores = MotoresDeMentira::default();

    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 2);
}

/// B6: el corte se simula con una carpeta en el sitio de la ficha, que no deja guardarla.
#[test]
fn un_corte_al_guardar_una_ficha_no_pierde_ni_duplica_el_encargo() {
    let (_dir, carpeta) = canal_con(UNO);
    let id = detectar("Programator", "codex.md", UNO)[0].id();
    let estorbo = disco::ruta_de_la_ficha(&carpeta_de_los_encargos(&carpeta, "Programator"), &id);
    std::fs::create_dir_all(&estorbo).unwrap();
    std::fs::write(estorbo.join("dentro"), "x").unwrap();
    let mut motores = MotoresDeMentira::default();

    assert!(pasar_sin_unwrap(&carpeta, &mut motores).is_err());

    std::fs::remove_dir_all(&estorbo).unwrap();
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);
    assert!(cola(&carpeta).is_empty());
}

/// Un canal que viene de la 1.1.0: Programator ya respondió, y `lectura.json` dice que leyó el
/// buzón de Codex tal como está ahora. No hay carpeta de encargos.
fn canal_de_la_1_1(buzon: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let (carpeta_temporal, carpeta) = carpeta_de_prueba();
    escribir_buzon(&carpeta, "codex.md", buzon);
    escribir_buzon(
        &carpeta,
        "programator.md",
        "LATIDO: 2026-11-20 10:00:01\nHecho.\n",
    );
    let mut lectura = RegistroLectura::default();
    lectura.delta("codex.md", buzon);
    lectura
        .guardar(&ruta_del_registro(&carpeta, "Programator"))
        .unwrap();
    (carpeta_temporal, carpeta)
}

const ATENDIDO_POR_LA_1_1: &str = "LATIDO: 2026-11-20 10:00:00\n- Para Programator: uno\n\n\
                                   LATIDO: 2026-11-21 10:00:00\n- Para Programator: dos\n";

/// B8.
#[test]
fn un_canal_de_la_1_1_ya_atendido_no_vuelve_a_atender_nada() {
    let (_dir, carpeta) = canal_de_la_1_1(ATENDIDO_POR_LA_1_1);
    let mut motores = MotoresDeMentira::default();

    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);
    // Y cuando cambia el buzón, tampoco: lo antiguo quedó como visto.
    escribir_buzon(
        &carpeta,
        "codex.md",
        &format!("{ATENDIDO_POR_LA_1_1}\nLATIDO: 2026-11-25 09:00:00\n- Sin encargos.\n"),
    );
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);
}

/// B8: lo escrito después del desplazamiento guardado sí se atiende.
#[test]
fn en_un_canal_de_la_1_1_lo_escrito_despues_de_lo_leido_se_atiende() {
    let (_dir, carpeta) = canal_de_la_1_1(ATENDIDO_POR_LA_1_1);
    escribir_buzon(
        &carpeta,
        "codex.md",
        &format!("{ATENDIDO_POR_LA_1_1}\nLATIDO: 2026-11-25 09:00:00\n- Para Programator: tres\n"),
    );
    let mut motores = MotoresDeMentira::default();

    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);
}

/// Lo que ya tiene ficha, esté como esté, no es nuevo; un buzón sin cambios no se mira.
#[test]
fn es_nuevo_solo_lo_que_no_tiene_ficha_en_los_buzones_que_cambiaron() {
    use super::reparto::encargos_nuevos;
    use crate::arnes::registro_de_encargos::{EstadoDelEncargo, RegistroDeEncargos};
    use crate::protocolo::Delta;

    let texto = "LATIDO: 2026-11-25 10:00:00\n- Para Programator: uno\n\n\
                 LATIDO: 2026-11-25 11:00:00\n- Para Programator: dos\n";
    let buzones = vec![
        ("codex.md".to_string(), texto.to_string()),
        ("quieto.md".to_string(), texto.to_string()),
    ];
    let deltas = vec![
        ("codex.md".to_string(), Delta::Completo(texto.to_string())),
        ("quieto.md".to_string(), Delta::Nada),
    ];
    let mut registro = RegistroDeEncargos::default();
    let uno = detectar("Programator", "codex.md", texto).remove(0);
    registro.anotar(&uno, "f", EstadoDelEncargo::Atendido);

    let nuevos = encargos_nuevos("Programator", &buzones, &deltas, &registro);

    let textos: Vec<_> = nuevos
        .iter()
        .map(|e| (e.de.as_str(), e.texto.as_str()))
        .collect();
    assert_eq!(textos, [("codex.md", "- Para Programator: dos")]);
}

/// B7: el agente pasa a su histórico un encargo ya atendido; su ficha sale del registro, y el
/// encargo no se atiende otra vez.
#[test]
fn lo_que_el_agente_poda_de_su_buzon_sale_del_registro() {
    let (_dir, carpeta) = canal_con(UNO);
    let mut motores = MotoresDeMentira::default();
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);
    let carpeta_encargos = carpeta_de_los_encargos(&carpeta, "Programator");
    assert_eq!(disco::leer(&carpeta_encargos).unwrap().fichas.len(), 1);

    escribir_buzon(
        &carpeta,
        "codex.md",
        "LATIDO: 2026-11-25 12:00:00\n- Lo anterior, al histórico.\n",
    );

    // Con plazo 0, la primera pasada que no lo ve solo lo marca; la siguiente lo olvida.
    assert_eq!(atendidos(pasar_olvidando_en(&carpeta, &mut motores, 0)), 0);
    assert_eq!(disco::leer(&carpeta_encargos).unwrap().fichas.len(), 1);
    assert_eq!(atendidos(pasar_olvidando_en(&carpeta, &mut motores, 0)), 0);
    assert!(disco::leer(&carpeta_encargos).unwrap().fichas.is_empty());
}

/// Una pasada con `[ciclo] olvidar_lo_cerrado_tras_minutos = minutos`.
fn pasar_olvidando_en(
    carpeta: &std::path::Path,
    motores: &mut MotoresDeMentira,
    minutos: u64,
) -> DesenlacePasada {
    pasar_ajustando(carpeta, motores, |ajustes| {
        ajustes.olvidar_lo_cerrado_tras_minutos = minutos;
    })
}

/// Una pasada con los ajustes de las pruebas, cambiados por `cambiar`.
fn pasar_ajustando(
    carpeta: &std::path::Path,
    motores: &mut MotoresDeMentira,
    cambiar: impl FnOnce(&mut Ajustes),
) -> DesenlacePasada {
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let mut ajustes = Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal);
    cambiar(&mut ajustes);
    ejecutar_pasada_con_motores(
        carpeta,
        "Programator",
        &ajustes,
        motores,
        &mut super::revisor::SinRevision,
    )
    .unwrap()
}

// Con el plazo de fábrica, un buzón leído vacío, ausente o cortado una pasada no hace olvidar
// lo ya atendido.

/// Un buzón leído vacío un momento (el agente lo reescribe a medias o se sincroniza) y después
/// entero otra vez: lo ya atendido no se vuelve a atender.
#[test]
fn c1_adv_un_buzon_vacio_un_momento_no_reatiende_lo_ya_atendido() {
    let (_dir, carpeta) = canal_con(UNO);
    let mut motores = MotoresDeMentira::default();
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);

    escribir_buzon(&carpeta, "codex.md", "");
    let _ = pasar_sin_unwrap(&carpeta, &mut motores);
    escribir_buzon(&carpeta, "codex.md", UNO);

    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);
}

/// Un buzón que desaparece un momento del canal y vuelve igual.
#[test]
fn c1_adv_un_buzon_que_falta_un_momento_no_reatiende_lo_ya_atendido() {
    let (_dir, carpeta) = canal_con(UNO);
    let mut motores = MotoresDeMentira::default();
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);

    std::fs::remove_file(carpeta.join(".gestor/canal/codex.md")).unwrap();
    let _ = pasar_sin_unwrap(&carpeta, &mut motores);
    escribir_buzon(&carpeta, "codex.md", UNO);

    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);
}

/// Un buzón leído a medias (cortado dentro de la línea del encargo) y después entero.
#[test]
fn c1_adv_un_buzon_cortado_a_media_linea_no_reatiende_lo_ya_atendido() {
    let (_dir, carpeta) = canal_con(UNO);
    let mut motores = MotoresDeMentira::default();
    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);

    escribir_buzon(&carpeta, "codex.md", &UNO[..UNO.len() - 4]);
    let _ = pasar_sin_unwrap(&carpeta, &mut motores);
    escribir_buzon(&carpeta, "codex.md", UNO);

    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 0);
}

/// Pone la fecha de modificación de un buzón `atras` en el pasado.
fn envejecer_buzon(carpeta: &std::path::Path, nombre: &str, atras: std::time::Duration) {
    let fichero = std::fs::OpenOptions::new()
        .write(true)
        .open(carpeta.join(".gestor/canal").join(nombre))
        .unwrap();
    fichero
        .set_modified(std::time::SystemTime::now() - atras)
        .unwrap();
}

/// Un buzón leído a medio escribir (cortado dentro de la línea del encargo) no da un encargo
/// cortado: la pasada espera, y cuando el buzón lleva quieto el plazo se atiende uno, el entero.
#[test]
fn un_buzon_a_medio_escribir_espera_y_se_atiende_entero() {
    const ENTERO: &str =
        "LATIDO: 2026-11-25 10:00:00\n- Para Programator: haz la lista entera de pendientes\n";
    let corte = ENTERO.find("lista").unwrap();
    let (_dir, carpeta) = canal_con(&ENTERO[..corte]);
    let mut motores = MotoresDeMentira::default();
    let estable = |ajustes: &mut Ajustes| {
        ajustes.buzon_estable_tras = std::time::Duration::from_millis(1500);
    };

    assert!(matches!(
        pasar_ajustando(&carpeta, &mut motores, estable),
        DesenlacePasada::BuzonAMedias
    ));
    let carpeta_encargos = carpeta_de_los_encargos(&carpeta, "Programator");
    assert!(disco::leer(&carpeta_encargos).unwrap().fichas.is_empty());

    escribir_buzon(&carpeta, "codex.md", ENTERO);
    envejecer_buzon(&carpeta, "codex.md", std::time::Duration::from_secs(60));

    assert_eq!(
        atendidos(pasar_ajustando(&carpeta, &mut motores, estable)),
        1
    );
    let fichas = disco::leer(&carpeta_encargos).unwrap().fichas;
    assert_eq!(fichas.len(), 1);
    assert!(fichas[0].texto.contains("entera"), "{:?}", fichas[0].texto);
}

/// Solo espera el buzón a medio escribir: el de otro agente, quieto, se atiende en la misma
/// pasada, y el resumen dice que queda uno para la vuelta siguiente (lo pidió Claude 1: un agente
/// que escribe sin parar no puede dejar a los demás sin atender).
#[test]
fn un_buzon_a_medias_no_frena_a_los_demas() {
    const CODEX: &str = "LATIDO: 2026-11-25 10:00:00\n- Para Programator: lo de Codex entero\n";
    const GEMINI: &str = "LATIDO: 2026-11-25 10:00:00\n- Para Programator: lo de Gemini\n";
    let corte = CODEX.find("entero").unwrap();
    let (_dir, carpeta) = canal_con(&CODEX[..corte]);
    escribir_buzon(&carpeta, "gemini.md", GEMINI);
    envejecer_buzon(&carpeta, "gemini.md", std::time::Duration::from_secs(60));
    let mut motores = MotoresDeMentira::default();
    let estable = |ajustes: &mut Ajustes| {
        ajustes.buzon_estable_tras = std::time::Duration::from_millis(1500);
    };

    match pasar_ajustando(&carpeta, &mut motores, estable) {
        DesenlacePasada::Atendidos(resumen) => {
            assert_eq!(resumen.atendidos, 1);
            assert_eq!(resumen.buzones_a_medias, 1);
        }
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }

    escribir_buzon(&carpeta, "codex.md", CODEX);
    envejecer_buzon(&carpeta, "codex.md", std::time::Duration::from_secs(60));
    assert_eq!(
        atendidos(pasar_ajustando(&carpeta, &mut motores, estable)),
        1
    );
    let carpeta_encargos = carpeta_de_los_encargos(&carpeta, "Programator");
    let mut textos: Vec<String> = disco::leer(&carpeta_encargos)
        .unwrap()
        .fichas
        .into_iter()
        .map(|ficha| ficha.texto)
        .collect();
    textos.sort();
    assert_eq!(textos.len(), 2, "{textos:?}");
    assert!(textos[0].contains("Codex entero"), "{textos:?}");
    assert!(textos[1].contains("Gemini"), "{textos:?}");
}

/// Un `preparar:` lo atiende el arnés, sin el modelo. La propuesta se copia con el
/// proyecto, la respuesta lo cuenta en el canal y el proyecto queda como estaba (F1).
#[test]
fn un_preparar_lo_atiende_el_arnes_en_una_copia() {
    use crate::arnes::entregas::{
        huella, EntradaDeEntrega, EstadoDeLaPropuesta, RegistroDeEntregas, VeredictoDeLaEntrega,
        LO_DIO_PROGRAMATOR,
    };
    let (_dir, carpeta) = canal_con("");
    let candidatos = carpeta.join(".gestor/candidatos/programator");
    std::fs::create_dir_all(&candidatos).unwrap();
    std::fs::write(candidatos.join("x-002.py"), "print(2)\n").unwrap();
    std::fs::write(carpeta.join("x.py"), "print(1)\n").unwrap();
    let registro = RegistroDeEntregas {
        entregas: vec![EntradaDeEntrega {
            propuesta: "x-002.py".to_string(),
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: "codex.md".to_string(),
            modelo: None,
            fecha: "2026-11-25 10:00".to_string(),
            veredicto: VeredictoDeLaEntrega::Pasa,
            huella: huella("print(2)\n"),
            estado: EstadoDeLaPropuesta::Propuesta,
            estado_lo_dio: LO_DIO_PROGRAMATOR.to_string(),
            destino: None,
            motivo: None,
            archivada: None,
            ya_en_el_proyecto: Vec::new(),
        }],
        ..RegistroDeEntregas::default()
    };
    crate::arnes::entregas::disco::guardar(
        &crate::arnes::espacio::ruta_de_las_entregas(&carpeta, "Programator"),
        &registro,
    )
    .unwrap();
    escribir_buzon(
        &carpeta,
        "codex.md",
        "LATIDO: 2026-11-25 10:00:00\n- Para Programator: preparar: `x-002.py` en `x.py`\n",
    );
    let mut motores = MotoresDeMentira::default();

    assert_eq!(atendidos(pasar(&carpeta, &mut motores)), 1);

    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(buzon.contains("Preparada «x-002.py» en «x.py»"), "{buzon}");
    assert!(buzon.contains("- print(1)"), "{buzon}");
    assert!(buzon.contains("+ print(2)"), "{buzon}");
    assert_eq!(
        std::fs::read_to_string(carpeta.join("x.py")).unwrap(),
        "print(1)\n"
    );
    assert!(motores.cargas.is_empty(), "el modelo no interviene");
}

/// F5 en la pasada: cada pasada borra las copias que sobran, aunque no haya encargos.
#[test]
fn cada_pasada_borra_las_copias_que_sobran() {
    let (_dir, carpeta) = canal_con("");
    let copias = carpeta.join(crate::config::preparacion::CARPETA_POR_DEFECTO);
    std::fs::create_dir_all(copias.join("sin-marca/proyecto")).unwrap();
    let mut motores = MotoresDeMentira::default();

    let _ = pasar_sin_unwrap(&carpeta, &mut motores).unwrap();

    assert!(!copias.join("sin-marca").exists());
}
