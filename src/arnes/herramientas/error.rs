//! Traducción y saneamiento de errores para el modelo.
//!
//! Garantiza que los mensajes de error devueltos al modelo utilicen exclusivamente
//! las rutas relativas que él solicitó, impidiendo la filtración de rutas absolutas
//! del anfitrión hacia el canal compartido.

use crate::error::Error;

/// Traduce un fallo de ruta en el motivo que se le devuelve al modelo, nombrando **la ruta que él
/// pidió** y nunca la ruta absoluta que resolvió el arnés.
///
/// El error tipado sigue llevando la ruta real, porque otros la necesitan para los registros del
/// anfitrión; lo que no puede salir de aquí es esa ruta. El modelo cita las denegaciones cuando
/// publica, y el canal lo leen los otros tres agentes: una ruta como `\\?\C:\Users\…\AppData\…`
/// filtraría la estructura de esta máquina al fichero compartido. `reservar` ya se tomaba esa
/// molestia; estas denegaciones no.
pub(super) fn motivo_para_el_modelo(solicitada: &str, fallo: &Error) -> String {
    match fallo {
        Error::FueraDeAmbito(_) => {
            format!("«{solicitada}» queda fuera del ámbito permitido")
        }
        Error::Lectura { causa, .. } => {
            format!("no se pudo acceder a «{solicitada}»: {causa}")
        }
        Error::Escritura { causa, .. } => {
            format!("no se pudo escribir lo relativo a «{solicitada}»: {causa}")
        }
        otro => format!("«{solicitada}» no se pudo usar: {otro}"),
    }
}

/// Redacta el aviso emitido cuando la propuesta se ha guardado en disco pero falla su publicación intermedia en el canal.
pub(super) fn mensaje_fallo_publicar_entrega(nombre_propuesta: &str, error: &Error) -> String {
    format!(
        "⚠️ No se pudo publicar el aviso de entrega de «{nombre_propuesta}» en el canal: {error}.\n\
         Consecuencia: la propuesta SÍ está escrita en disco en la carpeta de candidatos. \
         El arnés intentará adjuntar su veredicto al cierre del encargo; el trabajo no se ha perdido.\n\
         Qué hacer: no es necesario reintentar la acción ahora; si el fallo persiste al concluir el encargo, \
         comprueba los permisos del canal."
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn nombra_la_ruta_solicitada_en_fuera_de_ambito() {
        let error = Error::FueraDeAmbito(PathBuf::from("C:\\Users\\celti\\AppData\\secreto"));
        let motivo = motivo_para_el_modelo("../fuera.txt", &error);
        assert_eq!(motivo, "«../fuera.txt» queda fuera del ámbito permitido");
        assert!(!motivo.contains("secreto"));
    }

    #[test]
    fn nombra_la_ruta_solicitada_en_lectura() {
        let error = Error::Lectura {
            ruta: PathBuf::from("C:\\Privado\\doc.txt"),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permiso denegado"),
        };
        let motivo = motivo_para_el_modelo("doc.txt", &error);
        assert_eq!(motivo, "no se pudo acceder a «doc.txt»: permiso denegado");
        assert!(!motivo.contains("Privado"));
    }

    #[test]
    fn mensaje_fallo_publicar_entrega_declara_que_la_propuesta_esta_en_disco() {
        let error = Error::Escritura {
            ruta: PathBuf::from(".gestor/canal/programator.md"),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "bloqueado"),
        };
        let mensaje = mensaje_fallo_publicar_entrega("MiClase.cs", &error);
        assert!(mensaje.starts_with("⚠️ No se pudo publicar el aviso de entrega"));
        assert!(mensaje.contains("«MiClase.cs»"));
        assert!(
            mensaje.contains("la propuesta SÍ está escrita en disco en la carpeta de candidatos")
        );
        assert!(mensaje.contains("no se ha perdido"));
    }
}
