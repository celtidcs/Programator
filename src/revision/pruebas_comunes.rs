//! Dobles de prueba que comparten varios módulos de la revisión: una fuente de descarga que siempre
//! da el mismo contenido, un disco que nunca se llena y un Hugging Face de mentira. Solo existe en
//! las pruebas.

use std::cell::RefCell;
use std::path::Path;

use super::catalogo::{
    leer_busqueda, leer_detalle, Busqueda, CatalogoDeModelos, DetalleDeRepositorio,
    FicheroDelRepositorio, Repositorio, Respuesta,
};
use super::descarga::{Apertura, Fuente};
use crate::error::{Error, Resultado};

/// Lo que da [`FuenteFija`].
pub const CONTENIDO_DE_LA_FUENTE_FIJA: &[u8] = b"un modelo";

/// Una fuente que da siempre [`CONTENIDO_DE_LA_FUENTE_FIJA`] entero, sin reanudar.
pub struct FuenteFija;

impl Fuente for FuenteFija {
    fn abrir(&self, _: u64) -> Resultado<Apertura> {
        Ok(Apertura {
            reanuda: false,
            lector: Box::new(CONTENIDO_DE_LA_FUENTE_FIJA),
        })
    }
}

/// Un disco que nunca se llena.
pub fn sobra_espacio(_: &Path) -> Resultado<u64> {
    Ok(u64::MAX)
}

/// Una respuesta guardada de la búsqueda de Hugging Face.
pub const BUSQUEDA: &str = include_str!("pruebas/busqueda.json");
/// Un detalle guardado de un repositorio de bartowski, con licencia.
pub const CON_LICENCIA: &str = include_str!("pruebas/detalle-con-licencia.json");

/// Contesta cada búsqueda con la respuesta guardada y cada detalle con el de bartowski,
/// cambiándole el nombre. Anota qué se le pidió, y falla en lo que se le diga.
#[derive(Default)]
pub struct CatalogoFalso {
    pub pedidos: RefCell<Vec<String>>,
    pub falla_en: Option<String>,
    /// Ficheros que se ponen delante en cada detalle, para que se elijan antes que los de
    /// bartowski de su misma cuantización.
    pub anadidos: Vec<FicheroDelRepositorio>,
}

impl CatalogoDeModelos for CatalogoFalso {
    fn buscar(&self, busqueda: &Busqueda) -> Resultado<Respuesta<Vec<Repositorio>>> {
        self.pedidos.borrow_mut().push(busqueda.texto.clone());
        if self.falla_en.as_deref() == Some(busqueda.texto.as_str()) {
            return Err(Error::Revision("sin red".into()));
        }
        Ok(Respuesta {
            datos: leer_busqueda(BUSQUEDA)?,
            en_bruto: BUSQUEDA.into(),
        })
    }

    fn detalle(&self, repositorio: &str) -> Resultado<Respuesta<DetalleDeRepositorio>> {
        self.pedidos.borrow_mut().push(repositorio.to_string());
        let mut datos = leer_detalle(CON_LICENCIA)?;
        datos.id = repositorio.to_string();
        datos.ficheros.splice(0..0, self.anadidos.iter().cloned());
        Ok(Respuesta {
            datos,
            en_bruto: CON_LICENCIA.into(),
        })
    }
}
