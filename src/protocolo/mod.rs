//! Capa que lee y escribe el canal `.gestor/canal/` según las normas de la casa.
//! No conoce el motor de inferencia, y eso es lo que permite probarla sin GPU.

pub mod canal;
pub mod carpetas;
pub mod encargo;
pub mod estado;
pub mod latido;
pub mod lectura;
pub mod poda;
pub mod saneado;

pub use canal::{
    es_linea_de_latido, marca_de_tiempo_actual, marca_del_latido, tras_la_marca_de_latido, Canal,
    ResumenLeido,
};
pub use encargo::{detectar, Encargo};
pub use estado::{anotar_reserva, retirar_reserva};
pub use latido::{DetalleEncargo, EstadoLatido, Latido};
pub use lectura::{Delta, RegistroLectura};
pub use poda::{
    componer, guardar_propuesta_poda, proponer_poda_por_cierre, trocear, verificar_integridad,
    Bloque, Clasificacion, Propuesta,
};
