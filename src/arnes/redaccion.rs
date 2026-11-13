//! Redacta en prosa lo que `ficha` declara, para las plantillas que lo publican.
//!
//! **Por qué se genera en vez de escribirse.** La tabla de herramientas del preámbulo estaba
//! escrita a mano y llegó a nombrar mal un argumento: decía que `publicar` tomaba «cuerpo» cuando
//! el repertorio pide «texto», así que un modelo que siguiera sus propias instrucciones se llevaba
//! una denegación. También decía «tienes cinco herramientas», cifra que caducaba en cuanto
//! `verificar` volviera a ser concedible. Aquí no puede pasar ninguna de las dos cosas.
//!
//! Este módulo **no decide nada**: solo redacta. Qué existe lo dice `ficha`; qué se comprueba lo
//! dice `normas`.

use super::ficha::{self, Ficha};

/// Lo que las plantillas traen escrito donde va el repertorio.
pub const MARCADOR_REPERTORIO: &str = "{repertorio}";

/// Lo que la guía de los agentes trae escrito donde van las cifras de desempeño.
pub const MARCADOR_DESEMPENO: &str = "{desempeno}";

/// Sustituye el marcador del repertorio por la tabla redactada.
pub fn componer(plantilla: &str) -> String {
    plantilla.replace(MARCADOR_REPERTORIO, &tabla_de_herramientas())
}

/// Compone la guía que leen los demás agentes: el repertorio, de la ficha; el desempeño, del
/// fragmento medido que se le pasa ya leído.
///
/// El desempeño llega de fuera en vez de leerse aquí porque este módulo no toca disco: quién lee
/// qué fichero es asunto de `arranque`, y así esto se prueba sin tocar el sistema de ficheros.
pub fn componer_guia(plantilla: &str, desempeno: &str) -> String {
    componer(plantilla).replace(MARCADOR_DESEMPENO, desempeno)
}

/// La tabla de lo que hoy se concede, con el nombre exacto de cada argumento.
///
/// Solo entran las concedibles: anunciar lo que se deniega siempre solo sirve para que el modelo
/// queme solicitudes contra el tope del encargo.
pub fn tabla_de_herramientas() -> String {
    let concedibles: Vec<&Ficha> = ficha::todas()
        .iter()
        .filter(|f| f.es_concedible())
        .collect();

    let mut lineas = vec![
        format!(
            "Tienes **{} herramientas, y solo estas {}**:",
            concedibles.len(),
            concedibles.len()
        ),
        String::new(),
        "| Herramienta | Para qué | Argumentos |".to_string(),
        "|---|---|---|".to_string(),
    ];

    for ficha in &concedibles {
        let argumentos: Vec<String> = ficha
            .obligatorios
            .iter()
            .map(|a| format!("`{}`", a.nombre))
            .chain(
                ficha
                    .opcionales
                    .iter()
                    .map(|a| format!("`{}` (opcional)", a.nombre)),
            )
            .collect();
        let argumentos = if argumentos.is_empty() {
            "ninguno".to_string()
        } else {
            argumentos.join(" **y** ")
        };
        lineas.push(format!(
            "| `{}` | {} | {} |",
            ficha.nombre, ficha.para_que, argumentos
        ));
    }

    lineas.push(String::new());
    lineas.push(
        "**Los argumentos van con ese nombre exacto.** Si llamas a una herramienta sin un \
         argumento obligatorio, se deniega y se te recuerda su firma; si insistes tras una \
         denegación, el encargo se aborta y no entregas nada."
            .to_string(),
    );

    lineas.join("\n")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La huella SHA-256, en minúsculas, del nombre del proyecto privado en que se midió al
    /// principio.
    const HUELLA_DEL_PROYECTO_PRIVADO: &str =
        "3142c9009eac67fbc0354e37317b3ab3af42a3c01a5b9a315595989fddda539d";

    #[test]
    fn la_tabla_trae_los_concedibles_con_sus_argumentos_exactos() {
        let tabla = tabla_de_herramientas();

        assert!(
            tabla.contains("`leer_fichero`"),
            "falta leer_fichero:\n{tabla}"
        );
        assert!(
            tabla.contains("`texto`"),
            "publicar tiene que pedir «texto»:\n{tabla}"
        );
        assert!(
            !tabla.contains("`cuerpo`"),
            "«cuerpo» es el nombre equivocado que traía el preámbulo a mano:\n{tabla}"
        );
    }

    #[test]
    fn la_tabla_no_anuncia_lo_que_se_deniega_siempre() {
        let tabla = tabla_de_herramientas();
        assert!(!tabla.contains("proponer_poda"));
        assert!(!tabla.contains("buscar"));
    }

    #[test]
    fn la_tabla_dice_cuantas_son_en_vez_de_dejar_la_cifra_escrita_a_mano() {
        let tabla = tabla_de_herramientas();
        let cuantas = ficha::todas().iter().filter(|f| f.es_concedible()).count();
        assert!(
            tabla.contains(&cuantas.to_string()),
            "la cifra tiene que salir de la ficha, que es lo que impide que envejezca:\n{tabla}"
        );
    }

    #[test]
    fn componer_sustituye_el_marcador_y_deja_el_resto_intacto() {
        let compuesta = componer("antes\n\n{repertorio}\n\ndespués");
        assert!(compuesta.starts_with("antes"));
        assert!(compuesta.ends_with("después"));
        assert!(!compuesta.contains(MARCADOR_REPERTORIO));
    }

    #[test]
    fn una_plantilla_sin_marcador_sale_tal_cual() {
        assert_eq!(componer("sin marcador"), "sin marcador");
    }

    #[test]
    fn la_plantilla_del_preambulo_no_lleva_ninguna_firma_escrita_a_mano() {
        // La barrera contra la deriva: si alguien vuelve a escribir la tabla dentro de la
        // plantilla, esta prueba lo caza antes de que envejezca.
        let plantilla = std::fs::read_to_string("plantillas/preambulo-del-modelo.md")
            .expect("la plantilla del preámbulo está en el repositorio");

        assert!(
            plantilla.contains(MARCADOR_REPERTORIO),
            "el preámbulo tiene que pedir el repertorio generado"
        );
        for ficha in ficha::todas() {
            assert!(
                !plantilla.contains(&format!("| `{}` |", ficha.nombre)),
                "«{}» está en una tabla escrita a mano dentro de la plantilla",
                ficha.nombre
            );
        }
    }

    #[test]
    fn la_guia_no_nombra_el_proyecto_privado_en_que_se_midio_al_principio() {
        // El proyecto privado en que se midió al principio sirvió para desarrollar Programator,
        // no para los usuarios. La guía es lo que llega a los proyectos de otros, y no lo nombra.
        let guia = std::fs::read_to_string("plantillas/como-encargar-a-programator.md")
            .expect("la guía está en el repositorio");

        // Su nombre tampoco se escribe aquí: va su huella SHA-256 en minúsculas.
        assert!(!crate::ficheros::nombra_alguna_huella(
            &guia,
            &[HUELLA_DEL_PROYECTO_PRIVADO]
        ));
    }

    #[test]
    fn la_guia_explica_el_encargo_en_una_linea_del_canal() {
        // La guía que Programator publica explica el encargo con la forma del resto de agentes, una línea que empieza por `Para Programator`,
        // y el encabezado solo como forma antigua.
        let guia = std::fs::read_to_string("plantillas/como-encargar-a-programator.md")
            .expect("la guía está en el repositorio");

        assert!(guia.contains("\nPara Programator: nombre corto del encargo.\n"));
        assert!(guia.contains("LATIDO: 2026-11-13 16:05:00"));
        assert!(guia.contains("con copia a Programator"));
        assert!(guia.contains("la forma antigua"));
        assert!(
            !guia.contains("```markdown\n## Para Programator"),
            "ningún ejemplo enseña ya el encabezado"
        );
    }

    #[test]
    fn componer_guia_mete_el_repertorio_y_el_desempeno() {
        let compuesta = componer_guia("{repertorio}\n\n{desempeno}", "CIFRAS");
        assert!(compuesta.contains("`leer_fichero`"));
        assert!(compuesta.contains("CIFRAS"));
        assert!(!compuesta.contains(MARCADOR_DESEMPENO));
    }

    #[test]
    fn la_guia_de_los_agentes_nombra_lo_que_el_modelo_si_puede_hacer() {
        let guia = std::fs::read_to_string("plantillas/como-encargar-a-programator.md")
            .expect("la guía está en el repositorio");

        assert!(
            guia.contains(MARCADOR_REPERTORIO),
            "la guía tiene que traer el repertorio generado: sin él, quien dirige no sabe que el \
             modelo puede leer ficheros, y le pega el fichero entero en el encargo"
        );
        assert!(
            guia.contains(MARCADOR_DESEMPENO),
            "la guía tiene que traer las cifras medidas"
        );
    }

    #[test]
    fn la_guia_no_afirma_que_el_modelo_carezca_de_herramientas() {
        let guia = std::fs::read_to_string("plantillas/como-encargar-a-programator.md")
            .expect("la guía está en el repositorio");

        assert!(
            !guia.contains("tienes tú herramientas y él no"),
            "esa frase es falsa y costó una jornada: el modelo tiene cinco, leer_fichero entre ellas"
        );
    }

    #[test]
    fn la_guia_no_recomienda_la_tarea_que_el_modelo_falla_siempre() {
        let guia = std::fs::read_to_string("plantillas/como-encargar-a-programator.md")
            .expect("la guía está en el repositorio");

        assert!(
            !guia.contains("Lo mejor que hace"),
            "diagnosticar causas está medido en 0 de 12: la guía no puede encabezar con ello"
        );
    }

    #[test]
    fn ningun_verbo_concedible_falta_en_los_textos_que_se_publican() {
        let preambulo = componer(
            &std::fs::read_to_string("plantillas/preambulo-del-modelo.md")
                .expect("la plantilla del preámbulo está en el repositorio"),
        );
        let guia = componer_guia(
            &std::fs::read_to_string("plantillas/como-encargar-a-programator.md")
                .expect("la guía está en el repositorio"),
            "",
        );

        for ficha in ficha::todas().iter().filter(|f| f.es_concedible()) {
            assert!(
                preambulo.contains(ficha.nombre),
                "«{}» no aparece en el preámbulo del modelo",
                ficha.nombre
            );
            assert!(
                guia.contains(ficha.nombre),
                "«{}» no aparece en la guía de los agentes",
                ficha.nombre
            );
        }
    }

    #[test]
    fn ningun_texto_nombra_un_verbo_que_no_exista() {
        // Si alguien renombra un verbo en la ficha y se olvida de la prosa escrita a mano, esta
        // prueba lo caza. Busca en la prosa los nombres con guion bajo entre comillas simples de
        // Markdown, que es como se escriben los verbos en estas plantillas.
        let guia = std::fs::read_to_string("plantillas/como-encargar-a-programator.md")
            .expect("la guía está en el repositorio");

        // La sección «se inventa APIs» mide si el modelo alucina funciones ajenas (de Tokio, de
        // serde) y las nombra con la misma comilla invertida que un verbo: `spawn_blocking_scoped`,
        // `spawn_blocking`. No hay forma de distinguirlas de un verbo real por la forma de la
        // palabra —las dos son snake_case en minúsculas—, así que se excluye la sección entera de
        // la búsqueda: no habla del repertorio, habla de bibliotecas de terceros que no están, ni
        // tienen por qué estar, en `ficha`.
        //
        // El límite exacto de esta prueba: si alguien escribe dentro de esa sección un verbo
        // inventado del propio repertorio —por ejemplo `` `borrar_fichero` `` colado en la tabla de
        // APIs ajenas—, esta prueba no lo detecta, porque toda la sección queda fuera de la
        // búsqueda. Se acepta ese punto ciego porque esa sección habla de bibliotecas de terceros,
        // no del repertorio propio: un verbo del repertorio no tiene motivo para aparecer ahí, y si
        // apareciera sería un error de redacción ajeno a lo que esta prueba vigila.
        let guia_sin_ejemplos_de_apis_ajenas = guia
            .split("\n## ")
            .filter(|seccion| !seccion.contains("se inventa APIs"))
            .collect::<Vec<_>>()
            .join("\n## ");

        for palabra in guia_sin_ejemplos_de_apis_ajenas
            .split('`')
            .skip(1)
            .step_by(2)
        {
            let parece_un_verbo = palabra.contains('_')
                && palabra.chars().all(|c| c.is_ascii_lowercase() || c == '_');
            if parece_un_verbo {
                assert!(
                    ficha::buscar(palabra).is_some(),
                    "la guía nombra «{palabra}», que no está en el repertorio"
                );
            }
        }
    }
}
