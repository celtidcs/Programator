//! La atención de un grupo de encargos con el modelo: prepara lo que comparten (normas,
//! recordatorio, estado del canal y repertorio) y, por cada encargo, compone su conversación, la
//! pasa al motor, cuenta el desenlace y compone la respuesta, que publica y cierra `cierre`.

use super::cierre::{liberar_el_encargo, proponer_la_poda, publicar_el_desenlace, Atendido};
use super::instrucciones::leer_instrucciones;
use super::pasada::Pasada;
use super::publicacion::{self, con_avisos, con_denegaciones, con_veredictos, EntregaPedida};
use super::{Ajustes, ModeloPedido, Motores, ResumenPasada};
use crate::arnes::ciclo::{atender_encargo, Desenlace};
use crate::arnes::contexto::componer_contexto;
use crate::arnes::espacio::ruta_de_la_conversacion;
use crate::arnes::herramientas::Repertorio;
use crate::arnes::pendientes::Cola;
use crate::arnes::registro::MotorRegistrado;
use crate::arnes::Ambito;
use crate::error::Resultado;
use crate::motor::{Mensaje, Motor};
use crate::motores::preparar::Procedencia;
use crate::protocolo::{Canal, DetalleEncargo, Encargo, Latido};
use chrono::{DateTime, Local};
use std::collections::HashMap;

/// Cómo se marca el fichero del registro de una conversación: ordena por fecha al listarlos, con
/// segundos porque dos encargos de la misma pasada pueden empezar en el mismo minuto.
const FORMATO_DE_LA_MARCA_DE_LA_CONVERSACION: &str = "%Y%m%d-%H%M%S";

/// Atiende con el modelo un grupo de encargos y publica cada desenlace. Lo publicado sale de la
/// cola de pendientes; lo que no se pudo publicar entra en ella.
pub(super) fn atender_con_el_modelo(
    pasada: &Pasada,
    motores: &mut dyn Motores,
    motor: &mut dyn Motor,
    procedencia: Procedencia,
    encargos: Vec<(Encargo, ModeloPedido)>,
    cola: &mut Cola,
    resumen: &mut ResumenPasada,
) -> Resultado<()> {
    let mut atencion = Atencion::preparar(pasada, motores, motor, procedencia)?;
    // Los encargos de grupos anteriores ya usaron sus números: la marca de la conversación no se
    // repite aunque dos grupos empiecen en el mismo segundo.
    let ya_atendidos = resumen.atendidos + resumen.fallidos;

    for (indice, (encargo, pedido)) in encargos.into_iter().enumerate() {
        atencion.atender(ya_atendidos + indice + 1, encargo, pedido, cola, resumen);
    }

    Ok(())
}

/// Lo que comparten los encargos de un grupo atendido con el mismo motor.
struct Atencion<'a, 'p> {
    pasada: &'a Pasada<'p>,
    motores: &'a mut dyn Motores,
    motor: &'a mut dyn Motor,
    procedencia: Procedencia,
    normas: String,
    recordatorio: String,
    estado: String,
    /// Compartido por todo el grupo: las propuestas y los veredictos se cuentan por encargo.
    repertorio: Repertorio<'p>,
    /// Cómo quedó cada buzón de origen ya podado en este grupo.
    contenidos_buzones: HashMap<String, String>,
}

impl<'a, 'p> Atencion<'a, 'p> {
    /// Lee las normas, el recordatorio y el estado del canal, y monta el repertorio del grupo.
    fn preparar(
        pasada: &'a Pasada<'p>,
        motores: &'a mut dyn Motores,
        motor: &'a mut dyn Motor,
        procedencia: Procedencia,
    ) -> Resultado<Self> {
        let Pasada {
            carpeta,
            agente,
            ajustes,
            canal,
            leido,
            ..
        } = *pasada;
        let terminal = ajustes.terminal;
        let normas = leer_instrucciones(
            carpeta,
            agente,
            ajustes.preambulo,
            &ajustes.verificacion.comprobadores,
        )?;
        // Si no hay recordatorio, o no se deja leer, el encargo va como iba: nunca se deja de atender
        // un encargo por esto.
        let recordatorio = ajustes
            .recordatorio
            .and_then(|ruta| std::fs::read_to_string(ruta).ok())
            .unwrap_or_default();
        let estado = std::fs::read_to_string(canal.ruta_estado()).unwrap_or_default();

        let ambito = Ambito::nuevo(carpeta)?;
        let canal_repertorio = Canal::nuevo(carpeta, agente)?;
        let repertorio = Repertorio::nuevo(ambito, canal_repertorio, terminal)
            .con_ajustes_de_verificacion(ajustes.verificacion)
            .con_tope_de_lectura(ajustes.tope_lectura_bytes)
            .con_leido(leido.clone());

        Ok(Self {
            pasada,
            motores,
            motor,
            procedencia,
            normas,
            recordatorio,
            estado,
            repertorio,
            contenidos_buzones: HashMap::new(),
        })
    }

    /// Atiende un encargo de principio a fin: latido, conversación, motor, respuesta y cierre.
    /// `numero` es su orden en la pasada, que distingue el registro de su conversación.
    fn atender(
        &mut self,
        numero: usize,
        encargo: Encargo,
        pedido: ModeloPedido,
        cola: &mut Cola,
        resumen: &mut ResumenPasada,
    ) {
        let pasada = self.pasada;
        let ajustes = pasada.ajustes;
        let terminal = ajustes.terminal;
        let detalle = DetalleEncargo::nuevo(&encargo.de, &encargo.texto);
        Latido::en_atencion(Local::now(), ajustes.intervalo_segundos, detalle.clone())
            .guardar_con_aviso(pasada.ruta_latido, terminal);

        let mut avisos_del_encargo = avisos_de_la_eleccion(pedido, self.procedencia);

        let conversacion = componer_contexto(
            pasada.agente,
            &self.normas,
            &self.estado,
            pasada.deltas,
            &encargo,
            &self.recordatorio,
        );
        // El desbordamiento no avisa por sí solo: el motor rechaza y reintenta, y desde fuera
        // parece que el arnés ha dejado de atender. Esto lo dice antes de que pase.
        if let Some(aviso) = aviso_de_ocupacion(&conversacion, ajustes) {
            terminal.avisar(&aviso);
            // Solo en la respuesta de este encargo; el resumen de la pasada guarda una copia.
            resumen.avisos.push(aviso.clone());
            avisos_del_encargo.push(aviso);
        }
        // Fotografía previa a este encargo concreto: `repertorio` es compartido por toda la
        // pasada, así que sin esto no se podría saber si fue ESTE encargo el que entregó algo.
        let propuestas_antes = self.repertorio.propuestas().len();

        let desenlace = self.consultar_al_modelo(conversacion, numero);

        // Lo que costó en memoria, medido ya con el encargo terminado.
        avisos_del_encargo.extend(self.motores.tras_el_encargo());

        let pedida = publicacion::entrega_pedida(&encargo.texto);
        contar_el_desenlace(pasada, &encargo, &desenlace, pedida, detalle);

        let mut cuerpo = componer_la_respuesta(
            &desenlace,
            pedida,
            self.repertorio.veredictos_no_publicados(),
            self.repertorio.denegadas_del_encargo(),
            &avisos_del_encargo,
        );
        if let Some(poda) =
            proponer_la_poda(pasada, &encargo, &desenlace, &mut self.contenidos_buzones)
        {
            cuerpo.push_str(&poda);
        }

        let atendido = Atendido {
            encargo: &encargo,
            desenlace: &desenlace,
            propuestas_antes,
        };
        publicar_el_desenlace(
            pasada,
            &*self.motores,
            &self.repertorio,
            &atendido,
            &cuerpo,
            cola,
            resumen,
        );
        liberar_el_encargo(pasada, &mut self.repertorio);
    }

    /// Pasa la conversación al motor y devuelve cómo terminó. Si la configuración lo pide, el
    /// motor va envuelto para dejar escrita la conversación en su fichero.
    fn consultar_al_modelo(&mut self, conversacion: Vec<Mensaje>, numero: usize) -> Desenlace {
        let pasada = self.pasada;
        let ajustes = pasada.ajustes;
        if ajustes.registro_de_conversacion {
            let marca = marca_de_la_conversacion(Local::now(), numero);
            let fichero = ruta_de_la_conversacion(pasada.carpeta, pasada.agente, &marca);
            let mut registrado =
                MotorRegistrado::nuevo(&mut *self.motor, fichero, ajustes.terminal);
            atender_encargo(
                &mut registrado,
                &mut self.repertorio,
                conversacion,
                ajustes.limites,
            )
        } else {
            atender_encargo(
                &mut *self.motor,
                &mut self.repertorio,
                conversacion,
                ajustes.limites,
            )
        }
    }
}

/// Los avisos que la respuesta tiene que llevar por cómo se eligió el modelo: el del pedido, si lo
/// hay, y el de que un servidor ajeno no deja cambiar de modelo, si el encargo pidió una clase.
fn avisos_de_la_eleccion(pedido: ModeloPedido, procedencia: Procedencia) -> Vec<String> {
    let mut avisos: Vec<String> = pedido.aviso.into_iter().collect();
    if procedencia == Procedencia::Ajeno && pedido.por_clase {
        avisos.push(publicacion::aviso_servidor_ajeno());
    }
    avisos
}

/// El aviso de que la conversación va justa de contexto, si va.
fn aviso_de_ocupacion(conversacion: &[Mensaje], ajustes: &Ajustes) -> Option<String> {
    crate::arnes::contexto::aviso_de_ocupacion(
        crate::arnes::contexto::tokens_estimados(conversacion, ajustes.bytes_por_token),
        ajustes.contexto,
        ajustes.umbral_aviso_contexto,
    )
}

/// La marca del fichero donde se registra la conversación: la hora y el número del encargo.
fn marca_de_la_conversacion(ahora: DateTime<Local>, numero: usize) -> String {
    format!(
        "{}-{}",
        ahora.format(FORMATO_DE_LA_MARCA_DE_LA_CONVERSACION),
        numero
    )
}

/// Cuenta en el latido o en la terminal lo que el desenlace tiene de anómalo: un aborto deja el
/// latido en error; una entrega que falta o que no se publicó, o un encargo cortado, se avisan.
fn contar_el_desenlace(
    pasada: &Pasada,
    encargo: &Encargo,
    desenlace: &Desenlace,
    pedida: EntregaPedida,
    detalle: DetalleEncargo,
) {
    let ajustes = pasada.ajustes;
    let terminal = ajustes.terminal;
    if let Desenlace::Abortado(ref motivo) | Desenlace::FueraDePlazo(ref motivo) = desenlace {
        Latido::en_error(
            Local::now(),
            ajustes.intervalo_segundos,
            Some(detalle),
            motivo.clone(),
        )
        .guardar_con_aviso(pasada.ruta_latido, terminal);
    } else if let Desenlace::SinEntrega(texto) = desenlace {
        if publicacion::avisa_sin_entrega(pedida, texto) {
            terminal.avisar(&publicacion::aviso_sin_entrega_en_la_terminal(&encargo.de));
        }
    } else if let Desenlace::SinPublicar { propuestas, .. } = desenlace {
        terminal.avisar(&publicacion::aviso_sin_publicar_en_la_terminal(
            propuestas,
            &encargo.de,
        ));
    } else if let Desenlace::Cortado { propuestas, .. } = desenlace {
        terminal.avisar(&publicacion::aviso_cortado_en_la_terminal(
            propuestas,
            &encargo.de,
        ));
    }
}

/// La respuesta a un encargo, antes de la poda: el desenlace, los veredictos que quedaron sin
/// publicar, lo que el arnés denegó y los avisos del encargo.
fn componer_la_respuesta(
    desenlace: &Desenlace,
    pedida: EntregaPedida,
    veredictos_no_publicados: &[String],
    denegadas: &[String],
    avisos: &[String],
) -> String {
    // Los tres desenlaces se publican, no solo el bueno. El protocolo exige registrar la
    // parada: un agente que calla cuando algo le sale mal es peor que uno
    // que falla. Si no hubo entrega con herramientas, se antepone un aviso visible.
    let cuerpo_base = publicacion::cuerpo_base_desenlace(desenlace, pedida);

    // **El cuerpo lo redacta el modelo; el veredicto de sus propuestas, no.** Las propuestas se
    // publican de inmediato en el buzón al entregarse (visto en uso real). Al cierre del
    // encargo sólo se adjuntan veredictos si quedó alguno sin publicar por un fallo previo.
    let cuerpo = con_veredictos(&cuerpo_base, veredictos_no_publicados);
    let cuerpo = con_denegaciones(&cuerpo, denegadas);
    con_avisos(&cuerpo, avisos)
}
