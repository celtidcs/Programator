//! El aviso de que ningún modelo de la carpeta cabe en la tarjeta: en la terminal y, a quien puede
//! pedir la revisión, en el canal.

use programator::arranque::{self, disponibles::Disponibles};
use programator::config::Config;
use programator::protocolo::destinatarios::Identidad;
use std::path::Path;

/// Si ningún modelo cabe: se dice en la terminal y, a quien puede pedir la revisión, en el canal.
/// Una vez por arranque. Un fallo al publicar se avisa y no impide arrancar.
pub(super) fn avisar_de_que_ninguno_cabe(
    carpeta: &Path,
    config: &Config,
    disponibles: &Disponibles,
) {
    use arranque::disponibles::AvisoEnElCanal;
    use programator::protocolo::respuesta::linea_para_varios;
    let identidades = identidades_que_ordenan(config);
    let que_hacer = programator::revision::mensajes::que_hacer_sin_modelos(
        &programator::revision::donde::donde(carpeta, config.carpeta_de_instalacion()),
        &config
            .carpeta_de_instalacion()
            .join(programator::config::NOMBRE_DEL_FICHERO_DE_CONFIGURACION),
        &disponibles.carpeta,
        programator::revision::mensajes::PermisoDeRevision::de(&identidades),
    );
    let linea = linea_para_varios(
        &identidades,
        arranque::disponibles::asunto_del_aviso(disponibles),
    );
    let canal = match linea {
        None => AvisoEnElCanal::SinIdentidades,
        Some(linea) => programator::protocolo::Canal::nuevo(carpeta, &config.agente.nombre)
            .and_then(|canal| {
                canal.publicar_dirigido(
                    &arranque::disponibles::cuerpo_del_aviso(disponibles, &que_hacer),
                    &programator::protocolo::ResumenLeido::default(),
                    &programator::protocolo::marca_del_latido(),
                    &linea,
                )
            })
            .map_or_else(
                |fallo| AvisoEnElCanal::NoPublicado(fallo.to_string()),
                |_| AvisoEnElCanal::Publicado,
            ),
    };
    eprintln!(
        "{}",
        arranque::bloque(&arranque::disponibles::aviso_en_la_terminal(
            disponibles,
            &que_hacer,
            &canal
        ))
    );
}

/// Quienes pueden pedir la revisión, según `[revision] identidades_que_ordenan`; las que no se
/// dejan leer como identidad se descartan.
pub(super) fn identidades_que_ordenan(config: &Config) -> Vec<Identidad> {
    config
        .revision
        .identidades_que_ordenan
        .iter()
        .filter_map(|identidad| Identidad::leer(identidad))
        .collect()
}
