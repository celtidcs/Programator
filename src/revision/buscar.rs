//! `--revision-buscar`: consulta Hugging Face, clasifica lo encontrado y lo deja en la carpeta del
//! mes.
//!
//! Lo que se mide fuera —la tarjeta, la hora, el registro— llega en [`Entorno`], y Hugging Face
//! llega como un [`CatalogoDeModelos`], para que esto se pruebe sin red, GPU ni reloj. Nada se
//! escribe hasta que todas las consultas han contestado: un fallo a medias no deja una lista a
//! medias.
//!
//! Entre consultar y clasificar se reponen los modelos útiles del registro que faltan en la
//! carpeta de modelos (`reponer`): así la búsqueda les aporta su origen y lo repuesto ya está en la
//! carpeta cuando se clasifica.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

use super::candidatos::{clasificar, huellas_del_registro, Clasificacion, Criterios};
use super::catalogo::{Busqueda, CatalogoDeModelos, DetalleDeRepositorio};
use super::config::ConfigRevision;
use super::descarga::Fuente;
use super::lista::{self, EntradaDeLaLista, ListaDeCandidatos, NOMBRE_DE_LA_LISTA};
use super::proponer::ficheros_con_tamano;
use super::reponer::{self, AvisoDeReposicion, Reposicion};
use super::{informe, mes, traer, BYTES_POR_MIB, FORMATO_DEL_DIA};
use crate::error::{Error, Resultado};
use crate::ficheros;
use crate::motor::fichero_de_modelo;
use crate::motor::hardware::Gpu;
use crate::registro::modelo::{self, Registro};

/// La lista de candidatos para leerla.
pub const NOMBRE_DEL_INFORME: &str = "candidatos.md";
/// Lo que contestó Hugging Face, tal cual.
pub const CARPETA_DE_RESPUESTAS: &str = "respuestas";

/// Lo que la búsqueda necesita del mundo.
pub struct Entorno<'a> {
    pub catalogo: &'a dyn CatalogoDeModelos,
    /// La tarjeta medida; sin ella no se busca.
    pub tarjeta: Option<Gpu>,
    /// El margen de `[motor]`, en MiB.
    pub margen_vram_mib: u64,
    pub ruta_del_registro: PathBuf,
    pub carpeta_de_revisiones: PathBuf,
    pub carpeta_de_modelos: PathBuf,
    pub ahora: NaiveDateTime,
    pub para_reponer: ParaReponer<'a>,
}

/// Lo que hace falta, además, para reponer los modelos que faltan.
pub struct ParaReponer<'a> {
    /// La fuente de un fichero, dado su repositorio y su nombre.
    pub fuente_de: &'a dyn Fn(&str, &str) -> Box<dyn Fuente>,
    pub espacio_libre: &'a dyn Fn(&Path) -> Resultado<u64>,
    pub margen_disco_mib: u64,
    pub avisar_cada_mib: u64,
    /// El nombre, sin extensión, del modelo de la configuración.
    pub modelo_en_uso: Option<String>,
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
    /// Lo que se hizo con los modelos del registro que faltaban.
    pub reposicion: Reposicion,
}

/// Las respuestas de todas las consultas, y los detalles de cada repositorio una sola vez.
struct Consultas {
    detalles: Vec<DetalleDeRepositorio>,
    /// El nombre del fichero de cada respuesta y su texto en bruto.
    respuestas: Vec<(String, String)>,
}

/// Consulta Hugging Face, repone los modelos útiles que faltan y clasifica lo encontrado con el
/// registro y la carpeta como quedan después. Sin tarjeta no se consulta nada.
pub fn buscar(
    config: &ConfigRevision,
    entorno: &Entorno,
    avisar: &mut dyn FnMut(AvisoDeReposicion),
) -> Resultado<BusquedaHecha> {
    let tarjeta = la_tarjeta(entorno)?;
    let consultas = consultar(entorno.catalogo, config)?;
    let reposicion = reponer_lo_que_falta(entorno, tarjeta, &consultas, avisar)?;
    let registro = modelo::leer(&entorno.ruta_del_registro)?;
    clasificar_y_escribir(config, entorno, &registro, &consultas, reposicion)
}

fn reponer_lo_que_falta(
    entorno: &Entorno,
    tarjeta: &Gpu,
    consultas: &Consultas,
    avisar: &mut dyn FnMut(AvisoDeReposicion),
) -> Resultado<Reposicion> {
    let para = &entorno.para_reponer;
    let dia = entorno.ahora.format(FORMATO_DEL_DIA).to_string();
    reponer::reponer(
        &reponer::Entorno {
            descarga: traer::Entorno {
                carpeta_de_revisiones: entorno.carpeta_de_revisiones.clone(),
                carpeta_de_modelos: entorno.carpeta_de_modelos.clone(),
                margen_disco_mib: para.margen_disco_mib,
                avisar_cada_mib: para.avisar_cada_mib,
                fuente_de: para.fuente_de,
                espacio_libre: para.espacio_libre,
            },
            catalogo: entorno.catalogo,
            detalles: &consultas.detalles,
            tarjeta: &tarjeta.nombre,
            modelo_en_uso: para.modelo_en_uso.as_deref(),
            ruta_del_registro: &entorno.ruta_del_registro,
            dia: &dia,
        },
        avisar,
    )
}

fn la_tarjeta<'a>(entorno: &'a Entorno) -> Resultado<&'a Gpu> {
    entorno
        .tarjeta
        .as_ref()
        .ok_or_else(|| Error::Revision(super::mensajes::sin_tarjeta()))
}

/// Clasifica lo que contestaron las consultas con el registro y la carpeta de modelos, y lo deja
/// en la carpeta del mes.
fn clasificar_y_escribir(
    config: &ConfigRevision,
    entorno: &Entorno,
    registro: &Registro,
    consultas: &Consultas,
    reposicion: Reposicion,
) -> Resultado<BusquedaHecha> {
    let tarjeta = la_tarjeta(entorno)?;
    let en_modelos = ficheros_con_tamano(&entorno.carpeta_de_modelos)?;
    let huellas = huellas_del_registro(registro, &en_modelos);
    let sin_medir = ficheros_sin_medir(&entorno.carpeta_de_modelos, registro)?;
    let criterios = Criterios {
        config,
        vram_mib: tarjeta.vram_total / BYTES_POR_MIB,
        margen_vram_mib: entorno.margen_vram_mib,
        huellas: &huellas,
        ficheros_en_modelos: &sin_medir,
        hoy: entorno.ahora.date(),
    };
    let clasificacion = clasificar(&consultas.detalles, &criterios);
    let lista = componer_la_lista(&clasificacion, &criterios, tarjeta, config, entorno);
    escribir(
        &lista,
        &clasificacion,
        consultas,
        reposicion,
        config,
        entorno,
    )
}

/// Cada búsqueda y, después, el detalle de cada repositorio encontrado, una vez aunque salga en
/// varias búsquedas. Si una falla, no se devuelve nada.
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
            .map_err(|fallo| en_la_consulta(&super::mensajes::la_busqueda(texto), fallo))?;
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
        let respuesta = catalogo.detalle(&repositorio).map_err(|fallo| {
            en_la_consulta(&super::mensajes::el_detalle_de(&repositorio), fallo)
        })?;
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
    Error::Revision(super::mensajes::consulta_fallida_sin_escribir(cual, &causa))
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

/// Los GGUF de `modelos/` cuyo nombre no está en el registro. Sin carpeta, ninguno.
fn ficheros_sin_medir(carpeta: &Path, registro: &Registro) -> Resultado<BTreeSet<String>> {
    if !carpeta.is_dir() {
        return Ok(BTreeSet::new());
    }
    let medidos: BTreeSet<&str> = registro.modelo.iter().map(|e| e.nombre.as_str()).collect();
    Ok(ficheros::entradas(carpeta)?
        .into_iter()
        .filter(|ruta| fichero_de_modelo::es_ruta_gguf(ruta))
        .filter(|ruta| {
            fichero_de_modelo::nombre_de_modelo(ruta)
                .is_some_and(|nombre| !medidos.contains(nombre.as_str()))
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
    reposicion: Reposicion,
    config: &ConfigRevision,
    entorno: &Entorno,
) -> Resultado<BusquedaHecha> {
    let texto_de_la_lista = lista::escribir(lista)?;
    let texto_del_informe = informe::componer(
        lista,
        &clasificacion.descartados,
        &reposicion,
        &config.sitio,
    );
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
        reposicion,
    })
}

#[cfg(test)]
mod pruebas {
    use chrono::NaiveDate;

    use super::*;
    use crate::banco::resultados::{FilaDeDesempeno, FILAS_DE_DESEMPENO};
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;
    use crate::registro::modelo::{CodigoPuntuadoCon, EntradaDeModelo, NotaDeClase};
    use crate::revision::catalogo::{leer_busqueda, leer_detalle, FicheroDelRepositorio};
    use crate::revision::pruebas_comunes::{
        sobra_espacio, CatalogoFalso, FuenteFija, BUSQUEDA, CONTENIDO_DE_LA_FUENTE_FIJA,
        CON_LICENCIA,
    };
    use crate::revision::reponer::MotivoSinReponer;

    use crate::revision::pruebas_comunes::GIB;

    /// La búsqueda de siempre, sin mirar los avisos de la reposición.
    fn buscar(config: &ConfigRevision, entorno: &Entorno) -> Resultado<BusquedaHecha> {
        super::buscar(config, entorno, &mut |_| {})
    }

    fn fuente_fija(_: &str, _: &str) -> Box<dyn Fuente> {
        Box::new(FuenteFija)
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

    /// El entorno de la búsqueda, con `registro` escrito en la carpeta y una fuente que da siempre
    /// el mismo contenido.
    fn entorno<'a>(
        catalogo: &'a CatalogoFalso,
        registro: &Registro,
        raiz: &Path,
        ahora: NaiveDateTime,
    ) -> Entorno<'a> {
        let ruta_del_registro = raiz.join("registro-de-modelos.toml");
        modelo::escribir(&ruta_del_registro, registro).unwrap();
        Entorno {
            catalogo,
            tarjeta: Some(tarjeta()),
            margen_vram_mib: 2048,
            ruta_del_registro,
            carpeta_de_revisiones: raiz.join("revisiones"),
            carpeta_de_modelos: raiz.join("modelos"),
            ahora,
            para_reponer: ParaReponer {
                fuente_de: &fuente_fija,
                espacio_libre: &sobra_espacio,
                margen_disco_mib: 0,
                avisar_cada_mib: 512,
                modelo_en_uso: None,
            },
        }
    }

    /// Un modelo medido en la tarjeta de prueba, con `nota` en todas las clases.
    fn medido(nombre: &str, huella: &str, nota: f64) -> EntradaDeModelo {
        let mut e = entrada_de_ejemplo();
        e.nombre = nombre.into();
        e.tarjeta = tarjeta().nombre;
        e.huella_sha256 = huella.into();
        e.codigo_puntuado_con = Some(CodigoPuntuadoCon::PruebasOcultas);
        e.clase = FILAS_DE_DESEMPENO
            .iter()
            .map(|&clase| NotaDeClase {
                clase,
                nota: Some(nota),
                inventados: 0,
                tokens_por_segundo: Some(10.0),
            })
            .collect();
        e
    }

    fn en_hugging_face(nombre: &str, huella: &str) -> FicheroDelRepositorio {
        FicheroDelRepositorio {
            nombre: nombre.into(),
            tamano: Some(CONTENIDO_DE_LA_FUENTE_FIJA.len() as u64),
            huella_sha256: Some(huella.into()),
        }
    }

    fn el_informe(hecha: &BusquedaHecha) -> String {
        ficheros::leer(&hecha.carpeta.join(NOMBRE_DEL_INFORME)).unwrap()
    }

    #[test]
    fn la_busqueda_repone_antes_de_clasificar_y_lo_repuesto_sale_como_ya_registrado() {
        let raiz = tempfile::tempdir().unwrap();
        let huella = ficheros::huella_sha256_de_bytes(CONTENIDO_DE_LA_FUENTE_FIJA);
        let catalogo = CatalogoFalso {
            anadidos: vec![en_hugging_face("Repuesto-Q4_K_M.gguf", &huella)],
            ..CatalogoFalso::default()
        };
        // Sin origen: lo aprende de la búsqueda.
        let registro = Registro {
            modelo: vec![medido("repuesto", &huella, 4.0)],
        };
        let hecha = buscar(
            &config(),
            &entorno(&catalogo, &registro, raiz.path(), a_las(15)),
        )
        .unwrap();
        let nombres: Vec<&str> = hecha
            .reposicion
            .repuestos
            .iter()
            .map(|r| r.nombre.as_str())
            .collect();
        assert_eq!(nombres, ["repuesto"]);
        assert!(raiz.path().join("modelos").join("repuesto.gguf").is_file());
        let informe = el_informe(&hecha);
        assert!(
            informe.contains(
                "## Modelos del registro que faltaban (1)\n\n- `repuesto.gguf`: repuesto"
            ),
            "{informe}"
        );
        let registrados = informe.find("### Ya en el registro").unwrap();
        assert!(
            informe[registrados..].contains("Repuesto-Q4_K_M.gguf"),
            "{informe}"
        );
        assert!(
            !informe.contains("### En el registro, sin fichero"),
            "{informe}"
        );
        let lista =
            lista::leer(&ficheros::leer(&hecha.carpeta.join(NOMBRE_DE_LA_LISTA)).unwrap()).unwrap();
        assert!(lista
            .candidatos
            .iter()
            .all(|c| c.fichero.fichero != "Repuesto-Q4_K_M.gguf"));
    }

    #[test]
    fn el_registro_de_probador_1_con_modelos_vacia() {
        // El registro del 23/11, con huellas cortas de mentira: el modelo por defecto, que no gana
        // nada, y los dos que ganaban una clase cada uno. Hugging Face los tiene con esas huellas,
        // pero lo que se baja no cuadra con ellas: no se repone ninguno, y ninguno sale como «ya
        // en el registro».
        let raiz = tempfile::tempdir().unwrap();
        let por_defecto = "devstral-small-2-24b-Q4_K_M";
        let mut jackod = medido("JackOD-9B-Coder-CD-Q4_K_M", "j1", 2.0);
        let mut qwen = medido("Qwen2.5-Coder-14B-Instruct-Q4_K_M", "q1", 2.0);
        for (modelo, fila) in [
            (&mut jackod, FilaDeDesempeno::Diagnostico),
            (&mut qwen, FilaDeDesempeno::FicheroLimpio),
        ] {
            for clase in modelo.clase.iter_mut().filter(|c| c.clase == fila) {
                clase.nota = Some(5.0);
            }
        }
        let registro = Registro {
            modelo: vec![medido(por_defecto, "d1", 1.0), jackod, qwen],
        };
        let catalogo = CatalogoFalso {
            anadidos: vec![
                en_hugging_face("Devstral-Q4_K_M.gguf", "d1"),
                en_hugging_face("JackOD-Q6_K.gguf", "J1"),
                en_hugging_face("Qwen-Q8_0.gguf", "q1"),
            ],
            ..CatalogoFalso::default()
        };
        let mut con_el_de_por_defecto = entorno(&catalogo, &registro, raiz.path(), a_las(15));
        con_el_de_por_defecto.para_reponer.modelo_en_uso = Some(por_defecto.into());
        let hecha = buscar(&config(), &con_el_de_por_defecto).unwrap();

        let sin_reponer: Vec<&str> = hecha
            .reposicion
            .sin_reponer
            .iter()
            .map(|s| s.nombre.as_str())
            .collect();
        assert_eq!(
            sin_reponer,
            [
                por_defecto,
                "JackOD-9B-Coder-CD-Q4_K_M",
                "Qwen2.5-Coder-14B-Instruct-Q4_K_M"
            ]
        );
        assert!(hecha
            .reposicion
            .sin_reponer
            .iter()
            .all(|s| matches!(s.motivo, MotivoSinReponer::FalloAlDescargar(_))));
        let informe = el_informe(&hecha);
        assert!(!informe.contains("### Ya en el registro"), "{informe}");
        assert!(
            informe.contains("### En el registro, sin fichero"),
            "{informe}"
        );
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
