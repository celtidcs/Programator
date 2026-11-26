//! El registro de entregas en disco: un solo TOML, `entregas.toml`, en el espacio del agente.
//! Aquí no se decide nada.
//!
//! Se escribe de una vez, con un temporal al lado y un cambio de nombre
//! (`ficheros::escribir_de_una_vez`). Un registro que no se entiende se aparta con la extensión
//! `.ilegible` y se empieza otro vacío, con un aviso: las propuestas siguen en disco y ahí puede
//! haber algo que una persona quiera mirar.

use super::{EntradaDeEntrega, RegistroDeEntregas};
use crate::error::{Error, Resultado};
use crate::ficheros;
use std::path::Path;

/// Lo que se añade al nombre de un registro que no se entiende al apartarlo.
const SUFIJO_DEL_ILEGIBLE: &str = ".ilegible";

/// Lo que se leyó: el registro y, si el que había no se entendía, el aviso de que se apartó.
#[derive(Debug, Default)]
pub struct Leido {
    pub registro: RegistroDeEntregas,
    pub aviso: Option<String>,
}

/// Lee el registro. Si no existe, está vacío.
pub fn leer(ruta: &Path) -> Resultado<Leido> {
    let texto = ficheros::leer_si_existe(ruta)?;
    match toml::from_str::<RegistroDeEntregas>(&texto) {
        Ok(registro) => Ok(Leido {
            registro,
            aviso: None,
        }),
        Err(causa) => {
            let apartado = ruta.with_extension(format!("toml{SUFIJO_DEL_ILEGIBLE}"));
            ficheros::renombrar(ruta, &apartado)?;
            Ok(Leido {
                registro: RegistroDeEntregas::default(),
                aviso: Some(mensaje_registro_apartado(ruta, &apartado, &causa)),
            })
        }
    }
}

/// Guarda el registro entero, de una vez.
pub fn guardar(ruta: &Path, registro: &RegistroDeEntregas) -> Resultado<()> {
    let texto = toml::to_string(registro)
        .map_err(|causa| Error::RegistroDeEntregas(mensaje_sin_componer(&causa)))?;
    ficheros::escribir_de_una_vez(ruta, &texto)
}

/// Añade una entrada al final del registro. Devuelve el aviso, si el que había se apartó.
pub fn anotar(ruta: &Path, entrada: EntradaDeEntrega) -> Resultado<Option<String>> {
    let Leido {
        mut registro,
        aviso,
    } = leer(ruta)?;
    registro.entregas.push(entrada);
    guardar(ruta, &registro)?;
    Ok(aviso)
}

fn mensaje_registro_apartado(ruta: &Path, apartado: &Path, causa: &toml::de::Error) -> String {
    format!(
        "⚠️ El registro de entregas «{}» no se entiende ({causa}): se ha apartado como «{}» y se \
         empieza otro. Las propuestas siguen en la carpeta de candidatos.",
        ruta.display(),
        apartado.display()
    )
}

fn mensaje_sin_componer(causa: &toml::ser::Error) -> String {
    format!("no se pudo componer el registro de entregas: {causa}")
}

#[cfg(test)]
mod pruebas {
    use super::super::{EstadoDeLaPropuesta, VeredictoDeLaEntrega, LO_DIO_PROGRAMATOR};
    use super::*;

    fn entrada(propuesta: &str) -> EntradaDeEntrega {
        EntradaDeEntrega {
            propuesta: propuesta.to_string(),
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: "claude.md".to_string(),
            modelo: None,
            fecha: "2026-11-25 16:00:00".to_string(),
            veredicto: VeredictoDeLaEntrega::Pasa,
            huella: "h".to_string(),
            estado: EstadoDeLaPropuesta::Propuesta,
            estado_lo_dio: LO_DIO_PROGRAMATOR.to_string(),
            destino: None,
            motivo: None,
            archivada: None,
            ya_en_el_proyecto: Vec::new(),
        }
    }

    #[test]
    fn sin_fichero_el_registro_esta_vacio() {
        let carpeta = tempfile::tempdir().unwrap();
        let leido = leer(&carpeta.path().join("entregas.toml")).unwrap();
        assert!(leido.registro.entregas.is_empty());
        assert!(leido.aviso.is_none());
    }

    #[test]
    fn lo_anotado_se_lee_en_orden() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("sub/entregas.toml");

        anotar(&ruta, entrada("a.rs")).unwrap();
        anotar(&ruta, entrada("b.rs")).unwrap();

        let nombres: Vec<String> = leer(&ruta)
            .unwrap()
            .registro
            .entregas
            .into_iter()
            .map(|e| e.propuesta)
            .collect();
        assert_eq!(nombres, ["a.rs", "b.rs"]);
    }

    #[test]
    fn un_registro_que_no_se_entiende_se_aparta_y_se_empieza_otro() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("entregas.toml");
        std::fs::write(&ruta, "esto no es toml [[[").unwrap();

        let aviso = anotar(&ruta, entrada("a.rs")).unwrap();

        assert!(aviso.unwrap().contains("se ha apartado"));
        assert!(carpeta.path().join("entregas.toml.ilegible").exists());
        assert_eq!(leer(&ruta).unwrap().registro.entregas.len(), 1);
    }
}
