//! Pruebas de integración del ciclo de sesión (`ejecutar_pasada`): atención de encargos en el canal,
//! registro de lectura, actualización del latido, reintentos ante caídas del motor y desenlaces.

use super::*;
use crate::error::Error;

use crate::motor::{MotorDoble, Respuesta, SolicitudHerramienta};
use crate::protocolo::EstadoLatido;

fn carpeta_de_prueba() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let carpeta = dir.path().to_path_buf();
    std::fs::create_dir_all(carpeta.join(".gestor/canal")).unwrap();
    std::fs::write(carpeta.join("PROGRAMATOR.md"), "NORMAS DE LA CASA\n").unwrap();
    (dir, carpeta)
}

fn escribir_buzon(carpeta: &Path, nombre: &str, contenido: &str) {
    std::fs::write(carpeta.join(".gestor/canal").join(nombre), contenido).unwrap();
}

/// Simula que ya hubo una primera pasada, dejando el registro no vacío sin condicionar el
/// delta de ningún buzón real: la clave sembrada no coincide con ninguno.
fn sembrar_registro_no_vacio(carpeta: &Path) {
    let ruta = ruta_del_registro(carpeta, "Programator");
    let mut registro = RegistroLectura::default();
    registro.delta("__semilla__", "x");
    registro.guardar(&ruta).unwrap();
}

fn motor_con_guion(guion: Vec<Respuesta>) -> impl FnMut() -> Resultado<EstadoMotor> {
    let mut motor = Some(MotorDoble::con_guion(guion));
    move || Ok(EstadoMotor::Disponible(Box::new(motor.take().unwrap())))
}

fn pedir(nombre: &str, argumentos: serde_json::Value) -> Respuesta {
    Respuesta::Herramienta(SolicitudHerramienta {
        nombre: nombre.to_string(),
        argumentos,
    })
}

#[test]
fn con_el_registro_vacio_no_se_llama_al_modelo_y_el_registro_queda_guardado() {
    let (_dir, carpeta) = carpeta_de_prueba();
    escribir_buzon(&carpeta, "codex.md", "## Para Programator\n\nHaz algo.\n");
    let mut llamadas = 0u32;
    let mut asegurar_motor = || -> Resultado<EstadoMotor> {
        llamadas += 1;
        Err(Error::Configuracion(
            "no debía llamarse al motor".to_string(),
        ))
    };

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(matches!(desenlace, DesenlacePasada::PrimeraPasada));
    assert_eq!(llamadas, 0, "la primera pasada no debe tocar el motor");
    assert!(
        ruta_del_registro(&carpeta, "Programator").exists(),
        "el registro debe quedar guardado tras la primera pasada"
    );
}

#[test]
fn un_encargo_dirigido_a_programator_se_atiende_y_se_publica() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nRevisa las puertas.\n",
    );

    let mut asegurar_motor = motor_con_guion(vec![
        pedir(
            "publicar",
            serde_json::json!({"texto": "Puertas revisadas."}),
        ),
        Respuesta::Texto("Listo.".to_string()),
    ]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    match desenlace {
        DesenlacePasada::Atendidos(resumen) => {
            assert_eq!(resumen.atendidos, 1);
            assert_eq!(resumen.fallidos, 0);
        }
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }
    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(buzon.contains("Puertas revisadas."));
}

#[test]
fn un_encargo_que_termina_sin_entrega_se_publica_con_aviso_explicito() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude.md",
        "## Para Programator\n\nAnaliza pero no entregues nada.\n",
    );

    let mut asegurar_motor = motor_con_guion(vec![Respuesta::Texto(
        "He terminado de analizar sin usar herramientas.".to_string(),
    )]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    match desenlace {
        DesenlacePasada::Atendidos(resumen) => {
            assert_eq!(resumen.atendidos, 1);
            assert_eq!(resumen.fallidos, 0);
        }
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }

    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(
        buzon.contains("⚠️ Programator no invocó herramientas de entrega"),
        "no incluye el aviso explícito de falta de entrega: {buzon}"
    );
    assert!(
        buzon.contains("He terminado de analizar sin usar herramientas."),
        "no incluye la respuesta directa del modelo: {buzon}"
    );
}

#[test]
fn un_encargo_dirigido_a_otro_agente_no_dispara_nada() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Claude\n\nAlgo que no es mío.\n",
    );

    let mut llamadas = 0u32;
    let mut asegurar_motor = || -> Resultado<EstadoMotor> {
        llamadas += 1;
        Err(Error::Configuracion(
            "no debía llamarse al motor".to_string(),
        ))
    };

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(matches!(desenlace, DesenlacePasada::SinEncargos));
    assert_eq!(
        llamadas, 0,
        "que haya novedades no significa que vayan dirigidas a él"
    );
}

#[test]
fn el_desenlace_abortado_tambien_se_publica() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nTarea imposible.\n",
    );

    // Guion vacío: el motor se agota en la primera llamada y `atender_encargo` lo registra
    // como un `Abortado`, no como un éxito.
    let mut asegurar_motor = motor_con_guion(vec![]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    match desenlace {
        DesenlacePasada::Atendidos(resumen) => {
            assert_eq!(
                resumen.atendidos, 1,
                "un aborto también cuenta como publicado"
            );
        }
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }
    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(
        buzon.contains("Programator no pudo completar el encargo"),
        "el motivo del aborto debe quedar en el canal: {buzon}"
    );
}

#[test]
fn el_acuse_de_lectura_menciona_el_buzon_leido_y_su_latido() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "# Codex\n\n**LATIDO:** 2026-09-01 10:00 — puertas revisadas\n\n---\n\n\
         ## Para Programator\n\nRevisa esto.\n",
    );

    let mut asegurar_motor = motor_con_guion(vec![
        pedir("publicar", serde_json::json!({"texto": "Hecho."})),
        Respuesta::Texto("Listo.".to_string()),
    ]);

    ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(buzon.contains("codex.md"));
    assert!(
        buzon.contains("hasta su latido 2026-09-01 10:00"),
        "el acuse de lectura debe citar el latido del buzón leído: {buzon}"
    );
}

#[test]
fn tras_atender_la_pasada_siguiente_no_vuelve_a_atender_lo_mismo() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nRevisa esto.\n",
    );

    let mut primera_llamada = motor_con_guion(vec![
        pedir("publicar", serde_json::json!({"texto": "Hecho."})),
        Respuesta::Texto("Listo.".to_string()),
    ]);
    let primera = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut primera_llamada,
    )
    .unwrap();
    assert!(matches!(primera, DesenlacePasada::Atendidos(_)));

    let mut llamadas = 0u32;
    let mut segunda_llamada = || -> Resultado<EstadoMotor> {
        llamadas += 1;
        Err(Error::Configuracion(
            "no debía llamarse al motor".to_string(),
        ))
    };
    let segunda = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut segunda_llamada,
    )
    .unwrap();

    assert!(matches!(segunda, DesenlacePasada::SinEncargos));
    assert_eq!(
        llamadas, 0,
        "el mismo encargo no debe volver a detectarse ni a disparar el motor"
    );
}

#[test]
fn un_fallo_en_un_encargo_no_impide_atender_el_siguiente() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    // Orden alfabético de `buzones_ajenos`: claude.md antes que codex.md.
    escribir_buzon(
        &carpeta,
        "claude.md",
        "## Para Programator\n\nTarea 1 (fallará).\n",
    );
    escribir_buzon(&carpeta, "codex.md", "## Para Programator\n\nTarea 2.\n");

    let mut asegurar_motor = motor_con_guion(vec![
        // Tarea 1: dos denegaciones seguidas de la misma herramienta abortan el ciclo.
        pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
        pedir(
            "leer_fichero",
            serde_json::json!({"ruta": "../../otra.txt"}),
        ),
        // Tarea 2: se atiende con normalidad a continuación, con el mismo motor.
        pedir("publicar", serde_json::json!({"texto": "Tarea 2 hecha."})),
        Respuesta::Texto("Listo.".to_string()),
    ]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    match desenlace {
        DesenlacePasada::Atendidos(resumen) => assert_eq!(
            resumen.atendidos, 2,
            "las dos deben quedar publicadas, aunque una acabe abortada"
        ),
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }
    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(buzon.contains("Tarea 2 hecha."));
    assert!(
        buzon.contains("insistió"),
        "el aborto de la Tarea 1 también debe quedar: {buzon}"
    );
}

#[test]
fn si_el_motor_esta_cargando_no_se_atiende_nada_y_se_reintenta_despues() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nRevisa esto.\n",
    );

    let mut asegurar_motor = || -> Resultado<EstadoMotor> { Ok(EstadoMotor::Esperando) };
    let primera = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();
    assert!(matches!(primera, DesenlacePasada::MotorCargando));

    // El registro no debió avanzar: el mismo encargo se detecta de nuevo en la siguiente
    // pasada, en vez de haberse perdido.
    let mut llamadas = 0u32;
    let mut segunda_llamada = || -> Resultado<EstadoMotor> {
        llamadas += 1;
        Ok(EstadoMotor::Esperando)
    };
    let segunda = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut segunda_llamada,
    )
    .unwrap();

    assert!(matches!(segunda, DesenlacePasada::MotorCargando));
    assert_eq!(
        llamadas, 1,
        "debe volver a detectar el mismo encargo, no haberlo perdido"
    );
}

#[test]
fn al_atender_un_encargo_se_escribe_el_latido_en_disco() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude.md",
        "## Para Programator\n\nTarea A\nHaz algo.\n",
    );

    let mut asegurar_motor = motor_con_guion(vec![
        pedir("publicar", serde_json::json!({"texto": "Hecho."})),
        Respuesta::Texto("Listo.".to_string()),
    ]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(matches!(desenlace, DesenlacePasada::Atendidos(_)));

    let ruta_latido = ruta_del_latido(&carpeta, "Programator", "latido.json");
    let latido = Latido::cargar(&ruta_latido)
        .unwrap()
        .expect("latido debe existir");
    assert_eq!(latido.estado, EstadoLatido::Atendiendo);
    let encargo = latido.encargo.expect("debe haber encargo");
    assert_eq!(encargo.buzon, "claude.md");
    assert_eq!(encargo.resumen, "Tarea A");
}

#[test]
fn si_asegurar_motor_falla_se_escribe_latido_en_error() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude.md",
        "## Para Programator: Tarea B\nHaz algo.\n",
    );

    let mut asegurar_motor = || -> Resultado<EstadoMotor> {
        Err(Error::Configuracion("servidor no disponible".to_string()))
    };

    let resultado = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    );

    assert!(resultado.is_err());

    let ruta_latido = ruta_del_latido(&carpeta, "Programator", "latido.json");
    let latido = Latido::cargar(&ruta_latido)
        .unwrap()
        .expect("latido debe existir");
    assert_eq!(latido.estado, EstadoLatido::Error);
    assert!(latido
        .detalle_error
        .unwrap()
        .contains("servidor no disponible"));
}

#[test]
fn un_encargo_abortado_escribe_latido_de_error() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude.md",
        "## Para Programator: Tarea C\nHaz algo imposible.\n",
    );

    // Guion vacío aborta el ciclo
    let mut asegurar_motor = motor_con_guion(vec![]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(matches!(desenlace, DesenlacePasada::Atendidos(_)));

    let ruta_latido = ruta_del_latido(&carpeta, "Programator", "latido.json");
    let latido = Latido::cargar(&ruta_latido)
        .unwrap()
        .expect("latido debe existir");
    assert_eq!(latido.estado, EstadoLatido::Error);
    assert!(latido.encargo.is_some());
    assert_eq!(latido.encargo.unwrap().buzon, "claude.md");
    assert!(latido.detalle_error.is_some());
}

#[test]
fn entrega_de_propuesta_publica_en_buzon_antes_de_cerrar_y_no_duplica_al_cierre() {
    // Criterios Tarea 4 (Incidencia C2 de NatureLand):
    // 1. Escribir una propuesta deja rastro en el buzón antes de cerrar el encargo.
    // 2. El cierre del encargo sigue publicando su cuerpo.
    // 3. No se duplica el contenido ni los veredictos entre las dos escrituras.
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nGenera la propuesta solucion.rs y avisa.\n",
    );

    let mut asegurar_motor = motor_con_guion(vec![
        pedir(
            "escribir_propuesta",
            serde_json::json!({
                "nombre": "solucion.rs",
                "contenido": "pub fn resolver() -> i32 { 42 }"
            }),
        ),
        pedir(
            "publicar",
            serde_json::json!({"texto": "Encargo resuelto con éxito."}),
        ),
        Respuesta::Texto("Listo.".to_string()),
    ]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    match desenlace {
        DesenlacePasada::Atendidos(resumen) => {
            assert_eq!(resumen.atendidos, 1);
            assert_eq!(resumen.fallidos, 0);
        }
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }

    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();

    // 1. Rastro de la entrega intermedia presente
    assert!(
        buzon.contains("He dejado la propuesta «solucion.rs» en disco."),
        "el buzón debe contener el aviso de entrega intermedia: {buzon}"
    );

    // 2. El cierre sigue publicando su cuerpo
    assert!(
        buzon.contains("Encargo resuelto con éxito."),
        "el buzón debe contener el cuerpo final del encargo: {buzon}"
    );

    // 3. No se duplica el bloque de comprobación de propuestas
    let apariciones_comprobacion = buzon.matches("**Comprobación de las propuestas**").count();
    assert_eq!(
        apariciones_comprobacion, 1,
        "el bloque de comprobación no debe duplicarse al cerrar el encargo: {buzon}"
    );

    let apariciones_solucion = buzon.matches("`solucion.rs`: ⚠️ SIN COMPROBAR").count();
    assert_eq!(
        apariciones_solucion, 1,
        "el veredicto de la propuesta no debe duplicarse: {buzon}"
    );
}

#[test]
fn multiples_propuestas_se_publican_al_entregar_y_cierre_no_duplica() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nEntrega dos propuestas.\n",
    );

    let mut asegurar_motor = motor_con_guion(vec![
        pedir(
            "escribir_propuesta",
            serde_json::json!({"nombre": "alfa.rs", "contenido": "fn alfa() {}"}),
        ),
        pedir(
            "escribir_propuesta",
            serde_json::json!({"nombre": "beta.rs", "contenido": "fn beta() {}"}),
        ),
        pedir(
            "publicar",
            serde_json::json!({"texto": "Ambas propuestas entregadas."}),
        ),
        Respuesta::Texto("Listo.".to_string()),
    ]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(matches!(desenlace, DesenlacePasada::Atendidos(_)));

    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(buzon.contains("He dejado la propuesta «alfa.rs» en disco."));
    assert!(buzon.contains("He dejado la propuesta «beta.rs» en disco."));
    assert!(buzon.contains("Ambas propuestas entregadas."));

    // Dos publicaciones intermedias (una por propuesta), cero en el cierre final
    assert_eq!(
        buzon.matches("**Comprobación de las propuestas**").count(),
        2,
        "debe haber exactamente 2 bloques de comprobación (uno por entrega): {buzon}"
    );
}

#[test]
fn al_cerrar_encargo_se_genera_propuesta_de_poda_y_reserva_en_estado() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude.md",
        "# Claude\n\n**LATIDO:** 10:00\n\n---\n\n## Para Programator\n\nImplementa el validador.\n\n---\n\n## Para Codex\n\nTarea pendiente.\n",
    );

    let mut asegurar_motor = motor_con_guion(vec![
        pedir(
            "publicar",
            serde_json::json!({"texto": "Validador implementado con éxito."}),
        ),
        Respuesta::Texto("Fin.".to_string()),
    ]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(matches!(desenlace, DesenlacePasada::Atendidos(_)));

    // 1. Verificar propuesta de poda generada en candidatos
    let ruta_podado = carpeta.join(".gestor/candidatos/poda/claude.md");
    let ruta_archivo = carpeta.join(".gestor/candidatos/poda/claude-archivo.md");
    assert!(ruta_podado.exists(), "debe existir el fichero podado");
    assert!(ruta_archivo.exists(), "debe existir el fichero de archivo");

    let texto_podado = std::fs::read_to_string(&ruta_podado).unwrap();
    let texto_archivo = std::fs::read_to_string(&ruta_archivo).unwrap();

    assert!(texto_podado.contains("# Claude"));
    assert!(texto_podado.contains("## Para Codex"));
    assert!(!texto_podado.contains("Implementa el validador"));

    assert!(texto_archivo.contains("Implementa el validador"));

    // 2. Verificar reserva anotada en estado.md
    let estado = std::fs::read_to_string(carpeta.join(".gestor/canal/estado.md")).unwrap();
    assert!(
        estado.contains("- **Programator**: `.gestor/candidatos/poda/claude.md` — propuesta de poda tras cerrar encargo"),
        "la reserva debe constar en estado.md: {estado}"
    );

    // 3. Verificar aviso de poda en la respuesta en el buzón
    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(
        buzon.contains("**Propuesta de poda para claude:**"),
        "el buzón debe contener el aviso de poda: {buzon}"
    );
    assert!(buzon.contains(".gestor/candidatos/poda/claude.md"));
}

#[test]
fn nueva_pasada_rota_buzon_propio_a_historico_manteniendo_solo_ultimo_latido() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nPrimera tarea.\n",
    );

    let mut motor1 = motor_con_guion(vec![
        pedir(
            "publicar",
            serde_json::json!({"texto": "Primera tarea completada."}),
        ),
        Respuesta::Texto("Fin 1.".to_string()),
    ]);

    ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut motor1,
    )
    .unwrap();

    let buzon1 = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(buzon1.contains("Primera tarea completada."));
    let ruta_hist = carpeta.join(".gestor/canal/historico-programator.md");
    assert!(!ruta_hist.exists());

    // Segunda pasada con nueva tarea
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nPrimera tarea.\n\n---\n\n## Para Programator\n\nSegunda tarea.\n",
    );

    let mut motor2 = motor_con_guion(vec![
        pedir(
            "publicar",
            serde_json::json!({"texto": "Segunda tarea completada."}),
        ),
        Respuesta::Texto("Fin 2.".to_string()),
    ]);

    ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&Limites::default(), &Verificacion::default()),
        &mut motor2,
    )
    .unwrap();

    let buzon2 = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(buzon2.contains("Segunda tarea completada."));
    assert!(
        !buzon2.contains("Primera tarea completada."),
        "el buzón propio vivo solo debe mantener el último latido"
    );

    assert!(ruta_hist.exists(), "debe haberse creado el histórico");
    let texto_hist = std::fs::read_to_string(&ruta_hist).unwrap();
    assert!(
        texto_hist.contains("Primera tarea completada."),
        "el historial previo debe conservarse en el archivo histórico"
    );
}
