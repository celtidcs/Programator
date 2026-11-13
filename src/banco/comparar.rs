//! `programator --banco-comparar <tanda-a> <tanda-b>`: lee dos tandas guardadas y cuenta si son
//! repetición una de otra.
//!
//! Aquí solo se lee de disco y se imprime. Lo que decide está en `comparacion`, que no toca nada,
//! y el texto en `mensajes`.

use std::path::Path;

use super::comparacion::{self, PasadaParaComparar, TandaParaComparar};
use super::ejecucion::carpeta_del_encargo;
use super::mensajes;
use super::pasadas;
use super::tanda::{carpeta_del_banco, CARPETA_DE_TRABAJO};
use crate::arnes::espacio;
use crate::error::{Error, Resultado};
use crate::ficheros::leer;

/// Compara dos tandas e imprime el veredicto.
pub fn comparar(tanda_a: &Path, tanda_b: &Path, agente: &str) -> Resultado<()> {
    let a = leer_para_comparar(tanda_a, agente)?;
    let b = leer_para_comparar(tanda_b, agente)?;
    let veredicto = comparacion::comparar(&a, &b);
    println!(
        "{}",
        mensajes::mensaje_comparacion(&veredicto, tanda_a, tanda_b)
    );
    Ok(())
}

/// Los datos y las conversaciones de una tanda guardada. Exige, como `--banco-comprobar`, que la
/// carpeta esté en `resultados/` de un banco.
pub fn leer_para_comparar(
    carpeta_de_la_tanda: &Path,
    agente: &str,
) -> Resultado<TandaParaComparar> {
    carpeta_del_banco(carpeta_de_la_tanda)?;
    let guardadas = pasadas::cargar(carpeta_de_la_tanda)?;
    let trabajo = carpeta_de_la_tanda.join(CARPETA_DE_TRABAJO);
    let pasadas = guardadas
        .pasadas
        .iter()
        .map(|guardada| {
            let carpeta = carpeta_del_encargo(&trabajo, &guardada.encargo, guardada.semilla);
            Ok((
                (guardada.encargo.clone(), guardada.semilla),
                PasadaParaComparar {
                    conversacion: conversacion_de(&carpeta, agente)?,
                    a_tiempo: guardada.a_tiempo,
                },
            ))
        })
        .collect::<Resultado<_>>()?;
    Ok(TandaParaComparar {
        datos: guardadas.tanda,
        pasadas,
    })
}

/// Las conversaciones que dejó una pasada, por orden de nombre (la marca lleva la hora), una
/// detrás de otra. `None` si no dejó ninguna.
fn conversacion_de(carpeta_de_la_pasada: &Path, agente: &str) -> Resultado<Option<String>> {
    let carpeta = espacio::carpeta_del_agente(carpeta_de_la_pasada, agente);
    if !carpeta.is_dir() {
        return Ok(None);
    }
    let entradas = std::fs::read_dir(&carpeta).map_err(|causa| Error::Lectura {
        ruta: carpeta.clone(),
        causa,
    })?;
    let mut nombres: Vec<String> = entradas
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|nombre| espacio::es_fichero_de_conversacion(nombre))
        .collect();
    if nombres.is_empty() {
        return Ok(None);
    }
    nombres.sort();
    let mut todo = String::new();
    for nombre in nombres {
        todo.push_str(&leer(&carpeta.join(nombre))?);
    }
    Ok(Some(todo))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::pasadas::{PasadaGuardada, PasadasDeTanda};
    use crate::banco::resultados::{AjustesDelMotor, DatosDeTanda};
    use crate::banco::tanda::CARPETA_DE_RESULTADOS;

    const AGENTE: &str = "Programator";

    fn datos() -> DatosDeTanda {
        DatosDeTanda {
            modelo: "devstral".into(),
            huella_sha256: "ab".repeat(32),
            compilacion_motor: "b10993".into(),
            version_programator: "0.11.0".into(),
            fecha: "2026-11-12 13:00".into(),
            semillas: vec![42],
            temperatura: 0.2,
            margen_lineas: 2,
            tiempo_maximo_encargo_segundos: 900,
            maquina_de_comprobacion: None,
            capas_en_gpu: Some(34),
            vram_ocupada_mib: None,
            segundos_de_carga: None,
            tarjeta: None,
            sistema_de_comprobacion: None,
            rastro_fuera: Vec::new(),
            aislamiento: None,
            ajustes_del_motor: Some(AjustesDelMotor {
                contexto: 32768,
                cache_kv: "q8_0".into(),
                paralelo: 1,
                empezar_en_frio: true,
                top_k: 40,
                top_p: 0.95,
                min_p: 0.05,
                penalizacion_repeticion: 1.0,
            }),
        }
    }

    /// Una tanda guardada en `<dir>/resultados/<nombre>` con una pasada de E01 y la semilla 42.
    fn tanda_en_disco(dir: &Path, nombre: &str, conversacion: &str) -> std::path::PathBuf {
        let carpeta = dir.join(CARPETA_DE_RESULTADOS).join(nombre);
        std::fs::create_dir_all(&carpeta).unwrap();
        let guardadas = PasadasDeTanda {
            tanda: datos(),
            pasadas: vec![PasadaGuardada {
                encargo: "E01".into(),
                semilla: 42,
                a_tiempo: true,
                segundos: 60.0,
                respuestas: 2,
                respuestas_sin_consumo: 0,
                tokens_entrada: None,
                tokens_en_cache: None,
                tokens_generados: None,
                ms_entrada: None,
                ms_generacion: None,
                entregas: Vec::new(),
            }],
        };
        pasadas::guardar(&guardadas, &carpeta).unwrap();
        let pasada = carpeta_del_encargo(&carpeta.join(CARPETA_DE_TRABAJO), "E01", 42);
        let ruta = espacio::ruta_de_la_conversacion(&pasada, AGENTE, nombre);
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        std::fs::write(ruta, conversacion).unwrap();
        carpeta
    }

    #[test]
    fn dos_tandas_guardadas_con_la_misma_conversacion_son_repeticion() {
        let dir = tempfile::tempdir().unwrap();
        let a = tanda_en_disco(dir.path(), "a", "{\"vuelta\":1}\n");
        let b = tanda_en_disco(dir.path(), "b", "{\"vuelta\":1}\n");

        let veredicto = comparacion::comparar(
            &leer_para_comparar(&a, AGENTE).unwrap(),
            &leer_para_comparar(&b, AGENTE).unwrap(),
        );

        assert!(veredicto.es_repeticion(), "{veredicto:?}");
    }

    #[test]
    fn una_conversacion_distinta_en_disco_no_es_repeticion() {
        let dir = tempfile::tempdir().unwrap();
        let a = tanda_en_disco(dir.path(), "a", "{\"vuelta\":1}\n");
        let b = tanda_en_disco(dir.path(), "b", "{\"vuelta\":1,\"otro\":true}\n");

        let veredicto = comparacion::comparar(
            &leer_para_comparar(&a, AGENTE).unwrap(),
            &leer_para_comparar(&b, AGENTE).unwrap(),
        );

        assert!(!veredicto.es_repeticion());
    }

    #[test]
    fn una_carpeta_fuera_de_resultados_no_se_toma_por_una_tanda() {
        let dir = tempfile::tempdir().unwrap();

        assert!(leer_para_comparar(dir.path(), AGENTE).is_err());
    }
}
