//! De los resultados de una tanda a una entrada del registro, y cómo entra en él.
//!
//! Puro: no lee ni escribe nada. Las cifras salen de los mismos cálculos que el informe de la
//! tanda (`calcular_clases`, `nota_de_familia` y las velocidades), así que el registro y el
//! informe no pueden contar cosas distintas.

use crate::banco::ficha::{forma_de_puntuar, Familia, FormaDePuntuar};
use crate::banco::resultados::{
    calcular_clases, nota_de_familia, resumir_encargos, velocidad_de_generacion,
    velocidad_de_lectura, PuntuadaCon, ResultadoDeEncargo, ResultadosDeTanda,
};

use super::modelo::{CodigoPuntuadoCon, EntradaDeModelo, NotaDeClase, NotasDeFamilia, Registro};

/// Por qué una tanda no se puede anotar. Una entrada incompleta contaría cosas que no se midieron.
#[derive(Debug, Clone, PartialEq)]
pub enum MotivoParaNoAnotar {
    /// Quedan rúbricas sin nota (de criterio, o de código valorado leyéndolo): la media de su
    /// familia saldría de menos encargos.
    QuedanRubricas(usize),
    /// Sin la tarjeta no se sabe a qué máquina se refieren las cifras.
    FaltaLaTarjeta,
    /// Sin el contexto y la caché no se sabe con qué configuración se midió.
    FaltanLosAjustesDelMotor,
}

/// Lo que pasó al anotar.
#[derive(Debug, Clone, PartialEq)]
pub enum Anotacion {
    Nueva,
    /// Ya había una entrada del mismo modelo en la misma tarjeta, y se ha sustituido.
    Sustituye(Vec<Cambio>),
}

/// Qué cifra de una entrada cambió al sustituirla.
#[derive(Debug, Clone, PartialEq)]
pub struct Cambio {
    pub que: QueCambia,
    pub antes: Valor,
    pub despues: Valor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueCambia {
    Tanda,
    CompilacionDelMotor,
    VersionDeProgramator,
    CapasEnGpu,
    Contexto,
    Generando,
    Leyendo,
    Familia(Familia),
    Clase(crate::banco::resultados::FilaDeDesempeno),
}

/// Un valor de la entrada, para decir cómo estaba y cómo queda.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Texto(String),
    /// `None` es «sin medir».
    Cifra(Option<f64>),
}

/// La entrada que sale de una tanda, o por qué no puede salir.
pub fn entrada_de_la_tanda(
    resultados: &ResultadosDeTanda,
    nombre_de_la_tanda: &str,
) -> Result<EntradaDeModelo, MotivoParaNoAnotar> {
    let pendientes = resultados.pendientes();
    if pendientes > 0 {
        return Err(MotivoParaNoAnotar::QuedanRubricas(pendientes));
    }
    let datos = &resultados.tanda;
    let tarjeta = datos
        .tarjeta
        .clone()
        .ok_or(MotivoParaNoAnotar::FaltaLaTarjeta)?;
    let ajustes = datos
        .ajustes_del_motor
        .as_ref()
        .ok_or(MotivoParaNoAnotar::FaltanLosAjustesDelMotor)?;

    let encargos = &resultados.encargos;
    let resumenes = resumir_encargos(encargos);
    let semillas = || encargos.iter().flat_map(|e| e.semillas.iter());
    Ok(EntradaDeModelo {
        nombre: datos.modelo.clone(),
        huella_sha256: datos.huella_sha256.clone(),
        tarjeta,
        compilacion_motor: datos.compilacion_motor.clone(),
        version_programator: datos.version_programator.clone(),
        medido_el: datos.fecha.clone(),
        tanda: nombre_de_la_tanda.to_string(),
        semillas: datos.semillas.clone(),
        capas_en_gpu: datos.capas_en_gpu,
        contexto: ajustes.contexto,
        cache_kv: ajustes.cache_kv.clone(),
        vram_ocupada_mib: datos.vram_ocupada_mib,
        generando_tokens_por_segundo: velocidad_de_generacion(semillas()),
        leyendo_tokens_por_segundo: velocidad_de_lectura(semillas()),
        familias: NotasDeFamilia {
            oficio: nota_de_familia(&resumenes, Familia::Oficio),
            criterio: nota_de_familia(&resumenes, Familia::Criterio),
            uso: nota_de_familia(&resumenes, Familia::Uso),
        },
        retirado_el: None,
        motivo_de_retirada: None,
        codigo_puntuado_con: codigo_puntuado_con(encargos),
        clase: calcular_clases(encargos)
            .into_iter()
            .map(|c| NotaDeClase {
                clase: c.fila,
                nota: c.nota_media,
                inventados: c.inventados,
                tokens_por_segundo: c.tokens_por_segundo,
            })
            .collect(),
    })
}

/// Cómo se puntuaron los encargos de código de la tanda, mirando las semillas que lo dicen. Las
/// que no lo dicen (no se evaluaron, o la tanda es de antes del 13/11/2026) no cuentan, y las que
/// no entregaron solo cuentan si no entregó ninguna; si no lo dice ninguna, no se sabe.
fn codigo_puntuado_con(encargos: &[ResultadoDeEncargo]) -> Option<CodigoPuntuadoCon> {
    let formas: Vec<PuntuadaCon> = encargos
        .iter()
        .filter(|e| forma_de_puntuar(e.familia, e.clase) == Some(FormaDePuntuar::Comprobaciones))
        .flat_map(|e| e.semillas.iter())
        .filter_map(|s| s.puntuada_con)
        .collect();
    match (
        formas.contains(&PuntuadaCon::PruebasOcultas),
        formas.contains(&PuntuadaCon::Rubrica),
    ) {
        (true, true) => Some(CodigoPuntuadoCon::Mixto),
        (true, false) => Some(CodigoPuntuadoCon::PruebasOcultas),
        (false, true) => Some(CodigoPuntuadoCon::Rubrica),
        (false, false) if formas.contains(&PuntuadaCon::SinEntrega) => {
            Some(CodigoPuntuadoCon::SinEntregas)
        }
        (false, false) => None,
    }
}

/// Mete la entrada en el registro: sustituye la del mismo modelo en la misma tarjeta, o la añade.
pub fn anotar(registro: &mut Registro, entrada: EntradaDeModelo) -> Anotacion {
    match registro.modelo.iter_mut().find(|e| e.misma_clave(&entrada)) {
        Some(anterior) => {
            let cambios = cambios(anterior, &entrada);
            *anterior = entrada;
            Anotacion::Sustituye(cambios)
        }
        None => {
            registro.modelo.push(entrada);
            Anotacion::Nueva
        }
    }
}

/// Las cifras que cambian de una entrada a otra del mismo modelo y la misma tarjeta.
fn cambios(antes: &EntradaDeModelo, despues: &EntradaDeModelo) -> Vec<Cambio> {
    let texto = |s: &str| Valor::Texto(s.to_string());
    let entera = |n: Option<u32>| Valor::Cifra(n.map(f64::from));
    let mut pares = vec![
        (QueCambia::Tanda, texto(&antes.tanda), texto(&despues.tanda)),
        (
            QueCambia::CompilacionDelMotor,
            texto(&antes.compilacion_motor),
            texto(&despues.compilacion_motor),
        ),
        (
            QueCambia::VersionDeProgramator,
            texto(&antes.version_programator),
            texto(&despues.version_programator),
        ),
        (
            QueCambia::CapasEnGpu,
            entera(antes.capas_en_gpu),
            entera(despues.capas_en_gpu),
        ),
        (
            QueCambia::Contexto,
            entera(Some(antes.contexto)),
            entera(Some(despues.contexto)),
        ),
        (
            QueCambia::Generando,
            Valor::Cifra(antes.generando_tokens_por_segundo),
            Valor::Cifra(despues.generando_tokens_por_segundo),
        ),
        (
            QueCambia::Leyendo,
            Valor::Cifra(antes.leyendo_tokens_por_segundo),
            Valor::Cifra(despues.leyendo_tokens_por_segundo),
        ),
    ];
    for (familia, en_antes, en_despues) in [
        (
            Familia::Oficio,
            antes.familias.oficio,
            despues.familias.oficio,
        ),
        (
            Familia::Criterio,
            antes.familias.criterio,
            despues.familias.criterio,
        ),
        (Familia::Uso, antes.familias.uso, despues.familias.uso),
    ] {
        pares.push((
            QueCambia::Familia(familia),
            Valor::Cifra(en_antes),
            Valor::Cifra(en_despues),
        ));
    }
    for nueva in &despues.clase {
        let vieja = antes
            .clase
            .iter()
            .find(|c| c.clase == nueva.clase)
            .and_then(|c| c.nota);
        pares.push((
            QueCambia::Clase(nueva.clase),
            Valor::Cifra(vieja),
            Valor::Cifra(nueva.nota),
        ));
    }
    pares
        .into_iter()
        .filter(|(_, a, d)| a != d)
        .map(|(que, antes, despues)| Cambio {
            que,
            antes,
            despues,
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::pruebas::tanda_de_ejemplo;
    use crate::banco::resultados::{AjustesDelMotor, EstadoDeNota, FilaDeDesempeno};

    const NOMBRE: &str = "2026-11-12-1219-devstral";

    /// La tanda de ejemplo, con todo lo que hace falta para anotarla.
    fn tanda_completa() -> ResultadosDeTanda {
        let mut resultados = tanda_de_ejemplo();
        for semilla in resultados
            .encargos
            .iter_mut()
            .flat_map(|e| e.semillas.iter_mut())
            .filter(|s| s.estado == EstadoDeNota::Pendiente)
        {
            semilla.estado = EstadoDeNota::Evaluada;
            semilla.nota = Some(4.0);
        }
        resultados.tanda.tarjeta = Some("NVIDIA de prueba".into());
        resultados.tanda.ajustes_del_motor = Some(AjustesDelMotor {
            contexto: 32768,
            cache_kv: "q8_0".into(),
            paralelo: 1,
            empezar_en_frio: true,
            top_k: 40,
            top_p: 0.95,
            min_p: 0.05,
            penalizacion_repeticion: 1.0,
        });
        resultados
    }

    /// La tanda completa con las dos semillas de E03 (oficio) puntuadas como se diga.
    fn con_el_codigo_puntuado(
        primera: Option<PuntuadaCon>,
        segunda: Option<PuntuadaCon>,
    ) -> ResultadosDeTanda {
        let mut resultados = tanda_completa();
        let oficio = &mut resultados.encargos[0];
        assert_eq!(oficio.id, "E03");
        oficio.semillas[0].puntuada_con = primera;
        oficio.semillas[1].puntuada_con = segunda;
        resultados
    }

    #[test]
    fn la_entrada_dice_como_se_puntuo_el_codigo_de_la_tanda() {
        let casos = [
            (
                Some(PuntuadaCon::PruebasOcultas),
                None,
                Some(CodigoPuntuadoCon::PruebasOcultas),
            ),
            (
                Some(PuntuadaCon::Rubrica),
                Some(PuntuadaCon::Rubrica),
                Some(CodigoPuntuadoCon::Rubrica),
            ),
            (
                Some(PuntuadaCon::PruebasOcultas),
                Some(PuntuadaCon::Rubrica),
                Some(CodigoPuntuadoCon::Mixto),
            ),
            (None, None, None),
            (
                Some(PuntuadaCon::SinEntrega),
                Some(PuntuadaCon::SinEntrega),
                Some(CodigoPuntuadoCon::SinEntregas),
            ),
            (
                Some(PuntuadaCon::SinEntrega),
                Some(PuntuadaCon::PruebasOcultas),
                Some(CodigoPuntuadoCon::PruebasOcultas),
            ),
            (
                Some(PuntuadaCon::Rubrica),
                Some(PuntuadaCon::SinEntrega),
                Some(CodigoPuntuadoCon::Rubrica),
            ),
            (
                Some(PuntuadaCon::SinEntrega),
                None,
                Some(CodigoPuntuadoCon::SinEntregas),
            ),
        ];
        for (primera, segunda, esperada) in casos {
            let entrada =
                entrada_de_la_tanda(&con_el_codigo_puntuado(primera, segunda), NOMBRE).unwrap();
            assert_eq!(
                entrada.codigo_puntuado_con, esperada,
                "{primera:?} y {segunda:?}"
            );
        }
    }

    #[test]
    fn la_forma_de_un_encargo_que_no_es_de_codigo_no_cuenta() {
        let mut resultados = con_el_codigo_puntuado(Some(PuntuadaCon::PruebasOcultas), None);
        // E01, de criterio, no es de código: aunque una semilla trajera el dato, no cuenta.
        let criterio = &mut resultados.encargos[2];
        assert_eq!(criterio.id, "E01");
        criterio.semillas[0].puntuada_con = Some(PuntuadaCon::Rubrica);

        let entrada = entrada_de_la_tanda(&resultados, NOMBRE).unwrap();

        assert_eq!(
            entrada.codigo_puntuado_con,
            Some(CodigoPuntuadoCon::PruebasOcultas)
        );
    }

    #[test]
    fn una_tanda_completa_da_su_entrada_con_las_cifras_del_informe() {
        let resultados = tanda_completa();

        let entrada = entrada_de_la_tanda(&resultados, NOMBRE).unwrap();

        assert_eq!(entrada.nombre, resultados.tanda.modelo);
        assert_eq!(entrada.tanda, NOMBRE);
        assert_eq!(entrada.tarjeta, "NVIDIA de prueba");
        assert_eq!(entrada.contexto, 32768);
        assert_eq!(entrada.familias.criterio, Some(4.0));
        assert_eq!(entrada.familias.uso, Some(4.5));
        let clases = calcular_clases(&resultados.encargos);
        assert_eq!(entrada.clase.len(), clases.len());
        for (nota, clase) in entrada.clase.iter().zip(&clases) {
            assert_eq!(nota.clase, clase.fila);
            assert_eq!(nota.nota, clase.nota_media);
        }
    }

    #[test]
    fn una_clase_que_no_se_midio_queda_sin_nota() {
        let entrada = entrada_de_la_tanda(&tanda_completa(), NOMBRE).unwrap();

        let diagnostico = entrada
            .clase
            .iter()
            .find(|c| c.clase == FilaDeDesempeno::Diagnostico)
            .unwrap();

        assert_eq!(diagnostico.nota, None);
    }

    #[test]
    fn con_rubricas_por_puntuar_no_se_anota() {
        let mut resultados = tanda_completa();
        resultados.encargos[2].semillas[0].estado = EstadoDeNota::Pendiente;

        assert_eq!(
            entrada_de_la_tanda(&resultados, NOMBRE),
            Err(MotivoParaNoAnotar::QuedanRubricas(1))
        );
    }

    #[test]
    fn sin_tarjeta_no_se_anota() {
        let mut resultados = tanda_completa();
        resultados.tanda.tarjeta = None;

        assert_eq!(
            entrada_de_la_tanda(&resultados, NOMBRE),
            Err(MotivoParaNoAnotar::FaltaLaTarjeta)
        );
    }

    #[test]
    fn sin_ajustes_del_motor_no_se_anota() {
        let mut resultados = tanda_completa();
        resultados.tanda.ajustes_del_motor = None;

        assert_eq!(
            entrada_de_la_tanda(&resultados, NOMBRE),
            Err(MotivoParaNoAnotar::FaltanLosAjustesDelMotor)
        );
    }

    #[test]
    fn la_primera_vez_la_entrada_es_nueva() {
        let mut registro = Registro::default();
        let entrada = entrada_de_la_tanda(&tanda_completa(), NOMBRE).unwrap();

        assert_eq!(anotar(&mut registro, entrada), Anotacion::Nueva);
        assert_eq!(registro.modelo.len(), 1);
    }

    #[test]
    fn el_mismo_modelo_en_la_misma_tarjeta_sustituye_y_dice_que_cambio() {
        let mut registro = Registro::default();
        let primera = entrada_de_la_tanda(&tanda_completa(), NOMBRE).unwrap();
        let mut segunda = primera.clone();
        segunda.tanda = "otra-tanda".into();
        segunda.familias.uso = Some(3.0);
        anotar(&mut registro, primera);

        let anotacion = anotar(&mut registro, segunda);

        assert_eq!(
            anotacion,
            Anotacion::Sustituye(vec![
                Cambio {
                    que: QueCambia::Tanda,
                    antes: Valor::Texto(NOMBRE.into()),
                    despues: Valor::Texto("otra-tanda".into()),
                },
                Cambio {
                    que: QueCambia::Familia(Familia::Uso),
                    antes: Valor::Cifra(Some(4.5)),
                    despues: Valor::Cifra(Some(3.0)),
                },
            ])
        );
        assert_eq!(registro.modelo.len(), 1);
        assert_eq!(registro.modelo[0].tanda, "otra-tanda");
    }

    #[test]
    fn el_mismo_modelo_en_otra_tarjeta_es_otra_entrada() {
        let mut registro = Registro::default();
        let primera = entrada_de_la_tanda(&tanda_completa(), NOMBRE).unwrap();
        let mut otra = primera.clone();
        otra.tarjeta = "otra tarjeta".into();
        anotar(&mut registro, primera);

        assert_eq!(anotar(&mut registro, otra), Anotacion::Nueva);
        assert_eq!(registro.modelo.len(), 2);
    }
}
