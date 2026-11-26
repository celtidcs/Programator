//! Poner al día el registro de entregas en una pasada: deducir lo que se pueda y guardarlo si
//! cambió. Junta la búsqueda (E/S) con la deducción (pura) y decide cuándo toca buscar.
//!
//! Las sustituidas se deducen en cada pasada: solo miran el registro. Las aceptadas obligan a
//! recorrer el proyecto, así que se buscan como mucho cada `[entregas] deducir_cada_minutos`, y
//! solo si queda algo en `propuesta`. Después, lo que lleve `dias_sin_confirmar` sin nada queda sin
//! confirmar, y todo lo resuelto pasa al archivo de la carpeta de candidatos.

use super::archivo::{archivar, caducar};
use super::busqueda::buscar;
use super::deduccion::{deducir, ficheros_buscados};
use super::disco::{guardar, leer, Leido};
use crate::config::Entregas;
use crate::error::Resultado;
use chrono::NaiveDateTime;
use std::path::Path;
use std::time::Duration;

/// El formato con que se guarda la hora de la última búsqueda: el del latido.
const FORMATO_DE_LA_HORA: &str = crate::protocolo::canal::FORMATO_DE_LA_HORA_DEL_LATIDO;

/// Segundos en un minuto, para pasar `deducir_cada_minutos` a una duración.
const SEGUNDOS_POR_MINUTO: i64 = 60;

/// Pone al día el registro de `ruta` con lo que se encuentre en `raiz`, y ordena `candidatos`.
/// Devuelve los avisos que haya que dar (un registro ilegible que se apartó).
pub fn poner_al_dia(
    ruta: &Path,
    raiz: &Path,
    candidatos: &Path,
    ajustes: &Entregas,
    ahora: NaiveDateTime,
) -> Resultado<Vec<String>> {
    if !ruta.exists() {
        return Ok(Vec::new());
    }
    let Leido {
        mut registro,
        aviso,
    } = leer(ruta)?;
    let buscados = ficheros_buscados(&registro);
    let toca =
        !buscados.is_empty() && toca_buscar(registro.ultima_busqueda.as_deref(), ajustes, ahora);
    let encontrados = if toca {
        let tiempo = Duration::from_secs(ajustes.tiempo_de_busqueda_segundos);
        Some(buscar(raiz, &buscados, tiempo)?)
    } else {
        None
    };
    let mut cambio = deducir(&mut registro, encontrados.as_deref());
    if toca {
        registro.ultima_busqueda = Some(ahora.format(FORMATO_DE_LA_HORA).to_string());
        cambio = true;
    }
    cambio |= caducar(&mut registro, ahora, ajustes.dias_sin_confirmar);
    // Lo archivado ya se movió: se guarda aunque el archivo falle a medias.
    let archivado = archivar(candidatos, &mut registro);
    cambio |= matches!(archivado, Ok(true));
    if cambio {
        guardar(ruta, &registro)?;
    }
    archivado?;
    Ok(aviso.into_iter().collect())
}

/// Si ya toca buscar: nunca se buscó, la hora guardada no se entiende o pasó el intervalo.
pub fn toca_buscar(ultima: Option<&str>, ajustes: &Entregas, ahora: NaiveDateTime) -> bool {
    let Some(ultima) =
        ultima.and_then(|t| NaiveDateTime::parse_from_str(t, FORMATO_DE_LA_HORA).ok())
    else {
        return true;
    };
    let minutos =
        i64::try_from(ajustes.deducir_cada_minutos).unwrap_or(i64::MAX / SEGUNDOS_POR_MINUTO);
    let intervalo = chrono::Duration::seconds(minutos.saturating_mul(SEGUNDOS_POR_MINUTO));
    ahora.signed_duration_since(ultima) >= intervalo
}

#[cfg(test)]
mod pruebas {
    use super::super::{disco, huella, EncargoDeLaEntrega, EstadoDeLaPropuesta};
    use super::*;
    use crate::arnes::verificacion::Veredicto;

    fn hora(texto: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(texto, FORMATO_DE_LA_HORA).unwrap()
    }

    fn cada(minutos: u64) -> Entregas {
        Entregas {
            deducir_cada_minutos: minutos,
            ..Entregas::default()
        }
    }

    #[test]
    fn se_busca_la_primera_vez_y_luego_cada_intervalo() {
        let ahora = hora("2026-11-25 12:00:00");
        assert!(toca_buscar(None, &cada(60), ahora));
        assert!(toca_buscar(Some("no es una hora"), &cada(60), ahora));
        assert!(!toca_buscar(Some("2026-11-25 11:30:00"), &cada(60), ahora));
        assert!(toca_buscar(Some("2026-11-25 11:00:00"), &cada(60), ahora));
        assert!(toca_buscar(Some("2026-11-25 12:00:00"), &cada(0), ahora));
    }

    /// E2 con el disco: la propuesta copiada al proyecto, con CRLF, queda aceptada tras la pasada
    /// siguiente; y no se vuelve a buscar hasta que pase el intervalo.
    #[test]
    fn la_propuesta_copiada_queda_aceptada_y_se_guarda_cuando_se_busco() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join(".gestor/programator/entregas.toml");
        let encargo = EncargoDeLaEntrega {
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: "claude.md".to_string(),
            modelo: None,
        };
        let veredicto = Veredicto::SinComprobador {
            extension: "py".to_string(),
        };
        disco::anotar(
            &ruta,
            encargo.entrada("x-002.py", "a\nb\n", &veredicto, "f"),
        )
        .unwrap();
        std::fs::create_dir_all(carpeta.path().join("src")).unwrap();
        std::fs::write(carpeta.path().join("src/x.py"), "a\r\nb\r\n").unwrap();

        let avisos = poner_al_dia(
            &ruta,
            carpeta.path(),
            carpeta.path(),
            &cada(60),
            hora("2026-11-25 12:00:00"),
        )
        .unwrap();

        assert!(avisos.is_empty());
        let registro = disco::leer(&ruta).unwrap().registro;
        assert_eq!(registro.entregas[0].estado, EstadoDeLaPropuesta::Aceptada);
        assert_eq!(registro.entregas[0].destino.as_deref(), Some("src/x.py"));
        assert_eq!(
            registro.ultima_busqueda.as_deref(),
            Some("2026-11-25 12:00:00")
        );
        assert_eq!(registro.entregas[0].huella, huella("a\nb\n"));
    }

    #[test]
    fn dentro_del_intervalo_no_se_busca() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("entregas.toml");
        let encargo = EncargoDeLaEntrega {
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: "claude.md".to_string(),
            modelo: None,
        };
        let veredicto = Veredicto::SinComprobador {
            extension: "py".to_string(),
        };
        disco::anotar(&ruta, encargo.entrada("x.py", "a\n", &veredicto, "f")).unwrap();
        poner_al_dia(
            &ruta,
            carpeta.path(),
            carpeta.path(),
            &cada(60),
            hora("2026-11-25 12:00:00"),
        )
        .unwrap();
        std::fs::write(carpeta.path().join("x.py"), "a\n").unwrap();

        poner_al_dia(
            &ruta,
            carpeta.path(),
            carpeta.path(),
            &cada(60),
            hora("2026-11-25 12:30:00"),
        )
        .unwrap();
        assert_eq!(
            disco::leer(&ruta).unwrap().registro.entregas[0].estado,
            EstadoDeLaPropuesta::Propuesta
        );

        poner_al_dia(
            &ruta,
            carpeta.path(),
            carpeta.path(),
            &cada(60),
            hora("2026-11-25 13:00:00"),
        )
        .unwrap();
        assert_eq!(
            disco::leer(&ruta).unwrap().registro.entregas[0].estado,
            EstadoDeLaPropuesta::Aceptada
        );
    }

    /// Lo único que cambia es que algo resuelto sale al archivo: también se guarda, o el registro
    /// diría que sigue arriba.
    #[test]
    fn lo_archivado_se_guarda_aunque_no_cambie_nada_mas() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("entregas.toml");
        let candidatos = carpeta.path().join(".gestor/candidatos/programator");
        std::fs::create_dir_all(&candidatos).unwrap();
        std::fs::write(candidatos.join("x.py"), "a\n").unwrap();
        let encargo = EncargoDeLaEntrega {
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: "claude.md".to_string(),
            modelo: None,
        };
        let veredicto = Veredicto::SinComprobador {
            extension: "py".to_string(),
        };
        let mut entrada = encargo.entrada("x.py", "a\n", &veredicto, "2026-11-24 10:00:00");
        entrada.estado = EstadoDeLaPropuesta::Rechazada;
        disco::anotar(&ruta, entrada).unwrap();

        poner_al_dia(
            &ruta,
            carpeta.path(),
            &candidatos,
            &Entregas::default(),
            hora("2026-11-25 12:00:00"),
        )
        .unwrap();

        assert!(candidatos.join("archivo/2026-11/x.py").exists());
        assert_eq!(
            disco::leer(&ruta).unwrap().registro.entregas[0]
                .archivada
                .as_deref(),
            Some("archivo/2026-11/x.py")
        );
    }

    /// E5 y E6: pasado el plazo queda sin confirmar y sale al archivo; lo que espera, no.
    #[test]
    fn pasado_el_plazo_queda_sin_confirmar_y_sale_al_archivo() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("entregas.toml");
        let candidatos = carpeta.path().join(".gestor/candidatos/programator");
        std::fs::create_dir_all(&candidatos).unwrap();
        let encargo = EncargoDeLaEntrega {
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: "claude.md".to_string(),
            modelo: None,
        };
        let veredicto = Veredicto::SinComprobador {
            extension: "py".to_string(),
        };
        for (nombre, fecha) in [
            ("vieja.py", "2026-10-01 10:00:00"),
            ("nueva.py", "2026-11-24 10:00:00"),
        ] {
            std::fs::write(candidatos.join(nombre), "a\n").unwrap();
            disco::anotar(&ruta, encargo.entrada(nombre, "a\n", &veredicto, fecha)).unwrap();
        }
        let ajustes = Entregas {
            deducir_cada_minutos: 60,
            dias_sin_confirmar: 30,
            ..Entregas::default()
        };

        poner_al_dia(
            &ruta,
            carpeta.path(),
            &candidatos,
            &ajustes,
            hora("2026-11-25 12:00:00"),
        )
        .unwrap();

        let registro = disco::leer(&ruta).unwrap().registro;
        assert_eq!(
            registro.entregas[0].estado,
            EstadoDeLaPropuesta::SinConfirmar
        );
        assert_eq!(registro.entregas[1].estado, EstadoDeLaPropuesta::Propuesta);
        assert!(candidatos.join("archivo/2026-10/vieja.py").exists());
        assert!(!candidatos.join("vieja.py").exists());
        assert!(candidatos.join("nueva.py").exists());
    }

    #[test]
    fn sin_registro_no_hace_nada() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("entregas.toml");
        assert!(poner_al_dia(
            &ruta,
            carpeta.path(),
            carpeta.path(),
            &cada(0),
            hora("2026-11-25 12:00:00")
        )
        .unwrap()
        .is_empty());
        assert!(!ruta.exists());
    }
}
