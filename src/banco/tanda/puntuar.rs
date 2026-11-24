//! La nota de un encargo pasado con una semilla: decide cómo se puntúa según su forma y pone en
//! el resultado de la semilla la nota, el recuento o el motivo de que no la tenga. Lo que toca el
//! disco va aparte: las pruebas ocultas en `entrega` y los ficheros de rúbrica en `rubricas`.

use std::path::Path;

use super::entrega::comprobar_la_entrega;
use super::rubricas::{puntuar_con_rubrica, valorar_leyendo_el_codigo};
use super::{Pasado, Pruebas};
use crate::banco::ficha::{DefectoSembrado, FormaDePuntuar};
use crate::banco::puntuacion::{
    contar_auditoria, leer_hallazgos, nota_de_auditoria, nota_de_comprobaciones,
    nota_de_fichero_limpio, HechosDeComprobaciones, Nota, RecuentoDeAuditoria,
};
use crate::banco::resultados::{PuntuadaCon, ResultadoDeSemilla};
use crate::error::{Error, Resultado};
use crate::terminal::Terminal;

/// La semilla con lo que se midió al pasarla, sin nota todavía.
fn semilla_medida(pasado: &Pasado) -> ResultadoDeSemilla {
    let consumo = &pasado.recogida.consumo;
    ResultadoDeSemilla {
        semilla: pasado.semilla,
        segundos: pasado.recogida.duracion.as_secs_f64(),
        tokens_entrada: consumo.total.tokens_entrada,
        tokens_en_cache: consumo.total.tokens_en_cache,
        tokens_generados: consumo.total.tokens_generados,
        ms_entrada: consumo.total.ms_entrada,
        ms_generacion: consumo.total.ms_generacion,
        respuestas_sin_consumo: consumo.respuestas_sin_consumo,
        ..ResultadoDeSemilla::default()
    }
}

fn poner_recuento(semilla: &mut ResultadoDeSemilla, recuento: &RecuentoDeAuditoria) {
    semilla.sembrados = Some(recuento.sembrados);
    semilla.aciertos = Some(recuento.aciertos);
    semilla.inventados = Some(recuento.inventados);
    semilla.repetidos = Some(recuento.repetidos);
    semilla.sin_formato = Some(recuento.sin_formato);
}

pub(super) const NO_RESPONDIO: &str = "no respondió a tiempo";

/// Comprueba y puntúa un encargo pasado. Los de rúbrica dejan su fichero en `carpeta_rubricas`.
pub fn puntuar(
    pasado: &Pasado,
    margen: usize,
    pruebas: Option<&mut Pruebas>,
    carpeta_rubricas: &Path,
    terminal: &dyn Terminal,
) -> Resultado<ResultadoDeSemilla> {
    let ficha = &pasado.encargo.ficha;
    let recogida = &pasado.recogida;
    let mut semilla = semilla_medida(pasado);
    let forma = ficha
        .forma_de_puntuar()
        .ok_or_else(|| Error::Banco(crate::banco::mensajes::familia_y_clase_no_casan(&ficha.id)))?;
    // Lo que no llegó a tiempo no cuenta, aunque algo se escribiera.
    let respuesta = if recogida.a_tiempo {
        recogida.respuesta.as_str()
    } else {
        semilla.detalle = Some(NO_RESPONDIO.into());
        ""
    };

    match forma {
        FormaDePuntuar::Comprobaciones => {
            puntuar_comprobaciones(pasado, pruebas, carpeta_rubricas, &mut semilla, terminal)?
        }
        FormaDePuntuar::Auditoria => {
            puntuar_auditoria(respuesta, &ficha.defectos, margen, &mut semilla)
        }
        FormaDePuntuar::FicheroLimpio => {
            puntuar_fichero_limpio(respuesta, margen, recogida.a_tiempo, &mut semilla)
        }
        FormaDePuntuar::Rubrica => {
            puntuar_con_rubrica(pasado, respuesta, carpeta_rubricas, &mut semilla)?
        }
    }
    Ok(semilla)
}

/// Un encargo de código: se comprueba lo que entregó, si lo entregó a tiempo. Si las pruebas
/// ocultas no dan nota y el encargo trae rúbrica, se valora con ella leyendo el código.
fn puntuar_comprobaciones(
    pasado: &Pasado,
    pruebas: Option<&mut Pruebas>,
    carpeta_rubricas: &Path,
    semilla: &mut ResultadoDeSemilla,
    terminal: &dyn Terminal,
) -> Resultado<()> {
    let recogida = &pasado.recogida;
    let nombre = pasado.encargo.ficha.entrega.as_deref().unwrap_or_default();
    let hechos = match recogida.entrega(nombre).filter(|_| recogida.a_tiempo) {
        None => {
            semilla
                .detalle
                .get_or_insert(format!("no entregó {nombre}"));
            semilla.puntuada_con = Some(PuntuadaCon::SinEntrega);
            HechosDeComprobaciones::NoEntregado
        }
        Some(ruta) => {
            let hechos = comprobar_la_entrega(pasado, ruta, pruebas, semilla, terminal)?;
            if hechos == HechosDeComprobaciones::NoEvaluado {
                return valorar_leyendo_el_codigo(pasado, ruta, carpeta_rubricas, semilla);
            }
            semilla.puntuada_con = Some(PuntuadaCon::PruebasOcultas);
            hechos
        }
    };
    if let HechosDeComprobaciones::Compila { superadas, total } = hechos {
        semilla.comprobaciones_superadas = Some(superadas);
        semilla.comprobaciones_total = Some(total);
    }
    semilla.poner_nota(nota_de_comprobaciones(hechos));
    Ok(())
}

/// Un encargo de auditoría: los defectos sembrados que encontró y los que se inventó.
fn puntuar_auditoria(
    respuesta: &str,
    defectos: &[DefectoSembrado],
    margen: usize,
    semilla: &mut ResultadoDeSemilla,
) {
    let recuento = contar_auditoria(&leer_hallazgos(respuesta), defectos, margen);
    poner_recuento(semilla, &recuento);
    semilla.poner_nota(nota_de_auditoria(&recuento));
}

/// Un fichero sin defectos: cuenta lo que se inventó.
fn puntuar_fichero_limpio(
    respuesta: &str,
    margen: usize,
    a_tiempo: bool,
    semilla: &mut ResultadoDeSemilla,
) {
    let recuento = contar_auditoria(&leer_hallazgos(respuesta), &[], margen);
    poner_recuento(semilla, &recuento);
    // Callar no es confirmar que está limpio: sin respuesta, un 0.
    let nota = if a_tiempo {
        nota_de_fichero_limpio(recuento.inventados)
    } else {
        Nota::Evaluada(0.0)
    };
    semilla.poner_nota(nota);
}

#[cfg(test)]
mod pruebas {
    use super::super::dobles_de_prueba::{oficio, pasar, pedir};
    use super::super::CARPETA_DE_RUBRICAS;
    use super::*;
    use crate::banco::comprobacion::{Comprobado, ResultadoDePruebas};
    use crate::banco::ficha::{Encargo, NOMBRE_RUBRICA};
    use crate::banco::resultados::EstadoDeNota;
    use crate::banco::rubrica::nombre_del_fichero;
    use crate::motor::Respuesta;
    use crate::terminal::TerminalDeMentira;

    #[test]
    fn con_las_pruebas_apagadas_una_entrega_de_oficio_sale_no_evaluada_y_no_con_cero() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![
                pedir(
                    "escribir_propuesta",
                    serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                ),
                Respuesta::Texto("Entregado.".into()),
            ]],
        );

        let semilla = puntuar(
            &pasados[0],
            2,
            None,
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::NoEvaluado);
        let detalle = semilla.detalle.unwrap();
        assert!(detalle.contains("no se ha ejecutado"), "{detalle}");
        assert!(detalle.contains("escrito y guardado"), "{detalle}");
        assert!(detalle.contains("apagadas"), "{detalle}");
    }

    #[test]
    fn con_las_pruebas_apagadas_el_motivo_nombra_la_seccion_que_sigue_existiendo() {
        // Desde el 13/11/2026 no hay máquina aparte: la sección que falta es [comprobacion_local], y nombrar la
        // vieja [comprobacion] mandaría a escribir una sección que hace que el banco no arranque.
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![
                pedir(
                    "escribir_propuesta",
                    serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                ),
                Respuesta::Texto("Entregado.".into()),
            ]],
        );

        let detalle = puntuar(
            &pasados[0],
            2,
            None,
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap()
        .detalle
        .unwrap();

        assert!(detalle.contains("[comprobacion_local]"), "{detalle}");
    }

    #[test]
    fn con_pruebas_se_monta_la_carpeta_y_se_suman_los_requisitos_de_texto() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        std::fs::create_dir_all(encargos[0].carpeta.join("pruebas/tests")).unwrap();
        std::fs::write(encargos[0].carpeta.join("pruebas/tests/t.rs"), "").unwrap();
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![
                pedir(
                    "escribir_propuesta",
                    serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                ),
                Respuesta::Texto("Entregado.".into()),
            ]],
        );
        let mut montadas = Vec::new();
        let mut pruebas = |_: &Encargo, montada: &Path| {
            montadas.push(montada.to_path_buf());
            Comprobado {
                resultado: ResultadoDePruebas::Compila {
                    superadas: vec![],
                    fallidas: vec!["basico".into()],
                },
                intentos_de_red: vec!["pypi.org:443".into()],
            }
        };

        let semilla = puntuar(
            &pasados[0],
            2,
            Some(&mut pruebas),
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.intentos_de_red, ["pypi.org:443"]);
        assert_eq!(semilla.comprobaciones_superadas, Some(1));
        assert_eq!(semilla.comprobaciones_total, Some(2));
        assert_eq!(semilla.nota, Some(2.5));
        assert!(montadas[0].join("src/rangos.rs").is_file());
        assert!(montadas[0].join("tests/t.rs").is_file());
        assert!(montadas[0].ends_with("E03-semilla-42-pruebas"));
    }

    #[test]
    fn sin_entrega_el_oficio_saca_un_cero_sin_lanzar_pruebas() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![Respuesta::Texto("No sé hacerlo.".into())]],
        );
        let mut lanzadas = 0;
        let mut pruebas = |_: &Encargo, _: &Path| {
            lanzadas += 1;
            ResultadoDePruebas::NoEvaluado("no".into()).into()
        };

        let semilla = puntuar(
            &pasados[0],
            2,
            Some(&mut pruebas),
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.nota, Some(0.0));
        assert!(semilla.detalle.unwrap().contains("no entregó rangos.rs"));
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::SinEntrega));
        assert_eq!(lanzadas, 0);
    }

    /// Un encargo de oficio con rúbrica, para valorarlo leyendo cuando las pruebas no dan nota.
    fn oficio_con_rubrica(dir: &Path) -> Encargo {
        let encargo = oficio(dir);
        std::fs::write(
            encargo.carpeta.join(NOMBRE_RUBRICA),
            "- **5**: cubre los casos\n",
        )
        .unwrap();
        // Las pruebas ocultas se montan desde aquí, aunque el doble no las ejecute.
        std::fs::create_dir_all(encargo.carpeta.join("pruebas")).unwrap();
        encargo
    }

    /// Pasa un encargo de oficio que entrega `rangos.rs` con `pub fn r() {}`.
    fn pasar_entregando<'a>(dir: &Path, encargos: &'a [Encargo]) -> Vec<Pasado<'a>> {
        pasar(
            dir,
            encargos,
            vec![vec![
                pedir(
                    "escribir_propuesta",
                    serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                ),
                Respuesta::Texto("Entregado.".into()),
            ]],
        )
    }

    #[test]
    fn con_las_pruebas_apagadas_y_rubrica_el_codigo_se_deja_para_valorarlo_leyendo() {
        // Sin pruebas ocultas, el código lo valora quien dirige
        // con la rúbrica, como los encargos de criterio, y no se queda en «no evaluado».
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);

        let semilla = puntuar(
            &pasados[0],
            2,
            None,
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::Pendiente);
        assert_eq!(semilla.nota, None);
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::Rubrica));
        let ruta = dir.path().join(nombre_del_fichero("E03", 42));
        assert_eq!(
            semilla.rubrica.as_deref(),
            Some(format!("{CARPETA_DE_RUBRICAS}/{}", nombre_del_fichero("E03", 42)).as_str())
        );
        let fichero = std::fs::read_to_string(ruta).unwrap();
        assert!(fichero.contains("```\npub fn r() {}\n```"), "{fichero}");
        assert!(fichero.contains("**sin ejecutarlo**"), "{fichero}");
        assert!(
            fichero.contains("apagadas"),
            "el motivo va en el fichero:\n{fichero}"
        );
        assert!(fichero.contains("- **5**: cubre los casos"), "{fichero}");
    }

    #[test]
    fn una_rubrica_de_codigo_ya_puntuada_da_su_nota_y_dice_que_es_de_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);
        std::fs::write(
            dir.path().join(nombre_del_fichero("E03", 42)),
            "# E03\n\nNota: 4\n",
        )
        .unwrap();

        let semilla = puntuar(
            &pasados[0],
            2,
            None,
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::Evaluada);
        assert_eq!(semilla.nota, Some(4.0));
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::Rubrica));
    }

    #[test]
    fn si_las_pruebas_no_se_pueden_ejecutar_tambien_se_valora_con_la_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);
        let mut pruebas = |_: &Encargo, _: &Path| {
            ResultadoDePruebas::NoEvaluado("rust no está instalado en este equipo".into()).into()
        };

        let semilla = puntuar(
            &pasados[0],
            2,
            Some(&mut pruebas),
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::Pendiente);
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::Rubrica));
        let fichero =
            std::fs::read_to_string(dir.path().join(nombre_del_fichero("E03", 42))).unwrap();
        assert!(
            fichero.contains("rust no está instalado en este equipo"),
            "{fichero}"
        );
    }

    #[test]
    fn si_las_pruebas_dan_nota_manda_esa_y_no_se_deja_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);
        let mut pruebas = |_: &Encargo, _: &Path| {
            ResultadoDePruebas::Compila {
                superadas: vec!["basico".into()],
                fallidas: vec![],
            }
            .into()
        };

        let semilla = puntuar(
            &pasados[0],
            2,
            Some(&mut pruebas),
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::Evaluada);
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::PruebasOcultas));
        assert_eq!(semilla.rubrica, None);
        assert!(!dir.path().join(nombre_del_fichero("E03", 42)).exists());
    }

    #[test]
    fn lo_que_no_compila_es_nota_de_las_pruebas_y_no_va_a_la_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);
        let mut pruebas =
            |_: &Encargo, _: &Path| ResultadoDePruebas::NoCompila("error[E0425]".into()).into();

        let semilla = puntuar(
            &pasados[0],
            2,
            Some(&mut pruebas),
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.nota, Some(0.0));
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::PruebasOcultas));
        assert!(!dir.path().join(nombre_del_fichero("E03", 42)).exists());
    }

    #[test]
    fn sin_rubrica_el_motivo_dice_que_falta_para_valorarlo_leyendo() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);

        let semilla = puntuar(
            &pasados[0],
            2,
            None,
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::NoEvaluado);
        assert_eq!(semilla.puntuada_con, None);
        let detalle = semilla.detalle.unwrap();
        assert!(detalle.contains("apagadas"), "{detalle}");
        assert!(detalle.contains(NOMBRE_RUBRICA), "{detalle}");
    }

    #[test]
    fn sin_entrega_no_se_deja_rubrica_y_se_dice_que_no_entrego() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![Respuesta::Texto("No sé hacerlo.".into())]],
        );

        let semilla = puntuar(
            &pasados[0],
            2,
            None,
            dir.path(),
            &TerminalDeMentira::default(),
        )
        .unwrap();

        assert_eq!(semilla.nota, Some(0.0));
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::SinEntrega));
        assert!(!dir.path().join(nombre_del_fichero("E03", 42)).exists());
    }
}
