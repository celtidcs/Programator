//! El ciclo de un encargo: solicitar, conceder o denegar, y parar cuando toca.
//!
//! Los límites no son sugerencias al modelo: son condiciones del arnés. La Regla de los Dos Intentos
//! del §2.1 y el registro obligatorio en parada del §2.3 dejan de depender de que el modelo se
//! acuerde de ellos.

use super::herramientas::{Decision, Repertorio};
use crate::motor::{Mensaje, Motor, Papel, Respuesta};

mod reintentos;
mod repeticiones;
pub use reintentos::{evaluar_reintento, DecisionReintento};
pub use repeticiones::HistorialRepeticiones;

/// Cómo terminó un encargo.
#[derive(Debug)]
pub enum Desenlace {
    /// El modelo entregó un cuerpo para publicar.
    Publicado(String),
    /// El modelo terminó de hablar sin entregar nada publicable.
    SinEntrega(String),
    /// El arnés cortó el ciclo. El motivo se publica tal cual.
    Abortado(String),
}

/// Topes que impone el arnés.
#[derive(Debug, Clone)]
pub struct Limites {
    pub max_herramientas: u32,
    pub max_denegaciones_seguidas: u32,
    pub max_repeticiones_recordadas: usize,
    pub max_reintentos_motor: u32,
    pub espera_reintento_motor: std::time::Duration,
}

impl Default for Limites {
    fn default() -> Self {
        Self {
            max_herramientas: crate::config::max_herramientas_por_defecto(),
            max_denegaciones_seguidas: crate::config::max_denegaciones_seguidas_por_defecto(),
            max_repeticiones_recordadas: crate::config::max_repeticiones_recordadas_por_defecto(),
            max_reintentos_motor: crate::config::max_reintentos_motor_por_defecto(),
            espera_reintento_motor: std::time::Duration::from_millis(0),
        }
    }
}

/// Prefijo literal con el que el arnés etiqueta, en el historial que ve el modelo, una llamada a
/// herramienta ya resuelta. Vive aquí porque `etiqueta_solicitud` es la única fuente: si el
/// formato cambiara en un sitio y no en el otro, la detección de INC-N11 dejaría de encontrarlo.
const PREFIJO_ETIQUETA_SOLICITUD: &str = "[solicito ";

/// Cómo queda en el historial, para el modelo, una llamada a herramienta ya resuelta.
fn etiqueta_solicitud(nombre: &str) -> String {
    format!("{PREFIJO_ETIQUETA_SOLICITUD}{nombre}]")
}

/// Ejecuta un encargo de principio a fin.
pub fn atender_encargo(
    motor: &mut dyn Motor,
    repertorio: &mut Repertorio,
    conversacion_inicial: Vec<Mensaje>,
    limites: &Limites,
) -> Desenlace {
    let mut conversacion = conversacion_inicial;
    let mut usadas = 0u32;
    let mut denegaciones_seguidas = 0u32;
    let mut ultima_denegada: Option<String> = None;
    let mut intentos_etiqueta_como_texto = 0u32;
    let mut historial = HistorialRepeticiones::nuevo(limites.max_repeticiones_recordadas);

    loop {
        let mut reintentos_motor = 0u32;
        let respuesta = loop {
            match motor.responder(&conversacion) {
                Ok(r) => break r,
                Err(e) => {
                    let salud = motor.comprobar_salud();
                    match evaluar_reintento(
                        reintentos_motor,
                        limites.max_reintentos_motor,
                        salud,
                        &e,
                        limites.espera_reintento_motor,
                    ) {
                        DecisionReintento::Reintentar {
                            intento_siguiente,
                            espera,
                        } => {
                            reintentos_motor = intento_siguiente;
                            if !espera.is_zero() {
                                std::thread::sleep(espera);
                            }
                        }
                        DecisionReintento::Abortar(motivo) => {
                            let detalle_entrega = estado_entrega_propuestas(repertorio);
                            return Desenlace::Abortado(format!("{motivo}. {detalle_entrega}"));
                        }
                    }
                }
            }
        };

        match respuesta {
            Respuesta::Texto(texto) => {
                if let Some(cuerpo) = repertorio.pendiente_de_publicar() {
                    return Desenlace::Publicado(cuerpo.to_string());
                }

                // El modelo escribe la etiqueta de una llamada a herramienta como texto plano en
                // vez de invocarla de verdad (INC-N11 de NatureLand): imita el rótulo que el propio
                // arnés deja en el historial tras una llamada ya resuelta. Sin esto, el encargo
                // terminaba en el acto dando SinEntrega aunque el modelo solo necesitara corregir el
                // formato de la llamada, no abandonar el encargo.
                if texto.trim_start().starts_with(PREFIJO_ETIQUETA_SOLICITUD) {
                    intentos_etiqueta_como_texto += 1;
                    if intentos_etiqueta_como_texto >= limites.max_denegaciones_seguidas {
                        let detalle_entrega = estado_entrega_propuestas(repertorio);
                        return Desenlace::Abortado(format!(
                            "🔴 Programator abortó el encargo: el modelo escribió la etiqueta de una herramienta como texto en vez de invocarla, repetidamente. {detalle_entrega}"
                        ));
                    }
                    conversacion.push(Mensaje {
                        papel: Papel::Modelo,
                        contenido: texto,
                    });
                    conversacion.push(Mensaje {
                        papel: Papel::Usuario,
                        contenido: "[Aviso del arnés: Has escrito la petición de una herramienta como texto plano, no como una llamada real. Para usar una herramienta, invócala mediante la llamada estructurada; no escribas su etiqueta.]".to_string(),
                    });
                    continue;
                }

                return Desenlace::SinEntrega(texto);
            }

            Respuesta::Herramienta(solicitud) => {
                let nombre = solicitud.nombre.clone();

                // Colapsar llamadas idénticas repetidas sin gastar cupo ni volver a ejecutar
                if let Some(salida_previa) = historial.buscar_repeticion(&solicitud) {
                    denegaciones_seguidas = 0;
                    ultima_denegada = None;
                    intentos_etiqueta_como_texto = 0;
                    conversacion.push(Mensaje {
                        papel: Papel::Modelo,
                        contenido: etiqueta_solicitud(&nombre),
                    });
                    conversacion.push(Mensaje {
                        papel: Papel::Usuario,
                        contenido: format!(
                            "Resultado de «{nombre}» (repetición):\n\n{salida_previa}\n\n[Aviso del arnés: Esta llamada es idéntica a una anterior. Se devuelve el resultado previo sin volver a ejecutarla y sin gastar cupo. Rectifica o pasa a la siguiente acción.]"
                        ),
                    });
                    continue;
                }

                usadas += 1;
                if usadas > limites.max_herramientas {
                    let detalle_entrega = estado_entrega_propuestas(repertorio);
                    return Desenlace::Abortado(format!(
                        "🔴 Programator abortó el encargo: superó las {} solicitudes de herramienta \
                         sin cerrarlo. {detalle_entrega}",
                        limites.max_herramientas
                    ));
                }

                let decision = repertorio.atender(&solicitud);

                let devuelto = match &decision {
                    Decision::Concedida(salida) => {
                        denegaciones_seguidas = 0;
                        ultima_denegada = None;
                        intentos_etiqueta_como_texto = 0;
                        historial.registrar(&solicitud, salida.clone());
                        format!("Resultado de «{nombre}»:\n\n{salida}")
                    }
                    Decision::Denegada(motivo) => {
                        // Una denegación es, igualmente, una llamada estructurada de verdad: el
                        // modelo ya ha demostrado saber invocar la herramienta, aunque con un
                        // argumento que no cuadra.
                        intentos_etiqueta_como_texto = 0;
                        if ultima_denegada.as_deref() == Some(nombre.as_str()) {
                            denegaciones_seguidas += 1;
                        } else {
                            denegaciones_seguidas = 1;
                            ultima_denegada = Some(nombre.clone());
                        }

                        if denegaciones_seguidas >= limites.max_denegaciones_seguidas {
                            let detalle_entrega = estado_entrega_propuestas(repertorio);
                            return Desenlace::Abortado(format!(
                                "🔴 Programator abortó el encargo: insistió en «{nombre}» tras una \
                                 denegación. Último motivo: {motivo}. {detalle_entrega}"
                            ));
                        }

                        format!("Denegada «{nombre}»: {motivo}. Rectifica o usa otra herramienta.")
                    }
                };

                conversacion.push(Mensaje {
                    papel: Papel::Modelo,
                    contenido: etiqueta_solicitud(&nombre),
                });
                conversacion.push(Mensaje {
                    papel: Papel::Usuario,
                    contenido: devuelto,
                });
            }
        }
    }
}

/// Describe el estado de las propuestas dejadas en disco al momento de abortar un encargo.
///
/// Si el modelo alcanzó a escribir propuestas antes de que el arnés cortara el encargo,
/// el mensaje debe reflejar fielmente lo que quedó en disco en vez de afirmar que
/// «No hay entrega» (defecto INC-B1 de NatureLand).
fn estado_entrega_propuestas(repertorio: &Repertorio) -> String {
    let propuestas = deduplicar_conservando_orden(repertorio.propuestas());
    if propuestas.is_empty() {
        "No hay entrega.".to_string()
    } else if propuestas.len() == 1 {
        format!("Quedó en disco la propuesta «{}».", propuestas[0])
    } else {
        let lista = propuestas
            .iter()
            .map(|p| format!("«{p}»"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("Quedaron en disco las propuestas: {lista}.")
    }
}

/// Deduplica los nombres de propuestas preservando el orden de aparición.
fn deduplicar_conservando_orden(propuestas: &[String]) -> Vec<String> {
    let mut unicos = Vec::new();
    for p in propuestas {
        if !unicos.contains(p) {
            unicos.push(p.clone());
        }
    }
    unicos
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::arnes::herramientas::Repertorio;
    use crate::arnes::Ambito;
    use crate::error::Error;
    use crate::motor::proceso::EstadoServidor;
    use crate::motor::{MotorDoble, Respuesta, SolicitudHerramienta};

    fn repertorio() -> (tempfile::TempDir, Repertorio) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("nota.txt"), "contenido\n").unwrap();
        for i in 0..30 {
            std::fs::write(dir.path().join(format!("nota_{i}.txt")), "contenido\n").unwrap();
        }
        let ambito = Ambito::nuevo(dir.path()).unwrap();
        let canal = crate::protocolo::Canal::nuevo(dir.path(), "Programator").unwrap();
        (dir, Repertorio::nuevo(ambito, canal))
    }

    fn pedir(nombre: &str, argumentos: serde_json::Value) -> Respuesta {
        Respuesta::Herramienta(SolicitudHerramienta {
            nombre: nombre.to_string(),
            argumentos,
        })
    }

    #[test]
    fn un_encargo_normal_termina_publicando() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir(
                "publicar",
                serde_json::json!({"texto": "He leído la nota."}),
            ),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::Publicado(ref t) if t.contains("He leído la nota")));
    }

    #[test]
    fn dos_denegaciones_iguales_seguidas_abortan_el_ciclo() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir(
                "leer_fichero",
                serde_json::json!({"ruta": "../../otra.txt"}),
            ),
            Respuesta::Texto("nunca se llega aquí".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::Abortado(ref m) if m.contains("insistió")));
    }

    #[test]
    fn una_denegacion_aislada_no_aborta_y_el_modelo_puede_rectificar() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir("listar", serde_json::json!({"ruta": "."})),
            pedir("publicar", serde_json::json!({"texto": "Rectifiqué."})),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::Publicado(_)));
    }

    #[test]
    fn el_limite_de_herramientas_corta_un_bucle_improductivo() {
        let (_dir, mut r) = repertorio();
        let guion: Vec<Respuesta> = (0..20)
            .map(|i| {
                pedir(
                    "leer_fichero",
                    serde_json::json!({"ruta": format!("nota_{i}.txt")}),
                )
            })
            .collect();
        let mut motor = MotorDoble::con_guion(guion);
        let limites = Limites {
            max_herramientas: 12,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        assert!(matches!(desenlace, Desenlace::Abortado(ref m) if m.contains("12")));
    }

    #[test]
    fn un_motor_agotado_se_registra_como_parada_y_no_como_exito() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::Abortado(_)));
    }

    #[test]
    fn terminar_sin_publicar_no_cuenta_como_publicado() {
        let (_dir, mut r) = repertorio();
        let mut motor =
            MotorDoble::con_guion(vec![Respuesta::Texto("He pensado mucho.".to_string())]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::SinEntrega(_)));
    }

    #[test]
    fn abortar_por_tope_con_propuesta_escrita_nombra_la_propuesta() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "parcial.rs", "contenido": "fn f() {}"}),
            ),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_0.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_1.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 2,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("superó las 2 solicitudes"));
                assert!(motivo.contains("parcial.rs"));
                assert!(!motivo.contains("No hay entrega"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn abortar_por_tope_sin_propuesta_dice_que_no_hay_entrega() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_0.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_1.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_2.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 2,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("superó las 2 solicitudes"));
                assert!(motivo.contains("No hay entrega"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn abortar_por_insistencia_con_propuesta_escrita_nombra_la_propuesta() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "entrega.rs", "contenido": "struct S;"}),
            ),
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir(
                "leer_fichero",
                serde_json::json!({"ruta": "../../otra.txt"}),
            ),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("insistió"));
                assert!(motivo.contains("entrega.rs"));
                assert!(!motivo.contains("No hay entrega"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn abortar_con_multiples_propuestas_las_nombra_todas() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "primera.rs", "contenido": "struct A;"}),
            ),
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "segunda.rs", "contenido": "struct B;"}),
            ),
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir(
                "leer_fichero",
                serde_json::json!({"ruta": "../../otra.txt"}),
            ),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("insistió"));
                assert!(motivo.contains("primera.rs"));
                assert!(motivo.contains("segunda.rs"));
                assert!(motivo.contains("Quedaron en disco las propuestas"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn dos_llamadas_identicas_gastan_un_solo_cupo() {
        let (_dir, mut r) = repertorio();
        // Con max_herramientas: 1, si la segunda llamada idéntica consumiera cupo,
        // la cuenta llegaría a 2 y abortaría. Como no consume cupo, termina en SinEntrega sin abortar.
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            Respuesta::Texto("Listo tras repetir.".to_string()),
        ]);
        let limites = Limites {
            max_herramientas: 1,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        assert!(
            matches!(desenlace, Desenlace::SinEntrega(ref t) if t.contains("Listo tras repetir"))
        );
    }

    #[test]
    fn segunda_llamada_identica_devuelve_lo_mismo_y_declara_repeticion() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            Respuesta::Texto("Terminado.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());
        assert!(matches!(desenlace, Desenlace::SinEntrega(_)));

        // Verificamos en la conversación del motor qué recibió el modelo en el segundo turno
        let conversacion = motor.ultima_conversacion();
        let ultimo_mensaje_usuario = conversacion
            .iter()
            .rfind(|m| m.papel == Papel::Usuario)
            .expect("debe haber mensaje de usuario");

        assert!(ultimo_mensaje_usuario.contenido.contains("repetición"));
        assert!(ultimo_mensaje_usuario.contenido.contains("contenido"));
        assert!(ultimo_mensaje_usuario
            .contenido
            .contains("Esta llamada es idéntica a una anterior"));
        assert!(ultimo_mensaje_usuario.contenido.contains("sin gastar cupo"));
    }

    #[test]
    fn dos_llamadas_con_distintos_argumentos_no_se_colapsan() {
        let (_dir, mut r) = repertorio();
        // Con max_herramientas: 1, dos llamadas al mismo verbo con distintos argumentos
        // deben consumir 1 cupo cada una y abortar en la segunda por superar el cupo.
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_0.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_1.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 1,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("superó las 1 solicitudes de herramienta"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn abortar_por_insistencia_tras_denegacion_sigue_funcionando() {
        let (_dir, mut r) = repertorio();
        // Dos llamadas idénticas que resultan en denegación NO deben colapsarse ni ocultar
        // el mecanismo de insistencia tras denegación.
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 12,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("insistió en «leer_fichero» tras una denegación"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn propuesta_con_mismo_nombre_y_distinto_contenido_no_se_colapsa_y_se_versiona() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "calc.rs", "contenido": "v1"}),
            ),
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "calc.rs", "contenido": "v2"}),
            ),
            pedir("publicar", serde_json::json!({"texto": "Entregado."})),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());
        assert!(matches!(desenlace, Desenlace::Publicado(_)));

        let propuestas = r.propuestas();
        assert_eq!(propuestas.len(), 2);
        assert_eq!(propuestas[0], "calc.rs");
        assert_eq!(propuestas[1], "calc-002.rs");
    }

    #[test]
    fn propuesta_con_mismo_nombre_y_mismo_contenido_se_colapsa_sin_versionar() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "calc.rs", "contenido": "v1"}),
            ),
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "calc.rs", "contenido": "v1"}),
            ),
            pedir("publicar", serde_json::json!({"texto": "Entregado."})),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());
        assert!(matches!(desenlace, Desenlace::Publicado(_)));

        let propuestas = r.propuestas();
        // Solo debe haber una propuesta registrada en disco, no calc-002.rs
        assert_eq!(propuestas.len(), 1);
        assert_eq!(propuestas[0], "calc.rs");
    }

    #[test]
    fn lectura_repetida_tras_mutacion_no_se_colapsa() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "p.rs", "contenido": "fn p(){}"}),
            ),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 2,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("superó las 2 solicitudes de herramienta"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn el_modelo_que_escribe_la_etiqueta_de_solicitud_como_texto_recibe_un_aviso_y_puede_rectificar(
    ) {
        let (_dir, mut r) = repertorio();
        // El modelo imita «[solicito leer_fichero]» como texto plano (INC-N11 de NatureLand) en vez
        // de invocar la herramienta de verdad. El arnés no debe darlo por terminado: debe avisarle y
        // dejarle rectificar, igual que ante una denegación.
        let mut motor = MotorDoble::con_guion(vec![
            Respuesta::Texto("[solicito leer_fichero]".to_string()),
            pedir(
                "publicar",
                serde_json::json!({"texto": "Rectifiqué tras el aviso."}),
            ),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(
            matches!(desenlace, Desenlace::Publicado(ref t) if t.contains("Rectifiqué tras el aviso"))
        );
    }

    #[test]
    fn el_aviso_por_etiqueta_como_texto_explica_que_hay_que_invocar_la_herramienta() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            Respuesta::Texto("[solicito leer_fichero]".to_string()),
            pedir("publicar", serde_json::json!({"texto": "Ya."})),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        // El primer mensaje de usuario que ve el modelo es el aviso del mimetismo.
        let conversacion = motor.ultima_conversacion();
        let aviso = conversacion
            .iter()
            .find(|m| m.papel == Papel::Usuario)
            .expect("debe haber al menos un mensaje de usuario");
        assert!(aviso.contenido.contains("texto plano"));
        assert!(aviso.contenido.contains("invóca"));
    }

    #[test]
    fn insistir_escribiendo_la_etiqueta_como_texto_aborta_igual_que_insistir_en_una_denegacion() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            Respuesta::Texto("[solicito leer_fichero]".to_string()),
            Respuesta::Texto("[solicito leer_fichero]".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(
            matches!(desenlace, Desenlace::Abortado(ref m) if m.contains("etiqueta de una herramienta como texto"))
        );
    }

    #[test]
    fn un_texto_final_que_no_empieza_por_la_etiqueta_de_solicitud_termina_sin_entrega_como_siempre()
    {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![Respuesta::Texto(
            "He terminado de pensar, pero no publico nada.".to_string(),
        )]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::SinEntrega(_)));
    }

    #[test]
    fn un_fallo_transitorio_del_motor_seguido_de_respuesta_buena_entrega_el_encargo() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_fallos(vec![
            Err(Error::MotorSinRespuesta(
                "contención de GPU con Godot".into(),
            )),
            Ok(pedir(
                "publicar",
                serde_json::json!({"texto": "Completado tras reintento."}),
            )),
            Ok(Respuesta::Texto("Listo.".to_string())),
        ])
        .con_salud_fija(EstadoServidor::Listo);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(
            matches!(desenlace, Desenlace::Publicado(ref t) if t.contains("Completado tras reintento"))
        );
    }

    #[test]
    fn un_fallo_persistente_aborta_tras_los_reintentos_configurados_y_declara_cuenta_y_salud() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_fallos(vec![
            Err(Error::MotorSinRespuesta("GPU timeout 1".into())),
            Err(Error::MotorSinRespuesta("GPU timeout 2".into())),
            Err(Error::MotorSinRespuesta("GPU timeout 3".into())),
        ])
        .con_salud_fija(EstadoServidor::Listo);

        let limites = Limites {
            max_reintentos_motor: 2,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(
                    motivo.contains("2 reintentos"),
                    "debe indicar 2 reintentos: {motivo}"
                );
                assert!(
                    motivo.contains("3 intentos en total"),
                    "debe indicar 3 intentos: {motivo}"
                );
                assert!(
                    motivo.contains("Listo"),
                    "debe indicar que la salud era Listo: {motivo}"
                );
                assert!(
                    motivo.contains("No hay entrega"),
                    "debe reflejar que no hubo entrega: {motivo}"
                );
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn un_fallo_con_servidor_no_disponible_aborta_en_primer_intento_indicando_salud() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_fallos(vec![Err(Error::MotorSinRespuesta(
            "conexión rechazada".into(),
        ))])
        .con_salud_fija(EstadoServidor::NoDisponible);

        let limites = Limites {
            max_reintentos_motor: 3,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(
                    motivo.contains("1 intento"),
                    "debe indicar 1 intento: {motivo}"
                );
                assert!(
                    motivo.contains("NoDisponible"),
                    "debe indicar NoDisponible: {motivo}"
                );
                assert!(
                    motivo.contains("conexión rechazada"),
                    "debe incluir el error: {motivo}"
                );
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn fallo_persistente_con_propuesta_escrita_nombra_la_propuesta_y_la_salud() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_fallos(vec![
            Ok(pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "avance.rs", "contenido": "fn avance(){}"}),
            )),
            Err(Error::MotorSinRespuesta("GPU pegada".into())),
            Err(Error::MotorSinRespuesta("GPU pegada de nuevo".into())),
        ])
        .con_salud_fija(EstadoServidor::Listo);

        let limites = Limites {
            max_reintentos_motor: 1,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(
                    motivo.contains("1 reintento"),
                    "cuenta de reintentos: {motivo}"
                );
                assert!(
                    motivo.contains("2 intentos en total"),
                    "total de intentos: {motivo}"
                );
                assert!(motivo.contains("Listo"), "salud del servidor: {motivo}");
                assert!(
                    motivo.contains("avance.rs"),
                    "debe nombrar la propuesta en disco: {motivo}"
                );
                assert!(
                    !motivo.contains("No hay entrega"),
                    "no debe decir que no hay entrega: {motivo}"
                );
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }
}
