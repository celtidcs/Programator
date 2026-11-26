//! Capa que decide qué se le enseña al modelo y qué se le permite hacer.

pub mod cancelacion;
pub mod ciclo;
pub mod clase_pedida;
pub mod confinamiento;
pub mod contexto;
pub mod entregas;
pub mod espacio;
pub mod ficha;
pub mod herramientas;
pub mod linea_con_nombre;
pub mod lista_del_proyecto;
pub mod normas;
pub mod pendientes;
pub mod preparacion;
pub mod prioridad;
pub mod redaccion;
pub mod registro;
pub mod registro_de_encargos;
pub mod sesion;
pub mod verificacion;

pub use confinamiento::Ambito;
