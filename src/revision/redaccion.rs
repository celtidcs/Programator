//! Cómo queda escrita una propuesta: `propuesta.md`, para que la lea el usuario, y
//! `propuesta.toml`, con el motivo de cada retirada propuesta para que `--retirar-modelo` lo anote
//! en el registro sin tener que leer el Markdown.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::informe::NINGUNO;
use super::mensajes::{
    motivo_de_exclusion, motivo_de_retirada, nombre_de_categoria,
    sin_comparar as mensaje_sin_comparar, sin_datos_para_decidir,
};
use super::propuesta::Propuesta;
use super::DECIMALES_DE_LA_NOTA;
use crate::argumentos::OPCION_RETIRAR_MODELO;
use crate::banco::resultados::decimal;
use crate::error::{Error, Resultado};
use crate::plataforma::NOMBRE_DEL_EJECUTABLE;

/// La propuesta para que la lea el usuario.
pub const NOMBRE_DE_LA_PROPUESTA: &str = "propuesta.md";
/// Las retiradas propuestas, con su motivo, para que las lea `--retirar-modelo`.
pub const NOMBRE_DE_LAS_RETIRADAS: &str = "propuesta.toml";
/// Lo que dice la tabla de ganadores en una categoría que no se compara.
const SE_DEJA_FUERA: &str = "se deja fuera";

/// Las retiradas que propone la propuesta, cada una con su motivo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetiradasPropuestas {
    #[serde(default, rename = "retirar")]
    pub retiradas: Vec<RetiradaPropuesta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetiradaPropuesta {
    /// El nombre del registro: el del fichero sin `.gguf`.
    pub nombre: String,
    pub motivo: String,
}

pub fn retiradas(propuesta: &Propuesta) -> RetiradasPropuestas {
    RetiradasPropuestas {
        retiradas: propuesta
            .para_retirar
            .iter()
            .map(|r| RetiradaPropuesta {
                nombre: r.nombre.clone(),
                motivo: motivo_de_retirada(r),
            })
            .collect(),
    }
}

pub fn escribir_retiradas(retiradas: &RetiradasPropuestas) -> Resultado<String> {
    toml::to_string(retiradas)
        .map_err(|causa| Error::Revision(super::mensajes::retiradas_sin_componer(&causa)))
}

pub fn leer_retiradas(texto: &str) -> Resultado<RetiradasPropuestas> {
    toml::from_str(texto)
        .map_err(|causa| Error::Revision(super::mensajes::retiradas_que_no_se_entienden(&causa)))
}

/// `propuesta.md`. Empieza por de dónde salen las cifras, para que se puedan comprobar.
pub fn componer(propuesta: &Propuesta, registro: &Path, fecha: &str) -> String {
    let mut texto = format!("# Propuesta de la revisión del {fecha}\n\n");
    texto += &procedencia(propuesta, registro);
    texto += &ganadores(propuesta);
    texto += &excluidas(propuesta);
    texto += &se_quedan(propuesta);
    texto += &para_retirar(propuesta);
    texto += &sin_medir(propuesta);
    texto += &sin_fichero(propuesta);
    texto += &lo_que_pasa_al_cerrar();
    texto
}

/// Lo que hace `revisión: cerrar` con esta propuesta. Lo deciden las reglas de puntuación, sin
/// pedir permiso a nadie; antes este apartado decía que nada se retiraba sin una orden, y el cierre
/// retiraba igual.
fn lo_que_pasa_al_cerrar() -> String {
    format!(
        "## Lo que pasa al cerrar\n\n\
         Con «revisión: cerrar» se retiran los modelos de «Para retirar» y, si el modelo en uso no \
         gana nada y hay otro que lo sustituya, se cambia por él. Lo deciden las reglas de \
         puntuación, sin pedir permiso. A mano, un modelo se retira con \
         `{NOMBRE_DEL_EJECUTABLE} {OPCION_RETIRAR_MODELO} <fichero>`.\n"
    )
}

fn procedencia(propuesta: &Propuesta, registro: &Path) -> String {
    let mut texto = format!(
        "Las cifras salen de «{}», de las entradas de la tarjeta {} sin retirar:\n\n",
        registro.display(),
        propuesta.tarjeta
    );
    if propuesta.procedencia.is_empty() {
        return texto + NINGUNO + "\n";
    }
    for p in &propuesta.procedencia {
        texto += &format!(
            "- `{}`: tanda `{}`, medida el {}.\n",
            p.nombre, p.tanda, p.medido_el
        );
    }
    texto + "\n"
}

fn ganadores(propuesta: &Propuesta) -> String {
    let mut texto = "## Quién gana cada clase y cada familia\n\n\
                     | Clase o familia | Gana | Nota |\n|---|---|---|\n"
        .to_string();
    for g in &propuesta.ganadores {
        let excluida = propuesta
            .excluidas
            .iter()
            .any(|x| x.categoria == g.categoria);
        let (quien, nota) = match &g.ganador {
            Some(v) => (
                format!("`{}`", v.nombre),
                decimal(v.nota, DECIMALES_DE_LA_NOTA),
            ),
            None if excluida => (SE_DEJA_FUERA.to_string(), "—".to_string()),
            None => (crate::unidades::SIN_MEDIR.to_string(), "—".to_string()),
        };
        texto += &format!(
            "| {} | {quien} | {nota} |\n",
            nombre_de_categoria(g.categoria)
        );
    }
    texto + "\n"
}

/// Las cifras de código que no se comparan, por qué, y si eso deja la propuesta sin decidir.
fn excluidas(propuesta: &Propuesta) -> String {
    if propuesta.excluidas.is_empty() {
        return String::new();
    }
    let mut texto = format!(
        "## Cifras que no se comparan ({})\n\n",
        propuesta.excluidas.len()
    );
    for excluida in &propuesta.excluidas {
        texto += &format!("- {}.\n", motivo_de_exclusion(excluida));
    }
    for nombre in &propuesta.sin_comparar {
        texto += &mensaje_sin_comparar(nombre);
    }
    if propuesta.sin_datos_para_decidir {
        texto += &format!("\n{}\n", sin_datos_para_decidir());
    }
    texto + "\n"
}

fn se_quedan(propuesta: &Propuesta) -> String {
    let mut texto = format!("## Se quedan ({})\n\n", propuesta.se_quedan.len());
    if propuesta.se_quedan.is_empty() {
        return texto + NINGUNO + "\n";
    }
    for q in &propuesta.se_quedan {
        let en_uso = if q.en_uso {
            " (el de `programator.toml`)"
        } else {
            ""
        };
        let gana = if q.gana.is_empty() {
            "no gana nada".to_string()
        } else {
            let nombres: Vec<String> = q.gana.iter().map(|&c| nombre_de_categoria(c)).collect();
            format!("gana {}", nombres.join("; "))
        };
        texto += &format!("- `{}`{en_uso}: {gana}.\n", q.nombre);
    }
    if let Some(en_uso) = &propuesta.en_uso_sin_victorias {
        let sustituto = en_uso.sustituto.as_deref().map_or(
            "ningún otro modelo gana nada todavía, así que se queda".to_string(),
            |s| format!("al cerrar la revisión se cambia por `{s}`, el que más gana"),
        );
        texto += &format!(
            "\nEl modelo en uso, `{}`, no gana ninguna clase ni familia: {sustituto}.\n",
            en_uso.nombre
        );
    }
    texto + "\n"
}

fn para_retirar(propuesta: &Propuesta) -> String {
    let mut texto = format!(
        "## Se propone retirar ({})\n\n",
        propuesta.para_retirar.len()
    );
    if propuesta.para_retirar.is_empty() {
        return texto + NINGUNO + "\n";
    }
    for r in &propuesta.para_retirar {
        texto += &format!("- `{}`: {}.\n", r.nombre, motivo_de_retirada(r));
    }
    texto + "\n"
}

fn sin_medir(propuesta: &Propuesta) -> String {
    let mut texto = format!("## Sin medir ({})\n\n", propuesta.sin_medir.len());
    if propuesta.sin_medir.is_empty() {
        return texto + NINGUNO + "\n";
    }
    for fichero in &propuesta.sin_medir {
        texto += &format!("- `{fichero}`: pásale el banco antes de decidir.\n");
    }
    texto + "\n"
}

/// Los modelos del registro que no tienen fichero: no se han tenido en cuenta para nada.
fn sin_fichero(propuesta: &Propuesta) -> String {
    let mut texto = format!(
        "## En el registro, sin fichero ({})\n\n",
        propuesta.sin_fichero.len()
    );
    if propuesta.sin_fichero.is_empty() {
        return texto + NINGUNO + "\n";
    }
    for nombre in &propuesta.sin_fichero {
        texto += &format!(
            "- `{nombre}`: tiene cifras en el registro, pero su fichero no está en la carpeta de \
             modelos, así que no se ha tenido en cuenta.\n"
        );
    }
    texto + "\n" + QUE_SE_HACE_CON_LOS_QUE_FALTAN + "\n\n"
}

/// Lo que hace la revisión con los modelos del registro cuyo fichero falta.
const QUE_SE_HACE_CON_LOS_QUE_FALTAN: &str =
    "Los que ganarían algo, y el modelo por defecto, los intenta reponer la siguiente «revisión: \
     buscar»; los que no ganarían nada, «revisión: cerrar» los saca del registro.";

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::ficha::Familia;
    use crate::banco::resultados::FilaDeDesempeno;
    use crate::registro::modelo::CodigoPuntuadoCon;
    use crate::revision::propuesta::{
        Categoria, EnUsoSinVictorias, Excluida, Ganador, ParaRetirar, Procedencia, SeQueda,
        Vencedor,
    };

    const FORMAS: Categoria = Categoria::Clase(FilaDeDesempeno::Formas);

    fn propuesta() -> Propuesta {
        Propuesta {
            tarjeta: "Tarjeta".into(),
            procedencia: vec![Procedencia {
                nombre: "fuerte".into(),
                tanda: "2026-11-12-1219-fuerte".into(),
                medido_el: "2026-11-12 12:19".into(),
            }],
            ganadores: vec![
                Ganador {
                    categoria: FORMAS,
                    ganador: Some(Vencedor {
                        nombre: "fuerte".into(),
                        nota: 4.0,
                    }),
                },
                Ganador {
                    categoria: Categoria::Clase(FilaDeDesempeno::Diagnostico),
                    ganador: None,
                },
            ],
            se_quedan: vec![
                SeQueda {
                    nombre: "fuerte".into(),
                    gana: vec![FORMAS],
                    en_uso: false,
                },
                SeQueda {
                    nombre: "en-uso".into(),
                    gana: vec![],
                    en_uso: true,
                },
            ],
            para_retirar: vec![ParaRetirar {
                nombre: "flojo".into(),
                mejor_nota: 3.0,
                en: FORMAS,
                le_gana: Vencedor {
                    nombre: "fuerte".into(),
                    nota: 4.0,
                },
            }],
            sin_medir: vec!["nuevo.gguf".into()],
            sin_fichero: Vec::new(),
            en_uso_sin_victorias: Some(EnUsoSinVictorias {
                nombre: "en-uso".into(),
                sustituto: Some("fuerte".into()),
            }),
            excluidas: Vec::new(),
            sin_comparar: Vec::new(),
            sin_datos_para_decidir: false,
        }
    }

    #[test]
    fn dice_lo_que_hace_el_cierre_sin_pedir_permiso() {
        let texto = componer(&propuesta(), Path::new("r.toml"), "2026-11-25");
        assert!(texto.contains("## Lo que pasa al cerrar"), "{texto}");
        assert!(texto.contains("sin pedir permiso"), "{texto}");
        assert!(
            texto.contains("al cerrar la revisión se cambia por `fuerte`, el que más gana."),
            "{texto}"
        );
        assert!(!texto.contains("Director"), "{texto}");

        let mut sin_sustituto = propuesta();
        if let Some(en_uso) = sin_sustituto.en_uso_sin_victorias.as_mut() {
            en_uso.sustituto = None;
        }
        let texto = componer(&sin_sustituto, Path::new("r.toml"), "2026-11-25");
        assert!(
            texto.contains("ningún otro modelo gana nada todavía, así que se queda."),
            "{texto}"
        );
    }

    #[test]
    fn empieza_por_el_registro_y_las_tandas() {
        let texto = componer(
            &propuesta(),
            Path::new("registro-de-modelos.toml"),
            "2026-11-12",
        );
        let registro = texto.find("registro-de-modelos.toml").unwrap();
        let tanda = texto.find("2026-11-12-1219-fuerte").unwrap();
        let ganadores = texto.find("## Quién gana").unwrap();
        assert!(registro < ganadores && tanda < ganadores, "{texto}");
    }

    #[test]
    fn dice_cada_ganador_y_las_clases_sin_medir() {
        let texto = componer(&propuesta(), Path::new("r.toml"), "2026-11-12");
        assert!(
            texto.contains("| Auditar buscando formas | `fuerte` | 4,0 |"),
            "{texto}"
        );
        assert!(
            texto.contains("| Diagnosticar la causa de un fallo | sin medir | — |"),
            "{texto}"
        );
    }

    #[test]
    fn dice_que_se_retira_por_que_y_lo_que_falta_por_medir() {
        let texto = componer(&propuesta(), Path::new("r.toml"), "2026-11-12");
        assert!(
            texto.contains(
                "- `flojo`: no gana ninguna clase ni familia: su mejor nota es 3,0 en «Auditar \
                 buscando formas», donde gana fuerte con 4,0."
            ),
            "{texto}"
        );
        assert!(texto.contains("- `nuevo.gguf`: pásale el banco"), "{texto}");
    }

    #[test]
    fn nombra_los_modelos_del_registro_que_no_tienen_fichero() {
        let mut con_ausente = propuesta();
        con_ausente.sin_fichero = vec!["perdido".into()];

        let texto = componer(&con_ausente, Path::new("r.toml"), "2026-11-24");
        let sin_ausentes = componer(&propuesta(), Path::new("r.toml"), "2026-11-24");

        assert!(
            texto.contains(
                "## En el registro, sin fichero (1)\n\n- `perdido`: tiene cifras en el registro, \
                 pero su fichero no está en la carpeta de modelos, así que no se ha tenido en \
                 cuenta.\n\nLos que ganarían algo, y el modelo por defecto, los intenta reponer la \
                 siguiente «revisión: buscar»; los que no ganarían nada, «revisión: cerrar» los \
                 saca del registro.\n\n"
            ),
            "{texto}"
        );
        assert!(
            !sin_ausentes.contains("los intenta reponer"),
            "{sin_ausentes}"
        );
        assert!(
            sin_ausentes.contains("## En el registro, sin fichero (0)\n\nNinguno."),
            "{sin_ausentes}"
        );
    }

    /// La orden de retirada se escribe con el nombre del programa en cada sistema: en Windows,
    /// con su extensión.
    #[test]
    fn la_orden_de_retirada_lleva_el_nombre_del_programa_de_cada_sistema() {
        let texto = componer(&propuesta(), Path::new("r.toml"), "2026-11-12");
        let esperada = if cfg!(windows) {
            "`programator.exe --retirar-modelo <fichero>`"
        } else {
            "`programator --retirar-modelo <fichero>`"
        };
        assert!(texto.contains(esperada), "{texto}");
    }

    #[test]
    fn el_modelo_en_uso_sin_victorias_dice_quien_lo_sustituiria() {
        let texto = componer(&propuesta(), Path::new("r.toml"), "2026-11-12");
        assert!(texto.contains("- `en-uso` (el de `programator.toml`): no gana nada."));
        assert!(
            texto.contains("se cambia por `fuerte`") && texto.contains("revisión: cerrar"),
            "{texto}"
        );
    }

    const USO: Categoria = Categoria::Familia(Familia::Uso);

    /// La de siempre, con el uso dejado fuera: uno lo puntuó con pruebas y otro con rúbrica.
    fn con_el_uso_fuera() -> Propuesta {
        let mut p = propuesta();
        p.ganadores.push(Ganador {
            categoria: USO,
            ganador: None,
        });
        p.excluidas = vec![Excluida {
            categoria: USO,
            formas: vec![
                ("fuerte".into(), Some(CodigoPuntuadoCon::PruebasOcultas)),
                ("flojo".into(), Some(CodigoPuntuadoCon::Rubrica)),
            ],
        }];
        p
    }

    #[test]
    fn sin_excluidas_no_hay_apartado_de_cifras_que_no_se_comparan() {
        let texto = componer(&propuesta(), Path::new("r.toml"), "2026-11-13");
        assert!(!texto.contains("no se comparan"), "{texto}");
    }

    #[test]
    fn dice_que_cifras_se_dejan_fuera_y_por_que() {
        let texto = componer(&con_el_uso_fuera(), Path::new("r.toml"), "2026-11-13");
        assert!(
            texto.contains("| Familia de uso | se deja fuera | — |"),
            "{texto}"
        );
        assert!(
            texto.contains("## Cifras que no se comparan (1)"),
            "{texto}"
        );
        assert!(
            texto.contains(
                "`fuerte`, ejecutando las pruebas ocultas; `flojo`, leyendo el código con la \
                 rúbrica, sin ejecutarlo"
            ),
            "{texto}"
        );
        assert!(!texto.contains("No se propone ninguna"), "{texto}");
    }

    #[test]
    fn sin_datos_para_decidir_lo_dice_y_nombra_a_los_que_quedan_sin_comparar() {
        let mut p = con_el_uso_fuera();
        p.sin_comparar = vec!["flojo".into()];
        p.sin_datos_para_decidir = true;
        let texto = componer(&p, Path::new("r.toml"), "2026-11-13");
        assert!(texto.contains("- `flojo`: se queda sin decidir"), "{texto}");
        assert!(
            texto.contains("**No se propone ninguna sustitución ni retirada.**"),
            "{texto}"
        );
    }

    #[test]
    fn las_retiradas_van_y_vuelven_con_su_motivo() {
        let escritas = retiradas(&propuesta());
        let texto = escribir_retiradas(&escritas).unwrap();
        assert!(texto.contains("[[retirar]]"), "{texto}");
        let leidas = leer_retiradas(&texto).unwrap();
        assert_eq!(leidas, escritas);
        assert_eq!(leidas.retiradas[0].nombre, "flojo");
        assert!(leidas.retiradas[0].motivo.contains("gana fuerte con 4,0"));
    }
}
