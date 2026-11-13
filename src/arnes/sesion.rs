//! El encadenado del ciclo: lee el canal, decide si hay algo que atender, llama al modelo cuando
//! toca y publica el desenlace. Es la pieza que convierte las demás capas —protocolo, repertorio,
//! motor— en un agente de verdad: antes de ella todas existían probadas por separado y
//! ninguna se invocaba desde producción.
//!
//! Tres decisiones que no se repiten en cada función de aquí abajo:
//! - **El desenlace sale al canal únicamente por `Canal::publicar`.** Ninguna otra escritura al
//!   buzón, por ningún camino: es lo que neutraliza las marcas falsas que el modelo pueda haber
//!   colado en el texto (el nombre de una herramienta lo elige él, y acaba dentro del motivo de un
//!   aborto).
//! - **La primera pasada nunca atiende nada.** Solo fotografía dónde está cada buzón ajeno ahora
//!   mismo. Sobre un canal con meses de historia, atender todo lo que un día fue un encargo sería
//!   una avalancha de respuestas a cosas ya resueltas.
//! - **El motor se prepara como mucho una vez por pasada**, y solo si hay algo que atender: cargar
//!   el modelo es lo caro.

mod instrucciones;
#[cfg(test)]
mod pruebas;
mod publicacion;

use self::instrucciones::leer_instrucciones;
use self::publicacion::{con_avisos, con_veredictos, construir_resumen_leido};
use crate::arnes::ciclo::{atender_encargo, Desenlace, Limites};
use crate::arnes::contexto::componer_contexto;
use crate::arnes::espacio::{ruta_de_la_conversacion, ruta_del_latido, ruta_del_registro};
use crate::arnes::herramientas::Repertorio;
use crate::arnes::registro::MotorRegistrado;
use crate::arnes::Ambito;
use crate::config::Verificacion;
use crate::error::Resultado;

/// Cómo se marca el fichero del registro de una conversación: ordena por fecha al listarlos, con
/// segundos porque dos encargos de la misma pasada pueden empezar en el mismo minuto.
const FORMATO_DE_LA_MARCA_DE_LA_CONVERSACION: &str = "%Y%m%d-%H%M%S";
use crate::motor::Motor;
use crate::protocolo::encargo::Forma;
use crate::protocolo::{
    detectar, guardar_propuesta_poda, proponer_poda_por_cierre, retirar_reserva, Canal, Delta,
    DetalleEncargo, Encargo, Latido, RegistroLectura, ResumenLeido,
};
use chrono::Local;
use std::path::Path;

/// Lo que se obtuvo al intentar preparar el motor de inferencia para esta pasada.
pub enum EstadoMotor {
    /// Listo para responder encargos.
    Disponible(Box<dyn Motor>),
    /// Hay un servidor cargando el modelo todavía. No es un error, es «todavía no»: se reintenta
    /// en el ciclo siguiente sin arrancar nada más, porque hacerlo fallaría contra un puerto ya
    /// ocupado.
    Esperando,
}

/// Cuántos encargos se atendieron en la pasada y cuántos no llegaron a publicarse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumenPasada {
    pub atendidos: usize,
    pub fallidos: usize,
    pub avisos: Vec<String>,
}

/// Cómo terminó una pasada completa del ciclo. `main` decide qué imprimir a partir de esto.
#[derive(Debug)]
pub enum DesenlacePasada {
    /// El registro de lectura estaba vacío: se ha fotografiado el canal y no se ha llamado al
    /// modelo, para no responder de golpe a encargos que ya llevan meses resueltos.
    PrimeraPasada,
    /// Hubo novedades en el canal, pero ninguna era un encargo dirigido a este agente.
    SinEncargos,
    /// Hay un servidor cargando el modelo. Se reintenta en el ciclo siguiente sin perder los
    /// encargos detectados: el registro no avanza.
    MotorCargando,
    /// Se atendió al menos un encargo.
    Atendidos(ResumenPasada),
}

/// La configuración que gobierna una pasada, agrupada.
///
/// Iban sueltos como parámetros hasta que fueron tres y `ejecutar_pasada` empezó a pedir seis
/// argumentos posicionales, que es donde se confunden entre sí al llamar. Son lo mismo —ajustes
/// que vienen del TOML— y viajan juntos.
pub struct Ajustes<'a> {
    pub limites: &'a Limites,
    pub verificacion: &'a Verificacion,
    /// Las instrucciones de sistema que ve el modelo. `None` usa `PROGRAMATOR.md`, las normas
    /// completas, que es lo que se hacía antes de que el preámbulo existiera.
    pub preambulo: Option<&'a Path>,
    /// Lo que se le recuerda pegado al encargo. `None` no añade nada.
    ///
    /// Va aparte del preámbulo porque **el sitio importa**: medido el 22/09/2026, las mismas normas
    /// de código en el mensaje de sistema no cambian lo que escribe, y pegadas al encargo sí.
    pub recordatorio: Option<&'a Path>,
    /// Tamaño del contexto en tokens configurado para el motor. Se usa para estimar si la
    /// conversación va justa de espacio.
    pub contexto: u32,
    /// A partir de qué porcentaje de la ventana se avisa de que la conversación va justa.
    pub umbral_aviso_contexto: u32,
    /// Cuántos bytes se cuentan por token al estimar la ocupación. Es una aproximación que
    /// depende del modelo y del idioma.
    pub bytes_por_token: u32,
    /// Tope de bytes que se entregan al modelo en una lectura de fichero.
    pub tope_lectura_bytes: usize,
    /// Nombre del fichero donde se escribe el estado del arnés en cada sondeo.
    pub fichero_latido: &'a str,
    /// Segundos entre dos sondeos configurados en el ciclo.
    pub intervalo_segundos: u64,
    /// Si cada encargo deja escrita su conversación con el modelo (`[agente]
    /// registro_de_conversacion`).
    pub registro_de_conversacion: bool,
}

impl<'a> Ajustes<'a> {
    /// Los ajustes mínimos: los límites del ciclo, sin verificación y con las normas completas.
    pub fn con_limites(limites: &'a Limites, verificacion: &'a Verificacion) -> Self {
        Self {
            limites,
            verificacion,
            preambulo: None,
            recordatorio: None,
            contexto: 16384,
            umbral_aviso_contexto: 70,
            bytes_por_token: 4,
            tope_lectura_bytes: crate::config::tope_lectura_por_defecto(),
            fichero_latido: crate::config::FICHERO_LATIDO_POR_DEFECTO,
            intervalo_segundos: crate::config::intervalo_por_defecto(),
            registro_de_conversacion: false,
        }
    }

    /// Los ajustes del ciclo tal como los fija la configuración. La verificación va aparte para
    /// que quien no deba ejecutar nada en este equipo (el banco de modelos) pueda pasar una vacía.
    pub fn de_la_config(
        config: &'a crate::config::Config,
        limites: &'a Limites,
        verificacion: &'a Verificacion,
        preambulo: &'a Path,
        recordatorio: &'a Path,
    ) -> Self {
        Self {
            limites,
            verificacion,
            preambulo: Some(preambulo),
            recordatorio: Some(recordatorio),
            contexto: config.motor.contexto,
            umbral_aviso_contexto: config.motor.umbral_aviso_contexto,
            bytes_por_token: config.motor.bytes_por_token,
            tope_lectura_bytes: config.ciclo.tope_lectura_bytes,
            fichero_latido: &config.ciclo.fichero_latido,
            intervalo_segundos: config.ciclo.intervalo_segundos,
            registro_de_conversacion: config.agente.registro_de_conversacion,
        }
    }
}

/// Quien atiende, en lugar del modelo, los encargos que no son para el modelo: los de la revisión
/// de modelos (especificación de la revisión autónoma, §4). El modelo no los ve nunca.
pub trait Revisor {
    /// Si el encargo lo atiende el revisor. Solo mira el texto: no toca nada.
    fn es_suyo(&self, texto: &str) -> bool;
    /// Lo atiende y devuelve el cuerpo que se publica en el buzón propio.
    fn atender(&mut self, de: &str, texto: &str) -> String;
}

/// El revisor de quien no atiende revisiones, como el banco: todo va al modelo.
pub struct SinRevision;

impl Revisor for SinRevision {
    fn es_suyo(&self, _texto: &str) -> bool {
        false
    }
    fn atender(&mut self, _de: &str, _texto: &str) -> String {
        String::new()
    }
}

/// Ejecuta una pasada completa del ciclo sin revisor: todo encargo va al modelo.
pub fn ejecutar_pasada(
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
    asegurar_motor: &mut dyn FnMut() -> Resultado<EstadoMotor>,
) -> Resultado<DesenlacePasada> {
    ejecutar_pasada_con_revisor(carpeta, agente, ajustes, asegurar_motor, &mut SinRevision)
}

/// Ejecuta una pasada completa del ciclo: lee el canal, detecta encargos nuevos, los atiende y
/// publica el resultado.
///
/// `asegurar_motor` se invoca **como mucho una vez**, y solo si hay algún encargo para el modelo.
/// En producción decide si reutilizar un `llama-server` que ya esté escuchando, esperar a que uno
/// en marcha termine de cargar, o arrancar el propio; en las pruebas basta con envolver un
/// `MotorDoble` y devolverlo, sin tocar red ni proceso alguno.
///
/// Los encargos del `revisor` se atienden **después** de los del modelo: la revisión suelta el
/// motor para pasar el banco, y así no se lo quita a un encargo que espera en la misma pasada.
pub fn ejecutar_pasada_con_revisor(
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
    asegurar_motor: &mut dyn FnMut() -> Resultado<EstadoMotor>,
    revisor: &mut dyn Revisor,
) -> Resultado<DesenlacePasada> {
    let canal = Canal::nuevo(carpeta, agente)?;
    let ruta_registro = ruta_del_registro(carpeta, agente);
    let ruta_latido = ruta_del_latido(carpeta, agente, ajustes.fichero_latido);
    let mut registro = RegistroLectura::cargar(&ruta_registro)?;

    // Decisión 2: la primera pasada no atiende nada. Se limita a fotografiar dónde está cada
    // buzón ajeno ahora mismo, para que la próxima solo vea lo que llegue de aquí en adelante.
    if registro.esta_vacio() {
        for (nombre, contenido) in canal.buzones_ajenos()? {
            registro.delta(&nombre, &contenido);
        }
        registro.guardar(&ruta_registro)?;
        return Ok(DesenlacePasada::PrimeraPasada);
    }

    let buzones = canal.buzones_ajenos()?;
    let deltas: Vec<(String, Delta)> = buzones
        .iter()
        .map(|(nombre, contenido)| (nombre.clone(), registro.delta(nombre, contenido)))
        .collect();

    // Decisión 3: los encargos se buscan solo en lo nuevo. Con `Delta::Nada` no hay nada que
    // mirar, así que un encargo ya atendido no vuelve a aparecer.
    let mut encargos = Vec::new();
    for (nombre, delta) in &deltas {
        if let Some(texto) = texto_del_delta(delta) {
            encargos.extend(detectar(agente, nombre, texto));
        }
    }

    if encargos.is_empty() {
        registro.guardar(&ruta_registro)?;
        return Ok(DesenlacePasada::SinEncargos);
    }

    let (de_revision, para_el_modelo): (Vec<Encargo>, Vec<Encargo>) = encargos
        .into_iter()
        .partition(|encargo| revisor.es_suyo(encargo.cuerpo()));

    // Decisión 11: el motor se prepara una sola vez por pasada, no una por encargo, y solo si hay
    // algún encargo para el modelo: la revisión no lo necesita.
    let motor = if para_el_modelo.is_empty() {
        None
    } else {
        match asegurar_motor() {
            Ok(EstadoMotor::Disponible(motor)) => Some(motor),
            Ok(EstadoMotor::Esperando) => {
                // No se guarda el registro: los deltas de arriba solo viven en esta instancia en
                // memoria, así que la pasada siguiente los vuelve a calcular igual y encuentra
                // los mismos encargos. Tampoco se ha atendido aún ningún encargo de revisión.
                return Ok(DesenlacePasada::MotorCargando);
            }
            Err(fallo) => {
                Latido::en_error(
                    Local::now(),
                    ajustes.intervalo_segundos,
                    None,
                    fallo.to_string(),
                )
                .guardar_con_aviso(&ruta_latido);
                return Err(fallo);
            }
        }
    };

    // Rotar el historial anterior del buzón propio a su fichero histórico para que
    // esta pasada mantenga únicamente el último latido.
    canal.rotar_a_historico()?;

    let leido = construir_resumen_leido(&buzones);
    let pasada = Pasada {
        carpeta,
        agente,
        ajustes,
        canal: &canal,
        deltas: &deltas,
        leido: &leido,
        ruta_latido: &ruta_latido,
    };
    let mut resumen = ResumenPasada::default();
    let alguna_publicacion_fallo = match motor {
        Some(mut motor) => {
            atender_con_el_modelo(&pasada, motor.as_mut(), para_el_modelo, &mut resumen)?
        }
        None => false,
    };
    atender_con_el_revisor(&pasada, revisor, de_revision, &mut resumen);

    // Decisión 5: si la publicación falla, el registro no avanza y el encargo se reintenta en el
    // ciclo siguiente. Es preferible responder dos veces que perder un encargo, porque lo primero
    // se ve y lo segundo no.
    if !alguna_publicacion_fallo {
        registro.guardar(&ruta_registro)?;
    }

    Ok(DesenlacePasada::Atendidos(resumen))
}

/// Lo que comparten los encargos de una pasada.
struct Pasada<'a> {
    carpeta: &'a Path,
    agente: &'a str,
    ajustes: &'a Ajustes<'a>,
    canal: &'a Canal,
    deltas: &'a [(String, Delta)],
    leido: &'a ResumenLeido,
    ruta_latido: &'a Path,
}

/// Atiende con el modelo los encargos de la pasada y publica cada desenlace. Devuelve si alguna
/// publicación falló de manera que el encargo se perdería: entonces el registro no avanza.
fn atender_con_el_modelo(
    pasada: &Pasada,
    motor: &mut dyn Motor,
    encargos: Vec<Encargo>,
    resumen: &mut ResumenPasada,
) -> Resultado<bool> {
    let Pasada {
        carpeta,
        agente,
        ajustes,
        canal,
        deltas,
        leido,
        ruta_latido,
    } = *pasada;
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
    let mut repertorio = Repertorio::nuevo(ambito, canal_repertorio)
        .con_verificacion(
            ajustes.verificacion.comprobadores.clone(),
            std::time::Duration::from_secs(ajustes.verificacion.tiempo_maximo_segundos),
        )
        .con_tope_de_lectura(ajustes.tope_lectura_bytes)
        .con_leido(leido.clone());

    let mut alguna_publicacion_fallo = false;
    let mut contenidos_buzones: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for (indice, encargo) in encargos.into_iter().enumerate() {
        let detalle = DetalleEncargo::nuevo(&encargo.de, &encargo.texto);
        Latido::en_atencion(Local::now(), ajustes.intervalo_segundos, detalle.clone())
            .guardar_con_aviso(ruta_latido);

        let conversacion = componer_contexto(&normas, &estado, deltas, &encargo, &recordatorio);
        // El desbordamiento no avisa por sí solo: el motor rechaza y reintenta, y desde fuera
        // parece que el arnés ha dejado de atender. Esto lo dice antes de que pase.
        if let Some(aviso) = crate::arnes::contexto::aviso_de_ocupacion(
            crate::arnes::contexto::tokens_estimados(&conversacion, ajustes.bytes_por_token),
            ajustes.contexto,
            ajustes.umbral_aviso_contexto,
        ) {
            eprintln!("{aviso}");
            resumen.avisos.push(aviso);
        }
        // Fotografía previa a este encargo concreto: `repertorio` es compartido por toda la
        // pasada, así que sin esto no se podría saber si fue ESTE encargo el que entregó algo.
        let propuestas_antes = repertorio.propuestas().len();

        let desenlace = if ajustes.registro_de_conversacion {
            let marca = format!(
                "{}-{}",
                Local::now().format(FORMATO_DE_LA_MARCA_DE_LA_CONVERSACION),
                indice + 1
            );
            let fichero = ruta_de_la_conversacion(carpeta, agente, &marca);
            let mut registrado = MotorRegistrado::nuevo(&mut *motor, fichero);
            atender_encargo(
                &mut registrado,
                &mut repertorio,
                conversacion,
                ajustes.limites,
            )
        } else {
            atender_encargo(&mut *motor, &mut repertorio, conversacion, ajustes.limites)
        };

        if let Desenlace::Abortado(ref motivo) | Desenlace::FueraDePlazo(ref motivo) = desenlace {
            Latido::en_error(
                Local::now(),
                ajustes.intervalo_segundos,
                Some(detalle),
                motivo.clone(),
            )
            .guardar_con_aviso(ruta_latido);
        } else if let Desenlace::SinEntrega(_) = &desenlace {
            eprintln!(
                "{}",
                publicacion::aviso_sin_entrega_en_la_terminal(&encargo.de)
            );
        } else if let Desenlace::SinPublicar { propuestas, .. } = &desenlace {
            eprintln!(
                "{} (encargo de «{}»)",
                publicacion::aviso_sin_publicar(propuestas),
                encargo.de
            );
        } else if let Desenlace::Cortado { propuestas, .. } = &desenlace {
            eprintln!(
                "{} (encargo de «{}»)",
                publicacion::aviso_cortado(propuestas),
                encargo.de
            );
        }

        // Decisión 7: los tres desenlaces se publican, no solo el bueno. El §2.3 del protocolo
        // exige registrar la parada: un agente que calla cuando algo le sale mal es peor que uno
        // que falla. Si no hubo entrega con herramientas, se antepone un aviso visible.
        let cuerpo_base = publicacion::cuerpo_base_desenlace(&desenlace);

        // **El cuerpo lo redacta el modelo; el veredicto de sus propuestas, no.** Las propuestas se
        // publican de inmediato en el buzón al entregarse (visto en uso real). Al cierre del
        // encargo sólo se adjuntan veredictos si quedó alguno sin publicar por un fallo previo.
        let cuerpo = con_veredictos(&cuerpo_base, repertorio.veredictos_no_publicados());
        let mut cuerpo = con_avisos(&cuerpo, &resumen.avisos);

        // Vía A: Si el encargo concluyó (con entrega o sin entrega directa), proponer la poda
        // del buzón de origen retirando la tarea recién cerrada. Solo con un encargo bajo
        // encabezado: la poda trocea por `---`, y un buzón del gestor de agentes, donde los encargos
        // van en una línea `Para …`, no lo usa; el bloque sería el buzón entero, con los encargos
        // de los demás. Con el gestor de agentes cada uno poda su propia bandeja.
        if encargo.forma == Forma::Encabezado
            && matches!(
                desenlace,
                Desenlace::Publicado(_)
                    | Desenlace::SinEntrega(_)
                    | Desenlace::SinPublicar { .. }
                    | Desenlace::Cortado { .. }
            )
        {
            let agente_origen = encargo.de.trim_end_matches(".md");
            let contenido_origen = contenidos_buzones
                .get(&encargo.de)
                .cloned()
                .or_else(|| std::fs::read_to_string(canal.ruta_buzon(agente_origen)).ok());

            if let Some(ref texto_origen) = contenido_origen {
                if let Some(propuesta) = proponer_poda_por_cierre(texto_origen, &encargo.texto) {
                    if let Ok((_ruta_podado, _ruta_archivo)) =
                        guardar_propuesta_poda(carpeta, &encargo.de, &propuesta)
                    {
                        // Sin reserva: se anotaría y se quitaría en este mismo cierre, porque las
                        // reservas de un encargo se borran al terminarlo.
                        let rel_podado = format!(".gestor/candidatos/poda/{agente_origen}.md");
                        contenidos_buzones.insert(encargo.de.clone(), propuesta.podado);
                        cuerpo.push_str(&format!(
                            "\n\n---\n\n**Propuesta de poda para {agente_origen}:** He dejado una versión limpia de tu buzón en `{rel_podado}` y la tarea cerrada en `.gestor/candidatos/poda/{agente_origen}-archivo.md`. Puedes revisarla y aplicarla a tu buzón cuando gustes."
                        ));
                    }
                }
            }
        }

        let cuerpo = cuerpo.as_str();

        // Decisión 6: el desenlace sale al canal únicamente por `Canal::publicar`.
        match canal.publicar(cuerpo, leido, &marca_de_tiempo()) {
            Ok(()) => {
                resumen.atendidos += 1;
                repertorio.marcar_todos_veredictos_publicados();
            }
            Err(fallo) => {
                // Un fallo atendiendo un encargo no puede tumbar el proceso: se avisa por stderr
                // y se sigue con el siguiente.
                resumen.fallidos += 1;
                let propuestas_de_este_encargo = repertorio.propuestas().len() - propuestas_antes;
                if publicacion::fallo_de_cierre_pierde_el_encargo(
                    propuestas_de_este_encargo,
                    repertorio.veredictos_no_publicados().len(),
                ) {
                    eprintln!(
                        "{}",
                        publicacion::mensaje_fallo_publicar_desenlace(&encargo.de, &fallo)
                    );
                    alguna_publicacion_fallo = true;
                } else {
                    // Las propuestas de este encargo ya habían salido al canal al entregarse
                    // (`Canal::publicar_entrega`, incidencia C2): lo único que no se pudo publicar
                    // es el resumen de cierre. Reatender el encargo solo produciría una segunda
                    // respuesta que contradice a la primera (visto en uso real), así que el
                    // registro avanza igual que si el cierre hubiera salido bien.
                    eprintln!(
                        "{}",
                        publicacion::mensaje_fallo_publicar_cierre_con_entrega_ya_registrada(
                            &encargo.de,
                            &fallo
                        )
                    );
                }
            }
        }

        // El encargo está cerrado, se haya publicado o no: sus reservas se quitan.
        liberar_reservas(&canal.ruta_estado(), repertorio.tomar_reservas());
    }

    Ok(alguna_publicacion_fallo)
}

/// Quita de `estado.md` las reservas de un encargo cerrado. Si alguna no se deja quitar, se avisa
/// y se sigue: el encargo ya está atendido.
fn liberar_reservas(ruta_estado: &Path, lineas: Vec<String>) {
    for linea in lineas {
        if let Err(fallo) = retirar_reserva(ruta_estado, &linea) {
            eprintln!(
                "{}",
                publicacion::mensaje_fallo_liberar_reserva(&linea, &fallo)
            );
        }
    }
}

/// Atiende con el revisor sus encargos y publica lo que devuelve.
///
/// Si la publicación falla, el registro avanza igual: lo hecho queda en disco (en la revisión, en
/// `probados.toml` y en la carpeta del mes), y repetir el encargo volvería a descargar y a pasar el
/// banco, que son horas. Se avisa en la terminal.
fn atender_con_el_revisor(
    pasada: &Pasada,
    revisor: &mut dyn Revisor,
    encargos: Vec<Encargo>,
    resumen: &mut ResumenPasada,
) {
    for encargo in encargos {
        Latido::en_atencion(
            Local::now(),
            pasada.ajustes.intervalo_segundos,
            DetalleEncargo::nuevo(&encargo.de, &encargo.texto),
        )
        .guardar_con_aviso(pasada.ruta_latido);
        let cuerpo = revisor.atender(&encargo.de, encargo.cuerpo());
        match pasada
            .canal
            .publicar(&cuerpo, pasada.leido, &marca_de_tiempo())
        {
            Ok(()) => resumen.atendidos += 1,
            Err(fallo) => {
                resumen.fallidos += 1;
                eprintln!(
                    "{}",
                    publicacion::mensaje_fallo_publicar_revision(&encargo.de, &fallo)
                );
            }
        }
    }
}

/// Texto que aporta un `Delta` para buscar encargos, si aporta alguno.
fn texto_del_delta(delta: &Delta) -> Option<&str> {
    match delta {
        Delta::Nada => None,
        Delta::Incremento(texto) | Delta::Completo(texto) => Some(texto.as_str()),
    }
}

/// La hora del latido con que se publica en el buzón, con segundos (formato del canal).
fn marca_de_tiempo() -> String {
    crate::protocolo::marca_del_latido()
}
