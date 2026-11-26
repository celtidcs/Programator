//! Un `preparar:` de principio a fin: busca la propuesta, copia el proyecto con ella en su sitio,
//! ejecuta las órdenes de su extensión en la copia y cuenta lo que pasó. El proyecto no se toca.
//!
//! Cada preparación vive en `<[preparacion] carpeta>/<id del encargo>/`: la copia en `proyecto/`,
//! las cachés de las órdenes en `cache/` y la marca de su caducidad al lado.

use super::caducidad::{marcar, Marca, FORMATO_DE_LA_FECHA};
use super::copia::{copiar, Copia, PedidoDeCopia};
use super::diferencias::{diferencias, Diferencias};
use super::mensajes::{self, Informe, SinAislamiento};
use super::ordenes::{ejecutar, entornos_de_la_preparacion, Ejecucion, ResultadoDeOrden};
use super::pedido::Pedido;
use crate::arnes::entregas::RegistroDeEntregas;
use crate::arnes::verificacion::recortar;
use crate::banco::comprobacion::lanzador::LanzadorConEntorno;
use crate::banco::comprobacion::local::{
    crear_las_caches, prefijo_de_aislamiento, probar_el_envoltorio, Entornos,
};
use crate::banco::comprobacion::trampa::Trampa;
use crate::banco::config::ComprobacionLocal;
use crate::config::preparacion::OrdenesDeLaExtension;
use crate::config::Preparacion;
use crate::error::Resultado;
use crate::unidades::BYTES_POR_MIB;
use chrono::NaiveDateTime;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// La carpeta de la copia, dentro de la de cada preparación.
pub const CARPETA_DE_LA_COPIA: &str = "proyecto";
/// La de las cachés de las órdenes (`CARGO_HOME` y demás), al lado de la copia.
pub const CARPETA_DE_LAS_CACHES: &str = "cache";

/// Cuántas celdas puede tener la tabla con que se comparan lo que había y la propuesta: 16 MiB de
/// `u32`, dos ficheros de unas dos mil líneas distintas cada uno.
const TOPE_DE_CELDAS_DE_LAS_DIFERENCIAS: usize = 4 * 1024 * 1024;

/// Lo que hace falta para preparar.
pub struct Entorno<'a> {
    /// La carpeta de trabajo, la raíz del proyecto.
    pub raiz: &'a Path,
    /// La carpeta de candidatos, donde están las propuestas.
    pub candidatos: &'a Path,
    pub entregas: &'a RegistroDeEntregas,
    pub preparacion: &'a Preparacion,
    /// Cómo se aíslan las órdenes: el de las comprobaciones del banco.
    pub comprobacion: &'a ComprobacionLocal,
    /// Las variables de este proceso, para completar las de las órdenes.
    pub sistema: &'a BTreeMap<String, String>,
    /// Lo más que se cuenta de la salida de cada orden y de las diferencias.
    pub maximo_salida: usize,
    pub ahora: NaiveDateTime,
}

/// Prepara lo que pide `pedido`, como el encargo `id`, y devuelve la respuesta. Lo que impide
/// prepararlo también se contesta: un `preparar:` siempre tiene respuesta.
pub fn atender(
    pedido: &Pedido,
    id: &str,
    entorno: &Entorno,
    sin_aislar: &mut dyn LanzadorConEntorno,
    aislado: &mut dyn LanzadorConEntorno,
) -> String {
    if !pedido.esta_completo() {
        return mensajes::uso();
    }
    let Some(origen) = donde_esta(pedido, entorno) else {
        return mensajes::propuesta_desconocida(&pedido.propuesta);
    };
    let carpeta = entorno.raiz.join(&entorno.preparacion.carpeta).join(id);
    match preparar(pedido, &origen, &carpeta, entorno, sin_aislar, aislado) {
        Ok(respuesta) => respuesta,
        Err(motivo) => {
            // Lo que quedara a medias sobra; si no se puede borrar, lo borra la caducidad.
            let _ = crate::ficheros::borrar_si_existe(&carpeta);
            mensajes::fallo(&motivo)
        }
    }
}

/// El fichero de la propuesta: la última entrega con ese nombre, en su sitio o en el archivo.
fn donde_esta(pedido: &Pedido, entorno: &Entorno) -> Option<PathBuf> {
    let entrada = entorno
        .entregas
        .entregas
        .iter()
        .rev()
        .find(|entrada| entrada.propuesta == pedido.propuesta)?;
    let relativa = entrada.archivada.as_deref().unwrap_or(&entrada.propuesta);
    Some(entorno.candidatos.join(relativa))
}

fn preparar(
    pedido: &Pedido,
    origen: &Path,
    carpeta: &Path,
    entorno: &Entorno,
    sin_aislar: &mut dyn LanzadorConEntorno,
    aislado: &mut dyn LanzadorConEntorno,
) -> Resultado<String> {
    let contenido = crate::ficheros::leer(origen)?;
    let extension = Path::new(&pedido.ruta)
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ordenes = entorno
        .preparacion
        .ordenes_de(&extension)
        .cloned()
        .unwrap_or_default();
    let sin_ordenes = ordenes == OrdenesDeLaExtension::default();

    crate::ficheros::borrar_si_existe(carpeta)?;
    crate::ficheros::crear_carpeta(carpeta)?;
    // Antes que lo de dentro, para que lo herede: con integridad baja, la etiqueta.
    if !sin_ordenes {
        aislado.preparar_carpeta(carpeta)?;
    }
    let copia = carpeta.join(CARPETA_DE_LA_COPIA);
    let hecha = copiar(&PedidoDeCopia {
        raiz: entorno.raiz,
        carpeta_de_las_copias: &entorno.raiz.join(&entorno.preparacion.carpeta),
        destino: &copia,
        ruta: &pedido.ruta,
        contenido: &contenido,
        tope_bytes: entorno.preparacion.tope_mib.saturating_mul(BYTES_POR_MIB),
        tiempo: tiempo_maximo(entorno.preparacion),
    })?;
    let (fichero, anterior) = match hecha {
        Copia::Hecha {
            fichero, anterior, ..
        } => (fichero, anterior),
        Copia::PasaDelTope { bytes, tope } => {
            crate::ficheros::borrar_si_existe(carpeta)?;
            return Ok(mensajes::pasa_del_tope(bytes, tope));
        }
        Copia::FueraDelProyecto => {
            crate::ficheros::borrar_si_existe(carpeta)?;
            return Ok(mensajes::fuera_del_proyecto(&pedido.ruta));
        }
    };
    marcar(
        carpeta,
        &Marca {
            propuesta: pedido.propuesta.clone(),
            creada: entorno.ahora.format(FORMATO_DE_LA_FECHA).to_string(),
        },
    )?;

    let lanzadas = if sin_ordenes {
        Lanzadas::default()
    } else {
        ejecutar_en_la_copia(
            &ordenes, carpeta, &copia, &fichero, entorno, sin_aislar, aislado,
        )?
    };

    let formateada = crate::ficheros::leer(&fichero)?;
    let diferencias = diferencias(
        anterior.as_deref().unwrap_or_default(),
        &formateada,
        TOPE_DE_CELDAS_DE_LAS_DIFERENCIAS,
    );
    let texto_de_las_diferencias = match &diferencias {
        Diferencias::Cambios(texto) => recortar(texto, entorno.maximo_salida),
        Diferencias::Iguales | Diferencias::DemasiadoGrandes => String::new(),
    };
    Ok(mensajes::informe(&Informe {
        propuesta: pedido.propuesta.clone(),
        ruta: pedido.ruta.clone(),
        copia: relativa(entorno.raiz, &copia),
        ordenes: lanzadas
            .resultados
            .into_iter()
            .map(|resultado| ResultadoDeOrden {
                salida: recortar(&resultado.salida, entorno.maximo_salida),
                ..resultado
            })
            .collect(),
        sin_ordenes,
        nueva: anterior.is_none(),
        diferencias,
        texto_de_las_diferencias,
        intentos_de_red: lanzadas.intentos_de_red,
        dias_de_vida: entorno.preparacion.dias_de_vida,
        sin_aislamiento: lanzadas.sin_aislamiento,
    }))
}

/// Lo que pasó con las órdenes.
#[derive(Default)]
struct Lanzadas {
    resultados: Vec<ResultadoDeOrden>,
    /// Lo que se vio intentar salir a la red.
    intentos_de_red: Vec<String>,
    /// Si no se lanzó ninguna porque el envoltorio no aísla aquí.
    sin_aislamiento: Option<SinAislamiento>,
}

/// Las órdenes, con las cachés junto a la copia y la trampa de red abierta mientras corren. Antes
/// que ninguna, se prueba el envoltorio: si no aísla, no se lanza nada, ni las previas.
fn ejecutar_en_la_copia(
    ordenes: &OrdenesDeLaExtension,
    carpeta: &Path,
    copia: &Path,
    fichero: &Path,
    entorno: &Entorno,
    sin_aislar: &mut dyn LanzadorConEntorno,
    aislado: &mut dyn LanzadorConEntorno,
) -> Resultado<Lanzadas> {
    let cache = carpeta.join(CARPETA_DE_LAS_CACHES);
    crear_las_caches(entorno.comprobacion, &cache)?;
    let trampa = Trampa::abrir()?;
    let Entornos {
        del_codigo,
        para_preparar,
    } = entornos_de_la_preparacion(
        entorno.comprobacion,
        &cache,
        copia,
        trampa.puerto(),
        entorno.sistema,
    );
    let envoltorio = prefijo_de_aislamiento(entorno.comprobacion);
    if let Err(detalle) = probar_el_envoltorio(envoltorio, aislado, copia, &del_codigo) {
        return Ok(Lanzadas {
            sin_aislamiento: Some(SinAislamiento {
                envoltorio: envoltorio.join(" "),
                detalle,
            }),
            ..Lanzadas::default()
        });
    }
    let resultados = ejecutar(
        ordenes,
        &Ejecucion {
            copia,
            fichero,
            tiempo: tiempo_maximo(entorno.preparacion),
            entorno_de_las_previas: &para_preparar,
            entorno_de_las_aisladas: &del_codigo,
            envoltorio,
        },
        sin_aislar,
        aislado,
    );
    let mut intentos: Vec<String> = trampa
        .recoger()
        .into_iter()
        .chain(
            resultados
                .iter()
                .flat_map(|r| r.vigilancia.intentos_de_red.clone()),
        )
        .collect();
    intentos.sort();
    intentos.dedup();
    Ok(Lanzadas {
        resultados,
        intentos_de_red: intentos,
        sin_aislamiento: None,
    })
}

fn tiempo_maximo(preparacion: &Preparacion) -> Duration {
    Duration::from_secs(preparacion.tiempo_maximo_segundos)
}

/// `ruta` relativa a `raiz`, con barras normales; entera si no cuelga de ella.
fn relativa(raiz: &Path, ruta: &Path) -> String {
    ruta.strip_prefix(raiz)
        .unwrap_or(ruta)
        .display()
        .to_string()
        .replace('\\', "/")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::arnes::entregas::{
        huella, EntradaDeEntrega, EstadoDeLaPropuesta, VeredictoDeLaEntrega, LO_DIO_PROGRAMATOR,
    };
    use crate::banco::comprobacion::lanzador::Vigilancia;
    use crate::orden_acotada::DesenlaceDeOrden;

    /// La palabra con que una orden de [`DeMentira`] intenta salir a la red por el proxy.
    const SALE_A_LA_RED: &str = "sale-a-la-red";

    /// Lo que pide al proxy la orden que intenta salir a la red.
    const DESTINO_PEDIDO: &str = "crates.io:443";

    /// Pide [`DESTINO_PEDIDO`] al proxy de `HTTP_PROXY`, como haría cargo, y espera a que la
    /// trampa corte.
    fn pedir_al_proxy(entorno: &[(String, String)]) {
        use std::io::{Read, Write};
        let proxy = entorno
            .iter()
            .find(|(nombre, _)| nombre == "HTTP_PROXY")
            .map(|(_, valor)| valor.clone())
            .expect("las órdenes aisladas llevan HTTP_PROXY");
        let direccion = proxy.trim_start_matches("http://");
        let mut conexion = std::net::TcpStream::connect(direccion).unwrap();
        conexion
            .write_all(format!("CONNECT {DESTINO_PEDIDO} HTTP/1.1\r\n\r\n").as_bytes())
            .unwrap();
        let mut resto = Vec::new();
        let _ = conexion.read_to_end(&mut resto);
    }

    /// Un lanzador que apunta lo que se le pide; con `fmt`, «formatea» el fichero de la copia
    /// (pone un espacio tras `()`), con `rompe`, falla, y con [`SALE_A_LA_RED`], pide un destino
    /// al proxy trampa.
    #[derive(Default)]
    struct DeMentira {
        lanzadas: Vec<Vec<String>>,
        directorios: Vec<PathBuf>,
        carpetas_preparadas: Vec<PathBuf>,
        /// Si falla al preparar la carpeta, como un Windows que no deja ponerle la etiqueta.
        falla_al_preparar: bool,
    }

    impl LanzadorConEntorno for DeMentira {
        fn preparar_carpeta(&mut self, carpeta: &Path) -> Resultado<()> {
            self.carpetas_preparadas.push(carpeta.to_path_buf());
            if self.falla_al_preparar {
                return Err(crate::error::Error::Aislamiento("sin etiqueta".into()));
            }
            Ok(())
        }

        fn lanzar(
            &mut self,
            argumentos: &[String],
            directorio: &Path,
            entorno: &[(String, String)],
            _tope: Duration,
        ) -> DesenlaceDeOrden {
            if argumentos.iter().any(|a| a == SALE_A_LA_RED) {
                pedir_al_proxy(entorno);
            }
            self.lanzadas.push(argumentos.to_vec());
            self.directorios.push(directorio.to_path_buf());
            if argumentos.first().map(String::as_str) == Some("fmt") {
                let fichero = PathBuf::from(&argumentos[1]);
                let texto = std::fs::read_to_string(&fichero).unwrap();
                std::fs::write(&fichero, texto.replace("(){", "() {")).unwrap();
            }
            let falla = argumentos.iter().any(|a| a == "rompe");
            DesenlaceDeOrden::Terminada {
                exito: !falla,
                codigo: Some(i32::from(falla)),
                estado: String::new(),
                salida: String::new(),
                errores: if falla {
                    "error: roto".into()
                } else {
                    String::new()
                },
            }
        }

        fn vigilancia_de_la_ultima(&mut self) -> Vigilancia {
            Vigilancia::default()
        }
    }

    fn entrada(propuesta: &str, archivada: Option<&str>) -> EntradaDeEntrega {
        EntradaDeEntrega {
            propuesta: propuesta.to_string(),
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: "claude.md".to_string(),
            modelo: None,
            fecha: "2026-11-25 10:00".to_string(),
            veredicto: VeredictoDeLaEntrega::Pasa,
            huella: huella("x"),
            estado: EstadoDeLaPropuesta::Propuesta,
            estado_lo_dio: LO_DIO_PROGRAMATOR.to_string(),
            destino: None,
            motivo: None,
            archivada: archivada.map(String::from),
            ya_en_el_proyecto: Vec::new(),
        }
    }

    fn orden(partes: &[&str]) -> Vec<String> {
        partes.iter().map(|p| p.to_string()).collect()
    }

    /// Un proyecto con `src/lib.rs` y la propuesta `lib-002.rs` en los candidatos.
    struct Proyecto {
        _dir: tempfile::TempDir,
        raiz: PathBuf,
        candidatos: PathBuf,
        entregas: RegistroDeEntregas,
        preparacion: Preparacion,
        comprobacion: ComprobacionLocal,
        sistema: BTreeMap<String, String>,
        maximo_salida: usize,
    }

    impl Proyecto {
        fn nuevo() -> Self {
            let carpeta = tempfile::tempdir().unwrap();
            let raiz = carpeta.path().to_path_buf();
            std::fs::create_dir_all(raiz.join("src")).unwrap();
            std::fs::write(raiz.join("src/lib.rs"), "fn a(){}\n").unwrap();
            let candidatos = raiz.join(".gestor/candidatos/programator");
            std::fs::create_dir_all(&candidatos).unwrap();
            std::fs::write(candidatos.join("lib-002.rs"), "fn b(){}\n").unwrap();
            let mut preparacion = Preparacion::default();
            preparacion.ordenes.insert(
                "rs".into(),
                OrdenesDeLaExtension {
                    previas: vec![orden(&["fetch"])],
                    formatear: orden(&["fmt", "{fichero}"]),
                    aisladas: vec![orden(&["build"]), orden(&["test", "rompe"])],
                },
            );
            Self {
                _dir: carpeta,
                raiz,
                candidatos,
                entregas: RegistroDeEntregas {
                    entregas: vec![entrada("lib-002.rs", None)],
                    ..RegistroDeEntregas::default()
                },
                preparacion,
                comprobacion: ComprobacionLocal {
                    aislamiento: crate::banco::config::Aislamiento::Ninguno,
                    ..ComprobacionLocal::default()
                },
                sistema: BTreeMap::new(),
                maximo_salida: 4000,
            }
        }

        fn entorno(&self) -> Entorno<'_> {
            Entorno {
                raiz: &self.raiz,
                candidatos: &self.candidatos,
                entregas: &self.entregas,
                preparacion: &self.preparacion,
                comprobacion: &self.comprobacion,
                sistema: &self.sistema,
                maximo_salida: self.maximo_salida,
                ahora: NaiveDateTime::parse_from_str("2026-11-25 10:00:00", FORMATO_DE_LA_FECHA)
                    .unwrap(),
            }
        }

        fn preparar(
            &self,
            texto: &str,
            sin_aislar: &mut DeMentira,
            aislado: &mut DeMentira,
        ) -> String {
            let pedido = super::super::pedido::reconocer(texto).unwrap();
            atender(&pedido, "abc", &self.entorno(), sin_aislar, aislado)
        }

        fn carpeta(&self) -> PathBuf {
            self.raiz.join(&self.preparacion.carpeta).join("abc")
        }
    }

    /// F1 y F3: la propuesta, formateada en la copia, y el proyecto sin tocar; cada clase con su
    /// lanzador, todo en la copia, y la carpeta preparada para el aislamiento antes de copiar.
    #[test]
    fn se_prepara_en_la_copia_sin_tocar_el_proyecto() {
        let proyecto = Proyecto::nuevo();
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());

        let respuesta = proyecto.preparar(
            "preparar: lib-002.rs en src/lib.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert_eq!(
            std::fs::read_to_string(proyecto.raiz.join("src/lib.rs")).unwrap(),
            "fn a(){}\n"
        );
        let copia = proyecto.carpeta().join(CARPETA_DE_LA_COPIA);
        assert_eq!(
            std::fs::read_to_string(copia.join("src/lib.rs")).unwrap(),
            "fn b() {}\n"
        );
        assert!(
            respuesta.contains("    - fn a(){}\n    + fn b() {}"),
            "{respuesta}"
        );
        assert!(
            respuesta.contains("- falló (código 1), aislada: test rompe\n    error: roto"),
            "{respuesta}"
        );
        assert!(respuesta.contains("- pasó, aislada: build"), "{respuesta}");
        assert!(
            respuesta.contains("«.gestor/programator/preparacion/abc/proyecto»"),
            "{respuesta}"
        );
        assert_eq!(sin_aislar.lanzadas, [orden(&["fetch"])]);
        assert_eq!(aislado.lanzadas.len(), 3);
        assert!(aislado.directorios.iter().all(|d| d == &copia));
        assert_eq!(aislado.carpetas_preparadas, [proyecto.carpeta()]);
        assert!(proyecto
            .carpeta()
            .join(super::super::caducidad::FICHERO_DE_LA_MARCA)
            .exists());
        // Y las que nombran las variables, como TMPDIR: sin ella, los doctest de cargo fallan.
        assert!(proyecto
            .carpeta()
            .join(CARPETA_DE_LAS_CACHES)
            .join("tmp")
            .is_dir());
    }

    /// Lo que no se puede preparar se contesta, y no deja copia.
    #[test]
    fn lo_que_no_se_puede_preparar_se_contesta() {
        let proyecto = Proyecto::nuevo();
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());
        let mut preparar = |texto: &str| proyecto.preparar(texto, &mut sin_aislar, &mut aislado);

        assert_eq!(preparar("preparar: lib-002.rs"), mensajes::uso());
        assert_eq!(
            preparar("preparar: otra.rs en src/lib.rs"),
            mensajes::propuesta_desconocida("otra.rs")
        );
        // F2.
        assert_eq!(
            preparar("preparar: lib-002.rs en ../fuera.rs"),
            mensajes::fuera_del_proyecto("../fuera.rs")
        );
        assert!(!proyecto.carpeta().exists());
        assert!(!proyecto.raiz.join("../fuera.rs").exists());
        assert!(aislado.lanzadas.is_empty());
    }

    /// Una extensión sin órdenes solo se copia: ni se lanza nada ni se prepara la carpeta.
    #[test]
    fn sin_ordenes_para_la_extension_solo_se_copia() {
        let mut proyecto = Proyecto::nuevo();
        proyecto.preparacion.ordenes.clear();
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());

        let respuesta = proyecto.preparar(
            "preparar: lib-002.rs en src/nuevo.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert!(
            respuesta.contains("No hay órdenes en [preparacion]"),
            "{respuesta}"
        );
        assert!(respuesta.contains("sería un fichero nuevo"), "{respuesta}");
        assert!(aislado.lanzadas.is_empty() && sin_aislar.lanzadas.is_empty());
        assert!(aislado.carpetas_preparadas.is_empty());
        assert!(proyecto
            .carpeta()
            .join(CARPETA_DE_LA_COPIA)
            .join("src/nuevo.rs")
            .exists());
    }

    /// Una propuesta ya archivada se busca en el archivo.
    #[test]
    fn una_propuesta_archivada_se_busca_en_el_archivo() {
        let mut proyecto = Proyecto::nuevo();
        proyecto.preparacion.ordenes.clear();
        let archivo = proyecto.candidatos.join("archivo/2026-11");
        std::fs::create_dir_all(&archivo).unwrap();
        std::fs::write(archivo.join("lib-002.rs"), "fn c(){}\n").unwrap();
        // La que cuenta es la última con ese nombre.
        proyecto.entregas.entregas = vec![
            entrada("lib-002.rs", None),
            entrada("lib-002.rs", Some("archivo/2026-11/lib-002.rs")),
        ];
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());

        proyecto.preparar(
            "preparar: lib-002.rs en src/lib.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert_eq!(
            std::fs::read_to_string(proyecto.carpeta().join("proyecto/src/lib.rs")).unwrap(),
            "fn c(){}\n"
        );
    }

    /// F7 en Ubuntu 24.04 y posteriores: si el envoltorio no aísla, no se lanza ninguna orden, ni
    /// las previas, y se dice; la copia y el cambio sí se cuentan.
    #[test]
    fn si_el_envoltorio_no_aisla_no_se_lanza_nada() {
        let mut proyecto = Proyecto::nuevo();
        proyecto.comprobacion.aislamiento = crate::banco::config::Aislamiento::EspacioDeNombres;
        proyecto.comprobacion.envoltorio = orden(&["rompe"]);
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());

        let respuesta = proyecto.preparar(
            "preparar: lib-002.rs en src/lib.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert!(
            respuesta.contains("No se ha lanzado ninguna orden: no se pudieron aislar con «rompe»"),
            "{respuesta}"
        );
        assert!(respuesta.contains("    + fn b(){}"), "{respuesta}");
        assert!(sin_aislar.lanzadas.is_empty());
        assert_eq!(aislado.lanzadas, [orden(&["rompe", "true"])]);
    }

    /// Con un envoltorio que funciona, se prueba en la copia y va delante de cada orden aislada.
    #[test]
    fn con_un_envoltorio_que_aisla_se_lanza_todo_envuelto() {
        let mut proyecto = Proyecto::nuevo();
        proyecto.comprobacion.aislamiento = crate::banco::config::Aislamiento::EspacioDeNombres;
        proyecto.comprobacion.envoltorio = orden(&["envuelve"]);
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());

        proyecto.preparar(
            "preparar: lib-002.rs en src/lib.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert_eq!(aislado.lanzadas[0], orden(&["envuelve", "true"]));
        assert_eq!(aislado.lanzadas[2], orden(&["envuelve", "build"]));
        assert_eq!(
            aislado.directorios[0],
            proyecto.carpeta().join(CARPETA_DE_LA_COPIA)
        );
        assert_eq!(sin_aislar.lanzadas, [orden(&["fetch"])]);
    }

    /// Lo que una orden aislada pide al proxy trampa sale en la respuesta, no solo lo que ve la
    /// vigilancia de la red.
    #[test]
    fn lo_que_las_ordenes_piden_al_proxy_se_cuenta() {
        let mut proyecto = Proyecto::nuevo();
        if let Some(rs) = proyecto.preparacion.ordenes.get_mut("rs") {
            rs.aisladas = vec![orden(&["build", SALE_A_LA_RED])];
        }
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());

        let respuesta = proyecto.preparar(
            "preparar: lib-002.rs en src/lib.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert!(
            respuesta.contains(&format!("intentaron conectarse a {DESTINO_PEDIDO}")),
            "{respuesta}"
        );
    }

    /// Sobre un fichero que ya existe, la respuesta cuenta lo que cambiaría; sobre uno que no,
    /// que sería nuevo.
    #[test]
    fn la_respuesta_distingue_un_fichero_que_ya_existe_de_uno_nuevo() {
        let proyecto = Proyecto::nuevo();
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());

        let existente = proyecto.preparar(
            "preparar: lib-002.rs en src/lib.rs",
            &mut sin_aislar,
            &mut aislado,
        );
        let nuevo = proyecto.preparar(
            "preparar: lib-002.rs en src/otro.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert!(
            existente.contains("Lo que cambiaría en «src/lib.rs»"),
            "{existente}"
        );
        assert!(
            !existente.contains("no existe en el proyecto"),
            "{existente}"
        );
        assert!(
            nuevo.contains("«src/otro.rs» no existe en el proyecto"),
            "{nuevo}"
        );
    }

    /// Si algo falla a medias, la carpeta de la preparación no se queda: la respuesta dice el fallo.
    #[test]
    fn un_fallo_al_preparar_no_deja_la_carpeta() {
        let proyecto = Proyecto::nuevo();
        let mut sin_aislar = DeMentira::default();
        let mut aislado = DeMentira {
            falla_al_preparar: true,
            ..DeMentira::default()
        };

        let respuesta = proyecto.preparar(
            "preparar: lib-002.rs en src/lib.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert_eq!(aislado.carpetas_preparadas, [proyecto.carpeta()]);
        assert!(!proyecto.carpeta().exists(), "{respuesta}");
        assert!(respuesta.contains("sin etiqueta"), "{respuesta}");
    }

    /// F4: por encima del tope no se copia nada, y se dice.
    #[test]
    fn por_encima_del_tope_no_se_copia_nada() {
        let mut proyecto = Proyecto::nuevo();
        proyecto.preparacion.tope_mib = 0;
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());

        let respuesta = proyecto.preparar(
            "preparar: lib-002.rs en src/lib.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert!(
            respuesta.starts_with("No se ha preparado nada: el proyecto ocupa"),
            "{respuesta}"
        );
        assert!(!proyecto.carpeta().exists());
        assert!(aislado.lanzadas.is_empty());
    }

    /// La salida de cada orden y las diferencias se recortan como las de la verificación.
    #[test]
    fn la_salida_y_las_diferencias_se_recortan() {
        let mut proyecto = Proyecto::nuevo();
        proyecto.maximo_salida = 6;
        let (mut sin_aislar, mut aislado) = (DeMentira::default(), DeMentira::default());

        let respuesta = proyecto.preparar(
            "preparar: lib-002.rs en src/lib.rs",
            &mut sin_aislar,
            &mut aislado,
        );

        assert!(
            respuesta.contains("    error:\n    […recortado]"),
            "{respuesta}"
        );
        assert!(
            respuesta.contains("    - fn a\n    […recortado]"),
            "{respuesta}"
        );
    }
}
