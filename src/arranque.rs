//! Puesta en marcha: instalar las normas de la casa en la carpeta elegida, detectar novedades
//! y redactar avisos contextuales para el operador.

mod guia;
mod mensajes;
#[cfg(test)]
mod pruebas;
mod sondeo;

pub use self::guia::{instalar_normas, publicar_guia};
pub use self::mensajes::{
    cadencia_sondeo, hay_registro_previo, mensaje_argumento_falta_valor,
    mensaje_argumento_no_entendido, mensaje_canal_parcialmente_ilegible,
    mensaje_carpeta_de_trabajo, mensaje_desenlace_atendidos, mensaje_desenlace_motor_cargando,
    mensaje_desenlace_sin_encargos, mensaje_fallo_arranque, mensaje_fallo_pasada_ciclo,
    mensaje_fallo_publicar_guia, mensaje_fallo_sondeo_canal, mensaje_guia_publicada,
    mensaje_instrucciones_instaladas, mensaje_listo_primer_arranque, mensaje_listo_reinicio,
    mensaje_novedades_atendiendo, mensaje_reanudacion_encargo_esperando_motor, mensaje_version,
};
pub use self::sondeo::{hay_novedades, Sondeo};
