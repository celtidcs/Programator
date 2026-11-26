//! Lo que contesta Programator a un `preparar:`. Todo texto que lee el agente se compone aquí.

use super::diferencias::Diferencias;
use super::ordenes::{Clase, Desenlace, ResultadoDeOrden};
use crate::unidades::BYTES_POR_MIB;

/// Lo que se cuenta de una preparación que llegó a hacerse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Informe {
    pub propuesta: String,
    pub ruta: String,
    /// La copia, relativa a la carpeta de trabajo.
    pub copia: String,
    /// Las órdenes, en orden, con su salida ya recortada.
    pub ordenes: Vec<ResultadoDeOrden>,
    /// Si la extensión no tiene órdenes en `[preparacion]`.
    pub sin_ordenes: bool,
    /// Si en el proyecto no había nada en esa ruta.
    pub nueva: bool,
    pub diferencias: Diferencias,
    /// Ya recortadas.
    pub texto_de_las_diferencias: String,
    /// Los destinos fuera del equipo que se vio intentar a las órdenes aisladas.
    pub intentos_de_red: Vec<String>,
    pub dias_de_vida: u64,
    /// Si el envoltorio no funciona aquí y por eso no se lanzó ninguna orden.
    pub sin_aislamiento: Option<SinAislamiento>,
}

/// El envoltorio que no funcionó y lo que dijo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinAislamiento {
    pub envoltorio: String,
    pub detalle: String,
}

/// Cómo se pide, para quien escribió `preparar:` sin la propuesta o sin la ruta.
pub fn uso() -> String {
    "No se ha preparado nada: falta la propuesta o la ruta. Se pide así: «preparar: \
     <propuesta> en <ruta del proyecto>», con el nombre que tiene la propuesta en la carpeta de \
     candidatos y la ruta donde iría, relativa a la raíz del proyecto."
        .to_string()
}

pub fn propuesta_desconocida(propuesta: &str) -> String {
    format!(
        "No se ha preparado nada: Programator no tiene ninguna propuesta llamada «{propuesta}». \
         El nombre es el que da el aviso de la propuesta en el canal."
    )
}

pub fn fuera_del_proyecto(ruta: &str) -> String {
    format!(
        "No se ha preparado nada: «{ruta}» sale del proyecto o cae en lo que no se copia \
         (.gestor, .git, target, node_modules)."
    )
}

pub fn pasa_del_tope(bytes: u64, tope: u64) -> String {
    format!(
        "No se ha preparado nada: el proyecto ocupa {} MiB y la copia no puede pasar de {} MiB \
         ([preparacion] tope_mib).",
        bytes.div_ceil(BYTES_POR_MIB),
        tope / BYTES_POR_MIB
    )
}

pub fn fallo(motivo: &crate::error::Error) -> String {
    format!("No se pudo preparar la propuesta: {motivo}.")
}

/// Lo que se cuenta de una preparación hecha.
pub fn informe(informe: &Informe) -> String {
    let mut partes = vec![format!(
        "Preparada «{}» en «{}», en una copia del proyecto: «{}». El proyecto no se ha tocado.",
        informe.propuesta, informe.ruta, informe.copia
    )];
    partes.push(if let Some(sin) = &informe.sin_aislamiento {
        sin_aislamiento(sin)
    } else if informe.sin_ordenes {
        format!(
            "No hay órdenes en [preparacion] para los ficheros como «{}»: solo se ha copiado.",
            informe.ruta
        )
    } else {
        std::iter::once("Órdenes:".to_string())
            .chain(informe.ordenes.iter().map(linea_de_la_orden))
            .collect::<Vec<_>>()
            .join("\n")
    });
    partes.push(texto_de_las_diferencias(informe));
    if !informe.intentos_de_red.is_empty() {
        partes.push(format!(
            "Las órdenes aisladas intentaron conectarse a {}. Deberían trabajar sin red: en Linux no \
             la tienen, pero en Windows una conexión directa puede haber salido; mira por qué.",
            informe.intentos_de_red.join(", ")
        ));
    }
    partes.push(format!(
        "La copia se borra cuando digas en qué quedó la propuesta (aceptada: o rechazada:) o, si \
         no, a los {} días.",
        informe.dias_de_vida
    ));
    partes.join("\n\n")
}

/// Por qué no se lanzó ninguna orden: el envoltorio no aísla en este equipo.
fn sin_aislamiento(sin: &SinAislamiento) -> String {
    format!(
        "No se ha lanzado ninguna orden: no se pudieron aislar con «{}» ({}). Algunas \
         distribuciones, como Ubuntu 24.04 y posteriores, no dejan crear espacios de nombres a los \
         usuarios normales. Se puede permitir en el sistema o poner en [preparacion] envoltorio otro \
         que funcione aquí; las órdenes de preparar nunca se lanzan sin aislar.",
        sin.envoltorio, sin.detalle
    )
}

fn linea_de_la_orden(resultado: &ResultadoDeOrden) -> String {
    let clase = match resultado.clase {
        Clase::Previa => "previa",
        Clase::Formatear => "formatear",
        Clase::Aislada => "aislada",
    };
    let orden = resultado.orden.join(" ");
    let como = match &resultado.desenlace {
        Desenlace::Paso => "pasó".to_string(),
        Desenlace::Fallo(Some(codigo)) => format!("falló (código {codigo})"),
        Desenlace::Fallo(None) => "falló (la cortó una señal)".to_string(),
        Desenlace::Agotada => "se cortó al pasar de su tiempo máximo".to_string(),
        Desenlace::NoSeLanzo(motivo) => format!("no se pudo lanzar: {motivo}"),
    };
    let mut linea = format!("- {como}, {clase}: {orden}");
    if !resultado.paso() && !resultado.salida.is_empty() {
        for renglon in resultado.salida.lines() {
            linea.push_str("\n    ");
            linea.push_str(renglon);
        }
    }
    linea
}

/// Si la propuesta de la copia pasó por una orden de formatear que terminó bien. Sin ella, la
/// respuesta no puede decir que está formateada (uso real del 26/11: lo decía sin ninguna orden).
fn quedo_formateada(informe: &Informe) -> bool {
    informe
        .ordenes
        .iter()
        .any(|orden| orden.clase == Clase::Formatear && orden.paso())
}

fn texto_de_las_diferencias(informe: &Informe) -> String {
    let sangrado = || {
        informe
            .texto_de_las_diferencias
            .lines()
            .map(|renglon| format!("    {renglon}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let formateada = quedo_formateada(informe);
    let ruta = &informe.ruta;
    match (&informe.diferencias, informe.nueva) {
        (Diferencias::Iguales, _) if formateada => {
            format!("Ya formateada, la propuesta queda igual que lo que hay en «{ruta}».")
        }
        (Diferencias::Iguales, _) => {
            format!("La propuesta queda igual que lo que hay en «{ruta}».")
        }
        (Diferencias::DemasiadoGrandes, _) => {
            let propuesta = if formateada {
                "la propuesta formateada"
            } else {
                "la propuesta"
            };
            format!(
                "Los ficheros son demasiado grandes para compararlos aquí: {propuesta} está en la \
                 copia, en «{ruta}»."
            )
        }
        (Diferencias::Cambios(_), true) => {
            let formato = if formateada { ". Ya formateado" } else { "" };
            format!(
                "«{ruta}» no existe en el proyecto: sería un fichero nuevo{formato}:\n{}",
                sangrado()
            )
        }
        (Diferencias::Cambios(_), false) => {
            let formato = if formateada {
                ", con la propuesta ya formateada"
            } else {
                ""
            };
            format!(
                "Lo que cambiaría en «{ruta}»{formato} (- se quita, + se pone):\n{}",
                sangrado()
            )
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::comprobacion::lanzador::Vigilancia;

    fn orden(clase: Clase, texto: &str, desenlace: Desenlace, salida: &str) -> ResultadoDeOrden {
        ResultadoDeOrden {
            clase,
            orden: texto.split(' ').map(String::from).collect(),
            desenlace,
            salida: salida.into(),
            vigilancia: Vigilancia::default(),
        }
    }

    fn base() -> Informe {
        Informe {
            propuesta: "lib-002.rs".into(),
            ruta: "src/lib.rs".into(),
            copia: ".gestor/programator/preparacion/abc/proyecto".into(),
            ordenes: vec![
                orden(Clase::Formatear, "cargo fmt", Desenlace::Paso, "nada"),
                orden(
                    Clase::Aislada,
                    "cargo test --offline",
                    Desenlace::Fallo(Some(101)),
                    "error: uno\nerror: dos",
                ),
            ],
            sin_ordenes: false,
            nueva: false,
            diferencias: Diferencias::Cambios(String::new()),
            texto_de_las_diferencias: "- a\n+ b".into(),
            intentos_de_red: Vec::new(),
            dias_de_vida: 2,
            sin_aislamiento: None,
        }
    }

    /// Sin aislamiento no hay lista de órdenes: se dice por qué y cómo arreglarlo.
    #[test]
    fn sin_aislamiento_se_dice_en_lugar_de_las_ordenes() {
        let texto = informe(&Informe {
            ordenes: Vec::new(),
            sin_aislamiento: Some(SinAislamiento {
                envoltorio: "unshare --net".into(),
                detalle: "Operation not permitted".into(),
            }),
            ..base()
        });
        assert!(
            texto.contains(
                "No se ha lanzado ninguna orden: no se pudieron aislar con «unshare --net» \
                 (Operation not permitted)."
            ),
            "{texto}"
        );
        assert!(texto.contains("[preparacion] envoltorio"), "{texto}");
        assert!(!texto.contains("Órdenes:"), "{texto}");
        assert!(texto.contains("    - a\n    + b"), "{texto}");
    }

    /// F3 y la respuesta: cada orden con su desenlace, y la salida solo de las que fallan.
    #[test]
    fn el_informe_cuenta_cada_orden_y_lo_que_cambiaria() {
        let texto = informe(&base());
        assert!(
            texto.contains("«.gestor/programator/preparacion/abc/proyecto»"),
            "{texto}"
        );
        assert!(texto.contains("El proyecto no se ha tocado."), "{texto}");
        assert!(texto.contains("- pasó, formatear: cargo fmt\n"), "{texto}");
        assert!(!texto.contains("nada"), "{texto}");
        assert!(
            texto.contains("- falló (código 101), aislada: cargo test --offline\n    error: uno\n    error: dos"),
            "{texto}"
        );
        assert!(
            texto.contains("(- se quita, + se pone):\n    - a\n    + b"),
            "{texto}"
        );
        assert!(texto.contains("a los 2 días"), "{texto}");
        assert!(!texto.contains("intentaron conectarse"), "{texto}");
    }

    #[test]
    fn el_informe_dice_si_no_hay_ordenes_si_es_nueva_y_la_red() {
        let texto = informe(&Informe {
            sin_ordenes: true,
            nueva: true,
            intentos_de_red: vec!["crates.io:443".into()],
            ..base()
        });
        assert!(texto.contains("No hay órdenes en [preparacion]"), "{texto}");
        assert!(!texto.contains("Órdenes:"), "{texto}");
        assert!(texto.contains("sería un fichero nuevo"), "{texto}");
        assert!(
            texto.contains("intentaron conectarse a crates.io:443. Deberían trabajar sin red"),
            "{texto}"
        );

        let iguales = informe(&Informe {
            diferencias: Diferencias::Iguales,
            ..base()
        });
        assert!(iguales.contains("queda igual"), "{iguales}");
        let grandes = informe(&Informe {
            diferencias: Diferencias::DemasiadoGrandes,
            ..base()
        });
        assert!(grandes.contains("demasiado grandes"), "{grandes}");
    }

    /// Uso real del 26/11: sin órdenes no se formatea nada, y la respuesta no puede decir «ya
    /// formateado». Tampoco si la orden de formatear falló.
    #[test]
    fn sin_formatear_no_dice_que_la_propuesta_esta_formateada() {
        let sin_formatear = |nueva: bool, diferencias: Diferencias| {
            let sin_ordenes = informe(&Informe {
                ordenes: Vec::new(),
                sin_ordenes: true,
                nueva,
                diferencias: diferencias.clone(),
                ..base()
            });
            let formatear_falla = informe(&Informe {
                ordenes: vec![orden(
                    Clase::Formatear,
                    "cargo fmt",
                    Desenlace::Fallo(Some(1)),
                    "",
                )],
                nueva,
                diferencias,
                ..base()
            });
            [sin_ordenes, formatear_falla]
        };
        for texto in sin_formatear(true, Diferencias::Cambios(String::new())) {
            assert!(
                texto.contains("sería un fichero nuevo:\n    - a"),
                "{texto}"
            );
            assert!(!texto.contains("formatead"), "{texto}");
        }
        for texto in sin_formatear(false, Diferencias::Cambios(String::new())) {
            assert!(
                texto.contains("Lo que cambiaría en «src/lib.rs» (- se quita, + se pone):"),
                "{texto}"
            );
            assert!(!texto.contains("formatead"), "{texto}");
        }
        for texto in sin_formatear(false, Diferencias::Iguales) {
            assert!(
                texto.contains("La propuesta queda igual que lo que hay en «src/lib.rs»."),
                "{texto}"
            );
            assert!(!texto.contains("formatead"), "{texto}");
        }
        let formateada = informe(&Informe {
            nueva: true,
            ..base()
        });
        assert!(formateada.contains("Ya formateado:"), "{formateada}");
    }

    #[test]
    fn los_avisos_sin_preparar_lo_dicen() {
        assert!(uso().contains("preparar: <propuesta> en <ruta del proyecto>"));
        assert!(propuesta_desconocida("x.rs").contains("«x.rs»"));
        assert!(fuera_del_proyecto("../x").contains("«../x»"));
        assert_eq!(
            pasa_del_tope(600 * BYTES_POR_MIB + 1, 512 * BYTES_POR_MIB),
            "No se ha preparado nada: el proyecto ocupa 601 MiB y la copia no puede pasar de 512 \
             MiB ([preparacion] tope_mib)."
        );
    }
}
