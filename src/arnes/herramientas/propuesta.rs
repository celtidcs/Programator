//! `escribir_propuesta`: la entrega de un candidato en la carpeta de propuestas del agente.
//!
//! Es lo único que el arnés escribe en nombre del modelo, y solo bajo
//! `.gestor/candidatos/<agente>/`, sin pisar nunca una entrega anterior. Escrita la propuesta, se
//! comprueba (`comprobacion`), se anota su veredicto y se avisa en el buzón del agente.

use super::candidatos::{escribir_fichero_nuevo, motivo_sin_nombre_libre, siguiente_libre};
use super::error::{self, motivo_para_el_modelo};
use super::{argumentos, mensajes, Decision, Repertorio};
use crate::arnes::entregas::confirmacion::linea_de_confirmacion;
use crate::arnes::verificacion::Veredicto;
use crate::motor::SolicitudHerramienta;
use crate::protocolo::canal::EntregaPublicada;
use crate::protocolo::carpetas::{CARPETA_DEL_GESTOR, CARPETA_DE_CANDIDATOS};
use std::path::Path;

/// La subcarpeta de la carpeta de candidatos donde van las propuestas resueltas. Se reserva: el
/// modelo no puede escribir una propuesta con ese nombre ni dentro de ella.
pub const CARPETA_DEL_ARCHIVO: &str = "archivo";

/// Donde quedan las entregas de `escribir_propuesta` del agente que se llama `agente`, relativo a
/// la carpeta de trabajo: su nombre en minúsculas, como su buzón. Con el nombre de fábrica es
/// `.gestor/candidatos/programator`. Lo usan también quienes recogen esas entregas, como el banco
/// de modelos.
pub fn carpeta_de_candidatos(agente: &str) -> String {
    format!(
        "{CARPETA_DEL_GESTOR}/{CARPETA_DE_CANDIDATOS}/{}",
        agente.to_lowercase()
    )
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

        if es_del_archivo(nombre) {
            return Decision::Denegada(mensajes::nombre_reservado(nombre, CARPETA_DEL_ARCHIVO));
        }
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
        let anotada = relativa_a(&resuelta, &permitido, &nombre_real);
        let confirmacion =
            if self.anotar_la_entrega(&anotada, &con_salto_final(contenido), &veredicto) {
                linea_de_confirmacion(&anotada)
            } else {
                String::new()
            };

        // En el canal, la propuesta se nombra como en el registro de entregas, con su ruta dentro
        // de la carpeta de candidatos: es el nombre que valen `preparar:`, `aceptada:` y
        // `rechazada:`. Con el nombre del fichero solo, una propuesta en `src/` no se encontraba.
        let veredicto_canal = veredicto.para_el_canal(&anotada);
        self.propuestas.push(nombre_real.clone());
        self.veredictos.push(veredicto_canal.clone());
        self.del_encargo.push(mensajes::linea_de_lo_que_dejo(
            &anotada,
            veredicto.en_pocas_palabras(),
        ));

        // Publicar de inmediato en el buzón al entregar la propuesta (visto en uso real)
        let ahora = crate::protocolo::marca_del_latido();
        let entrega = EntregaPublicada {
            propuesta: &anotada,
            veredicto: &veredicto_canal,
            confirmacion: &confirmacion,
        };
        if let Err(e) = self.canal.publicar_entrega(&entrega, &self.leido, &ahora) {
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

impl Repertorio<'_> {
    /// Anota la propuesta en el registro de entregas del encargo en curso, si se lleva. Si no se
    /// puede, se avisa en la terminal y la propuesta sigue: ya está en disco. Devuelve si quedó
    /// anotada: solo entonces tiene sentido pedir que se diga en qué quedó.
    fn anotar_la_entrega(
        &mut self,
        propuesta: &str,
        contenido: &str,
        veredicto: &Veredicto,
    ) -> bool {
        let Some((encargo, ruta)) = self.entregas.as_ref() else {
            return false;
        };
        let fecha = crate::protocolo::marca_del_latido();
        let mut entrada = encargo.entrada(propuesta, contenido, veredicto, &fecha);
        entrada.ya_en_el_proyecto = self.ya_en_el_proyecto(&entrada);
        match crate::arnes::entregas::disco::anotar(ruta, entrada) {
            Ok(None) => true,
            Ok(Some(aviso)) => {
                self.terminal.avisar(&aviso);
                true
            }
            Err(fallo) => {
                self.terminal
                    .avisar(&error::mensaje_fallo_anotar_entrega(propuesta, &fallo));
                false
            }
        }
    }
}

impl Repertorio<'_> {
    /// Los ficheros del proyecto que ya tienen el nombre y el contenido de la propuesta. Si no se
    /// puede buscar, ninguno: se avisa, y lo peor es una aceptación deducida de más.
    fn ya_en_el_proyecto(&self, entrada: &crate::arnes::entregas::EntradaDeEntrega) -> Vec<String> {
        let nombre = crate::arnes::entregas::deduccion::fichero_buscado(&entrada.propuesta);
        let nombres = std::collections::BTreeSet::from([nombre]);
        match crate::arnes::entregas::busqueda::buscar(
            self.ambito.raiz(),
            &nombres,
            self.tiempo_de_busqueda,
        ) {
            Ok(encontrados) => encontrados
                .into_iter()
                .filter(|f| f.huella == entrada.huella)
                .map(|f| f.ruta)
                .collect(),
            Err(fallo) => {
                self.terminal
                    .avisar(&error::mensaje_fallo_buscar_lo_que_ya_estaba(&fallo));
                Vec::new()
            }
        }
    }
}

/// Si el nombre pedido cae en la carpeta reservada del archivo: su primer tramo es `archivo`.
fn es_del_archivo(nombre: &str) -> bool {
    nombre
        .trim_start_matches(['.', '/', '\\'])
        .split(['/', '\\'])
        .next()
        .is_some_and(|tramo| tramo.eq_ignore_ascii_case(CARPETA_DEL_ARCHIVO))
}

/// El nombre de la propuesta relativo a la carpeta de candidatos, con `/`: el que se anota.
fn relativa_a(escrita: &Path, carpeta: &Path, nombre_real: &str) -> String {
    escrita
        .strip_prefix(carpeta)
        .map(|resto| {
            resto
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_else(|_| nombre_real.to_string())
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
