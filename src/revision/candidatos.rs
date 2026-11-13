//! La clasificación de los ficheros de los repositorios encontrados: candidatos, descartados con
//! su motivo, y ya descargados sin medir (especificación, §3.2, puntos 4 y 5).
//!
//! Puro: recibe los detalles ya pedidos, la tarjeta ya medida y lo que ya hay en el registro y en
//! `modelos/`, y no toca red, disco ni reloj.

use std::collections::BTreeSet;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::catalogo::{DetalleDeRepositorio, FicheroDelRepositorio};
use super::config::ConfigRevision;
use super::{BYTES_POR_MIB, LARGO_DE_LA_FECHA};

/// La extensión de los ficheros que carga `llama-server`.
const EXTENSION_GGUF: &str = ".gguf";
/// Los proyectores de visión (`mmproj-…`) también son GGUF y llevan cuantización en el nombre,
/// pero no son un modelo que se pueda cargar solo.
const PREFIJO_DE_PROYECTOR: &str = "mmproj";
/// Un modelo partido se reparte en ficheros `-00001-of-00003.gguf`: el número de la parte y el
/// total, de cinco cifras cada uno, como los numera `llama-gguf-split`.
const SEPARADOR_DE_PARTES: &str = "-of-";
const CIFRAS_DEL_NUMERO_DE_PARTE: usize = 5;
/// Para el plazo basta el día de la fecha de la API.
const FORMATO_DE_LA_FECHA: &str = "%Y-%m-%d";

/// Con qué se clasifica.
pub struct Criterios<'a> {
    pub config: &'a ConfigRevision,
    /// La VRAM medida de la tarjeta, en MiB.
    pub vram_mib: u64,
    /// El margen de `[motor]`, en MiB.
    pub margen_vram_mib: u64,
    /// Las huellas SHA-256 de los modelos del registro.
    pub huellas_registradas: &'a BTreeSet<String>,
    /// Los nombres de los ficheros que hay en `modelos/` y no están en el registro.
    pub ficheros_en_modelos: &'a BTreeSet<String>,
    pub hoy: NaiveDate,
}

impl Criterios<'_> {
    /// Lo que puede pesar un fichero para caber: la VRAM menos el margen de `[motor]` y la
    /// reserva de `[revision]`, en MiB.
    pub fn cabe_mib(&self) -> u64 {
        self.vram_mib
            .saturating_sub(self.margen_vram_mib)
            .saturating_sub(self.config.reserva_mib)
    }
}

/// Un fichero que se puede descargar, con lo que hace falta para decidirlo y para descargarlo.
/// Es también cada entrada de `candidatos.toml` (`lista`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FicheroCandidato {
    pub repositorio: String,
    pub fichero: String,
    pub cuantizacion: String,
    /// En bytes. Siempre está en los candidatos; puede faltar en un ya descargado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tamano: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub huella_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licencia: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultima_modificacion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descargas: Option<u64>,
}

/// Por qué un fichero, o un repositorio entero, se queda fuera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotivoDeDescarte {
    /// El fichero más la reserva no cabe en la VRAM menos el margen. En bytes.
    NoCabe { tamano: u64, cabe: u64 },
    /// La API no dio el tamaño, y sin él no se puede decir si cabe.
    SinTamano,
    /// El repositorio no se ha modificado en el plazo, o la API no dio la fecha.
    FueraDePlazo { ultima_modificacion: Option<String> },
    /// Su huella ya está en el registro, se llame como se llame.
    YaRegistrado,
    /// Está repartido en varios ficheros, y el motor y el lector GGUF trabajan con uno.
    Partido { partes: usize },
    /// El repositorio no tiene ningún fichero de las cuantizaciones que interesan.
    SinCuantizacion,
}

/// Un descarte, con lo que se sabe del fichero para que se vea qué se dejó fuera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descartado {
    pub repositorio: String,
    /// `None` cuando el motivo es del repositorio entero.
    pub fichero: Option<String>,
    pub tamano: Option<u64>,
    pub motivo: MotivoDeDescarte,
}

/// Los tres grupos de la §3.2, punto 5.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Clasificacion {
    pub candidatos: Vec<FicheroCandidato>,
    pub descartados: Vec<Descartado>,
    pub ya_descargados: Vec<FicheroCandidato>,
}

/// Lo que da un repositorio para una cuantización.
enum Eleccion<'a> {
    Entero(&'a FicheroDelRepositorio),
    Partido {
        primera: &'a FicheroDelRepositorio,
        partes: usize,
        tamano: Option<u64>,
    },
}

/// Clasifica cada fichero de cada repositorio, uno por cuantización y en el orden de la lista.
pub fn clasificar(detalles: &[DetalleDeRepositorio], criterios: &Criterios) -> Clasificacion {
    let mut clasificacion = Clasificacion::default();
    for detalle in detalles {
        clasificar_repositorio(detalle, criterios, &mut clasificacion);
    }
    clasificacion
}

fn clasificar_repositorio(
    detalle: &DetalleDeRepositorio,
    criterios: &Criterios,
    clasificacion: &mut Clasificacion,
) {
    let elecciones: Vec<(&str, Eleccion)> = criterios
        .config
        .cuantizaciones
        .iter()
        .filter_map(|cuantizacion| {
            elegir(&detalle.ficheros, cuantizacion).map(|e| (cuantizacion.as_str(), e))
        })
        .collect();
    if elecciones.is_empty() {
        clasificacion.descartados.push(Descartado {
            repositorio: detalle.id.clone(),
            fichero: None,
            tamano: None,
            motivo: MotivoDeDescarte::SinCuantizacion,
        });
        return;
    }
    let en_plazo = esta_en_plazo(
        detalle.ultima_modificacion.as_deref(),
        criterios.config.modificados_en_los_ultimos_dias,
        criterios.hoy,
    );
    for (cuantizacion, eleccion) in elecciones {
        let fichero = match eleccion {
            Eleccion::Entero(fichero) => fichero,
            Eleccion::Partido {
                primera,
                partes,
                tamano,
            } => {
                let motivo = if en_plazo {
                    MotivoDeDescarte::Partido { partes }
                } else {
                    fuera_de_plazo(detalle)
                };
                clasificacion
                    .descartados
                    .push(descarte(detalle, primera, tamano, motivo));
                continue;
            }
        };
        if esta_registrado(fichero, criterios.huellas_registradas) {
            let motivo = MotivoDeDescarte::YaRegistrado;
            clasificacion
                .descartados
                .push(descarte(detalle, fichero, fichero.tamano, motivo));
        } else if esta_en_modelos(fichero, criterios.ficheros_en_modelos) {
            clasificacion.ya_descargados.push(candidato(
                detalle,
                fichero,
                cuantizacion,
                fichero.tamano,
            ));
        } else if !en_plazo {
            let motivo = fuera_de_plazo(detalle);
            clasificacion
                .descartados
                .push(descarte(detalle, fichero, fichero.tamano, motivo));
        } else {
            match decidir_si_cabe(fichero.tamano, criterios) {
                Ok(tamano) => clasificacion.candidatos.push(candidato(
                    detalle,
                    fichero,
                    cuantizacion,
                    Some(tamano),
                )),
                Err(motivo) => clasificacion.descartados.push(descarte(
                    detalle,
                    fichero,
                    fichero.tamano,
                    motivo,
                )),
            }
        }
    }
}

/// El fichero entero de una cuantización y, si no lo hay, sus partes.
fn elegir<'a>(ficheros: &'a [FicheroDelRepositorio], cuantizacion: &str) -> Option<Eleccion<'a>> {
    let de_la_cuantizacion: Vec<&FicheroDelRepositorio> = ficheros
        .iter()
        .filter(|f| es_modelo(&f.nombre) && lleva_la_cuantizacion(&f.nombre, cuantizacion))
        .collect();
    if let Some(entero) = de_la_cuantizacion.iter().find(|f| !es_parte(&f.nombre)) {
        return Some(Eleccion::Entero(entero));
    }
    let partes: Vec<&FicheroDelRepositorio> = de_la_cuantizacion
        .into_iter()
        .filter(|f| es_parte(&f.nombre))
        .collect();
    let primera = partes.first()?;
    Some(Eleccion::Partido {
        primera,
        partes: partes.len(),
        tamano: partes.iter().map(|f| f.tamano).sum(),
    })
}

/// Un GGUF que no es un proyector de visión.
fn es_modelo(nombre: &str) -> bool {
    let minusculas = nombre.to_ascii_lowercase();
    minusculas.ends_with(EXTENSION_GGUF) && !minusculas.starts_with(PREFIJO_DE_PROYECTOR)
}

/// El nombre lleva la cuantización como palabra suelta, sin distinguir mayúsculas: `Q6_K` no está
/// en `Q6_K_L`, ni `Q4_K_M` en `IQ4_K_M`.
fn lleva_la_cuantizacion(nombre: &str, cuantizacion: &str) -> bool {
    let nombre = nombre.to_ascii_lowercase();
    let buscada = cuantizacion.to_ascii_lowercase();
    nombre.match_indices(&buscada).any(|(inicio, _)| {
        let antes = nombre[..inicio].chars().next_back();
        let despues = nombre[inicio + buscada.len()..].chars().next();
        !antes.is_some_and(|c| c.is_ascii_alphanumeric())
            && matches!(despues, Some('.') | Some('-'))
    })
}

/// `…-00001-of-00003.gguf`.
fn es_parte(nombre: &str) -> bool {
    let minusculas = nombre.to_ascii_lowercase();
    let Some(base) = minusculas.strip_suffix(EXTENSION_GGUF) else {
        return false;
    };
    let Some((izquierda, total)) = base.rsplit_once(SEPARADOR_DE_PARTES) else {
        return false;
    };
    let Some((_, parte)) = izquierda.rsplit_once('-') else {
        return false;
    };
    [parte, total]
        .iter()
        .all(|n| n.len() == CIFRAS_DEL_NUMERO_DE_PARTE && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Modificado en los últimos `dias` días, contando el día del borde. 0 quita el filtro; sin
/// fecha no se puede decir que esté en plazo.
fn esta_en_plazo(ultima_modificacion: Option<&str>, dias: u32, hoy: NaiveDate) -> bool {
    if dias == 0 {
        return true;
    }
    let Some(fecha) = ultima_modificacion
        .and_then(|texto| texto.get(..LARGO_DE_LA_FECHA))
        .and_then(|dia| NaiveDate::parse_from_str(dia, FORMATO_DE_LA_FECHA).ok())
    else {
        return false;
    };
    (hoy - fecha).num_days() <= i64::from(dias)
}

fn esta_registrado(fichero: &FicheroDelRepositorio, huellas: &BTreeSet<String>) -> bool {
    fichero
        .huella_sha256
        .as_deref()
        .is_some_and(|huella| huellas.iter().any(|h| h.eq_ignore_ascii_case(huella)))
}

/// Por el nombre, sin distinguir mayúsculas, como los compara el sistema de ficheros de Windows.
fn esta_en_modelos(fichero: &FicheroDelRepositorio, en_modelos: &BTreeSet<String>) -> bool {
    en_modelos
        .iter()
        .any(|nombre| nombre.eq_ignore_ascii_case(&fichero.nombre))
}

/// El tamaño, si el fichero más la reserva cabe en la VRAM menos el margen; si no, el motivo.
fn decidir_si_cabe(tamano: Option<u64>, criterios: &Criterios) -> Result<u64, MotivoDeDescarte> {
    let tamano = tamano.ok_or(MotivoDeDescarte::SinTamano)?;
    let cabe = criterios.cabe_mib().saturating_mul(BYTES_POR_MIB);
    if tamano <= cabe {
        Ok(tamano)
    } else {
        Err(MotivoDeDescarte::NoCabe { tamano, cabe })
    }
}

fn fuera_de_plazo(detalle: &DetalleDeRepositorio) -> MotivoDeDescarte {
    MotivoDeDescarte::FueraDePlazo {
        ultima_modificacion: detalle.ultima_modificacion.clone(),
    }
}

fn descarte(
    detalle: &DetalleDeRepositorio,
    fichero: &FicheroDelRepositorio,
    tamano: Option<u64>,
    motivo: MotivoDeDescarte,
) -> Descartado {
    Descartado {
        repositorio: detalle.id.clone(),
        fichero: Some(fichero.nombre.clone()),
        tamano,
        motivo,
    }
}

fn candidato(
    detalle: &DetalleDeRepositorio,
    fichero: &FicheroDelRepositorio,
    cuantizacion: &str,
    tamano: Option<u64>,
) -> FicheroCandidato {
    FicheroCandidato {
        repositorio: detalle.id.clone(),
        fichero: fichero.nombre.clone(),
        cuantizacion: cuantizacion.to_string(),
        tamano,
        huella_sha256: fichero.huella_sha256.clone(),
        licencia: detalle.licencia.clone(),
        ultima_modificacion: detalle.ultima_modificacion.clone(),
        descargas: detalle.descargas,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::revision::catalogo::leer_detalle;

    const GIB: u64 = 1024 * BYTES_POR_MIB;

    fn hoy() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 12).unwrap()
    }

    fn fichero(nombre: &str, tamano: u64, huella: &str) -> FicheroDelRepositorio {
        FicheroDelRepositorio {
            nombre: nombre.into(),
            tamano: Some(tamano),
            huella_sha256: Some(huella.into()),
        }
    }

    fn repositorio(
        id: &str,
        fecha: &str,
        ficheros: Vec<FicheroDelRepositorio>,
    ) -> DetalleDeRepositorio {
        DetalleDeRepositorio {
            id: id.into(),
            ultima_modificacion: Some(format!("{fecha}T10:00:00.000Z")),
            descargas: Some(100),
            licencia: Some("mit".into()),
            ficheros,
        }
    }

    /// Una tarjeta de 16 GiB con 2 GiB de margen y 2 GiB de reserva: cabe hasta 12 GiB.
    fn clasificar_con(
        detalles: &[DetalleDeRepositorio],
        config: &ConfigRevision,
        registradas: &[&str],
        en_modelos: &[&str],
    ) -> Clasificacion {
        let huellas = registradas.iter().map(|s| s.to_string()).collect();
        let modelos = en_modelos.iter().map(|s| s.to_string()).collect();
        clasificar(
            detalles,
            &Criterios {
                config,
                vram_mib: 16 * 1024,
                margen_vram_mib: 2048,
                huellas_registradas: &huellas,
                ficheros_en_modelos: &modelos,
                hoy: hoy(),
            },
        )
    }

    fn motivos(clasificacion: &Clasificacion) -> Vec<&MotivoDeDescarte> {
        clasificacion
            .descartados
            .iter()
            .map(|d| &d.motivo)
            .collect()
    }

    #[test]
    fn un_fichero_por_cuantizacion_en_el_orden_de_la_lista() {
        let detalle = repositorio(
            "a/b",
            "2026-11-01",
            vec![
                fichero("m-Q8_0.gguf", 8 * GIB, "c"),
                fichero("m-Q4_K_M.gguf", 4 * GIB, "a"),
                fichero("m-Q6_K_L.gguf", 7 * GIB, "d"),
                fichero("m-IQ4_XS.gguf", 3 * GIB, "e"),
            ],
        );
        let resultado = clasificar_con(&[detalle], &ConfigRevision::default(), &[], &[]);
        let nombres: Vec<_> = resultado
            .candidatos
            .iter()
            .map(|c| c.fichero.as_str())
            .collect();
        assert_eq!(nombres, ["m-Q4_K_M.gguf", "m-Q8_0.gguf"]);
        assert_eq!(resultado.candidatos[0].cuantizacion, "Q4_K_M");
        assert_eq!(resultado.candidatos[0].licencia.as_deref(), Some("mit"));
        assert!(resultado.descartados.is_empty());
    }

    #[test]
    fn el_borde_exacto_de_cabe() {
        let justo = repositorio(
            "a/justo",
            "2026-11-01",
            vec![fichero("j-Q4_K_M.gguf", 12 * GIB, "a")],
        );
        let uno_mas = repositorio(
            "a/uno-mas",
            "2026-11-01",
            vec![fichero("u-Q4_K_M.gguf", 12 * GIB + 1, "b")],
        );
        let resultado = clasificar_con(&[justo, uno_mas], &ConfigRevision::default(), &[], &[]);
        assert_eq!(resultado.candidatos.len(), 1);
        assert_eq!(resultado.candidatos[0].repositorio, "a/justo");
        assert_eq!(
            motivos(&resultado),
            [&MotivoDeDescarte::NoCabe {
                tamano: 12 * GIB + 1,
                cabe: 12 * GIB
            }]
        );
    }

    #[test]
    fn sin_tamano_no_se_puede_decir_si_cabe() {
        let mut sin = fichero("s-Q4_K_M.gguf", 0, "a");
        sin.tamano = None;
        let resultado = clasificar_con(
            &[repositorio("a/b", "2026-11-01", vec![sin])],
            &ConfigRevision::default(),
            &[],
            &[],
        );
        assert_eq!(motivos(&resultado), [&MotivoDeDescarte::SinTamano]);
    }

    #[test]
    fn fuera_de_plazo_se_descarta_y_el_dia_del_borde_cuenta() {
        // 62 días antes del 12/11/2026 es el 11/09/2026.
        let en_el_borde = repositorio(
            "a/borde",
            "2026-09-11",
            vec![fichero("b-Q4_K_M.gguf", GIB, "a")],
        );
        let viejo = repositorio(
            "a/viejo",
            "2026-09-10",
            vec![fichero("v-Q4_K_M.gguf", GIB, "b")],
        );
        let resultado = clasificar_con(&[en_el_borde, viejo], &ConfigRevision::default(), &[], &[]);
        assert_eq!(resultado.candidatos.len(), 1);
        assert_eq!(resultado.candidatos[0].repositorio, "a/borde");
        assert_eq!(
            motivos(&resultado),
            [&MotivoDeDescarte::FueraDePlazo {
                ultima_modificacion: Some("2026-09-10T10:00:00.000Z".into())
            }]
        );
    }

    #[test]
    fn sin_fecha_no_esta_en_plazo() {
        let mut sin_fecha = repositorio(
            "a/b",
            "2026-11-01",
            vec![fichero("m-Q4_K_M.gguf", GIB, "a")],
        );
        sin_fecha.ultima_modificacion = None;
        let resultado = clasificar_con(&[sin_fecha], &ConfigRevision::default(), &[], &[]);
        assert_eq!(
            motivos(&resultado),
            [&MotivoDeDescarte::FueraDePlazo {
                ultima_modificacion: None
            }]
        );
    }

    #[test]
    fn cero_dias_quita_el_filtro_del_plazo() {
        let config = ConfigRevision {
            modificados_en_los_ultimos_dias: 0,
            ..ConfigRevision::default()
        };
        let viejo = repositorio(
            "a/viejo",
            "2020-01-01",
            vec![fichero("v-Q4_K_M.gguf", GIB, "a")],
        );
        let resultado = clasificar_con(&[viejo], &config, &[], &[]);
        assert_eq!(resultado.candidatos.len(), 1);
    }

    #[test]
    fn uno_registrado_por_huella_se_descarta_aunque_tenga_otro_nombre() {
        let detalle = repositorio(
            "a/b",
            "2026-11-01",
            vec![fichero("otro-nombre-Q4_K_M.gguf", GIB, "ABC123")],
        );
        let resultado = clasificar_con(&[detalle], &ConfigRevision::default(), &["abc123"], &[]);
        assert!(resultado.candidatos.is_empty());
        assert_eq!(motivos(&resultado), [&MotivoDeDescarte::YaRegistrado]);
    }

    #[test]
    fn uno_ya_en_modelos_sin_medir_sale_como_ya_descargado() {
        let detalle = repositorio(
            "a/b",
            "2026-11-01",
            vec![fichero("m-Q4_K_M.gguf", GIB, "a")],
        );
        let resultado = clasificar_con(
            &[detalle],
            &ConfigRevision::default(),
            &[],
            &["M-q4_k_m.gguf"],
        );
        assert!(resultado.candidatos.is_empty());
        assert!(resultado.descartados.is_empty());
        assert_eq!(resultado.ya_descargados[0].fichero, "m-Q4_K_M.gguf");
    }

    #[test]
    fn un_modelo_solo_partido_se_descarta_y_dice_cuantas_partes() {
        let detalle = repositorio(
            "a/b",
            "2026-11-01",
            vec![
                fichero("m-Q4_K_M-00001-of-00002.gguf", GIB, "a"),
                fichero("m-Q4_K_M-00002-of-00002.gguf", GIB, "b"),
            ],
        );
        let resultado = clasificar_con(&[detalle], &ConfigRevision::default(), &[], &[]);
        assert_eq!(
            motivos(&resultado),
            [&MotivoDeDescarte::Partido { partes: 2 }]
        );
        let descartado = &resultado.descartados[0];
        assert_eq!(
            descartado.fichero.as_deref(),
            Some("m-Q4_K_M-00001-of-00002.gguf")
        );
        assert_eq!(descartado.tamano, Some(2 * GIB));
    }

    #[test]
    fn si_hay_entero_y_partido_se_toma_el_entero() {
        let detalle = leer_detalle(include_str!("pruebas/detalle-partido.json")).unwrap();
        let config = ConfigRevision {
            modificados_en_los_ultimos_dias: 0,
            ..ConfigRevision::default()
        };
        let resultado = clasificar_con(&[detalle], &config, &[], &[]);
        let nombres: Vec<_> = resultado
            .candidatos
            .iter()
            .map(|c| c.fichero.as_str())
            .collect();
        assert_eq!(
            nombres,
            [
                "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
                "qwen2.5-coder-7b-instruct-q8_0.gguf"
            ]
        );
        assert!(resultado.descartados.is_empty());
    }

    #[test]
    fn un_repositorio_sin_ninguna_cuantizacion_se_descarta_entero() {
        let detalle = repositorio(
            "a/b",
            "2026-11-01",
            vec![
                fichero("m-F16.gguf", GIB, "a"),
                fichero("mmproj-m-Q8_0.gguf", GIB, "b"),
                fichero("README-Q4_K_M.md", 10, "c"),
            ],
        );
        let resultado = clasificar_con(&[detalle], &ConfigRevision::default(), &[], &[]);
        assert_eq!(
            resultado.descartados,
            [Descartado {
                repositorio: "a/b".into(),
                fichero: None,
                tamano: None,
                motivo: MotivoDeDescarte::SinCuantizacion,
            }]
        );
    }

    #[test]
    fn la_cuantizacion_va_como_palabra_suelta() {
        assert!(lleva_la_cuantizacion("m-Q6_K.gguf", "Q6_K"));
        assert!(lleva_la_cuantizacion(
            "m.q4_k_m-00001-of-00002.gguf",
            "Q4_K_M"
        ));
        assert!(!lleva_la_cuantizacion("m-Q6_K_L.gguf", "Q6_K"));
        assert!(!lleva_la_cuantizacion("m-IQ4_K_M.gguf", "Q4_K_M"));
    }

    #[test]
    fn las_partes_se_reconocen_por_sus_cinco_cifras() {
        assert!(es_parte("m-Q8_0-00001-of-00003.gguf"));
        assert!(!es_parte("m-Q8_0.gguf"));
        assert!(!es_parte("m-Q8_0-1-of-3.gguf"));
        assert!(!es_parte("m-Q8_0-00001-of-00003.bin"));
    }
}
