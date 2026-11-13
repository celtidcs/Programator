//! La carpeta de cada revisión, `revisiones/AAAA-MM`, y cómo se aparta lo que ya hubiera en ella
//! antes de escribir encima. Nada se borra (especificación, §3.2).

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

use crate::error::Resultado;
use crate::ficheros;

/// La carpeta de cada revisión, dentro de la de revisiones: `2026-11`.
const FORMATO_DE_LA_CARPETA_DEL_MES: &str = "%Y-%m";
/// El sufijo con que se aparta lo anterior del mismo mes: `candidatos-153012.toml`.
const FORMATO_DE_LA_HORA: &str = "%H%M%S";

pub fn carpeta_del_mes(carpeta_de_revisiones: &Path, ahora: NaiveDateTime) -> PathBuf {
    carpeta_de_revisiones.join(ahora.format(FORMATO_DE_LA_CARPETA_DEL_MES).to_string())
}

/// Si en la carpeta ya está `principal`, él y cada uno de `acompanantes` que exista pasan a
/// llevar la hora en el nombre. Devuelve el nombre nuevo de `principal`, si lo había.
pub fn apartar_lo_anterior(
    carpeta: &Path,
    principal: &str,
    acompanantes: &[&str],
    ahora: NaiveDateTime,
) -> Resultado<Option<String>> {
    if !carpeta.join(principal).exists() {
        return Ok(None);
    }
    let hora = ahora.format(FORMATO_DE_LA_HORA).to_string();
    for nombre in acompanantes {
        let actual = carpeta.join(nombre);
        if actual.exists() {
            ficheros::renombrar(&actual, &carpeta.join(con_la_hora(nombre, &hora)))?;
        }
    }
    let apartado = con_la_hora(principal, &hora);
    ficheros::renombrar(&carpeta.join(principal), &carpeta.join(&apartado))?;
    Ok(Some(apartado))
}

/// La carpeta de mes más reciente que tenga `nombre`. Las carpetas se llaman `AAAA-MM`, así que el
/// orden de los nombres es el de las fechas. Sin carpeta de revisiones, ninguna.
pub fn el_mes_mas_reciente_con(
    carpeta_de_revisiones: &Path,
    nombre: &str,
) -> Resultado<Option<PathBuf>> {
    if !carpeta_de_revisiones.is_dir() {
        return Ok(None);
    }
    let mut meses: Vec<PathBuf> = ficheros::entradas(carpeta_de_revisiones)?
        .into_iter()
        .filter(|ruta| ruta.join(nombre).is_file())
        .collect();
    meses.sort();
    Ok(meses.pop())
}

/// El fichero `nombre` de [`el_mes_mas_reciente_con`]: la ruta del fichero, no la de su carpeta.
pub fn el_mas_reciente(carpeta_de_revisiones: &Path, nombre: &str) -> Resultado<Option<PathBuf>> {
    Ok(el_mes_mas_reciente_con(carpeta_de_revisiones, nombre)?.map(|mes| mes.join(nombre)))
}

/// `candidatos.toml` → `candidatos-153012.toml`; `respuestas` → `respuestas-153012`.
fn con_la_hora(nombre: &str, hora: &str) -> String {
    match nombre.rsplit_once('.') {
        Some((base, extension)) => format!("{base}-{hora}.{extension}"),
        None => format!("{nombre}-{hora}"),
    }
}

#[cfg(test)]
mod pruebas {
    use chrono::NaiveDate;

    use super::*;

    fn a_las_tres() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 12)
            .unwrap()
            .and_hms_opt(15, 30, 12)
            .unwrap()
    }

    #[test]
    fn la_carpeta_del_mes_es_ano_y_mes() {
        assert_eq!(
            carpeta_del_mes(Path::new("revisiones"), a_las_tres()),
            Path::new("revisiones").join("2026-11")
        );
    }

    #[test]
    fn la_hora_va_antes_de_la_extension() {
        assert_eq!(
            con_la_hora("candidatos.toml", "091500"),
            "candidatos-091500.toml"
        );
        assert_eq!(con_la_hora("respuestas", "091500"), "respuestas-091500");
    }

    #[test]
    fn el_mas_reciente_es_el_del_ultimo_mes_que_lo_tiene() {
        let raiz = tempfile::tempdir().unwrap();
        assert_eq!(
            el_mas_reciente(&raiz.path().join("no-esta"), "x.toml").unwrap(),
            None
        );
        for (mes, con_fichero) in [("2026-09", true), ("2026-10", true), ("2026-11", false)] {
            let carpeta = raiz.path().join(mes);
            ficheros::crear_carpeta(&carpeta).unwrap();
            if con_fichero {
                ficheros::escribir(&carpeta.join("x.toml"), "").unwrap();
            }
        }
        assert_eq!(
            el_mas_reciente(raiz.path(), "x.toml").unwrap(),
            Some(raiz.path().join("2026-10").join("x.toml"))
        );
        // La carpeta, no el fichero: en uso real (13/11/2026) se cerró sin leer `probados.toml`
        // porque se tomó la ruta del fichero por la de la carpeta del mes.
        assert_eq!(
            el_mes_mas_reciente_con(raiz.path(), "x.toml").unwrap(),
            Some(raiz.path().join("2026-10"))
        );
        assert_eq!(
            el_mes_mas_reciente_con(&raiz.path().join("no-esta"), "x.toml").unwrap(),
            None
        );
    }

    #[test]
    fn sin_el_principal_no_se_aparta_nada() {
        let carpeta = tempfile::tempdir().unwrap();
        ficheros::escribir(&carpeta.path().join("informe.md"), "x").unwrap();
        let apartado =
            apartar_lo_anterior(carpeta.path(), "lista.toml", &["informe.md"], a_las_tres())
                .unwrap();
        assert_eq!(apartado, None);
        assert!(carpeta.path().join("informe.md").exists());
    }

    #[test]
    fn con_el_principal_se_apartan_el_y_sus_acompanantes() {
        let carpeta = tempfile::tempdir().unwrap();
        ficheros::escribir(&carpeta.path().join("lista.toml"), "x").unwrap();
        ficheros::escribir(&carpeta.path().join("informe.md"), "y").unwrap();
        let apartado = apartar_lo_anterior(
            carpeta.path(),
            "lista.toml",
            &["informe.md", "no-esta"],
            a_las_tres(),
        )
        .unwrap();
        assert_eq!(apartado.as_deref(), Some("lista-153012.toml"));
        assert!(carpeta.path().join("lista-153012.toml").exists());
        assert!(carpeta.path().join("informe-153012.md").exists());
        assert!(!carpeta.path().join("lista.toml").exists());
    }
}
