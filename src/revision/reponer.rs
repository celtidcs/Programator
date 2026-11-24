//! Reponer los modelos útiles del registro que faltan en la carpeta de modelos, y sacar del
//! registro los que faltan y no ganan nada.
//!
//! Lo que se decide lo deciden piezas puras: `origenes::completar_origenes` y
//! `ausentes::clasificar`. Aquí se lee y se escribe el registro, se pregunta a Hugging Face y se
//! descarga con la pieza de siempre (`traer::traer_fichero`). Un fallo con un modelo no para los
//! demás: se anota y se sigue.

use std::path::Path;

use super::ausentes::{clasificar, Util};
use super::catalogo::{CatalogoDeModelos, DetalleDeRepositorio};
use super::descarga::Aviso;
use super::mensajes;
use super::origenes::{self, completar_origenes, ListaIlegible};
use super::proponer::ficheros_con_tamano;
use super::propuesta::ParaRetirar;
use super::traer::{self, que_hay_en, traer_fichero, EnElDestino, FicheroPedido};
use crate::error::Resultado;
use crate::motor::fichero_de_modelo::con_extension;
use crate::registro::modelo::{self, Registro};

/// Lo que la reposición necesita del mundo.
pub struct Entorno<'a> {
    /// Las carpetas de modelos y de revisiones, los márgenes, la fuente y el disco.
    pub descarga: traer::Entorno<'a>,
    pub catalogo: &'a dyn CatalogoDeModelos,
    /// Los repositorios que ya contestó la búsqueda en curso: no se vuelven a pedir.
    pub detalles: &'a [DetalleDeRepositorio],
    pub tarjeta: &'a str,
    /// El nombre, sin extensión, del modelo de la configuración.
    pub modelo_en_uso: Option<&'a str>,
    pub ruta_del_registro: &'a Path,
    /// El día de hoy, `AAAA-MM-DD`, para la retirada.
    pub dia: &'a str,
}

/// En qué quedó la reposición.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reposicion {
    pub repuestos: Vec<Repuesto>,
    pub sin_reponer: Vec<SinReponer>,
    /// Los que faltaban y no ganan nada: quedan retirados en esta tarjeta.
    pub fuera_del_registro: Vec<String>,
    /// Los que faltan y no se sabe si son útiles: ni se reponen ni se retiran.
    pub sin_decidir: Vec<String>,
    /// Cuántas entradas del registro ganaron su origen.
    pub origenes_completados: usize,
    /// Las listas de búsquedas anteriores que no se pudieron leer.
    pub listas_ilegibles: Vec<ListaIlegible>,
}

impl Reposicion {
    /// Si faltaba algún modelo del registro.
    pub fn hubo_ausentes(&self) -> bool {
        !(self.repuestos.is_empty()
            && self.sin_reponer.is_empty()
            && self.fuera_del_registro.is_empty()
            && self.sin_decidir.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repuesto {
    pub nombre: String,
    /// Lo que pesa el fichero.
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinReponer {
    pub nombre: String,
    pub motivo: MotivoSinReponer,
}

/// Por qué no se repuso un modelo útil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotivoSinReponer {
    SinOrigen,
    FalloAlConsultar(String),
    YaNoEstaEnElSitio,
    HuellaCambiada,
    SinTamano,
    HayOtroFichero,
    FalloAlDescargar(String),
}

/// Lo que va pasando, para decirlo según ocurre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvisoDeReposicion {
    Empieza {
        nombre: String,
        repositorio: String,
        bytes: u64,
    },
    Descarga(Aviso),
}

pub fn reponer(
    entorno: &Entorno,
    avisar: &mut dyn FnMut(AvisoDeReposicion),
) -> Resultado<Reposicion> {
    let mut registro = modelo::leer(entorno.ruta_del_registro)?;
    let de_las_listas = origenes::de_las_listas(&entorno.descarga.carpeta_de_revisiones)?;
    let mut conocidos = origenes::de_la_busqueda(entorno.detalles);
    conocidos.extend(de_las_listas.conocidos);
    let origenes_completados = completar_origenes(&mut registro, &conocidos);
    let en_modelos = ficheros_con_tamano(&entorno.descarga.carpeta_de_modelos)?;
    let ausentes = clasificar(
        &registro,
        entorno.tarjeta,
        &en_modelos,
        entorno.modelo_en_uso,
    );
    let mut reposicion = Reposicion {
        sin_decidir: ausentes.sin_decidir.clone(),
        origenes_completados,
        listas_ilegibles: de_las_listas.ilegibles,
        ..Reposicion::default()
    };
    for util in &ausentes.utiles {
        match reponer_uno(util, entorno, avisar) {
            Ok(repuesto) => reposicion.repuestos.push(repuesto),
            Err(motivo) => reposicion.sin_reponer.push(SinReponer {
                nombre: util.nombre.clone(),
                motivo,
            }),
        }
    }
    reposicion.fuera_del_registro = retirar_los_no_utiles(
        &mut registro,
        &ausentes.no_utiles,
        entorno.tarjeta,
        entorno.dia,
    );
    if origenes_completados > 0 || !reposicion.fuera_del_registro.is_empty() {
        modelo::escribir(entorno.ruta_del_registro, &registro)?;
    }
    Ok(reposicion)
}

/// Antes de bajar nada se comprueba que el fichero de su origen sigue siendo el que se midió.
fn reponer_uno(
    util: &Util,
    entorno: &Entorno,
    avisar: &mut dyn FnMut(AvisoDeReposicion),
) -> Result<Repuesto, MotivoSinReponer> {
    let origen = util.origen.as_ref().ok_or(MotivoSinReponer::SinOrigen)?;
    let detalle = detalle_de(&origen.repositorio, entorno)?;
    let en_el_sitio = detalle
        .ficheros
        .iter()
        .find(|f| f.nombre == origen.fichero)
        .ok_or(MotivoSinReponer::YaNoEstaEnElSitio)?;
    let misma_huella = en_el_sitio
        .huella_sha256
        .as_deref()
        .is_some_and(|h| h.eq_ignore_ascii_case(&util.huella));
    if !misma_huella {
        return Err(MotivoSinReponer::HuellaCambiada);
    }
    let bytes = en_el_sitio.tamano.ok_or(MotivoSinReponer::SinTamano)?;
    let repuesto = Repuesto {
        nombre: util.nombre.clone(),
        bytes,
    };
    let destino = entorno
        .descarga
        .carpeta_de_modelos
        .join(con_extension(&util.nombre));
    match que_hay_en(&destino, &util.huella) {
        Ok(EnElDestino::ElMismo) => return Ok(repuesto),
        Ok(EnElDestino::Otro) => return Err(MotivoSinReponer::HayOtroFichero),
        Ok(EnElDestino::Nada) => {}
        Err(fallo) => return Err(MotivoSinReponer::FalloAlDescargar(fallo.to_string())),
    }
    avisar(AvisoDeReposicion::Empieza {
        nombre: util.nombre.clone(),
        repositorio: origen.repositorio.clone(),
        bytes,
    });
    let pedido = FicheroPedido {
        repositorio: &origen.repositorio,
        fichero_en_el_sitio: &origen.fichero,
        destino: &destino,
        huella: &util.huella,
        tamano: bytes,
    };
    traer_fichero(&pedido, &entorno.descarga, &mut |aviso| {
        avisar(AvisoDeReposicion::Descarga(aviso))
    })
    .map_err(|fallo| MotivoSinReponer::FalloAlDescargar(fallo.to_string()))?;
    Ok(repuesto)
}

/// El detalle que ya contestó la búsqueda o, si no se consultó, el que dé Hugging Face ahora.
fn detalle_de(
    repositorio: &str,
    entorno: &Entorno,
) -> Result<DetalleDeRepositorio, MotivoSinReponer> {
    if let Some(ya) = entorno
        .detalles
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case(repositorio))
    {
        return Ok(ya.clone());
    }
    entorno
        .catalogo
        .detalle(repositorio)
        .map(|respuesta| respuesta.datos)
        .map_err(|fallo| MotivoSinReponer::FalloAlConsultar(fallo.to_string()))
}

/// Saca del registro de `ruta_del_registro` los no útiles, como la reposición, y lo escribe si
/// sacó alguno. Es la que usa `revisión: cerrar`. Devuelve sus nombres.
pub(crate) fn sacar_del_registro(
    ruta_del_registro: &Path,
    no_utiles: &[ParaRetirar],
    tarjeta: &str,
    dia: &str,
) -> Resultado<Vec<String>> {
    if no_utiles.is_empty() {
        return Ok(Vec::new());
    }
    let mut registro = modelo::leer(ruta_del_registro)?;
    let fuera = retirar_los_no_utiles(&mut registro, no_utiles, tarjeta, dia);
    modelo::escribir(ruta_del_registro, &registro)?;
    Ok(fuera)
}

/// Marca retiradas en esta tarjeta las entradas de los no útiles, con su motivo. Las de otras
/// tarjetas no se tocan: allí el mismo modelo puede ganar. Devuelve sus nombres.
fn retirar_los_no_utiles(
    registro: &mut Registro,
    no_utiles: &[ParaRetirar],
    tarjeta: &str,
    dia: &str,
) -> Vec<String> {
    for retirar in no_utiles {
        let motivo = mensajes::motivo_de_retirada_por_ausencia(retirar);
        for entrada in registro.modelo.iter_mut().filter(|e| {
            e.tarjeta == tarjeta && e.retirado_el.is_none() && e.nombre == retirar.nombre
        }) {
            entrada.retirado_el = Some(dia.to_string());
            entrada.motivo_de_retirada = Some(motivo.clone());
        }
    }
    no_utiles.iter().map(|r| r.nombre.clone()).collect()
}

#[cfg(test)]
mod pruebas {
    use std::cell::RefCell;

    use super::*;
    use crate::banco::resultados::{FilaDeDesempeno, FILAS_DE_DESEMPENO};
    use crate::error::Error;
    use crate::ficheros;
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;
    use crate::registro::modelo::{
        CodigoPuntuadoCon, EntradaDeModelo, NotaDeClase, NotasDeFamilia, Origen,
    };
    use crate::revision::catalogo::{Busqueda, FicheroDelRepositorio, Repositorio, Respuesta};
    use crate::revision::descarga::Fuente;
    use crate::revision::pruebas_comunes::{
        sobra_espacio, FuenteFija, CONTENIDO_DE_LA_FUENTE_FIJA as CONTENIDO,
    };

    const TARJETA: &str = "NVIDIA GeForce RTX 4070 Ti SUPER";
    const DIA: &str = "2026-11-24";

    fn huella_del_contenido() -> String {
        ficheros::huella_sha256_de_bytes(CONTENIDO)
    }

    /// Contesta el detalle de los repositorios que se le dan, y anota cuáles se le pidieron. No
    /// busca: la reposición no busca.
    #[derive(Default)]
    struct CatalogoDeDetalles {
        detalles: Vec<DetalleDeRepositorio>,
        pedidos: RefCell<Vec<String>>,
    }

    impl CatalogoDeModelos for CatalogoDeDetalles {
        fn buscar(&self, _: &Busqueda) -> Resultado<Respuesta<Vec<Repositorio>>> {
            Err(Error::Revision("la reposición no busca".into()))
        }

        fn detalle(&self, repositorio: &str) -> Resultado<Respuesta<DetalleDeRepositorio>> {
            self.pedidos.borrow_mut().push(repositorio.to_string());
            let datos = self
                .detalles
                .iter()
                .find(|d| d.id == repositorio)
                .cloned()
                .ok_or_else(|| Error::Revision(format!("no existe «{repositorio}»")))?;
            Ok(Respuesta {
                datos,
                en_bruto: String::new(),
            })
        }
    }

    /// Un repositorio con un fichero, de la huella y el tamaño que se digan.
    fn repositorio(id: &str, fichero: &str, huella: Option<&str>) -> DetalleDeRepositorio {
        DetalleDeRepositorio {
            id: id.into(),
            ultima_modificacion: None,
            descargas: None,
            licencia: None,
            ficheros: vec![FicheroDelRepositorio {
                nombre: fichero.into(),
                tamano: Some(CONTENIDO.len() as u64),
                huella_sha256: huella.map(str::to_string),
            }],
        }
    }

    /// Un modelo de esta tarjeta con la misma nota en todo, con la huella del contenido de la
    /// fuente fija y su origen en `r/<nombre>`.
    fn modelo(nombre: &str, nota: f64) -> EntradaDeModelo {
        let mut e = entrada_de_ejemplo();
        e.nombre = nombre.into();
        e.tarjeta = TARJETA.into();
        e.tanda = format!("tanda-{nombre}");
        e.huella_sha256 = huella_del_contenido();
        e.codigo_puntuado_con = Some(CodigoPuntuadoCon::PruebasOcultas);
        e.familias = NotasDeFamilia {
            oficio: Some(nota),
            criterio: Some(nota),
            uso: Some(nota),
        };
        e.clase = FILAS_DE_DESEMPENO
            .iter()
            .map(|&fila| NotaDeClase {
                clase: fila,
                nota: Some(nota),
                inventados: 0,
                tokens_por_segundo: Some(10.0),
            })
            .collect();
        e.origen = Some(Origen {
            repositorio: format!("r/{nombre}"),
            fichero: format!("{nombre}-Q4_K_M.gguf"),
            tamano: Some(CONTENIDO.len() as u64),
        });
        e
    }

    /// El repositorio del origen de `modelo(nombre, _)`, con el fichero de la fuente fija.
    fn su_repositorio(nombre: &str) -> DetalleDeRepositorio {
        repositorio(
            &format!("r/{nombre}"),
            &format!("{nombre}-Q4_K_M.gguf"),
            Some(&huella_del_contenido()),
        )
    }

    /// Un modelo que está en la carpeta de modelos, con otra huella: no se repone.
    fn presente(nombre: &str, nota: f64) -> EntradaDeModelo {
        let mut e = modelo(nombre, nota);
        e.huella_sha256 = format!("huella-{nombre}");
        e
    }

    /// Una carpeta temporal con el registro, la carpeta de modelos (con los ficheros de
    /// `presentes`) y la de revisiones.
    struct Mesa {
        raiz: tempfile::TempDir,
    }

    impl Mesa {
        fn nueva(registro: &Registro, presentes: &[&str]) -> Self {
            let raiz = tempfile::tempdir().unwrap();
            let mesa = Self { raiz };
            ficheros::crear_carpeta(&mesa.modelos()).unwrap();
            for nombre in presentes {
                ficheros::escribir(&mesa.modelos().join(con_extension(nombre)), "presente")
                    .unwrap();
            }
            modelo::escribir(&mesa.registro(), registro).unwrap();
            mesa
        }

        fn modelos(&self) -> std::path::PathBuf {
            self.raiz.path().join("modelos")
        }

        fn registro(&self) -> std::path::PathBuf {
            self.raiz.path().join("registro-de-modelos.toml")
        }

        fn leer_registro(&self) -> Registro {
            modelo::leer(&self.registro()).unwrap()
        }
    }

    /// Lo que pasó al reponer: el resultado, los ficheros que se abrieron y los avisos.
    struct Hecho {
        reposicion: Reposicion,
        abiertos: Vec<(String, String)>,
        avisos: Vec<AvisoDeReposicion>,
    }

    fn reponer_en(
        mesa: &Mesa,
        catalogo: &CatalogoDeDetalles,
        detalles: &[DetalleDeRepositorio],
        en_uso: Option<&str>,
    ) -> Hecho {
        let abiertos = RefCell::new(Vec::new());
        let fuente_de = |repositorio: &str, fichero: &str| -> Box<dyn Fuente> {
            abiertos
                .borrow_mut()
                .push((repositorio.to_string(), fichero.to_string()));
            Box::new(FuenteFija)
        };
        let ruta_del_registro = mesa.registro();
        let entorno = Entorno {
            descarga: traer::Entorno {
                carpeta_de_revisiones: mesa.raiz.path().join("revisiones"),
                carpeta_de_modelos: mesa.modelos(),
                margen_disco_mib: 0,
                avisar_cada_mib: 512,
                fuente_de: &fuente_de,
                espacio_libre: &sobra_espacio,
            },
            catalogo,
            detalles,
            tarjeta: TARJETA,
            modelo_en_uso: en_uso,
            ruta_del_registro: &ruta_del_registro,
            dia: DIA,
        };
        let mut avisos = Vec::new();
        let reposicion = reponer(&entorno, &mut |aviso| avisos.push(aviso)).unwrap();
        Hecho {
            reposicion,
            abiertos: abiertos.into_inner(),
            avisos,
        }
    }

    fn catalogo_con(detalles: Vec<DetalleDeRepositorio>) -> CatalogoDeDetalles {
        CatalogoDeDetalles {
            detalles,
            ..CatalogoDeDetalles::default()
        }
    }

    fn sin_reponer(nombre: &str, motivo: MotivoSinReponer) -> Vec<SinReponer> {
        vec![SinReponer {
            nombre: nombre.into(),
            motivo,
        }]
    }

    #[test]
    fn un_util_ausente_se_baja_con_el_nombre_del_registro_y_su_huella() {
        let registro = Registro {
            modelo: vec![presente("a", 3.0), modelo("b", 4.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let catalogo = catalogo_con(vec![su_repositorio("b")]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert_eq!(
            hecho.reposicion.repuestos,
            [Repuesto {
                nombre: "b".into(),
                bytes: CONTENIDO.len() as u64,
            }]
        );
        assert!(hecho.reposicion.sin_reponer.is_empty());
        assert_eq!(
            hecho.abiertos,
            [("r/b".to_string(), "b-Q4_K_M.gguf".to_string())]
        );
        assert_eq!(
            std::fs::read(mesa.modelos().join("b.gguf")).unwrap(),
            CONTENIDO
        );
        assert_eq!(
            hecho.avisos.first(),
            Some(&AvisoDeReposicion::Empieza {
                nombre: "b".into(),
                repositorio: "r/b".into(),
                bytes: CONTENIDO.len() as u64,
            })
        );
    }

    #[test]
    fn sin_origen_no_se_pide_nada_y_queda_sin_reponer() {
        let mut b = modelo("b", 4.0);
        b.origen = None;
        let registro = Registro {
            modelo: vec![presente("a", 3.0), b],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let catalogo = catalogo_con(vec![su_repositorio("b")]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert_eq!(
            hecho.reposicion.sin_reponer,
            sin_reponer("b", MotivoSinReponer::SinOrigen)
        );
        assert!(catalogo.pedidos.borrow().is_empty());
        assert!(hecho.abiertos.is_empty());
    }

    #[test]
    fn con_la_huella_cambiada_en_el_sitio_no_se_descarga() {
        let registro = Registro {
            modelo: vec![presente("a", 3.0), modelo("b", 4.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let catalogo = catalogo_con(vec![repositorio("r/b", "b-Q4_K_M.gguf", Some("otra"))]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert_eq!(
            hecho.reposicion.sin_reponer,
            sin_reponer("b", MotivoSinReponer::HuellaCambiada)
        );
        assert!(hecho.abiertos.is_empty());
        assert!(!mesa.modelos().join("b.gguf").exists());
    }

    #[test]
    fn si_el_fichero_ya_no_esta_en_el_repositorio_no_se_descarga() {
        let registro = Registro {
            modelo: vec![presente("a", 3.0), modelo("b", 4.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let otro_fichero = repositorio("r/b", "b-Q8_0.gguf", Some(&huella_del_contenido()));
        let catalogo = catalogo_con(vec![otro_fichero]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert_eq!(
            hecho.reposicion.sin_reponer,
            sin_reponer("b", MotivoSinReponer::YaNoEstaEnElSitio)
        );
        assert!(hecho.abiertos.is_empty());
    }

    #[test]
    fn un_fallo_de_descarga_no_para_el_siguiente() {
        // b gana el diagnóstico y c todo lo demás: los dos son útiles. La huella de b, en el
        // registro y en el sitio, no es la de lo que se baja: su descarga falla al comprobarla.
        let mut b = modelo("b", 2.0);
        b.huella_sha256 = "huella-de-b".into();
        for clase in b
            .clase
            .iter_mut()
            .filter(|c| c.clase == FilaDeDesempeno::Diagnostico)
        {
            clase.nota = Some(5.0);
        }
        let registro = Registro {
            modelo: vec![presente("a", 1.0), b, modelo("c", 3.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let catalogo = catalogo_con(vec![
            repositorio("r/b", "b-Q4_K_M.gguf", Some("huella-de-b")),
            su_repositorio("c"),
        ]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert_eq!(hecho.reposicion.sin_reponer.len(), 1);
        assert_eq!(hecho.reposicion.sin_reponer[0].nombre, "b");
        assert!(matches!(
            hecho.reposicion.sin_reponer[0].motivo,
            MotivoSinReponer::FalloAlDescargar(_)
        ));
        assert_eq!(
            hecho
                .reposicion
                .repuestos
                .iter()
                .map(|r| r.nombre.as_str())
                .collect::<Vec<_>>(),
            ["c"]
        );
        assert!(mesa.modelos().join("c.gguf").is_file());
        assert!(!mesa.modelos().join("b.gguf").exists());
    }

    #[test]
    fn uno_que_ya_esta_con_su_huella_no_se_baja() {
        // a está en la carpeta y gana el diagnóstico; b falta y gana lo demás. Solo se baja b.
        let mut a = presente("a", 1.0);
        for clase in a
            .clase
            .iter_mut()
            .filter(|c| c.clase == FilaDeDesempeno::Diagnostico)
        {
            clase.nota = Some(5.0);
        }
        let registro = Registro {
            modelo: vec![a, modelo("b", 3.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let catalogo = catalogo_con(vec![su_repositorio("a"), su_repositorio("b")]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert_eq!(
            hecho.abiertos,
            [("r/b".to_string(), "b-Q4_K_M.gguf".to_string())]
        );
        assert_eq!(
            ficheros::leer(&mesa.modelos().join("a.gguf")).unwrap(),
            "presente"
        );
    }

    #[test]
    fn los_no_utiles_quedan_retirados_en_esta_tarjeta_y_no_en_otra() {
        let mut en_otra = modelo("b", 2.0);
        en_otra.tarjeta = "otra".into();
        let registro = Registro {
            modelo: vec![presente("a", 4.0), modelo("b", 2.0), en_otra],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let catalogo = catalogo_con(vec![]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert_eq!(hecho.reposicion.fuera_del_registro, ["b"]);
        assert!(hecho.reposicion.repuestos.is_empty());
        assert!(hecho.abiertos.is_empty());
        let leido = mesa.leer_registro();
        let de = |tarjeta: &str| {
            leido
                .modelo
                .iter()
                .find(|e| e.nombre == "b" && e.tarjeta == tarjeta)
                .unwrap()
                .clone()
        };
        assert_eq!(de(TARJETA).retirado_el.as_deref(), Some(DIA));
        let motivo = de(TARJETA).motivo_de_retirada.unwrap();
        assert!(
            motivo.starts_with("Faltaba su fichero en «modelos». No gana ninguna clase"),
            "{motivo}"
        );
        assert_eq!(de("otra").retirado_el, None);
    }

    #[test]
    fn el_origen_aprendido_de_la_busqueda_queda_en_el_registro() {
        let mut b = modelo("b", 4.0);
        b.origen = None;
        let registro = Registro {
            modelo: vec![presente("a", 3.0), b],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let catalogo = catalogo_con(vec![]);
        let hecho = reponer_en(&mesa, &catalogo, &[su_repositorio("b")], None);
        assert_eq!(hecho.reposicion.origenes_completados, 1);
        assert_eq!(hecho.reposicion.repuestos.len(), 1);
        // El detalle ya lo había contestado la búsqueda: no se vuelve a pedir.
        assert!(catalogo.pedidos.borrow().is_empty());
        let leido = mesa.leer_registro();
        let origen = leido
            .modelo
            .iter()
            .find(|e| e.nombre == "b")
            .and_then(|e| e.origen.clone());
        assert_eq!(
            origen,
            Some(Origen {
                repositorio: "r/b".into(),
                fichero: "b-Q4_K_M.gguf".into(),
                tamano: Some(CONTENIDO.len() as u64),
            })
        );
    }

    #[test]
    fn sin_ausentes_no_se_escribe_el_registro() {
        let registro = Registro {
            modelo: vec![presente("a", 3.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        // Un comentario que se perdería si se volviera a escribir el registro.
        let a_mano = format!(
            "# puesto a mano\n{}",
            ficheros::leer(&mesa.registro()).unwrap()
        );
        ficheros::escribir(&mesa.registro(), &a_mano).unwrap();
        let catalogo = catalogo_con(vec![]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert!(!hecho.reposicion.hubo_ausentes());
        assert_eq!(ficheros::leer(&mesa.registro()).unwrap(), a_mano);
    }

    #[test]
    fn sacar_del_registro_retira_los_no_utiles_y_sin_ninguno_no_lo_escribe() {
        let registro = Registro {
            modelo: vec![presente("a", 4.0), modelo("b", 2.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let a_mano = format!(
            "# puesto a mano\n{}",
            ficheros::leer(&mesa.registro()).unwrap()
        );
        ficheros::escribir(&mesa.registro(), &a_mano).unwrap();

        let nada = sacar_del_registro(&mesa.registro(), &[], TARJETA, DIA).unwrap();
        assert!(nada.is_empty());
        assert_eq!(ficheros::leer(&mesa.registro()).unwrap(), a_mano);

        let en_modelos = [(con_extension("a"), 1)];
        let no_utiles = clasificar(&registro, TARJETA, &en_modelos, None).no_utiles;
        let fuera = sacar_del_registro(&mesa.registro(), &no_utiles, TARJETA, DIA).unwrap();
        assert_eq!(fuera, ["b"]);
        let leido = mesa.leer_registro();
        let b = leido.modelo.iter().find(|e| e.nombre == "b").unwrap();
        assert_eq!(b.retirado_el.as_deref(), Some(DIA));
    }

    #[test]
    fn sin_tamano_en_el_sitio_no_se_descarga() {
        let registro = Registro {
            modelo: vec![presente("a", 3.0), modelo("b", 4.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let mut sin_tamano = su_repositorio("b");
        sin_tamano.ficheros[0].tamano = None;
        let catalogo = catalogo_con(vec![sin_tamano]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert_eq!(
            hecho.reposicion.sin_reponer,
            sin_reponer("b", MotivoSinReponer::SinTamano)
        );
        assert!(hecho.abiertos.is_empty());
        assert!(!mesa.modelos().join("b.gguf").exists());
    }

    /// En Windows, «B.gguf» y «b.gguf» son el mismo fichero, pero la carpeta lo lista como «B» y
    /// «b» cuenta como ausente: es el caso en que la reposición se encuentra algo en el destino.
    #[cfg(windows)]
    #[test]
    fn con_otro_fichero_en_el_destino_no_se_pisa() {
        let registro = Registro {
            modelo: vec![presente("a", 3.0), modelo("b", 4.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        let en_el_destino = mesa.modelos().join("B.gguf");
        ficheros::escribir(&en_el_destino, "otro contenido").unwrap();
        let catalogo = catalogo_con(vec![su_repositorio("b")]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        assert_eq!(
            hecho.reposicion.sin_reponer,
            sin_reponer("b", MotivoSinReponer::HayOtroFichero)
        );
        assert!(hecho.abiertos.is_empty());
        assert_eq!(ficheros::leer(&en_el_destino).unwrap(), "otro contenido");
    }

    /// El mismo caso que el anterior, con el fichero que se midió: no se baja y cuenta como
    /// repuesto.
    #[cfg(windows)]
    #[test]
    fn con_el_mismo_fichero_en_el_destino_no_se_baja() {
        let registro = Registro {
            modelo: vec![presente("a", 3.0), modelo("b", 4.0)],
        };
        let mesa = Mesa::nueva(&registro, &["a"]);
        std::fs::write(mesa.modelos().join("B.gguf"), CONTENIDO).unwrap();
        let catalogo = catalogo_con(vec![su_repositorio("b")]);
        let hecho = reponer_en(&mesa, &catalogo, &[], None);
        // Ni se anuncia una descarga que no va a haber.
        assert!(hecho.avisos.is_empty(), "{:?}", hecho.avisos);
        assert_eq!(
            hecho.reposicion.repuestos,
            [Repuesto {
                nombre: "b".into(),
                bytes: CONTENIDO.len() as u64,
            }]
        );
        assert!(hecho.abiertos.is_empty());
    }

    #[test]
    fn los_que_quedan_sin_decidir_tambien_son_ausentes() {
        assert!(!Reposicion::default().hubo_ausentes());
        let sin_decidir = Reposicion {
            sin_decidir: vec!["a".into()],
            ..Reposicion::default()
        };
        assert!(sin_decidir.hubo_ausentes());
    }
}
