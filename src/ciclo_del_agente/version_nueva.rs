//! El aviso de versión nueva al arrancar: la consulta a GitHub sale en un hilo de trabajo antes
//! del informe y su resultado se cuenta después, sin impedir nunca que Programator arranque.

use crate::VERSION;
use programator::arranque;
use programator::aviso_de_version::{self, PreferenciasDeVersion, ResultadoDeVersion};
use programator::config::Config;
use std::fmt::Display;
use std::sync::mpsc::Receiver;

/// Arranca, en un hilo de trabajo, la comprobación de si hay una versión más nueva en GitHub
/// (`aviso_de_version`). Va antes del informe para que la consulta corra mientras se mide la
/// máquina; su resultado lo cuenta `contar_el_aviso_de_version`. Se comprueba también en
/// `--diagnostico`, que es justo para preguntar «¿esto cómo está hoy?».
pub(super) fn arrancar_el_aviso_de_version(
    config: &Config,
) -> Option<Receiver<ResultadoDeVersion>> {
    let registro: aviso_de_version::Registro = std::sync::Arc::new(|fallo: &dyn Display| {
        eprintln!(
            "{}",
            arranque::bloque(&aviso_de_version::mensaje_no_comprobado(fallo))
        );
    });
    aviso_de_version::arrancar(
        &PreferenciasDeVersion::desde_la_configuracion(&config.actualizaciones),
        VERSION,
        aviso_de_version::consulta_real,
        registro,
    )
}

/// Cuenta en la consola lo que llegó de la comprobación de versión. Nunca puede impedir que
/// Programator arranque: espera como mucho `[actualizaciones] tiempo_espera_segundos` desde que
/// salió la consulta, y un fallo solo dice que no se pudo comprobar.
pub(super) fn contar_el_aviso_de_version(canal: Option<Receiver<ResultadoDeVersion>>) {
    let Some(resultado) = canal.and_then(|canal| canal.recv().ok()) else {
        return;
    };
    match &resultado {
        ResultadoDeVersion::NoComprobado(causa) => eprintln!(
            "{}",
            arranque::bloque(&aviso_de_version::mensaje_no_comprobado(causa))
        ),
        otro => {
            if let Some(texto) = aviso_de_version::mensaje(VERSION, otro) {
                eprintln!("{}", arranque::bloque(&texto));
            }
        }
    }
}
