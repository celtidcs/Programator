//! La revisión de modelos: buscar candidatos en Hugging Face, descargarlos, pasarles el banco,
//! quedarse con el mejor de cada clase y retirar los que sobren. Se lanza a mano con sus órdenes o
//! la pide un modelo de pago por el canal.
//!
//! Un asunto por fichero. Lo que decide es puro y se prueba sin red ni disco; la red y el disco se quedan en los
//! bordes.

pub mod atender;
pub mod ausentes;
pub mod buscar;
pub mod candidatos;
pub mod catalogo;
pub mod cerrar;
pub mod config;
pub mod descarga;
pub mod donde;
pub mod eleccion;
pub mod encargo;
pub mod informe;
pub mod lista;
pub mod mensajes;
pub mod mes;
pub mod mundo;
pub mod ordenes;
pub mod origenes;
pub mod probados;
pub mod probar;
pub mod proponer;
pub mod propuesta;
#[cfg(test)]
pub(crate) mod pruebas_comunes;
pub mod publicacion;
pub mod redaccion;
pub mod reponer;
pub mod retirada;
pub mod traer;

use crate::unidades::BYTES_POR_MIB;
/// Las fechas de la API son ISO 8601 (`2025-12-09T21:25:08.000Z`); el día son los diez primeros
/// caracteres.
const LARGO_DE_LA_FECHA: usize = 10;
/// Cómo se escribe y se lee un día en la revisión: el nombre de las carpetas, `retirado_el` del
/// registro y el día de las fechas de la API.
const FORMATO_DEL_DIA: &str = "%Y-%m-%d";
/// Las notas del banco van sobre 5 con un decimal, como en el informe de la tanda.
const DECIMALES_DE_LA_NOTA: usize = 1;
