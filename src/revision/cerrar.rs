//! `revisión: cerrar`: terminar las tandas que quedaron sin comprobar, rehacer los informes,
//! anotar las tandas en el registro y aplicar la regla: se queda quien gana algo, se retira quien no
//! gana nada y, si el modelo por defecto no gana nada y hay sustituto, se cambia (especificación de
//! la revisión autónoma, §8).
//!
//! Aquí solo se decide el recorrido. Lo que toca disco y procesos va en el rasgo [`Mesa`], y las
//! piezas de verdad son las mismas de `--banco-comprobar`, `--registro-anotar`,
//! `--revision-proponer` y `--retirar-modelo`.

use std::path::{Path, PathBuf};

use super::probados::{Estado, Probados};
use super::propuesta::{EnUsoSinVictorias, Propuesta};
use crate::error::Resultado;

/// Lo que `cerrar` necesita del mundo.
pub trait Mesa {
    /// La fase 2 del banco sobre una tanda guardada.
    fn comprobar_tanda(&mut self, tanda: &Path) -> Resultado<()>;
    /// Las rúbricas de la tanda que siguen sin nota.
    fn rubricas_sin_puntuar(&mut self, tanda: &Path) -> Resultado<Vec<PathBuf>>;
    fn rehacer_informe(&mut self, tanda: &Path) -> Resultado<()>;
    /// Anota la tanda en el registro. `Err` de dentro: por qué no se anota, que no para el cierre.
    fn anotar(&mut self, tanda: &Path) -> Resultado<Result<(), String>>;
    /// Escribe la propuesta del mes con ese modelo en uso y la devuelve.
    fn proponer(&mut self, modelo_en_uso: Option<&str>) -> Resultado<Propuesta>;
    /// Cambia `[motor] modelo` de `programator.toml` al modelo de ese nombre.
    fn cambiar_el_modelo_por_defecto(&mut self, nombre: &str) -> Resultado<()>;
    /// Retira el modelo de ese nombre. Devuelve los bytes que liberó.
    fn retirar(&mut self, nombre: &str, modelo_en_uso: Option<&str>) -> Resultado<u64>;
    fn guardar_lo_probado(&mut self, probados: &Probados) -> Resultado<()>;
}

/// Lo que pasó al cerrar.
#[derive(Debug, Clone, PartialEq)]
pub struct Cerrado {
    /// Las tandas que estaban sin comprobar y ahora sí lo están.
    pub comprobadas_ahora: Vec<PathBuf>,
    /// Las que siguen sin comprobar, con el motivo.
    pub sin_comprobar: Vec<(PathBuf, String)>,
    pub desenlace: Desenlace,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Desenlace {
    /// Quedan rúbricas sin nota: no se ha anotado, propuesto ni retirado nada.
    FaltanRubricas(Vec<PathBuf>),
    Aplicada(Regla),
}

/// La regla aplicada.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Regla {
    /// Las tandas que no se anotaron en el registro, con el motivo.
    pub sin_anotar: Vec<(PathBuf, String)>,
    pub se_quedan: Vec<String>,
    /// El modelo por defecto que se cambió, y por cuál.
    pub cambio_de_modelo: Option<(String, String)>,
    /// El modelo por defecto no gana nada, pero no hay quien lo sustituya.
    pub en_uso_sin_sustituto: Option<String>,
    /// Los retirados, con los bytes liberados.
    pub retirados: Vec<(String, u64)>,
    /// Los que tocaba retirar y no se pudo, con el motivo.
    pub sin_retirar: Vec<(String, String)>,
}

/// Cierra la revisión del mes. `modelo_en_uso` es el nombre, sin extensión, del modelo de
/// `programator.toml`.
pub fn cerrar(
    probados: &mut Probados,
    modelo_en_uso: Option<&str>,
    mesa: &mut dyn Mesa,
) -> Resultado<Cerrado> {
    let (comprobadas_ahora, sin_comprobar) = comprobar_lo_pendiente(probados, mesa)?;
    let tandas: Vec<PathBuf> = probados
        .probados
        .iter()
        .filter(|p| p.estado == Estado::Comprobada)
        .filter_map(|p| p.tanda.clone())
        .collect();
    let mut faltan = Vec::new();
    for tanda in &tandas {
        faltan.extend(mesa.rubricas_sin_puntuar(tanda)?);
    }
    let desenlace = if faltan.is_empty() {
        Desenlace::Aplicada(aplicar_la_regla(&tandas, modelo_en_uso, mesa)?)
    } else {
        Desenlace::FaltanRubricas(faltan)
    };
    Ok(Cerrado {
        comprobadas_ahora,
        sin_comprobar,
        desenlace,
    })
}

type Comprobacion = (Vec<PathBuf>, Vec<(PathBuf, String)>);

/// Una sola vez por tanda: si vuelve a fallar, ya se intentará en el siguiente `cerrar`.
fn comprobar_lo_pendiente(probados: &mut Probados, mesa: &mut dyn Mesa) -> Resultado<Comprobacion> {
    let mut comprobadas = Vec::new();
    let mut sin_comprobar = Vec::new();
    for probado in probados
        .probados
        .iter_mut()
        .filter(|p| p.estado == Estado::SinComprobar)
    {
        let Some(tanda) = probado.tanda.clone() else {
            continue;
        };
        match mesa.comprobar_tanda(&tanda) {
            Ok(()) => {
                probado.estado = Estado::Comprobada;
                probado.motivo = None;
                comprobadas.push(tanda);
            }
            Err(fallo) => {
                probado.motivo = Some(fallo.to_string());
                sin_comprobar.push((tanda, fallo.to_string()));
            }
        }
    }
    if !comprobadas.is_empty() || !sin_comprobar.is_empty() {
        mesa.guardar_lo_probado(probados)?;
    }
    Ok((comprobadas, sin_comprobar))
}

fn aplicar_la_regla(
    tandas: &[PathBuf],
    modelo_en_uso: Option<&str>,
    mesa: &mut dyn Mesa,
) -> Resultado<Regla> {
    let mut regla = Regla::default();
    for tanda in tandas {
        mesa.rehacer_informe(tanda)?;
        if let Err(motivo) = mesa.anotar(tanda)? {
            regla.sin_anotar.push((tanda.clone(), motivo));
        }
    }
    let mut en_uso = modelo_en_uso.map(str::to_string);
    let mut propuesta = mesa.proponer(en_uso.as_deref())?;
    match propuesta.en_uso_sin_victorias.clone() {
        Some(EnUsoSinVictorias {
            nombre,
            sustituto: Some(sustituto),
        }) => {
            mesa.cambiar_el_modelo_por_defecto(&sustituto)?;
            // Con el sustituto en uso, el viejo ya cuenta como cualquier otro: si no gana nada,
            // sale en las retiradas de la propuesta nueva, con su motivo.
            propuesta = mesa.proponer(Some(&sustituto))?;
            en_uso = Some(sustituto.clone());
            regla.cambio_de_modelo = Some((nombre, sustituto));
        }
        Some(EnUsoSinVictorias {
            nombre,
            sustituto: None,
        }) => regla.en_uso_sin_sustituto = Some(nombre),
        None => {}
    }
    for retirar in &propuesta.para_retirar {
        match mesa.retirar(&retirar.nombre, en_uso.as_deref()) {
            Ok(liberados) => regla.retirados.push((retirar.nombre.clone(), liberados)),
            Err(fallo) => regla
                .sin_retirar
                .push((retirar.nombre.clone(), fallo.to_string())),
        }
    }
    regla.se_quedan = propuesta
        .se_quedan
        .iter()
        .map(|q| q.nombre.clone())
        .collect();
    Ok(regla)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::ficha::Familia;
    use crate::error::Error;
    use crate::revision::probados::Probado;
    use crate::revision::propuesta::{Categoria, ParaRetirar, SeQueda, Vencedor};
    use std::collections::{BTreeMap, VecDeque};

    #[derive(Default)]
    struct MesaFalsa {
        llamadas: Vec<String>,
        comprobaciones: VecDeque<Resultado<()>>,
        sin_puntuar: BTreeMap<PathBuf, Vec<PathBuf>>,
        propuestas: VecDeque<Propuesta>,
        no_se_retira: Vec<String>,
    }

    impl Mesa for MesaFalsa {
        fn comprobar_tanda(&mut self, tanda: &Path) -> Resultado<()> {
            self.llamadas.push(format!("comprobar {}", tanda.display()));
            self.comprobaciones.pop_front().unwrap_or(Ok(()))
        }
        fn rubricas_sin_puntuar(&mut self, tanda: &Path) -> Resultado<Vec<PathBuf>> {
            Ok(self.sin_puntuar.get(tanda).cloned().unwrap_or_default())
        }
        fn rehacer_informe(&mut self, tanda: &Path) -> Resultado<()> {
            self.llamadas.push(format!("rehacer {}", tanda.display()));
            Ok(())
        }
        fn anotar(&mut self, tanda: &Path) -> Resultado<Result<(), String>> {
            self.llamadas.push(format!("anotar {}", tanda.display()));
            Ok(Ok(()))
        }
        fn proponer(&mut self, modelo_en_uso: Option<&str>) -> Resultado<Propuesta> {
            self.llamadas
                .push(format!("proponer con {}", modelo_en_uso.unwrap_or("nada")));
            Ok(self.propuestas.pop_front().unwrap_or_else(propuesta))
        }
        fn cambiar_el_modelo_por_defecto(&mut self, nombre: &str) -> Resultado<()> {
            self.llamadas.push(format!("por defecto {nombre}"));
            Ok(())
        }
        fn retirar(&mut self, nombre: &str, modelo_en_uso: Option<&str>) -> Resultado<u64> {
            self.llamadas.push(format!(
                "retirar {nombre} con {}",
                modelo_en_uso.unwrap_or("nada")
            ));
            if self.no_se_retira.iter().any(|n| n == nombre) {
                return Err(Error::Revision("no está en la carpeta".into()));
            }
            Ok(1000)
        }
        fn guardar_lo_probado(&mut self, _probados: &Probados) -> Resultado<()> {
            self.llamadas.push("guardar lo probado".into());
            Ok(())
        }
    }

    fn propuesta() -> Propuesta {
        Propuesta {
            tarjeta: "NVIDIA de prueba".into(),
            procedencia: Vec::new(),
            ganadores: Vec::new(),
            se_quedan: Vec::new(),
            para_retirar: Vec::new(),
            sin_medir: Vec::new(),
            en_uso_sin_victorias: None,
            excluidas: Vec::new(),
            sin_comparar: Vec::new(),
            sin_datos_para_decidir: false,
        }
    }

    fn se_queda(nombre: &str, en_uso: bool) -> SeQueda {
        SeQueda {
            nombre: nombre.into(),
            gana: if en_uso {
                Vec::new()
            } else {
                vec![Categoria::Familia(Familia::Oficio)]
            },
            en_uso,
        }
    }

    fn para_retirar(nombre: &str) -> ParaRetirar {
        ParaRetirar {
            nombre: nombre.into(),
            mejor_nota: 2.0,
            en: Categoria::Familia(Familia::Oficio),
            le_gana: Vencedor {
                nombre: "nuevo".into(),
                nota: 4.0,
            },
        }
    }

    fn probado(tanda: &str, estado: Estado) -> Probado {
        Probado {
            repositorio: "autor/Modelo-GGUF".into(),
            fichero: format!("{tanda}.gguf"),
            estado,
            tanda: Some(PathBuf::from(tanda)),
            motivo: None,
        }
    }

    fn probados(lista: Vec<Probado>) -> Probados {
        Probados { probados: lista }
    }

    #[test]
    fn se_rehacen_y_anotan_las_tandas_comprobadas_y_se_retira_lo_que_no_gana_nada() {
        let mut mesa = MesaFalsa {
            propuestas: VecDeque::from([Propuesta {
                se_quedan: vec![se_queda("devstral", true), se_queda("nuevo", false)],
                para_retirar: vec![para_retirar("viejo")],
                ..propuesta()
            }]),
            ..MesaFalsa::default()
        };
        let mut lo_probado = probados(vec![
            probado("t1", Estado::Comprobada),
            probado("t2", Estado::Fallido),
        ]);

        let cerrado = cerrar(&mut lo_probado, Some("devstral"), &mut mesa).unwrap();

        assert_eq!(
            mesa.llamadas,
            [
                "rehacer t1",
                "anotar t1",
                "proponer con devstral",
                "retirar viejo con devstral"
            ]
        );
        let Desenlace::Aplicada(regla) = cerrado.desenlace else {
            panic!("tenía que aplicarse la regla");
        };
        assert_eq!(regla.retirados, [("viejo".to_string(), 1000)]);
        assert_eq!(regla.se_quedan, ["devstral", "nuevo"]);
        assert_eq!(regla.cambio_de_modelo, None);
    }

    #[test]
    fn con_rubricas_sin_nota_no_se_anota_ni_se_retira_nada_y_se_dicen_cuales() {
        let mut mesa = MesaFalsa {
            sin_puntuar: BTreeMap::from([(
                PathBuf::from("t1"),
                vec![PathBuf::from("t1/rubricas/E01-42.md")],
            )]),
            ..MesaFalsa::default()
        };
        let mut lo_probado = probados(vec![probado("t1", Estado::Comprobada)]);

        let cerrado = cerrar(&mut lo_probado, Some("devstral"), &mut mesa).unwrap();

        assert_eq!(
            cerrado.desenlace,
            Desenlace::FaltanRubricas(vec![PathBuf::from("t1/rubricas/E01-42.md")])
        );
        assert!(mesa.llamadas.is_empty(), "{:?}", mesa.llamadas);
    }

    #[test]
    fn el_modelo_por_defecto_que_no_gana_nada_se_cambia_y_el_viejo_se_retira_despues() {
        let mut mesa = MesaFalsa {
            propuestas: VecDeque::from([
                Propuesta {
                    se_quedan: vec![se_queda("devstral", true), se_queda("nuevo", false)],
                    en_uso_sin_victorias: Some(EnUsoSinVictorias {
                        nombre: "devstral".into(),
                        sustituto: Some("nuevo".into()),
                    }),
                    ..propuesta()
                },
                Propuesta {
                    se_quedan: vec![se_queda("nuevo", false)],
                    para_retirar: vec![para_retirar("devstral")],
                    ..propuesta()
                },
            ]),
            ..MesaFalsa::default()
        };

        let cerrado = cerrar(&mut Probados::default(), Some("devstral"), &mut mesa).unwrap();

        assert_eq!(
            mesa.llamadas,
            [
                "proponer con devstral",
                "por defecto nuevo",
                "proponer con nuevo",
                "retirar devstral con nuevo"
            ]
        );
        let Desenlace::Aplicada(regla) = cerrado.desenlace else {
            panic!("tenía que aplicarse la regla");
        };
        assert_eq!(
            regla.cambio_de_modelo,
            Some(("devstral".to_string(), "nuevo".to_string()))
        );
        assert_eq!(regla.se_quedan, ["nuevo"]);
    }

    #[test]
    fn sin_sustituto_el_modelo_por_defecto_se_queda_y_se_dice() {
        let mut mesa = MesaFalsa {
            propuestas: VecDeque::from([Propuesta {
                se_quedan: vec![se_queda("devstral", true)],
                en_uso_sin_victorias: Some(EnUsoSinVictorias {
                    nombre: "devstral".into(),
                    sustituto: None,
                }),
                ..propuesta()
            }]),
            ..MesaFalsa::default()
        };

        let cerrado = cerrar(&mut Probados::default(), Some("devstral"), &mut mesa).unwrap();

        let Desenlace::Aplicada(regla) = cerrado.desenlace else {
            panic!("tenía que aplicarse la regla");
        };
        assert_eq!(regla.en_uso_sin_sustituto, Some("devstral".to_string()));
        assert!(!mesa.llamadas.iter().any(|l| l.starts_with("por defecto")));
    }

    #[test]
    fn lo_que_estaba_sin_comprobar_se_intenta_una_vez_y_lo_que_vuelve_a_fallar_no_cuenta() {
        let mut mesa = MesaFalsa {
            comprobaciones: VecDeque::from([
                Ok(()),
                Err(Error::Banco("el intérprete no responde".into())),
            ]),
            ..MesaFalsa::default()
        };
        let mut lo_probado = probados(vec![
            probado("t1", Estado::SinComprobar),
            probado("t2", Estado::SinComprobar),
        ]);

        let cerrado = cerrar(&mut lo_probado, Some("devstral"), &mut mesa).unwrap();

        assert_eq!(cerrado.comprobadas_ahora, [PathBuf::from("t1")]);
        assert_eq!(cerrado.sin_comprobar.len(), 1);
        assert!(cerrado.sin_comprobar[0].1.contains("no responde"));
        assert_eq!(lo_probado.probados[0].estado, Estado::Comprobada);
        assert_eq!(lo_probado.probados[1].estado, Estado::SinComprobar);
        assert!(mesa.llamadas.contains(&"guardar lo probado".to_string()));
        assert!(mesa.llamadas.contains(&"anotar t1".to_string()));
        assert!(!mesa.llamadas.contains(&"anotar t2".to_string()));
    }

    #[test]
    fn una_retirada_que_falla_no_para_las_demas() {
        let mut mesa = MesaFalsa {
            propuestas: VecDeque::from([Propuesta {
                para_retirar: vec![para_retirar("a"), para_retirar("b")],
                ..propuesta()
            }]),
            no_se_retira: vec!["a".into()],
            ..MesaFalsa::default()
        };

        let cerrado = cerrar(&mut Probados::default(), None, &mut mesa).unwrap();

        let Desenlace::Aplicada(regla) = cerrado.desenlace else {
            panic!("tenía que aplicarse la regla");
        };
        assert_eq!(regla.retirados, [("b".to_string(), 1000)]);
        assert_eq!(regla.sin_retirar.len(), 1);
        assert!(regla.sin_retirar[0].1.contains("no está en la carpeta"));
    }
}
