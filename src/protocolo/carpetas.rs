//! Dónde vive cada cosa del protocolo dentro de una carpeta de trabajo.
//!
//! Es el único sitio que nombra `.gestor/` y `.gestor/canal/`: son el contrato en disco con los
//! demás agentes del equipo, y escrito a mano en cinco sitios bastaba con que uno cambiara para que
//! Programator mirase en una carpeta y escribiese en otra.

use std::path::{Path, PathBuf};

/// La carpeta de coordinación del equipo, en la raíz de la carpeta de trabajo. La fija el
/// protocolo del canal, el mismo que usa el gestor de agentes.
pub const CARPETA_DEL_GESTOR: &str = ".gestor";

/// La carpeta del canal, dentro de [`CARPETA_DEL_GESTOR`].
pub const CARPETA_DEL_CANAL: &str = "canal";

/// El nombre con que se publica en el canal la guía para los demás agentes. Es un contrato
/// publicado: la nombran las instrucciones que se les dan a los modelos de pago.
pub const NOMBRE_DE_LA_GUIA: &str = "COMO-ENCARGAR-A-PROGRAMATOR.md";

/// `<carpeta>/.gestor`.
pub fn carpeta_del_gestor(carpeta: &Path) -> PathBuf {
    carpeta.join(CARPETA_DEL_GESTOR)
}

/// `<carpeta>/.gestor/canal`.
pub fn carpeta_del_canal(carpeta: &Path) -> PathBuf {
    carpeta_del_gestor(carpeta).join(CARPETA_DEL_CANAL)
}

/// La ruta de la guía relativa a la carpeta de trabajo, tal como se le enseña a una persona:
/// `.gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md`, con barras normales en cualquier sistema.
pub fn ruta_legible_de_la_guia() -> String {
    format!("{CARPETA_DEL_GESTOR}/{CARPETA_DEL_CANAL}/{NOMBRE_DE_LA_GUIA}")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_canal_esta_en_la_carpeta_de_siempre() {
        let carpeta = Path::new("proyecto");
        assert_eq!(
            carpeta_del_canal(carpeta),
            carpeta.join(".gestor").join("canal")
        );
    }

    #[test]
    fn la_guia_se_ensena_con_su_ruta_de_siempre() {
        assert_eq!(
            ruta_legible_de_la_guia(),
            ".gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md"
        );
    }
}
