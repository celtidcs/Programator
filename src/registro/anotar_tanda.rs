//! `--registro-anotar`: lee una tanda del disco, la anota y escribe el registro.
//!
//! Solo entrada y salida: lo que se decide está en [`super::anotar`].

use std::path::Path;

use crate::banco::resultados::{leer_con_las_rubricas, NOMBRE_RESULTADOS};
use crate::error::Resultado;

use super::anotar::{anotar, entrada_de_la_tanda, Anotacion, MotivoParaNoAnotar};
use super::modelo::{self, EntradaDeModelo};

/// Lo que salió de anotar una tanda.
#[derive(Debug, Clone, PartialEq)]
pub enum Desenlace {
    Anotada(Anotacion, Box<EntradaDeModelo>),
    NoSeAnota(MotivoParaNoAnotar),
}

/// Anota la tanda de `carpeta` en el registro de `ruta_del_registro`. Lee las notas de las
/// rúbricas tal como están ahora, aunque no se haya rehecho el informe. Si la tanda no se puede
/// anotar, el registro no se toca.
pub fn anotar_tanda(carpeta: &Path, ruta_del_registro: &Path) -> Resultado<Desenlace> {
    let resultados = leer_con_las_rubricas(&carpeta.join(NOMBRE_RESULTADOS))?;
    let nombre = carpeta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| carpeta.display().to_string());
    let entrada = match entrada_de_la_tanda(&resultados, &nombre) {
        Ok(entrada) => entrada,
        Err(motivo) => return Ok(Desenlace::NoSeAnota(motivo)),
    };
    let mut registro = modelo::leer(ruta_del_registro)?;
    let anotacion = anotar(&mut registro, entrada.clone());
    modelo::escribir(ruta_del_registro, &registro)?;
    Ok(Desenlace::Anotada(anotacion, Box::new(entrada)))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::pruebas::tanda_de_ejemplo;
    use crate::banco::resultados::{guardar, AjustesDelMotor};

    fn guardar_tanda(carpeta: &Path, con_tarjeta: bool) {
        let mut resultados = tanda_de_ejemplo();
        // La de ejemplo deja una rúbrica pendiente: se quita para que se pueda anotar.
        resultados.encargos.retain(|e| e.id != "E01");
        resultados.tanda.tarjeta = con_tarjeta.then(|| "NVIDIA de prueba".to_string());
        resultados.tanda.ajustes_del_motor = Some(AjustesDelMotor {
            contexto: 32768,
            cache_kv: "q8_0".into(),
            paralelo: 1,
            empezar_en_frio: true,
            top_k: 40,
            top_p: 0.95,
            min_p: 0.05,
            penalizacion_repeticion: 1.0,
        });
        std::fs::create_dir_all(carpeta).unwrap();
        guardar(&resultados, carpeta).unwrap();
    }

    #[test]
    fn anotar_una_tanda_escribe_el_registro_con_el_nombre_de_su_carpeta() {
        let dir = tempfile::tempdir().unwrap();
        let tanda = dir.path().join("2026-11-12-1219-modelo");
        guardar_tanda(&tanda, true);
        let registro = dir.path().join("registro-de-modelos.toml");

        let desenlace = anotar_tanda(&tanda, &registro).unwrap();

        assert!(matches!(desenlace, Desenlace::Anotada(Anotacion::Nueva, _)));
        let leido = modelo::leer(&registro).unwrap();
        assert_eq!(leido.modelo.len(), 1);
        assert_eq!(leido.modelo[0].tanda, "2026-11-12-1219-modelo");
    }

    #[test]
    fn una_tanda_que_no_se_anota_no_toca_el_registro() {
        let dir = tempfile::tempdir().unwrap();
        let tanda = dir.path().join("antigua");
        guardar_tanda(&tanda, false);
        let registro = dir.path().join("registro-de-modelos.toml");

        let desenlace = anotar_tanda(&tanda, &registro).unwrap();

        assert_eq!(
            desenlace,
            Desenlace::NoSeAnota(MotivoParaNoAnotar::FaltaLaTarjeta)
        );
        assert!(!registro.exists());
    }
}
