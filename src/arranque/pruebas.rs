//! Pruebas unitarias de `arranque`: sondeo de novedades en el canal, composición y publicación de normas
//! y guía para el equipo y redacción de avisos explicativos de consola.

use super::*;
use crate::error::Error;
use crate::plataforma::{EJEMPLO_DE_RUTA, NOMBRE_DEL_EJECUTABLE};

/// El fragmento de desempeño no es lo que estas pruebas examinan, así que va uno fijo, vacío de
/// marcadores.
const DESEMPENO_VACIO: &str = "CIFRAS";

#[test]
fn la_guia_para_el_equipo_se_actualiza_en_cada_arranque() {
    // Al revés que las normas: una guía vieja es peor que ninguna, porque dice qué sabe hacer
    // el modelo y dónde miente, y eso cambia con cada versión y con cada modelo que se ponga
    // detrás. Se lee como vigente aunque no lo sea.
    let dir = tempfile::tempdir().unwrap();
    let plantilla = dir.path().join("guia.md");
    let canal = tempfile::tempdir().unwrap();
    let comprobadores = crate::arnes::verificacion::Comprobadores::new();

    std::fs::write(&plantilla, "VERSION VIEJA").unwrap();
    publicar_guia(
        canal.path(),
        &plantilla,
        "GUIA.md",
        16384,
        DESEMPENO_VACIO,
        &comprobadores,
    )
    .unwrap();

    std::fs::write(&plantilla, "VERSION NUEVA").unwrap();
    publicar_guia(
        canal.path(),
        &plantilla,
        "GUIA.md",
        16384,
        DESEMPENO_VACIO,
        &comprobadores,
    )
    .unwrap();

    let texto = std::fs::read_to_string(canal.path().join("GUIA.md")).unwrap();
    assert_eq!(
        texto, "VERSION NUEVA",
        "la guía no puede quedarse anticuada"
    );
}

#[test]
fn la_guia_publica_el_contexto_en_vigor_y_no_una_cifra_escrita_a_mano() {
    // En uso real la guía decía 16.384 mientras el TOML declaraba 32.768. Las dos cifras eran
    // ciertas —una era el defecto y la otra el ajuste local— y ninguna decía cuál mandaba.
    let plantilla_dir = tempfile::tempdir().unwrap();
    let plantilla = plantilla_dir.path().join("guia.md");
    std::fs::write(&plantilla, "ventana de {contexto} tokens").unwrap();
    let canal = tempfile::tempdir().unwrap();
    let comprobadores = crate::arnes::verificacion::Comprobadores::new();

    publicar_guia(
        canal.path(),
        &plantilla,
        "GUIA.md",
        32768,
        DESEMPENO_VACIO,
        &comprobadores,
    )
    .unwrap();

    let publicada = std::fs::read_to_string(canal.path().join("GUIA.md")).unwrap();
    assert_eq!(publicada, "ventana de 32768 tokens");
}

#[test]
fn la_guia_compone_el_repertorio_el_desempeno_y_los_comprobadores() {
    // El defecto que cierra esta tarea: la guía real le decía al modelo dirigido que carecía de
    // herramientas, cuando `publicar_guia` ya podía interpolar las tres cosas que lo desmienten.
    let dir = tempfile::tempdir().unwrap();
    let plantilla = dir.path().join("guia.md");
    std::fs::write(&plantilla, "{repertorio}\n{desempeno}\n{comprobadores}").unwrap();
    let desempeno = "27 de 41 auditando formas";
    let canal = tempfile::tempdir().unwrap();
    let mut comprobadores = crate::arnes::verificacion::Comprobadores::new();
    comprobadores.insert("py".to_string(), vec!["python".to_string()]);

    publicar_guia(
        canal.path(),
        &plantilla,
        "GUIA.md",
        16384,
        desempeno,
        &comprobadores,
    )
    .unwrap();

    let publicada = std::fs::read_to_string(canal.path().join("GUIA.md")).unwrap();
    assert!(
        publicada.contains("`leer_fichero`"),
        "falta el repertorio:\n{publicada}"
    );
    assert!(
        publicada.contains("27 de 41 auditando formas"),
        "falta el desempeño:\n{publicada}"
    );
    assert!(
        publicada.contains("`.py`"),
        "faltan los comprobadores:\n{publicada}"
    );
}

#[test]
fn instalar_crea_las_carpetas_que_falten() {
    // El destino cuelga de «.gestor/<agente>/», que en un proyecto recién estrenado no existe.
    let destino_dir = tempfile::tempdir().unwrap();
    let destino = destino_dir
        .path()
        .join(".gestor")
        .join("programator")
        .join("PROGRAMATOR.md");

    let copiada = instalar_normas(&destino, "INSTRUCCIONES\n").unwrap();

    assert!(copiada);
    assert_eq!(
        std::fs::read_to_string(&destino).unwrap(),
        "INSTRUCCIONES\n"
    );
}

#[test]
fn no_sobrescribe_unas_normas_que_ya_estaban() {
    // Es lo que permite adaptar el fichero a un proyecto sin que el arnés lo revierta.
    let destino_dir = tempfile::tempdir().unwrap();
    let destino = destino_dir.path().join("PROGRAMATOR.md");
    std::fs::write(&destino, "LAS DE ANTES").unwrap();

    let copiada = instalar_normas(&destino, "LAS NUEVAS").unwrap();

    assert!(!copiada);
    assert_eq!(
        std::fs::read_to_string(&destino).unwrap(),
        "LAS DE ANTES",
        "el fichero del proyecto manda sobre la plantilla"
    );
}

#[test]
fn el_sondeo_detecta_que_un_buzon_ha_cambiado() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("claude.md"), "uno").unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    std::fs::write(dir.path().join("claude.md"), "uno y dos").unwrap();

    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::Novedades
    );
}

#[test]
fn el_sondeo_no_da_falsos_positivos_cuando_nada_cambia() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("claude.md"), "uno").unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::SinCambios
    );
}

#[test]
fn el_sondeo_detecta_un_buzon_nuevo() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("claude.md"), "uno").unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    std::fs::write(dir.path().join("gemini.md"), "nuevo agente").unwrap();

    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::Novedades
    );
}

#[test]
fn detecta_dos_escrituras_del_mismo_tamano_en_el_mismo_segundo() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("claude.md");
    std::fs::write(&ruta, "aaaaa").unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    // Mismo tamaño, contenido distinto, casi con toda seguridad dentro del mismo segundo: si
    // la huella dependiera de la fecha de modificación, esta escritura pasaría desapercibida.
    std::fs::write(&ruta, "bbbbb").unwrap();

    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::Novedades,
        "dos escrituras del mismo tamaño en el mismo segundo deben distinguirse por el hash \
         SHA-256 del contenido, que no depende de ninguna resolución de reloj"
    );
}

#[test]
fn un_fichero_nuevo_en_una_subcarpeta_se_detecta() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("claude.md"), "uno").unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    let subcarpeta = dir.path().join("msg");
    std::fs::create_dir_all(&subcarpeta).unwrap();
    std::fs::write(subcarpeta.join("001.md"), "primer mensaje").unwrap();

    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::Novedades
    );
}

#[test]
fn un_fichero_nuevo_en_una_subcarpeta_anidada_dos_niveles_se_detecta() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("claude.md"), "uno").unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    let anidada = dir.path().join("msg").join("archivo");
    std::fs::create_dir_all(&anidada).unwrap();
    std::fs::write(anidada.join("002.md"), "mensaje archivado").unwrap();

    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::Novedades
    );
}

#[test]
fn dos_ficheros_con_el_mismo_nombre_en_carpetas_distintas_no_se_confunden() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("a")).unwrap();
    std::fs::create_dir_all(dir.path().join("b")).unwrap();
    std::fs::write(dir.path().join("a").join("x.md"), "contenido").unwrap();
    std::fs::write(dir.path().join("b").join("x.md"), "contenido").unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    // Si la huella confundiera ambos ficheros por compartir nombre, borrar uno mientras el
    // otro sigue intacto podría pasar desapercibido.
    std::fs::remove_file(dir.path().join("a").join("x.md")).unwrap();

    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::Novedades,
        "borrar uno de los dos ficheros homónimos debe detectarse aunque el otro siga intacto"
    );
}

#[test]
fn un_fichero_borrado_dentro_de_una_subcarpeta_se_detecta() {
    let dir = tempfile::tempdir().unwrap();
    let subcarpeta = dir.path().join("msg");
    std::fs::create_dir_all(&subcarpeta).unwrap();
    std::fs::write(subcarpeta.join("001.md"), "uno").unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    std::fs::remove_file(subcarpeta.join("001.md")).unwrap();

    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::Novedades
    );
}

#[test]
fn un_directorio_vacio_no_da_falsos_positivos() {
    let dir = tempfile::tempdir().unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::SinCambios
    );
}

#[test]
fn un_directorio_inexistente_da_un_error_claro() {
    let dir = tempfile::tempdir().unwrap();
    let inexistente = dir.path().join("no-existe");
    let mut huella = None;

    let fallo = hay_novedades(&inexistente, &mut huella).unwrap_err();

    assert!(
        fallo.to_string().contains("no-existe"),
        "el error decía: {fallo}"
    );
}

/// Bloquea un fichero en exclusiva para que `std::fs::read` falle mientras viva el manejador.
///
/// Es la única forma fiable de provocar en una prueba el fallo transitorio de lectura que se da
/// de verdad en disco (un antivirus que abre el fichero un instante, una unidad virtual que
/// todavía no lo tiene local).
#[cfg(windows)]
fn bloquear_en_exclusiva(ruta: &std::path::Path) -> std::fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(ruta)
        .unwrap()
}

#[test]
#[cfg(windows)]
fn un_fichero_que_no_se_puede_leer_no_se_confunde_con_uno_borrado() {
    let dir = tempfile::tempdir().unwrap();
    let fichero = dir.path().join("codex.md");
    std::fs::write(&fichero, "contenido estable\n").unwrap();

    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();
    let referencia = huella.clone();

    // Con el fichero bloqueado, la foto del canal está incompleta: la pasada se ignora.
    let bloqueo = bloquear_en_exclusiva(&fichero);
    let sondeo = hay_novedades(dir.path(), &mut huella).unwrap();
    drop(bloqueo);

    assert_eq!(
        sondeo,
        Sondeo::Ignorada,
        "una lectura fallida no puede anunciarse como novedad"
    );
    assert_eq!(
        huella, referencia,
        "la pasada ignorada no puede pisar la referencia buena"
    );
}

#[test]
#[cfg(windows)]
fn tras_una_pasada_ignorada_el_canal_quieto_sigue_sin_dar_novedades() {
    let dir = tempfile::tempdir().unwrap();
    let fichero = dir.path().join("codex.md");
    std::fs::write(&fichero, "contenido estable\n").unwrap();

    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    let bloqueo = bloquear_en_exclusiva(&fichero);
    hay_novedades(dir.path(), &mut huella).unwrap();
    drop(bloqueo);

    // Éste es el fallo que se vio en uso real: al recuperarse la lectura, el sondeo anunciaba
    // una segunda novedad falsa. Con la referencia intacta, no hay nada que anunciar.
    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::SinCambios
    );
}

#[test]
#[cfg(windows)]
fn un_cambio_real_ocurrido_durante_el_bloqueo_se_detecta_al_recuperarse() {
    let dir = tempfile::tempdir().unwrap();
    let fichero = dir.path().join("codex.md");
    std::fs::write(&fichero, "contenido estable\n").unwrap();

    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    let bloqueo = bloquear_en_exclusiva(&fichero);
    hay_novedades(dir.path(), &mut huella).unwrap();
    std::fs::write(dir.path().join("gemini.md"), "encargo nuevo\n").unwrap();
    drop(bloqueo);

    // Ignorar una pasada no puede tragarse una novedad de verdad: solo la retrasa una vuelta.
    assert_eq!(
        hay_novedades(dir.path(), &mut huella).unwrap(),
        Sondeo::Novedades
    );
}

#[test]
fn mensaje_fallo_arranque_explica_fallo_consecuencia_y_accion_segun_error() {
    // Caso 1: lectura de programator.toml
    let fallo_toml = Error::Lectura {
        ruta: std::path::PathBuf::from("programator.toml"),
        causa: std::io::Error::new(std::io::ErrorKind::NotFound, "no encontrado"),
    };
    let texto_1 = mensaje_fallo_arranque(&fallo_toml);
    assert!(texto_1.starts_with("🔴 Programator no pudo arrancar"));
    assert!(texto_1.contains("Consecuencia:"));
    assert!(texto_1.contains("Qué hacer:"));
    assert!(texto_1.contains("programator.ejemplo.toml"));

    // Caso 2: configuración con problema de carpeta
    let fallo_carpeta =
        Error::ConfiguracionDeLaCarpeta("la carpeta de trabajo no existe".to_string());
    let texto_2 = mensaje_fallo_arranque(&fallo_carpeta);
    assert!(texto_2.contains("--ruta <DIRECTORIO>"));

    // Caso 3: configuración con problema de motor
    let fallo_motor =
        Error::ConfiguracionDelMotor("no hay ningún motor escuchando en el puerto".to_string());
    let texto_3 = mensaje_fallo_arranque(&fallo_motor);
    assert!(texto_3.contains("--diagnostico"));
    assert!(texto_3.contains("[motor]"));

    // Caso 4: un fallo de configuración cualquiera con «carpeta», «motor» o «modelo» en la ruta
    // (`D:\proyectos\modelos-3d\…`) lleva el consejo general: el consejo no sale del texto.
    let fallo_con_ruta = Error::Configuracion(
        "«D:\\proyectos\\modelos-3d\\carpeta-del-motor\\latido.json» está corrupto".to_string(),
    );
    let texto_4 = mensaje_fallo_arranque(&fallo_con_ruta);
    assert!(
        texto_4.contains("la sintaxis de «programator.toml»"),
        "{texto_4}"
    );
    assert!(!texto_4.contains("[carpeta]"), "{texto_4}");
    assert!(!texto_4.contains("[motor]"), "{texto_4}");
}

#[test]
fn mensaje_fallo_publicar_guia_explica_fallo_consecuencia_y_accion() {
    let fallo = Error::Escritura {
        ruta: std::path::PathBuf::from(".gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md"),
        causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denegado"),
    };
    let mensaje = mensaje_fallo_publicar_guia(&fallo);
    assert!(mensaje.starts_with("⚠️ No se pudo publicar la guía para el equipo"));
    assert!(mensaje.contains("Consecuencia:"));
    assert!(mensaje.contains("Qué hacer:"));
}

#[test]
fn mensaje_canal_parcialmente_ilegible_explica_fallo_consecuencia_y_accion() {
    let mensaje = mensaje_canal_parcialmente_ilegible();
    assert!(mensaje.starts_with("⚠️ Parte del canal no se pudo leer"));
    assert!(mensaje.contains("Consecuencia:"));
    assert!(mensaje.contains("Qué hacer:"));
    assert!(mensaje.contains("no hace falta hacer nada"));
}

#[test]
fn mensaje_fallo_pasada_ciclo_explica_fallo_consecuencia_y_accion() {
    let fallo = Error::MotorSinRespuesta("GPU en contención".to_string());
    let mensaje = mensaje_fallo_pasada_ciclo(&fallo);
    assert!(mensaje.starts_with("⚠️ La pasada de este ciclo falló"));
    assert!(mensaje.contains("Consecuencia:"));
    assert!(mensaje.contains("Qué hacer:"));
    assert!(mensaje.contains("contención temporal de GPU"));
}

#[test]
fn mensaje_fallo_sondeo_canal_explica_fallo_consecuencia_y_accion() {
    let fallo = Error::Lectura {
        ruta: std::path::PathBuf::from(".gestor/canal"),
        causa: std::io::Error::new(std::io::ErrorKind::TimedOut, "tiempo de espera agotado"),
    };
    let mensaje = mensaje_fallo_sondeo_canal(&fallo);
    assert!(mensaje.starts_with("⚠️ No se pudo sondear el canal"));
    assert!(mensaje.contains("Consecuencia:"));
    assert!(mensaje.contains("Qué hacer:"));
}

#[test]
fn cadencia_sondeo_expresa_tiempo_en_unidad_natural_y_cubre_casos_de_corte() {
    // Casos inferiores a un minuto
    assert_eq!(cadencia_sondeo(0), "cada 0 segundos");
    assert_eq!(cadencia_sondeo(1), "cada segundo");
    assert_eq!(cadencia_sondeo(45), "cada 45 segundos");
    assert_eq!(cadencia_sondeo(59), "cada 59 segundos");

    // Múltiplos exactos de minuto
    assert_eq!(cadencia_sondeo(60), "cada minuto");
    assert_eq!(cadencia_sondeo(120), "cada 2 minutos");
    assert_eq!(cadencia_sondeo(180), "cada 3 minutos");
    assert_eq!(cadencia_sondeo(300), "cada 5 minutos");

    // Combinaciones de minutos y segundos
    assert_eq!(cadencia_sondeo(61), "cada 1 minuto y 1 segundo");
    assert_eq!(cadencia_sondeo(90), "cada 1 minuto y 30 segundos");
    assert_eq!(cadencia_sondeo(121), "cada 2 minutos y 1 segundo");
    assert_eq!(cadencia_sondeo(150), "cada 2 minutos y 30 segundos");
}

#[test]
fn mensaje_listo_primer_arranque_informa_punto_de_partida_y_cadencia_sin_jerga_interna() {
    let mensaje = mensaje_listo_primer_arranque(180);
    assert!(mensaje.contains("punto de partida sin atender encargos anteriores"));
    assert!(mensaje.contains("Programator ya está escuchando"));
    assert!(mensaje.contains("cualquier encargo que se publique a partir de ahora será atendido"));
    assert!(mensaje.contains("cada 3 minutos"));
    // Sin jerga técnica interna dirigida a operadores
    assert!(!mensaje.contains("registro vacío"));
    assert!(!mensaje.contains("primera pasada"));

    // Caso singular 1 segundo
    let mensaje_1s = mensaje_listo_primer_arranque(1);
    assert!(mensaje_1s.contains("cada segundo"));
}

#[test]
fn mensaje_listo_reinicio_informa_escucha_activa_y_cadencia() {
    let mensaje = mensaje_listo_reinicio(180);
    assert!(mensaje.contains("Programator está escuchando el canal"));
    assert!(mensaje.contains("cualquier encargo nuevo será atendido"));
    assert!(mensaje.contains("cada 3 minutos"));

    let mensaje_1s = mensaje_listo_reinicio(1);
    assert!(mensaje_1s.contains("cada segundo"));
}

#[test]
fn mensaje_version_compone_nombre_y_version() {
    assert_eq!(mensaje_version("0.10.0"), "Programator 0.10.0");
}

#[test]
fn mensaje_argumentos_orientan_al_usuario() {
    let no_entendido = mensaje_argumento_no_entendido("--invento");
    assert!(no_entendido.contains("No entiendo «--invento»"));
    // El nombre se compone según la plataforma, así que la prueba lo compone igual en vez de
    // fijar el de Windows: lo que se afirma es que el mensaje enseña a pedir ayuda, y eso vale
    // en los dos sistemas.
    assert!(no_entendido.contains(&format!("{NOMBRE_DEL_EJECUTABLE} --ayuda")));

    let falta_valor = mensaje_argumento_falta_valor("--ruta");
    assert!(falta_valor.contains("«--ruta» necesita un valor detrás"));
    assert!(falta_valor.contains(&format!("Ejemplo: {NOMBRE_DEL_EJECUTABLE} --ruta")));
    assert!(falta_valor.contains(EJEMPLO_DE_RUTA));
}

#[test]
fn mensajes_de_inicio_y_guia_formatean_correctamente() {
    let ruta = std::path::Path::new("O:/proyecto");
    let texto_carpeta = mensaje_carpeta_de_trabajo(ruta, "de la línea de órdenes (--ruta)");
    assert!(texto_carpeta.contains("Carpeta de trabajo:"));
    assert!(texto_carpeta.contains("de la línea de órdenes (--ruta)"));

    let destino = std::path::Path::new("O:/proyecto/.gestor/PROGRAMATOR.md");
    let texto_normas = mensaje_instrucciones_instaladas(destino);
    assert!(texto_normas.contains("Instaladas las instrucciones del proyecto en"));

    let texto_guia = mensaje_guia_publicada();
    assert!(texto_guia.contains("COMO-ENCARGAR-A-PROGRAMATOR.md"));

    let texto_orientacion = mensaje_orientacion_modelo_pago();
    assert!(texto_orientacion.contains("Cómo empezar:"));
    assert!(texto_orientacion.contains("Abre tu modelo de pago habitual"));
    assert!(texto_orientacion.contains("COMO-ENCARGAR-A-PROGRAMATOR.md"));
    assert!(texto_orientacion.contains(&super::vigilante::orden_para_el_agente()));
    assert!(texto_orientacion.contains("«# Bandeja de <tu identidad>»"));

    let texto_personas = mensaje_guia_para_personas();
    assert!(texto_personas.contains("LEEME.md"), "{texto_personas}");
    assert!(
        texto_personas.contains("docs/Manual de uso.md"),
        "{texto_personas}"
    );
    assert!(
        texto_personas.contains("COMO-ENCARGAR-A-PROGRAMATOR.md"),
        "{texto_personas}"
    );
}

#[test]
fn mensajes_de_ciclo_y_desenlace_formatean_correctamente() {
    assert_eq!(
        mensaje_novedades_atendiendo(),
        "Novedades en el canal: atendiendo…"
    );
    assert_eq!(
        mensaje_reanudacion_encargo_esperando_motor(),
        "Se retoma el encargo que quedó esperando a que cargara el modelo."
    );
    assert_eq!(
        mensaje_desenlace_sin_encargos(),
        "Había novedades, pero ninguna era un encargo para Programator."
    );
    assert_eq!(
        mensaje_desenlace_motor_cargando(),
        "El motor local todavía está cargando el modelo: se reintenta en el ciclo siguiente."
    );
    assert_eq!(
        mensaje_desenlace_atendidos(3, 1),
        "Encargos atendidos: 3 (fallos al publicar: 1)."
    );
}

/// Para que el texto del arranque no salga apelotonado, cada bloque termina en una sola línea, y
/// al imprimirlo queda una en blanco debajo.
#[test]
fn cada_bloque_del_arranque_deja_una_linea_en_blanco_debajo() {
    assert_eq!(bloque("aviso"), "aviso\n");
    assert_eq!(bloque("aviso\n"), "aviso\n");
    assert_eq!(bloque("aviso\n\n\n"), "aviso\n");
    assert_eq!(format!("{}\n", bloque("uno\ndos")), "uno\ndos\n\n");
}
