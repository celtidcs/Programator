//! Lo que el modelo deja anotado para el canal: `publicar` y `reservar`.
//!
//! `publicar` no escribe todavía: deja el cuerpo pendiente para que la sesión lo publique al cerrar
//! el ciclo. `reservar` anota en `estado.md` la ruta relativa que el modelo pidió, nunca la
//! absoluta, y recuerda la línea para quitarla al cerrar el encargo.

use super::auditoria::corregir_lineas_de_auditoria;
use super::error::motivo_para_el_modelo;
use super::{argumentos, mensajes, Decision, Repertorio};
use crate::motor::SolicitudHerramienta;

impl Repertorio<'_> {
    pub(super) fn publicar(&mut self, s: &SolicitudHerramienta) -> Decision {
        let texto = match argumentos::texto(s, "texto") {
            Ok(t) => t.to_string(),
            Err(d) => return d,
        };
        // Corrige, línea a línea, la cita de un hallazgo de auditoría contra el fichero real
        // (visto en uso real). No toca nada que no tenga la forma exacta del
        // formato formal: un texto sin auditorías sale idéntico.
        let texto = corregir_lineas_de_auditoria(&texto, &self.ambito);
        self.pendiente_de_publicar = Some(texto);
        Decision::Concedida(mensajes::PUBLICACION_ANOTADA.to_string())
    }

    pub(super) fn reservar(&mut self, s: &SolicitudHerramienta) -> Decision {
        let ruta = match argumentos::texto(s, "ruta") {
            Ok(r) => r,
            Err(d) => return d,
        };
        let motivo = match argumentos::texto(s, "motivo") {
            Ok(m) => m,
            Err(d) => return d,
        };

        // `Ambito::resolver` actúa aquí de portero, no de traductor: solo comprueba que la ruta cae
        // dentro del ámbito (admite ficheros que todavía no existen). Lo que se anota en
        // `estado.md` es la ruta relativa tal como la escribió el modelo, nunca la absoluta que
        // devuelve el resolutor — `estado.md` es un fichero compartido y una ruta absoluta de este
        // disco ahí sería ruido y filtraría la estructura de la máquina al canal.
        if let Err(e) = self.ambito.resolver(ruta) {
            return Decision::Denegada(motivo_para_el_modelo(ruta, &e));
        }

        let cuando = crate::protocolo::marca_de_tiempo_actual();
        let reserva = crate::protocolo::estado::Reserva {
            agente: self.canal.yo(),
            cuando: &cuando,
            ruta,
            motivo,
        };
        match crate::protocolo::anotar_reserva(&self.canal.ruta_estado(), &reserva) {
            Ok(linea) => {
                if !self.reservas.contains(&linea) {
                    self.reservas.push(linea);
                }
                Decision::Concedida(mensajes::RESERVA_ANOTADA.to_string())
            }
            // Por la misma razón que arriba: el error lleva la ruta absoluta de `estado.md` y esa
            // ruta no puede acabar en el canal si el modelo cita la denegación.
            Err(e) => Decision::Denegada(motivo_para_el_modelo(ruta, &e)),
        }
    }
}
