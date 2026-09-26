//! El encadenado del ciclo: lee el canal, decide si hay algo que atender, llama al modelo cuando
//! toca y publica el desenlace. Es la pieza que convierte las demás capas —protocolo, repertorio,
//! motor— en un agente de verdad: hasta la Tarea 16.2 todas existían probadas por separado y
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
use crate::arnes::espacio::{ruta_del_latido, ruta_del_registro};
use crate::arnes::herramientas::Repertorio;
use crate::arnes::Ambito;
use crate::config::Verificacion;
use crate::error::Resultado;
use crate::motor::Motor;
use crate::protocolo::{detectar, Canal, Delta, DetalleEncargo, Latido, RegistroLectura};
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
    /// Prefijos de ficheros del canal que no son buzones de nadie, declarados por el proyecto
    /// anfitrión. Vacío deja los de por defecto de `Canal` (`historico-`, `histórico-`).
    pub prefijos_ignorados: &'a [String],
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
}

impl<'a> Ajustes<'a> {
    /// Los ajustes mínimos: los límites del ciclo, sin verificación y con las normas completas.
    pub fn con_limites(limites: &'a Limites, verificacion: &'a Verificacion) -> Self {
        Self {
            limites,
            verificacion,
            preambulo: None,
            recordatorio: None,
            prefijos_ignorados: &[],
            contexto: 16384,
            umbral_aviso_contexto: 70,
            bytes_por_token: 4,
            tope_lectura_bytes: crate::config::tope_lectura_por_defecto(),
            fichero_latido: crate::config::FICHERO_LATIDO_POR_DEFECTO,
            intervalo_segundos: crate::config::intervalo_por_defecto(),
        }
    }
}

/// Ejecuta una pasada completa del ciclo: lee el canal, detecta encargos nuevos, los atiende y
/// publica el resultado.
///
/// `asegurar_motor` se invoca **como mucho una vez**, y solo si hay algún encargo que atender. En
/// producción decide si reutilizar un `llama-server` que ya esté escuchando, esperar a que uno en
/// marcha termine de cargar, o arrancar el propio; en las pruebas basta con envolver un
/// `MotorDoble` y devolverlo, sin tocar red ni proceso alguno.
pub fn ejecutar_pasada(
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
    asegurar_motor: &mut dyn FnMut() -> Resultado<EstadoMotor>,
) -> Resultado<DesenlacePasada> {
    // Los prefijos van aquí y no en `canal_repertorio`, más abajo: es este `canal` el que llama a
    // `buzones_ajenos` para detectar encargos (Decisión 3); el otro solo usa `ruta_estado()` y
    // nunca sondea el canal, así que filtrar ahí no evitaría que un histórico se volviera a
    // encargar.
    let canal =
        Canal::nuevo(carpeta, agente)?.con_prefijos_ignorados(ajustes.prefijos_ignorados.to_vec());
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

    // Decisión 11: el motor se prepara una sola vez por pasada, no una por encargo.
    let mut motor = match asegurar_motor() {
        Ok(EstadoMotor::Disponible(motor)) => motor,
        Ok(EstadoMotor::Esperando) => {
            // No se guarda el registro: los deltas de arriba solo viven en esta instancia en
            // memoria, así que la pasada siguiente los vuelve a calcular igual y encuentra los
            // mismos encargos.
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
    };

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
    let leido = construir_resumen_leido(&buzones);

    let ambito = Ambito::nuevo(carpeta)?;
    let canal_repertorio = Canal::nuevo(carpeta, agente)?;
    let mut repertorio = Repertorio::nuevo(ambito, canal_repertorio)
        .con_verificacion(
            ajustes.verificacion.comprobadores.clone(),
            std::time::Duration::from_secs(ajustes.verificacion.tiempo_maximo_segundos),
        )
        .con_tope_de_lectura(ajustes.tope_lectura_bytes)
        .con_leido(leido.clone());

    let mut resumen = ResumenPasada::default();
    let mut alguna_publicacion_fallo = false;

    for encargo in encargos {
        let detalle = DetalleEncargo::nuevo(&encargo.de, &encargo.texto);
        Latido::en_atencion(Local::now(), ajustes.intervalo_segundos, detalle.clone())
            .guardar_con_aviso(&ruta_latido);

        let conversacion = componer_contexto(&normas, &estado, &deltas, &encargo, &recordatorio);
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
        let desenlace = atender_encargo(
            motor.as_mut(),
            &mut repertorio,
            conversacion,
            ajustes.limites,
        );

        if let Desenlace::Abortado(ref motivo) = desenlace {
            Latido::en_error(
                Local::now(),
                ajustes.intervalo_segundos,
                Some(detalle),
                motivo.clone(),
            )
            .guardar_con_aviso(&ruta_latido);
        }

        // Decisión 7: los tres desenlaces se publican, no solo el bueno. El §2.3 del protocolo
        // exige registrar la parada: un agente que calla cuando algo le sale mal es peor que uno
        // que falla.
        let cuerpo = match &desenlace {
            Desenlace::Publicado(cuerpo) => cuerpo.as_str(),
            Desenlace::SinEntrega(texto) => texto.as_str(),
            Desenlace::Abortado(motivo) => motivo.as_str(),
        };

        // **El cuerpo lo redacta el modelo; el veredicto de sus propuestas, no.** Las propuestas se
        // publican de inmediato en el buzón al entregarse (incidencia C2 de NatureLand). Al cierre del
        // encargo sólo se adjuntan veredictos si quedó alguno sin publicar por un fallo previo.
        let cuerpo = con_veredictos(cuerpo, repertorio.veredictos_no_publicados());
        let cuerpo = con_avisos(&cuerpo, &resumen.avisos);
        let cuerpo = cuerpo.as_str();

        // Decisión 6: el desenlace sale al canal únicamente por `Canal::publicar`.
        match canal.publicar(cuerpo, &leido, &marca_de_tiempo()) {
            Ok(()) => {
                resumen.atendidos += 1;
                repertorio.marcar_todos_veredictos_publicados();
            }
            Err(fallo) => {
                // Un fallo atendiendo un encargo no puede tumbar el proceso: se avisa por stderr
                // y se sigue con el siguiente.
                eprintln!(
                    "{}",
                    publicacion::mensaje_fallo_publicar_desenlace(&encargo.de, &fallo)
                );
                resumen.fallidos += 1;
                alguna_publicacion_fallo = true;
            }
        }
    }

    // Decisión 5: si la publicación falla, el registro no avanza y el encargo se reintenta en el
    // ciclo siguiente. Es preferible responder dos veces que perder un encargo, porque lo primero
    // se ve y lo segundo no.
    if !alguna_publicacion_fallo {
        registro.guardar(&ruta_registro)?;
    }

    Ok(DesenlacePasada::Atendidos(resumen))
}

/// Texto que aporta un `Delta` para buscar encargos, si aporta alguno.
fn texto_del_delta(delta: &Delta) -> Option<&str> {
    match delta {
        Delta::Nada => None,
        Delta::Incremento(texto) | Delta::Completo(texto) => Some(texto.as_str()),
    }
}

/// La marca de tiempo local en el formato que exige el protocolo (§3.2): `2026-09-13 08:30`.
fn marca_de_tiempo() -> String {
    crate::protocolo::marca_de_tiempo_actual()
}
