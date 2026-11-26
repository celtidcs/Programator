//! Los textos que la carga de `programator.toml` le dice a una persona: avisos de lo que se
//! corrigió al cargar y errores de un fichero que no vale.

use super::claves_retiradas::ClaveRetirada;
use super::{NOMBRE_DEL_FICHERO_DE_CONFIGURACION, TECHO_DE_SEGUNDOS};
use std::path::Path;

/// El aviso de un tiempo del TOML que pasaba del techo.
pub(super) fn aviso_de_techo(clave: &str, valor: u64) -> String {
    format!(
        "⚠️ «{clave} = {valor}» es más de un día: se usa un día ({TECHO_DE_SEGUNDOS} segundos)."
    )
}

/// El aviso de un envoltorio de preparación vacío, que se cambia por el de fábrica.
pub(super) fn aviso_de_envoltorio_vacio() -> String {
    "⚠️ «[preparacion] envoltorio = []» dejaría sin aislar las órdenes de preparar: se usa el \
     envoltorio de fábrica."
        .to_string()
}

/// El aviso de una clave retirada que trae el fichero: qué pasa y qué hacer.
pub(super) fn aviso_de_clave_retirada(retirada: &ClaveRetirada) -> String {
    format!(
        "⚠️ {NOMBRE_DEL_FICHERO_DE_CONFIGURACION} trae «[{}] {}», que ya no vale.\n\
         Consecuencia: no se tiene en cuenta. {}\n\
         Qué hacer: {}",
        retirada.seccion, retirada.clave, retirada.porque, retirada.que_hacer
    )
}

/// El error de un `programator.toml` que no se deja interpretar, con la ruta como la escribe el
/// sistema: con `{:?}` saldría entre comillas y, en Windows, con las barras dobladas.
pub(super) fn toml_ininteligible(ruta: &Path, causa: &dyn std::fmt::Display) -> String {
    format!("«{}» no se pudo interpretar: {causa}", ruta.display())
}

/// El error de un fichero sin `[motor] modelo`.
pub(super) fn falta_el_modelo() -> String {
    "falta «modelo» en la sección [motor]: sin fichero GGUF no hay nada que arrancar".to_string()
}
