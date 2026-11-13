//! Atender un pedido de revisión que llega por el canal: reconocerlo, ver si quien lo pide puede,
//! hacerlo y redactar lo que se publica (especificación de la revisión autónoma, §4).
//!
//! [`atender`] decide y se prueba con una revisión falsa; [`RevisorReal`] la monta con el mundo de
//! verdad para el ciclo de `main`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use super::cerrar::cerrar;
use super::config::ConfigRevision;
use super::encargo::{puede_pedirla, reconocer, CandidatoPedido, Pedido, Reconocido};
use super::mes;
use super::mundo::{rubricas_sin_puntuar, AvisoEnElLatido, Mundo};
use super::ordenes;
use super::probados::{self, Estado, NOMBRE_DE_LO_PROBADO};
use super::probar::probar;
use super::publicacion::{self, Pendientes};
use crate::arnes::sesion::Revisor;
use crate::config::Config;
use crate::error::Resultado;
use crate::motor::proceso::ServidorLocal;

/// Lo que hace cada pedido. Devuelve el cuerpo que se publica.
pub trait Revision {
    fn buscar(&mut self) -> Resultado<String>;
    fn probar(&mut self, candidatos: &[CandidatoPedido]) -> Resultado<String>;
    fn cerrar(&mut self) -> Resultado<String>;
}

/// Atiende el texto de un encargo de `de`. Si no es de revisión, no hace nada y devuelve `None`.
pub fn atender(
    de: &str,
    texto: &str,
    config: &ConfigRevision,
    revision: &mut dyn Revision,
) -> Option<String> {
    let pedido = match reconocer(texto) {
        Reconocido::NoEsDeRevision => return None,
        Reconocido::MalEscrito(motivo) => return Some(publicacion::rechazado(&motivo)),
        Reconocido::Pedido(pedido) => pedido,
    };
    if let Err(motivo) = puede_pedirla(de, config) {
        return Some(publicacion::rechazado(&motivo));
    }
    let (nombre, hecho) = match &pedido {
        Pedido::Buscar => ("búsqueda", revision.buscar()),
        Pedido::Probar(candidatos) => ("prueba", revision.probar(candidatos)),
        Pedido::Cerrar => ("cierre", revision.cerrar()),
    };
    Some(hecho.unwrap_or_else(|fallo| publicacion::fallido(nombre, &fallo)))
}

/// El revisor del ciclo: lee la configuración en cada pedido y trabaja con el servidor propio, que
/// comparte con el motor del ciclo.
pub struct RevisorReal<'a> {
    pub ruta_de_la_configuracion: PathBuf,
    pub ruta_del_latido: PathBuf,
    pub servidor: &'a RefCell<Option<ServidorLocal>>,
    pub version: &'a str,
    cambio_el_modelo: bool,
}

impl<'a> RevisorReal<'a> {
    pub fn nuevo(
        ruta_de_la_configuracion: PathBuf,
        ruta_del_latido: PathBuf,
        servidor: &'a RefCell<Option<ServidorLocal>>,
        version: &'a str,
    ) -> Self {
        Self {
            ruta_de_la_configuracion,
            ruta_del_latido,
            servidor,
            version,
            cambio_el_modelo: false,
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

    fn atender(&mut self, de: &str, texto: &str) -> String {
        let config = match Config::cargar(&self.ruta_de_la_configuracion) {
            Ok(config) => config,
            Err(fallo) => return publicacion::fallido("lectura de la configuración", &fallo),
        };
        let ajustes = config.revision.clone();
        let latido = AvisoEnElLatido {
            ruta: self.ruta_del_latido.clone(),
            intervalo_segundos: config.ciclo.intervalo_segundos,
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
            },
        };
        let cuerpo = atender(de, texto, &ajustes, &mut en_el_mundo).unwrap_or_default();
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
        ordenes::buscar(&self.mundo.config).map(|hecha| publicacion::busqueda(&hecha))
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

    #[test]
    fn se_cierra_la_carpeta_del_mes_con_lo_probado_aunque_sea_el_anterior() {
        let dir = tempfile::tempdir().unwrap();
        let anterior = dir.path().join("2026-10");
        let este = dir.path().join("2026-11");
        std::fs::create_dir_all(&anterior).unwrap();
        std::fs::create_dir_all(&este).unwrap();
        std::fs::write(anterior.join(NOMBRE_DE_LO_PROBADO), "").unwrap();

        assert_eq!(carpeta_que_se_cierra(dir.path(), &este).unwrap(), anterior);
    }

    #[test]
    fn sin_nada_probado_se_cierra_la_carpeta_de_este_mes() {
        let dir = tempfile::tempdir().unwrap();
        let este = dir.path().join("2026-11");

        assert_eq!(carpeta_que_se_cierra(dir.path(), &este).unwrap(), este);
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
            buzones_que_ordenan: vec!["claude".into()],
            ..ConfigRevision::default()
        }
    }

    #[test]
    fn cada_pedido_va_a_lo_suyo() {
        let mut revision = RevisionFalsa::default();
        let config = con_permiso();

        assert_eq!(
            atender("claude.md", "revisión: buscar", &config, &mut revision).as_deref(),
            Some("buscado")
        );
        atender(
            "claude.md",
            "revisión: probar\na/b c.gguf\nd/e f.gguf",
            &config,
            &mut revision,
        );
        atender("claude.md", "revisión: cerrar", &config, &mut revision);

        assert_eq!(revision.pedidos, ["buscar", "probar 2", "cerrar"]);
    }

    #[test]
    fn un_encargo_normal_no_se_atiende_aqui() {
        let mut revision = RevisionFalsa::default();
        assert_eq!(
            atender(
                "claude.md",
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
            "gemini.md",
            "revisión: buscar",
            &con_permiso(),
            &mut revision,
        )
        .unwrap();
        let deshabilitada = atender(
            "claude.md",
            "revisión: buscar",
            &ConfigRevision::default(),
            &mut revision,
        )
        .unwrap();
        let mal_escrito = atender(
            "claude.md",
            "revisión: probar",
            &con_permiso(),
            &mut revision,
        )
        .unwrap();

        assert!(sin_permiso.contains("«gemini»"), "{sin_permiso}");
        assert!(
            deshabilitada.contains("buzones_que_ordenan"),
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
            "claude.md",
            "revisión: buscar",
            &con_permiso(),
            &mut revision,
        )
        .unwrap();
        assert!(texto.contains("búsqueda"), "{texto}");
        assert!(texto.contains("Hugging Face no contesta"), "{texto}");
    }
}
