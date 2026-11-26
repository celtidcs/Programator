//! Qué modelo atiende un encargo, según la clase que pide su línea `Clase:`.
//!
//! Puro: recibe el registro, la tarjeta, los ficheros de la carpeta de modelos y la clase pedida,
//! y no toca disco ni GPU. Usa la misma regla que la revisión (`propuesta::ganadores`), para que
//! el modelo que se carga para una clase sea el que la revisión dice que la gana.

use crate::arnes::clase_pedida::ClasePedida;
use crate::banco::resultados::FilaDeDesempeno;
use crate::registro::modelo::{EntradaDeModelo, Registro};

use super::propuesta::{ganadores, tiene_fichero, Categoria, Entrada};

/// Lo que se decidió y por qué.
#[derive(Debug, Clone, PartialEq)]
pub struct Eleccion {
    /// El nombre del modelo, sin extensión. `None`: el modelo por defecto de la configuración.
    pub modelo: Option<String>,
    pub motivo: Motivo,
}

/// Por qué se eligió ese modelo.
#[derive(Debug, Clone, PartialEq)]
pub enum Motivo {
    /// El encargo no trae línea `Clase:`.
    SinClase,
    /// Trae una clase que no existe; se guarda lo que escribió.
    ClaseDesconocida(String),
    /// No se pudo medir la tarjeta, así que no se sabe qué cifras del registro valen aquí.
    SinTarjeta,
    /// Ningún modelo aprobado tiene nota comparable en esa clase.
    SinGanador(FilaDeDesempeno),
    /// El que mejor la hace, con su nota.
    Ganador { fila: FilaDeDesempeno, nota: f64 },
}

/// Elige el modelo de un encargo.
///
/// Solo cuentan los modelos aprobados: los del registro medidos en esta tarjeta, sin retirar y
/// con su fichero en la carpeta de modelos (`en_modelos`, con su tamaño en bytes).
pub fn elegir_modelo(
    registro: &Registro,
    clase: &ClasePedida,
    tarjeta: Option<&str>,
    en_modelos: &[(String, u64)],
) -> Eleccion {
    let fila = match clase {
        ClasePedida::Ninguna => return por_defecto(Motivo::SinClase),
        ClasePedida::Desconocida(texto) => {
            return por_defecto(Motivo::ClaseDesconocida(texto.clone()))
        }
        ClasePedida::Valida(fila) => *fila,
    };
    let Some(tarjeta) = tarjeta else {
        return por_defecto(Motivo::SinTarjeta);
    };
    // `ganadores` ya deja fuera a los que no tienen fichero: no se pueden cargar.
    let (ganadores, _) = ganadores(&Entrada {
        registro,
        tarjeta,
        en_modelos,
        modelo_en_uso: None,
    });
    let vencedor = ganadores
        .into_iter()
        .find(|g| g.categoria == Categoria::Clase(fila))
        .and_then(|g| g.ganador);
    match vencedor {
        Some(v) => Eleccion {
            modelo: Some(v.nombre),
            motivo: Motivo::Ganador { fila, nota: v.nota },
        },
        None => por_defecto(Motivo::SinGanador(fila)),
    }
}

fn por_defecto(motivo: Motivo) -> Eleccion {
    Eleccion {
        modelo: None,
        motivo,
    }
}

/// Si una entrada cuenta como aprobada para elegir: medida en esta tarjeta y con su fichero en la
/// carpeta de modelos. La retirada la mira quien llama.
pub fn es_elegible(entrada: &EntradaDeModelo, tarjeta: &str, en_modelos: &[(String, u64)]) -> bool {
    entrada.tarjeta == tarjeta && tiene_fichero(&entrada.nombre, en_modelos)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::FILAS_DE_DESEMPENO;
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;
    use crate::registro::modelo::{CodigoPuntuadoCon, EntradaDeModelo, NotaDeClase};

    use crate::revision::pruebas_comunes::TARJETA;
    const FILA: FilaDeDesempeno = FilaDeDesempeno::Diagnostico;

    /// Un modelo de esta tarjeta con `nota` en todas las clases, sin inventados, a 10 tokens por
    /// segundo y con el código puntuado igual que los demás.
    fn modelo(nombre: &str, nota: f64) -> EntradaDeModelo {
        let mut e = entrada_de_ejemplo();
        e.nombre = nombre.into();
        e.tarjeta = TARJETA.into();
        e.retirado_el = None;
        e.codigo_puntuado_con = Some(CodigoPuntuadoCon::PruebasOcultas);
        e.clase = FILAS_DE_DESEMPENO
            .iter()
            .map(|&clase| NotaDeClase {
                clase,
                nota: Some(nota),
                inventados: 0,
                tokens_por_segundo: Some(10.0),
            })
            .collect();
        e
    }

    fn en_fila(mut e: EntradaDeModelo, cambiar: impl Fn(&mut NotaDeClase)) -> EntradaDeModelo {
        for clase in e.clase.iter_mut().filter(|c| c.clase == FILA) {
            cambiar(clase);
        }
        e
    }

    fn ficheros(nombres: &[&str]) -> Vec<(String, u64)> {
        nombres.iter().map(|n| (format!("{n}.gguf"), 1)).collect()
    }

    fn elegir(modelos: Vec<EntradaDeModelo>, presentes: &[&str]) -> Eleccion {
        elegir_modelo(
            &Registro { modelo: modelos },
            &ClasePedida::Valida(FILA),
            Some(TARJETA),
            &ficheros(presentes),
        )
    }

    #[test]
    fn gana_la_mejor_nota_de_la_clase() {
        let eleccion = elegir(
            vec![
                modelo("a", 3.0),
                en_fila(modelo("b", 1.0), |c| c.nota = Some(4.5)),
            ],
            &["a", "b"],
        );
        assert_eq!(eleccion.modelo.as_deref(), Some("b"));
        assert_eq!(
            eleccion.motivo,
            Motivo::Ganador {
                fila: FILA,
                nota: 4.5
            }
        );
    }

    #[test]
    fn con_la_misma_nota_gana_el_que_inventa_menos_y_luego_el_mas_rapido() {
        let menos_inventados = elegir(
            vec![
                en_fila(modelo("a", 3.0), |c| c.inventados = 2),
                modelo("b", 3.0),
            ],
            &["a", "b"],
        );
        assert_eq!(menos_inventados.modelo.as_deref(), Some("b"));

        let mas_rapido = elegir(
            vec![
                modelo("a", 3.0),
                en_fila(modelo("b", 3.0), |c| c.tokens_por_segundo = Some(20.0)),
            ],
            &["a", "b"],
        );
        assert_eq!(mas_rapido.modelo.as_deref(), Some("b"));
    }

    #[test]
    fn un_retirado_no_se_elige() {
        let mut retirado = modelo("b", 5.0);
        retirado.retirado_el = Some("2026-11-13".into());
        let eleccion = elegir(vec![modelo("a", 3.0), retirado], &["a", "b"]);
        assert_eq!(eleccion.modelo.as_deref(), Some("a"));
    }

    #[test]
    fn un_modelo_sin_fichero_no_se_elige() {
        let eleccion = elegir(vec![modelo("a", 3.0), modelo("b", 5.0)], &["a"]);
        assert_eq!(eleccion.modelo.as_deref(), Some("a"));
    }

    #[test]
    fn las_cifras_de_otra_tarjeta_no_cuentan() {
        let mut de_otra = modelo("b", 5.0);
        de_otra.tarjeta = "otra".into();
        let eleccion = elegir(vec![modelo("a", 3.0), de_otra], &["a", "b"]);
        assert_eq!(eleccion.modelo.as_deref(), Some("a"));
    }

    #[test]
    fn sin_notas_en_la_clase_va_el_de_por_defecto() {
        let eleccion = elegir(vec![en_fila(modelo("a", 3.0), |c| c.nota = None)], &["a"]);
        assert_eq!(eleccion, por_defecto(Motivo::SinGanador(FILA)));
    }

    #[test]
    fn con_el_registro_vacio_va_el_de_por_defecto() {
        assert_eq!(elegir(vec![], &[]), por_defecto(Motivo::SinGanador(FILA)));
    }

    #[test]
    fn sin_clase_con_clase_desconocida_o_sin_tarjeta_va_el_de_por_defecto() {
        let registro = Registro {
            modelo: vec![modelo("a", 5.0)],
        };
        let presentes = ficheros(&["a"]);
        assert_eq!(
            elegir_modelo(&registro, &ClasePedida::Ninguna, Some(TARJETA), &presentes),
            por_defecto(Motivo::SinClase)
        );
        assert_eq!(
            elegir_modelo(
                &registro,
                &ClasePedida::Desconocida("traducir".into()),
                Some(TARJETA),
                &presentes
            ),
            por_defecto(Motivo::ClaseDesconocida("traducir".into()))
        );
        assert_eq!(
            elegir_modelo(&registro, &ClasePedida::Valida(FILA), None, &presentes),
            por_defecto(Motivo::SinTarjeta)
        );
    }

    #[test]
    fn solo_es_elegible_lo_medido_en_esta_tarjeta_y_con_su_fichero() {
        let entrada = entrada_de_ejemplo();
        let fichero = vec![(format!("{}.gguf", entrada.nombre), 1)];
        let tarjeta = entrada.tarjeta.clone();

        assert!(es_elegible(&entrada, &tarjeta, &fichero));
        assert!(!es_elegible(&entrada, "otra tarjeta", &fichero));
        assert!(!es_elegible(&entrada, &tarjeta, &[("otro.gguf".into(), 1)]));
        assert!(!es_elegible(&entrada, &tarjeta, &[]));
    }
}
