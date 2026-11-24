//! Cómo lleva el arnés al modelo durante la fase 1: con las mismas instrucciones y límites que el
//! ciclo, pero sin ejecutar su código y guardando siempre la conversación.

use std::path::PathBuf;
use std::time::Duration;

use crate::arnes::ciclo::Limites;
use crate::arnes::normas;
use crate::arnes::sesion::Ajustes;
use crate::banco::ejecucion::Preparacion;
use crate::config::{Config, Verificacion};
use crate::error::Resultado;
use crate::ficheros::leer;
use crate::terminal::Terminal;

/// Cómo lleva el arnés al modelo durante la fase 1: con las mismas instrucciones y límites que el
/// ciclo, pero sin ejecutar su código y guardando siempre la conversación.
pub(super) struct ArnesDelBanco {
    verificacion: Verificacion,
    limites: Limites,
    preambulo: PathBuf,
    recordatorio: PathBuf,
    normas: String,
}

impl ArnesDelBanco {
    pub(super) fn de_la_config(config: &Config) -> Resultado<Self> {
        // Mientras se pasa, el código del modelo no se ejecuta: sin comprobadores en la
        // verificación. Lo que se ejecuta son las pruebas ocultas de la fase 2, con las reglas de
        // su lugar.
        let verificacion = Verificacion::default();
        let plantilla = leer(&config.resolver(&config.agente.plantilla_normas))?;
        let normas = normas::componer(&plantilla, &verificacion.comprobadores);
        Ok(Self {
            limites: Limites::de_la_config(&config.ciclo),
            preambulo: config.resolver(&config.agente.preambulo),
            recordatorio: config.resolver(&config.agente.recordatorio),
            verificacion,
            normas,
        })
    }

    /// El banco guarda siempre la conversación de cada pasada: una nota sin el porqué no sirve
    /// para corregir nada (el bucle de reescrituras del 11/11/2026 no se pudo explicar sin ella).
    pub(super) fn ajustes<'a>(
        &'a self,
        config: &'a Config,
        terminal: &'a dyn Terminal,
    ) -> Ajustes<'a> {
        Ajustes {
            registro_de_conversacion: true,
            ..Ajustes::de_la_config(
                config,
                &self.limites,
                &self.verificacion,
                &self.preambulo,
                &self.recordatorio,
                terminal,
            )
        }
    }

    /// Cómo se prepara la carpeta de cada encargo: el agente de la configuración, el buzón del
    /// banco que encarga, las normas de este arnés y el tope de cada encargo.
    pub(super) fn preparacion<'a>(
        &'a self,
        config: &'a Config,
        buzon_que_encarga: &'a str,
        tiempo_maximo: Duration,
    ) -> Preparacion<'a> {
        Preparacion {
            agente: &config.agente.nombre,
            buzon_que_encarga,
            normas: &self.normas,
            tiempo_maximo,
        }
    }
}
