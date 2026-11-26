//! Una tanda completa del banco: pasar todos los encargos con todas las semillas, comprobar,
//! puntuar y dejar los resultados.
//!
//! Va en dos fases, y a propósito: primero se pasan todos los encargos con el motor cargado, y
//! después, con el motor ya apagado, se comprueban las entregas. Así las pruebas ocultas no
//! compiten con el motor por el equipo, y si la fase 2 falla, la 1 queda en disco y se termina
//! sin volver a cargar el modelo.
//!
//! [`pasar_encargos`](recorrido::pasar_encargos), [`puntuar`](puntuar::puntuar) y
//! [`puntuar_tanda`](fase_de_comprobar::puntuar_tanda) reciben el motor y las pruebas desde fuera,
//! para probarlos con dobles. [`ejecutar`] y [`rehacer`] son la capa fina que los conecta con el
//! motor de verdad, la configuración y la terminal.
//!
//! Este módulo guarda las órdenes del banco y lo que comparten las dos fases. Lo demás va aparte,
//! una cosa por submódulo: el banco en el disco (`carga_del_banco`), el lugar donde se comprobará
//! (`lugar_de_comprobacion`), la fase 1 con el motor (`fase_de_pasar`), con su recorrido
//! (`recorrido`), el arnés que lleva al modelo (`arnes_del_banco`), la compilación del motor
//! (`version_del_motor`) y los datos que se guardan de la tanda (`datos_de_la_tanda`); y la fase 2
//! (`fase_de_comprobar`), con la nota de cada semilla (`puntuar`), las pruebas ocultas de cada
//! entrega (`entrega`) y los ficheros de rúbrica (`rubricas`).

mod arnes_del_banco;
mod carga_del_banco;
mod datos_de_la_tanda;
#[cfg(test)]
mod dobles_de_prueba;
mod entrega;
mod fase_de_comprobar;
mod fase_de_pasar;
mod lugar_de_comprobacion;
mod puntuar;
mod recorrido;
mod rubricas;
mod version_del_motor;

use std::path::{Path, PathBuf};

use super::comprobacion::Comprobado;
use super::config::ConfigBanco;
use super::ejecucion::{Recogida, Reloj};
use super::ficha::Encargo;
use super::lugar::{elegir_lugar, LugarElegido};
use super::mensajes;
use super::pasadas;
use super::resultados::{
    guardar, rehacer_informe, DatosDeTanda, ResultadosDeTanda, NOMBRE_INFORME, NOMBRE_RESULTADOS,
};
use crate::config::Config;
use crate::error::{Error, Resultado};
use crate::terminal::Terminal;

use carga_del_banco::cargar_banco;
pub(super) use carga_del_banco::carpeta_del_banco;
use fase_de_comprobar::comprobar_y_puntuar;
use fase_de_pasar::{pasar_con_el_motor, BancoCargado};
use lugar_de_comprobacion::probar_el_lugar;

/// La configuración del banco, dentro de su carpeta.
pub const NOMBRE_BANCO_TOML: &str = "banco.toml";
/// Los encargos, uno por subcarpeta.
pub const CARPETA_DE_ENCARGOS: &str = "encargos";
/// Una subcarpeta por tanda.
pub const CARPETA_DE_RESULTADOS: &str = "resultados";
/// Dentro de la tanda: los ficheros que puntúa el agente que dirige.
pub const CARPETA_DE_RUBRICAS: &str = "rubricas";
/// Dentro de la carpeta de rúbricas: los motivos de cada nota, que escribe quien puntúa
/// (`plantillas/revision-mensual.md`).
pub const FICHERO_DE_MOTIVOS: &str = "motivos-de-las-notas.md";
/// Dentro de la tanda: la carpeta de trabajo de cada encargo y semilla.
pub const CARPETA_DE_TRABAJO: &str = "trabajo";

/// Un encargo pasado con una semilla, a falta de comprobar y puntuar.
pub struct Pasado<'a> {
    pub encargo: &'a Encargo,
    pub semilla: i64,
    pub carpeta: PathBuf,
    pub recogida: Recogida,
}

/// Lanza las pruebas ocultas sobre una carpeta ya montada.
pub type Pruebas<'p> = dyn FnMut(&Encargo, &Path) -> Comprobado + 'p;

/// Lo que se hace antes de cargar el modelo al pasar el banco: leerlo y probar dónde se
/// comprobará, para que lo que impida comprobar se sepa sin haber pasado la tanda.
fn cargar_y_probar(
    carpeta_banco: &Path,
    terminal: &dyn Terminal,
) -> Resultado<(ConfigBanco, Vec<Encargo>)> {
    let (banco, encargos) = cargar_banco(carpeta_banco, terminal)?;
    probar_el_lugar(&elegir_lugar(&banco, &encargos), carpeta_banco, terminal)?;
    Ok((banco, encargos))
}

/// La fase 2 y los resultados en disco. La usan `--banco` y `--banco-comprobar`.
///
/// Si algo falla aquí, la fase 1 ya está en `pasadas.toml`, y el error dice con qué orden se
/// termina la tanda sin volver a pasar los encargos por el modelo.
pub fn cerrar_tanda(
    pasados: &[Pasado],
    datos: DatosDeTanda,
    lugar: LugarElegido,
    carpeta: &Path,
    terminal: &dyn Terminal,
) -> Resultado<ResultadosDeTanda> {
    comprobar_y_puntuar(pasados, datos, lugar, carpeta, terminal)
        .and_then(|resultados| {
            guardar(&resultados, carpeta)?;
            Ok(resultados)
        })
        .map_err(|fallo| Error::Banco(mensajes::mensaje_tanda_sin_cerrar(&fallo, carpeta)))
}

fn contar_el_fin(resultados: &ResultadosDeTanda, carpeta: &Path, terminal: &dyn Terminal) {
    terminal.decir(&mensajes::mensaje_de_fin(
        &carpeta.join(NOMBRE_RESULTADOS),
        &carpeta.join(NOMBRE_INFORME),
        resultados.pendientes(),
    ));
}

/// `programator --banco <carpeta>`: la tanda entera con el motor de verdad. `reloj` mide la carga
/// del motor y el tiempo máximo de cada encargo.
pub fn ejecutar(
    config: &Config,
    carpeta_banco: &Path,
    version: &str,
    reloj: &mut dyn Reloj,
    terminal: &dyn Terminal,
) -> Resultado<()> {
    let (banco, encargos) = cargar_y_probar(carpeta_banco, terminal)?;
    let lugar = elegir_lugar(&banco, &encargos);
    let pasada = pasar_con_el_motor(
        config,
        BancoCargado {
            carpeta: carpeta_banco,
            config: &banco,
            encargos: &encargos,
            lugar: &lugar,
        },
        version,
        reloj,
        terminal,
    )?;
    let resultados = cerrar_tanda(
        &pasada.pasados,
        pasada.datos,
        lugar,
        &pasada.carpeta,
        terminal,
    )?;
    contar_el_fin(&resultados, &pasada.carpeta, terminal);
    Ok(())
}

/// Solo la fase 1 de una tanda: los encargos con el motor, guardados en disco. Devuelve la carpeta
/// de la tanda, que la fase 2 retoma con [`comprobar_guardada`]. `reloj`, como en [`ejecutar`].
pub fn pasar(
    config: &Config,
    carpeta_banco: &Path,
    version: &str,
    reloj: &mut dyn Reloj,
    terminal: &dyn Terminal,
) -> Resultado<PathBuf> {
    let (banco, encargos) = cargar_y_probar(carpeta_banco, terminal)?;
    let lugar = elegir_lugar(&banco, &encargos);
    pasar_con_el_motor(
        config,
        BancoCargado {
            carpeta: carpeta_banco,
            config: &banco,
            encargos: &encargos,
            lugar: &lugar,
        },
        version,
        reloj,
        terminal,
    )
    .map(|pasada| pasada.carpeta)
}

/// La fase 2 de una tanda cuya fase 1 quedó en disco, sin motor ni modelo.
pub fn comprobar_guardada(
    carpeta_de_la_tanda: &Path,
    agente: &str,
    terminal: &dyn Terminal,
) -> Resultado<ResultadosDeTanda> {
    let guardadas = pasadas::cargar(carpeta_de_la_tanda)?;
    let (banco, encargos) = cargar_banco(&carpeta_del_banco(carpeta_de_la_tanda)?, terminal)?;
    let lugar = elegir_lugar(&banco, &encargos);
    let pasados = pasadas::recuperar(
        &guardadas.pasadas,
        &encargos,
        &carpeta_de_la_tanda.join(CARPETA_DE_TRABAJO),
        agente,
    )?;
    terminal.decir(&mensajes::mensaje_comprobando_guardada(
        pasados.len(),
        carpeta_de_la_tanda,
    ));
    let datos = DatosDeTanda {
        maquina_de_comprobacion: lugar.nombre(),
        sistema_de_comprobacion: lugar.sistema(std::env::consts::OS),
        aislamiento: lugar.aislamiento(),
        ..guardadas.tanda
    };
    cerrar_tanda(&pasados, datos, lugar, carpeta_de_la_tanda, terminal)
}

/// `programator --banco-comprobar <carpeta de la tanda>`: solo la fase 2.
pub fn comprobar(
    config: &Config,
    carpeta_de_la_tanda: &Path,
    terminal: &dyn Terminal,
) -> Resultado<()> {
    let resultados = comprobar_guardada(carpeta_de_la_tanda, &config.agente.nombre, terminal)?;
    contar_el_fin(&resultados, carpeta_de_la_tanda, terminal);
    Ok(())
}

/// `programator --banco-informe <resultados.toml>`.
pub fn rehacer(ruta_resultados: &Path, terminal: &dyn Terminal) -> Resultado<()> {
    let pendientes = rehacer_informe(ruta_resultados)?;
    terminal.decir(&mensajes::mensaje_informe_rehecho(
        ruta_resultados,
        pendientes,
    ));
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::dobles_de_prueba::{
        carpeta_tanda, criterio, datos, formas, oficio, oficio_en_php, pasar, pedir,
    };
    use super::*;
    use crate::banco::rubrica::nombre_del_fichero;
    use crate::motor::Respuesta;
    use crate::terminal::TerminalDeMentira;

    #[test]
    fn comprobar_una_tanda_guardada_da_las_mismas_notas_que_la_tanda_completa() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let encargos = vec![
            criterio(carpeta_temporal.path()),
            formas(carpeta_temporal.path()),
            oficio(carpeta_temporal.path()),
        ];
        let pasados = pasar(
            carpeta_temporal.path(),
            &encargos,
            vec![
                vec![Respuesta::Texto("El protocolo dice esto.".into())],
                vec![Respuesta::Texto(
                    "NUMERO_MAGICO | Pedidos.cs | Calcular | 11 | 0.21 | sin nombre".into(),
                )],
                vec![
                    pedir(
                        "escribir_propuesta",
                        serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                    ),
                    pedir("publicar", serde_json::json!({"texto": "Entregado."})),
                ],
            ],
        );
        let carpeta = carpeta_tanda(carpeta_temporal.path());
        let completa = cerrar_tanda(
            &pasados,
            datos(),
            LugarElegido::Ninguno,
            &carpeta,
            &TerminalDeMentira::default(),
        )
        .unwrap();
        std::fs::remove_file(carpeta.join(NOMBRE_RESULTADOS)).unwrap();
        // Como la tanda completa, sin ejecutar código: desde el 13/11/2026 las pruebas ocultas van
        // encendidas por defecto, y una prueba no puede lanzar nada de verdad.
        std::fs::write(
            carpeta_temporal.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\nactiva = false\n",
        )
        .unwrap();

        let aparte =
            comprobar_guardada(&carpeta, "Programator", &TerminalDeMentira::default()).unwrap();

        assert_eq!(aparte, completa);
        assert!(carpeta.join(NOMBRE_RESULTADOS).is_file());
    }

    /// El 12/11/2026, `--banco-comprobar` sobre una tanda con las rúbricas ya puntuadas las
    /// escribió de nuevo en blanco y se perdieron las 21 notas puestas a mano (en una copia).
    #[test]
    fn comprobar_de_nuevo_no_pisa_las_rubricas_ya_puntuadas() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let encargos = vec![criterio(carpeta_temporal.path())];
        let pasados = pasar(
            carpeta_temporal.path(),
            &encargos,
            vec![vec![Respuesta::Texto("El protocolo dice esto.".into())]],
        );
        let carpeta = carpeta_tanda(carpeta_temporal.path());
        let primera = cerrar_tanda(
            &pasados,
            datos(),
            LugarElegido::Ninguno,
            &carpeta,
            &TerminalDeMentira::default(),
        )
        .unwrap();
        assert_eq!(primera.pendientes(), 1);
        let rubrica = carpeta
            .join(CARPETA_DE_RUBRICAS)
            .join(nombre_del_fichero("E01", 42));
        let puntuada = std::fs::read_to_string(&rubrica)
            .unwrap()
            .trim_end()
            .to_string()
            + " 4\n";
        std::fs::write(&rubrica, &puntuada).unwrap();

        let otra_vez =
            comprobar_guardada(&carpeta, "Programator", &TerminalDeMentira::default()).unwrap();

        assert_eq!(std::fs::read_to_string(&rubrica).unwrap(), puntuada);
        assert_eq!(otra_vez.pendientes(), 0);
        assert_eq!(otra_vez.encargos[0].semillas[0].nota, Some(4.0));
    }

    #[test]
    fn si_falla_la_fase_2_quedan_las_pasadas_y_se_dice_como_terminar() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let encargos = vec![criterio(carpeta_temporal.path())];
        let pasados = pasar(
            carpeta_temporal.path(),
            &encargos,
            vec![vec![Respuesta::Texto("El protocolo dice esto.".into())]],
        );
        let carpeta = carpeta_tanda(carpeta_temporal.path());
        // Una carpeta donde va el fichero de resultados: guardarlos falla.
        std::fs::create_dir_all(carpeta.join(NOMBRE_RESULTADOS)).unwrap();

        let fallo = cerrar_tanda(
            &pasados,
            datos(),
            LugarElegido::Ninguno,
            &carpeta,
            &TerminalDeMentira::default(),
        )
        .unwrap_err()
        .to_string();

        assert!(fallo.contains("--banco-comprobar"), "{fallo}");
        assert!(fallo.contains(pasadas::NOMBRE_PASADAS), "{fallo}");
        assert!(!fallo.contains("--maquina-autorizada"), "{fallo}");
        assert!(carpeta.join(pasadas::NOMBRE_PASADAS).is_file());
    }

    #[test]
    fn antes_de_cargar_el_modelo_se_prueba_donde_se_comprobara() {
        // La carpeta de resultados es un fichero: no se puede preparar la de comprobación, y eso
        // se sabe antes del motor. Si no se probara, `cargar_y_probar` saldría bien.
        let carpeta_temporal = tempfile::tempdir().unwrap();
        oficio_en_php(carpeta_temporal.path());
        std::fs::write(
            carpeta_temporal.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\naislamiento = \"ninguno\"\n\
             [comprobacion_local.herramientas]\nphp = \"php --version\"\n",
        )
        .unwrap();
        std::fs::write(
            carpeta_temporal.path().join(CARPETA_DE_RESULTADOS),
            "no es una carpeta",
        )
        .unwrap();

        assert!(cargar_banco(carpeta_temporal.path(), &TerminalDeMentira::default()).is_ok());
        assert!(cargar_y_probar(carpeta_temporal.path(), &TerminalDeMentira::default()).is_err());
    }

    #[test]
    fn pasar_y_ejecutar_el_banco_hacen_la_prueba_previa_antes_que_nada() {
        // Sin «motor.modelo», el motor diría que le falta: si sale el error de la prueba previa es
        // que se probó antes, y el motor no llega a arrancar nunca, se pruebe o no.
        let carpeta_temporal = tempfile::tempdir().unwrap();
        oficio_en_php(carpeta_temporal.path());
        std::fs::write(
            carpeta_temporal.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\naislamiento = \"ninguno\"\n\
             [comprobacion_local.herramientas]\nphp = \"php --version\"\n",
        )
        .unwrap();
        std::fs::write(
            carpeta_temporal.path().join(CARPETA_DE_RESULTADOS),
            "no es una carpeta",
        )
        .unwrap();
        let config: Config = toml::from_str("[motor]\n").unwrap();

        let reloj = &mut crate::reloj::RelojDeMentira::default();
        let fallos = [
            super::pasar(
                &config,
                carpeta_temporal.path(),
                "0.0.0",
                reloj,
                &TerminalDeMentira::default(),
            )
            .unwrap_err()
            .to_string(),
            ejecutar(
                &config,
                carpeta_temporal.path(),
                "0.0.0",
                reloj,
                &TerminalDeMentira::default(),
            )
            .unwrap_err()
            .to_string(),
        ];

        for fallo in fallos {
            assert!(
                fallo.contains(
                    crate::banco::comprobacion::prueba_previa::CARPETA_DE_LA_PRUEBA_PREVIA
                ),
                "{fallo}"
            );
        }
    }

    #[test]
    fn sin_codigo_que_comprobar_no_se_prueba_nada() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        oficio_en_php(carpeta_temporal.path());
        std::fs::write(
            carpeta_temporal.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\nactiva = false\n",
        )
        .unwrap();
        std::fs::write(
            carpeta_temporal.path().join(CARPETA_DE_RESULTADOS),
            "no es una carpeta",
        )
        .unwrap();

        assert!(cargar_y_probar(carpeta_temporal.path(), &TerminalDeMentira::default()).is_ok());
    }
}
