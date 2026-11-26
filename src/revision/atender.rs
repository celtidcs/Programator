//! Atender un pedido de revisión que llega por el canal: reconocerlo, ver si quien lo pide puede,
//! hacerlo y redactar lo que se publica.
//!
//! [`atender`] decide y se prueba con una revisión falsa; [`RevisorReal`] la monta con el mundo de
//! verdad para el ciclo de `main`.

use crate::protocolo::destinatarios::Identidad;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

use super::cerrar::cerrar;
use super::config::ConfigRevision;
use super::donde::{donde, Donde};
use super::encargo::{puede_pedirla, reconocer, CandidatoPedido, Pedido, Reconocido};
use super::mensajes;
use super::mes;
use super::mundo::{rubricas_sin_puntuar, AvisoEnElLatido, Mundo};
use super::ordenes;
use super::probados::{self, Estado, NOMBRE_DE_LO_PROBADO};
use super::probar::{probar, Taller};
use super::publicacion::{self, Pendientes};
use crate::arnes::sesion::Revisor;
use crate::config::Config;
use crate::error::Resultado;
use crate::motor::proceso::ServidorLocal;
use crate::terminal::Terminal;

/// Lo que hace cada pedido. Devuelve el cuerpo que se publica.
pub trait Revision {
    fn buscar(&mut self) -> Resultado<String>;
    fn probar(&mut self, candidatos: &[CandidatoPedido]) -> Resultado<String>;
    fn cerrar(&mut self) -> Resultado<String>;
}

/// Atiende el texto de un encargo de `quien` (la identidad de la cabecera de su buzón). Si no es de revisión, no hace nada y devuelve `None`.
pub fn atender(
    quien: Option<&Identidad>,
    donde: &Donde,
    texto: &str,
    config: &ConfigRevision,
    revision: &mut dyn Revision,
) -> Option<String> {
    let pedido = match reconocer(texto) {
        Reconocido::NoEsDeRevision => return None,
        _ if *donde != Donde::EnLaInstalacion => {
            return Some(publicacion::rechazado(&fuera_de_la_instalacion(donde)))
        }
        Reconocido::MalEscrito(motivo) => return Some(publicacion::rechazado(&motivo)),
        Reconocido::Pedido(pedido) => pedido,
    };
    if let Err(motivo) = puede_pedirla(quien, config) {
        return Some(publicacion::rechazado(&motivo));
    }
    let (nombre, hecho) = match &pedido {
        Pedido::Buscar => ("búsqueda", revision.buscar()),
        Pedido::Probar(candidatos) => ("prueba", revision.probar(candidatos)),
        Pedido::Cerrar => ("cierre", revision.cerrar()),
    };
    Some(hecho.unwrap_or_else(|fallo| publicacion::fallido(nombre, &fallo)))
}

fn fuera_de_la_instalacion(donde: &Donde) -> String {
    match donde {
        Donde::Fuera { instalacion } => mensajes::revision_fuera_de_la_instalacion(instalacion),
        Donde::EnLaInstalacion => String::new(),
    }
}

/// El revisor del ciclo: lee la configuración en cada pedido y trabaja con el servidor propio, que
/// comparte con el motor del ciclo.
///
/// Los campos son privados: se construye con `nuevo` y lo único que cambia
/// por fuera es la ruta del latido, cuando se relee la configuración.
pub struct RevisorReal<'a> {
    ruta_de_la_configuracion: PathBuf,
    /// La carpeta de trabajo: la revisión solo se hace si es la de instalación.
    carpeta_de_trabajo: PathBuf,
    ruta_del_latido: PathBuf,
    servidor: &'a RefCell<Option<ServidorLocal>>,
    version: &'a str,
    cambio_el_modelo: bool,
    /// Por dónde cuenta la revisión lo que va haciendo.
    terminal: &'a dyn Terminal,
}

impl<'a> RevisorReal<'a> {
    /// La ruta del latido sale de la configuración: al releerla, puede cambiar.
    pub fn cambiar_la_ruta_del_latido(&mut self, ruta: PathBuf) {
        self.ruta_del_latido = ruta;
    }

    pub fn nuevo(
        ruta_de_la_configuracion: PathBuf,
        carpeta_de_trabajo: PathBuf,
        ruta_del_latido: PathBuf,
        servidor: &'a RefCell<Option<ServidorLocal>>,
        version: &'a str,
        terminal: &'a dyn Terminal,
    ) -> Self {
        Self {
            ruta_de_la_configuracion,
            carpeta_de_trabajo,
            ruta_del_latido,
            servidor,
            version,
            cambio_el_modelo: false,
            terminal,
        }
    }

    /// Si algún pedido cambió `[motor] modelo`, y lo olvida: quien lleve el ciclo vuelve a leer la
    /// configuración una vez.
    pub fn tomar_cambio_de_modelo(&mut self) -> bool {
        std::mem::take(&mut self.cambio_el_modelo)
    }
}

impl Revisor for RevisorReal<'_> {
    fn es_suyo(&self, texto: &str) -> bool {
        reconocer(texto) != Reconocido::NoEsDeRevision
    }
    fn necesita_el_motor(&self, texto: &str) -> bool {
        super::encargo::necesita_el_motor(texto)
    }

    fn atender(&mut self, de: &str, quien: Option<&Identidad>, texto: &str) -> String {
        let config = match Config::cargar(&self.ruta_de_la_configuracion) {
            Ok(config) => config,
            Err(fallo) => return publicacion::fallido("lectura de la configuración", &fallo),
        };
        let ajustes = config.revision.clone();
        let donde = donde(&self.carpeta_de_trabajo, config.carpeta_de_instalacion());
        let latido = AvisoEnElLatido {
            ruta: self.ruta_del_latido.clone(),
            buzon: de.to_string(),
        };
        let mut en_el_mundo = EnElMundo {
            mundo: Mundo {
                carpeta_del_mes: mes::carpeta_del_mes(
                    &config.resolver(&config.revision.carpeta),
                    chrono::Local::now().naive_local(),
                ),
                config,
                ruta_de_la_configuracion: self.ruta_de_la_configuracion.clone(),
                servidor: self.servidor,
                version: self.version,
                cambio_el_modelo: false,
                latido,
                terminal: self.terminal,
            },
        };
        let cuerpo = atender(quien, &donde, texto, &ajustes, &mut en_el_mundo).unwrap_or_default();
        self.cambio_el_modelo |= en_el_mundo.mundo.cambio_el_modelo;
        cuerpo
    }
}

/// Los tres pedidos, hechos con el mundo de verdad.
struct EnElMundo<'a> {
    mundo: Mundo<'a>,
}

impl Revision for EnElMundo<'_> {
    fn buscar(&mut self) -> Resultado<String> {
        // Cada aviso de la reposición va a la terminal y al latido, como en «revisión: probar».
        let config = self.mundo.config.clone();
        let mundo = &mut self.mundo;
        ordenes::buscar(&config, &mut |aviso| {
            mundo.avisar(&mensajes::aviso_de_reposicion(&aviso))
        })
        .map(|hecha| publicacion::busqueda(&hecha))
    }

    fn probar(&mut self, candidatos: &[CandidatoPedido]) -> Resultado<String> {
        let tope = self.mundo.config.revision.candidatos_por_revision;
        let mut probados = probados::cargar(&self.mundo.carpeta_del_mes)?;
        let prueba = probar(candidatos, tope, &mut probados, &mut self.mundo)?;
        let mut pendientes: Pendientes = Vec::new();
        for tanda in prueba
            .probados
            .iter()
            .filter(|p| p.estado == Estado::Comprobada)
            .filter_map(|p| p.tanda.as_ref())
        {
            pendientes.push((tanda.clone(), rubricas_sin_puntuar(tanda)?));
        }
        Ok(publicacion::prueba(&prueba, &pendientes))
    }

    fn cerrar(&mut self) -> Resultado<String> {
        let revisiones = self
            .mundo
            .config
            .resolver(&self.mundo.config.revision.carpeta);
        self.mundo.carpeta_del_mes =
            carpeta_que_se_cierra(&revisiones, &self.mundo.carpeta_del_mes)?;
        let mut probados = probados::cargar(&self.mundo.carpeta_del_mes)?;
        let en_uso = self.mundo.config.motor.nombre_del_modelo();
        let cerrado = cerrar(&mut probados, en_uso.as_deref(), &mut self.mundo)?;
        Ok(publicacion::cierre(&cerrado))
    }
}

/// La carpeta del mes que se cierra: la más reciente que tenga lo probado, que puede ser la del
/// mes anterior si la revisión empezó a final de mes; si no hay ninguna, la del mes en curso.
fn carpeta_que_se_cierra(revisiones: &Path, la_de_este_mes: &Path) -> Resultado<PathBuf> {
    Ok(
        mes::el_mes_mas_reciente_con(revisiones, NOMBRE_DE_LO_PROBADO)?
            .unwrap_or_else(|| la_de_este_mes.to_path_buf()),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::error::Error;

    /// El revisor del ciclo dice, sin tocar nada, qué pedidos tocan el motor: solo «probar».
    #[test]
    fn el_revisor_real_dice_que_solo_probar_necesita_el_motor() {
        let servidor = RefCell::new(None);
        let terminal = crate::terminal::TerminalDeMentira::default();
        let revisor = RevisorReal::nuevo(
            PathBuf::from("no-existe.toml"),
            PathBuf::from("."),
            PathBuf::from("latido.json"),
            &servidor,
            "0.0.0",
            &terminal,
        );

        assert!(!revisor.necesita_el_motor("revisión: buscar"));
        assert!(!revisor.necesita_el_motor("revisión: cerrar"));
        assert!(revisor.necesita_el_motor("revisión: probar\na/b c.gguf"));
    }

    #[test]
    fn se_cierra_la_carpeta_del_mes_con_lo_probado_aunque_sea_el_anterior() {
        let carpeta = tempfile::tempdir().unwrap();
        let anterior = carpeta.path().join("2026-10");
        let este = carpeta.path().join("2026-11");
        std::fs::create_dir_all(&anterior).unwrap();
        std::fs::create_dir_all(&este).unwrap();
        std::fs::write(anterior.join(NOMBRE_DE_LO_PROBADO), "").unwrap();

        assert_eq!(
            carpeta_que_se_cierra(carpeta.path(), &este).unwrap(),
            anterior
        );
    }

    #[test]
    fn sin_nada_probado_se_cierra_la_carpeta_de_este_mes() {
        let carpeta = tempfile::tempdir().unwrap();
        let este = carpeta.path().join("2026-11");

        assert_eq!(carpeta_que_se_cierra(carpeta.path(), &este).unwrap(), este);
    }

    #[derive(Default)]
    struct RevisionFalsa {
        pedidos: Vec<String>,
        falla: bool,
    }

    impl Revision for RevisionFalsa {
        fn buscar(&mut self) -> Resultado<String> {
            self.pedidos.push("buscar".into());
            if self.falla {
                return Err(Error::Revision("Hugging Face no contesta".into()));
            }
            Ok("buscado".into())
        }
        fn probar(&mut self, candidatos: &[CandidatoPedido]) -> Resultado<String> {
            self.pedidos.push(format!("probar {}", candidatos.len()));
            Ok("probado".into())
        }
        fn cerrar(&mut self) -> Resultado<String> {
            self.pedidos.push("cerrar".into());
            Ok("cerrado".into())
        }
    }

    fn con_permiso() -> ConfigRevision {
        ConfigRevision {
            identidades_que_ordenan: vec!["Claude 1".into()],
            ..ConfigRevision::default()
        }
    }

    #[test]
    fn cada_pedido_va_a_lo_suyo() {
        let mut revision = RevisionFalsa::default();
        let config = con_permiso();

        assert_eq!(
            atender(
                Identidad::leer("Claude 1").as_ref(),
                &Donde::EnLaInstalacion,
                "revisión: buscar",
                &config,
                &mut revision
            )
            .as_deref(),
            Some("buscado")
        );
        atender(
            Identidad::leer("Claude 1").as_ref(),
            &Donde::EnLaInstalacion,
            "revisión: probar\na/b c.gguf\nd/e f.gguf",
            &config,
            &mut revision,
        );
        atender(
            Identidad::leer("Claude 1").as_ref(),
            &Donde::EnLaInstalacion,
            "revisión: cerrar",
            &config,
            &mut revision,
        );

        assert_eq!(revision.pedidos, ["buscar", "probar 2", "cerrar"]);
    }

    /// Fuera de la carpeta de instalación no se hace nada, aunque haya permiso.
    #[test]
    fn fuera_de_la_instalacion_no_se_hace_nada_y_se_dice_adonde_ir() {
        let mut revision = RevisionFalsa::default();
        let fuera = Donde::Fuera {
            instalacion: "C:/Programator".into(),
        };

        for pedido in ["revisión: buscar", "revisión: cerrar", "revisión: probar"] {
            let respuesta = atender(
                Identidad::leer("Claude 1").as_ref(),
                &fuera,
                pedido,
                &con_permiso(),
                &mut revision,
            )
            .unwrap();
            assert!(respuesta.contains("«C:/Programator»"), "{respuesta}");
        }
        assert!(revision.pedidos.is_empty(), "{:?}", revision.pedidos);
        assert_eq!(
            atender(
                Identidad::leer("Claude 1").as_ref(),
                &fuera,
                "Escribe una función.",
                &con_permiso(),
                &mut revision
            ),
            None
        );
    }

    #[test]
    fn un_encargo_normal_no_se_atiende_aqui() {
        let mut revision = RevisionFalsa::default();
        assert_eq!(
            atender(
                Identidad::leer("Claude 1").as_ref(),
                &Donde::EnLaInstalacion,
                "Escribe una función.",
                &con_permiso(),
                &mut revision
            ),
            None
        );
        assert!(revision.pedidos.is_empty());
    }

    #[test]
    fn sin_permiso_o_mal_escrito_se_rechaza_sin_hacer_nada() {
        let mut revision = RevisionFalsa::default();

        let sin_permiso = atender(
            Identidad::leer("Gemini 1").as_ref(),
            &Donde::EnLaInstalacion,
            "revisión: buscar",
            &con_permiso(),
            &mut revision,
        )
        .unwrap();
        let deshabilitada = atender(
            Identidad::leer("Claude 1").as_ref(),
            &Donde::EnLaInstalacion,
            "revisión: buscar",
            &ConfigRevision::default(),
            &mut revision,
        )
        .unwrap();
        let mal_escrito = atender(
            Identidad::leer("Claude 1").as_ref(),
            &Donde::EnLaInstalacion,
            "revisión: probar",
            &con_permiso(),
            &mut revision,
        )
        .unwrap();

        assert!(sin_permiso.contains("«Gemini 1»"), "{sin_permiso}");
        assert!(
            deshabilitada.contains("identidades_que_ordenan"),
            "{deshabilitada}"
        );
        assert!(mal_escrito.contains("ningún candidato"), "{mal_escrito}");
        assert!(revision.pedidos.is_empty());
    }

    #[test]
    fn un_pedido_que_falla_se_publica_con_su_fallo() {
        let mut revision = RevisionFalsa {
            falla: true,
            ..RevisionFalsa::default()
        };
        let texto = atender(
            Identidad::leer("Claude 1").as_ref(),
            &Donde::EnLaInstalacion,
            "revisión: buscar",
            &con_permiso(),
            &mut revision,
        )
        .unwrap();
        assert!(texto.contains("búsqueda"), "{texto}");
        assert!(texto.contains("Hugging Face no contesta"), "{texto}");
    }
}
