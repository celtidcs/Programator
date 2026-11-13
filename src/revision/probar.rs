//! `revisión: probar`: descargar cada candidato, soltar el motor propio, pasarle el banco y
//! comprobar la tanda (especificación de la revisión autónoma, §3 a §7).
//!
//! Lo que toca el mundo va en el rasgo [`Taller`], y aquí solo se decide el recorrido: así se
//! prueba sin GPU ni red.

use std::path::{Path, PathBuf};

use super::encargo::CandidatoPedido;
use super::mensajes;
use super::probados::{Estado, Probado, Probados};
use crate::error::Resultado;

/// Lo que `probar` necesita del mundo.
pub trait Taller {
    /// Descarga el candidato, o lo encuentra ya descargado, y devuelve la ruta del modelo.
    fn descargar(&mut self, candidato: &CandidatoPedido) -> Resultado<PathBuf>;
    /// Para el motor propio y espera a que la tarjeta quede libre. Falla si en el puerto escucha
    /// un motor que no es de Programator: no se mata un proceso de otro.
    fn soltar_motor(&mut self) -> Resultado<()>;
    /// La fase 1 del banco con este modelo. Devuelve la carpeta de la tanda.
    fn pasar_tanda(&mut self, modelo: &Path) -> Resultado<PathBuf>;
    /// La fase 2 del banco: las pruebas ocultas en este equipo.
    fn comprobar_tanda(&mut self, tanda: &Path) -> Resultado<()>;
    /// Lo que se va haciendo, para la terminal y el latido.
    fn avisar(&mut self, linea: &str);
    fn guardar_lo_probado(&mut self, probados: &Probados) -> Resultado<()>;
}

/// Lo que pasó en un `revisión: probar`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prueba {
    /// Cada candidato probado, en el orden pedido.
    pub probados: Vec<Probado>,
    /// Los que pasaban del tope y no se probaron.
    pub fuera_del_tope: Vec<CandidatoPedido>,
}

/// Prueba los candidatos pedidos, hasta `tope` (`[revision] candidatos_por_revision`), y apunta
/// cada uno en `probados` en cuanto se sabe en qué quedó.
pub fn probar(
    pedidos: &[CandidatoPedido],
    tope: usize,
    probados: &mut Probados,
    taller: &mut dyn Taller,
) -> Resultado<Prueba> {
    let (dentro, fuera) = pedidos.split_at(pedidos.len().min(tope));
    let mut hechos = Vec::new();
    for (indice, candidato) in dentro.iter().enumerate() {
        taller.avisar(&mensajes::probando(candidato, indice + 1, dentro.len()));
        let probado = probar_uno(candidato, taller)?;
        probados.anotar(probado.clone());
        taller.guardar_lo_probado(probados)?;
        hechos.push(probado);
    }
    Ok(Prueba {
        probados: hechos,
        fuera_del_tope: fuera.to_vec(),
    })
}

/// Un candidato. Solo devuelve error si no se puede soltar el motor, porque entonces no se podría
/// probar ninguno; lo demás queda anotado en el candidato.
fn probar_uno(candidato: &CandidatoPedido, taller: &mut dyn Taller) -> Resultado<Probado> {
    let fallido = |motivo: String| Probado {
        repositorio: candidato.repositorio.clone(),
        fichero: candidato.fichero.clone(),
        estado: Estado::Fallido,
        tanda: None,
        motivo: Some(motivo),
    };
    let modelo = match taller.descargar(candidato) {
        Ok(modelo) => modelo,
        Err(fallo) => return Ok(fallido(fallo.to_string())),
    };
    taller.soltar_motor()?;
    let tanda = match taller.pasar_tanda(&modelo) {
        Ok(tanda) => tanda,
        Err(fallo) => return Ok(fallido(fallo.to_string())),
    };
    let (estado, motivo) = match taller.comprobar_tanda(&tanda) {
        Ok(()) => (Estado::Comprobada, None),
        Err(fallo) => (Estado::SinComprobar, Some(fallo.to_string())),
    };
    Ok(Probado {
        repositorio: candidato.repositorio.clone(),
        fichero: candidato.fichero.clone(),
        estado,
        tanda: Some(tanda),
        motivo,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::error::Error;
    use std::collections::VecDeque;

    /// Un taller que apunta lo que se le pide y contesta lo que se le diga.
    #[derive(Default)]
    struct TallerFalso {
        llamadas: Vec<String>,
        descargas_que_fallan: Vec<String>,
        tandas_que_fallan: Vec<String>,
        comprobaciones: VecDeque<Resultado<()>>,
        soltar_falla: bool,
        guardados: usize,
    }

    impl Taller for TallerFalso {
        fn descargar(&mut self, candidato: &CandidatoPedido) -> Resultado<PathBuf> {
            self.llamadas
                .push(format!("descargar {}", candidato.fichero));
            if self.descargas_que_fallan.contains(&candidato.fichero) {
                return Err(Error::Revision("la huella no coincide".into()));
            }
            Ok(PathBuf::from("modelos").join(&candidato.fichero))
        }
        fn soltar_motor(&mut self) -> Resultado<()> {
            self.llamadas.push("soltar".into());
            if self.soltar_falla {
                return Err(Error::Revision("hay un motor ajeno".into()));
            }
            Ok(())
        }
        fn pasar_tanda(&mut self, modelo: &Path) -> Resultado<PathBuf> {
            let nombre = modelo.file_name().unwrap().to_string_lossy().into_owned();
            self.llamadas.push(format!("pasar {nombre}"));
            if self.tandas_que_fallan.contains(&nombre) {
                return Err(Error::Banco("el motor no cargó".into()));
            }
            Ok(PathBuf::from("banco/resultados").join(nombre))
        }
        fn comprobar_tanda(&mut self, _tanda: &Path) -> Resultado<()> {
            self.llamadas.push("comprobar".into());
            self.comprobaciones.pop_front().unwrap_or(Ok(()))
        }
        fn avisar(&mut self, _linea: &str) {}
        fn guardar_lo_probado(&mut self, _probados: &Probados) -> Resultado<()> {
            self.guardados += 1;
            Ok(())
        }
    }

    fn candidato(fichero: &str) -> CandidatoPedido {
        CandidatoPedido {
            repositorio: "autor/Modelo-GGUF".into(),
            fichero: fichero.into(),
        }
    }

    #[test]
    fn cada_candidato_se_descarga_suelta_el_motor_se_pasa_y_se_comprueba() {
        let mut taller = TallerFalso::default();
        let mut probados = Probados::default();

        let prueba = probar(
            &[candidato("a.gguf"), candidato("b.gguf")],
            3,
            &mut probados,
            &mut taller,
        )
        .unwrap();

        assert_eq!(
            taller.llamadas,
            [
                "descargar a.gguf",
                "soltar",
                "pasar a.gguf",
                "comprobar",
                "descargar b.gguf",
                "soltar",
                "pasar b.gguf",
                "comprobar"
            ]
        );
        assert!(prueba
            .probados
            .iter()
            .all(|p| p.estado == Estado::Comprobada));
        assert_eq!(probados.probados, prueba.probados);
        assert_eq!(taller.guardados, 2, "se guarda tras cada candidato");
    }

    #[test]
    fn los_que_pasan_del_tope_no_se_prueban_y_se_dicen() {
        let mut taller = TallerFalso::default();
        let prueba = probar(
            &[
                candidato("a.gguf"),
                candidato("b.gguf"),
                candidato("c.gguf"),
            ],
            2,
            &mut Probados::default(),
            &mut taller,
        )
        .unwrap();

        assert_eq!(prueba.probados.len(), 2);
        assert_eq!(prueba.fuera_del_tope, [candidato("c.gguf")]);
        assert!(!taller.llamadas.iter().any(|l| l.contains("c.gguf")));
    }

    #[test]
    fn una_descarga_o_una_tanda_que_falla_queda_anotada_y_se_sigue_con_el_siguiente() {
        let mut taller = TallerFalso {
            descargas_que_fallan: vec!["a.gguf".into()],
            tandas_que_fallan: vec!["b.gguf".into()],
            ..TallerFalso::default()
        };
        let prueba = probar(
            &[
                candidato("a.gguf"),
                candidato("b.gguf"),
                candidato("c.gguf"),
            ],
            3,
            &mut Probados::default(),
            &mut taller,
        )
        .unwrap();

        let estados: Vec<Estado> = prueba.probados.iter().map(|p| p.estado).collect();
        assert_eq!(
            estados,
            [Estado::Fallido, Estado::Fallido, Estado::Comprobada]
        );
        assert!(prueba.probados[0]
            .motivo
            .as_deref()
            .unwrap()
            .contains("huella"));
        assert!(prueba.probados[1]
            .motivo
            .as_deref()
            .unwrap()
            .contains("no cargó"));
        assert!(
            !taller.llamadas.contains(&"pasar a.gguf".to_string()),
            "sin modelo no se suelta el motor ni se pasa la tanda"
        );
    }

    #[test]
    fn si_la_comprobacion_falla_la_tanda_queda_sin_comprobar_y_se_dice_por_que() {
        let mut taller = TallerFalso {
            comprobaciones: VecDeque::from([Err(Error::Banco("el intérprete no responde".into()))]),
            ..TallerFalso::default()
        };
        let prueba = probar(
            &[candidato("a.gguf")],
            1,
            &mut Probados::default(),
            &mut taller,
        )
        .unwrap();

        let probado = &prueba.probados[0];
        assert_eq!(probado.estado, Estado::SinComprobar);
        assert!(probado.tanda.is_some(), "las pasadas quedan para cerrar");
        assert!(probado.motivo.as_deref().unwrap().contains("no responde"));
        assert_eq!(
            taller.llamadas.iter().filter(|l| *l == "comprobar").count(),
            1,
            "un fallo al comprobar no se reintenta"
        );
    }

    #[test]
    fn si_no_se_puede_soltar_el_motor_no_se_prueba_nada_mas() {
        let mut taller = TallerFalso {
            soltar_falla: true,
            ..TallerFalso::default()
        };
        let fallo = probar(
            &[candidato("a.gguf"), candidato("b.gguf")],
            3,
            &mut Probados::default(),
            &mut taller,
        )
        .unwrap_err();

        assert!(fallo.to_string().contains("motor ajeno"));
        assert!(!taller.llamadas.iter().any(|l| l.starts_with("pasar")));
    }
}
