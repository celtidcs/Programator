//! Qué se le ha pedido a Programator en la línea de órdenes.
//!
//! Función pura sobre un vector de cadenas: no lee el entorno ni escribe en pantalla, para que
//! todas las combinaciones se puedan probar sin lanzar el programa.

use std::collections::BTreeMap;

mod ayuda;
pub use ayuda::texto_de_ayuda;
#[cfg(test)]
use ayuda::{linea_de_uso, COLUMNA_DE_LA_DESCRIPCION};

/// Lo que el usuario ha pedido al invocar el ejecutable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Orden {
    /// Arrancar y entrar en el ciclo de trabajo, sobre la carpeta pedida si se pidió alguna.
    Ejecutar { ruta: Option<String> },
    /// Escribir el texto de ayuda y terminar, sin arrancar el ciclo.
    Ayuda,
    /// Escribir la versión del programa y terminar, sin arrancar el ciclo.
    Version,
    /// Escribir el estado de la máquina, el motor y el modelo, y terminar sin arrancar el ciclo.
    Diagnostico {
        /// Otro modelo en lugar del de la configuración, solo en esta ejecución.
        modelo: Option<String>,
    },
    /// Un argumento que no se reconoce: el primero de la lista, que es el único que se mira, para
    /// poder nombrarlo.
    NoEntendido(String),
    /// Pasar una tanda del banco de modelos con los encargos de esa carpeta.
    Banco {
        carpeta: String,
        /// Otro modelo en lugar del de la configuración, solo en esta tanda.
        modelo: Option<String>,
    },
    /// Hacer solo las pruebas ocultas de una tanda cuya fase con el modelo quedó en disco
    /// (`pasadas.toml`), sin volver a cargarlo.
    BancoComprobar { tanda: String },
    /// Rehacer el informe de una tanda ya pasada, con las rúbricas que se hayan puntuado.
    BancoInforme { resultados: String },
    /// Comparar dos tandas guardadas para saber si una es repetición de la otra.
    BancoComparar { tanda_a: String, tanda_b: String },
    /// Anotar en el registro de modelos lo que midió una tanda terminada.
    RegistroAnotar { tanda: String },
    /// Buscar en Hugging Face los modelos candidatos de la revisión mensual, y dejarlos en la
    /// carpeta del mes.
    RevisionBuscar,
    /// Comparar el registro clase a clase y escribir qué modelos se quedan y cuáles se proponen
    /// para retirar.
    RevisionProponer,
    /// Anotar en el registro que se retira un modelo y borrar su fichero de la carpeta de modelos.
    RetirarModelo { fichero: String },
    /// Descargar un fichero de la última búsqueda a la carpeta de modelos, comprobando su huella.
    DescargarModelo {
        repositorio: String,
        fichero: String,
    },
    /// Hacer de ayudante del aislamiento de Windows: lanzar `orden` con integridad baja. Lo usa la
    /// comprobación local y no sale en la ayuda (`ayudante_aislado`).
    LanzarAislado { orden: Vec<String> },
    /// Una opción que necesita un valor y no lo trae detrás.
    ///
    /// Existe aparte de `NoEntendido` porque el remedio es distinto: ahí falta entender qué se
    /// pedía, y aquí se entendió perfectamente y falta el dato.
    FaltaValor(String),
}

/// Interpreta los argumentos, **sin** el nombre del ejecutable.
///
/// Un argumento desconocido no arranca el ciclo: un proceso que va a correr desatendido durante
/// días no debe ponerse en marcha porque no se entendió lo que se le pedía.
///
/// El primer argumento reconocible o irreconocible decide: se mira solo el primero, sin mirar
/// por delante. Así, tanto la ayuda como el primer desconocido ganan si van primero, y solo
/// entonces.
pub fn interpretar(argumentos: &[String]) -> Orden {
    match argumentos.first() {
        None => Orden::Ejecutar { ruta: None },
        // La ayuda gana a lo que venga detrás: quien pregunta, pregunta. Pero solo si es ella
        // la que aparece primero.
        Some(argumento) => match argumento.as_str() {
            OPCION_AYUDA | OPCION_AYUDA_CORTA => Orden::Ayuda,
            OPCION_VERSION => sin_sobrantes(argumentos, 1, Orden::Version),
            OPCION_DIAGNOSTICO => match opciones_con_valor(&argumentos[1..], &[OPCION_MODELO]) {
                Ok(mut leidas) => Orden::Diagnostico {
                    modelo: leidas.remove(OPCION_MODELO),
                },
                Err(orden) => orden,
            },
            // El valor va detrás, y tiene que parecer una carpeta: cualquier cosa que empiece por
            // «--» es otra opción, no la carpeta que alguien olvidó escribir.
            OPCION_RUTA => match argumentos.get(1) {
                Some(valor) if !valor.starts_with(PREFIJO_DE_OPCION) => sin_sobrantes(
                    argumentos,
                    2,
                    Orden::Ejecutar {
                        ruta: Some(valor.clone()),
                    },
                ),
                _ => Orden::FaltaValor(OPCION_RUTA.to_string()),
            },
            OPCION_BANCO => match valor_tras(argumentos, 0) {
                Some(carpeta) => match opciones_con_valor(&argumentos[2..], &[OPCION_MODELO]) {
                    Ok(mut leidas) => Orden::Banco {
                        carpeta: carpeta.clone(),
                        modelo: leidas.remove(OPCION_MODELO),
                    },
                    Err(orden) => orden,
                },
                None => Orden::FaltaValor(OPCION_BANCO.to_string()),
            },
            OPCION_BANCO_COMPROBAR => match valor_tras(argumentos, 0) {
                Some(tanda) => match argumentos.get(2) {
                    None => Orden::BancoComprobar {
                        tanda: tanda.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                None => Orden::FaltaValor(OPCION_BANCO_COMPROBAR.to_string()),
            },
            OPCION_BANCO_INFORME => match valor_tras(argumentos, 0) {
                Some(resultados) => sin_sobrantes(
                    argumentos,
                    2,
                    Orden::BancoInforme {
                        resultados: resultados.clone(),
                    },
                ),
                None => Orden::FaltaValor(OPCION_BANCO_INFORME.to_string()),
            },
            OPCION_BANCO_COMPARAR => match (valor_tras(argumentos, 0), valor_tras(argumentos, 1)) {
                (Some(tanda_a), Some(tanda_b)) => match argumentos.get(3) {
                    None => Orden::BancoComparar {
                        tanda_a: tanda_a.clone(),
                        tanda_b: tanda_b.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                _ => Orden::FaltaValor(OPCION_BANCO_COMPARAR.to_string()),
            },
            OPCION_REGISTRO_ANOTAR => match valor_tras(argumentos, 0) {
                Some(tanda) => match argumentos.get(2) {
                    None => Orden::RegistroAnotar {
                        tanda: tanda.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                None => Orden::FaltaValor(OPCION_REGISTRO_ANOTAR.to_string()),
            },
            OPCION_REVISION_BUSCAR => match argumentos.get(1) {
                None => Orden::RevisionBuscar,
                Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
            },
            OPCION_REVISION_PROPONER => match argumentos.get(1) {
                None => Orden::RevisionProponer,
                Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
            },
            OPCION_RETIRAR_MODELO => match valor_tras(argumentos, 0) {
                Some(fichero) => match argumentos.get(2) {
                    None => Orden::RetirarModelo {
                        fichero: fichero.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                None => Orden::FaltaValor(OPCION_RETIRAR_MODELO.to_string()),
            },
            OPCION_DESCARGAR_MODELO => match (valor_tras(argumentos, 0), valor_tras(argumentos, 1))
            {
                (Some(repositorio), Some(fichero)) => match argumentos.get(3) {
                    None => Orden::DescargarModelo {
                        repositorio: repositorio.clone(),
                        fichero: fichero.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                _ => Orden::FaltaValor(OPCION_DESCARGAR_MODELO.to_string()),
            },
            OPCION_LANZAR_AISLADO => match argumentos.get(1).map(String::as_str) {
                Some(SEPARADOR_DE_LA_ORDEN) if argumentos.len() > 2 => Orden::LanzarAislado {
                    orden: argumentos[2..].to_vec(),
                },
                _ => Orden::FaltaValor(OPCION_LANZAR_AISLADO.to_string()),
            },
            otro => Orden::NoEntendido(otro.to_string()),
        },
    }
}

/// La orden oculta del ayudante del aislamiento de Windows.
const OPCION_LANZAR_AISLADO: &str = "--lanzar-aislado";
/// Lo que separa la opción de la orden que se lanza: lo que va detrás no son opciones de
/// Programator, aunque empiecen por «--».
const SEPARADOR_DE_LA_ORDEN: &str = "--";

/// Los argumentos con que Programator se lanza a sí mismo como ayudante para ejecutar `orden`: lo
/// contrario de lo que [`interpretar`] reconoce como [`Orden::LanzarAislado`].
pub fn orden_del_ayudante(ejecutable: &str, orden: &[String]) -> Vec<String> {
    [ejecutable, OPCION_LANZAR_AISLADO, SEPARADOR_DE_LA_ORDEN]
        .into_iter()
        .map(String::from)
        .chain(orden.iter().cloned())
        .collect()
}

/// La opción que cambia el modelo solo en esta ejecución, detrás de `--banco` y de `--diagnostico`.
/// La comprobación aparte no la admite: no carga ningún modelo.
pub const OPCION_MODELO: &str = "--modelo";

/// Lo que distingue una opción de un valor: una carpeta o un fichero no empiezan así.
const PREFIJO_DE_OPCION: &str = "--";

// Las opciones de la línea de órdenes, escritas una vez. Las usan el análisis, la ayuda y los
// mensajes que enseñan a usarlas, para que renombrar una no deje textos con el nombre viejo.
/// Muestra la ayuda.
pub const OPCION_AYUDA: &str = "--ayuda";
/// La forma corta de la ayuda, la de casi todas las herramientas.
pub const OPCION_AYUDA_CORTA: &str = "-h";
/// Escribe la versión.
pub const OPCION_VERSION: &str = "--version";
/// El estado de la máquina, el motor y el modelo, sin arrancar el ciclo.
pub const OPCION_DIAGNOSTICO: &str = "--diagnostico";
/// La carpeta de trabajo, que manda sobre la del TOML.
pub const OPCION_RUTA: &str = "--ruta";
/// Pasa una tanda del banco de modelos.
pub const OPCION_BANCO: &str = "--banco";
/// Solo las pruebas ocultas de una tanda ya pasada.
pub const OPCION_BANCO_COMPROBAR: &str = "--banco-comprobar";
/// Rehace el informe de una tanda.
pub const OPCION_BANCO_INFORME: &str = "--banco-informe";
/// Compara dos tandas guardadas.
pub const OPCION_BANCO_COMPARAR: &str = "--banco-comparar";
/// Anota una tanda en el registro de modelos.
pub const OPCION_REGISTRO_ANOTAR: &str = "--registro-anotar";
/// Busca candidatos en Hugging Face.
pub const OPCION_REVISION_BUSCAR: &str = "--revision-buscar";
/// Propone qué modelos se quedan y cuáles se retiran.
pub const OPCION_REVISION_PROPONER: &str = "--revision-proponer";
/// Retira un modelo.
pub const OPCION_RETIRAR_MODELO: &str = "--retirar-modelo";
/// Descarga un candidato de la última búsqueda.
pub const OPCION_DESCARGAR_MODELO: &str = "--descargar-modelo";

/// `orden`, si no queda nada desde `desde`; si queda algo, no se entiende. Una orden no ignora en
/// silencio lo que sobra: si no, `--ruta X --banco-comprobar T` arrancaría el ciclo sobre X y se
/// olvidaría de la comprobación.
fn sin_sobrantes(argumentos: &[String], desde: usize, orden: Orden) -> Orden {
    match argumentos.get(desde) {
        Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
        None => orden,
    }
}

/// El valor que va detrás de la opción en `posicion`, si lo hay y no es otra opción.
fn valor_tras(argumentos: &[String], posicion: usize) -> Option<&String> {
    argumentos
        .get(posicion + 1)
        .filter(|valor| !valor.starts_with(PREFIJO_DE_OPCION))
}

/// Las opciones de `admitidas` que trae `resto`, cada una con su valor detrás, en cualquier orden.
///
/// Nada sobra sin decirlo: lo primero que no sea una opción admitida, o una que se repita, es la
/// orden [`Orden::NoEntendido`] con ello; una opción sin valor detrás, [`Orden::FaltaValor`].
fn opciones_con_valor(
    resto: &[String],
    admitidas: &[&'static str],
) -> Result<BTreeMap<&'static str, String>, Orden> {
    let mut leidas = BTreeMap::new();
    let mut posicion = 0;
    while let Some(argumento) = resto.get(posicion) {
        let opcion = admitidas
            .iter()
            .find(|admitida| **admitida == argumento.as_str())
            .filter(|admitida| !leidas.contains_key(**admitida))
            .ok_or_else(|| Orden::NoEntendido(argumento.clone()))?;
        let valor =
            valor_tras(resto, posicion).ok_or_else(|| Orden::FaltaValor(opcion.to_string()))?;
        leidas.insert(*opcion, valor.clone());
        posicion += 2;
    }
    Ok(leidas)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn argumentos(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Lo que sobra detrás de estas órdenes no se ignora en silencio.
    #[test]
    fn lo_que_sobra_tras_una_orden_completa_no_se_entiende() {
        for (orden, sobrante) in [
            (vec![OPCION_VERSION, "extra"], "extra"),
            (
                vec![OPCION_RUTA, "carpeta", OPCION_BANCO_COMPROBAR],
                OPCION_BANCO_COMPROBAR,
            ),
            (vec![OPCION_BANCO_INFORME, "res.json", "otro"], "otro"),
        ] {
            assert_eq!(
                interpretar(&argumentos(&orden)),
                Orden::NoEntendido(sobrante.to_string()),
                "{orden:?}"
            );
        }
        assert_eq!(interpretar(&argumentos(&[OPCION_VERSION])), Orden::Version);
    }

    /// La columna se mide en caracteres: en bytes, una invocación con acentos quedaría corta.
    #[test]
    fn la_columna_de_la_ayuda_se_mide_en_caracteres() {
        let linea = linea_de_uso("año", "x");
        assert_eq!(
            linea.find('x').map(|i| linea[..i].chars().count()),
            Some(COLUMNA_DE_LA_DESCRIPCION)
        );
    }

    /// Cada opción se escribe una vez. Las que se dan solas se reconocen, y
    /// todas salen en la ayuda tal como se reconocen.
    #[test]
    fn cada_opcion_se_reconoce_y_sale_en_la_ayuda() {
        let ayuda = texto_de_ayuda("0.0.0");
        for opcion in [
            OPCION_AYUDA,
            OPCION_VERSION,
            OPCION_DIAGNOSTICO,
            OPCION_RUTA,
            OPCION_BANCO,
            OPCION_BANCO_COMPROBAR,
            OPCION_BANCO_INFORME,
            OPCION_BANCO_COMPARAR,
            OPCION_REGISTRO_ANOTAR,
            OPCION_REVISION_BUSCAR,
            OPCION_REVISION_PROPONER,
            OPCION_RETIRAR_MODELO,
            OPCION_DESCARGAR_MODELO,
            OPCION_MODELO,
        ] {
            assert!(ayuda.contains(opcion), "{opcion} no sale en la ayuda");
            if opcion != OPCION_MODELO {
                assert!(
                    !matches!(interpretar(&argumentos(&[opcion])), Orden::NoEntendido(_)),
                    "{opcion} no se reconoce"
                );
            }
        }
        assert_eq!(
            interpretar(&argumentos(&[OPCION_AYUDA_CORTA])),
            Orden::Ayuda
        );
    }

    #[test]
    fn la_orden_del_ayudante_se_reconoce_tal_cual_se_compone() {
        let orden = argumentos(&["bash", "-c", "cargo test -- --nocapture"]);

        let compuesta = orden_del_ayudante("C:/p/programator.exe", &orden);

        assert_eq!(compuesta[0], "C:/p/programator.exe");
        assert_eq!(interpretar(&compuesta[1..]), Orden::LanzarAislado { orden });
    }

    #[test]
    fn el_ayudante_sin_separador_o_sin_orden_no_se_lanza() {
        let falta = Orden::FaltaValor("--lanzar-aislado".to_string());
        assert_eq!(interpretar(&argumentos(&["--lanzar-aislado"])), falta);
        assert_eq!(interpretar(&argumentos(&["--lanzar-aislado", "--"])), falta);
        assert_eq!(
            interpretar(&argumentos(&["--lanzar-aislado", "bash"])),
            falta
        );
    }

    #[test]
    fn el_ayudante_no_sale_en_la_ayuda() {
        assert!(!texto_de_ayuda("0.0.0").contains("--lanzar-aislado"));
    }

    #[test]
    fn sin_argumentos_se_ejecuta_el_ciclo() {
        assert_eq!(
            interpretar(&argumentos(&[])),
            Orden::Ejecutar { ruta: None }
        );
    }

    #[test]
    fn la_ruta_se_recoge_con_su_valor() {
        assert_eq!(
            interpretar(&argumentos(&["--ruta", "C:/proyectos/mi-proyecto"])),
            Orden::Ejecutar {
                ruta: Some("C:/proyectos/mi-proyecto".to_string())
            }
        );
    }

    #[test]
    fn una_ruta_sin_valor_no_arranca() {
        // Arrancar un proceso desatendido sobre una carpeta que no se sabe cuál es sería peor que
        // no arrancar.
        assert_eq!(
            interpretar(&argumentos(&["--ruta"])),
            Orden::FaltaValor("--ruta".to_string())
        );
    }

    #[test]
    fn una_ruta_seguida_de_otra_orden_tampoco_arranca() {
        // «--ruta --version» es casi seguro un dedo que se saltó la carpeta.
        assert_eq!(
            interpretar(&argumentos(&["--ruta", "--version"])),
            Orden::FaltaValor("--ruta".to_string())
        );
    }

    /// Solo «--» anuncia otra opción: una carpeta que empiece por un guion es un valor válido.
    #[test]
    fn un_valor_que_empieza_por_un_solo_guion_es_un_valor() {
        assert_eq!(
            interpretar(&argumentos(&["--ruta", "-borrador"])),
            Orden::Ejecutar {
                ruta: Some("-borrador".to_string())
            }
        );
        assert_eq!(
            interpretar(&argumentos(&["--banco", "-banco"])),
            Orden::Banco {
                carpeta: "-banco".to_string(),
                modelo: None,
            }
        );
    }

    #[test]
    fn el_texto_de_ayuda_documenta_la_ruta() {
        let texto = texto_de_ayuda("0.9.0");
        assert!(texto.contains("--ruta"), "{texto}");
    }

    #[test]
    fn las_dos_formas_de_pedir_ayuda_valen() {
        assert_eq!(interpretar(&argumentos(&["--ayuda"])), Orden::Ayuda);
        assert_eq!(interpretar(&argumentos(&["-h"])), Orden::Ayuda);
    }

    #[test]
    fn se_reconocen_version_y_diagnostico() {
        assert_eq!(interpretar(&argumentos(&["--version"])), Orden::Version);
        assert_eq!(
            interpretar(&argumentos(&["--diagnostico"])),
            Orden::Diagnostico { modelo: None }
        );
    }

    #[test]
    fn un_argumento_desconocido_no_arranca_el_ciclo() {
        // Arrancar un proceso desatendido porque no se entendió lo que se pedía es la peor
        // respuesta posible: se dice qué no se entendió y se termina.
        assert_eq!(
            interpretar(&argumentos(&["--actualizar-motor"])),
            Orden::NoEntendido("--actualizar-motor".to_string())
        );
    }

    #[test]
    fn el_primer_argumento_que_no_se_entiende_es_el_que_se_nombra() {
        assert_eq!(
            interpretar(&argumentos(&["--nada", "--ayuda"])),
            Orden::NoEntendido("--nada".to_string())
        );
    }

    #[test]
    fn la_ayuda_gana_a_lo_que_venga_detras() {
        // Pedir ayuda y otra cosa a la vez no es ambiguo: quien pregunta, pregunta.
        assert_eq!(
            interpretar(&argumentos(&["--ayuda", "--version"])),
            Orden::Ayuda
        );
    }

    #[test]
    fn el_banco_se_recoge_con_su_carpeta() {
        assert_eq!(
            interpretar(&argumentos(&["--banco", "evaluacion/banco"])),
            Orden::Banco {
                carpeta: "evaluacion/banco".to_string(),
                modelo: None,
            }
        );
    }

    #[test]
    fn la_autorizacion_de_maquina_ya_no_existe() {
        // Programator solo usa el equipo donde corre, y ejecutar
        // ahí no se autoriza. Una orden de antes con la opción no se toma por otra cosa.
        let opcion = "--maquina-autorizada".to_string();
        assert_eq!(
            interpretar(&argumentos(&[
                "--banco",
                "b",
                "--maquina-autorizada",
                "local"
            ])),
            Orden::NoEntendido(opcion.clone())
        );
        assert_eq!(
            interpretar(&argumentos(&[
                "--banco-comprobar",
                "t",
                "--maquina-autorizada",
                "local"
            ])),
            Orden::NoEntendido(opcion)
        );
        assert!(!texto_de_ayuda("0.0.0").contains("--maquina-autorizada"));
    }

    #[test]
    fn al_banco_le_falta_la_carpeta() {
        assert_eq!(
            interpretar(&argumentos(&["--banco"])),
            Orden::FaltaValor("--banco".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--banco", "--modelo", "x.gguf"])),
            Orden::FaltaValor("--banco".to_string())
        );
    }

    #[test]
    fn las_opciones_con_valor_valen_en_cualquier_orden_y_una_sola_vez() {
        let admitidas = ["--a", "--b"];
        let leidas =
            opciones_con_valor(&argumentos(&["--b", "2", "--a", "1"]), &admitidas).unwrap();
        assert_eq!(leidas["--a"], "1");
        assert_eq!(leidas["--b"], "2");
        assert!(opciones_con_valor(&[], &admitidas).unwrap().is_empty());
        assert_eq!(
            opciones_con_valor(&argumentos(&["--a", "1", "--a", "3"]), &admitidas),
            Err(Orden::NoEntendido("--a".to_string()))
        );
        assert_eq!(
            opciones_con_valor(&argumentos(&["--a", "--b", "2"]), &admitidas),
            Err(Orden::FaltaValor("--a".to_string()))
        );
        assert_eq!(
            opciones_con_valor(&argumentos(&["--c", "1"]), &admitidas),
            Err(Orden::NoEntendido("--c".to_string()))
        );
    }

    #[test]
    fn el_banco_y_el_diagnostico_recogen_el_modelo() {
        assert_eq!(
            interpretar(&argumentos(&["--banco", "b", "--modelo", "otro.gguf"])),
            Orden::Banco {
                carpeta: "b".to_string(),
                modelo: Some("otro.gguf".to_string()),
            }
        );
        assert_eq!(
            interpretar(&argumentos(&["--diagnostico", "--modelo", "otro.gguf"])),
            Orden::Diagnostico {
                modelo: Some("otro.gguf".to_string())
            }
        );
    }

    #[test]
    fn el_modelo_sin_valor_o_algo_sobrante_no_pasa() {
        assert_eq!(
            interpretar(&argumentos(&["--banco", "b", "--modelo"])),
            Orden::FaltaValor("--modelo".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--diagnostico", "--modelo"])),
            Orden::FaltaValor("--modelo".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--diagnostico", "otra"])),
            Orden::NoEntendido("otra".to_string())
        );
        // La comprobación aparte no carga el modelo, así que no lo admite.
        assert_eq!(
            interpretar(&argumentos(&[
                "--banco-comprobar",
                "t",
                "--modelo",
                "x.gguf"
            ])),
            Orden::NoEntendido("--modelo".to_string())
        );
    }

    #[test]
    fn la_ayuda_cuenta_el_modelo() {
        let ayuda = texto_de_ayuda("0.0.0");
        assert!(
            ayuda.contains("--diagnostico [--modelo <fichero>]"),
            "{ayuda}"
        );
        assert!(ayuda.contains("--modelo <fichero>]"), "{ayuda}");
    }

    #[test]
    fn una_opcion_mal_escrita_o_algo_sobrante_no_pasa_el_banco() {
        assert_eq!(
            interpretar(&argumentos(&["--banco", "b", "--maquina", "VM"])),
            Orden::NoEntendido("--maquina".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--banco", "b", "--modelo", "x.gguf", "otra"])),
            Orden::NoEntendido("otra".to_string())
        );
    }

    #[test]
    fn la_comprobacion_aparte_se_recoge_con_su_tanda() {
        assert_eq!(
            interpretar(&argumentos(&["--banco-comprobar", "b/resultados/t1"])),
            Orden::BancoComprobar {
                tanda: "b/resultados/t1".to_string(),
            }
        );
        assert_eq!(
            interpretar(&argumentos(&["--banco-comprobar"])),
            Orden::FaltaValor("--banco-comprobar".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--banco-comprobar", "t", "--maquina", "VM"])),
            Orden::NoEntendido("--maquina".to_string())
        );
    }

    #[test]
    fn el_informe_del_banco_se_recoge_y_pide_su_fichero() {
        assert_eq!(
            interpretar(&argumentos(&["--banco-informe", "r/resultados.toml"])),
            Orden::BancoInforme {
                resultados: "r/resultados.toml".to_string()
            }
        );
        assert_eq!(
            interpretar(&argumentos(&["--banco-informe"])),
            Orden::FaltaValor("--banco-informe".to_string())
        );
    }

    #[test]
    fn anotar_en_el_registro_pide_la_tanda_y_no_admite_nada_detras() {
        assert_eq!(
            interpretar(&argumentos(&["--registro-anotar", "r/a"])),
            Orden::RegistroAnotar {
                tanda: "r/a".to_string()
            }
        );
        assert_eq!(
            interpretar(&argumentos(&["--registro-anotar"])),
            Orden::FaltaValor("--registro-anotar".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--registro-anotar", "r/a", "r/b"])),
            Orden::NoEntendido("r/b".to_string())
        );
    }

    #[test]
    fn buscar_en_la_revision_no_lleva_valores() {
        assert_eq!(
            interpretar(&argumentos(&["--revision-buscar"])),
            Orden::RevisionBuscar
        );
        assert_eq!(
            interpretar(&argumentos(&["--revision-buscar", "x"])),
            Orden::NoEntendido("x".to_string())
        );
        assert!(texto_de_ayuda("0.0.0").contains("--revision-buscar"));
    }

    #[test]
    fn proponer_en_la_revision_no_lleva_valores() {
        assert_eq!(
            interpretar(&argumentos(&["--revision-proponer"])),
            Orden::RevisionProponer
        );
        assert_eq!(
            interpretar(&argumentos(&["--revision-proponer", "x"])),
            Orden::NoEntendido("x".to_string())
        );
        assert!(texto_de_ayuda("0.0.0").contains("--revision-proponer"));
    }

    #[test]
    fn retirar_un_modelo_pide_el_fichero_y_no_admite_nada_detras() {
        assert_eq!(
            interpretar(&argumentos(&["--retirar-modelo", "m.gguf"])),
            Orden::RetirarModelo {
                fichero: "m.gguf".to_string()
            }
        );
        assert_eq!(
            interpretar(&argumentos(&["--retirar-modelo"])),
            Orden::FaltaValor("--retirar-modelo".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--retirar-modelo", "m.gguf", "x"])),
            Orden::NoEntendido("x".to_string())
        );
        assert!(texto_de_ayuda("0.0.0").contains("--retirar-modelo"));
    }

    #[test]
    fn descargar_un_modelo_pide_el_repositorio_y_el_fichero() {
        assert_eq!(
            interpretar(&argumentos(&["--descargar-modelo", "a/b", "m.gguf"])),
            Orden::DescargarModelo {
                repositorio: "a/b".to_string(),
                fichero: "m.gguf".to_string()
            }
        );
        assert_eq!(
            interpretar(&argumentos(&["--descargar-modelo", "a/b"])),
            Orden::FaltaValor("--descargar-modelo".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--descargar-modelo", "a/b", "m.gguf", "x"])),
            Orden::NoEntendido("x".to_string())
        );
        assert!(texto_de_ayuda("0.0.0").contains("--descargar-modelo"));
    }

    #[test]
    fn comparar_tandas_pide_las_dos_y_no_admite_nada_detras() {
        assert_eq!(
            interpretar(&argumentos(&["--banco-comparar", "r/a", "r/b"])),
            Orden::BancoComparar {
                tanda_a: "r/a".to_string(),
                tanda_b: "r/b".to_string()
            }
        );
        assert_eq!(
            interpretar(&argumentos(&["--banco-comparar", "r/a"])),
            Orden::FaltaValor("--banco-comparar".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--banco-comparar", "r/a", "--ayuda"])),
            Orden::FaltaValor("--banco-comparar".to_string())
        );
        assert_eq!(
            interpretar(&argumentos(&["--banco-comparar", "r/a", "r/b", "r/c"])),
            Orden::NoEntendido("r/c".to_string())
        );
    }

    #[test]
    fn la_ayuda_recoge_las_dos_ordenes_del_banco() {
        let texto = texto_de_ayuda("0.11.0");
        for esperado in [
            "--banco <carpeta>",
            "--banco-informe",
            "--banco-comprobar <tanda>",
            "--banco-comparar <tanda-a> <tanda-b>",
        ] {
            assert!(texto.contains(esperado), "falta «{esperado}»:\n{texto}");
        }
    }

    #[test]
    fn el_texto_de_ayuda_nombra_las_cuatro_formas_de_uso_y_la_version() {
        let texto = texto_de_ayuda("0.5.0");
        assert!(texto.contains("0.5.0"), "{texto}");
        for esperado in ["--ayuda", "--version", "--diagnostico", "programator.toml"] {
            assert!(
                texto.contains(esperado),
                "falta «{esperado}» en la ayuda:\n{texto}"
            );
        }
    }
}
