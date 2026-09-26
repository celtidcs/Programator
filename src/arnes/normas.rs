//! Compone el fichero de instrucciones que se instala en el proyecto anfitrión.
//!
//! **Por qué se genera una parte en vez de escribirla en la plantilla.** Hasta la 0.9.0 la
//! plantilla afirmaba criterios de aceptación mecánicos —`cargo test` y compañía— que el arnés no
//! ejecuta y que en un proyecto sin Rust son sencillamente falsos. Lo que el modelo lee y lo que el
//! arnés hace no pueden divergir: la sección sale de `[verificacion.comprobadores]`, que es
//! exactamente lo que se va a ejecutar sobre cada propuesta.

use crate::arnes::verificacion::Comprobadores;

/// Lo que la plantilla trae escrito y esta función sustituye.
pub const MARCADOR_COMPROBADORES: &str = "{comprobadores}";

/// El fichero de instrucciones, listo para instalar.
pub fn componer(plantilla: &str, comprobadores: &Comprobadores) -> String {
    plantilla.replace(MARCADOR_COMPROBADORES, &seccion(comprobadores))
}

/// La sección de verificación, redactada a partir de lo que hay configurado.
///
/// Sin comprobadores lo dice con todas las letras. Callar dejaría al modelo suponiendo que alguien
/// comprueba lo que entrega, que es justo la suposición que costó cuatro propuestas sin verificar
/// en la jornada del 23/09/2026.
fn seccion(comprobadores: &Comprobadores) -> String {
    if comprobadores.is_empty() {
        return "**En este proyecto no hay ningún comprobador configurado.** El arnés no puede \
                comprobar lo que entregues: quien reciba tu propuesta tendrá que compilarla a \
                mano. Revísala tú con más cuidado del habitual antes de publicarla."
            .to_string();
    }

    let mut lineas = vec![
        "El arnés comprueba cada propuesta **en cuanto la escribes**, y publica el veredicto junto \
         a lo que tú declares. Esto es lo que ejecutará, según la extensión del fichero:"
            .to_string(),
        String::new(),
    ];
    for (extension, orden) in comprobadores {
        lineas.push(format!("- `.{extension}` → `{}`", orden.join(" ")));
    }
    lineas.push(String::new());
    lineas.push(
        "Una extensión que no esté en esa lista se entrega **sin comprobar**, y así se dirá en el \
         canal."
            .to_string(),
    );
    lineas.join("\n")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn comprobadores(pares: &[(&str, &[&str])]) -> Comprobadores {
        pares
            .iter()
            .map(|(extension, orden)| {
                (
                    extension.to_string(),
                    orden.iter().map(|a| a.to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn el_marcador_nunca_sobrevive_a_la_composicion() {
        // Un marcador que llegue al fichero instalado es texto sin sentido delante del modelo.
        let compuesta = componer("antes {comprobadores} después", &comprobadores(&[]));
        assert!(!compuesta.contains(MARCADOR_COMPROBADORES), "{compuesta}");
    }

    #[test]
    fn sin_comprobadores_se_dice_que_no_hay_ninguno() {
        let compuesta = componer("{comprobadores}", &comprobadores(&[]));
        assert!(
            compuesta.contains("no hay ningún comprobador"),
            "{compuesta}"
        );
    }

    #[test]
    fn cada_comprobador_configurado_sale_con_su_orden_entera() {
        let compuesta = componer(
            "{comprobadores}",
            &comprobadores(&[("py", &["python", "-m", "py_compile", "{fichero}"])]),
        );
        assert!(
            compuesta.contains("`.py` → `python -m py_compile {fichero}`"),
            "{compuesta}"
        );
    }

    #[test]
    fn se_listan_todos_y_en_orden() {
        let compuesta = componer(
            "{comprobadores}",
            &comprobadores(&[("rs", &["rustc"]), ("cs", &["csc"])]),
        );
        let sitio_cs = compuesta.find("`.cs`").expect("falta cs");
        let sitio_rs = compuesta.find("`.rs`").expect("falta rs");
        // `Comprobadores` es un `BTreeMap`: el orden es alfabético y estable, así que el fichero
        // instalado no cambia de una ejecución a otra sin que cambie la configuración.
        assert!(sitio_cs < sitio_rs, "{compuesta}");
    }

    #[test]
    fn lo_que_rodea_al_marcador_se_conserva_tal_cual() {
        let compuesta = componer(
            "# Título\n\n{comprobadores}\n\n## Siguiente",
            &comprobadores(&[]),
        );
        assert!(compuesta.starts_with("# Título"), "{compuesta}");
        assert!(compuesta.ends_with("## Siguiente"), "{compuesta}");
    }
}
