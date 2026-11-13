//! Las piezas de la revisión con su entorno de verdad, montado desde `programator.toml`: la tarjeta
//! medida, Hugging Face, el disco y el reloj. Las usan las órdenes a mano (`--revision-buscar`,
//! `--revision-proponer`, `--retirar-modelo`, `--descargar-modelo`) y la revisión que se pide por
//! el canal, para que las dos hagan exactamente lo mismo.

use std::time::Duration;

use super::buscar::{self, BusquedaHecha};
use super::catalogo::HuggingFace;
use super::descarga::{espacio_libre, url_del_fichero, Aviso, Fuente, FuenteHttp};
use super::proponer::{self, PropuestaHecha};
use super::retirada::{self, Retirada};
use super::traer::{self, Traido};
use crate::config::Config;
use crate::error::{Error, Resultado};
use crate::motor::hardware::describir_gpu;
use crate::registro::modelo;

/// Busca candidatos en Hugging Face y deja la lista en la carpeta del mes.
pub fn buscar(config: &Config) -> Resultado<BusquedaHecha> {
    exigir_la_revision_bien_configurada(config)?;
    let registro = modelo::leer(&config.resolver(&config.agente.registro_de_modelos))?;
    let catalogo = HuggingFace {
        api: config.revision.api.clone(),
        tiempo_espera: Duration::from_secs(config.revision.tiempo_espera_segundos),
    };
    let entorno = buscar::Entorno {
        catalogo: &catalogo,
        tarjeta: describir_gpu(),
        margen_vram_mib: config.motor.margen_vram_mib,
        registro: &registro,
        carpeta_de_revisiones: config.resolver(&config.revision.carpeta),
        carpeta_de_modelos: config.resolver(&config.revision.carpeta_de_modelos),
        ahora: chrono::Local::now().naive_local(),
    };
    buscar::buscar(&config.revision, &entorno)
}

/// Escribe la propuesta del mes con el modelo en uso de la configuración.
pub fn proponer(config: &Config) -> Resultado<PropuestaHecha> {
    proponer_con(config, config.motor.nombre_del_modelo())
}

/// Escribe la propuesta del mes con otro modelo en uso: el que `revisión: cerrar` acaba de poner
/// por defecto.
pub fn proponer_con(config: &Config, modelo_en_uso: Option<String>) -> Resultado<PropuestaHecha> {
    let ruta_del_registro = config.resolver(&config.agente.registro_de_modelos);
    let registro = modelo::leer(&ruta_del_registro)?;
    let entorno = proponer::Entorno {
        registro: &registro,
        ruta_del_registro,
        tarjeta: describir_gpu().map(|gpu| gpu.nombre),
        carpeta_de_modelos: config.resolver(&config.revision.carpeta_de_modelos),
        carpeta_de_revisiones: config.resolver(&config.revision.carpeta),
        modelo_en_uso,
        ahora: chrono::Local::now().naive_local(),
    };
    proponer::proponer_y_escribir(&entorno)
}

/// Retira un modelo de la carpeta de modelos, que no puede ser el de la configuración.
pub fn retirar(config: &Config, fichero: &str) -> Resultado<Retirada> {
    retirar_con(config, fichero, config.motor.nombre_del_modelo())
}

/// Retira un modelo, que no puede ser `modelo_en_uso`.
pub fn retirar_con(
    config: &Config,
    fichero: &str,
    modelo_en_uso: Option<String>,
) -> Resultado<Retirada> {
    let entorno = retirada::Entorno {
        carpeta_de_modelos: config.resolver(&config.revision.carpeta_de_modelos),
        carpeta_de_revisiones: config.resolver(&config.revision.carpeta),
        ruta_del_registro: config.resolver(&config.agente.registro_de_modelos),
        modelo_en_uso,
        hoy: chrono::Local::now().date_naive(),
    };
    retirada::retirar(fichero, &entorno)
}

/// Descarga un candidato de la lista del mes, o lo encuentra ya descargado.
pub fn descargar(
    config: &Config,
    repositorio: &str,
    fichero: &str,
    avisar: &mut dyn FnMut(Aviso),
) -> Resultado<Traido> {
    exigir_la_revision_bien_configurada(config)?;
    let ajustes = &config.revision;
    let tiempo_espera = Duration::from_secs(ajustes.tiempo_espera_segundos);
    let fuente_de = |repositorio: &str, fichero: &str| -> Box<dyn Fuente> {
        Box::new(FuenteHttp {
            url: url_del_fichero(&ajustes.sitio, repositorio, fichero),
            tiempo_espera,
        })
    };
    let entorno = traer::Entorno {
        carpeta_de_revisiones: config.resolver(&ajustes.carpeta),
        carpeta_de_modelos: config.resolver(&ajustes.carpeta_de_modelos),
        margen_disco_mib: ajustes.margen_disco_mib,
        avisar_cada_mib: ajustes.avisar_cada_mib,
        fuente_de: &fuente_de,
        espacio_libre: &espacio_libre,
    };
    traer::traer(repositorio, fichero, &entorno, avisar)
}

fn exigir_la_revision_bien_configurada(config: &Config) -> Resultado<()> {
    match config.revision.problema() {
        Some(problema) => Err(Error::Configuracion(problema)),
        None => Ok(()),
    }
}
