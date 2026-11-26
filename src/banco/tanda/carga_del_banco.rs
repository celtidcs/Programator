//! El banco en el disco: dónde está a partir de una de sus tandas, y la lectura de su
//! `banco.toml` y sus encargos, con lo que falte para poder comprobarlos dicho antes de cargar el
//! modelo.

use std::path::{Path, PathBuf};

use super::{CARPETA_DE_ENCARGOS, CARPETA_DE_RESULTADOS, NOMBRE_BANCO_TOML};
use crate::banco::config::{ConfigBanco, ModoDeComprobacion};
use crate::banco::ficha::{cargar_encargos, Encargo};
use crate::banco::mensajes;
use crate::error::{Error, Resultado};
use crate::terminal::Terminal;

/// El `banco.toml` y los encargos de la carpeta del banco.
pub(super) fn cargar_banco(
    carpeta_banco: &Path,
    terminal: &dyn Terminal,
) -> Resultado<(ConfigBanco, Vec<Encargo>)> {
    let ruta_toml = carpeta_banco.join(NOMBRE_BANCO_TOML);
    let banco = if ruta_toml.is_file() {
        ConfigBanco::cargar(&ruta_toml)?
    } else {
        terminal.avisar(&mensajes::mensaje_sin_banco_toml(&ruta_toml));
        ConfigBanco::desde_texto("")?
    };
    let encargos = cargar_encargos(&carpeta_banco.join(CARPETA_DE_ENCARGOS))?;
    exigir_ordenes_de_herramientas(&banco, &encargos)?;
    Ok((banco, encargos))
}

/// Si el banco ejecuta código, cada herramienta que pidan las fichas tiene que tener una orden
/// que mire si está instalada: sin ella, el banco no sabría distinguir «no está instalada» de «no
/// se sabe comprobar», y se dice antes de cargar el modelo. Con las pruebas ocultas apagadas no
/// se ejecuta nada y no hace falta.
fn exigir_ordenes_de_herramientas(banco: &ConfigBanco, encargos: &[Encargo]) -> Resultado<()> {
    let ModoDeComprobacion::Local(local) = banco.modo_de_comprobacion() else {
        return Ok(());
    };
    let sin_orden = local.herramientas_sin_orden(encargos);
    if sin_orden.is_empty() {
        Ok(())
    } else {
        Err(Error::Banco(mensajes::mensaje_herramientas_sin_orden(
            &sin_orden,
        )))
    }
}

/// La carpeta del banco al que pertenece una tanda, que vive en `<banco>/resultados/<tanda>`.
pub(in crate::banco) fn carpeta_del_banco(carpeta_de_la_tanda: &Path) -> Resultado<PathBuf> {
    carpeta_de_la_tanda
        .parent()
        .filter(|resultados| {
            resultados
                .file_name()
                .is_some_and(|n| n == CARPETA_DE_RESULTADOS)
        })
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| Error::Banco(mensajes::tanda_fuera_de_un_banco(carpeta_de_la_tanda)))
}

#[cfg(test)]
mod pruebas {
    use super::super::dobles_de_prueba::{carpeta_tanda, oficio, oficio_en_php};
    use super::*;
    use crate::terminal::{Flujo, TerminalDeMentira};

    #[test]
    fn una_carpeta_fuera_de_resultados_no_se_toma_por_una_tanda() {
        let carpeta = tempfile::tempdir().unwrap();
        let fallo = carpeta_del_banco(&carpeta.path().join("otra").join("t1")).unwrap_err();
        assert!(fallo.to_string().contains("resultados"), "{fallo}");
        assert_eq!(
            carpeta_del_banco(&carpeta_tanda(carpeta.path())).unwrap(),
            carpeta.path()
        );
    }

    #[test]
    fn un_lenguaje_sin_orden_para_el_banco_antes_de_cargar_el_modelo_y_dice_que_anadir() {
        let carpeta = tempfile::tempdir().unwrap();
        oficio_en_php(carpeta.path());
        oficio(carpeta.path());
        std::fs::write(
            carpeta.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\n",
        )
        .unwrap();

        let fallo = cargar_banco(carpeta.path(), &TerminalDeMentira::default())
            .unwrap_err()
            .to_string();
        // Sin banco.toml van encendidas, así que también se exige.
        std::fs::remove_file(carpeta.path().join(NOMBRE_BANCO_TOML)).unwrap();
        let sin_toml = cargar_banco(carpeta.path(), &TerminalDeMentira::default())
            .unwrap_err()
            .to_string();
        assert_eq!(sin_toml, fallo);

        assert!(fallo.contains("P01 pide «php»"), "{fallo}");
        assert!(!fallo.contains("«rust»"), "rust es de serie: {fallo}");
        assert!(
            fallo.contains("[comprobacion_local.herramientas]"),
            "{fallo}"
        );
        assert!(fallo.contains("php = \"php --version\""), "{fallo}");
    }

    #[test]
    fn un_lenguaje_declarado_en_banco_toml_se_acepta() {
        let carpeta = tempfile::tempdir().unwrap();
        oficio_en_php(carpeta.path());
        std::fs::write(
            carpeta.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\n[comprobacion_local.herramientas]\nphp = \"php --version\"\n",
        )
        .unwrap();

        let (_, encargos) = cargar_banco(carpeta.path(), &TerminalDeMentira::default()).unwrap();

        assert_eq!(encargos.len(), 1);
    }

    #[test]
    fn con_las_pruebas_ocultas_apagadas_un_lenguaje_sin_orden_no_impide_pasar_el_banco() {
        // Apagadas no se ejecuta código: el modelo escribe el PHP igual, y lo que no se puede es
        // comprobar si funciona, que sale «no evaluado» con su motivo.
        let carpeta = tempfile::tempdir().unwrap();
        oficio_en_php(carpeta.path());
        std::fs::write(
            carpeta.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\nactiva = false\n",
        )
        .unwrap();

        let (_, encargos) = cargar_banco(carpeta.path(), &TerminalDeMentira::default()).unwrap();

        assert_eq!(encargos.len(), 1);
    }

    /// Que falte `banco.toml` se avisa por la salida de errores de la terminal
    /// que recibe el banco, no directamente por `stderr`.
    #[test]
    fn sin_banco_toml_se_avisa_por_la_salida_de_errores_de_la_terminal_inyectada() {
        let carpeta = tempfile::tempdir().unwrap();
        oficio(carpeta.path());
        let terminal = TerminalDeMentira::default();

        cargar_banco(carpeta.path(), &terminal).unwrap();

        assert_eq!(
            terminal.capturado(),
            vec![(
                Flujo::Errores,
                mensajes::mensaje_sin_banco_toml(&carpeta.path().join(NOMBRE_BANCO_TOML))
            )]
        );
    }
}
