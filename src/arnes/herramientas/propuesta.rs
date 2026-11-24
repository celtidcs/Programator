//! `escribir_propuesta`: la entrega de un candidato en la carpeta de propuestas del agente.
//!
//! Es lo único que el arnés escribe en nombre del modelo, y solo bajo
//! `.gestor/candidatos/<agente>/`, sin pisar nunca una entrega anterior. Escrita la propuesta, se
//! comprueba (`comprobacion`), se anota su veredicto y se avisa en el buzón del agente.

use super::candidatos::{escribir_fichero_nuevo, motivo_sin_nombre_libre, siguiente_libre};
use super::error::{self, motivo_para_el_modelo};
use super::{argumentos, mensajes, Decision, Repertorio};
use crate::motor::SolicitudHerramienta;
use std::path::Path;

/// La carpeta de las propuestas de todos los agentes, relativa a la carpeta de trabajo.
const CARPETA_DE_LAS_PROPUESTAS: &str = ".gestor/candidatos";

/// Donde quedan las entregas de `escribir_propuesta` del agente que se llama `agente`, relativo a
/// la carpeta de trabajo: su nombre en minúsculas, como su buzón. Con el nombre de fábrica es
/// `.gestor/candidatos/programator`. Lo usan también quienes recogen esas entregas, como el banco
/// de modelos.
pub fn carpeta_de_candidatos(agente: &str) -> String {
    format!("{CARPETA_DE_LAS_PROPUESTAS}/{}", agente.to_lowercase())
}

impl Repertorio<'_> {
    pub(super) fn escribir_propuesta(&mut self, s: &SolicitudHerramienta) -> Decision {
        let nombre = match argumentos::texto(s, "nombre") {
            Ok(n) => n,
            Err(d) => return d,
        };
        let contenido = match argumentos::texto(s, "contenido") {
            Ok(c) => c,
            Err(d) => return d,
        };

        let carpeta = carpeta_de_candidatos(self.canal.yo());
        let relativa = format!("{carpeta}/{nombre}");
        let resuelta = match self.ambito.resolver(&relativa) {
            Ok(r) => r,
            Err(e) => return Decision::Denegada(motivo_para_el_modelo(nombre, &e)),
        };

        // Doble cinturón: además del ámbito, la ruta debe seguir bajo el directorio de candidatos.
        let permitido = match self.ambito.resolver(&carpeta) {
            Ok(p) => p,
            Err(e) => return Decision::Denegada(motivo_para_el_modelo(&carpeta, &e)),
        };
        if !resuelta.starts_with(&permitido) {
            return Decision::Denegada(mensajes::saldria_de_la_carpeta(nombre, &carpeta));
        }

        // **Nunca se borra una entrega anterior.** Si el nombre ya está ocupado se busca el
        // siguiente libre y se le dice al modelo dónde quedó. Sobrescribir costó que en el encargo
        // 006 se revisara una versión distinta de la que estaba en disco.
        let Some(resuelta) = siguiente_libre(&resuelta) else {
            return Decision::Denegada(motivo_sin_nombre_libre(nombre));
        };

        if let Some(padre) = resuelta.parent() {
            if let Err(e) = std::fs::create_dir_all(padre) {
                return Decision::Denegada(mensajes::carpeta_no_creada(&e));
            }
        }
        // Nunca a través de lo que haya con ese nombre, aunque apareciera entre buscar el nombre
        // libre y escribir.
        if let Err(e) = escribir_fichero_nuevo(&resuelta, &con_salto_final(contenido)) {
            return Decision::Denegada(mensajes::propuesta_no_escrita(&e));
        }

        // La propuesta ya está en disco, **pase lo que pase con la comprobación**. Denegar la
        // escritura por un error de compilación perdería el trabajo y gastaría intentos del tope de
        // herramientas, y una propuesta que no compila sigue sirviendo para que una persona vea por
        // dónde iba.
        let veredicto = self.verificar_propuesta(&resuelta);

        let nombre_real = nombre_real(&resuelta, nombre);

        let veredicto_canal = veredicto.para_el_canal(&nombre_real);
        self.propuestas.push(nombre_real.clone());
        self.veredictos.push(veredicto_canal.clone());

        // Publicar de inmediato en el buzón al entregar la propuesta (visto en uso real)
        let ahora = crate::protocolo::marca_del_latido();
        if let Err(e) =
            self.canal
                .publicar_entrega(&nombre_real, &veredicto_canal, &self.leido, &ahora)
        {
            self.terminal
                .avisar(&error::mensaje_fallo_publicar_entrega(&nombre_real, &e));
            self.sin_publicar.push(veredicto_canal);
        }

        Decision::Concedida(mensajes::propuesta_escrita(
            &nombre_real,
            &veredicto.para_el_modelo(),
        ))
    }
}

/// El nombre con el que quedó **de verdad** la propuesta, no el que se pidió.
///
/// Si `siguiente_libre` movió la entrega a `calculo-002.rs`, tanto el veredicto que se publica como
/// el aviso al modelo tienen que decir ese nombre. Si dijeran el pedido, quien revise buscaría el
/// fichero equivocado — el mismo fallo que costó el encargo 006.
fn nombre_real(escrita: &Path, pedido: &str) -> String {
    escrita
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| pedido.to_string())
}

/// El contenido de una propuesta acabado en salto de línea.
/// Vacío se deja vacío. Prueba 4 con Gestor: el modelo la cerró sin él.
pub(crate) fn con_salto_final(contenido: &str) -> std::borrow::Cow<'_, str> {
    if contenido.is_empty() || contenido.ends_with('\n') {
        std::borrow::Cow::Borrowed(contenido)
    } else {
        std::borrow::Cow::Owned(format!("{contenido}\n"))
    }
}
