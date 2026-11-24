//! La fase 2 de una tanda: prepara el lugar donde corren las pruebas ocultas, si lo hay, comprueba
//! y puntúa todo lo pasado, lo agrupa por encargo y deja el lugar como estaba.

use std::path::Path;

use super::carga_del_banco::carpeta_del_banco;
use super::lugar_de_comprobacion::interprete_de_este_equipo;
use super::puntuar::puntuar;
use super::{Pasado, Pruebas, CARPETA_DE_RUBRICAS};
use crate::banco::comprobacion::lanzador::LanzadorLocal;
use crate::banco::comprobacion::local::{CarpetaLocal, CARPETA_DE_COMPROBACION};
use crate::banco::comprobacion::{Comprobador, LugarDeEjecucion};
use crate::banco::config::Aislamiento;
use crate::banco::ficha::{Encargo, FichaEncargo};
use crate::banco::lugar::LugarElegido;
use crate::banco::mensajes;
use crate::banco::rastro;
use crate::banco::resultados::{
    calcular_clases, DatosDeTanda, ResultadoDeEncargo, ResultadoDeSemilla, ResultadosDeTanda,
};
use crate::error::Resultado;
use crate::ficheros::crear_carpeta;
use crate::terminal::Terminal;

/// Fase 2: comprueba y puntúa todo lo pasado y lo agrupa por encargo, en el orden en que se pasó.
pub fn puntuar_tanda(
    pasados: &[Pasado],
    datos: DatosDeTanda,
    margen: usize,
    mut pruebas: Option<&mut Pruebas>,
    carpeta_de_la_tanda: &Path,
    terminal: &dyn Terminal,
) -> Resultado<ResultadosDeTanda> {
    let carpeta_rubricas = carpeta_de_la_tanda.join(CARPETA_DE_RUBRICAS);
    crear_carpeta(&carpeta_rubricas)?;
    let mut encargos: Vec<ResultadoDeEncargo> = Vec::new();
    for pasado in pasados {
        let semilla = puntuar(
            pasado,
            margen,
            pruebas.as_deref_mut(),
            &carpeta_rubricas,
            terminal,
        )?;
        anadir_a_su_encargo(&mut encargos, &pasado.encargo.ficha, semilla);
    }
    Ok(ResultadosDeTanda {
        tanda: datos,
        clases: calcular_clases(&encargos),
        encargos,
    })
}

/// Pone la semilla con las de su encargo, o abre el encargo detrás de los que ya hay si es su
/// primera semilla: así quedan en el orden en que se pasaron.
fn anadir_a_su_encargo(
    encargos: &mut Vec<ResultadoDeEncargo>,
    ficha: &FichaEncargo,
    semilla: ResultadoDeSemilla,
) {
    let posicion = match encargos.iter().position(|e| e.id == ficha.id) {
        Some(posicion) => posicion,
        None => {
            encargos.push(ResultadoDeEncargo {
                id: ficha.id.clone(),
                cuenta_como: ficha.encargo_que_cuenta().to_string(),
                titulo: ficha.titulo.clone(),
                familia: ficha.familia,
                clase: ficha.clase,
                semillas: Vec::new(),
            });
            encargos.len() - 1
        }
    };
    encargos[posicion].semillas.push(semilla);
}

/// La fase 2: las pruebas ocultas en el lugar elegido, si lo hay, y la puntuación.
pub(super) fn comprobar_y_puntuar(
    pasados: &[Pasado],
    datos: DatosDeTanda,
    lugar: LugarElegido,
    carpeta: &Path,
    terminal: &dyn Terminal,
) -> Resultado<ResultadosDeTanda> {
    match lugar {
        LugarElegido::Ninguno => {
            let margen = datos.margen_lineas;
            puntuar_tanda(pasados, datos, margen, None, carpeta, terminal)
        }
        LugarElegido::Local(local) => {
            terminal.decir(&mensajes::mensaje_comprobando_en_local(
                &carpeta.join(CARPETA_DE_COMPROBACION),
            ));
            let entorno_del_sistema = std::env::vars().collect();
            let local = interprete_de_este_equipo(local, &carpeta_del_banco(carpeta)?, terminal)?;
            if local.aislamiento == Aislamiento::Ninguno {
                terminal.decir(&mensajes::mensaje_sin_aislar());
            }
            let comprobador = Comprobador::nuevo(CarpetaLocal::empezar(
                &local,
                carpeta,
                LanzadorLocal::para(&local),
                &entorno_del_sistema,
                terminal,
            )?);
            comprobar_en(comprobador, pasados, datos, carpeta, terminal)
        }
    }
}

/// Puntúa la tanda comprobando en un lugar ya preparado, y lo deja como estaba.
fn comprobar_en<G: LugarDeEjecucion>(
    mut comprobador: Comprobador<G>,
    pasados: &[Pasado],
    datos: DatosDeTanda,
    carpeta: &Path,
    terminal: &dyn Terminal,
) -> Resultado<ResultadosDeTanda> {
    let margen = datos.margen_lineas;
    // Se dice en el acto: el 12/11/2026 faltaron todas por un fallo del adaptador y la tanda
    // terminó sin avisar, con todo el código «no evaluado».
    let ausentes = comprobador.herramientas_ausentes();
    if !ausentes.is_empty() {
        terminal.avisar(&mensajes::mensaje_herramientas_ausentes(&ausentes));
    }
    let mut pruebas = |encargo: &Encargo, montada: &Path| {
        comprobador.comprobar_y_vigilar(&encargo.ficha, montada)
    };
    let puntuada = puntuar_tanda(
        pasados,
        datos,
        margen,
        Some(&mut pruebas),
        carpeta,
        terminal,
    );
    // Deja el lugar como estaba también si puntuar falló.
    let terminada = comprobador.terminar();
    let mut puntuada = puntuada?;
    let rastro: Vec<String> = terminada?.iter().map(rastro::describir).collect();
    if !rastro.is_empty() {
        terminal.avisar(&mensajes::mensaje_rastro_fuera(&rastro));
    }
    puntuada.tanda.rastro_fuera = rastro;
    Ok(puntuada)
}

#[cfg(test)]
mod pruebas {
    use super::super::dobles_de_prueba::{carpeta_tanda, criterio, datos, formas, pasar};
    use super::*;
    use crate::banco::resultados::{
        guardar, leer_toml, EstadoDeNota, FilaDeDesempeno, NOMBRE_INFORME, NOMBRE_RESULTADOS,
    };
    use crate::motor::Respuesta;
    use crate::terminal::TerminalDeMentira;

    #[test]
    fn una_tanda_completa_sin_comprobacion_deja_un_toml_y_un_informe_validos() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![criterio(dir.path()), formas(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![
                vec![Respuesta::Texto("El protocolo dice esto.".into())],
                vec![Respuesta::Texto(
                    "NUMERO_MAGICO | Pedidos.cs | Calcular | 11 | 0.21 | sin nombre\n\
                     METODO_LARGO | Pedidos.cs | Otro | 90 | Otro | inventado"
                        .into(),
                )],
            ],
        );
        let carpeta = carpeta_tanda(dir.path());

        let resultados = puntuar_tanda(
            &pasados,
            datos(),
            2,
            None,
            &carpeta,
            &TerminalDeMentira::default(),
        )
        .unwrap();
        guardar(&resultados, &carpeta).unwrap();

        let leido =
            leer_toml(&std::fs::read_to_string(carpeta.join(NOMBRE_RESULTADOS)).unwrap()).unwrap();
        assert_eq!(leido, resultados);
        let rubrica = &leido.encargos[0].semillas[0];
        assert_eq!(rubrica.estado, EstadoDeNota::Pendiente);
        let fichero = carpeta.join(rubrica.rubrica.as_ref().unwrap());
        assert!(std::fs::read_to_string(fichero)
            .unwrap()
            .contains("El protocolo dice esto."));
        let auditoria = &leido.encargos[1].semillas[0];
        assert_eq!(auditoria.aciertos, Some(1));
        assert_eq!(auditoria.inventados, Some(1));
        // 5 · 1 acierto / (2 sembrados + 1 inventado) = 1,67, a medio punto: 1,5.
        assert_eq!(auditoria.nota, Some(1.5));
        let formas = leido
            .clases
            .iter()
            .find(|c| c.fila == FilaDeDesempeno::Formas)
            .unwrap();
        assert_eq!(formas.nota_media, Some(1.5));
        let informe = std::fs::read_to_string(carpeta.join(NOMBRE_INFORME)).unwrap();
        assert!(informe.contains("Queda 1 rúbrica por puntuar"));
    }

    #[test]
    fn las_semillas_se_agrupan_por_encargo_en_el_orden_en_que_se_pasaron() {
        let ficha = |id: &str| -> FichaEncargo {
            toml::from_str(&format!(
                "id = \"{id}\"\ntitulo = \"T\"\nfamilia = \"criterio\"\n"
            ))
            .unwrap()
        };
        let semilla = |numero: i64| ResultadoDeSemilla {
            semilla: numero,
            ..ResultadoDeSemilla::default()
        };
        let mut encargos = Vec::new();

        anadir_a_su_encargo(&mut encargos, &ficha("E02"), semilla(1));
        anadir_a_su_encargo(&mut encargos, &ficha("E01"), semilla(1));
        anadir_a_su_encargo(&mut encargos, &ficha("E02"), semilla(2));

        let ids: Vec<&str> = encargos.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["E02", "E01"]);
        let semillas: Vec<i64> = encargos[0].semillas.iter().map(|s| s.semilla).collect();
        assert_eq!(semillas, [1, 2]);
        assert_eq!(encargos[1].semillas.len(), 1);
    }
}
