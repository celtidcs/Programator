//! Si dos tandas guardadas son repetición una de otra.
//!
//! **Por qué existe.** La misma prueba tiene que dar el mismo resultado. Afirmarlo exige poder comprobarlo: esta pieza compara dos tandas y dice, pasada a
//! pasada, si la conversación fue la misma. Es lo que cierra el plan de reproducibilidad y lo que
//! usará la revisión mensual.
//!
//! **Dos preguntas, en este orden.** Primero, si las tandas son comparables: con otro modelo, otra
//! compilación del motor u otro reparto de capas, que salgan distintas no dice nada. Después,
//! pasada a pasada, si la conversación coincide, y si no, en qué vuelta se separa.
//!
//! Función pura: recibe lo ya leído de disco y no toca nada. Quién lee está en `tanda.rs` y quién
//! redacta, en `mensajes.rs`.

use std::collections::BTreeMap;

use super::resultados::DatosDeTanda;

/// Una pasada, por encargo y semilla.
pub type ClaveDePasada = (String, i64);

/// Lo que hace falta de una tanda para compararla.
#[derive(Debug, Clone)]
pub struct TandaParaComparar {
    pub datos: DatosDeTanda,
    pub pasadas: BTreeMap<ClaveDePasada, PasadaParaComparar>,
}

/// Una pasada: su conversación, tal como quedó en disco, y si la cortó el tiempo máximo.
#[derive(Debug, Clone)]
pub struct PasadaParaComparar {
    /// `None` si la pasada no dejó conversación.
    pub conversacion: Option<String>,
    pub a_tiempo: bool,
}

/// Lo de la configuración en que se distinguen dos tandas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    Modelo,
    HuellaDelModelo,
    CompilacionDelMotor,
    VersionDeProgramator,
    Semillas,
    Temperatura,
    TiempoMaximo,
    CapasEnGpu,
    AjustesDelMotor,
    Tarjeta,
}

/// Un campo que no coincide, con su valor en cada tanda.
#[derive(Debug, Clone, PartialEq)]
pub struct Diferencia {
    pub campo: Campo,
    pub en_a: String,
    pub en_b: String,
}

/// Cómo salió una pasada al compararla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoDePasada {
    Identica,
    /// Las conversaciones se separan en esa línea del registro. Va la vuelta si se pudo leer.
    Diverge {
        linea: usize,
        vuelta: Option<u64>,
    },
    SoloEnA,
    SoloEnB,
    /// Alguna de las dos no dejó conversación.
    SinConversacion,
}

/// El resultado de una pasada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparacionDePasada {
    pub encargo: String,
    pub semilla: i64,
    pub estado: EstadoDePasada,
    /// Si el tiempo máximo cortó alguna de las dos. El corte depende del reloj y no de la
    /// semilla, así que una pasada cortada puede no repetirse aunque todo lo demás sea igual.
    pub cortada_por_tiempo: bool,
}

/// El veredicto entero.
#[derive(Debug, Clone, PartialEq)]
pub struct Veredicto {
    pub diferencias: Vec<Diferencia>,
    /// Si alguna tanda no guardó los ajustes del motor: no se pueden comparar y se dice.
    pub faltan_ajustes_del_motor: bool,
    pub pasadas: Vec<ComparacionDePasada>,
}

impl Veredicto {
    /// Las dos tandas son comparables y todas sus pasadas coinciden.
    pub fn es_repeticion(&self) -> bool {
        self.diferencias.is_empty()
            && !self.faltan_ajustes_del_motor
            && self
                .pasadas
                .iter()
                .all(|p| p.estado == EstadoDePasada::Identica)
    }

    pub fn identicas(&self) -> usize {
        self.pasadas
            .iter()
            .filter(|p| p.estado == EstadoDePasada::Identica)
            .count()
    }
}

/// Compara dos tandas.
pub fn comparar(a: &TandaParaComparar, b: &TandaParaComparar) -> Veredicto {
    let mut claves: Vec<&ClaveDePasada> = a.pasadas.keys().chain(b.pasadas.keys()).collect();
    claves.sort();
    claves.dedup();

    let pasadas = claves
        .into_iter()
        .map(|clave| {
            let (en_a, en_b) = (a.pasadas.get(clave), b.pasadas.get(clave));
            ComparacionDePasada {
                encargo: clave.0.clone(),
                semilla: clave.1,
                estado: estado_de_pasada(en_a, en_b),
                cortada_por_tiempo: [en_a, en_b].iter().flatten().any(|p| !p.a_tiempo),
            }
        })
        .collect();

    Veredicto {
        diferencias: diferencias(&a.datos, &b.datos),
        faltan_ajustes_del_motor: a.datos.ajustes_del_motor.is_none()
            || b.datos.ajustes_del_motor.is_none(),
        pasadas,
    }
}

fn estado_de_pasada(
    a: Option<&PasadaParaComparar>,
    b: Option<&PasadaParaComparar>,
) -> EstadoDePasada {
    match (a, b) {
        (Some(_), None) => EstadoDePasada::SoloEnA,
        (None, _) => EstadoDePasada::SoloEnB,
        (Some(a), Some(b)) => match (&a.conversacion, &b.conversacion) {
            (Some(a), Some(b)) => comparar_conversaciones(a, b),
            _ => EstadoDePasada::SinConversacion,
        },
    }
}

/// Compara dos registros de conversación línea a línea: cada línea es un turno.
fn comparar_conversaciones(a: &str, b: &str) -> EstadoDePasada {
    let (mut lineas_a, mut lineas_b) = (a.lines(), b.lines());
    let mut linea = 0;
    loop {
        linea += 1;
        match (lineas_a.next(), lineas_b.next()) {
            (None, None) => return EstadoDePasada::Identica,
            (en_a, en_b) if en_a == en_b => continue,
            (en_a, en_b) => {
                return EstadoDePasada::Diverge {
                    linea,
                    vuelta: en_a.or(en_b).and_then(vuelta_de_la_linea),
                }
            }
        }
    }
}

/// La vuelta del ciclo que anota cada línea del registro, si se puede leer.
fn vuelta_de_la_linea(linea: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(linea)
        .ok()?
        .get("vuelta")?
        .as_u64()
}

/// Los campos de configuración que no coinciden. No cuentan la fecha, la máquina de
/// comprobación, la VRAM ni el tiempo de carga: son medidas o lugares, no causas.
fn diferencias(a: &DatosDeTanda, b: &DatosDeTanda) -> Vec<Diferencia> {
    let pares = [
        (Campo::Modelo, a.modelo.clone(), b.modelo.clone()),
        (
            Campo::HuellaDelModelo,
            a.huella_sha256.clone(),
            b.huella_sha256.clone(),
        ),
        (
            Campo::CompilacionDelMotor,
            a.compilacion_motor.clone(),
            b.compilacion_motor.clone(),
        ),
        (
            Campo::VersionDeProgramator,
            a.version_programator.clone(),
            b.version_programator.clone(),
        ),
        (
            Campo::Semillas,
            format!("{:?}", a.semillas),
            format!("{:?}", b.semillas),
        ),
        (
            Campo::Temperatura,
            a.temperatura.to_string(),
            b.temperatura.to_string(),
        ),
        (
            Campo::TiempoMaximo,
            a.tiempo_maximo_encargo_segundos.to_string(),
            b.tiempo_maximo_encargo_segundos.to_string(),
        ),
        (
            Campo::CapasEnGpu,
            opcional(a.capas_en_gpu),
            opcional(b.capas_en_gpu),
        ),
    ];
    let mut diferencias: Vec<Diferencia> = pares
        .into_iter()
        .filter(|(_, en_a, en_b)| en_a != en_b)
        .map(|(campo, en_a, en_b)| Diferencia { campo, en_a, en_b })
        .collect();
    // Lo que las tandas antiguas no guardaban solo se compara si las dos lo tienen: que falte ya
    // lo avisa el veredicto, y contarlo como diferencia lo diría dos veces.
    diferencias.extend(diferencia_si_las_dos_lo_tienen(
        Campo::AjustesDelMotor,
        a.ajustes_del_motor.as_ref(),
        b.ajustes_del_motor.as_ref(),
        |ajustes| format!("{ajustes:?}"),
    ));
    diferencias.extend(diferencia_si_las_dos_lo_tienen(
        Campo::Tarjeta,
        a.tarjeta.as_ref(),
        b.tarjeta.as_ref(),
        String::clone,
    ));
    diferencias
}

fn diferencia_si_las_dos_lo_tienen<T: PartialEq>(
    campo: Campo,
    en_a: Option<&T>,
    en_b: Option<&T>,
    mostrar: impl Fn(&T) -> String,
) -> Option<Diferencia> {
    let (en_a, en_b) = en_a.zip(en_b)?;
    (en_a != en_b).then(|| Diferencia {
        campo,
        en_a: mostrar(en_a),
        en_b: mostrar(en_b),
    })
}

fn opcional(valor: Option<u32>) -> String {
    valor.map_or_else(|| "-".into(), |v| v.to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::AjustesDelMotor;

    fn datos() -> DatosDeTanda {
        DatosDeTanda {
            modelo: "devstral".into(),
            huella_sha256: "ab".repeat(32),
            compilacion_motor: "b10993".into(),
            version_programator: "0.11.0".into(),
            fecha: "2026-11-12 13:00".into(),
            semillas: vec![42],
            temperatura: 0.2,
            margen_lineas: 2,
            tiempo_maximo_encargo_segundos: 900,
            maquina_de_comprobacion: None,
            capas_en_gpu: Some(34),
            vram_ocupada_mib: Some(14_000),
            segundos_de_carga: Some(8.0),
            tarjeta: Some("NVIDIA de prueba".into()),
            sistema_de_comprobacion: None,
            rastro_fuera: Vec::new(),
            aislamiento: None,
            ajustes_del_motor: Some(AjustesDelMotor {
                contexto: 32768,
                cache_kv: "q8_0".into(),
                paralelo: 1,
                empezar_en_frio: true,
                top_k: 40,
                top_p: 0.95,
                min_p: 0.05,
                penalizacion_repeticion: 1.0,
            }),
        }
    }

    const CONVERSACION: &str = "{\"papel\":\"sistema\",\"vuelta\":0}\n\
                                {\"papel\":\"modelo\",\"vuelta\":1}\n\
                                {\"papel\":\"modelo\",\"vuelta\":2}\n";

    fn tanda(pasadas: &[(&str, i64, Option<&str>, bool)]) -> TandaParaComparar {
        TandaParaComparar {
            datos: datos(),
            pasadas: pasadas
                .iter()
                .map(|(encargo, semilla, conversacion, a_tiempo)| {
                    (
                        (encargo.to_string(), *semilla),
                        PasadaParaComparar {
                            conversacion: conversacion.map(str::to_string),
                            a_tiempo: *a_tiempo,
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn dos_tandas_iguales_son_repeticion() {
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);

        let veredicto = comparar(&a, &a.clone());

        assert!(veredicto.es_repeticion(), "{veredicto:?}");
        assert_eq!(veredicto.identicas(), 1);
    }

    #[test]
    fn una_vuelta_cambiada_dice_en_cual_se_separan() {
        let cambiada = CONVERSACION.replace(
            "{\"papel\":\"modelo\",\"vuelta\":2}",
            "{\"papel\":\"modelo\",\"vuelta\":2,\"contenido\":\"otro\"}",
        );
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);
        let b = tanda(&[("E01", 42, Some(&cambiada), true)]);

        let veredicto = comparar(&a, &b);

        assert!(!veredicto.es_repeticion());
        assert_eq!(
            veredicto.pasadas[0].estado,
            EstadoDePasada::Diverge {
                linea: 3,
                vuelta: Some(2)
            }
        );
    }

    #[test]
    fn una_conversacion_mas_larga_diverge_donde_acaba_la_corta() {
        let corta = CONVERSACION.lines().take(2).collect::<Vec<_>>().join("\n");
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);
        let b = tanda(&[("E01", 42, Some(&corta), true)]);

        assert_eq!(
            comparar(&a, &b).pasadas[0].estado,
            EstadoDePasada::Diverge {
                linea: 3,
                vuelta: Some(2)
            }
        );
    }

    #[test]
    fn otras_capas_hacen_las_tandas_no_comparables_y_se_dice_por_que() {
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);
        let mut b = a.clone();
        b.datos.capas_en_gpu = Some(35);

        let veredicto = comparar(&a, &b);

        assert!(!veredicto.es_repeticion());
        assert_eq!(
            veredicto.diferencias,
            vec![Diferencia {
                campo: Campo::CapasEnGpu,
                en_a: "34".into(),
                en_b: "35".into()
            }]
        );
    }

    #[test]
    fn la_fecha_la_vram_y_la_maquina_no_cuentan_como_diferencia() {
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);
        let mut b = a.clone();
        b.datos.fecha = "otra".into();
        b.datos.vram_ocupada_mib = Some(1);
        b.datos.segundos_de_carga = Some(99.0);
        b.datos.maquina_de_comprobacion = Some("maquina-linux".into());

        assert!(comparar(&a, &b).es_repeticion());
    }

    #[test]
    fn otros_ajustes_del_motor_son_una_diferencia() {
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);
        let mut b = a.clone();
        if let Some(ajustes) = &mut b.datos.ajustes_del_motor {
            ajustes.empezar_en_frio = false;
        }

        let veredicto = comparar(&a, &b);

        assert_eq!(veredicto.diferencias[0].campo, Campo::AjustesDelMotor);
    }

    #[test]
    fn otra_tarjeta_es_una_diferencia() {
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);
        let mut b = a.clone();
        b.datos.tarjeta = Some("otra tarjeta".into());

        let veredicto = comparar(&a, &b);

        assert_eq!(veredicto.diferencias[0].campo, Campo::Tarjeta);
        assert_eq!(veredicto.diferencias[0].en_b, "otra tarjeta");
        assert!(!veredicto.es_repeticion());
    }

    #[test]
    fn una_tanda_sin_tarjeta_guardada_no_cuenta_como_diferencia() {
        // Las tandas anteriores al 12/11/2026 no la guardan, y eso no las hace distintas.
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);
        let mut b = a.clone();
        b.datos.tarjeta = None;

        let veredicto = comparar(&a, &b);

        assert!(veredicto.diferencias.is_empty());
    }

    #[test]
    fn una_tanda_sin_ajustes_guardados_no_se_da_por_repeticion() {
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);
        let mut b = a.clone();
        b.datos.ajustes_del_motor = None;

        let veredicto = comparar(&a, &b);

        assert!(veredicto.faltan_ajustes_del_motor);
        assert!(!veredicto.es_repeticion());
    }

    #[test]
    fn una_pasada_que_falta_en_una_de_las_dos_se_dice() {
        let a = tanda(&[
            ("E01", 42, Some(CONVERSACION), true),
            ("E02", 42, Some(CONVERSACION), true),
        ]);
        let b = tanda(&[
            ("E01", 42, Some(CONVERSACION), true),
            ("E03", 42, Some(CONVERSACION), true),
        ]);

        let estados: Vec<EstadoDePasada> = comparar(&a, &b)
            .pasadas
            .into_iter()
            .map(|p| p.estado)
            .collect();

        assert_eq!(
            estados,
            vec![
                EstadoDePasada::Identica,
                EstadoDePasada::SoloEnA,
                EstadoDePasada::SoloEnB
            ]
        );
    }

    #[test]
    fn una_pasada_sin_conversacion_no_se_da_por_identica() {
        let a = tanda(&[("E01", 42, None, true)]);

        let veredicto = comparar(&a, &a.clone());

        assert_eq!(veredicto.pasadas[0].estado, EstadoDePasada::SinConversacion);
        assert!(!veredicto.es_repeticion());
    }

    #[test]
    fn se_marca_la_pasada_que_corto_el_tiempo_maximo() {
        let a = tanda(&[("E01", 42, Some(CONVERSACION), true)]);
        let b = tanda(&[("E01", 42, Some(CONVERSACION), false)]);

        assert!(comparar(&a, &b).pasadas[0].cortada_por_tiempo);
    }
}
