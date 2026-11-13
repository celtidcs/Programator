//! El registro de modelos: lo que el banco midió de cada modelo, guardado en la carpeta de
//! producción para que el agente de pago sepa qué encargarle al modelo local.
//!
//! El banco mide y el registro guarda lo medido. **Nadie teclea una cifra:** cada entrada sale de
//! una tanda terminada y dice de cuál, para que se pueda comprobar.

pub mod anotar;
pub mod anotar_tanda;
pub mod mensajes;
pub mod modelo;
pub mod redaccion;
