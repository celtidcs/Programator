//! «En uso real»: cuántas propuestas entregó cada modelo en el proyecto donde trabaja Programator,
//! y cuántas se aceptaron y se rechazaron. Sale de su registro de entregas
//! (`arnes::entregas`).
//!
//! Es informativo: no entra en la regla de puntuación. El banco mide encargos de prueba; esto, lo que pasó con el
//! trabajo de verdad.

use std::collections::BTreeMap;
use std::path::Path;

use crate::arnes::entregas::{EstadoDeLaPropuesta, RegistroDeEntregas};

/// Cómo sale en la tabla una propuesta escrita sin modelo anotado.
const SIN_MODELO: &str = "(sin anotar)";

/// Las cuentas de un modelo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cuentas {
    pub entregadas: usize,
    pub aceptadas: usize,
    pub rechazadas: usize,
}

/// Cuenta por modelo, en orden alfabético.
pub fn por_modelo(registro: &RegistroDeEntregas) -> BTreeMap<String, Cuentas> {
    let mut cuentas: BTreeMap<String, Cuentas> = BTreeMap::new();
    for entrada in &registro.entregas {
        let modelo = entrada.modelo.as_deref().unwrap_or(SIN_MODELO).to_string();
        let de_este = cuentas.entry(modelo).or_default();
        de_este.entregadas += 1;
        match entrada.estado {
            EstadoDeLaPropuesta::Aceptada => de_este.aceptadas += 1,
            EstadoDeLaPropuesta::Rechazada => de_este.rechazadas += 1,
            _ => {}
        }
    }
    cuentas
}

/// Lo que se sabe del registro del proyecto, para el apartado.
pub enum Leido {
    /// No hay carpeta de trabajo conocida o no tiene registro.
    SinRegistro,
    /// El fichero existe pero no se entiende.
    Ilegible(String),
    Registro(RegistroDeEntregas),
}

/// Lee el registro sin tocarlo: la revisión no aparta un fichero que no entiende, como hace la
/// pasada; solo lo dice.
pub fn leer(ruta: Option<&Path>) -> Leido {
    let Some(ruta) = ruta.filter(|ruta| ruta.is_file()) else {
        return Leido::SinRegistro;
    };
    match std::fs::read_to_string(ruta)
        .map_err(|e| e.to_string())
        .and_then(|texto| toml::from_str(&texto).map_err(|e| e.to_string()))
    {
        Ok(registro) => Leido::Registro(registro),
        Err(causa) => Leido::Ilegible(causa),
    }
}

/// El apartado de `propuesta.md`.
pub fn seccion(leido: &Leido, ruta: Option<&Path>) -> String {
    let donde = ruta.map_or_else(
        || "ninguno: no hay carpeta de trabajo conocida".to_string(),
        |ruta| format!("«{}»", ruta.display()),
    );
    let mut texto = format!(
        "\n## En uso real\n\nPropuestas que cada modelo entregó en el proyecto, según su registro \
         de entregas ({donde}). Es informativo: no cuenta en la regla de puntuación.\n\n"
    );
    match leido {
        Leido::SinRegistro => texto += "Todavía no hay propuestas anotadas.\n",
        Leido::Ilegible(causa) => {
            texto += &format!("El registro no se pudo leer: {causa}.\n");
        }
        Leido::Registro(registro) => {
            let cuentas = por_modelo(registro);
            if cuentas.is_empty() {
                texto += "Todavía no hay propuestas anotadas.\n";
                return texto;
            }
            texto += "| Modelo | Entregadas | Aceptadas | Rechazadas |\n|---|---|---|---|\n";
            for (modelo, c) in cuentas {
                texto += &format!(
                    "| `{modelo}` | {} | {} | {} |\n",
                    c.entregadas, c.aceptadas, c.rechazadas
                );
            }
        }
    }
    texto
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un `entregas.toml` de ejemplo, como lo deja la pasada.
    const EJEMPLO: &str = r#"
[[entrega]]
propuesta = "a.rs"
encargo = "1"
nombre_del_encargo = "PG-1"
de = "claude.md"
modelo = "devstral"
fecha = "2026-11-20 10:00:00"
veredicto = "pasa"
huella = "h"
estado = "aceptada"
estado_lo_dio = "programator"

[[entrega]]
propuesta = "b.rs"
encargo = "2"
nombre_del_encargo = "PG-2"
de = "claude.md"
modelo = "devstral"
fecha = "2026-11-21 10:00:00"
veredicto = "no_pasa"
huella = "h"
estado = "rechazada"
estado_lo_dio = "Claude 1"
motivo = "no compila"

[[entrega]]
propuesta = "c.rs"
encargo = "3"
nombre_del_encargo = "PG-3"
de = "codex.md"
modelo = "qwen"
fecha = "2026-11-22 10:00:00"
veredicto = "pasa"
huella = "h"
estado = "propuesta"
estado_lo_dio = "programator"

[[entrega]]
propuesta = "d.rs"
encargo = "4"
nombre_del_encargo = "PG-4"
de = "codex.md"
fecha = "2026-11-22 11:00:00"
veredicto = "pasa"
huella = "h"
estado = "sustituida"
estado_lo_dio = "programator"
"#;

    #[test]
    fn el_informe_cuenta_por_modelo_lo_entregado_aceptado_y_rechazado() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = carpeta.path().join("entregas.toml");
        std::fs::write(&ruta, EJEMPLO).unwrap();

        let texto = seccion(&leer(Some(&ruta)), Some(&ruta));

        assert!(texto.contains("| `devstral` | 2 | 1 | 1 |"), "{texto}");
        assert!(texto.contains("| `qwen` | 1 | 0 | 0 |"), "{texto}");
        assert!(texto.contains("| `(sin anotar)` | 1 | 0 | 0 |"), "{texto}");
        assert!(
            texto.contains("no cuenta en la regla de puntuación"),
            "{texto}"
        );
    }

    #[test]
    fn sin_registro_o_ilegible_se_dice_y_no_se_toca() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let ruta = carpeta_temporal.path().join("entregas.toml");
        assert!(seccion(&leer(Some(&ruta)), Some(&ruta)).contains("Todavía no hay"));
        assert!(seccion(&leer(None), None).contains("no hay carpeta de trabajo conocida"));

        std::fs::write(&ruta, "esto no es toml [").unwrap();
        assert!(seccion(&leer(Some(&ruta)), Some(&ruta)).contains("no se pudo leer"));
        assert!(ruta.exists(), "la revisión no aparta el fichero");
    }
}
