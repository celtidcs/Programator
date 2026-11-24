//! La comprobación de una propuesta ya escrita: el subproceso del comprobador de su extensión.
//!
//! Corre en una carpeta propia y vacía dentro del proyecto, que se borra al terminar. Quedarse sin
//! comprobar nunca tumba un encargo: lo que no se pudo comprobar se dice en el veredicto.

use super::{mensajes, Repertorio};
use crate::arnes::verificacion::{verificar, Veredicto};
use std::path::Path;

impl Repertorio<'_> {
    /// Pasa la propuesta por el comprobador de su extensión, si hay alguno configurado.
    ///
    /// El trabajo se hace en una carpeta propia y vacía, `.gestor/<agente>/verificacion-<marca>/`,
    /// que se borra al terminar: comprobadores como `rustc --emit=metadata` dejan artefactos, y el
    /// canal no es sitio para ellos. Va dentro de la carpeta de trabajo, y no en el temporal del
    /// sistema, porque Programator no escribe fuera del proyecto en el que trabaja. Si ni siquiera
    /// se puede crear, no se verifica y se dice: **quedarse sin comprobar nunca puede tumbar un
    /// encargo**.
    pub(super) fn verificar_propuesta(&self, fichero: &Path) -> Veredicto {
        if self.comprobadores.is_empty() {
            return Veredicto::SinComprobador {
                extension: extension_en_minusculas(fichero),
            };
        }

        let temporal = crate::arnes::espacio::carpeta_de_verificacion(
            &crate::plataforma::sin_prefijo_extendido(self.ambito.raiz()),
            self.canal.yo(),
            &marca_unica(),
        );
        if let Err(e) = std::fs::create_dir_all(&temporal) {
            return Veredicto::NoSePudo {
                motivo: mensajes::sin_carpeta_de_comprobacion(&e),
            };
        }

        let veredicto = verificar(
            fichero,
            &self.comprobadores,
            self.tiempo_de_verificacion,
            &temporal,
            self.maximo_salida_de_verificacion,
        );

        // Si no se deja borrar, se sigue igual: el veredicto ya está y un temporal huérfano no es
        // motivo para tumbar un encargo.
        let _ = std::fs::remove_dir_all(&temporal);
        veredicto
    }
}

/// La extensión del fichero en minúsculas, o vacía si no tiene: es la que busca el comprobador.
fn extension_en_minusculas(fichero: &Path) -> String {
    fichero
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Una marca que no se repite entre comprobaciones: el proceso y el instante, en nanosegundos.
///
/// Directorio temporal propio en vez de `tempfile`: esa crate está en las dependencias de
/// desarrollo y meterla en producción por esto no compensa, cuando lo que hace falta es una
/// carpeta vacía con nombre único.
fn marca_unica() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    )
}
