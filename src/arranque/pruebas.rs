//! Pruebas unitarias de `arranque`: sondeo de novedades en el canal, composición y publicación de normas
//! y guía para el equipo, redacción de avisos explicativos de consola y comprobación de registro previo.

use super::*;
use crate::error::Error;
use crate::plataforma::{EJEMPLO_DE_RUTA, NOMBRE_DEL_EJECUTABLE};

/// El fragmento de desempeño no es lo que estas pruebas examinan, así que se escribe una vez,
/// vacío de marcadores, y se reutiliza.
fn escribir_desempeno_vacio(dir: &std::path::Path) -> std::path::PathBuf {
    let ruta = dir.join("desempeno.md");
    std::fs::write(&ruta, "CIFRAS").unwrap();
    ruta
}

#[test]
fn la_guia_para_el_equipo_se_actualiza_en_cada_arranque() {
    // Al revés que las normas: una guía vieja es peor que ninguna, porque dice qué sabe hacer
    // el modelo y dónde miente, y eso cambia con cada versión y con cada modelo que se ponga
    // detrás. Se lee como vigente aunque no lo sea.
    let dir = tempfile::tempdir().unwrap();
    let plantilla = dir.path().join("guia.md");
    let desempeno = escribir_desempeno_vacio(dir.path());
    let canal = tempfile::tempdir().unwrap();
    let comprobadores = crate::arnes::verificacion::Comprobadores::new();

    std::fs::write(&plantilla, "VERSION VIEJA").unwrap();
    publicar_guia(
        canal.path(),
        &plantilla,
        "GUIA.md",
        16384,
        &desempeno,
        &comprobadores,
    )
    .unwrap();

    std::fs::write(&plantilla, "VERSION NUEVA").unwrap();
    publicar_guia(
        canal.path(),
        &plantilla,
        "GUIA.md",
        16384,
        &desempeno,
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
    // En NatureLand la guía decía 16.384 mientras el TOML declaraba 32.768. Las dos cifras eran
    // ciertas —una era el defecto y la otra el ajuste local— y ninguna decía cuál mandaba.
    let plantilla_dir = tempfile::tempdir().unwrap();
    let plantilla = plantilla_dir.path().join("guia.md");
    std::fs::write(&plantilla, "ventana de {contexto} tokens").unwrap();
    let desempeno = escribir_desempeno_vacio(plantilla_dir.path());
    let canal = tempfile::tempdir().unwrap();
    let comprobadores = crate::arnes::verificacion::Comprobadores::new();

    publicar_guia(
        canal.path(),
        &plantilla,
        "GUIA.md",
        32768,
        &desempeno,
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
    let desempeno = dir.path().join("desempeno.md");
    std::fs::write(&desempeno, "27 de 41 auditando formas").unwrap();
    let canal = tempfile::tempdir().unwrap();
    let mut comprobadores = crate::arnes::verificacion::Comprobadores::new();
    comprobadores.insert("py".to_string(), vec!["python".to_string()]);

    publicar_guia(
        canal.path(),
        &plantilla,
        "GUIA.md",
        16384,
        &desempeno,
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

    let msg = dir.path().join("msg");
    std::fs::create_dir_all(&msg).unwrap();
    std::fs::write(msg.join("001.md"), "primer mensaje").unwrap();

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
    let msg = dir.path().join("msg");
    std::fs::create_dir_all(&msg).unwrap();
    std::fs::write(msg.join("001.md"), "uno").unwrap();
    let mut huella = None;
    hay_novedades(dir.path(), &mut huella).unwrap();

    std::fs::remove_file(msg.join("001.md")).unwrap();

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

    // Éste es el fallo que sufrió el Director: al recuperarse la lectura, el sondeo anunciaba
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
    let err_toml = Error::Lectura {
        ruta: std::path::PathBuf::from("programator.toml"),
        causa: std::io::Error::new(std::io::ErrorKind::NotFound, "no encontrado"),
    };
    let txt1 = mensaje_fallo_arranque(&err_toml);
    assert!(txt1.starts_with("🔴 Programator no pudo arrancar"));
    assert!(txt1.contains("Consecuencia:"));
    assert!(txt1.contains("Qué hacer:"));
    assert!(txt1.contains("programator.ejemplo.toml"));

    // Caso 2: configuración con problema de carpeta
    let err_carpeta = Error::Configuracion("la carpeta de trabajo no existe".to_string());
    let txt2 = mensaje_fallo_arranque(&err_carpeta);
    assert!(txt2.contains("--ruta <DIRECTORIO>"));

    // Caso 3: configuración con problema de motor
    let err_motor = Error::Configuracion("no hay ningún motor escuchando en el puerto".to_string());
    let txt3 = mensaje_fallo_arranque(&err_motor);
    assert!(txt3.contains("--diagnostico"));
}

#[test]
fn mensaje_fallo_publicar_guia_explica_fallo_consecuencia_y_accion() {
    let err = Error::Escritura {
        ruta: std::path::PathBuf::from(".gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md"),
        causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denegado"),
    };
    let msg = mensaje_fallo_publicar_guia(&err);
    assert!(msg.starts_with("⚠️ No se pudo publicar la guía para el equipo"));
    assert!(msg.contains("Consecuencia:"));
    assert!(msg.contains("Qué hacer:"));
}

#[test]
fn mensaje_canal_parcialmente_ilegible_explica_fallo_consecuencia_y_accion() {
    let msg = mensaje_canal_parcialmente_ilegible();
    assert!(msg.starts_with("⚠️ Parte del canal no se pudo leer"));
    assert!(msg.contains("Consecuencia:"));
    assert!(msg.contains("Qué hacer:"));
    assert!(msg.contains("no hace falta hacer nada"));
}

#[test]
fn mensaje_fallo_pasada_ciclo_explica_fallo_consecuencia_y_accion() {
    let err = Error::MotorSinRespuesta("GPU en contención".to_string());
    let msg = mensaje_fallo_pasada_ciclo(&err);
    assert!(msg.starts_with("⚠️ La pasada de este ciclo falló"));
    assert!(msg.contains("Consecuencia:"));
    assert!(msg.contains("Qué hacer:"));
    assert!(msg.contains("contención temporal de GPU"));
}

#[test]
fn mensaje_fallo_sondeo_canal_explica_fallo_consecuencia_y_accion() {
    let err = Error::Lectura {
        ruta: std::path::PathBuf::from(".gestor/canal"),
        causa: std::io::Error::new(std::io::ErrorKind::TimedOut, "tiempo de espera agotado"),
    };
    let msg = mensaje_fallo_sondeo_canal(&err);
    assert!(msg.starts_with("⚠️ No se pudo sondear el canal"));
    assert!(msg.contains("Consecuencia:"));
    assert!(msg.contains("Qué hacer:"));
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
    let msg = mensaje_listo_primer_arranque(180);
    assert!(msg.contains("punto de partida sin atender encargos anteriores"));
    assert!(msg.contains("Programator ya está escuchando"));
    assert!(msg.contains("cualquier encargo que se publique a partir de ahora será atendido"));
    assert!(msg.contains("cada 3 minutos"));
    // Sin jerga técnica interna dirigida a operadores
    assert!(!msg.contains("registro vacío"));
    assert!(!msg.contains("primera pasada"));

    // Caso singular 1 segundo
    let msg_1s = mensaje_listo_primer_arranque(1);
    assert!(msg_1s.contains("cada segundo"));
}

#[test]
fn mensaje_listo_reinicio_informa_escucha_activa_y_cadencia() {
    let msg = mensaje_listo_reinicio(180);
    assert!(msg.contains("Programator está escuchando el canal"));
    assert!(msg.contains("cualquier encargo nuevo será atendido"));
    assert!(msg.contains("cada 3 minutos"));

    let msg_1s = mensaje_listo_reinicio(1);
    assert!(msg_1s.contains("cada segundo"));
}

#[test]
fn hay_registro_previo_distingue_inexistente_vacio_y_con_datos() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("lectura.json");

    // 1. Inexistente -> false
    assert!(!hay_registro_previo(&ruta));

    // 2. Registro vacío guardado -> false
    let mut reg = crate::protocolo::RegistroLectura::default();
    reg.guardar(&ruta).unwrap();
    assert!(!hay_registro_previo(&ruta));

    // 3. Registro con datos de lectura -> true
    reg.delta("claude.md", "hola");
    reg.guardar(&ruta).unwrap();
    assert!(hay_registro_previo(&ruta));
}

#[test]
fn mensaje_version_compone_nombre_y_version() {
    assert_eq!(mensaje_version("0.10.0"), "Programator 0.10.0");
}

#[test]
fn mensaje_argumentos_orientan_al_usuario() {
    let err_arg = mensaje_argumento_no_entendido("--invento");
    assert!(err_arg.contains("No entiendo «--invento»"));
    // El nombre se compone según la plataforma, así que la prueba lo compone igual en vez de
    // fijar el de Windows: lo que se afirma es que el mensaje enseña a pedir ayuda, y eso vale
    // en los dos sistemas.
    assert!(err_arg.contains(&format!("{NOMBRE_DEL_EJECUTABLE} --ayuda")));

    let err_falta = mensaje_argumento_falta_valor("--ruta");
    assert!(err_falta.contains("«--ruta» necesita un valor detrás"));
    assert!(err_falta.contains(&format!("Ejemplo: {NOMBRE_DEL_EJECUTABLE} --ruta")));
    assert!(err_falta.contains(EJEMPLO_DE_RUTA));
}

#[test]
fn mensajes_de_inicio_y_guia_formatean_correctamente() {
    let ruta = std::path::Path::new("O:/proyecto");
    let msg_carpeta = mensaje_carpeta_de_trabajo(ruta, "de la línea de órdenes (--ruta)");
    assert!(msg_carpeta.contains("Carpeta de trabajo:"));
    assert!(msg_carpeta.contains("de la línea de órdenes (--ruta)"));

    let destino = std::path::Path::new("O:/proyecto/.gestor/PROGRAMATOR.md");
    let msg_normas = mensaje_instrucciones_instaladas(destino);
    assert!(msg_normas.contains("Instaladas las instrucciones del proyecto en"));

    let msg_guia = mensaje_guia_publicada();
    assert_eq!(
        msg_guia,
        "Publicada la guía para el resto del equipo en el canal."
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
