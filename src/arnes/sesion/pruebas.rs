//! Pruebas de integración del ciclo de sesión (`ejecutar_pasada`): atención de encargos en el canal,
//! registro de lectura, actualización del latido, reintentos ante caídas del motor y desenlaces.

use super::*;
use crate::arnes::espacio::{ruta_del_latido, ruta_del_registro};
use crate::error::Error;
use crate::protocolo::{Canal, Latido, RegistroLectura};

use crate::motor::{MotorDoble, Respuesta, SolicitudHerramienta};
use crate::protocolo::EstadoLatido;

pub(super) fn carpeta_de_prueba() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let carpeta = dir.path().to_path_buf();
    std::fs::create_dir_all(carpeta.join(".gestor/canal")).unwrap();
    std::fs::write(carpeta.join("PROGRAMATOR.md"), "NORMAS DE LA CASA\n").unwrap();
    (dir, carpeta)
}

pub(super) fn escribir_buzon(carpeta: &Path, nombre: &str, contenido: &str) {
    std::fs::write(carpeta.join(".gestor/canal").join(nombre), contenido).unwrap();
}

/// Simula que ya hubo una primera pasada, dejando el registro no vacío sin condicionar el
/// delta de ningún buzón real: la clave sembrada no coincide con ninguno.
pub(super) fn sembrar_registro_no_vacio(carpeta: &Path) {
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
fn sin_registro_y_con_respuestas_previas_no_se_llama_al_modelo_y_el_registro_queda_guardado() {
    // Desde el 22/11/2026 la foto solo da por visto lo anterior si Programator ya respondió alguna
    // vez en el canal; si no, lo atiende (da igual abrir antes el agente o Programator).
    let (_dir, carpeta) = carpeta_de_prueba();
    escribir_buzon(&carpeta, "codex.md", "## Para Programator\n\nHaz algo.\n");
    escribir_buzon(
        &carpeta,
        "programator.md",
        "# Programator\n\nLATIDO: 2026-11-20 10:00:00\nHecho.\n",
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
fn tras_fotografiar_un_canal_vacio_el_primer_encargo_se_atiende() {
    // Una prueba real mostró que, en un proyecto nuevo, la foto se tomaba en el primer cambio del
    // canal, que era el primer encargo, y se perdía. Ahora se toma al arrancar, y un registro
    // guardado sin buzones ya cuenta como foto tomada.
    let (_dir, carpeta) = carpeta_de_prueba();

    assert_eq!(
        tomar_instantanea_inicial(&carpeta, "Programator").unwrap(),
        Instantanea::SinRespuestasPrevias
    );
    assert_eq!(
        tomar_instantanea_inicial(&carpeta, "Programator").unwrap(),
        Instantanea::YaHabia
    );
    escribir_buzon(
        &carpeta,
        "claude-1.md",
        "# Bandeja de Claude 1\n\nLATIDO: 2026-11-22 19:07:53\n\nPara Programator: revisa.\n",
    );
    let mut asegurar_motor = motor_con_guion(vec![
        pedir("publicar", serde_json::json!({"texto": "Revisado."})),
        Respuesta::Texto("Listo.".to_string()),
    ]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    match desenlace {
        DesenlacePasada::Atendidos(resumen) => assert_eq!(resumen.atendidos, 1),
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }
}

/// El buzón de una carpeta de prueba tras arrancar Programator, con un encargo publicado antes.
fn encargo_anterior_al_arranque(carpeta: &Path, programator_ya_respondio: bool) -> DesenlacePasada {
    escribir_buzon(
        carpeta,
        "codex2.md",
        "# Bandeja de Codex 2\n\nLATIDO: 2026-11-22 19:00:00\n\nPara Programator: anterior.\n",
    );
    if programator_ya_respondio {
        escribir_buzon(
            carpeta,
            "programator.md",
            "# Programator\n\nLATIDO: 2026-11-20 10:00:00\nHecho.\n",
        );
    }
    tomar_instantanea_inicial(carpeta, "Programator").unwrap();
    let mut asegurar_motor = motor_con_guion(vec![
        pedir("publicar", serde_json::json!({"texto": "Hecho otra vez."})),
        Respuesta::Texto("Listo.".to_string()),
    ]);
    ejecutar_pasada(
        carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap()
}

#[test]
fn da_igual_abrir_antes_el_agente_si_programator_nunca_respondio_en_el_canal() {
    let (_dir, carpeta) = carpeta_de_prueba();

    match encargo_anterior_al_arranque(&carpeta, false) {
        DesenlacePasada::Atendidos(resumen) => assert_eq!(resumen.atendidos, 1),
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }
}

#[test]
fn sin_foto_al_arrancar_la_pasada_la_toma_y_atiende_si_programator_nunca_respondio() {
    let (_dir, carpeta) = carpeta_de_prueba();
    escribir_buzon(
        &carpeta,
        "codex2.md",
        "# Bandeja de Codex 2\n\nPara Programator: anterior.\n",
    );
    let mut asegurar_motor = motor_con_guion(vec![
        pedir("publicar", serde_json::json!({"texto": "Hecho."})),
        Respuesta::Texto("Listo.".to_string()),
    ]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    match desenlace {
        DesenlacePasada::Atendidos(resumen) => assert_eq!(resumen.atendidos, 1),
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }
}

#[test]
fn un_buzon_propio_con_la_marca_de_latido_antigua_tambien_cuenta_como_respondido() {
    let (_dir, carpeta) = carpeta_de_prueba();
    escribir_buzon(
        &carpeta,
        "programator.md",
        "# Programator\n\n**LATIDO:** 2026-10-15 10:00\nHecho.\n",
    );

    assert_eq!(
        tomar_instantanea_inicial(&carpeta, "Programator").unwrap(),
        Instantanea::Tomada
    );
}

#[test]
fn si_programator_ya_respondio_en_el_canal_lo_anterior_al_arranque_se_da_por_visto() {
    let (_dir, carpeta) = carpeta_de_prueba();

    let desenlace = encargo_anterior_al_arranque(&carpeta, true);

    assert!(
        !matches!(desenlace, DesenlacePasada::Atendidos(_)),
        "{desenlace:?}"
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        // Pide un fichero: sin pedirlo, la respuesta en texto es la entrega y no hay aviso.
        "## Para Programator\n\nDeja el análisis con escribir_propuesta.\n",
    );

    let mut asegurar_motor = motor_con_guion(vec![Respuesta::Texto(
        "He terminado de analizar sin usar herramientas.".to_string(),
    )]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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

    let ruta_latido = ruta_del_latido(&carpeta, "Programator", "latido.json");
    let visto = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut motor = Some(MotorQueMiraElLatido {
        doble: MotorDoble::con_guion(vec![
            pedir("publicar", serde_json::json!({"texto": "Hecho."})),
            Respuesta::Texto("Listo.".to_string()),
        ]),
        ruta: ruta_latido.clone(),
        visto: visto.clone(),
    });
    let mut asegurar_motor =
        move || Ok(EstadoMotor::Disponible(Box::new(motor.take().unwrap())) as _);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(matches!(desenlace, DesenlacePasada::Atendidos(_)));

    // Mientras el modelo trabaja: atendiendo, con su encargo.
    let durante: Latido = visto
        .borrow_mut()
        .take()
        .expect("latido durante la atención");
    assert_eq!(durante.estado(), EstadoLatido::Atendiendo);
    let encargo = durante.encargo().expect("debe haber encargo");
    assert_eq!(encargo.buzon, "claude.md");
    assert_eq!(encargo.resumen, "Tarea A");
    // Una prueba real mostró que, publicada la respuesta, seguía en «atendiendo» hasta el final
    // de la vuelta.
    let despues = Latido::cargar(&ruta_latido)
        .unwrap()
        .expect("latido debe existir");
    assert_eq!(despues.estado(), EstadoLatido::Reposo);
}

/// Un motor de guion que, en su primera llamada, guarda el latido que hay en disco.
struct MotorQueMiraElLatido {
    doble: MotorDoble,
    ruta: std::path::PathBuf,
    visto: std::rc::Rc<std::cell::RefCell<Option<Latido>>>,
}

impl crate::motor::Motor for MotorQueMiraElLatido {
    fn responder(&mut self, conversacion: &[crate::motor::Mensaje]) -> Resultado<Respuesta> {
        if self.visto.borrow().is_none() {
            *self.visto.borrow_mut() = Latido::cargar(&self.ruta).unwrap();
        }
        self.doble.responder(conversacion)
    }

    fn comprobar_salud(&mut self) -> crate::motor::proceso::EstadoServidor {
        self.doble.comprobar_salud()
    }

    fn ultimo_consumo(&self) -> Option<crate::motor::Consumo> {
        self.doble.ultimo_consumo()
    }
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    );

    // El fallo no corta la pasada: el encargo va a la cola de pendientes con su motivo.
    assert!(resultado.is_ok(), "{resultado:?}");

    let ruta_latido = ruta_del_latido(&carpeta, "Programator", "latido.json");
    let latido = Latido::cargar(&ruta_latido)
        .unwrap()
        .expect("latido debe existir");
    assert_eq!(latido.estado(), EstadoLatido::Error);
    assert!(latido
        .detalle_error()
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(matches!(desenlace, DesenlacePasada::Atendidos(_)));

    let ruta_latido = ruta_del_latido(&carpeta, "Programator", "latido.json");
    let latido = Latido::cargar(&ruta_latido)
        .unwrap()
        .expect("latido debe existir");
    assert_eq!(latido.estado(), EstadoLatido::Error);
    assert!(latido.encargo().is_some());
    assert_eq!(latido.encargo().unwrap().buzon, "claude.md");
    assert!(latido.detalle_error().is_some());
}

#[test]
fn entrega_de_propuesta_publica_en_buzon_antes_de_cerrar_y_no_duplica_al_cierre() {
    // Criterios (visto en uso real):
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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

/// Un motor de guion que tapa el buzón propio con un directorio en la primera llamada (después de
/// que la pasada lo haya leído) y lo destapa en la llamada `destapar_en`: entre medias nada se
/// puede publicar.
struct MotorQueDestapa {
    doble: MotorDoble,
    llamadas: usize,
    destapar_en: usize,
    tapon: std::path::PathBuf,
}

impl crate::motor::Motor for MotorQueDestapa {
    fn responder(&mut self, conversacion: &[crate::motor::Mensaje]) -> Resultado<Respuesta> {
        self.llamadas += 1;
        if self.llamadas == 1 {
            let _ = std::fs::remove_file(&self.tapon);
            std::fs::create_dir_all(&self.tapon).unwrap();
        }
        if self.llamadas == self.destapar_en {
            std::fs::remove_dir(&self.tapon).unwrap();
        }
        self.doble.responder(conversacion)
    }

    fn comprobar_salud(&mut self) -> crate::motor::proceso::EstadoServidor {
        self.doble.comprobar_salud()
    }

    fn ultimo_consumo(&self) -> Option<crate::motor::Consumo> {
        self.doble.ultimo_consumo()
    }
}

/// El veredicto que el primer encargo no pudo publicar no puede salir en la respuesta del segundo.
#[test]
fn el_veredicto_sin_publicar_de_un_encargo_no_pasa_al_siguiente() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nEscribe ajena.rs.\n\n## Para Programator\n\nCuenta hasta tres.\n",
    );
    let tapon = carpeta.join(".gestor/canal/programator.md");
    let motor = MotorQueDestapa {
        doble: MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "ajena.rs", "contenido": "pub fn a() {}"}),
            ),
            Respuesta::Texto("Hecha.".to_string()),
            Respuesta::Texto("Uno, dos, tres.".to_string()),
        ]),
        llamadas: 0,
        destapar_en: 3,
        tapon,
    };
    let mut motor = Some(motor);
    let mut asegurar_motor =
        move || Ok(EstadoMotor::Disponible(Box::new(motor.take().unwrap())) as _);

    ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    let buzon = buzon_propio(&carpeta);
    assert!(buzon.contains("Uno, dos, tres."), "{buzon}");
    assert!(!buzon.contains("ajena.rs"), "{buzon}");
}

/// El aviso de ocupación del primer encargo no puede salir también en la respuesta del segundo.
/// En cada conversación entran todas las novedades y, además, su propio encargo: con uno
/// largo y otro corto, solo la del largo pasa del umbral.
#[test]
fn el_aviso_de_ocupacion_de_un_encargo_no_pasa_al_siguiente() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let largo = "palabra ".repeat(LARGO_DEL_ENCARGO_QUE_OCUPA / 8);
    escribir_buzon(
        &carpeta,
        "claude.md",
        &format!("## Para Programator\n\nResume esto: {largo}\n"),
    );
    escribir_buzon(&carpeta, "codex.md", "## Para Programator\n\nDi hola.\n");
    let mut asegurar_motor = motor_con_guion(vec![
        Respuesta::Texto("Resumido.".to_string()),
        Respuesta::Texto("Hola.".to_string()),
    ]);

    ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    let buzon = buzon_propio(&carpeta);
    let (del_largo, del_corto) = buzon.split_once("Hola.").unwrap();
    assert!(
        del_largo.contains("AVISO DEL ARNÉS: la conversación ocupa"),
        "{buzon}"
    );
    let tras_el_corto = del_corto.split("\nLATIDO:").next().unwrap_or_default();
    let respuesta_corta = format!("Hola.{tras_el_corto}");
    assert!(
        !respuesta_corta.contains("AVISO DEL ARNÉS"),
        "{respuesta_corta}"
    );
}

/// Bytes del encargo largo de la prueba de ocupación: con los valores por defecto (16 384 tokens,
/// 4 bytes por token, aviso al 70 %) el umbral está en unos 45 900 bytes. El largo entra dos veces
/// en su conversación (novedad y encargo) y una en la del corto: 30 000 queda a cada lado.
const LARGO_DEL_ENCARGO_QUE_OCUPA: usize = 30_000;

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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
fn al_cerrar_encargo_se_genera_propuesta_de_poda_sin_dejar_reserva() {
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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

    // 2. La poda no deja reserva: se anotaría y se quitaría en el mismo cierre.
    let estado =
        std::fs::read_to_string(carpeta.join(".gestor/canal/estado.md")).unwrap_or_default();
    assert!(
        !estado.contains("Programator"),
        "no debe quedar ninguna reserva de Programator: {estado}"
    );

    // 3. Verificar aviso de poda en la respuesta en el buzón
    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(
        buzon.contains("**Propuesta de poda para claude:**"),
        "el buzón debe contener el aviso de poda: {buzon}"
    );
    assert!(buzon.contains(".gestor/candidatos/poda/claude.md"));
}

/// Con un buzón `Claude.MD`, la respuesta cita la ruta que se escribió, no `Claude.MD.md`.
#[test]
fn la_propuesta_de_poda_cita_las_rutas_que_se_escribieron() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "Claude.MD",
        "# Claude\n\n**LATIDO:** 10:00\n\n---\n\n## Para Programator\n\nImplementa el validador.\n\n---\n\n## Para Codex\n\nTarea pendiente.\n",
    );
    let mut asegurar_motor = motor_con_guion(vec![
        pedir("publicar", serde_json::json!({"texto": "Hecho."})),
        Respuesta::Texto("Fin.".to_string()),
    ]);

    ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(
        buzon.contains("`.gestor/candidatos/poda/claude.md`"),
        "{buzon}"
    );
    assert!(
        buzon.contains("`.gestor/candidatos/poda/claude-archivo.md`"),
        "{buzon}"
    );
    assert!(!buzon.contains(".MD.md"), "{buzon}");
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
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

/// Los ficheros de conversación que dejó la pasada en el espacio del agente.
pub(super) fn conversaciones_registradas(carpeta: &Path) -> Vec<std::path::PathBuf> {
    let espacio = crate::arnes::espacio::carpeta_del_agente(carpeta, "Programator");
    let Ok(entradas) = std::fs::read_dir(espacio) else {
        return Vec::new();
    };
    entradas
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let nombre = p.file_name().unwrap().to_string_lossy();
            nombre.starts_with("conversacion-") && nombre.ends_with(".jsonl")
        })
        .collect()
}

fn atender_un_encargo(carpeta: &Path, registro_de_conversacion: bool) {
    sembrar_registro_no_vacio(carpeta);
    escribir_buzon(
        carpeta,
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
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let ajustes = Ajustes {
        registro_de_conversacion,
        ..Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal)
    };
    ejecutar_pasada(carpeta, "Programator", &ajustes, &mut asegurar_motor).unwrap();
}

#[test]
fn sin_registro_de_conversacion_no_queda_ningun_fichero_de_conversacion() {
    let (_dir, carpeta) = carpeta_de_prueba();
    atender_un_encargo(&carpeta, false);
    assert!(conversaciones_registradas(&carpeta).is_empty());
}

#[test]
fn con_registro_de_conversacion_queda_cada_vuelta_con_su_respuesta() {
    let (_dir, carpeta) = carpeta_de_prueba();
    atender_un_encargo(&carpeta, true);

    let ficheros = conversaciones_registradas(&carpeta);
    assert_eq!(ficheros.len(), 1, "{ficheros:?}");
    let texto = std::fs::read_to_string(&ficheros[0]).unwrap();
    let respuestas: Vec<serde_json::Value> = texto
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .filter(|l| l["papel"] == "respuesta")
        .collect();
    // `publicar` cierra el encargo: la respuesta «Listo.» del guion no llega a pedirse.
    assert_eq!(respuestas.len(), 1, "{texto}");
    assert_eq!(respuestas[0]["herramienta"], "publicar");
    assert!(
        texto.contains("Revisa las puertas."),
        "el encargo debe constar: {texto}"
    );
}

/// Un revisor que atiende lo que empieza por «revisión:» y apunta lo que se le pidió.
#[derive(Default)]
struct RevisorFalso {
    /// Buzón, identidad recibida (si la hubo) y texto de cada revisión atendida.
    atendidos: Vec<(String, Option<String>, String)>,
}

impl Revisor for RevisorFalso {
    fn es_suyo(&self, texto: &str) -> bool {
        texto.trim_start().starts_with("revisión:")
    }
    fn atender(
        &mut self,
        de: &str,
        quien: Option<&crate::protocolo::destinatarios::Identidad>,
        texto: &str,
    ) -> String {
        self.atendidos.push((
            de.to_string(),
            quien.map(ToString::to_string),
            texto.trim().to_string(),
        ));
        "Revisión atendida por el arnés.".to_string()
    }
}

pub(super) fn buzon_propio(carpeta: &Path) -> String {
    std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap()
}

#[test]
fn un_encargo_de_revision_no_llega_al_modelo_ni_prepara_el_motor() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude.md",
        "# Bandeja de Claude 1\n\n## Para Programator\n\nrevisión: buscar\n",
    );
    let llamadas = std::cell::Cell::new(0u32);
    let mut asegurar_motor = || -> Resultado<EstadoMotor> {
        llamadas.set(llamadas.get() + 1);
        Err(Error::Configuracion(
            "no debía llamarse al motor".to_string(),
        ))
    };
    let mut revisor = RevisorFalso::default();
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let ajustes = Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal);

    let desenlace = ejecutar_pasada_con_revisor(
        &carpeta,
        "Programator",
        &ajustes,
        &mut asegurar_motor,
        &mut revisor,
    )
    .unwrap();

    assert!(matches!(
        desenlace,
        DesenlacePasada::Atendidos(ResumenPasada { atendidos: 1, .. })
    ));
    assert_eq!(llamadas.get(), 0);
    assert_eq!(
        revisor.atendidos,
        [(
            "claude.md".to_string(),
            Some("Claude 1".to_string()),
            "revisión: buscar".to_string()
        )]
    );
    assert!(buzon_propio(&carpeta).contains("Revisión atendida por el arnés."));
    // La respuesta de la revisión también despierta a quien la pidió.
    assert!(
        buzon_propio(&carpeta).contains("\nPara Claude 1: respuesta a revisión: buscar\n"),
        "{}",
        buzon_propio(&carpeta)
    );
    // Publicada, el latido vuelve a reposo.
    let latido = Latido::cargar(&ruta_del_latido(&carpeta, "Programator", "latido.json"))
        .unwrap()
        .expect("latido escrito");
    assert_eq!(latido.estado(), EstadoLatido::Reposo);

    let otra = ejecutar_pasada_con_revisor(
        &carpeta,
        "Programator",
        &ajustes,
        &mut asegurar_motor,
        &mut revisor,
    )
    .unwrap();
    assert!(
        matches!(otra, DesenlacePasada::SinEncargos),
        "el registro avanzó: la revisión no se repite"
    );
    assert_eq!(revisor.atendidos.len(), 1);
}

#[test]
fn con_un_encargo_normal_y_uno_de_revision_el_modelo_va_primero() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude.md",
        "## Para Programator\n\nrevisión: cerrar\n",
    );
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Programator\n\nRevisa las puertas.\n",
    );
    let mut asegurar_motor = motor_con_guion(vec![pedir(
        "publicar",
        serde_json::json!({"texto": "Puertas revisadas por el modelo."}),
    )]);
    let mut revisor = RevisorFalso::default();
    let limites = Limites::default();
    let verificacion = Verificacion::default();

    let desenlace = ejecutar_pasada_con_revisor(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal),
        &mut asegurar_motor,
        &mut revisor,
    )
    .unwrap();

    assert!(matches!(
        desenlace,
        DesenlacePasada::Atendidos(ResumenPasada { atendidos: 2, .. })
    ));
    let buzon = buzon_propio(&carpeta);
    let del_modelo = buzon.find("Puertas revisadas por el modelo.").unwrap();
    let de_la_revision = buzon.find("Revisión atendida por el arnés.").unwrap();
    assert!(del_modelo < de_la_revision, "{buzon}");
    assert_eq!(revisor.atendidos.len(), 1);
    // Sin cabecera, el revisor no recibe identidad ninguna, tampoco la que daría el nombre del
    // fichero.
    assert_eq!(revisor.atendidos[0].1, None);
}

/// `estado.md` con la plantilla del canal y una reserva ajena.
const ESTADO_CON_RESERVA_AJENA: &str = "# Estado\n\n## Reservas activas\n\n\
    - Codex, 2026-11-13 10:00: otro.rs — lo suyo\n\n## Mutaciones vivas\n\nNinguna.\n";

/// Atiende un encargo en el que el modelo reserva `src/main.rs` y luego hace lo que diga el resto
/// del guion; devuelve `estado.md` al acabar la pasada.
fn estado_tras_un_encargo_que_reserva(resto_del_guion: Vec<Respuesta>) -> String {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let ruta_estado = carpeta.join(".gestor/canal/estado.md");
    std::fs::write(&ruta_estado, ESTADO_CON_RESERVA_AJENA).unwrap();
    escribir_buzon(&carpeta, "codex.md", "## Para Programator\n\nToca main.\n");
    let mut guion = vec![pedir(
        "reservar",
        serde_json::json!({"ruta": "src/main.rs", "motivo": "voy a tocarlo"}),
    )];
    guion.extend(resto_del_guion);
    let mut asegurar_motor = motor_con_guion(guion);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(matches!(desenlace, DesenlacePasada::Atendidos(_)));
    std::fs::read_to_string(&ruta_estado).unwrap()
}

#[test]
fn la_reserva_de_un_encargo_se_quita_al_cerrarlo_publicando() {
    let estado = estado_tras_un_encargo_que_reserva(vec![
        pedir("publicar", serde_json::json!({"texto": "Hecho."})),
        Respuesta::Texto("Fin.".to_string()),
    ]);

    assert_eq!(
        estado, ESTADO_CON_RESERVA_AJENA,
        "solo se quita la suya; la ajena se queda"
    );
}

#[test]
fn la_reserva_de_un_encargo_se_quita_aunque_se_cierre_sin_publicar() {
    let estado =
        estado_tras_un_encargo_que_reserva(vec![Respuesta::Texto("Sin publicar.".to_string())]);

    assert_eq!(estado, ESTADO_CON_RESERVA_AJENA);
}

#[test]
fn la_reserva_de_un_encargo_se_quita_aunque_se_aborte() {
    let estado = estado_tras_un_encargo_que_reserva(vec![
        pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
        pedir(
            "leer_fichero",
            serde_json::json!({"ruta": "../../otra.txt"}),
        ),
    ]);

    assert_eq!(estado, ESTADO_CON_RESERVA_AJENA);
}

#[test]
fn mientras_dura_el_encargo_la_reserva_esta_anotada() {
    // Lo que se quita al cerrar tiene que haber estado puesto: si `reservar` no anotara nada, las
    // pruebas de arriba pasarían igual.
    let (dir, carpeta) = carpeta_de_prueba();
    let ruta_estado = carpeta.join(".gestor/canal/estado.md");
    std::fs::write(&ruta_estado, ESTADO_CON_RESERVA_AJENA).unwrap();
    let canal = Canal::nuevo(&carpeta, "Programator").unwrap();
    let mut repertorio = crate::arnes::herramientas::Repertorio::nuevo(
        crate::arnes::Ambito::nuevo(&carpeta).unwrap(),
        canal,
        &crate::terminal::TerminalReal,
    );

    repertorio.atender(&SolicitudHerramienta {
        nombre: "reservar".to_string(),
        argumentos: serde_json::json!({"ruta": "src/main.rs", "motivo": "voy a tocarlo"}),
    });

    let durante = std::fs::read_to_string(&ruta_estado).unwrap();
    let reservas = repertorio.tomar_reservas();
    assert_eq!(reservas.len(), 1);
    assert!(durante.lines().any(|l| l == reservas[0]));
    assert!(
        repertorio.tomar_reservas().is_empty(),
        "quien las toma las quita"
    );
    drop(dir);
}

#[test]
fn un_encargo_en_una_linea_para_programator_se_atiende_y_una_mencion_no() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude.md",
        "# Claude\n\nLATIDO: 2026-11-13 16:00:00\n\n\
         Hablé con el Director. Se lo diré a Programator más tarde.\n\n\
         LATIDO: 2026-11-13 16:05:00\n\n\
         Para Programator: revisa las puertas.\n\n\
         Para Gemini 1: verifica después.\n",
    );
    let mut asegurar_motor = motor_con_guion(vec![
        pedir(
            "publicar",
            serde_json::json!({"texto": "Puertas revisadas."}),
        ),
        Respuesta::Texto("Fin.".to_string()),
    ]);

    let desenlace = ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    match desenlace {
        DesenlacePasada::Atendidos(resumen) => assert_eq!(
            resumen.atendidos, 1,
            "solo la línea con «Para Programator» es un encargo; nombrarlo no"
        ),
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }
    let buzon = std::fs::read_to_string(carpeta.join(".gestor/canal/programator.md")).unwrap();
    assert!(buzon.contains("Puertas revisadas."));
    assert!(
        buzon.contains("hasta su latido 2026-11-13 16:05:00"),
        "{buzon}"
    );
}

#[test]
fn un_encargo_en_linea_no_propone_podar_el_buzon_de_origen() {
    // La poda trocea por `---` y un buzón del gestor de agentes no lo usa: el bloque «superado» sería
    // el buzón entero, con el encargo de Gemini dentro.
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "claude.md",
        "# Claude\n\nLATIDO: 2026-11-13 16:05:00\n\n\
         Para Programator: revisa las puertas.\n\nPara Gemini 1: verifica después.\n",
    );
    let mut asegurar_motor = motor_con_guion(vec![
        pedir("publicar", serde_json::json!({"texto": "Hecho."})),
        Respuesta::Texto("Fin.".to_string()),
    ]);

    ejecutar_pasada(
        &carpeta,
        "Programator",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    assert!(!carpeta.join(".gestor/candidatos/poda").exists());
}

/// Un motor de guion que guarda la primera conversación que recibe.
struct MotorQueGuardaLaConversacion {
    doble: MotorDoble,
    vista: std::rc::Rc<std::cell::RefCell<Vec<crate::motor::Mensaje>>>,
}

impl crate::motor::Motor for MotorQueGuardaLaConversacion {
    fn responder(&mut self, conversacion: &[crate::motor::Mensaje]) -> Resultado<Respuesta> {
        if self.vista.borrow().is_empty() {
            *self.vista.borrow_mut() = conversacion.to_vec();
        }
        self.doble.responder(conversacion)
    }

    fn comprobar_salud(&mut self) -> crate::motor::proceso::EstadoServidor {
        self.doble.comprobar_salud()
    }

    fn ultimo_consumo(&self) -> Option<crate::motor::Consumo> {
        self.doble.ultimo_consumo()
    }
}

/// El nombre de `[agente] nombre` tiene que llegar al modelo: con «Programator» fijo en la
/// llamada a `componer_contexto`, el resto de las pruebas seguiría en verde.
#[test]
fn el_modelo_recibe_el_nombre_configurado_del_agente() {
    let (_dir, carpeta) = carpeta_de_prueba();
    let mut registro = RegistroLectura::default();
    registro.delta("__semilla__", "x");
    registro
        .guardar(&ruta_del_registro(&carpeta, "Cobalto"))
        .unwrap();
    escribir_buzon(
        &carpeta,
        "codex.md",
        "## Para Cobalto\n\nCuenta hasta tres.\n",
    );
    let vista = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut motor = Some(MotorQueGuardaLaConversacion {
        doble: MotorDoble::con_guion(vec![Respuesta::Texto("Uno, dos, tres.".to_string())]),
        vista: std::rc::Rc::clone(&vista),
    });
    let mut asegurar_motor = move || -> Resultado<EstadoMotor> {
        Ok(EstadoMotor::Disponible(Box::new(motor.take().unwrap())))
    };

    ejecutar_pasada(
        &carpeta,
        "Cobalto",
        &Ajustes::con_limites(
            &Limites::default(),
            &Verificacion::default(),
            &crate::terminal::TerminalReal,
        ),
        &mut asegurar_motor,
    )
    .unwrap();

    let conversacion = vista.borrow();
    let sistema = &conversacion
        .first()
        .expect("el modelo recibió algo")
        .contenido;
    assert!(sistema.contains("Eres Cobalto,"), "{sistema}");
    assert!(!sistema.contains("Eres Programator"), "{sistema}");
}
