//! `--revision-buscar`: consulta Hugging Face, clasifica lo encontrado y lo deja en la carpeta del
//! mes (especificación, §3).
//!
//! Lo que se mide fuera —la tarjeta, la hora, el registro— llega en [`Entorno`], y Hugging Face
//! llega como un [`CatalogoDeModelos`], para que esto se pruebe sin red, GPU ni reloj. Nada se
//! escribe hasta que todas las consultas han contestado: un fallo a medias no deja una lista a
//! medias.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

use super::candidatos::{clasificar, Clasificacion, Criterios};
use super::catalogo::{Busqueda, CatalogoDeModelos, DetalleDeRepositorio};
use super::config::ConfigRevision;
use super::lista::{self, EntradaDeLaLista, ListaDeCandidatos, NOMBRE_DE_LA_LISTA};
use super::{informe, mes, BYTES_POR_MIB};
use crate::error::{Error, Resultado};
use crate::ficheros;
use crate::motor::hardware::Gpu;
use crate::registro::modelo::Registro;

const FORMATO_DEL_DIA: &str = "%Y-%m-%d";
const NOMBRE_DEL_INFORME: &str = "candidatos.md";
const CARPETA_DE_RESPUESTAS: &str = "respuestas";
const EXTENSION_DEL_MODELO: &str = "gguf";

/// Lo que la búsqueda necesita del mundo.
pub struct Entorno<'a> {
    pub catalogo: &'a dyn CatalogoDeModelos,
    /// La tarjeta medida; sin ella no se busca.
    pub tarjeta: Option<Gpu>,
    /// El margen de `[motor]`, en MiB.
    pub margen_vram_mib: u64,
    pub registro: &'a Registro,
    pub carpeta_de_revisiones: PathBuf,
    pub carpeta_de_modelos: PathBuf,
    pub ahora: NaiveDateTime,
}

/// Lo que dejó una búsqueda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusquedaHecha {
    pub carpeta: PathBuf,
    pub candidatos: usize,
    pub ya_descargados: usize,
    pub descartados: usize,
    /// El nombre con que se guardó la lista anterior del mes, si la había.
    pub anterior: Option<String>,
}

/// Las respuestas de todas las consultas, y los detalles de cada repositorio una sola vez.
struct Consultas {
    detalles: Vec<DetalleDeRepositorio>,
    /// El nombre del fichero de cada respuesta y su texto en bruto.
    respuestas: Vec<(String, String)>,
}

pub fn buscar(config: &ConfigRevision, entorno: &Entorno) -> Resultado<BusquedaHecha> {
    let tarjeta = entorno
        .tarjeta
        .as_ref()
        .ok_or_else(|| Error::Revision(super::mensajes::SIN_TARJETA.to_string()))?;
    let consultas = consultar(entorno.catalogo, config)?;
    let huellas = huellas_del_registro(entorno.registro);
    let sin_medir = ficheros_sin_medir(&entorno.carpeta_de_modelos, entorno.registro)?;
    let criterios = Criterios {
        config,
        vram_mib: tarjeta.vram_total / BYTES_POR_MIB,
        margen_vram_mib: entorno.margen_vram_mib,
        huellas_registradas: &huellas,
        ficheros_en_modelos: &sin_medir,
        hoy: entorno.ahora.date(),
    };
    let clasificacion = clasificar(&consultas.detalles, &criterios);
    let lista = componer_la_lista(&clasificacion, &criterios, tarjeta, config, entorno);
    escribir(&lista, &clasificacion, &consultas, config, entorno)
}

/// Cada búsqueda y, después, el detalle de cada repositorio encontrado, una vez aunque salga en
/// varias búsquedas.
fn consultar(catalogo: &dyn CatalogoDeModelos, config: &ConfigRevision) -> Resultado<Consultas> {
    let mut respuestas = Vec::new();
    let mut vistos = BTreeSet::new();
    let mut repositorios = Vec::new();
    for texto in &config.busquedas {
        let busqueda = Busqueda {
            texto: texto.clone(),
            biblioteca: config.biblioteca.clone(),
            limite: config.resultados_por_busqueda,
        };
        let respuesta = catalogo
            .buscar(&busqueda)
            .map_err(|fallo| en_la_consulta(&format!("la búsqueda «{texto}»"), fallo))?;
        respuestas.push((
            nombre_de_la_respuesta("busqueda", texto),
            respuesta.en_bruto,
        ));
        for repositorio in respuesta.datos {
            if vistos.insert(repositorio.id.clone()) {
                repositorios.push(repositorio.id);
            }
        }
    }
    let mut detalles = Vec::new();
    for repositorio in repositorios {
        let respuesta = catalogo
            .detalle(&repositorio)
            .map_err(|fallo| en_la_consulta(&format!("el detalle de «{repositorio}»"), fallo))?;
        respuestas.push((
            nombre_de_la_respuesta("detalle", &repositorio),
            respuesta.en_bruto,
        ));
        detalles.push(respuesta.datos);
    }
    Ok(Consultas {
        detalles,
        respuestas,
    })
}

/// El fallo de una consulta, diciendo cuál fue.
fn en_la_consulta(cual: &str, fallo: Error) -> Error {
    let causa = match fallo {
        Error::Revision(texto) => texto,
        otro => otro.to_string(),
    };
    Error::Revision(format!("{cual} falló, y no se ha escrito nada: {causa}"))
}

/// Un nombre de fichero sacado de la consulta: `busqueda-coder_GGUF.json`,
/// `detalle-bartowski--Modelo-GGUF.json`.
fn nombre_de_la_respuesta(clase: &str, consulta: &str) -> String {
    let saneado: String = consulta
        .replace('/', "--")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{clase}-{saneado}.json")
}

fn huellas_del_registro(registro: &Registro) -> BTreeSet<String> {
    registro
        .modelo
        .iter()
        .map(|entrada| entrada.huella_sha256.clone())
        .collect()
}

/// Los GGUF de `modelos/` cuyo nombre no está en el registro. Sin carpeta, ninguno.
fn ficheros_sin_medir(carpeta: &Path, registro: &Registro) -> Resultado<BTreeSet<String>> {
    if !carpeta.is_dir() {
        return Ok(BTreeSet::new());
    }
    let medidos: BTreeSet<&str> = registro.modelo.iter().map(|e| e.nombre.as_str()).collect();
    Ok(ficheros::entradas(carpeta)?
        .into_iter()
        .filter(|ruta| {
            ruta.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION_DEL_MODELO))
        })
        .filter(|ruta| {
            ruta.file_stem()
                .is_some_and(|nombre| !medidos.contains(nombre.to_string_lossy().as_ref()))
        })
        .filter_map(|ruta| Some(ruta.file_name()?.to_string_lossy().into_owned()))
        .collect())
}

fn componer_la_lista(
    clasificacion: &Clasificacion,
    criterios: &Criterios,
    tarjeta: &Gpu,
    config: &ConfigRevision,
    entorno: &Entorno,
) -> ListaDeCandidatos {
    let entradas = |ficheros: &[super::candidatos::FicheroCandidato]| {
        ficheros
            .iter()
            .map(|f| EntradaDeLaLista::nueva(f.clone(), &config.sitio))
            .collect()
    };
    ListaDeCandidatos {
        fecha: entorno.ahora.format(FORMATO_DEL_DIA).to_string(),
        tarjeta: tarjeta.nombre.clone(),
        vram_mib: criterios.vram_mib,
        cabe_mib: criterios.cabe_mib(),
        candidatos: entradas(&clasificacion.candidatos),
        ya_descargados: entradas(&clasificacion.ya_descargados),
    }
}

/// Escribe la lista, el informe y las respuestas en la carpeta del mes, guardando antes con la
/// hora lo que dejara una búsqueda anterior del mismo mes.
fn escribir(
    lista: &ListaDeCandidatos,
    clasificacion: &Clasificacion,
    consultas: &Consultas,
    config: &ConfigRevision,
    entorno: &Entorno,
) -> Resultado<BusquedaHecha> {
    let texto_de_la_lista = lista::escribir(lista)?;
    let texto_del_informe = informe::componer(lista, &clasificacion.descartados, &config.sitio);
    let carpeta = mes::carpeta_del_mes(&entorno.carpeta_de_revisiones, entorno.ahora);
    ficheros::crear_carpeta(&carpeta)?;
    let anterior = mes::apartar_lo_anterior(
        &carpeta,
        NOMBRE_DE_LA_LISTA,
        &[NOMBRE_DEL_INFORME, CARPETA_DE_RESPUESTAS],
        entorno.ahora,
    )?;
    let respuestas = carpeta.join(CARPETA_DE_RESPUESTAS);
    ficheros::crear_carpeta(&respuestas)?;
    for (nombre, texto) in &consultas.respuestas {
        ficheros::escribir(&respuestas.join(nombre), texto)?;
    }
    ficheros::escribir(&carpeta.join(NOMBRE_DEL_INFORME), &texto_del_informe)?;
    ficheros::escribir(&carpeta.join(NOMBRE_DE_LA_LISTA), &texto_de_la_lista)?;
    Ok(BusquedaHecha {
        carpeta,
        candidatos: clasificacion.candidatos.len(),
        ya_descargados: clasificacion.ya_descargados.len(),
        descartados: clasificacion.descartados.len(),
        anterior,
    })
}

#[cfg(test)]
mod pruebas {
    use std::cell::RefCell;

    use chrono::NaiveDate;

    use super::*;
    use crate::revision::catalogo::{leer_busqueda, leer_detalle, Repositorio, Respuesta};

    const BUSQUEDA: &str = include_str!("pruebas/busqueda.json");
    const CON_LICENCIA: &str = include_str!("pruebas/detalle-con-licencia.json");
    const GIB: u64 = 1024 * BYTES_POR_MIB;

    /// Contesta cada búsqueda con la respuesta guardada y cada detalle con el de bartowski,
    /// cambiándole el nombre. Anota qué se le pidió, y falla en lo que se le diga.
    #[derive(Default)]
    struct CatalogoFalso {
        pedidos: RefCell<Vec<String>>,
        falla_en: Option<String>,
    }

    impl CatalogoDeModelos for CatalogoFalso {
        fn buscar(&self, busqueda: &Busqueda) -> Resultado<Respuesta<Vec<Repositorio>>> {
            self.pedidos.borrow_mut().push(busqueda.texto.clone());
            if self.falla_en.as_deref() == Some(busqueda.texto.as_str()) {
                return Err(Error::Revision("sin red".into()));
            }
            Ok(Respuesta {
                datos: leer_busqueda(BUSQUEDA)?,
                en_bruto: BUSQUEDA.into(),
            })
        }

        fn detalle(&self, repositorio: &str) -> Resultado<Respuesta<DetalleDeRepositorio>> {
            self.pedidos.borrow_mut().push(repositorio.to_string());
            let mut datos = leer_detalle(CON_LICENCIA)?;
            datos.id = repositorio.to_string();
            Ok(Respuesta {
                datos,
                en_bruto: CON_LICENCIA.into(),
            })
        }
    }

    fn config() -> ConfigRevision {
        ConfigRevision {
            busquedas: vec!["coder GGUF".into(), "devstral GGUF".into()],
            modificados_en_los_ultimos_dias: 0,
            ..ConfigRevision::default()
        }
    }

    fn tarjeta() -> Gpu {
        Gpu {
            nombre: "Tarjeta de prueba".into(),
            vram_total: 24 * GIB,
            vram_libre: 20 * GIB,
        }
    }

    fn a_las(hora: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 12)
            .unwrap()
            .and_hms_opt(hora, 30, 12)
            .unwrap()
    }

    fn entorno<'a>(
        catalogo: &'a CatalogoFalso,
        registro: &'a Registro,
        raiz: &Path,
        ahora: NaiveDateTime,
    ) -> Entorno<'a> {
        Entorno {
            catalogo,
            tarjeta: Some(tarjeta()),
            margen_vram_mib: 2048,
            registro,
            carpeta_de_revisiones: raiz.join("revisiones"),
            carpeta_de_modelos: raiz.join("modelos"),
            ahora,
        }
    }

    #[test]
    fn deja_la_lista_el_informe_y_las_respuestas_en_la_carpeta_del_mes() {
        let raiz = tempfile::tempdir().unwrap();
        let catalogo = CatalogoFalso::default();
        let registro = Registro::default();
        let hecha = buscar(
            &config(),
            &entorno(&catalogo, &registro, raiz.path(), a_las(15)),
        )
        .unwrap();
        let mes = raiz.path().join("revisiones").join("2026-11");
        assert_eq!(hecha.carpeta, mes);
        assert_eq!(hecha.anterior, None);
        let leida = lista::leer(&ficheros::leer(&mes.join(NOMBRE_DE_LA_LISTA)).unwrap()).unwrap();
        assert_eq!(leida.fecha, "2026-11-12");
        assert_eq!(leida.tarjeta, "Tarjeta de prueba");
        assert_eq!(leida.cabe_mib, 20 * 1024);
        assert_eq!(leida.candidatos.len(), hecha.candidatos);
        assert!(hecha.candidatos > 0);
        assert!(ficheros::leer(&mes.join(NOMBRE_DEL_INFORME))
            .unwrap()
            .starts_with("# Candidatos de la revisión del 2026-11-12"));
        let respuestas = mes.join(CARPETA_DE_RESPUESTAS);
        assert!(respuestas.join("busqueda-coder_GGUF.json").is_file());
        assert!(respuestas
            .join("detalle-bartowski--mistralai_Devstral-Small-2-24B-Instruct-2512-GGUF.json")
            .is_file());
    }

    #[test]
    fn un_repositorio_que_sale_en_varias_busquedas_se_pide_una_vez() {
        let raiz = tempfile::tempdir().unwrap();
        let catalogo = CatalogoFalso::default();
        let registro = Registro::default();
        buscar(
            &config(),
            &entorno(&catalogo, &registro, raiz.path(), a_las(15)),
        )
        .unwrap();
        let pedidos = catalogo.pedidos.borrow();
        let repositorios = leer_busqueda(BUSQUEDA).unwrap();
        assert_eq!(pedidos.len(), 2 + repositorios.len(), "{pedidos:?}");
        for repositorio in repositorios {
            assert_eq!(pedidos.iter().filter(|p| **p == repositorio.id).count(), 1);
        }
    }

    #[test]
    fn una_segunda_busqueda_del_mes_guarda_la_anterior_con_la_hora() {
        let raiz = tempfile::tempdir().unwrap();
        let catalogo = CatalogoFalso::default();
        let registro = Registro::default();
        buscar(
            &config(),
            &entorno(&catalogo, &registro, raiz.path(), a_las(9)),
        )
        .unwrap();
        let hecha = buscar(
            &config(),
            &entorno(&catalogo, &registro, raiz.path(), a_las(15)),
        )
        .unwrap();
        assert_eq!(hecha.anterior.as_deref(), Some("candidatos-153012.toml"));
        let mes = &hecha.carpeta;
        for nombre in [
            "candidatos.toml",
            "candidatos-153012.toml",
            "candidatos.md",
            "candidatos-153012.md",
        ] {
            assert!(mes.join(nombre).is_file(), "falta {nombre}");
        }
        assert!(mes.join("respuestas-153012").is_dir());
        assert!(mes.join(CARPETA_DE_RESPUESTAS).is_dir());
    }

    #[test]
    fn sin_tarjeta_se_niega_y_no_escribe_nada() {
        let raiz = tempfile::tempdir().unwrap();
        let catalogo = CatalogoFalso::default();
        let registro = Registro::default();
        let mut sin_tarjeta = entorno(&catalogo, &registro, raiz.path(), a_las(15));
        sin_tarjeta.tarjeta = None;
        let fallo = buscar(&config(), &sin_tarjeta).unwrap_err().to_string();
        assert!(fallo.contains("tarjeta"), "{fallo}");
        assert!(!raiz.path().join("revisiones").exists());
        assert!(catalogo.pedidos.borrow().is_empty());
    }

    #[test]
    fn un_fallo_de_red_dice_que_consulta_fallo_y_no_deja_lista() {
        let raiz = tempfile::tempdir().unwrap();
        let catalogo = CatalogoFalso {
            falla_en: Some("devstral GGUF".into()),
            ..CatalogoFalso::default()
        };
        let registro = Registro::default();
        let fallo = buscar(
            &config(),
            &entorno(&catalogo, &registro, raiz.path(), a_las(15)),
        )
        .unwrap_err()
        .to_string();
        assert!(
            fallo.contains("«devstral GGUF»") && fallo.contains("sin red"),
            "{fallo}"
        );
        assert!(!raiz.path().join("revisiones").exists());
    }

    #[test]
    fn lo_que_hay_en_modelos_sin_medir_sale_como_ya_descargado() {
        let raiz = tempfile::tempdir().unwrap();
        let modelos = raiz.path().join("modelos");
        ficheros::crear_carpeta(&modelos).unwrap();
        let nombre = "mistralai_Devstral-Small-2-24B-Instruct-2512-Q4_K_M";
        ficheros::escribir(&modelos.join(format!("{nombre}.gguf")), "").unwrap();
        let catalogo = CatalogoFalso::default();
        let registro = Registro::default();
        let hecha = buscar(
            &config(),
            &entorno(&catalogo, &registro, raiz.path(), a_las(15)),
        )
        .unwrap();
        assert!(hecha.ya_descargados > 0);
    }

    #[test]
    fn lo_que_hay_en_modelos_y_esta_medido_no_cuenta_como_sin_medir() {
        let raiz = tempfile::tempdir().unwrap();
        let modelos = raiz.path().join("modelos");
        ficheros::crear_carpeta(&modelos).unwrap();
        ficheros::escribir(&modelos.join("medido.gguf"), "").unwrap();
        ficheros::escribir(&modelos.join("nuevo.GGUF"), "").unwrap();
        ficheros::escribir(&modelos.join("notas.txt"), "").unwrap();
        let mut medido = crate::registro::modelo::pruebas::entrada_de_ejemplo();
        medido.nombre = "medido".into();
        let registro = Registro {
            modelo: vec![medido],
        };
        let sin_medir = ficheros_sin_medir(&modelos, &registro).unwrap();
        let esperado: BTreeSet<String> = ["nuevo.GGUF".to_string()].into();
        assert_eq!(sin_medir, esperado);
    }

    #[test]
    fn la_huella_de_lo_registrado_descarta_su_fichero() {
        let raiz = tempfile::tempdir().unwrap();
        let detalle = leer_detalle(CON_LICENCIA).unwrap();
        let mut medido = crate::registro::modelo::pruebas::entrada_de_ejemplo();
        medido.huella_sha256 = detalle.ficheros[1].huella_sha256.clone().unwrap();
        let registro = Registro {
            modelo: vec![medido],
        };
        let catalogo = CatalogoFalso::default();
        let sin = buscar(
            &config(),
            &entorno(&catalogo, &Registro::default(), raiz.path(), a_las(9)),
        )
        .unwrap();
        let con = buscar(
            &config(),
            &entorno(&catalogo, &registro, raiz.path(), a_las(15)),
        )
        .unwrap();
        assert!(con.descartados > sin.descartados, "{sin:?} {con:?}");
    }

    #[test]
    fn el_nombre_de_cada_respuesta_sale_de_su_consulta() {
        assert_eq!(
            nombre_de_la_respuesta("busqueda", "code instruct GGUF"),
            "busqueda-code_instruct_GGUF.json"
        );
        assert_eq!(
            nombre_de_la_respuesta("detalle", "Qwen/Qwen2.5-Coder"),
            "detalle-Qwen--Qwen2.5-Coder.json"
        );
    }
}
