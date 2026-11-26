//! El bucle del agente: en cada vuelta relee la configuración si cambió el modelo, sondea el canal,
//! atiende cuando toca, suelta el modelo si lleva tiempo sin encargos, escribe el latido y duerme.
//! Aquí está el orden de la vuelta; lo que decide cada paso vive en sus submódulos.

mod atencion;
mod cola;
mod decisiones;
mod derivados;
mod latido;
mod motor_propio;

use super::configuracion::completar_el_comprobador_de_python;
use super::guia_del_equipo::republicar_la_guia_del_equipo;
use crate::VERSION;
use atencion::atender_si_toca;
use cola::hay_encargos_en_cola;
use decisiones::{
    mensaje_de_escucha, sondeo_de_la_vuelta, vuelta_siguiente, Cola, Pendiente, Vuelta,
};
use derivados::DerivadosDeLaConfig;
use latido::{registrar_latido_fin_vuelta, EstadoFinVuelta};
use motor_propio::{modelo_cargado, soltar_si_toca};
use programator::arnes::sesion::{self, DesenlacePasada, Revisor};
use programator::arranque::{self, hay_novedades, Sondeo};
use programator::config::Config;
use programator::error::Resultado;
use programator::motor::proceso;
use programator::motores::MotoresDelCiclo;
use programator::revision;
use programator::terminal::TerminalReal;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// Lo que el bucle recuerda del canal de una vuelta a la siguiente.
struct SeguimientoDelCanal {
    /// La huella del canal en la última foto, para saber si cambió.
    huella: Option<String>,
    /// La primera vuelta mira el canal aunque no haya cambiado: lo publicado antes de arrancar, o
    /// mientras Programator estaba cerrado, se atiende ya y no cuando alguien vuelva a escribir.
    vuelta: Vuelta,
    /// La vuelta anterior dejó algo para luego: un encargo que no se pudo atender porque el motor
    /// seguía cargando el modelo, o un buzón a medio escribir. Mientras sea
    /// [`Pendiente::Aplazado`], el bucle vuelve a intentarlo aunque nadie escriba en el canal.
    aplazado: Pendiente,
}

impl SeguimientoDelCanal {
    fn al_empezar() -> Self {
        Self {
            huella: None,
            vuelta: Vuelta::Primera,
            aplazado: Pendiente::Nada,
        }
    }
}

/// El bucle del agente: sondea el canal, atiende cuando toca, escribe el latido y duerme. No
/// termina: lo para Ctrl+C.
pub(super) fn vigilar_el_canal(
    ruta_config: PathBuf,
    mut config: Config,
    carpeta: &Path,
    carpeta_del_canal: &Path,
    instantanea: Option<sesion::Instantanea>,
) -> Resultado<()> {
    let mut derivados = DerivadosDeLaConfig::de(&config, carpeta);

    // El servidor que Programator arranque por su cuenta (si le toca) tiene que sobrevivir a
    // todas las vueltas del bucle: si viviera dentro de él, se cerraría al final de cada
    // iteración y con él el modelo que tanto costó cargar en VRAM. Lo comparten el motor del
    // ciclo y la revisión de modelos, que lo suelta antes de pasar el banco.
    let servidor_local: RefCell<Option<proceso::ServidorLocal>> = RefCell::new(None);

    let mut revisor = revision::atender::RevisorReal::nuevo(
        ruta_config.clone(),
        carpeta.to_path_buf(),
        derivados.ruta_latido.clone(),
        &servidor_local,
        VERSION,
        &TerminalReal,
    );

    if let Some(mensaje) = mensaje_de_escucha(instantanea, config.ciclo.intervalo_segundos) {
        println!("{mensaje}");
    }

    let mut seguimiento = SeguimientoDelCanal::al_empezar();
    // Cuándo terminó la última pasada que atendió algo: de aquí salen los minutos sin encargos
    // con los que se decide soltar el modelo (`[motor] soltar_modelo`).
    let mut ultimo_encargo = std::time::Instant::now();
    loop {
        // `revisión: cerrar` puede haber cambiado `[motor] modelo`: el motor siguiente tiene que
        // arrancar con el nuevo, y la configuración en memoria todavía dice el viejo.
        if revisor.tomar_cambio_de_modelo() {
            if let Some((nueva, nuevos_derivados)) = releer_la_configuracion(&ruta_config, carpeta)
            {
                revisor.cambiar_la_ruta_del_latido(nuevos_derivados.ruta_latido.clone());
                derivados = nuevos_derivados;
                config = nueva;
                println!("{}", revision::mensajes::configuracion_releida(&config));
                // La guía cuenta el modelo por defecto y sus cifras: con otro, hay que
                // volver a publicarla, con el registro y los modelos disponibles de ahora.
                republicar_la_guia_del_equipo(carpeta_del_canal, &config);
            }
        }

        let estado_fin_vuelta = sondear_y_atender(
            carpeta_del_canal,
            carpeta,
            &config,
            &mut seguimiento,
            || {
                let desenlace =
                    pasar_por_el_canal(carpeta, &config, &derivados, &servidor_local, &mut revisor);
                if matches!(desenlace, Ok(DesenlacePasada::Atendidos(_))) {
                    ultimo_encargo = std::time::Instant::now();
                }
                desenlace
            },
        );

        // El latido se escribe exactamente una vez por vuelta al terminar el sondeo y antes de
        // dormir, garantizando por tipos (Rust match exhaustivo) que ninguna rama futura omita
        // el registro en disco.
        soltar_si_toca(
            &config,
            &servidor_local,
            ultimo_encargo,
            seguimiento
                .aplazado
                .con_la_cola(Cola::segun(hay_encargos_en_cola(
                    carpeta,
                    &config.agente.nombre,
                ))),
        );

        registrar_latido_fin_vuelta(
            &derivados.ruta_latido,
            config.ciclo.intervalo_segundos,
            &estado_fin_vuelta,
            modelo_cargado(&servidor_local),
        );

        std::thread::sleep(std::time::Duration::from_secs(
            config.ciclo.intervalo_segundos,
        ));
    }
}

/// La configuración releída del TOML y completada, con todo lo que sale de ella: el preámbulo,
/// el recordatorio, los límites y el latido no se quedan con los del arranque.
/// `None` si no se pudo leer: se avisa y el bucle sigue con la que tenía.
fn releer_la_configuracion(
    ruta_config: &Path,
    carpeta: &Path,
) -> Option<(Config, DerivadosDeLaConfig)> {
    match Config::cargar(ruta_config) {
        Ok(mut nueva) => {
            completar_el_comprobador_de_python(&mut nueva);
            let derivados = DerivadosDeLaConfig::de(&nueva, carpeta);
            Some((nueva, derivados))
        }
        Err(fallo) => {
            eprintln!("{}", revision::mensajes::configuracion_sin_releer(&fallo));
            None
        }
    }
}

/// Una vuelta por el canal: lo sondea, dice si no se pudo leer entero y, si toca, pasa por él con
/// `pasar`.
///
/// Un fallo al leer la raíz del canal no puede tumbar el proceso. El canal puede estar en una
/// unidad de red o sincronizada, como Google Drive, donde un directorio puede quedar
/// momentáneamente inaccesible mientras se sincroniza, y Programator está pensado para vivir días:
/// salir con código 1 por un hipo de la unidad sería perder al agente hasta que alguien lo note. Se avisa por `stderr` con el motivo y
/// se vuelve a sondear en la siguiente vuelta. La decisión de aguantar es de este bucle:
/// `hay_novedades` sigue devolviendo el error para que cualquier otro que la llame pueda
/// distinguir el fallo.
fn sondear_y_atender(
    carpeta_del_canal: &Path,
    carpeta: &Path,
    config: &Config,
    seguimiento: &mut SeguimientoDelCanal,
    pasar: impl FnOnce() -> Resultado<DesenlacePasada>,
) -> EstadoFinVuelta {
    let leido = match hay_novedades(carpeta_del_canal, &mut seguimiento.huella) {
        Ok(sondeo) => sondeo,
        Err(fallo) => {
            eprintln!("{}", arranque::mensaje_fallo_sondeo_canal(&fallo));
            return EstadoFinVuelta::Error(fallo.to_string());
        }
    };
    let sondeo = sondeo_de_la_vuelta(leido, seguimiento.vuelta);
    seguimiento.vuelta = vuelta_siguiente(seguimiento.vuelta, leido);
    if sondeo == Sondeo::Ignorada {
        // Algo del canal no se dejó leer, así que esta foto no vale para comparar. Se
        // dice: un canal ilegible es un problema que hay que ver, y callarlo lo
        // convertiría en un agente que parece despierto y no se entera de nada.
        eprintln!("{}", arranque::mensaje_canal_parcialmente_ilegible());
    }
    atender_si_toca(
        sondeo,
        &mut seguimiento.aplazado,
        Cola::segun(hay_encargos_en_cola(carpeta, &config.agente.nombre)),
        config.ciclo.intervalo_segundos,
        pasar,
    )
}

/// Una pasada por el canal con los motores del ciclo y los ajustes de la configuración vigente.
fn pasar_por_el_canal(
    carpeta: &Path,
    config: &Config,
    derivados: &DerivadosDeLaConfig,
    servidor_local: &RefCell<Option<proceso::ServidorLocal>>,
    revisor: &mut dyn Revisor,
) -> Resultado<DesenlacePasada> {
    let mut motores = MotoresDelCiclo::nuevo(config, servidor_local, carpeta, &TerminalReal);
    sesion::ejecutar_pasada_con_motores(
        carpeta,
        &config.agente.nombre,
        &sesion::Ajustes::de_la_config(
            config,
            &derivados.limites,
            &config.verificacion,
            &derivados.preambulo,
            &derivados.recordatorio,
            &TerminalReal,
        ),
        &mut motores,
        revisor,
    )
}
