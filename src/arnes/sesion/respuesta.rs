//! La única salida de la sesión al canal: publica una respuesta en el buzón propio, dirigida a
//! quien escribió el encargo (decisión 6 de `sesion`: el desenlace sale únicamente por
//! `Canal::publicar`).

use super::publicacion;
use crate::error::Resultado;
use crate::protocolo::{Canal, Encargo, ResumenLeido};
use crate::terminal::Terminal;

/// Publica `cuerpo` como respuesta a `encargo`, dirigida a quien lo escribió. Si su buzón no dice
/// quién es, se publica igual, con lo que le falta para que la próxima le avise, y se dice en la
/// terminal.
pub(super) fn responder(
    canal: &Canal,
    cuerpo: &str,
    leido: &ResumenLeido,
    encargo: &Encargo,
    terminal: &dyn Terminal,
) -> Resultado<()> {
    let cabecera = canal.cabecera_de(encargo);
    if cabecera.is_some() {
        return canal.publicar_respuesta(cuerpo, leido, &marca_de_tiempo(), cabecera.as_ref());
    }
    terminal.avisar(&publicacion::mensaje_sin_identidad(&encargo.de));
    let con_aviso = publicacion::con_aviso_sin_identidad(cuerpo, &encargo.de);
    canal.publicar_respuesta(&con_aviso, leido, &marca_de_tiempo(), None)
}

/// La hora del latido con que se publica en el buzón, con segundos (formato del canal).
fn marca_de_tiempo() -> String {
    crate::protocolo::marca_del_latido()
}
