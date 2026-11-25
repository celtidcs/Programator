//! El revisor: quien atiende, en lugar del modelo, los encargos de la revisión de modelos, y cómo
//! se publica lo que devuelve.

use super::pasada::Pasada;
use super::publicacion;
use super::respuesta::responder;
use super::ResumenPasada;
use crate::protocolo::{DetalleEncargo, Encargo, Latido};
use chrono::Local;

/// Quien atiende, en lugar del modelo, los encargos que no son para el modelo: los de la revisión
/// de modelos. El modelo no los ve nunca.
pub trait Revisor {
    /// Si el encargo lo atiende el revisor. Solo mira el texto: no toca nada.
    fn es_suyo(&self, texto: &str) -> bool;
    /// Lo atiende y devuelve el cuerpo que se publica en el buzón propio. `de` es el buzón;
    /// `quien`, la identidad de su cabecera, si la trae.
    fn atender(
        &mut self,
        de: &str,
        quien: Option<&crate::protocolo::destinatarios::Identidad>,
        texto: &str,
    ) -> String;
}

/// El revisor de quien no atiende revisiones, como el banco: todo va al modelo.
pub struct SinRevision;

impl Revisor for SinRevision {
    fn es_suyo(&self, _texto: &str) -> bool {
        false
    }
    fn atender(
        &mut self,
        _de: &str,
        _quien: Option<&crate::protocolo::destinatarios::Identidad>,
        _texto: &str,
    ) -> String {
        String::new()
    }
}

/// Atiende con el revisor sus encargos y publica lo que devuelve.
///
/// Si la publicación falla, el registro avanza igual: lo hecho queda en disco (en la revisión, en
/// `probados.toml` y en la carpeta del mes), y repetir el encargo volvería a descargar y a pasar el
/// banco, que son horas. Se avisa en la terminal.
pub(super) fn atender_con_el_revisor(
    pasada: &Pasada,
    revisor: &mut dyn Revisor,
    encargos: Vec<Encargo>,
    resumen: &mut ResumenPasada,
    modelo_cargado: Option<String>,
) {
    let terminal = pasada.ajustes.terminal;
    for encargo in encargos {
        Latido::en_atencion(
            Local::now(),
            pasada.ajustes.intervalo_segundos,
            DetalleEncargo::nuevo(&encargo.de, &encargo.texto),
        )
        .guardar_con_aviso(pasada.ruta_latido, terminal);
        let quien = pasada.canal.cabecera_de(&encargo);
        let cuerpo = revisor.atender(
            &encargo.de,
            quien.as_ref().map(|cabecera| cabecera.identidad()),
            encargo.cuerpo(pasada.agente),
        );
        match responder(
            pasada.canal,
            &cuerpo,
            pasada.leido,
            &encargo,
            pasada.recibido_a,
            terminal,
        ) {
            Ok(()) => {
                resumen.atendidos += 1;
                // Como tras un encargo del modelo: publicada, ya no se está atendiendo.
                Latido::en_reposo(Local::now(), pasada.ajustes.intervalo_segundos)
                    .con_modelo_cargado(modelo_cargado.clone())
                    .guardar_con_aviso(pasada.ruta_latido, terminal);
            }
            Err(fallo) => {
                resumen.fallidos += 1;
                terminal.avisar(&publicacion::mensaje_fallo_publicar_revision(
                    &encargo.de,
                    &fallo,
                ));
            }
        }
    }
}
