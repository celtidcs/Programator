//! La carpeta de candidatos ordenada: lo resuelto (aceptado, rechazado, sustituido o sin
//! confirmar a tiempo) pasa a `archivo/AAAA-MM/`, con el mes en que se propuso, y arriba solo
//! queda lo que espera revisión.
//!
//! El banco de modelos cuenta como entrega solo los ficheros del primer nivel de la carpeta
//! (`banco::ejecucion::ficheros_de`), así que la subcarpeta no cambia sus cuentas. El modelo no
//! puede escribir en ella (`herramientas::propuesta::CARPETA_DEL_ARCHIVO`).

use super::{EntradaDeEntrega, EstadoDeLaPropuesta, RegistroDeEntregas, LO_DIO_PROGRAMATOR};
use crate::arnes::herramientas::{con_variante, CARPETA_DEL_ARCHIVO, PRIMERA_VARIANTE};
use crate::error::Resultado;
use chrono::NaiveDateTime;
use std::path::{Path, PathBuf};

/// El formato de la fecha de las entradas: el del latido.
const FORMATO_DE_LA_FECHA: &str = crate::protocolo::canal::FORMATO_DE_LA_HORA_DEL_LATIDO;
/// El formato de la carpeta del mes dentro del archivo.
const FORMATO_DEL_MES: &str = "%Y-%m";
/// La carpeta del mes cuando la fecha de la entrada no se entiende.
const MES_DESCONOCIDO: &str = "sin-fecha";
/// Cuántas variantes `-NNN` se prueban si el nombre ya está ocupado en el archivo.
const MAX_VARIANTES_EN_EL_ARCHIVO: u32 = 999;

/// Pasa a `sin_confirmar` lo que sigue en `propuesta` pasados `dias` desde que se escribió.
/// Devuelve si cambió algo. Puro.
pub fn caducar(registro: &mut RegistroDeEntregas, ahora: NaiveDateTime, dias: u64) -> bool {
    let plazo = chrono::Duration::days(i64::try_from(dias).unwrap_or(i64::MAX / 86_400));
    let mut cambio = false;
    for entrada in registro
        .entregas
        .iter_mut()
        .filter(|e| e.estado == EstadoDeLaPropuesta::Propuesta)
    {
        let Some(escrita) = fecha_de(entrada) else {
            continue;
        };
        if ahora.signed_duration_since(escrita) >= plazo {
            entrada.estado = EstadoDeLaPropuesta::SinConfirmar;
            entrada.estado_lo_dio = LO_DIO_PROGRAMATOR.to_string();
            cambio = true;
        }
    }
    cambio
}

/// Si la entrada está resuelta y todavía no se archivó.
pub fn por_archivar(entrada: &EntradaDeEntrega) -> bool {
    entrada.estado != EstadoDeLaPropuesta::Propuesta && entrada.archivada.is_none()
}

/// Dónde va en el archivo, relativo a la carpeta de candidatos: `archivo/AAAA-MM/<propuesta>`.
pub fn destino_en_el_archivo(entrada: &EntradaDeEntrega) -> String {
    let mes = fecha_de(entrada)
        .map(|fecha| fecha.format(FORMATO_DEL_MES).to_string())
        .unwrap_or_else(|| MES_DESCONOCIDO.to_string());
    format!("{CARPETA_DEL_ARCHIVO}/{mes}/{}", entrada.propuesta)
}

/// Mueve al archivo lo resuelto que siga arriba en `candidatos`, y anota dónde quedó. Lo que ya no
/// está en la carpeta (alguien lo movió o lo borró) se deja como está. Devuelve si cambió algo.
pub fn archivar(candidatos: &Path, registro: &mut RegistroDeEntregas) -> Resultado<bool> {
    let mut cambio = false;
    for entrada in registro.entregas.iter_mut().filter(|e| por_archivar(e)) {
        let origen = candidatos.join(&entrada.propuesta);
        if !std::fs::symlink_metadata(&origen).is_ok_and(|datos| datos.is_file()) {
            continue;
        }
        let deseado = candidatos.join(destino_en_el_archivo(entrada));
        let Some(destino) = libre(&deseado) else {
            continue;
        };
        if let Some(padre) = destino.parent() {
            crate::ficheros::crear_carpeta(padre)?;
        }
        crate::ficheros::renombrar(&origen, &destino)?;
        entrada.archivada = Some(relativa(candidatos, &destino));
        cambio = true;
    }
    Ok(cambio)
}

/// `ruta` si está libre; si no, la primera variante `-NNN` libre.
fn libre(ruta: &Path) -> Option<PathBuf> {
    if !ruta.exists() {
        return Some(ruta.to_path_buf());
    }
    ruta.file_stem()?;
    (PRIMERA_VARIANTE..=MAX_VARIANTES_EN_EL_ARCHIVO)
        .map(|variante| con_variante(ruta, variante))
        .find(|candidata| !candidata.exists())
}

fn relativa(base: &Path, ruta: &Path) -> String {
    ruta.strip_prefix(base)
        .map(|resto| {
            resto
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_else(|_| ruta.display().to_string())
}

fn fecha_de(entrada: &EntradaDeEntrega) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(&entrada.fecha, FORMATO_DE_LA_FECHA).ok()
}

#[cfg(test)]
mod pruebas {
    use super::super::{huella, VeredictoDeLaEntrega};
    use super::*;

    fn entrada(propuesta: &str, fecha: &str, estado: EstadoDeLaPropuesta) -> EntradaDeEntrega {
        EntradaDeEntrega {
            propuesta: propuesta.to_string(),
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: "claude.md".to_string(),
            modelo: None,
            fecha: fecha.to_string(),
            veredicto: VeredictoDeLaEntrega::Pasa,
            huella: huella("x"),
            estado,
            estado_lo_dio: LO_DIO_PROGRAMATOR.to_string(),
            destino: None,
            motivo: None,
            archivada: None,
            ya_en_el_proyecto: Vec::new(),
        }
    }

    fn hora(texto: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(texto, FORMATO_DE_LA_FECHA).unwrap()
    }

    #[test]
    fn lo_propuesto_pasado_el_plazo_queda_sin_confirmar() {
        let mut registro = RegistroDeEntregas {
            entregas: vec![
                entrada(
                    "a.rs",
                    "2026-10-01 10:00:00",
                    EstadoDeLaPropuesta::Propuesta,
                ),
                entrada(
                    "b.rs",
                    "2026-11-20 10:00:00",
                    EstadoDeLaPropuesta::Propuesta,
                ),
                entrada(
                    "c.rs",
                    "2026-10-01 10:00:00",
                    EstadoDeLaPropuesta::Rechazada,
                ),
                entrada("d.rs", "no es fecha", EstadoDeLaPropuesta::Propuesta),
            ],
            ..RegistroDeEntregas::default()
        };

        assert!(caducar(&mut registro, hora("2026-11-25 10:00:00"), 30));

        let estados: Vec<_> = registro.entregas.iter().map(|e| e.estado).collect();
        assert_eq!(
            estados,
            [
                EstadoDeLaPropuesta::SinConfirmar,
                EstadoDeLaPropuesta::Propuesta,
                EstadoDeLaPropuesta::Rechazada,
                EstadoDeLaPropuesta::Propuesta,
            ]
        );
    }

    /// E5: lo resuelto sale del primer nivel al archivo de su mes, con sus subcarpetas, y lo que
    /// espera revisión se queda.
    #[test]
    fn lo_resuelto_pasa_al_archivo_de_su_mes() {
        let carpeta = tempfile::tempdir().unwrap();
        let candidatos = carpeta.path();
        std::fs::create_dir_all(candidatos.join("sub")).unwrap();
        for nombre in ["a.rs", "b.rs", "sub/c.rs"] {
            std::fs::write(candidatos.join(nombre), "x").unwrap();
        }
        let mut registro = RegistroDeEntregas {
            entregas: vec![
                entrada("a.rs", "2026-10-03 10:00:00", EstadoDeLaPropuesta::Aceptada),
                entrada(
                    "b.rs",
                    "2026-11-03 10:00:00",
                    EstadoDeLaPropuesta::Propuesta,
                ),
                entrada(
                    "sub/c.rs",
                    "2026-11-03 10:00:00",
                    EstadoDeLaPropuesta::Sustituida,
                ),
                entrada(
                    "ya-no-esta.rs",
                    "2026-11-03 10:00:00",
                    EstadoDeLaPropuesta::Rechazada,
                ),
            ],
            ..RegistroDeEntregas::default()
        };

        assert!(archivar(candidatos, &mut registro).unwrap());

        assert!(!candidatos.join("a.rs").exists());
        assert!(candidatos.join("archivo/2026-10/a.rs").exists());
        assert!(candidatos.join("b.rs").exists());
        assert!(candidatos.join("archivo/2026-11/sub/c.rs").exists());
        let archivadas: Vec<Option<&str>> = registro
            .entregas
            .iter()
            .map(|e| e.archivada.as_deref())
            .collect();
        assert_eq!(
            archivadas,
            [
                Some("archivo/2026-10/a.rs"),
                None,
                Some("archivo/2026-11/sub/c.rs"),
                None
            ]
        );
        assert!(
            !archivar(candidatos, &mut registro).unwrap(),
            "ya no queda nada"
        );
    }

    #[test]
    fn en_el_archivo_no_se_pisa_nada() {
        let carpeta = tempfile::tempdir().unwrap();
        let candidatos = carpeta.path();
        std::fs::create_dir_all(candidatos.join("archivo/2026-11")).unwrap();
        std::fs::write(candidatos.join("archivo/2026-11/a.rs"), "antes").unwrap();
        std::fs::write(candidatos.join("archivo/2026-11/a-002.rs"), "otra").unwrap();
        std::fs::write(candidatos.join("a.rs"), "ahora").unwrap();
        let mut registro = RegistroDeEntregas {
            entregas: vec![entrada(
                "a.rs",
                "2026-11-03 10:00:00",
                EstadoDeLaPropuesta::Rechazada,
            )],
            ..RegistroDeEntregas::default()
        };

        archivar(candidatos, &mut registro).unwrap();

        assert_eq!(
            std::fs::read_to_string(candidatos.join("archivo/2026-11/a.rs")).unwrap(),
            "antes"
        );
        assert_eq!(
            std::fs::read_to_string(candidatos.join("archivo/2026-11/a-002.rs")).unwrap(),
            "otra"
        );
        assert_eq!(
            std::fs::read_to_string(candidatos.join("archivo/2026-11/a-003.rs")).unwrap(),
            "ahora"
        );
        assert_eq!(
            registro.entregas[0].archivada.as_deref(),
            Some("archivo/2026-11/a-003.rs")
        );
    }
}
