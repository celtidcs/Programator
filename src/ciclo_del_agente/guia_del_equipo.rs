//! La guía para los demás agentes: se compone con lo que el registro de modelos cuenta del modelo
//! configurado, la tabla de modelos por clase y los modelos disponibles, y se publica en el canal
//! en cada arranque y cada vez que cambia el modelo.

use programator::arnes::clase_pedida::ClasePedida;
use programator::arranque::{self, disponibles::Disponibles, publicar_guia};
use programator::config::Config;
use programator::registro::modelo::Registro;
use programator::registro::{mensajes, modelo, redaccion};
use programator::revision::eleccion::{elegir_modelo, es_elegible};
use std::path::Path;

/// Lo medido que cuenta la guía: el registro de modelos, los modelos disponibles (si se midió la
/// tarjeta) y el nombre de la tarjeta.
pub(super) struct LoMedido<'a> {
    pub(super) registro: &'a Registro,
    pub(super) disponibles: Option<&'a Disponibles>,
    /// El nombre de la tarjeta, de la misma medición que dio `disponibles`; `None` sin tarjeta.
    pub(super) tarjeta: Option<&'a str>,
}

/// El registro de modelos. Un registro que no se puede leer no impide arrancar: se avisa, y la
/// guía dice que el modelo no está medido.
pub(super) fn leer_el_registro_de_modelos(config: &Config) -> Registro {
    let ruta = config.resolver(&config.agente.registro_de_modelos);
    modelo::leer(&ruta).unwrap_or_else(|fallo| {
        eprintln!("{}", mensajes::mensaje_registro_ilegible(&fallo));
        Registro::default()
    })
}

/// La guía para los demás agentes, al día en cada arranque. Si no se puede escribir se avisa y
/// se sigue: quedarse sin guía es malo, pero no arrancar es peor.
pub(super) fn publicar_la_guia_del_equipo(canal_dir: &Path, config: &Config, medido: &LoMedido) {
    let plantilla_guia = config.resolver(&config.agente.guia);
    let en_modelos = programator::revision::proponer::ficheros_con_tamano(
        &config.resolver(&config.revision.carpeta_de_modelos),
    )
    .unwrap_or_default();
    let desempeno = desempeno_para_la_guia(config, medido, &en_modelos);
    match publicar_guia(
        canal_dir,
        &plantilla_guia,
        programator::protocolo::carpetas::NOMBRE_DE_LA_GUIA,
        config.motor.contexto,
        &desempeno,
        &config.verificacion.comprobadores,
    ) {
        Ok(()) => {
            println!("{}", arranque::bloque(&arranque::mensaje_guia_publicada()));
            println!(
                "{}",
                arranque::bloque(&arranque::mensaje_orientacion_modelo_pago())
            );
            println!(
                "{}",
                arranque::bloque(&arranque::mensaje_guia_para_personas())
            );
        }
        Err(fallo) => eprintln!("{}", arranque::mensaje_fallo_publicar_guia(&fallo)),
    }
}

/// Vuelve a publicar la guía con el registro, la tarjeta y los modelos disponibles de ahora: la
/// guía cuenta el modelo por defecto y sus cifras, y con otro hay que rehacerla.
pub(super) fn republicar_la_guia_del_equipo(canal_dir: &Path, config: &Config) {
    let registro = leer_el_registro_de_modelos(config);
    let gpu = programator::motor::hardware::describir_gpu();
    let disponibles = arranque::disponibles::reunir(config, gpu.as_ref(), &registro);
    publicar_la_guia_del_equipo(
        canal_dir,
        config,
        &LoMedido {
            registro: &registro,
            disponibles: disponibles.as_ref(),
            tarjeta: gpu.as_ref().map(|gpu| gpu.nombre.as_str()),
        },
    );
}

/// Lo que la guía cuenta del modelo configurado, sacado del registro de modelos, la tabla por
/// clase y, si se midió la tarjeta, los modelos disponibles. `en_modelos`: los ficheros de la
/// carpeta de modelos, con su tamaño.
fn desempeno_para_la_guia(
    config: &Config,
    medido: &LoMedido,
    en_modelos: &[(String, u64)],
) -> String {
    let nombre = config.motor.nombre_del_modelo().unwrap_or_default();
    let desempeno = redaccion::componer_desempeno(
        &nombre,
        redaccion::entrada_del_modelo(medido.registro, &nombre),
        &config.motor,
    );
    let modelos = medido
        .disponibles
        .map(|d| format!("\n{}", arranque::disponibles::componer(d)))
        .unwrap_or_default();
    format!(
        "{desempeno}\n{}{modelos}",
        modelos_por_clase(medido.registro, medido.tarjeta, en_modelos)
    )
}

/// La tabla de la guía con los modelos aprobados por clase, elegidos igual que en el ciclo.
fn modelos_por_clase(
    registro: &Registro,
    tarjeta: Option<&str>,
    en_modelos: &[(String, u64)],
) -> String {
    redaccion::componer_modelos_por_clase(
        registro,
        |entrada| tarjeta.is_some_and(|t| es_elegible(entrada, t, en_modelos)),
        |fila| elegir_modelo(registro, &ClasePedida::Valida(fila), tarjeta, en_modelos).modelo,
    )
}
