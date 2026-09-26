//! Motor de pruebas: devuelve un guion fijo. Permite comprobar el protocolo y el arnés enteros sin
//! GPU, sin red y en milisegundos.

use super::proceso::EstadoServidor;
use super::{Mensaje, Motor, Respuesta};
use crate::error::{Error, Resultado};
use std::collections::VecDeque;

pub struct MotorDoble {
    guion: VecDeque<Resultado<Respuesta>>,
    ultima: Vec<Mensaje>,
    salud: VecDeque<EstadoServidor>,
    salud_por_defecto: EstadoServidor,
}

impl MotorDoble {
    pub fn con_guion(respuestas: Vec<Respuesta>) -> Self {
        Self {
            guion: respuestas.into_iter().map(Ok).collect(),
            ultima: Vec::new(),
            salud: VecDeque::new(),
            salud_por_defecto: EstadoServidor::Listo,
        }
    }

    /// Permite especificar un guion con fallos (errores) intercalados o simulados.
    pub fn con_fallos(respuestas: Vec<Resultado<Respuesta>>) -> Self {
        Self {
            guion: respuestas.into(),
            ultima: Vec::new(),
            salud: VecDeque::new(),
            salud_por_defecto: EstadoServidor::Listo,
        }
    }

    /// Fija la secuencia de respuestas de salud que devolverá `comprobar_salud`.
    pub fn con_salud(mut self, estados: Vec<EstadoServidor>) -> Self {
        self.salud = estados.into();
        self
    }

    /// Fija el estado de salud constante por defecto.
    pub fn con_salud_fija(mut self, estado: EstadoServidor) -> Self {
        self.salud_por_defecto = estado;
        self
    }

    /// La última conversación que recibió, para comprobar qué se le enseñó al modelo.
    pub fn ultima_conversacion(&self) -> &[Mensaje] {
        &self.ultima
    }
}

impl Motor for MotorDoble {
    fn responder(&mut self, conversacion: &[Mensaje]) -> Resultado<Respuesta> {
        self.ultima = conversacion.to_vec();
        self.guion.pop_front().unwrap_or_else(|| {
            Err(Error::MotorSinRespuesta(
                "el guion del motor doble se agotó".to_string(),
            ))
        })
    }

    fn comprobar_salud(&mut self) -> EstadoServidor {
        self.salud.pop_front().unwrap_or(self.salud_por_defecto)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::{Papel, SolicitudHerramienta};

    #[test]
    fn devuelve_el_guion_en_orden() {
        let mut motor = MotorDoble::con_guion(vec![
            Respuesta::Herramienta(SolicitudHerramienta {
                nombre: "leer_fichero".to_string(),
                argumentos: serde_json::json!({ "ruta": "src/main.rs" }),
            }),
            Respuesta::Texto("He leído el fichero.".to_string()),
        ]);
        let conversacion = vec![Mensaje {
            papel: Papel::Usuario,
            contenido: "Lee src/main.rs".to_string(),
        }];

        let primera = motor.responder(&conversacion).unwrap();
        let segunda = motor.responder(&conversacion).unwrap();

        assert!(matches!(primera, Respuesta::Herramienta(ref s) if s.nombre == "leer_fichero"));
        assert!(matches!(segunda, Respuesta::Texto(ref t) if t.contains("He leído")));
    }

    #[test]
    fn un_guion_agotado_es_un_error_y_no_un_panico() {
        let mut motor = MotorDoble::con_guion(vec![]);

        let fallo = motor.responder(&[]).unwrap_err();

        assert!(matches!(fallo, crate::error::Error::MotorSinRespuesta(_)));
    }

    #[test]
    fn recuerda_la_ultima_conversacion_recibida() {
        let mut motor = MotorDoble::con_guion(vec![Respuesta::Texto("ok".to_string())]);

        motor
            .responder(&[Mensaje {
                papel: Papel::Sistema,
                contenido: "normas de la casa".to_string(),
            }])
            .unwrap();

        assert!(motor.ultima_conversacion()[0].contenido.contains("normas"));
    }

    #[test]
    fn un_guion_con_fallos_devuelve_errores_intercalados() {
        let mut motor = MotorDoble::con_fallos(vec![
            Err(Error::MotorSinRespuesta("fallo transitorio".into())),
            Ok(Respuesta::Texto("recuperado".into())),
        ]);

        let fallo = motor.responder(&[]).unwrap_err();
        assert!(matches!(fallo, Error::MotorSinRespuesta(ref m) if m.contains("transitorio")));

        let exito = motor.responder(&[]).unwrap();
        assert!(matches!(exito, Respuesta::Texto(ref t) if t == "recuperado"));
    }

    #[test]
    fn comprobar_salud_sigue_la_secuencia_y_usa_defecto() {
        let mut motor = MotorDoble::con_guion(vec![])
            .con_salud(vec![EstadoServidor::Listo, EstadoServidor::Cargando])
            .con_salud_fija(EstadoServidor::NoDisponible);

        assert_eq!(motor.comprobar_salud(), EstadoServidor::Listo);
        assert_eq!(motor.comprobar_salud(), EstadoServidor::Cargando);
        assert_eq!(motor.comprobar_salud(), EstadoServidor::NoDisponible);
        assert_eq!(motor.comprobar_salud(), EstadoServidor::NoDisponible);
    }
}
