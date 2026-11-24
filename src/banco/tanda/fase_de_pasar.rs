//! La fase 1 de una tanda con el motor de verdad: abre la carpeta de la tanda, mide el modelo,
//! carga el motor, pasa cada encargo con cada semilla guardándolo en disco en cuanto termina y
//! apaga el motor si lo arrancó. Cada paso es una función con nombre; el recorrido en sí está en
//! `recorrido`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::arnes_del_banco::ArnesDelBanco;
use super::datos_de_la_tanda::{carpeta_de_la_tanda, datos_de_la_tanda, Medido};
use super::recorrido::{pasar_encargos, Recorrido};
use super::version_del_motor::compilacion_del_motor;
use super::{Pasado, CARPETA_DE_TRABAJO};
use crate::banco::comprobacion::local::CARPETA_DE_COMPROBACION;
use crate::banco::config::ConfigBanco;
use crate::banco::ejecucion::Reloj;
use crate::banco::ficha::Encargo;
use crate::banco::lugar::{hay_codigo_que_comprobar, LugarElegido};
use crate::banco::mensajes;
use crate::banco::pasadas::{self, PasadaGuardada, PasadasDeTanda};
use crate::banco::resultados::DatosDeTanda;
use crate::config::{Config, Muestreo};
use crate::error::{Error, Resultado};
use crate::ficheros::{crear_carpeta, huella_sha256};
use crate::motor::proceso::ServidorLocal;
use crate::motores::preparar::{
    asegurar_motor_con_muestreo, cargar_midiendo, Carga, DesenlaceDeCarga,
};
use crate::terminal::Terminal;

/// Lo que deja la fase 1 para la fase 2.
pub(super) struct TandaPasada<'a> {
    pub(super) carpeta: PathBuf,
    pub(super) pasados: Vec<Pasado<'a>>,
    pub(super) datos: DatosDeTanda,
}

/// El banco ya leído y probado: su carpeta, su configuración, sus encargos y dónde se comprobará.
/// Los encargos llevan su propia vida, `'a`, porque la tanda pasada los sigue mirando.
#[derive(Clone, Copy)]
pub(super) struct BancoCargado<'b, 'a> {
    pub(super) carpeta: &'b Path,
    pub(super) config: &'b ConfigBanco,
    pub(super) encargos: &'a [Encargo],
    pub(super) lugar: &'b LugarElegido<'b>,
}

/// La fase 1: carga el motor, pasa cada encargo con cada semilla, lo guarda en disco y apaga el
/// motor si lo arrancó. `reloj` mide la carga del motor y el tiempo máximo de cada encargo.
pub(super) fn pasar_con_el_motor<'a>(
    config: &Config,
    el_banco: BancoCargado<'_, 'a>,
    version: &str,
    reloj: &mut dyn Reloj,
    terminal: &dyn Terminal,
) -> Resultado<TandaPasada<'a>> {
    let BancoCargado {
        carpeta: carpeta_banco,
        config: banco,
        encargos,
        lugar,
    } = el_banco;
    let config = &banco.para_la_tanda(config);
    let (ruta_modelo, nombre_modelo) = modelo_que_se_mide(config)?;

    let ahora = chrono::Local::now();
    let carpeta = carpeta_de_la_tanda(carpeta_banco, &ahora, &nombre_modelo);
    let trabajo = carpeta.join(CARPETA_DE_TRABAJO);
    crear_carpeta(&trabajo)?;
    contar_el_inicio(encargos, banco, lugar, &carpeta, terminal);

    terminal.avisar(&mensajes::mensaje_calculando_huella(&ruta_modelo));
    let huella = huella_sha256(&ruta_modelo)?;
    let binario = config.motor.binario.as_ref().map(|b| config.resolver(b));
    let compilacion = compilacion_del_motor(binario.as_deref());

    let tope = Duration::from_secs(banco.tiempo_maximo_encargo_segundos);
    let mut servidor = None;
    let carga = cargar_el_motor(
        config,
        &muestreo_de_la_semilla(config, banco, config.muestreo.semilla),
        &mut servidor,
        tope,
        reloj,
        terminal,
    )?;

    let datos = datos_de_la_tanda(
        Medido {
            modelo: nombre_modelo,
            huella_sha256: huella,
            compilacion_motor: compilacion,
            carga: carga.as_ref(),
        },
        version,
        &ahora,
        banco,
        lugar,
        config,
    );
    // Lo de la GPU va a disco en cuanto se mide: si la fase 2 falla, no se pierde.
    let mut guardadas = PasadasDeTanda {
        tanda: datos.clone(),
        pasadas: Vec::new(),
    };
    pasadas::guardar(&guardadas, &carpeta)?;

    let arnes = ArnesDelBanco::de_la_config(config)?;
    let pasados = pasar_encargos(
        &Recorrido {
            encargos,
            semillas: &banco.semillas,
            carpeta_de_trabajo: &trabajo,
        },
        &arnes.preparacion(config, &banco.buzon_que_encarga, tope),
        &arnes.ajustes(config, terminal),
        &mut |semilla| {
            asegurar_motor_con_muestreo(
                config,
                &muestreo_de_la_semilla(config, banco, semilla),
                &mut servidor,
                terminal,
            )
        },
        reloj,
        &mut |linea| terminal.decir(&linea),
        &mut |pasado| {
            guardadas.pasadas.push(PasadaGuardada::de(pasado));
            pasadas::guardar(&guardadas, &carpeta)
        },
    )?;
    // Se apaga antes de las pruebas, si lo arrancó el banco: la tarjeta ya no hace falta.
    drop(servidor.take());

    Ok(TandaPasada {
        carpeta,
        pasados,
        datos,
    })
}

/// La ruta del modelo que mide la tanda y el nombre con que se guarda. Sin `motor.modelo` no hay
/// qué medir.
fn modelo_que_se_mide(config: &Config) -> Resultado<(PathBuf, String)> {
    let Some(modelo) = &config.motor.modelo else {
        return Err(Error::ConfiguracionDelMotor(
            "el banco necesita «motor.modelo» para saber qué modelo mide".into(),
        ));
    };
    let ruta_modelo = config.resolver(modelo);
    let nombre_modelo = crate::motor::fichero_de_modelo::nombre_de_modelo(&ruta_modelo)
        .unwrap_or_else(|| modelo.clone());
    Ok((ruta_modelo, nombre_modelo))
}

/// El muestreo de la configuración con la semilla dada y la temperatura del banco.
fn muestreo_de_la_semilla(config: &Config, banco: &ConfigBanco, semilla: i64) -> Muestreo {
    Muestreo {
        semilla,
        temperatura: banco.temperatura,
        ..config.muestreo.clone()
    }
}

/// Cuenta en la terminal qué se va a pasar y dónde quedará.
fn contar_el_inicio(
    encargos: &[Encargo],
    banco: &ConfigBanco,
    lugar: &LugarElegido,
    carpeta: &Path,
    terminal: &dyn Terminal,
) {
    terminal.decir(&mensajes::mensaje_de_inicio(&mensajes::Inicio {
        encargos: encargos.len(),
        semillas: &banco.semillas,
        carpeta_de_comprobacion: lugar.para_contar(&carpeta.join(CARPETA_DE_COMPROBACION)),
        hay_codigo_que_comprobar: hay_codigo_que_comprobar(encargos),
        resultados: carpeta,
    }));
}

/// Carga el motor midiendo lo que cuesta y lo cuenta. Devuelve la carga medida, o nada si el motor
/// ya estaba y no es de Programator; que no cargue dentro del tope es un fallo de la tanda.
fn cargar_el_motor(
    config: &Config,
    muestreo: &Muestreo,
    servidor: &mut Option<ServidorLocal>,
    tope: Duration,
    reloj: &mut dyn Reloj,
    terminal: &dyn Terminal,
) -> Resultado<Option<Carga>> {
    match cargar_midiendo(config, muestreo, servidor, tope, reloj, terminal)? {
        DesenlaceDeCarga::Listo(carga) => {
            terminal.decir(&mensajes::mensaje_motor_cargado(
                carga.capas,
                carga.vram_mib,
                carga.duracion,
            ));
            Ok(Some(carga))
        }
        DesenlaceDeCarga::Ajeno => {
            terminal.decir(&mensajes::mensaje_motor_ajeno());
            Ok(None)
        }
        DesenlaceDeCarga::TiempoAgotado => Err(Error::Banco(mensajes::mensaje_carga_agotada(tope))),
    }
}
