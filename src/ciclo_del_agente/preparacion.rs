//! La preparación de la carpeta de trabajo antes del bucle: las instrucciones del agente, el
//! directorio del canal, su foto inicial, el vigilante, la guía del equipo, el aviso si ningún
//! modelo cabe y el de los modelos útiles que faltan en la carpeta de modelos.

use super::guia_del_equipo::{publicar_la_guia_del_equipo, LoMedido};
use super::modelos_que_faltan::avisar_de_los_que_faltan;
use super::ninguno_cabe::avisar_de_que_ninguno_cabe;
use programator::arnes::{espacio, normas, sesion};
use programator::arranque::{self, instalar_normas};
use programator::config::Config;
use programator::error::{Error, Resultado};
use std::path::{Path, PathBuf};

/// Deja la carpeta de trabajo lista para el ciclo: las instrucciones del agente, el canal, su foto
/// inicial y la guía para los demás. Devuelve la carpeta del canal y lo que hizo la foto (`None` si
/// falló: ya se ha dicho, y la tomará la primera pasada).
pub(super) fn preparar_la_carpeta(
    carpeta: &Path,
    config: &Config,
    medido: &LoMedido,
) -> Resultado<(PathBuf, Option<sesion::Instantanea>)> {
    instalar_las_instrucciones(carpeta, config)?;

    let carpeta_del_canal = programator::protocolo::carpetas::carpeta_del_canal(carpeta);
    std::fs::create_dir_all(&carpeta_del_canal).map_err(|causa| Error::Escritura {
        ruta: carpeta_del_canal.clone(),
        causa,
    })?;

    // La foto, antes de publicar nada en el buzón propio: un aviso publicado antes haría creer
    // que Programator ya respondió en este canal, y se darían por vistos encargos sin atender.
    let instantanea = match sesion::tomar_instantanea_inicial(carpeta, &config.agente.nombre) {
        Ok(instantanea) => Some(instantanea),
        Err(fallo) => {
            eprintln!("{}", arranque::mensaje_fallo_pasada_ciclo(&fallo));
            None
        }
    };

    dejar_el_vigilante(carpeta, config);
    publicar_la_guia_del_equipo(&carpeta_del_canal, config, medido);
    if let Some(disponibles) = medido.disponibles.filter(|d| d.ninguno_cabe()) {
        avisar_de_que_ninguno_cabe(carpeta, config, disponibles);
    }
    avisar_de_los_que_faltan(carpeta, config, medido);
    Ok((carpeta_del_canal, instantanea))
}

/// El fichero de instrucciones: se compone con lo que el arnés va a ejecutar de verdad y se
/// instala en el espacio propio del agente, no en la raíz de un repositorio que no es suyo.
fn instalar_las_instrucciones(carpeta: &Path, config: &Config) -> Resultado<()> {
    let plantilla = config.resolver(&config.agente.plantilla_normas);
    let texto_plantilla = std::fs::read_to_string(&plantilla).map_err(|causa| Error::Lectura {
        ruta: plantilla.clone(),
        causa,
    })?;
    let instrucciones = normas::componer(&texto_plantilla, &config.verificacion.comprobadores);
    let destino_normas = espacio::normas_en_uso(carpeta, &config.agente.nombre);
    if instalar_normas(&destino_normas, &instrucciones)? {
        println!(
            "{}",
            arranque::mensaje_instrucciones_instaladas(&destino_normas)
        );
    }
    Ok(())
}

/// El vigilante del canal en el proyecto, si no hay uno. Un fallo se avisa y no impide arrancar.
fn dejar_el_vigilante(carpeta: &Path, config: &Config) {
    use arranque::vigilante;
    match vigilante::instalar(&config.resolver(&config.agente.vigilante), carpeta) {
        Ok(decision) => {
            if let Some(mensaje) = vigilante::mensaje(decision, carpeta) {
                eprintln!("{mensaje}");
            }
        }
        Err(fallo) => eprintln!("{}", vigilante::mensaje_de_fallo(&fallo)),
    }
}
