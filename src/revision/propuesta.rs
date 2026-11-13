//! Qué modelo gana cada clase y cada familia, cuáles se quedan y cuáles se propone retirar
//! (especificación, §6).
//!
//! Puro: recibe el registro, la tarjeta, lo que hay en la carpeta de modelos y el modelo en uso, y
//! no toca disco ni GPU. La redacción va aparte, en `redaccion`.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::banco::ficha::{forma_de_puntuar, Familia, FormaDePuntuar};
use crate::banco::resultados::{FilaDeDesempeno, FAMILIAS, FILAS_DE_DESEMPENO};
use crate::registro::modelo::{CodigoPuntuadoCon, EntradaDeModelo, Registro};

/// La extensión de los modelos de la carpeta: el nombre del registro es el del fichero sin ella.
const EXTENSION_DEL_MODELO: &str = "gguf";

/// Lo que se compara: una fila de desempeño o una familia del banco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Categoria {
    Clase(FilaDeDesempeno),
    Familia(Familia),
}

impl Categoria {
    /// Si su nota sale, en todo o en parte, de encargos de código: los que se puntúan con pruebas
    /// ocultas o, si no dan nota, con su rúbrica. Se deduce de cómo se puntúa cada familia y cada
    /// clase, para que no haya una lista que se quede vieja.
    pub fn depende_del_codigo(self) -> bool {
        let es_de_codigo = |familia, clase| {
            forma_de_puntuar(familia, clase) == Some(FormaDePuntuar::Comprobaciones)
        };
        let clases_de = |fila: FilaDeDesempeno| fila.clases().iter().copied();
        match self {
            Categoria::Clase(fila) => clases_de(fila).any(|c| es_de_codigo(Familia::Uso, Some(c))),
            Categoria::Familia(familia) => {
                es_de_codigo(familia, None)
                    || FILAS_DE_DESEMPENO
                        .iter()
                        .flat_map(|&fila| clases_de(fila))
                        .any(|c| es_de_codigo(familia, Some(c)))
            }
        }
    }
}

/// Las seis clases y después las tres familias, en el orden en que se cuentan.
pub fn categorias() -> Vec<Categoria> {
    FILAS_DE_DESEMPENO
        .iter()
        .map(|&fila| Categoria::Clase(fila))
        .chain(FAMILIAS.iter().map(|&familia| Categoria::Familia(familia)))
        .collect()
}

/// Lo que manda la propuesta.
pub struct Entrada<'a> {
    pub registro: &'a Registro,
    /// La tarjeta de esta máquina: solo cuentan sus cifras.
    pub tarjeta: &'a str,
    /// Los ficheros de la carpeta de modelos, con su tamaño en bytes.
    pub en_modelos: &'a [(String, u64)],
    /// El nombre, sin extensión, del modelo de `programator.toml`.
    pub modelo_en_uso: Option<&'a str>,
}

/// El mejor de una categoría, o nadie si nadie tiene nota.
#[derive(Debug, Clone, PartialEq)]
pub struct Ganador {
    pub categoria: Categoria,
    pub ganador: Option<Vencedor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vencedor {
    pub nombre: String,
    pub nota: f64,
}

/// Un modelo que gana algo, o el que está en uso aunque no gane nada.
#[derive(Debug, Clone, PartialEq)]
pub struct SeQueda {
    pub nombre: String,
    pub gana: Vec<Categoria>,
    pub en_uso: bool,
}

/// Un modelo que no gana nada, con su mejor nota y quién le gana ahí.
#[derive(Debug, Clone, PartialEq)]
pub struct ParaRetirar {
    pub nombre: String,
    pub mejor_nota: f64,
    pub en: Categoria,
    pub le_gana: Vencedor,
}

/// El modelo en uso cuando no gana nada: se queda, y se dice cuál lo sustituiría.
#[derive(Debug, Clone, PartialEq)]
pub struct EnUsoSinVictorias {
    pub nombre: String,
    pub sustituto: Option<String>,
}

/// De qué tanda sale cada cifra.
#[derive(Debug, Clone, PartialEq)]
pub struct Procedencia {
    pub nombre: String,
    pub tanda: String,
    pub medido_el: String,
}

/// Una categoría que depende del código y no se compara, porque sus notas no se puntuaron igual:
/// unas ejecutando las pruebas ocultas, otras leyendo el código con la rúbrica, o no se sabe.
#[derive(Debug, Clone, PartialEq)]
pub struct Excluida {
    pub categoria: Categoria,
    /// Cada modelo con nota en ella, y cómo se puntuó su código (`None`: no se sabe).
    pub formas: Vec<(String, Option<CodigoPuntuadoCon>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Propuesta {
    pub tarjeta: String,
    pub procedencia: Vec<Procedencia>,
    /// Todas las categorías; las excluidas, sin ganador.
    pub ganadores: Vec<Ganador>,
    /// Las categorías que no se comparan, y por qué.
    pub excluidas: Vec<Excluida>,
    pub se_quedan: Vec<SeQueda>,
    pub para_retirar: Vec<ParaRetirar>,
    /// Modelos con notas solo en categorías excluidas: no se decide nada sobre ellos.
    pub sin_comparar: Vec<String>,
    /// Ficheros de la carpeta de modelos sin cifras en esta tarjeta.
    pub sin_medir: Vec<String>,
    pub en_uso_sin_victorias: Option<EnUsoSinVictorias>,
    /// Hay excluidas y no queda ninguna categoría con ganador: no se propone ninguna sustitución
    /// ni retirada, y se dice por qué.
    pub sin_datos_para_decidir: bool,
}

/// Lo que se compara de un modelo en una categoría.
struct Medida<'a> {
    nombre: &'a str,
    nota: f64,
    inventados: usize,
    tokens_por_segundo: Option<f64>,
    tamano: Option<u64>,
}

pub fn proponer(entrada: &Entrada) -> Propuesta {
    let cuentan: Vec<&EntradaDeModelo> = entrada
        .registro
        .modelo
        .iter()
        .filter(|e| e.tarjeta == entrada.tarjeta && e.retirado_el.is_none())
        .collect();
    let tamanos: BTreeMap<&str, u64> = entrada
        .en_modelos
        .iter()
        .filter_map(|(fichero, tamano)| Some((nombre_sin_extension(fichero)?, *tamano)))
        .collect();
    let excluidas: Vec<Excluida> = categorias()
        .into_iter()
        .filter_map(|categoria| excluida(categoria, &cuentan))
        .collect();
    let esta_excluida = |categoria: Categoria| excluidas.iter().any(|x| x.categoria == categoria);
    let ganadores: Vec<Ganador> = categorias()
        .into_iter()
        .map(|categoria| Ganador {
            categoria,
            ganador: if esta_excluida(categoria) {
                None
            } else {
                ganador_de(categoria, &cuentan, &tamanos)
            },
        })
        .collect();
    let sin_datos_para_decidir =
        !excluidas.is_empty() && ganadores.iter().all(|g| g.ganador.is_none());
    let mut se_quedan = Vec::new();
    let mut para_retirar = Vec::new();
    let mut sin_comparar = Vec::new();
    let mut sin_notas = Vec::new();
    for modelo in &cuentan {
        let en_uso = entrada.modelo_en_uso == Some(modelo.nombre.as_str());
        let gana = victorias_de(&modelo.nombre, &ganadores);
        if !gana.is_empty() || en_uso {
            se_quedan.push(SeQueda {
                nombre: modelo.nombre.clone(),
                gana,
                en_uso,
            });
        } else if let Some(retirar) = para_retirar_a(modelo, &ganadores) {
            para_retirar.push(retirar);
        } else if excluidas
            .iter()
            .any(|x| x.formas.iter().any(|(nombre, _)| *nombre == modelo.nombre))
        {
            sin_comparar.push(modelo.nombre.clone());
        } else {
            sin_notas.push(modelo.nombre.clone());
        }
    }
    // Sin nada que comparar no se propone sustituir al modelo en uso: no gana nada porque nadie
    // gana nada, no porque otro sea mejor.
    let en_uso_sin_victorias = se_quedan
        .iter()
        .find(|q| q.en_uso && q.gana.is_empty())
        .filter(|_| !sin_datos_para_decidir)
        .map(|q| EnUsoSinVictorias {
            nombre: q.nombre.clone(),
            sustituto: sustituto(&se_quedan),
        });
    Propuesta {
        tarjeta: entrada.tarjeta.to_string(),
        procedencia: cuentan
            .iter()
            .map(|e| Procedencia {
                nombre: e.nombre.clone(),
                tanda: e.tanda.clone(),
                medido_el: e.medido_el.clone(),
            })
            .collect(),
        ganadores,
        excluidas,
        se_quedan,
        para_retirar,
        sin_comparar,
        sin_medir: sin_medir(entrada, &sin_notas),
        en_uso_sin_victorias,
        sin_datos_para_decidir,
    }
}

/// Una categoría que depende del código se compara solo si todos los que tienen nota en ella
/// puntuaron su código igual, y de una forma conocida y no mezclada: una nota de pruebas y una de
/// rúbrica no son comparables, y de una mezclada o desconocida no se sabe qué parte es cuál. Con
/// uno solo no hay nada que comparar, y vale. Uno que no entregó código tiene un 0 que vale igual
/// con cualquier forma: se compara con todos y no decide la forma de los demás.
fn excluida(categoria: Categoria, cuentan: &[&EntradaDeModelo]) -> Option<Excluida> {
    if !categoria.depende_del_codigo() {
        return None;
    }
    let formas: Vec<(String, Option<CodigoPuntuadoCon>)> = cuentan
        .iter()
        .filter(|e| medida(e, categoria, None).is_some())
        .map(|e| (e.nombre.clone(), e.codigo_puntuado_con))
        .collect();
    let con_forma: Vec<Option<CodigoPuntuadoCon>> = formas
        .iter()
        .map(|(_, forma)| *forma)
        .filter(|forma| *forma != Some(CodigoPuntuadoCon::SinEntregas))
        .collect();
    let comparables = match con_forma.first() {
        None => true,
        Some(primera) => {
            matches!(
                primera,
                Some(CodigoPuntuadoCon::PruebasOcultas | CodigoPuntuadoCon::Rubrica)
            ) && con_forma.iter().all(|forma| forma == primera)
        }
    };
    (formas.len() > 1 && !comparables).then_some(Excluida { categoria, formas })
}

/// El mejor por nota; si empatan, el que inventó menos; luego el más rápido; luego el más pequeño.
fn ganador_de(
    categoria: Categoria,
    cuentan: &[&EntradaDeModelo],
    tamanos: &BTreeMap<&str, u64>,
) -> Option<Vencedor> {
    cuentan
        .iter()
        .filter_map(|e| medida(e, categoria, tamanos.get(e.nombre.as_str()).copied()))
        .min_by(mejor_primero)
        .map(|m| Vencedor {
            nombre: m.nombre.to_string(),
            nota: m.nota,
        })
}

fn medida<'a>(
    entrada: &'a EntradaDeModelo,
    categoria: Categoria,
    tamano: Option<u64>,
) -> Option<Medida<'a>> {
    let (nota, inventados, tokens_por_segundo) = match categoria {
        Categoria::Clase(fila) => {
            let clase = entrada.clase.iter().find(|c| c.clase == fila)?;
            (clase.nota?, clase.inventados, clase.tokens_por_segundo)
        }
        Categoria::Familia(familia) => (
            nota_de_familia(entrada, familia)?,
            entrada.clase.iter().map(|c| c.inventados).sum(),
            entrada.generando_tokens_por_segundo,
        ),
    };
    Some(Medida {
        nombre: &entrada.nombre,
        nota,
        inventados,
        tokens_por_segundo,
        tamano,
    })
}

fn nota_de_familia(entrada: &EntradaDeModelo, familia: Familia) -> Option<f64> {
    match familia {
        Familia::Oficio => entrada.familias.oficio,
        Familia::Criterio => entrada.familias.criterio,
        Familia::Uso => entrada.familias.uso,
    }
}

/// `Less` si `a` va delante. Lo que falta va detrás; al final, el nombre, para que no dependa del
/// orden del registro.
fn mejor_primero(a: &Medida, b: &Medida) -> Ordering {
    b.nota
        .total_cmp(&a.nota)
        .then(a.inventados.cmp(&b.inventados))
        .then(lo_que_falta_detras(
            a.tokens_por_segundo,
            b.tokens_por_segundo,
            |x, y| y.total_cmp(x),
        ))
        .then(lo_que_falta_detras(a.tamano, b.tamano, |x, y| x.cmp(y)))
        .then(a.nombre.cmp(b.nombre))
}

/// Si están los dos, `orden` dice cuál va delante (`Less`, el de `a`); si falta uno, va detrás.
fn lo_que_falta_detras<T>(
    a: Option<T>,
    b: Option<T>,
    orden: impl Fn(&T, &T) -> Ordering,
) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => orden(&x, &y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn victorias_de(nombre: &str, ganadores: &[Ganador]) -> Vec<Categoria> {
    ganadores
        .iter()
        .filter(|g| g.ganador.as_ref().is_some_and(|v| v.nombre == nombre))
        .map(|g| g.categoria)
        .collect()
}

/// Su mejor nota y quién le gana en esa categoría. Sin ninguna nota, nada: no se propone retirar
/// un modelo sin medir.
fn para_retirar_a(modelo: &EntradaDeModelo, ganadores: &[Ganador]) -> Option<ParaRetirar> {
    // Solo donde alguien gana: en una categoría excluida hay notas pero no ganador, y no sirve
    // para decir quién le gana.
    ganadores
        .iter()
        .filter(|g| g.ganador.is_some())
        .filter_map(|g| {
            let nota = medida(modelo, g.categoria, None)?.nota;
            Some((g, nota))
        })
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .and_then(|(g, nota)| {
            Some(ParaRetirar {
                nombre: modelo.nombre.clone(),
                mejor_nota: nota,
                en: g.categoria,
                le_gana: g.ganador.clone()?,
            })
        })
}

/// El que más categorías gana, de los que se quedan; si empatan, el primero en el registro.
fn sustituto(se_quedan: &[SeQueda]) -> Option<String> {
    se_quedan
        .iter()
        .filter(|q| !q.gana.is_empty())
        .fold(None::<&SeQueda>, |mejor, q| match mejor {
            Some(m) if m.gana.len() >= q.gana.len() => Some(m),
            _ => Some(q),
        })
        .map(|q| q.nombre.clone())
}

/// Los GGUF de la carpeta sin entrada de esta tarjeta, y los que la tienen sin ninguna nota.
fn sin_medir(entrada: &Entrada, sin_notas: &[String]) -> Vec<String> {
    let con_entrada: Vec<&str> = entrada
        .registro
        .modelo
        .iter()
        .filter(|e| e.tarjeta == entrada.tarjeta)
        .map(|e| e.nombre.as_str())
        .collect();
    entrada
        .en_modelos
        .iter()
        .filter_map(|(fichero, _)| {
            let nombre = nombre_sin_extension(fichero)?;
            let medido = con_entrada.contains(&nombre) && !sin_notas.iter().any(|s| s == nombre);
            (!medido).then(|| fichero.clone())
        })
        .collect()
}

/// `modelo.gguf` → `modelo`; lo que no es un GGUF, nada.
fn nombre_sin_extension(fichero: &str) -> Option<&str> {
    let (nombre, extension) = fichero.rsplit_once('.')?;
    extension
        .eq_ignore_ascii_case(EXTENSION_DEL_MODELO)
        .then_some(nombre)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;
    use crate::registro::modelo::{NotaDeClase, NotasDeFamilia};

    const TARJETA: &str = "NVIDIA GeForce RTX 4070 Ti SUPER";

    /// Un modelo con la misma nota en todo, sin inventados, a 10 tokens por segundo, y con su
    /// código puntuado ejecutando las pruebas ocultas, como todos los demás de estas pruebas:
    /// así sus cifras de código se comparan.
    fn modelo(nombre: &str, nota: f64) -> EntradaDeModelo {
        let mut e = entrada_de_ejemplo();
        e.codigo_puntuado_con = Some(CodigoPuntuadoCon::PruebasOcultas);
        e.nombre = nombre.into();
        e.tanda = format!("tanda-{nombre}");
        e.huella_sha256 = format!("huella-{nombre}");
        e.familias = NotasDeFamilia {
            oficio: Some(nota),
            criterio: Some(nota),
            uso: Some(nota),
        };
        e.generando_tokens_por_segundo = Some(10.0);
        e.clase = FILAS_DE_DESEMPENO
            .iter()
            .map(|&fila| NotaDeClase {
                clase: fila,
                nota: Some(nota),
                inventados: 0,
                tokens_por_segundo: Some(10.0),
            })
            .collect();
        e
    }

    fn con_nota_en(mut e: EntradaDeModelo, fila: FilaDeDesempeno, nota: f64) -> EntradaDeModelo {
        for clase in &mut e.clase {
            if clase.clase == fila {
                clase.nota = Some(nota);
            }
        }
        e
    }

    fn proponer_con(
        modelos: Vec<EntradaDeModelo>,
        en_modelos: &[(&str, u64)],
        en_uso: Option<&str>,
    ) -> Propuesta {
        let registro = Registro { modelo: modelos };
        let en_modelos: Vec<(String, u64)> = en_modelos
            .iter()
            .map(|(f, t)| (f.to_string(), *t))
            .collect();
        proponer(&Entrada {
            registro: &registro,
            tarjeta: TARJETA,
            en_modelos: &en_modelos,
            modelo_en_uso: en_uso,
        })
    }

    fn ganador(propuesta: &Propuesta, categoria: Categoria) -> Option<&str> {
        propuesta
            .ganadores
            .iter()
            .find(|g| g.categoria == categoria)
            .and_then(|g| g.ganador.as_ref())
            .map(|v| v.nombre.as_str())
    }

    const FORMAS: Categoria = Categoria::Clase(FilaDeDesempeno::Formas);

    #[test]
    fn hay_seis_clases_y_tres_familias() {
        assert_eq!(categorias().len(), 9);
    }

    #[test]
    fn gana_la_mejor_nota() {
        let propuesta = proponer_con(vec![modelo("a", 2.0), modelo("b", 3.0)], &[], None);
        assert_eq!(ganador(&propuesta, FORMAS), Some("b"));
        assert_eq!(
            ganador(&propuesta, Categoria::Familia(Familia::Uso)),
            Some("b")
        );
    }

    #[test]
    fn si_empatan_gana_el_que_invento_menos() {
        let mut a = modelo("a", 3.0);
        a.clase[0].inventados = 2;
        let propuesta = proponer_con(vec![a, modelo("b", 3.0)], &[], None);
        assert_eq!(ganador(&propuesta, FORMAS), Some("b"));
        // En las familias cuentan los inventados de todas sus clases.
        assert_eq!(
            ganador(&propuesta, Categoria::Familia(Familia::Oficio)),
            Some("b")
        );
    }

    #[test]
    fn si_siguen_empatados_gana_el_mas_rapido() {
        // Por nombre ganaría «lento»: si gana «rapido» es por la velocidad.
        let mut rapido = modelo("rapido", 3.0);
        rapido.clase[0].tokens_por_segundo = Some(20.0);
        let mut lento = modelo("lento", 3.0);
        lento.clase[0].tokens_por_segundo = Some(5.0);
        let propuesta = proponer_con(vec![lento.clone(), rapido.clone()], &[], None);
        assert_eq!(ganador(&propuesta, FORMAS), Some("rapido"));
        lento.clase[0].tokens_por_segundo = None;
        let propuesta = proponer_con(vec![lento, rapido], &[], None);
        assert_eq!(ganador(&propuesta, FORMAS), Some("rapido"));
    }

    #[test]
    fn si_aun_empatan_gana_el_mas_pequeno() {
        let propuesta = proponer_con(
            vec![modelo("a", 3.0), modelo("b", 3.0), modelo("c", 3.0)],
            &[("a.gguf", 300), ("b.gguf", 100)],
            None,
        );
        assert_eq!(ganador(&propuesta, FORMAS), Some("b"));
    }

    #[test]
    fn una_clase_sin_nota_de_nadie_queda_sin_ganador() {
        let mut a = modelo("a", 3.0);
        a.clase[0].nota = None;
        let propuesta = proponer_con(vec![a], &[], None);
        assert_eq!(ganador(&propuesta, FORMAS), None);
        assert_eq!(
            ganador(&propuesta, Categoria::Clase(FilaDeDesempeno::Diagnostico)),
            Some("a")
        );
    }

    #[test]
    fn otra_tarjeta_y_los_retirados_no_cuentan() {
        let mut otra = modelo("otra", 5.0);
        otra.tarjeta = "Otra tarjeta".into();
        let mut retirado = modelo("retirado", 5.0);
        retirado.retirado_el = Some("2026-10-01".into());
        let propuesta = proponer_con(vec![otra, retirado, modelo("a", 1.0)], &[], None);
        assert_eq!(ganador(&propuesta, FORMAS), Some("a"));
        assert_eq!(propuesta.procedencia.len(), 1);
        assert_eq!(propuesta.procedencia[0].tanda, "tanda-a");
    }

    #[test]
    fn se_quedan_los_que_ganan_algo_y_los_demas_se_proponen_con_su_mejor_nota() {
        let fuerte = modelo("fuerte", 4.0);
        let especialista = con_nota_en(
            modelo("especialista", 1.0),
            FilaDeDesempeno::Diagnostico,
            5.0,
        );
        let flojo = con_nota_en(modelo("flojo", 1.0), FilaDeDesempeno::Formas, 3.0);
        let propuesta = proponer_con(vec![fuerte, especialista, flojo], &[], None);
        let quedan: Vec<&str> = propuesta
            .se_quedan
            .iter()
            .map(|q| q.nombre.as_str())
            .collect();
        assert_eq!(quedan, ["fuerte", "especialista"]);
        assert_eq!(
            propuesta.se_quedan[1].gana,
            [Categoria::Clase(FilaDeDesempeno::Diagnostico)]
        );
        assert_eq!(
            propuesta.para_retirar,
            [ParaRetirar {
                nombre: "flojo".into(),
                mejor_nota: 3.0,
                en: FORMAS,
                le_gana: Vencedor {
                    nombre: "fuerte".into(),
                    nota: 4.0
                },
            }]
        );
    }

    #[test]
    fn un_fichero_sin_entrada_sale_sin_medir_y_no_se_propone_retirar() {
        let propuesta = proponer_con(
            vec![modelo("a", 3.0)],
            &[("a.gguf", 1), ("nuevo.gguf", 1), ("notas.txt", 1)],
            None,
        );
        assert_eq!(propuesta.sin_medir, ["nuevo.gguf"]);
        assert!(propuesta.para_retirar.is_empty());
    }

    #[test]
    fn uno_medido_en_otra_tarjeta_aqui_esta_sin_medir() {
        let mut otra = modelo("b", 5.0);
        otra.tarjeta = "Otra tarjeta".into();
        let propuesta = proponer_con(vec![otra], &[("b.gguf", 1)], None);
        assert_eq!(propuesta.sin_medir, ["b.gguf"]);
    }

    #[test]
    fn el_modelo_en_uso_nunca_se_propone_y_si_lo_pierde_todo_se_dice_quien_lo_sustituiria() {
        let propuesta = proponer_con(
            vec![modelo("en-uso", 1.0), modelo("mejor", 4.0)],
            &[],
            Some("en-uso"),
        );
        assert!(propuesta.para_retirar.is_empty());
        assert!(propuesta
            .se_quedan
            .iter()
            .any(|q| q.nombre == "en-uso" && q.en_uso && q.gana.is_empty()));
        assert_eq!(
            propuesta.en_uso_sin_victorias,
            Some(EnUsoSinVictorias {
                nombre: "en-uso".into(),
                sustituto: Some("mejor".into()),
            })
        );
    }

    #[test]
    fn el_modelo_en_uso_que_gana_algo_no_necesita_sustituto() {
        let propuesta = proponer_con(vec![modelo("en-uso", 4.0)], &[], Some("en-uso"));
        assert_eq!(propuesta.en_uso_sin_victorias, None);
    }

    const FUNCION_ACOTADA: Categoria = Categoria::Clase(FilaDeDesempeno::FuncionAcotada);

    fn con_codigo(mut e: EntradaDeModelo, forma: Option<CodigoPuntuadoCon>) -> EntradaDeModelo {
        e.codigo_puntuado_con = forma;
        e
    }

    /// Las categorías cuyas notas salen de encargos de código.
    fn las_de_codigo() -> Vec<Categoria> {
        categorias()
            .into_iter()
            .filter(|c| c.depende_del_codigo())
            .collect()
    }

    /// Sin más notas que las que dependen del código.
    fn solo_con_notas_de_codigo(mut e: EntradaDeModelo) -> EntradaDeModelo {
        e.familias.criterio = None;
        for clase in &mut e.clase {
            if !Categoria::Clase(clase.clase).depende_del_codigo() {
                clase.nota = None;
            }
        }
        e
    }

    fn categorias_excluidas(propuesta: &Propuesta) -> Vec<Categoria> {
        propuesta.excluidas.iter().map(|x| x.categoria).collect()
    }

    #[test]
    fn dependen_del_codigo_el_oficio_el_uso_y_la_funcion_acotada() {
        assert_eq!(
            las_de_codigo(),
            [
                FUNCION_ACOTADA,
                Categoria::Familia(Familia::Oficio),
                Categoria::Familia(Familia::Uso),
            ]
        );
    }

    #[test]
    fn con_la_misma_forma_de_puntuar_el_codigo_se_compara_como_siempre() {
        let rubrica = Some(CodigoPuntuadoCon::Rubrica);
        let propuesta = proponer_con(
            vec![
                con_codigo(modelo("a", 2.0), rubrica),
                con_codigo(modelo("b", 3.0), rubrica),
            ],
            &[],
            None,
        );
        assert!(propuesta.excluidas.is_empty());
        for categoria in las_de_codigo() {
            assert_eq!(ganador(&propuesta, categoria), Some("b"), "{categoria:?}");
        }
        assert!(!propuesta.sin_datos_para_decidir);
    }

    #[test]
    fn con_formas_distintas_las_cifras_de_codigo_se_dejan_fuera_y_se_dice_de_quien() {
        let propuesta = proponer_con(
            vec![
                con_codigo(modelo("a", 2.0), Some(CodigoPuntuadoCon::PruebasOcultas)),
                con_codigo(modelo("b", 3.0), Some(CodigoPuntuadoCon::Rubrica)),
            ],
            &[],
            None,
        );
        assert_eq!(categorias_excluidas(&propuesta), las_de_codigo());
        for categoria in las_de_codigo() {
            assert_eq!(ganador(&propuesta, categoria), None, "{categoria:?}");
        }
        assert_eq!(
            propuesta.excluidas[0].formas,
            [
                ("a".to_string(), Some(CodigoPuntuadoCon::PruebasOcultas)),
                ("b".to_string(), Some(CodigoPuntuadoCon::Rubrica)),
            ]
        );
        // Lo que no depende del código se sigue comparando, y decide.
        assert_eq!(ganador(&propuesta, FORMAS), Some("b"));
        assert!(!propuesta.sin_datos_para_decidir);
        assert_eq!(propuesta.para_retirar[0].nombre, "a");
        assert_ne!(propuesta.para_retirar[0].en, FUNCION_ACOTADA);
    }

    #[test]
    fn una_forma_desconocida_deja_fuera_las_cifras_de_codigo() {
        let propuesta = proponer_con(
            vec![modelo("a", 2.0), con_codigo(modelo("antiguo", 3.0), None)],
            &[],
            None,
        );
        assert_eq!(categorias_excluidas(&propuesta), las_de_codigo());
        assert_eq!(
            ganador(&propuesta, Categoria::Familia(Familia::Oficio)),
            None
        );
    }

    #[test]
    fn uno_que_no_entrego_codigo_se_compara_con_cualquier_forma() {
        let sin_entregas = Some(CodigoPuntuadoCon::SinEntregas);
        for forma in [
            Some(CodigoPuntuadoCon::PruebasOcultas),
            Some(CodigoPuntuadoCon::Rubrica),
            sin_entregas,
        ] {
            let propuesta = proponer_con(
                vec![
                    con_codigo(modelo("nada", 0.0), sin_entregas),
                    con_codigo(modelo("a", 2.0), forma),
                    con_codigo(modelo("b", 3.0), forma),
                ],
                &[],
                None,
            );
            assert!(propuesta.excluidas.is_empty(), "{forma:?}");
            assert_eq!(ganador(&propuesta, FUNCION_ACOTADA), Some("b"), "{forma:?}");
        }
    }

    #[test]
    fn no_entregar_no_hace_comparables_formas_que_no_lo_son() {
        let sin_entregas = Some(CodigoPuntuadoCon::SinEntregas);
        for otras in [
            [
                Some(CodigoPuntuadoCon::PruebasOcultas),
                Some(CodigoPuntuadoCon::Rubrica),
            ],
            [Some(CodigoPuntuadoCon::PruebasOcultas), None],
            [None, None],
            [
                Some(CodigoPuntuadoCon::Mixto),
                Some(CodigoPuntuadoCon::Mixto),
            ],
        ] {
            let propuesta = proponer_con(
                vec![
                    con_codigo(modelo("nada", 0.0), sin_entregas),
                    con_codigo(modelo("a", 2.0), otras[0]),
                    con_codigo(modelo("b", 3.0), otras[1]),
                ],
                &[],
                None,
            );
            assert_eq!(
                categorias_excluidas(&propuesta),
                las_de_codigo(),
                "{otras:?}"
            );
        }
    }

    #[test]
    fn la_propuesta_dice_que_no_entrego_sin_hablar_de_tandas_antiguas() {
        let texto =
            crate::revision::mensajes::forma_del_codigo(Some(CodigoPuntuadoCon::SinEntregas));
        assert!(texto.contains("sin entregar"), "{texto}");
        assert!(!texto.contains("antes del"), "{texto}");
    }

    #[test]
    fn una_forma_mezclada_no_se_compara_ni_con_otra_mezclada() {
        let mixto = Some(CodigoPuntuadoCon::Mixto);
        let propuesta = proponer_con(
            vec![
                con_codigo(modelo("a", 2.0), mixto),
                con_codigo(modelo("b", 3.0), mixto),
            ],
            &[],
            None,
        );
        assert_eq!(categorias_excluidas(&propuesta), las_de_codigo());
    }

    #[test]
    fn uno_solo_no_tiene_con_quien_compararse_y_su_forma_no_importa() {
        let propuesta = proponer_con(vec![con_codigo(modelo("a", 2.0), None)], &[], None);
        assert!(propuesta.excluidas.is_empty());
        assert_eq!(ganador(&propuesta, FUNCION_ACOTADA), Some("a"));
    }

    #[test]
    fn si_todo_lo_que_hay_se_deja_fuera_no_se_propone_nada() {
        let en_uso = solo_con_notas_de_codigo(modelo("en-uso", 1.0));
        let otro = solo_con_notas_de_codigo(con_codigo(
            modelo("otro", 4.0),
            Some(CodigoPuntuadoCon::Rubrica),
        ));
        let propuesta = proponer_con(vec![en_uso, otro], &[], Some("en-uso"));
        assert!(propuesta.sin_datos_para_decidir);
        assert!(propuesta.ganadores.iter().all(|g| g.ganador.is_none()));
        assert_eq!(propuesta.en_uso_sin_victorias, None);
        assert!(propuesta.para_retirar.is_empty());
        assert_eq!(propuesta.sin_comparar, ["otro"]);
        assert!(propuesta.sin_medir.is_empty());
    }
}
