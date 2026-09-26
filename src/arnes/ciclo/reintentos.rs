//! Gestión pura de reintentos ante fallos del motor de inferencia.
//!
//! Cuando `motor.responder()` falla (por ejemplo por contención de GPU con otros procesos como
//! renderizado o captura gráfica), no se aborta a la primera: se comprueba la salud del servidor
//! y, si responde, se reintenta hasta el número de veces configurado.
//!
//! **Restricción de diseño (§4.2):** Este módulo no duerme (`thread::sleep`) ni lee el reloj.
//! Devuelve una `DecisionReintento` pura que el orquestador (`atender_encargo`) ejecuta. Esto
//! permite probar exhaustivamente toda la lógica de reintentos de forma instantánea y determinista.

use crate::motor::proceso::EstadoServidor;
use std::time::Duration;

/// Resultado de evaluar si corresponde reintentar o abortar tras un fallo del motor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionReintento {
    /// Corresponde reintentar la llamada al motor tras esperar la duración indicada.
    Reintentar {
        /// Número ordinal del próximo intento (1-indexed: 1 es el primer reintento).
        intento_siguiente: u32,
        /// Cuánto tiempo debe esperar el arnés antes de ejecutar el reintento.
        espera: Duration,
    },
    /// No se debe reintentar: se corta el encargo y se aborta con el mensaje explicativo.
    Abortar(String),
}

/// Evalúa si corresponde reintentar la llamada al motor o abortar el encargo.
///
/// Parámetros:
/// - `reintentos_realizados`: cuántos reintentos se han ejecutado ya en esta llamada (0 en el primer fallo).
/// - `max_reintentos`: tope de reintentos configurado en `programator.toml`.
/// - `salud`: resultado de consultar la salud del motor (`motor.comprobar_salud()`).
/// - `error`: el error producido por `motor.responder()`.
/// - `espera`: duración de la pausa entre reintentos configurada en el arnés.
pub fn evaluar_reintento(
    reintentos_realizados: u32,
    max_reintentos: u32,
    salud: EstadoServidor,
    error: &crate::error::Error,
    espera: Duration,
) -> DecisionReintento {
    match salud {
        EstadoServidor::NoDisponible => {
            let intentos_totales = reintentos_realizados + 1;
            let detalle_intentos = if intentos_totales == 1 {
                "1 intento".to_string()
            } else if reintentos_realizados == 1 {
                "1 reintento (2 intentos en total)".to_string()
            } else {
                format!("{reintentos_realizados} reintentos ({intentos_totales} intentos en total)")
            };
            DecisionReintento::Abortar(format!(
                "🔴 Programator no pudo completar el encargo: el motor no respondió tras {detalle_intentos} y la comprobación de salud indica que {salud}. Último error del motor: {error}"
            ))
        }
        EstadoServidor::Listo | EstadoServidor::Cargando => {
            if reintentos_realizados < max_reintentos {
                DecisionReintento::Reintentar {
                    intento_siguiente: reintentos_realizados + 1,
                    espera,
                }
            } else {
                let intentos_totales = reintentos_realizados + 1;
                let detalle_intentos = if reintentos_realizados == 0 {
                    "0 reintentos (1 intento en total)".to_string()
                } else if reintentos_realizados == 1 {
                    "1 reintento (2 intentos en total)".to_string()
                } else {
                    format!(
                        "{reintentos_realizados} reintentos ({intentos_totales} intentos en total)"
                    )
                };
                DecisionReintento::Abortar(format!(
                    "🔴 Programator no pudo completar el encargo: el motor no respondió tras {detalle_intentos}. Última comprobación de salud: {salud}. Último error del motor: {error}"
                ))
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::error::Error;

    #[test]
    fn primer_fallo_con_servidor_listo_decide_reintentar() {
        let error = Error::MotorSinRespuesta("timeout de lectura".into());
        let decision = evaluar_reintento(
            0,
            2,
            EstadoServidor::Listo,
            &error,
            Duration::from_millis(50),
        );

        assert_eq!(
            decision,
            DecisionReintento::Reintentar {
                intento_siguiente: 1,
                espera: Duration::from_millis(50),
            }
        );
    }

    #[test]
    fn primer_fallo_con_servidor_cargando_decide_reintentar() {
        let error = Error::MotorSinRespuesta("503 Service Unavailable".into());
        let decision = evaluar_reintento(
            0,
            3,
            EstadoServidor::Cargando,
            &error,
            Duration::from_secs(2),
        );

        assert_eq!(
            decision,
            DecisionReintento::Reintentar {
                intento_siguiente: 1,
                espera: Duration::from_secs(2),
            }
        );
    }

    #[test]
    fn fallo_con_servidor_no_disponible_aborta_indicando_salud_e_intentos() {
        let error = Error::MotorSinRespuesta("conexión rechazada".into());
        let decision = evaluar_reintento(
            0,
            3,
            EstadoServidor::NoDisponible,
            &error,
            Duration::from_secs(5),
        );

        match decision {
            DecisionReintento::Abortar(motivo) => {
                assert!(motivo.contains("1 intento"));
                assert!(motivo.contains("NoDisponible"));
                assert!(motivo.contains("conexión rechazada"));
            }
            otro => panic!("se esperaba Abortar, se obtuvo {otro:?}"),
        }
    }

    #[test]
    fn agotar_reintentos_configurados_aborta_indicando_cuenta_y_salud() {
        let error = Error::MotorSinRespuesta("GPU timeout".into());
        // Ya se hicieron 2 reintentos (max_reintentos = 2)
        let decision =
            evaluar_reintento(2, 2, EstadoServidor::Listo, &error, Duration::from_secs(5));

        match decision {
            DecisionReintento::Abortar(motivo) => {
                assert!(motivo.contains("2 reintentos"));
                assert!(motivo.contains("3 intentos en total"));
                assert!(motivo.contains("Listo"));
                assert!(motivo.contains("GPU timeout"));
            }
            otro => panic!("se esperaba Abortar, se obtuvo {otro:?}"),
        }
    }

    #[test]
    fn cero_reintentos_configurados_aborta_en_primer_fallo() {
        let error = Error::MotorSinRespuesta("fallo inmediato".into());
        let decision =
            evaluar_reintento(0, 0, EstadoServidor::Listo, &error, Duration::from_secs(5));

        match decision {
            DecisionReintento::Abortar(motivo) => {
                assert!(motivo.contains("0 reintentos"));
                assert!(motivo.contains("1 intento en total"));
                assert!(motivo.contains("Listo"));
            }
            otro => panic!("se esperaba Abortar, se obtuvo {otro:?}"),
        }
    }

    #[test]
    fn reintento_intermedio_incrementa_intento_siguiente() {
        let error = Error::MotorSinRespuesta("contención".into());
        let decision = evaluar_reintento(
            1,
            3,
            EstadoServidor::Listo,
            &error,
            Duration::from_millis(100),
        );

        assert_eq!(
            decision,
            DecisionReintento::Reintentar {
                intento_siguiente: 2,
                espera: Duration::from_millis(100),
            }
        );
    }
}
