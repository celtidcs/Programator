//! Las órdenes de la revisión de modelos (`--revision-buscar`, `--revision-proponer`,
//! `--retirar-modelo`, `--descargar-modelo`): cada una carga la configuración, hace lo suyo sin
//! arrancar el ciclo y cuenta cómo acabó. Un fallo termina el proceso con el código de fallo.

use crate::{ruta_de_la_configuracion, CODIGO_DE_FALLO};
use programator::config::Config;
use programator::error::{Error, Resultado};
use programator::revision;

/// `--revision-buscar`.
pub(super) fn buscar() {
    match buscar_en_la_revision() {
        Ok(hecha) => println!("{}", revision::mensajes::busqueda_hecha(&hecha)),
        Err(fallo) => salir_por_la_revision(&fallo),
    }
}

/// `--revision-proponer`.
pub(super) fn proponer() {
    match proponer_en_la_revision() {
        Ok(hecha) => println!("{}", revision::mensajes::propuesta_hecha(&hecha)),
        Err(fallo) => salir_por_la_revision(&fallo),
    }
}

/// `--retirar-modelo`.
pub(super) fn retirar(fichero: &str) {
    match retirar_un_modelo(fichero) {
        Ok(retirada) => println!("{}", revision::mensajes::retirada_hecha(&retirada)),
        Err(fallo) => {
            eprintln!("{}", revision::mensajes::fallo_de_la_retirada(&fallo));
            std::process::exit(CODIGO_DE_FALLO);
        }
    }
}

/// `--descargar-modelo`.
pub(super) fn descargar(repositorio: &str, fichero: &str) {
    match descargar_un_modelo(repositorio, fichero) {
        Ok(revision::traer::Traido::Descargado(hecho)) => {
            println!("{}", revision::mensajes::descarga_hecha(&hecho))
        }
        Ok(revision::traer::Traido::YaEstaba(ruta)) => {
            println!("{}", revision::mensajes::ya_estaba(&ruta))
        }
        Err(fallo) => salir_por_la_revision(&fallo),
    }
}

/// Sin motor. Mide la tarjeta, lee el registro, pregunta a Hugging Face y repone lo que falte.
fn buscar_en_la_revision() -> Resultado<revision::buscar::BusquedaHecha> {
    let config = Config::cargar(&ruta_de_la_configuracion())?;
    revision::ordenes::buscar(&config, &mut |aviso| {
        println!("{}", revision::mensajes::aviso_de_reposicion(&aviso))
    })
}

/// Sin motor ni red. De la tarjeta solo hace falta su nombre.
fn proponer_en_la_revision() -> Resultado<revision::proponer::PropuestaHecha> {
    revision::ordenes::proponer(&Config::cargar(&ruta_de_la_configuracion())?)
}

/// Sin motor, GPU ni red.
fn retirar_un_modelo(fichero: &str) -> Resultado<revision::retirada::Retirada> {
    revision::ordenes::retirar(&Config::cargar(&ruta_de_la_configuracion())?, fichero)
}

/// Sin motor ni GPU. Del TOML solo hace falta `[revision]`.
fn descargar_un_modelo(repositorio: &str, fichero: &str) -> Resultado<revision::traer::Traido> {
    let config = Config::cargar(&ruta_de_la_configuracion())?;
    revision::ordenes::descargar(&config, repositorio, fichero, &mut |aviso| {
        println!("{}", revision::mensajes::aviso_de_descarga(&aviso))
    })
}

fn salir_por_la_revision(fallo: &Error) -> ! {
    eprintln!("{}", revision::mensajes::fallo(fallo));
    std::process::exit(CODIGO_DE_FALLO);
}
