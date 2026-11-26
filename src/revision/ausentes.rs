//! Los modelos del registro que faltan en la carpeta de modelos, y si son útiles.
//!
//! Un ausente es una entrada medida en esta tarjeta, sin retirar, cuyo fichero no está en la
//! carpeta de modelos. No se inventa una regla para él: se le pregunta a `propuesta::proponer`
//! qué haría con él si estuviera, poniendo en la carpeta su fichero con el tamaño que guarda su
//! origen. Puro: no toca disco ni red.

use crate::motor::fichero_de_modelo::con_extension;
use crate::registro::modelo::{EntradaDeModelo, Origen, Registro};

use super::propuesta::{proponer, tiene_fichero, Categoria, Entrada, ParaRetirar, Propuesta};

/// El tamaño que se pone al fichero de un ausente cuyo origen no lo guarda. El mayor posible: en
/// el desempate por tamaño va detrás de cualquier tamaño real y, entre dos desconocidos, decide
/// el nombre, como hace la propuesta con un tamaño que falta. Con 0 ganaría todos los desempates.
const TAMANO_DESCONOCIDO: u64 = u64::MAX;

/// Los ausentes de una tarjeta, repartidos. Cada lista va en el orden del registro.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ausentes {
    /// Ganaría algo, o es el modelo en uso: se repone.
    pub utiles: Vec<Util>,
    /// No ganaría nada y alguien le gana donde tiene nota: sale del registro.
    pub no_utiles: Vec<ParaRetirar>,
    /// Notas solo en categorías que no se comparan, o ninguna: ni se repone ni se retira.
    pub sin_decidir: Vec<String>,
}

/// Un ausente que se repone.
#[derive(Debug, Clone, PartialEq)]
pub struct Util {
    pub nombre: String,
    /// Las categorías que ganaría. Vacío si solo es útil por ser el modelo en uso.
    pub gana: Vec<Categoria>,
    pub en_uso: bool,
    /// La del fichero que se midió: la que tiene que tener el repuesto.
    pub huella: String,
    /// De dónde reponerlo, si se sabe.
    pub origen: Option<Origen>,
}

/// Clasifica los ausentes de `tarjeta`. `en_modelos` son los ficheros de la carpeta de modelos,
/// con su tamaño en bytes; `modelo_en_uso`, el nombre sin extensión del modelo de la
/// configuración. El modelo en uso que falta es útil aunque no gane nada, también cuando no hay
/// datos para decidir: la propuesta nunca lo retira.
pub fn clasificar(
    registro: &Registro,
    tarjeta: &str,
    en_modelos: &[(String, u64)],
    modelo_en_uso: Option<&str>,
) -> Ausentes {
    let ausentes = ausentes_de(registro, tarjeta, en_modelos);
    if ausentes.is_empty() {
        return Ausentes::default();
    }
    let como_si_estuvieran = con_los_ausentes(en_modelos, &ausentes);
    let propuesta = proponer(&Entrada {
        registro,
        tarjeta,
        en_modelos: &como_si_estuvieran,
        modelo_en_uso,
    });
    repartir(&ausentes, &propuesta)
}

/// Las entradas de esta tarjeta, sin retirar y sin fichero, sin repetir nombre.
fn ausentes_de<'a>(
    registro: &'a Registro,
    tarjeta: &str,
    en_modelos: &[(String, u64)],
) -> Vec<&'a EntradaDeModelo> {
    let mut ausentes: Vec<&EntradaDeModelo> = Vec::new();
    for entrada in registro.modelo.iter().filter(|e| {
        e.tarjeta == tarjeta && e.retirado_el.is_none() && !tiene_fichero(&e.nombre, en_modelos)
    }) {
        if !ausentes.iter().any(|a| a.nombre == entrada.nombre) {
            ausentes.push(entrada);
        }
    }
    ausentes
}

/// La carpeta de modelos con el fichero de cada ausente, del tamaño que guarda su origen.
fn con_los_ausentes(
    en_modelos: &[(String, u64)],
    ausentes: &[&EntradaDeModelo],
) -> Vec<(String, u64)> {
    let mut todos = en_modelos.to_vec();
    todos.extend(ausentes.iter().map(|a| {
        let tamano = a
            .origen
            .as_ref()
            .and_then(|o| o.tamano)
            .unwrap_or(TAMANO_DESCONOCIDO);
        (con_extension(&a.nombre), tamano)
    }));
    todos
}

fn repartir(ausentes: &[&EntradaDeModelo], propuesta: &Propuesta) -> Ausentes {
    let mut repartidos = Ausentes::default();
    for ausente in ausentes {
        if let Some(queda) = propuesta
            .se_quedan
            .iter()
            .find(|q| q.nombre == ausente.nombre)
        {
            repartidos.utiles.push(Util {
                nombre: queda.nombre.clone(),
                gana: queda.gana.clone(),
                en_uso: queda.en_uso,
                huella: ausente.huella_sha256.clone(),
                origen: ausente.origen.clone(),
            });
        } else if let Some(retirar) = propuesta
            .para_retirar
            .iter()
            .find(|r| r.nombre == ausente.nombre)
        {
            repartidos.no_utiles.push(retirar.clone());
        } else {
            repartidos.sin_decidir.push(ausente.nombre.clone());
        }
    }
    repartidos
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::{FilaDeDesempeno, FILAS_DE_DESEMPENO};
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;
    use crate::registro::modelo::{CodigoPuntuadoCon, NotaDeClase, NotasDeFamilia};

    use crate::revision::pruebas_comunes::TARJETA;
    const DIAGNOSTICO: Categoria = Categoria::Clase(FilaDeDesempeno::Diagnostico);

    /// Un modelo de esta tarjeta con la misma nota en todo, sin inventados, a 10 tokens por
    /// segundo, con su código puntuado con las pruebas ocultas y sin origen.
    fn modelo(nombre: &str, nota: f64) -> EntradaDeModelo {
        let mut e = entrada_de_ejemplo();
        e.nombre = nombre.into();
        e.tarjeta = TARJETA.into();
        e.tanda = format!("tanda-{nombre}");
        e.huella_sha256 = format!("huella-{nombre}");
        e.codigo_puntuado_con = Some(CodigoPuntuadoCon::PruebasOcultas);
        e.familias = NotasDeFamilia {
            oficio: Some(nota),
            criterio: Some(nota),
            uso: Some(nota),
        };
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

    fn con_origen(mut e: EntradaDeModelo, tamano: Option<u64>) -> EntradaDeModelo {
        e.origen = Some(Origen {
            repositorio: format!("alguien/{}-GGUF", e.nombre),
            fichero: format!("{}.gguf", e.nombre),
            tamano,
        });
        e
    }

    fn sin_notas(mut e: EntradaDeModelo) -> EntradaDeModelo {
        e.familias = NotasDeFamilia {
            oficio: None,
            criterio: None,
            uso: None,
        };
        for clase in &mut e.clase {
            clase.nota = None;
        }
        e
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

    fn clasificar_con(
        modelos: Vec<EntradaDeModelo>,
        presentes: &[(&str, u64)],
        en_uso: Option<&str>,
    ) -> Ausentes {
        let presentes: Vec<(String, u64)> = presentes
            .iter()
            .map(|(nombre, tamano)| (format!("{nombre}.gguf"), *tamano))
            .collect();
        clasificar(&Registro { modelo: modelos }, TARJETA, &presentes, en_uso)
    }

    fn nombres_de_los_utiles(ausentes: &Ausentes) -> Vec<&str> {
        ausentes.utiles.iter().map(|u| u.nombre.as_str()).collect()
    }

    fn nombres_de_los_no_utiles(ausentes: &Ausentes) -> Vec<&str> {
        ausentes
            .no_utiles
            .iter()
            .map(|r| r.nombre.as_str())
            .collect()
    }

    #[test]
    fn un_ausente_que_ganaria_una_clase_es_util_y_dice_cual() {
        let mut b = con_origen(modelo("b", 1.0), Some(100));
        for clase in b
            .clase
            .iter_mut()
            .filter(|c| c.clase == FilaDeDesempeno::Diagnostico)
        {
            clase.nota = Some(4.5);
        }
        let origen = b.origen.clone();
        let ausentes = clasificar_con(vec![modelo("a", 3.0), b], &[("a", 100)], None);
        assert_eq!(
            ausentes.utiles,
            [Util {
                nombre: "b".into(),
                gana: vec![DIAGNOSTICO],
                en_uso: false,
                huella: "huella-b".into(),
                origen,
            }]
        );
        assert!(ausentes.no_utiles.is_empty());
        assert!(ausentes.sin_decidir.is_empty());
    }

    #[test]
    fn el_modelo_en_uso_ausente_es_util_aunque_no_gane_nada() {
        let ausentes = clasificar_con(
            vec![modelo("a", 4.0), modelo("b", 2.0)],
            &[("a", 100)],
            Some("b"),
        );
        assert_eq!(nombres_de_los_utiles(&ausentes), ["b"]);
        assert!(ausentes.utiles[0].en_uso);
        assert!(ausentes.utiles[0].gana.is_empty());
        assert!(ausentes.no_utiles.is_empty());
    }

    #[test]
    fn un_ausente_que_no_gana_nada_no_es_util() {
        let ausentes = clasificar_con(
            vec![modelo("a", 4.0), modelo("b", 2.0)],
            &[("a", 100)],
            None,
        );
        assert!(ausentes.utiles.is_empty());
        assert_eq!(nombres_de_los_no_utiles(&ausentes), ["b"]);
        assert_eq!(ausentes.no_utiles[0].le_gana.nombre, "a");
        assert!(ausentes.sin_decidir.is_empty());
    }

    #[test]
    fn un_ausente_sin_notas_queda_sin_decidir() {
        let ausentes = clasificar_con(
            vec![modelo("a", 4.0), sin_notas(modelo("b", 2.0))],
            &[("a", 100)],
            None,
        );
        assert!(ausentes.utiles.is_empty());
        assert!(ausentes.no_utiles.is_empty());
        assert_eq!(ausentes.sin_decidir, ["b"]);
    }

    #[test]
    fn sin_datos_para_decidir_todos_quedan_sin_decidir() {
        // Dos formas de puntuar el código que no se comparan, y ninguna otra nota: nadie gana
        // nada, así que nadie es útil ni sobra.
        let a = solo_con_notas_de_codigo(modelo("a", 4.0));
        let mut b = solo_con_notas_de_codigo(modelo("b", 2.0));
        b.codigo_puntuado_con = Some(CodigoPuntuadoCon::Rubrica);
        let ausentes = clasificar_con(vec![a, b], &[], None);
        assert!(ausentes.utiles.is_empty());
        assert!(ausentes.no_utiles.is_empty());
        assert_eq!(ausentes.sin_decidir, ["a", "b"]);
    }

    #[test]
    fn el_tamano_del_origen_cuenta_en_el_desempate() {
        // Empatan en todo menos en el tamaño: gana el más pequeño, aunque por el nombre iría
        // detrás.
        let ausentes = clasificar_con(
            vec![modelo("a", 3.0), con_origen(modelo("b", 3.0), Some(100))],
            &[("a", 200)],
            None,
        );
        assert_eq!(nombres_de_los_utiles(&ausentes), ["b"]);

        // Sin tamaño conocido va detrás de cualquier tamaño real, aunque por el nombre iría
        // delante.
        let ausentes = clasificar_con(
            vec![con_origen(modelo("a", 3.0), None), modelo("b", 3.0)],
            &[("b", 200)],
            None,
        );
        assert!(ausentes.utiles.is_empty());
        assert_eq!(nombres_de_los_no_utiles(&ausentes), ["a"]);
    }

    #[test]
    fn los_retirados_y_los_de_otra_tarjeta_no_son_ausentes() {
        let mut retirado = modelo("b", 5.0);
        retirado.retirado_el = Some("2026-11-13".into());
        let mut de_otra = modelo("c", 5.0);
        de_otra.tarjeta = "otra".into();
        let ausentes = clasificar_con(
            vec![modelo("a", 1.0), retirado, de_otra],
            &[("a", 100)],
            None,
        );
        assert_eq!(ausentes, Ausentes::default());
    }

    /// Dos tandas del mismo modelo son dos entradas con el mismo nombre: sale una sola vez.
    #[test]
    fn un_ausente_con_dos_entradas_sale_una_vez() {
        let mut otra_tanda = modelo("b", 4.0);
        otra_tanda.tanda = "otra".into();
        let ausentes = clasificar_con(
            vec![modelo("a", 3.0), modelo("b", 4.0), otra_tanda],
            &[("a", 100)],
            None,
        );
        assert_eq!(nombres_de_los_utiles(&ausentes), ["b"]);
    }
}
