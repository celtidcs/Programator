//! Cambiar `[motor] modelo` en `programator.toml` sin tocar nada más del fichero.
//!
//! Lo usa `revisión: cerrar` cuando el modelo por defecto ya no gana nada y hay sustituto
//! (especificación de la revisión autónoma, §6). El fichero lo escribe el usuario a mano y está
//! lleno de comentarios que explican cada clave: reescribirlo desde la estructura los perdería, así
//! que se edita el documento tal cual con `toml_edit`.

use std::path::Path;

use toml_edit::{value, DocumentMut};

use crate::error::{Error, Resultado};
use crate::ficheros;

/// La tabla y la clave que se cambian.
const TABLA_DEL_MOTOR: &str = "motor";
const CLAVE_DEL_MODELO: &str = "modelo";

/// El texto de la configuración con `[motor] modelo` cambiado a `modelo`. El resto queda igual,
/// comentarios y orden incluidos, y el comentario que acompaña a la clave también.
pub fn cambiar(texto: &str, modelo: &str) -> Resultado<String> {
    let mut documento: DocumentMut = texto.parse().map_err(|causa| {
        Error::Configuracion(format!("no se puede editar la configuración: {causa}"))
    })?;
    let motor = documento
        .get_mut(TABLA_DEL_MOTOR)
        .and_then(|tabla| tabla.as_table_like_mut())
        .ok_or_else(|| Error::Configuracion(format!("falta la tabla «[{TABLA_DEL_MOTOR}]»")))?;
    match motor.get_mut(CLAVE_DEL_MODELO) {
        Some(anterior) => {
            let adorno = anterior.as_value().map(|v| v.decor().clone());
            *anterior = value(modelo);
            if let (Some(adorno), Some(nuevo)) = (adorno, anterior.as_value_mut()) {
                *nuevo.decor_mut() = adorno;
            }
        }
        None => {
            motor.insert(CLAVE_DEL_MODELO, value(modelo));
        }
    }
    Ok(documento.to_string())
}

/// Lo mismo sobre el fichero.
pub fn cambiar_en(ruta: &Path, modelo: &str) -> Resultado<()> {
    let texto = ficheros::leer(ruta)?;
    ficheros::escribir(ruta, &cambiar(&texto, modelo)?)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const CONFIGURACION: &str = "# Cabecera escrita a mano.\n\
        [motor]\n\
        # Ruta al fichero GGUF del modelo.\n\
        modelo = \"modelos/viejo.gguf\" # el de siempre\n\
        # Puerto.\n\
        puerto = 8080\n\
        \n\
        [canal]\n\
        agente = \"programator\"\n";

    #[test]
    fn solo_cambia_el_modelo_y_los_comentarios_se_quedan() {
        let nuevo = cambiar(CONFIGURACION, "modelos/nuevo.gguf").unwrap();

        assert_eq!(
            nuevo,
            CONFIGURACION.replace("modelos/viejo.gguf", "modelos/nuevo.gguf")
        );
    }

    #[test]
    fn sin_la_clave_se_anade_en_su_tabla() {
        let nuevo = cambiar("[motor]\npuerto = 8080\n", "modelos/nuevo.gguf").unwrap();

        let leido: toml::Value = toml::from_str(&nuevo).unwrap();
        assert_eq!(
            leido["motor"]["modelo"].as_str(),
            Some("modelos/nuevo.gguf")
        );
        assert_eq!(leido["motor"]["puerto"].as_integer(), Some(8080));
    }

    #[test]
    fn sin_tabla_del_motor_o_con_un_fichero_roto_se_dice() {
        assert!(cambiar("[canal]\n", "m.gguf")
            .unwrap_err()
            .to_string()
            .contains("[motor]"));
        assert!(cambiar("[motor\n", "m.gguf").is_err());
    }

    #[test]
    fn el_fichero_se_cambia_en_su_sitio() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("programator.toml");
        std::fs::write(&ruta, CONFIGURACION).unwrap();

        cambiar_en(&ruta, "modelos/nuevo.gguf").unwrap();

        let leido = std::fs::read_to_string(&ruta).unwrap();
        assert!(leido.contains("modelo = \"modelos/nuevo.gguf\" # el de siempre"));
        assert!(leido.contains("# Ruta al fichero GGUF del modelo."));
    }
}
