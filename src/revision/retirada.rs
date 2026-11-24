//! `--retirar-modelo <fichero>`: anotar la retirada en el registro y borrar el fichero.
//!
//! El registro se escribe **antes** de borrar: si el borrado falla, la retirada consta y el
//! fichero sigue, y se dice así. Al revés quedaría un modelo borrado sin constancia.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use super::mensajes;
use super::mes;
use super::redaccion::{leer_retiradas, RetiradasPropuestas, NOMBRE_DE_LAS_RETIRADAS};
use super::FORMATO_DEL_DIA;
use crate::error::{Error, Resultado};
use crate::ficheros;
use crate::registro::modelo::{self, EntradaDeModelo, Registro};

/// Lo que la orden necesita del mundo.
pub struct Entorno {
    pub carpeta_de_modelos: PathBuf,
    pub carpeta_de_revisiones: PathBuf,
    pub ruta_del_registro: PathBuf,
    /// El nombre, sin extensión, del modelo de `programator.toml`.
    pub modelo_en_uso: Option<String>,
    pub hoy: NaiveDate,
}

/// Una retirada hecha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retirada {
    pub ruta: PathBuf,
    /// Cuántas entradas del registro tenían esa huella y quedaron anotadas.
    pub anotadas: usize,
    pub motivo: String,
    /// Lo que pesaba el fichero, en bytes.
    pub liberados: u64,
    /// El fichero ya no estaba: solo se ha anotado la retirada en el registro.
    pub sin_fichero: bool,
}

pub fn retirar(fichero: &str, entorno: &Entorno) -> Resultado<Retirada> {
    let ruta = entorno.carpeta_de_modelos.join(fichero);
    let hay_fichero = ruta.is_file();
    let mut registro = modelo::leer(&entorno.ruta_del_registro)?;
    let nombre =
        crate::motor::fichero_de_modelo::nombre_de_modelo(Path::new(fichero)).unwrap_or_default();
    let hay_entrada = registro
        .modelo
        .iter()
        .any(|e| e.retirado_el.is_none() && es_el(&nombre, &e.nombre));
    if let Some(negativa) = negativa(
        fichero,
        hay_fichero || hay_entrada,
        entorno.modelo_en_uso.as_deref(),
    ) {
        return Err(Error::Revision(negativa));
    }
    let propuesta = propuesta_mas_reciente(&entorno.carpeta_de_revisiones)?;
    let motivo = motivo(propuesta.as_ref(), &nombre);
    let dia = entorno.hoy.format(FORMATO_DEL_DIA).to_string();
    if !hay_fichero {
        let sin_retirar =
            |e: &EntradaDeModelo| e.retirado_el.is_none() && es_el(&nombre, &e.nombre);
        let anotadas = anotar(&mut registro, sin_retirar, &dia, &motivo);
        modelo::escribir(&entorno.ruta_del_registro, &registro)?;
        return Ok(Retirada {
            ruta,
            anotadas,
            motivo,
            liberados: 0,
            sin_fichero: true,
        });
    }
    let liberados = std::fs::metadata(&ruta)
        .map_err(|causa| Error::Lectura {
            ruta: ruta.clone(),
            causa,
        })?
        .len();
    let huella = ficheros::huella_sha256(&ruta)?;
    let anotadas = anotar(
        &mut registro,
        |e| e.huella_sha256.eq_ignore_ascii_case(&huella),
        &dia,
        &motivo,
    );
    if anotadas > 0 {
        modelo::escribir(&entorno.ruta_del_registro, &registro)?;
    }
    std::fs::remove_file(&ruta)
        .map_err(|causa| Error::Revision(mensajes::retirada_sin_borrar(&ruta, anotadas, &causa)))?;
    Ok(Retirada {
        ruta,
        anotadas,
        motivo,
        liberados,
        sin_fichero: false,
    })
}

/// Si una entrada del registro es la del modelo de ese nombre. Sin distinguir mayúsculas, como
/// el sistema de ficheros de Windows.
fn es_el(nombre: &str, de_la_entrada: &str) -> bool {
    !nombre.is_empty() && nombre.eq_ignore_ascii_case(de_la_entrada)
}

/// Por qué no se puede retirar, o `None` si se puede. Solo un nombre de fichero, sin carpetas: así
/// no se puede salir de la carpeta de modelos.
fn negativa(fichero: &str, esta_en_modelos: bool, modelo_en_uso: Option<&str>) -> Option<String> {
    if fichero.is_empty() || fichero.contains(['/', '\\']) || fichero == ".." {
        return Some(mensajes::retirar_solo_un_nombre(fichero));
    }
    let minusculas = fichero.to_ascii_lowercase();
    let Some(nombre) = crate::motor::fichero_de_modelo::nombre_sin_extension(&minusculas) else {
        return Some(mensajes::retirar_solo_gguf(fichero));
    };
    if modelo_en_uso.is_some_and(|en_uso| en_uso.eq_ignore_ascii_case(nombre)) {
        return Some(mensajes::retirar_el_modelo_en_uso(fichero));
    }
    if !esta_en_modelos {
        return Some(mensajes::retirar_lo_que_no_esta(fichero));
    }
    None
}

/// El motivo de la propuesta más reciente, si lo trae; si no, que se retiró a mano.
fn motivo(propuesta: Option<&RetiradasPropuestas>, nombre: &str) -> String {
    propuesta
        .and_then(|p| p.retiradas.iter().find(|r| r.nombre == nombre))
        .map_or(mensajes::RETIRADO_POR_ORDEN.to_string(), |r| {
            r.motivo.clone()
        })
}

fn propuesta_mas_reciente(carpeta_de_revisiones: &Path) -> Resultado<Option<RetiradasPropuestas>> {
    mes::el_mas_reciente(carpeta_de_revisiones, NOMBRE_DE_LAS_RETIRADAS)?
        .map(|ruta| leer_retiradas(&ficheros::leer(&ruta)?))
        .transpose()
}

/// Anota la retirada en cada entrada que sea del modelo, en cualquier tarjeta: con fichero, las
/// de su huella, que son el mismo fichero; sin él, las sin retirar de su nombre. Las cifras se
/// quedan. Devuelve cuántas anotó.
fn anotar(
    registro: &mut Registro,
    es_del_modelo: impl Fn(&EntradaDeModelo) -> bool,
    dia: &str,
    motivo: &str,
) -> usize {
    let mut anotadas = 0;
    for entrada in registro.modelo.iter_mut().filter(|e| es_del_modelo(e)) {
        entrada.retirado_el = Some(dia.to_string());
        entrada.motivo_de_retirada = Some(motivo.to_string());
        anotadas += 1;
    }
    anotadas
}

#[cfg(test)]
mod pruebas {

    use super::*;
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;
    use crate::revision::redaccion::{escribir_retiradas, RetiradaPropuesta};

    const CONTENIDO: &[u8] = b"un modelo que sobra";

    fn huella(bytes: &[u8]) -> String {
        crate::ficheros::huella_sha256_de_bytes(bytes)
    }

    /// Una carpeta con `modelos/sobra.gguf` y un registro con dos entradas de ese fichero, en dos
    /// tarjetas, y una de otro.
    fn montar(raiz: &Path) -> Entorno {
        let modelos = raiz.join("modelos");
        ficheros::crear_carpeta(&modelos).unwrap();
        ficheros::escribir(&modelos.join("sobra.gguf"), "un modelo que sobra").unwrap();
        let mut aqui = entrada_de_ejemplo();
        aqui.nombre = "sobra".into();
        aqui.huella_sha256 = huella(CONTENIDO);
        let mut alli = aqui.clone();
        alli.tarjeta = "Otra tarjeta".into();
        let otro = entrada_de_ejemplo();
        let ruta_del_registro = raiz.join("registro-de-modelos.toml");
        modelo::escribir(
            &ruta_del_registro,
            &Registro {
                modelo: vec![aqui, alli, otro],
            },
        )
        .unwrap();
        Entorno {
            carpeta_de_modelos: modelos,
            carpeta_de_revisiones: raiz.join("revisiones"),
            ruta_del_registro,
            modelo_en_uso: Some("en-uso".into()),
            hoy: NaiveDate::from_ymd_opt(2026, 11, 13).unwrap(),
        }
    }

    fn escribir_propuesta(raiz: &Path, nombre: &str, motivo: &str) {
        let carpeta = raiz.join("revisiones").join("2026-11");
        ficheros::crear_carpeta(&carpeta).unwrap();
        let retiradas = RetiradasPropuestas {
            retiradas: vec![RetiradaPropuesta {
                nombre: nombre.into(),
                motivo: motivo.into(),
            }],
        };
        ficheros::escribir(
            &carpeta.join(NOMBRE_DE_LAS_RETIRADAS),
            &escribir_retiradas(&retiradas).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn anota_cada_entrada_con_esa_huella_conserva_las_cifras_y_borra_el_fichero() {
        let raiz = tempfile::tempdir().unwrap();
        let entorno = montar(raiz.path());
        let antes = modelo::leer(&entorno.ruta_del_registro).unwrap();
        let retirada = retirar("sobra.gguf", &entorno).unwrap();
        assert_eq!(retirada.anotadas, 2);
        assert_eq!(retirada.liberados, CONTENIDO.len() as u64);
        assert_eq!(retirada.motivo, mensajes::RETIRADO_POR_ORDEN);
        assert!(!entorno.carpeta_de_modelos.join("sobra.gguf").exists());
        let despues = modelo::leer(&entorno.ruta_del_registro).unwrap();
        for (a, d) in antes.modelo.iter().zip(&despues.modelo) {
            if d.nombre == "sobra" {
                assert_eq!(d.retirado_el.as_deref(), Some("2026-11-13"));
                assert_eq!(
                    d.motivo_de_retirada.as_deref(),
                    Some(mensajes::RETIRADO_POR_ORDEN)
                );
                assert_eq!(d.clase, a.clase);
                assert_eq!(d.familias, a.familias);
            } else {
                assert_eq!(d, a);
            }
        }
    }

    #[test]
    fn el_motivo_es_el_de_la_propuesta_mas_reciente_si_lo_trae() {
        let raiz = tempfile::tempdir().unwrap();
        let entorno = montar(raiz.path());
        escribir_propuesta(raiz.path(), "sobra", "no gana nada");
        let retirada = retirar("sobra.gguf", &entorno).unwrap();
        assert_eq!(retirada.motivo, "no gana nada");
        let registro = modelo::leer(&entorno.ruta_del_registro).unwrap();
        assert_eq!(
            registro.modelo[0].motivo_de_retirada.as_deref(),
            Some("no gana nada")
        );
    }

    #[test]
    fn si_la_propuesta_no_lo_nombra_es_por_orden_del_director() {
        let raiz = tempfile::tempdir().unwrap();
        let entorno = montar(raiz.path());
        escribir_propuesta(raiz.path(), "otro", "no gana nada");
        let retirada = retirar("sobra.gguf", &entorno).unwrap();
        assert_eq!(retirada.motivo, mensajes::RETIRADO_POR_ORDEN);
    }

    #[test]
    fn cada_negativa_dice_lo_suyo_y_no_toca_nada() {
        let casos = [
            ("../fuera.gguf", false, "nombre"),
            ("carpeta\\m.gguf", false, "nombre"),
            ("notas.txt", true, ".gguf"),
            ("EN-USO.gguf", true, "programator.toml"),
            ("no-esta.gguf", false, "no está"),
        ];
        for (fichero, esta, clave) in casos {
            let texto = negativa(fichero, esta, Some("en-uso")).unwrap_or_default();
            assert!(texto.contains(clave), "{fichero}: {texto}");
        }
        assert_eq!(negativa("sobra.gguf", true, Some("en-uso")), None);
    }

    #[test]
    fn una_negativa_no_toca_el_registro_ni_el_fichero() {
        let raiz = tempfile::tempdir().unwrap();
        let mut entorno = montar(raiz.path());
        entorno.modelo_en_uso = Some("sobra".into());
        let antes = ficheros::leer(&entorno.ruta_del_registro).unwrap();
        assert!(retirar("sobra.gguf", &entorno).is_err());
        assert!(entorno.carpeta_de_modelos.join("sobra.gguf").exists());
        assert_eq!(ficheros::leer(&entorno.ruta_del_registro).unwrap(), antes);
    }

    /// En Windows, un fichero abierto sin compartir el borrado no se deja borrar: así se ve que el
    /// registro ya consta cuando el borrado falla.
    #[cfg(windows)]
    #[test]
    fn si_el_borrado_falla_la_retirada_consta_y_el_fichero_sigue() {
        use std::os::windows::fs::OpenOptionsExt;
        let raiz = tempfile::tempdir().unwrap();
        let entorno = montar(raiz.path());
        let ruta = entorno.carpeta_de_modelos.join("sobra.gguf");
        let bloqueo = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1) // FILE_SHARE_READ: se puede leer la huella, no borrar.
            .open(&ruta)
            .unwrap();
        let fallo = retirar("sobra.gguf", &entorno).unwrap_err().to_string();
        drop(bloqueo);
        assert!(
            fallo.contains("consta") && fallo.contains("sobra.gguf"),
            "{fallo}"
        );
        assert!(ruta.exists());
        let registro = modelo::leer(&entorno.ruta_del_registro).unwrap();
        assert_eq!(
            registro.modelo[0].retirado_el.as_deref(),
            Some("2026-11-13")
        );
    }

    #[test]
    fn un_fichero_sin_entradas_se_borra_sin_tocar_el_registro() {
        let raiz = tempfile::tempdir().unwrap();
        let entorno = montar(raiz.path());
        ficheros::escribir(&entorno.carpeta_de_modelos.join("suelto.gguf"), "otro").unwrap();
        let antes = ficheros::leer(&entorno.ruta_del_registro).unwrap();
        let retirada = retirar("suelto.gguf", &entorno).unwrap();
        assert_eq!(retirada.anotadas, 0);
        assert_eq!(ficheros::leer(&entorno.ruta_del_registro).unwrap(), antes);
    }

    #[test]
    fn un_modelo_sin_fichero_con_entrada_se_retira_del_registro_sin_borrar_nada() {
        let raiz = tempfile::tempdir().unwrap();
        let entorno = montar(raiz.path());
        let otro = entorno.carpeta_de_modelos.join("otro.gguf");
        ficheros::escribir(&otro, "otro").unwrap();
        std::fs::remove_file(entorno.carpeta_de_modelos.join("sobra.gguf")).unwrap();
        let antes = modelo::leer(&entorno.ruta_del_registro).unwrap();

        let retirada = retirar("Sobra.gguf", &entorno).unwrap();

        assert_eq!(retirada.anotadas, 2);
        assert_eq!(retirada.liberados, 0);
        assert!(retirada.sin_fichero);
        assert!(otro.exists());
        let despues = modelo::leer(&entorno.ruta_del_registro).unwrap();
        for (a, d) in antes.modelo.iter().zip(&despues.modelo) {
            if d.nombre == "sobra" {
                assert_eq!(d.retirado_el.as_deref(), Some("2026-11-13"));
                assert_eq!(d.clase, a.clase);
            } else {
                assert_eq!(d, a);
            }
        }
        // Ya retirado: la segunda vez no queda nada que retirar.
        let otra_vez = retirar("sobra.gguf", &entorno).unwrap_err().to_string();
        assert!(otra_vez.contains("no está"), "{otra_vez}");
    }

    #[test]
    fn sin_fichero_ni_entrada_se_niega_como_siempre() {
        let raiz = tempfile::tempdir().unwrap();
        let entorno = montar(raiz.path());
        let antes = ficheros::leer(&entorno.ruta_del_registro).unwrap();
        let negativa = retirar("nunca.gguf", &entorno).unwrap_err().to_string();
        assert!(negativa.contains("no está"), "{negativa}");
        assert_eq!(ficheros::leer(&entorno.ruta_del_registro).unwrap(), antes);
    }

    #[test]
    fn el_modelo_en_uso_sin_fichero_tampoco_se_retira() {
        let raiz = tempfile::tempdir().unwrap();
        let mut entorno = montar(raiz.path());
        entorno.modelo_en_uso = Some("sobra".into());
        std::fs::remove_file(entorno.carpeta_de_modelos.join("sobra.gguf")).unwrap();
        let antes = ficheros::leer(&entorno.ruta_del_registro).unwrap();
        let negativa = retirar("sobra.gguf", &entorno).unwrap_err().to_string();
        assert!(negativa.contains("programator.toml"), "{negativa}");
        assert_eq!(ficheros::leer(&entorno.ruta_del_registro).unwrap(), antes);
    }

    /// Cambia el registro de `montar` con `cambio`.
    fn cambiar_el_registro(entorno: &Entorno, cambio: impl FnOnce(&mut Registro)) {
        let mut registro = modelo::leer(&entorno.ruta_del_registro).unwrap();
        cambio(&mut registro);
        modelo::escribir(&entorno.ruta_del_registro, &registro).unwrap();
    }

    #[test]
    fn sin_fichero_la_entrada_ya_retirada_conserva_su_fecha_y_su_motivo() {
        let raiz = tempfile::tempdir().unwrap();
        let entorno = montar(raiz.path());
        cambiar_el_registro(&entorno, |registro| {
            registro.modelo[1].retirado_el = Some("2026-10-01".into());
            registro.modelo[1].motivo_de_retirada = Some("de antes".into());
        });
        std::fs::remove_file(entorno.carpeta_de_modelos.join("sobra.gguf")).unwrap();

        let retirada = retirar("sobra.gguf", &entorno).unwrap();

        assert_eq!(retirada.anotadas, 1);
        let despues = modelo::leer(&entorno.ruta_del_registro).unwrap();
        assert_eq!(despues.modelo[0].retirado_el.as_deref(), Some("2026-11-13"));
        assert_eq!(despues.modelo[1].retirado_el.as_deref(), Some("2026-10-01"));
        assert_eq!(
            despues.modelo[1].motivo_de_retirada.as_deref(),
            Some("de antes")
        );
    }

    #[test]
    fn la_huella_del_registro_en_mayusculas_tambien_se_anota() {
        let raiz = tempfile::tempdir().unwrap();
        let entorno = montar(raiz.path());
        cambiar_el_registro(&entorno, |registro| {
            registro.modelo[0].huella_sha256 = huella(CONTENIDO).to_uppercase();
        });

        let retirada = retirar("sobra.gguf", &entorno).unwrap();

        assert_eq!(retirada.anotadas, 2);
    }
}
