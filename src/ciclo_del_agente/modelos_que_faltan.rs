//! El aviso del arranque de que faltan en la carpeta de modelos modelos del registro que siguen
//! haciendo falta. Solo en la terminal, sin red, sin leer ningún GGUF y sin calcular huellas: de
//! dónde salió cada uno lo dicen el registro y las listas de búsquedas anteriores, como en la
//! reposición.

use super::guia_del_equipo::LoMedido;
use super::ninguno_cabe::identidades_que_ordenan;
use programator::arranque;
use programator::config::{Config, NOMBRE_DEL_FICHERO_DE_CONFIGURACION};
use programator::revision::{donde, mensajes, origenes, proponer::ficheros_con_tamano};
use std::path::Path;

/// Si faltan modelos útiles, se dice en la terminal. Sin tarjeta medida no se sabe qué cifras
/// valen, y no se dice nada. Si la carpeta de modelos no se deja leer, tampoco: el arranque ya
/// cuenta por su lado qué modelos hay, y este aviso no debe impedir arrancar.
pub(super) fn avisar_de_los_que_faltan(carpeta: &Path, config: &Config, medido: &LoMedido) {
    let Some(tarjeta) = medido.tarjeta else {
        return;
    };
    let carpeta_de_modelos = &config.revision.carpeta_de_modelos;
    let Ok(en_modelos) = ficheros_con_tamano(&config.resolver(carpeta_de_modelos)) else {
        return;
    };
    let modelo_en_uso = config.motor.nombre_del_modelo();
    let faltan = origenes::ausentes_al_arrancar(
        medido.registro,
        &config.resolver(&config.revision.carpeta),
        tarjeta,
        &en_modelos,
        modelo_en_uso.as_deref(),
    );
    let instalacion = config.carpeta_de_instalacion();
    let como_pedir = mensajes::como_pedir_la_revision(
        &donde::donde(carpeta, instalacion),
        &instalacion.join(NOMBRE_DEL_FICHERO_DE_CONFIGURACION),
        !identidades_que_ordenan(config).is_empty(),
    );
    if let Some(aviso) =
        mensajes::aviso_de_los_que_faltan(&faltan.utiles, &como_pedir, carpeta_de_modelos)
    {
        eprintln!("{}", arranque::bloque(&aviso));
    }
}
