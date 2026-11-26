//! La atención de un grupo de encargos con el modelo: prepara lo que comparten (normas,
//! recordatorio, estado del canal y repertorio) y, por cada encargo, compone su conversación, la
//! pasa al motor, cuenta el desenlace y compone la respuesta, que publica y cierra `cierre`.

use super::cierre::{liberar_el_encargo, proponer_la_poda, publicar_el_desenlace, Atendido};
use super::decisiones::{self, EntregaPedida};
use super::instrucciones::leer_instrucciones;
use super::pasada::Pasada;
use super::publicacion::{self, con_avisos, con_denegaciones, con_veredictos};
use super::sin_atender::fecha_de_ahora;
use super::{Ajustes, ModeloPedido, Motores, ResumenPasada};
use crate::arnes::cancelacion::cancelaciones_nuevas_de;
use crate::arnes::ciclo::{atender_encargo, CancelacionEnCurso, Desenlace, Limites};
use crate::arnes::contexto::componer_contexto;
use crate::arnes::entregas::EncargoDeLaEntrega;
use crate::arnes::espacio::{ruta_de_la_conversacion, ruta_de_las_entregas};
use crate::arnes::herramientas::Repertorio;
use crate::arnes::registro::MotorRegistrado;
use crate::arnes::registro_de_encargos::{EstadoDelEncargo, RegistroDeEncargos};
use crate::arnes::Ambito;
use crate::error::Resultado;
use crate::motor::{Mensaje, Motor};
use crate::motores::preparar::Procedencia;
use crate::protocolo::respuesta::{identidad_del_buzon, nombre_corto};
use crate::protocolo::{Canal, DetalleEncargo, Encargo, Latido, ResumenLeido};
use chrono::{DateTime, Local};
use std::collections::{BTreeSet, HashMap};

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
    fichas: &mut RegistroDeEncargos,
    resumen: &mut ResumenPasada,
) -> Resultado<()> {
    let mut atencion = Atencion::preparar(pasada, motores, motor, procedencia)?;
    // Los encargos de grupos anteriores ya usaron sus números: la marca de la conversación no se
    // repite aunque dos grupos empiecen en el mismo segundo.
    let ya_atendidos = resumen.atendidos + resumen.fallidos;

    for (indice, (encargo, pedido)) in encargos.into_iter().enumerate() {
        atencion.atender(ya_atendidos + indice + 1, encargo, pedido, fichas, resumen);
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
            .con_tiempo_de_busqueda(std::time::Duration::from_secs(
                ajustes.entregas.tiempo_de_busqueda_segundos,
            ))
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
        fichas: &mut RegistroDeEncargos,
        resumen: &mut ResumenPasada,
    ) {
        let pasada = self.pasada;
        let ajustes = pasada.ajustes;
        let terminal = ajustes.terminal;
        let detalle = DetalleEncargo::del_encargo(&encargo);
        Latido::en_atencion(Local::now(), detalle.clone())
            .guardar_con_aviso(pasada.ruta_latido, terminal);

        let mut avisos_del_encargo = avisos_de_la_eleccion(pedido, self.procedencia);
        avisos_del_encargo.extend(crate::arnes::prioridad::aviso_de_la_prioridad(
            &encargo.texto,
        ));
        if fichas.es_reatencion(&encargo) {
            avisos_del_encargo.push(publicacion::aviso_de_reatencion());
        }

        let catalogo = catalogo_para(pasada, &encargo.de);
        let conversacion = componer_contexto(
            pasada.agente,
            &self.normas,
            &self.estado,
            pasada.deltas,
            &encargo,
            &self.recordatorio,
            &catalogo,
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

        self.repertorio.empezar_encargo(
            EncargoDeLaEntrega {
                encargo: encargo.id(),
                nombre_del_encargo: nombre_corto(&encargo.texto, pasada.agente),
                de: encargo.de.clone(),
                modelo: self.motores.cargado(),
            },
            ruta_de_las_entregas(pasada.carpeta, pasada.agente),
        );
        let cancelacion = comprobar_la_cancelacion(pasada, &encargo, fichas);
        let desenlace = self.consultar_al_modelo(conversacion, numero, cancelacion);
        let leido = match desenlace {
            Desenlace::Cancelado { .. } => anotar_las_cancelaciones(pasada, &encargo, fichas),
            _ => pasada.leido.clone(),
        };
        let sin_empezar = matches!(
            desenlace,
            Desenlace::Cancelado {
                sin_empezar: true,
                ..
            }
        );

        // Lo que costó en memoria, medido ya con el encargo terminado. Si no llegó a empezar, no
        // costó nada: no se usó el modelo.
        if !sin_empezar {
            avisos_del_encargo.extend(self.motores.tras_el_encargo());
        }

        let pedida = decisiones::entrega_pedida(&encargo.texto);
        contar_el_desenlace(pasada, &encargo, &desenlace, pedida, detalle);

        let mut cuerpo = if sin_empezar {
            publicacion::aviso_cancelado_sin_empezar(&nombre_corto(&encargo.texto, pasada.agente))
        } else {
            componer_la_respuesta(&desenlace, pedida, &self.repertorio, &avisos_del_encargo)
        };
        if let Some(poda) =
            proponer_la_poda(pasada, &encargo, &desenlace, &mut self.contenidos_buzones)
        {
            cuerpo.push_str(&poda);
        }

        let atendido = Atendido {
            encargo: &encargo,
            desenlace: &desenlace,
            propuestas_antes,
            leido: &leido,
        };
        publicar_el_desenlace(
            pasada,
            &*self.motores,
            &self.repertorio,
            &atendido,
            &cuerpo,
            fichas,
            resumen,
        );
        liberar_el_encargo(pasada, &mut self.repertorio);
    }

    /// Pasa la conversación al motor y devuelve cómo terminó. Si la configuración lo pide, el
    /// motor va envuelto para dejar escrita la conversación en su fichero.
    fn consultar_al_modelo(
        &mut self,
        conversacion: Vec<Mensaje>,
        numero: usize,
        cancelacion: CancelacionEnCurso,
    ) -> Desenlace {
        let pasada = self.pasada;
        let ajustes = pasada.ajustes;
        let limites = Limites {
            cancelacion: Some(cancelacion),
            ..ajustes.limites.clone()
        };
        if ajustes.registro_de_conversacion {
            let marca = marca_de_la_conversacion(Local::now(), numero);
            let fichero = ruta_de_la_conversacion(pasada.carpeta, pasada.agente, &marca);
            let mut registrado =
                MotorRegistrado::nuevo(&mut *self.motor, fichero, ajustes.terminal);
            atender_encargo(
                &mut registrado,
                &mut self.repertorio,
                conversacion,
                &limites,
            )
        } else {
            atender_encargo(
                &mut *self.motor,
                &mut self.repertorio,
                conversacion,
                &limites,
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
/// El apartado «Lo que ya entregaste» para un encargo de `de`, con el límite de
/// `[ciclo] entregas_en_contexto`. Sin registro, o si no se puede leer, no sale: es ayuda para el
/// modelo, no algo sin lo que no se pueda atender.
fn catalogo_para(pasada: &Pasada, de: &str) -> String {
    let limite = pasada.ajustes.entregas_en_contexto;
    let ruta = ruta_de_las_entregas(pasada.carpeta, pasada.agente);
    if !ruta.exists() {
        return String::new();
    }
    match crate::arnes::entregas::disco::leer(&ruta) {
        Ok(leido) => {
            if let Some(aviso) = leido.aviso {
                pasada.ajustes.terminal.avisar(&aviso);
            }
            crate::arnes::entregas::catalogo::apartado(&leido.registro, de, limite)
        }
        Err(fallo) => {
            pasada
                .ajustes
                .terminal
                .avisar(&publicacion::mensaje_catalogo_sin_leer(&fallo));
            String::new()
        }
    }
}

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

/// La comprobación que el ciclo hace antes de cada llamada al motor: relee el buzón de quien
/// encargó, el único que puede cancelar, y mira si trae una cancelación de este encargo que el
/// registro no conocía. Devuelve la identidad de su cabecera o, sin ella, el nombre del buzón.
fn comprobar_la_cancelacion(
    pasada: &Pasada,
    encargo: &Encargo,
    fichas: &RegistroDeEncargos,
) -> CancelacionEnCurso {
    let ruta = pasada.canal.ruta_del_fichero(&encargo.de);
    let agente = pasada.agente.to_string();
    let encargo = encargo.clone();
    let conocidos = ids_conocidos(fichas);
    CancelacionEnCurso::con(move || {
        let contenido = std::fs::read_to_string(&ruta).ok()?;
        if cancelaciones_nuevas_de(&encargo, &contenido, &agente, &conocidos).is_empty() {
            return None;
        }
        Some(
            identidad_del_buzon(&contenido)
                .map(|identidad| identidad.to_string())
                .unwrap_or_else(|| encargo.de.clone()),
        )
    })
}

/// Las cancelaciones que cortaron el encargo quedan atendidas con él: su respuesta es la del
/// encargo cancelado, y la pasada siguiente no las vuelve a contestar. Devuelve lo leído con el
/// buzón de quien canceló hasta donde se leyó ahora, que es lo que dice la respuesta.
fn anotar_las_cancelaciones(
    pasada: &Pasada,
    encargo: &Encargo,
    fichas: &mut RegistroDeEncargos,
) -> ResumenLeido {
    let ruta = pasada.canal.ruta_del_fichero(&encargo.de);
    let Ok(contenido) = std::fs::read_to_string(ruta) else {
        return pasada.leido.clone();
    };
    let conocidos = ids_conocidos(fichas);
    let fecha = fecha_de_ahora();
    for cancelacion in cancelaciones_nuevas_de(encargo, &contenido, pasada.agente, &conocidos) {
        fichas.anotar(&cancelacion, &fecha, EstadoDelEncargo::Pendiente);
        fichas.terminar(&cancelacion, EstadoDelEncargo::Atendido, &fecha);
    }
    // No se guardan aquí: el cierre del encargo, justo después, guarda todas las fichas cambiadas
    // (`guardado::terminar_y_guardar` o, si no se publica, `dejar_pendiente`).
    publicacion::con_lo_leido_despues(pasada.leido, &encargo.de, &contenido)
}

fn ids_conocidos(fichas: &RegistroDeEncargos) -> BTreeSet<String> {
    fichas
        .fichas()
        .iter()
        .map(|ficha| ficha.id.clone())
        .collect()
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
        if decisiones::avisa_sin_entrega(pedida, texto) {
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

/// La respuesta a un encargo, antes de la poda: el desenlace, lo que dejó si publicó, los
/// veredictos que quedaron sin publicar, lo que el arnés denegó y los avisos del encargo.
fn componer_la_respuesta(
    desenlace: &Desenlace,
    pedida: EntregaPedida,
    repertorio: &Repertorio,
    avisos: &[String],
) -> String {
    // Los tres desenlaces se publican, no solo el bueno. El protocolo exige registrar la
    // parada: un agente que calla cuando algo le sale mal es peor que uno
    // que falla. Si no hubo entrega con herramientas, se antepone un aviso visible.
    let mut cuerpo = publicacion::cuerpo_base_desenlace(desenlace, pedida);

    // **El cuerpo lo redacta el modelo; lo que dejó el encargo, no.** Los demás desenlaces ya
    // nombran lo que quedó en disco.
    if let Desenlace::Publicado(_) = desenlace {
        cuerpo.push_str("\n\n");
        cuerpo.push_str(&publicacion::lo_que_dejo_el_encargo(
            repertorio.lo_que_dejo_el_encargo(),
        ));
    }

    // Las propuestas se publican de inmediato en el buzón al entregarse (visto en uso real). Al
    // cierre del encargo sólo se adjuntan veredictos si quedó alguno sin publicar por un fallo
    // previo.
    let cuerpo = con_veredictos(&cuerpo, repertorio.veredictos_no_publicados());
    let cuerpo = con_denegaciones(&cuerpo, repertorio.denegadas_del_encargo());
    con_avisos(&cuerpo, avisos)
}
