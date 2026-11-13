//! Qué se le ha pedido a Programator en la línea de órdenes.
//!
//! Función pura sobre un vector de cadenas: no lee el entorno ni escribe en pantalla, para que
//! todas las combinaciones se puedan probar sin lanzar el programa.

use std::collections::BTreeMap;

/// Lo que el Director ha pedido al invocar el ejecutable.
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
            "--ayuda" | "-h" => Orden::Ayuda,
            "--version" => Orden::Version,
            "--diagnostico" => match opciones_con_valor(&argumentos[1..], &[OPCION_MODELO]) {
                Ok(mut leidas) => Orden::Diagnostico {
                    modelo: leidas.remove(OPCION_MODELO),
                },
                Err(orden) => orden,
            },
            // El valor va detrás, y tiene que parecer una carpeta: cualquier cosa que empiece por
            // «--» es otra opción, no la carpeta que alguien olvidó escribir.
            "--ruta" => match argumentos.get(1) {
                Some(valor) if !valor.starts_with("--") => Orden::Ejecutar {
                    ruta: Some(valor.clone()),
                },
                _ => Orden::FaltaValor("--ruta".to_string()),
            },
            "--banco" => match valor_tras(argumentos, 0) {
                Some(carpeta) => match opciones_con_valor(&argumentos[2..], &[OPCION_MODELO]) {
                    Ok(mut leidas) => Orden::Banco {
                        carpeta: carpeta.clone(),
                        modelo: leidas.remove(OPCION_MODELO),
                    },
                    Err(orden) => orden,
                },
                None => Orden::FaltaValor("--banco".to_string()),
            },
            "--banco-comprobar" => match valor_tras(argumentos, 0) {
                Some(tanda) => match argumentos.get(2) {
                    None => Orden::BancoComprobar {
                        tanda: tanda.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                None => Orden::FaltaValor("--banco-comprobar".to_string()),
            },
            "--banco-informe" => match valor_tras(argumentos, 0) {
                Some(resultados) => Orden::BancoInforme {
                    resultados: resultados.clone(),
                },
                None => Orden::FaltaValor("--banco-informe".to_string()),
            },
            "--banco-comparar" => match (valor_tras(argumentos, 0), valor_tras(argumentos, 1)) {
                (Some(tanda_a), Some(tanda_b)) => match argumentos.get(3) {
                    None => Orden::BancoComparar {
                        tanda_a: tanda_a.clone(),
                        tanda_b: tanda_b.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                _ => Orden::FaltaValor("--banco-comparar".to_string()),
            },
            "--registro-anotar" => match valor_tras(argumentos, 0) {
                Some(tanda) => match argumentos.get(2) {
                    None => Orden::RegistroAnotar {
                        tanda: tanda.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                None => Orden::FaltaValor("--registro-anotar".to_string()),
            },
            "--revision-buscar" => match argumentos.get(1) {
                None => Orden::RevisionBuscar,
                Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
            },
            "--revision-proponer" => match argumentos.get(1) {
                None => Orden::RevisionProponer,
                Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
            },
            "--retirar-modelo" => match valor_tras(argumentos, 0) {
                Some(fichero) => match argumentos.get(2) {
                    None => Orden::RetirarModelo {
                        fichero: fichero.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                None => Orden::FaltaValor("--retirar-modelo".to_string()),
            },
            "--descargar-modelo" => match (valor_tras(argumentos, 0), valor_tras(argumentos, 1)) {
                (Some(repositorio), Some(fichero)) => match argumentos.get(3) {
                    None => Orden::DescargarModelo {
                        repositorio: repositorio.clone(),
                        fichero: fichero.clone(),
                    },
                    Some(sobrante) => Orden::NoEntendido(sobrante.clone()),
                },
                _ => Orden::FaltaValor("--descargar-modelo".to_string()),
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
const OPCION_MODELO: &str = "--modelo";

/// El valor que va detrás de la opción en `posicion`, si lo hay y no es otra opción.
fn valor_tras(argumentos: &[String], posicion: usize) -> Option<&String> {
    argumentos
        .get(posicion + 1)
        .filter(|valor| !valor.starts_with("--"))
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

/// Columna en la que empieza la descripción de cada forma de uso.
///
/// Se compone, no se escribe a mano con espacios, porque el nombre del ejecutable cambia de
/// longitud entre Windows y el resto de sistemas y la tabla quedaría desalineada en uno de los
/// dos.
const COLUMNA_DE_LA_DESCRIPCION: usize = 31;

/// Una línea de la sección USO: la invocación a la izquierda y qué hace a la derecha.
///
/// Si la invocación no cabe en su columna, la descripción baja a la línea siguiente en vez de
/// empujar la tabla hacia la derecha. Así el texto sigue leyéndose en una terminal estrecha.
fn linea_de_uso(invocacion: &str, descripcion: &str) -> String {
    let sangrado = "  ";
    let escrito = sangrado.len() + invocacion.len();
    if escrito < COLUMNA_DE_LA_DESCRIPCION {
        let relleno = COLUMNA_DE_LA_DESCRIPCION - escrito;
        format!(
            "{sangrado}{invocacion}{:relleno$}{descripcion}
",
            ""
        )
    } else {
        format!(
            "{sangrado}{invocacion}
{:COLUMNA_DE_LA_DESCRIPCION$}{descripcion}
",
            ""
        )
    }
}

/// El texto que escribe `--ayuda`.
pub fn texto_de_ayuda(version: &str) -> String {
    let programa = crate::plataforma::NOMBRE_DEL_EJECUTABLE;
    let uso = [
        (
            programa.to_string(),
            "Arranca y entra en el ciclo de trabajo.",
        ),
        (
            format!("{programa} --ruta <carpeta>"),
            "Arranca sobre esa carpeta, sin abrir el diálogo.",
        ),
        (
            format!("{programa} --ayuda"),
            "Muestra esta ayuda y termina.",
        ),
        (
            format!("{programa} --version"),
            "Escribe la versión y termina.",
        ),
        (
            format!("{programa} --diagnostico [{OPCION_MODELO} <fichero>]"),
            "Escribe el estado de la máquina, el motor y el modelo.",
        ),
        (
            format!("{programa} --banco <carpeta> [{OPCION_MODELO} <fichero>]"),
            "Pasa al modelo una tanda del banco de modelos.",
        ),
        (
            format!("{programa} --banco-comprobar <tanda>"),
            "Hace solo las pruebas ocultas de una tanda ya pasada.",
        ),
        (
            format!("{programa} --banco-informe <resultados.toml>"),
            "Rehace el informe de una tanda con las rúbricas puntuadas.",
        ),
        (
            format!("{programa} --banco-comparar <tanda-a> <tanda-b>"),
            "Dice si dos tandas guardadas son repetición una de otra.",
        ),
        (
            format!("{programa} --registro-anotar <tanda>"),
            "Anota en el registro de modelos lo que midió una tanda terminada.",
        ),
        (
            format!("{programa} --revision-buscar"),
            "Busca en Hugging Face modelos que quepan en la tarjeta.",
        ),
        (
            format!("{programa} --descargar-modelo <repositorio> <fichero>"),
            "Descarga un candidato de la última búsqueda y comprueba su huella.",
        ),
        (
            format!("{programa} --revision-proponer"),
            "Propone qué modelos se quedan y cuáles se retiran, según el registro.",
        ),
        (
            format!("{programa} --retirar-modelo <fichero>"),
            "Anota la retirada en el registro y borra el fichero del modelo.",
        ),
    ]
    .iter()
    .map(|(invocacion, que_hace)| linea_de_uso(invocacion, que_hace))
    .collect::<String>();

    format!(
        "Programator {version} — arnés que convierte un modelo local en un agente del canal

USO
{uso}
  «--ruta» manda sobre «[carpeta] ruta» del TOML, y «--diagnostico» termina
  sin arrancar el ciclo.
  «--banco» lee el banco.toml de esa carpeta. Ejecuta en este equipo las pruebas
  ocultas, salvo que [comprobacion_local] diga «activa = false». Si fallan las
  pruebas, «--banco-comprobar» termina la tanda con lo que dejó en «pasadas.toml», sin
  volver a cargar el modelo.
  «{OPCION_MODELO}» prueba otro fichero GGUF en lugar de «[motor] modelo», solo en esa
  ejecución y sin tocar el TOML; se resuelve igual que el del TOML.
  La revisión mensual va por pasos: buscar, descargar, pasar el banco, proponer y
  retirar. Se configura en «[revision]» y deja lo suyo en «revisiones/AAAA-MM/».
  También se puede pedir por el canal, con «revisión: buscar», «revisión: probar» y
  «revisión: cerrar»: la atiende el arnés, no el modelo, y solo de los buzones de
  «[revision] buzones_que_ordenan». El procedimiento está en
  «plantillas/revision-mensual.md».

CONFIGURACIÓN
  Se lee de «programator.toml», en la misma carpeta que el ejecutable.
  Las rutas relativas se resuelven contra ese fichero, nunca contra el
  directorio desde el que lances el programa.

DÓNDE MIRAR
  Pruebas manuales:   docs/pruebas-manuales.md
  Defectos conocidos: docs/defectos-conocidos.md
"
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn la_orden_del_ayudante_se_reconoce_tal_cual_se_compone() {
        let orden = args(&["bash", "-c", "cargo test -- --nocapture"]);

        let compuesta = orden_del_ayudante("C:/p/programator.exe", &orden);

        assert_eq!(compuesta[0], "C:/p/programator.exe");
        assert_eq!(interpretar(&compuesta[1..]), Orden::LanzarAislado { orden });
    }

    #[test]
    fn el_ayudante_sin_separador_o_sin_orden_no_se_lanza() {
        let falta = Orden::FaltaValor("--lanzar-aislado".to_string());
        assert_eq!(interpretar(&args(&["--lanzar-aislado"])), falta);
        assert_eq!(interpretar(&args(&["--lanzar-aislado", "--"])), falta);
        assert_eq!(interpretar(&args(&["--lanzar-aislado", "bash"])), falta);
    }

    #[test]
    fn el_ayudante_no_sale_en_la_ayuda() {
        assert!(!texto_de_ayuda("0.0.0").contains("--lanzar-aislado"));
    }

    #[test]
    fn sin_argumentos_se_ejecuta_el_ciclo() {
        assert_eq!(interpretar(&args(&[])), Orden::Ejecutar { ruta: None });
    }

    #[test]
    fn la_ruta_se_recoge_con_su_valor() {
        assert_eq!(
            interpretar(&args(&["--ruta", "C:/proyectos/mi-proyecto"])),
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
            interpretar(&args(&["--ruta"])),
            Orden::FaltaValor("--ruta".to_string())
        );
    }

    #[test]
    fn una_ruta_seguida_de_otra_orden_tampoco_arranca() {
        // «--ruta --version» es casi seguro un dedo que se saltó la carpeta.
        assert_eq!(
            interpretar(&args(&["--ruta", "--version"])),
            Orden::FaltaValor("--ruta".to_string())
        );
    }

    #[test]
    fn el_texto_de_ayuda_documenta_la_ruta() {
        let texto = texto_de_ayuda("0.9.0");
        assert!(texto.contains("--ruta"), "{texto}");
    }

    #[test]
    fn las_dos_formas_de_pedir_ayuda_valen() {
        assert_eq!(interpretar(&args(&["--ayuda"])), Orden::Ayuda);
        assert_eq!(interpretar(&args(&["-h"])), Orden::Ayuda);
    }

    #[test]
    fn se_reconocen_version_y_diagnostico() {
        assert_eq!(interpretar(&args(&["--version"])), Orden::Version);
        assert_eq!(
            interpretar(&args(&["--diagnostico"])),
            Orden::Diagnostico { modelo: None }
        );
    }

    #[test]
    fn un_argumento_desconocido_no_arranca_el_ciclo() {
        // Arrancar un proceso desatendido porque no se entendió lo que se pedía es la peor
        // respuesta posible: se dice qué no se entendió y se termina.
        assert_eq!(
            interpretar(&args(&["--actualizar-motor"])),
            Orden::NoEntendido("--actualizar-motor".to_string())
        );
    }

    #[test]
    fn el_primer_argumento_que_no_se_entiende_es_el_que_se_nombra() {
        assert_eq!(
            interpretar(&args(&["--nada", "--ayuda"])),
            Orden::NoEntendido("--nada".to_string())
        );
    }

    #[test]
    fn la_ayuda_gana_a_lo_que_venga_detras() {
        // Pedir ayuda y otra cosa a la vez no es ambiguo: quien pregunta, pregunta.
        assert_eq!(interpretar(&args(&["--ayuda", "--version"])), Orden::Ayuda);
    }

    #[test]
    fn el_banco_se_recoge_con_su_carpeta() {
        assert_eq!(
            interpretar(&args(&["--banco", "evaluacion/banco"])),
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
            interpretar(&args(&["--banco", "b", "--maquina-autorizada", "local"])),
            Orden::NoEntendido(opcion.clone())
        );
        assert_eq!(
            interpretar(&args(&[
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
            interpretar(&args(&["--banco"])),
            Orden::FaltaValor("--banco".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--banco", "--modelo", "x.gguf"])),
            Orden::FaltaValor("--banco".to_string())
        );
    }

    #[test]
    fn las_opciones_con_valor_valen_en_cualquier_orden_y_una_sola_vez() {
        let admitidas = ["--a", "--b"];
        let leidas = opciones_con_valor(&args(&["--b", "2", "--a", "1"]), &admitidas).unwrap();
        assert_eq!(leidas["--a"], "1");
        assert_eq!(leidas["--b"], "2");
        assert!(opciones_con_valor(&[], &admitidas).unwrap().is_empty());
        assert_eq!(
            opciones_con_valor(&args(&["--a", "1", "--a", "3"]), &admitidas),
            Err(Orden::NoEntendido("--a".to_string()))
        );
        assert_eq!(
            opciones_con_valor(&args(&["--a", "--b", "2"]), &admitidas),
            Err(Orden::FaltaValor("--a".to_string()))
        );
        assert_eq!(
            opciones_con_valor(&args(&["--c", "1"]), &admitidas),
            Err(Orden::NoEntendido("--c".to_string()))
        );
    }

    #[test]
    fn el_banco_y_el_diagnostico_recogen_el_modelo() {
        assert_eq!(
            interpretar(&args(&["--banco", "b", "--modelo", "otro.gguf"])),
            Orden::Banco {
                carpeta: "b".to_string(),
                modelo: Some("otro.gguf".to_string()),
            }
        );
        assert_eq!(
            interpretar(&args(&["--diagnostico", "--modelo", "otro.gguf"])),
            Orden::Diagnostico {
                modelo: Some("otro.gguf".to_string())
            }
        );
    }

    #[test]
    fn el_modelo_sin_valor_o_algo_sobrante_no_pasa() {
        assert_eq!(
            interpretar(&args(&["--banco", "b", "--modelo"])),
            Orden::FaltaValor("--modelo".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--diagnostico", "--modelo"])),
            Orden::FaltaValor("--modelo".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--diagnostico", "otra"])),
            Orden::NoEntendido("otra".to_string())
        );
        // La comprobación aparte no carga el modelo, así que no lo admite.
        assert_eq!(
            interpretar(&args(&["--banco-comprobar", "t", "--modelo", "x.gguf"])),
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
            interpretar(&args(&["--banco", "b", "--maquina", "VM"])),
            Orden::NoEntendido("--maquina".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--banco", "b", "--modelo", "x.gguf", "otra"])),
            Orden::NoEntendido("otra".to_string())
        );
    }

    #[test]
    fn la_comprobacion_aparte_se_recoge_con_su_tanda() {
        assert_eq!(
            interpretar(&args(&["--banco-comprobar", "b/resultados/t1"])),
            Orden::BancoComprobar {
                tanda: "b/resultados/t1".to_string(),
            }
        );
        assert_eq!(
            interpretar(&args(&["--banco-comprobar"])),
            Orden::FaltaValor("--banco-comprobar".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--banco-comprobar", "t", "--maquina", "VM"])),
            Orden::NoEntendido("--maquina".to_string())
        );
    }

    #[test]
    fn el_informe_del_banco_se_recoge_y_pide_su_fichero() {
        assert_eq!(
            interpretar(&args(&["--banco-informe", "r/resultados.toml"])),
            Orden::BancoInforme {
                resultados: "r/resultados.toml".to_string()
            }
        );
        assert_eq!(
            interpretar(&args(&["--banco-informe"])),
            Orden::FaltaValor("--banco-informe".to_string())
        );
    }

    #[test]
    fn anotar_en_el_registro_pide_la_tanda_y_no_admite_nada_detras() {
        assert_eq!(
            interpretar(&args(&["--registro-anotar", "r/a"])),
            Orden::RegistroAnotar {
                tanda: "r/a".to_string()
            }
        );
        assert_eq!(
            interpretar(&args(&["--registro-anotar"])),
            Orden::FaltaValor("--registro-anotar".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--registro-anotar", "r/a", "r/b"])),
            Orden::NoEntendido("r/b".to_string())
        );
    }

    #[test]
    fn buscar_en_la_revision_no_lleva_valores() {
        assert_eq!(
            interpretar(&args(&["--revision-buscar"])),
            Orden::RevisionBuscar
        );
        assert_eq!(
            interpretar(&args(&["--revision-buscar", "x"])),
            Orden::NoEntendido("x".to_string())
        );
        assert!(texto_de_ayuda("0.0.0").contains("--revision-buscar"));
    }

    #[test]
    fn proponer_en_la_revision_no_lleva_valores() {
        assert_eq!(
            interpretar(&args(&["--revision-proponer"])),
            Orden::RevisionProponer
        );
        assert_eq!(
            interpretar(&args(&["--revision-proponer", "x"])),
            Orden::NoEntendido("x".to_string())
        );
        assert!(texto_de_ayuda("0.0.0").contains("--revision-proponer"));
    }

    #[test]
    fn retirar_un_modelo_pide_el_fichero_y_no_admite_nada_detras() {
        assert_eq!(
            interpretar(&args(&["--retirar-modelo", "m.gguf"])),
            Orden::RetirarModelo {
                fichero: "m.gguf".to_string()
            }
        );
        assert_eq!(
            interpretar(&args(&["--retirar-modelo"])),
            Orden::FaltaValor("--retirar-modelo".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--retirar-modelo", "m.gguf", "x"])),
            Orden::NoEntendido("x".to_string())
        );
        assert!(texto_de_ayuda("0.0.0").contains("--retirar-modelo"));
    }

    #[test]
    fn descargar_un_modelo_pide_el_repositorio_y_el_fichero() {
        assert_eq!(
            interpretar(&args(&["--descargar-modelo", "a/b", "m.gguf"])),
            Orden::DescargarModelo {
                repositorio: "a/b".to_string(),
                fichero: "m.gguf".to_string()
            }
        );
        assert_eq!(
            interpretar(&args(&["--descargar-modelo", "a/b"])),
            Orden::FaltaValor("--descargar-modelo".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--descargar-modelo", "a/b", "m.gguf", "x"])),
            Orden::NoEntendido("x".to_string())
        );
        assert!(texto_de_ayuda("0.0.0").contains("--descargar-modelo"));
    }

    #[test]
    fn comparar_tandas_pide_las_dos_y_no_admite_nada_detras() {
        assert_eq!(
            interpretar(&args(&["--banco-comparar", "r/a", "r/b"])),
            Orden::BancoComparar {
                tanda_a: "r/a".to_string(),
                tanda_b: "r/b".to_string()
            }
        );
        assert_eq!(
            interpretar(&args(&["--banco-comparar", "r/a"])),
            Orden::FaltaValor("--banco-comparar".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--banco-comparar", "r/a", "--ayuda"])),
            Orden::FaltaValor("--banco-comparar".to_string())
        );
        assert_eq!(
            interpretar(&args(&["--banco-comparar", "r/a", "r/b", "r/c"])),
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
