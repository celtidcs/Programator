//! `--descargar-modelo <repositorio> <fichero>`: traer a la carpeta de modelos un candidato de la
//! búsqueda más reciente (especificación, §4).
//!
//! Solo se baja lo que está en la lista, con la huella y el tamaño de la lista, no con lo que diga
//! el servidor en ese momento: así un error al teclear no baja cualquier cosa.

use std::path::{Path, PathBuf};

use super::candidatos::FicheroCandidato;
use super::descarga::{descargar, Aviso, Descarga, Descargado, Fuente};
use super::lista::{self, NOMBRE_DE_LA_LISTA};
use super::{mensajes, mes, BYTES_POR_MIB};
use crate::error::{Error, Resultado};
use crate::ficheros;

/// Lo que la orden necesita del mundo.
pub struct Entorno<'a> {
    pub carpeta_de_revisiones: PathBuf,
    pub carpeta_de_modelos: PathBuf,
    pub margen_disco_mib: u64,
    pub avisar_cada_mib: u64,
    /// La fuente de un fichero, dado su repositorio y su nombre.
    pub fuente_de: &'a dyn Fn(&str, &str) -> Box<dyn Fuente>,
    pub espacio_libre: &'a dyn Fn(&Path) -> Resultado<u64>,
}

/// En qué quedó la orden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Traido {
    Descargado(Descargado),
    /// Ya estaba en la carpeta de modelos con la huella de la lista.
    YaEstaba(PathBuf),
}

pub fn traer(
    repositorio: &str,
    fichero: &str,
    entorno: &Entorno,
    avisar: &mut dyn FnMut(Aviso),
) -> Resultado<Traido> {
    let candidato = de_la_lista(&entorno.carpeta_de_revisiones, repositorio, fichero)?;
    let (Some(huella), Some(tamano)) = (&candidato.huella_sha256, candidato.tamano) else {
        return Err(Error::Revision(mensajes::sin_huella_o_tamano(
            repositorio,
            fichero,
        )));
    };
    let destino = entorno.carpeta_de_modelos.join(&candidato.fichero);
    if destino.exists() {
        let que_hay = ficheros::huella_sha256(&destino)?;
        if que_hay.eq_ignore_ascii_case(huella) {
            return Ok(Traido::YaEstaba(destino));
        }
        return Err(Error::Revision(mensajes::ya_hay_otro(&destino)));
    }
    let fuente = (entorno.fuente_de)(repositorio, &candidato.fichero);
    let descarga = Descarga {
        fuente: fuente.as_ref(),
        destino: &destino,
        huella_sha256: huella,
        tamano,
        margen_disco: entorno.margen_disco_mib.saturating_mul(BYTES_POR_MIB),
        avisar_cada: entorno.avisar_cada_mib.saturating_mul(BYTES_POR_MIB),
        espacio_libre: entorno.espacio_libre,
    };
    descargar(&descarga, avisar).map(Traido::Descargado)
}

/// El candidato con ese repositorio y ese fichero en la lista de la revisión más reciente.
fn de_la_lista(
    carpeta_de_revisiones: &Path,
    repositorio: &str,
    fichero: &str,
) -> Resultado<FicheroCandidato> {
    let ruta = mes::el_mas_reciente(carpeta_de_revisiones, NOMBRE_DE_LA_LISTA)?
        .ok_or_else(|| Error::Revision(mensajes::sin_revision(carpeta_de_revisiones)))?;
    let leida = lista::leer(&ficheros::leer(&ruta)?)?;
    leida
        .candidatos
        .into_iter()
        .map(|entrada| entrada.fichero)
        .find(|c| c.repositorio == repositorio && c.fichero == fichero)
        .ok_or_else(|| Error::Revision(mensajes::no_esta_en_la_lista(repositorio, fichero, &ruta)))
}

#[cfg(test)]
mod pruebas {
    use std::cell::Cell;

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::revision::descarga::Apertura;
    use crate::revision::lista::{EntradaDeLaLista, ListaDeCandidatos};

    const CONTENIDO: &[u8] = b"un modelo";

    fn huella(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    struct FuenteFija;

    impl Fuente for FuenteFija {
        fn abrir(&self, _: u64) -> Resultado<Apertura> {
            Ok(Apertura {
                reanuda: false,
                lector: Box::new(CONTENIDO),
            })
        }
    }

    fn sobra_espacio(_: &Path) -> Resultado<u64> {
        Ok(u64::MAX)
    }

    /// Escribe una lista con un candidato en `revisiones/<mes>`.
    fn escribir_lista(raiz: &Path, mes: &str, fichero: &str) {
        let carpeta = raiz.join("revisiones").join(mes);
        ficheros::crear_carpeta(&carpeta).unwrap();
        let candidato = FicheroCandidato {
            repositorio: "a/b".into(),
            fichero: fichero.into(),
            cuantizacion: "Q4_K_M".into(),
            tamano: Some(CONTENIDO.len() as u64),
            huella_sha256: Some(huella(CONTENIDO)),
            licencia: None,
            ultima_modificacion: None,
            descargas: None,
        };
        let lista = ListaDeCandidatos {
            fecha: format!("{mes}-01"),
            tarjeta: "t".into(),
            vram_mib: 1,
            cabe_mib: 1,
            candidatos: vec![EntradaDeLaLista::nueva(candidato, "https://x")],
            ya_descargados: vec![],
        };
        ficheros::escribir(
            &carpeta.join(NOMBRE_DE_LA_LISTA),
            &lista::escribir(&lista).unwrap(),
        )
        .unwrap();
    }

    fn traer_en(raiz: &Path, fichero: &str, pedidas: &Cell<u32>) -> Resultado<Traido> {
        let fuente_de = |_: &str, _: &str| -> Box<dyn Fuente> {
            pedidas.set(pedidas.get() + 1);
            Box::new(FuenteFija)
        };
        let entorno = Entorno {
            carpeta_de_revisiones: raiz.join("revisiones"),
            carpeta_de_modelos: raiz.join("modelos"),
            margen_disco_mib: 0,
            avisar_cada_mib: 512,
            fuente_de: &fuente_de,
            espacio_libre: &sobra_espacio,
        };
        traer("a/b", fichero, &entorno, &mut |_| {})
    }

    #[test]
    fn lo_que_esta_en_la_lista_mas_reciente_se_descarga_a_modelos() {
        let raiz = tempfile::tempdir().unwrap();
        escribir_lista(raiz.path(), "2026-10", "viejo.gguf");
        escribir_lista(raiz.path(), "2026-11", "m.gguf");
        let pedidas = Cell::new(0);
        let traido = traer_en(raiz.path(), "m.gguf", &pedidas).unwrap();
        let destino = raiz.path().join("modelos").join("m.gguf");
        assert!(matches!(traido, Traido::Descargado(ref d) if d.ruta == destino));
        assert_eq!(std::fs::read(destino).unwrap(), CONTENIDO);
    }

    #[test]
    fn lo_que_no_esta_en_la_lista_mas_reciente_se_niega_y_dice_donde_mirar() {
        let raiz = tempfile::tempdir().unwrap();
        escribir_lista(raiz.path(), "2026-10", "viejo.gguf");
        escribir_lista(raiz.path(), "2026-11", "m.gguf");
        let pedidas = Cell::new(0);
        let fallo = traer_en(raiz.path(), "viejo.gguf", &pedidas)
            .unwrap_err()
            .to_string();
        assert!(
            fallo.contains("2026-11") && fallo.contains("candidatos.md"),
            "{fallo}"
        );
        assert_eq!(pedidas.get(), 0);
    }

    #[test]
    fn sin_ninguna_revision_se_niega_y_dice_que_hay_que_buscar() {
        let raiz = tempfile::tempdir().unwrap();
        let pedidas = Cell::new(0);
        let fallo = traer_en(raiz.path(), "m.gguf", &pedidas)
            .unwrap_err()
            .to_string();
        assert!(fallo.contains("--revision-buscar"), "{fallo}");
        assert_eq!(pedidas.get(), 0);
    }

    #[test]
    fn el_que_ya_esta_con_su_huella_no_se_descarga() {
        let raiz = tempfile::tempdir().unwrap();
        escribir_lista(raiz.path(), "2026-11", "m.gguf");
        let modelos = raiz.path().join("modelos");
        ficheros::crear_carpeta(&modelos).unwrap();
        std::fs::write(modelos.join("m.gguf"), CONTENIDO).unwrap();
        let pedidas = Cell::new(0);
        let traido = traer_en(raiz.path(), "m.gguf", &pedidas).unwrap();
        assert_eq!(traido, Traido::YaEstaba(modelos.join("m.gguf")));
        assert_eq!(pedidas.get(), 0);
    }

    #[test]
    fn otro_fichero_con_el_mismo_nombre_no_se_pisa() {
        let raiz = tempfile::tempdir().unwrap();
        escribir_lista(raiz.path(), "2026-11", "m.gguf");
        let modelos = raiz.path().join("modelos");
        ficheros::crear_carpeta(&modelos).unwrap();
        std::fs::write(modelos.join("m.gguf"), b"otra cosa").unwrap();
        let pedidas = Cell::new(0);
        let fallo = traer_en(raiz.path(), "m.gguf", &pedidas)
            .unwrap_err()
            .to_string();
        assert!(fallo.contains("m.gguf"), "{fallo}");
        assert_eq!(std::fs::read(modelos.join("m.gguf")).unwrap(), b"otra cosa");
        assert_eq!(pedidas.get(), 0);
    }
}
