//! Las órdenes de `[preparacion]` en la copia: primero las previas, sin aislar, para bajar
//! dependencias; luego la de formatear y las aisladas, con el mismo aislamiento que las
//! comprobaciones del banco (integridad baja y un Job en Windows, un espacio de nombres sin red en
//! Linux).
//!
//! Qué se ejecuta y en qué orden se decide aquí; cómo se crea el proceso lo decide el lanzador
//! (`banco::comprobacion::lanzador`), así que se prueba con uno de mentira. Una orden que falla se
//! cuenta con su salida y las demás siguen, salvo la de formatear: si falla, lo demás probaría un
//! fichero sin formatear y no se sigue.

use crate::banco::comprobacion::lanzador::{LanzadorConEntorno, Vigilancia};
use crate::banco::comprobacion::local::{entornos, Entornos};
use crate::banco::config::ComprobacionLocal;
use crate::config::preparacion::OrdenesDeLaExtension;
use crate::orden_acotada::DesenlaceDeOrden;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// Lo que en una orden se sustituye por la ruta de la propuesta dentro de la copia.
pub const MARCADOR_DEL_FICHERO: &str = "{fichero}";

/// Las tres clases de órdenes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Previa,
    Formatear,
    Aislada,
}

/// En qué quedó una orden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Desenlace {
    Paso,
    /// Terminó con un código distinto de 0, o por una señal (`None`).
    Fallo(Option<i32>),
    /// Seguía en marcha al llegar al tiempo máximo.
    Agotada,
    /// No se pudo lanzar o esperar; el motivo.
    NoSeLanzo(String),
}

/// Lo que se cuenta de cada orden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoDeOrden {
    pub clase: Clase,
    pub orden: Vec<String>,
    pub desenlace: Desenlace,
    /// Lo que escribió, salida y errores juntos.
    pub salida: String,
    /// Lo que se vio de la red mientras corría (en Windows, con integridad baja). Las previas no
    /// se vigilan: bajan dependencias.
    pub vigilancia: Vigilancia,
}

impl ResultadoDeOrden {
    pub fn paso(&self) -> bool {
        self.desenlace == Desenlace::Paso
    }
}

/// Dónde y cómo se ejecutan.
pub struct Ejecucion<'a> {
    /// La copia: el directorio de trabajo de todas.
    pub copia: &'a Path,
    /// La propuesta dentro de la copia, para `{fichero}`.
    pub fichero: &'a Path,
    pub tiempo: Duration,
    /// Las variables de las previas, con red.
    pub entorno_de_las_previas: &'a [(String, String)],
    /// Las de la de formatear y las aisladas, sin red.
    pub entorno_de_las_aisladas: &'a [(String, String)],
    /// Lo que se antepone a las aisladas (el `unshare` de Linux); vacío en Windows, donde aísla el
    /// lanzador.
    pub envoltorio: &'a [String],
}

/// Ejecuta las órdenes de la extensión y devuelve lo que pasó con cada una, en orden.
pub fn ejecutar(
    ordenes: &OrdenesDeLaExtension,
    ejecucion: &Ejecucion,
    sin_aislar: &mut dyn LanzadorConEntorno,
    aislado: &mut dyn LanzadorConEntorno,
) -> Vec<ResultadoDeOrden> {
    let mut resultados: Vec<ResultadoDeOrden> = ordenes
        .previas
        .iter()
        .map(|orden| {
            let (desenlace, salida) = lanzar(
                orden,
                ejecucion,
                sin_aislar,
                &[],
                ejecucion.entorno_de_las_previas,
            );
            resultado(
                Clase::Previa,
                orden,
                ejecucion,
                desenlace,
                salida,
                Vigilancia::default(),
            )
        })
        .collect();
    if !ordenes.formatear.is_empty() {
        let formato = lanzar_aislada(Clase::Formatear, &ordenes.formatear, ejecucion, aislado);
        let corta = !formato.paso();
        resultados.push(formato);
        if corta {
            return resultados;
        }
    }
    resultados.extend(
        ordenes
            .aisladas
            .iter()
            .map(|orden| lanzar_aislada(Clase::Aislada, orden, ejecucion, aislado)),
    );
    resultados
}

/// Una de formatear o aislada: con el envoltorio, sin red y vigilada.
fn lanzar_aislada(
    clase: Clase,
    orden: &[String],
    ejecucion: &Ejecucion,
    lanzador: &mut dyn LanzadorConEntorno,
) -> ResultadoDeOrden {
    let (desenlace, salida) = lanzar(
        orden,
        ejecucion,
        lanzador,
        ejecucion.envoltorio,
        ejecucion.entorno_de_las_aisladas,
    );
    let vigilancia = lanzador.vigilancia_de_la_ultima();
    resultado(clase, orden, ejecucion, desenlace, salida, vigilancia)
}

/// La orden con `{fichero}` sustituido.
fn con_el_fichero(orden: &[String], ejecucion: &Ejecucion) -> Vec<String> {
    let fichero = ejecucion.fichero.display().to_string();
    orden
        .iter()
        .map(|parte| parte.replace(MARCADOR_DEL_FICHERO, &fichero))
        .collect()
}

fn resultado(
    clase: Clase,
    orden: &[String],
    ejecucion: &Ejecucion,
    desenlace: Desenlace,
    salida: String,
    vigilancia: Vigilancia,
) -> ResultadoDeOrden {
    ResultadoDeOrden {
        clase,
        orden: con_el_fichero(orden, ejecucion),
        desenlace,
        salida,
        vigilancia,
    }
}

fn lanzar(
    orden: &[String],
    ejecucion: &Ejecucion,
    lanzador: &mut dyn LanzadorConEntorno,
    envoltorio: &[String],
    entorno: &[(String, String)],
) -> (Desenlace, String) {
    let argumentos: Vec<String> = envoltorio
        .iter()
        .cloned()
        .chain(con_el_fichero(orden, ejecucion))
        .collect();
    match lanzador.lanzar(&argumentos, ejecucion.copia, entorno, ejecucion.tiempo) {
        DesenlaceDeOrden::Terminada {
            exito,
            codigo,
            salida,
            errores,
            ..
        } => {
            let juntas = [salida.trim_end(), errores.trim_end()]
                .into_iter()
                .filter(|parte| !parte.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let desenlace = if exito {
                Desenlace::Paso
            } else {
                Desenlace::Fallo(codigo)
            };
            (desenlace, juntas)
        }
        DesenlaceDeOrden::Agotada => (Desenlace::Agotada, String::new()),
        DesenlaceDeOrden::NoSeLanzo(motivo)
        | DesenlaceDeOrden::NoSePudoEsperar(motivo)
        | DesenlaceDeOrden::SalidaIlegible(motivo) => (Desenlace::NoSeLanzo(motivo), String::new()),
    }
}

/// Las variables que se añaden en la copia, además de las del banco. `{copia}` es su carpeta.
/// Sin `CARGO_TARGET_DIR`, cargo compilaría donde diga el sistema o un `.cargo/config.toml` de
/// una carpeta superior, fuera de la copia.
const VARIABLES_DE_LA_COPIA: [(&str, &str); 1] = [("CARGO_TARGET_DIR", "{copia}/target")];
const MARCADOR_DE_LA_COPIA: &str = "{copia}";

/// Las variables de las previas y las de las aisladas: las mismas que usa el banco para sus
/// comprobaciones, con la caché en `cache`, y las de la copia encima. Así `cargo fetch` llena el
/// `CARGO_HOME` de `cache` y `cargo build --offline` lo encuentra.
pub fn entornos_de_la_preparacion(
    config: &ComprobacionLocal,
    cache: &Path,
    copia: &Path,
    puerto_de_la_trampa: u16,
    sistema: &BTreeMap<String, String>,
) -> Entornos {
    let Entornos {
        del_codigo,
        para_preparar,
    } = entornos(config, cache, puerto_de_la_trampa, sistema);
    // Con barras normales y sin `\\?\`, como las del banco: las entienden igual cargo y bash.
    let copia = crate::plataforma::sin_prefijo_extendido(copia)
        .display()
        .to_string()
        .replace('\\', "/");
    let con_las_de_la_copia = |mut variables: Vec<(String, String)>| {
        variables.retain(|(nombre, _)| !VARIABLES_DE_LA_COPIA.iter().any(|(n, _)| n == nombre));
        variables.extend(VARIABLES_DE_LA_COPIA.iter().map(|(nombre, valor)| {
            (
                nombre.to_string(),
                valor.replace(MARCADOR_DE_LA_COPIA, &copia),
            )
        }));
        variables
    };
    Entornos {
        del_codigo: con_las_de_la_copia(del_codigo),
        para_preparar: con_las_de_la_copia(para_preparar),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un lanzador que apunta lo que se le pide y contesta según el programa.
    #[derive(Default)]
    struct DeMentira {
        lanzadas: Vec<Vec<String>>,
        fallan: Vec<&'static str>,
    }

    impl LanzadorConEntorno for DeMentira {
        fn lanzar(
            &mut self,
            argumentos: &[String],
            _directorio: &Path,
            _entorno: &[(String, String)],
            _tope: Duration,
        ) -> DesenlaceDeOrden {
            self.lanzadas.push(argumentos.to_vec());
            let falla = argumentos.iter().any(|a| self.fallan.contains(&a.as_str()));
            DesenlaceDeOrden::Terminada {
                exito: !falla,
                codigo: Some(i32::from(falla)),
                estado: String::new(),
                salida: format!("dijo {}", argumentos.join(" ")),
                errores: if falla {
                    "error: mal".into()
                } else {
                    String::new()
                },
            }
        }
    }

    fn orden(partes: &[&str]) -> Vec<String> {
        partes.iter().map(|p| p.to_string()).collect()
    }

    fn rust() -> OrdenesDeLaExtension {
        OrdenesDeLaExtension {
            previas: vec![orden(&["cargo", "fetch"])],
            formatear: orden(&["cargo", "fmt"]),
            aisladas: vec![
                orden(&["cargo", "build", "--offline"]),
                orden(&["cargo", "test", "--offline", "{fichero}"]),
            ],
        }
    }

    fn con<'a>(envoltorio: &'a [String]) -> Ejecucion<'a> {
        Ejecucion {
            copia: Path::new("copia"),
            fichero: Path::new("copia/src/lib.rs"),
            tiempo: Duration::from_secs(1),
            entorno_de_las_previas: &[],
            entorno_de_las_aisladas: &[],
            envoltorio,
        }
    }

    /// F3: una que falla se cuenta con su salida y las demás siguen; las previas van sin aislar y
    /// sin envoltorio, el resto con él.
    #[test]
    fn una_orden_que_falla_se_cuenta_y_las_demas_siguen() {
        let envoltorio = orden(&["unshare", "--net", "--"]);
        let mut sin_aislar = DeMentira::default();
        let mut aislado = DeMentira {
            fallan: vec!["build"],
            ..DeMentira::default()
        };

        let resultados = ejecutar(&rust(), &con(&envoltorio), &mut sin_aislar, &mut aislado);

        let clases: Vec<Clase> = resultados.iter().map(|r| r.clase).collect();
        assert_eq!(
            clases,
            [
                Clase::Previa,
                Clase::Formatear,
                Clase::Aislada,
                Clase::Aislada
            ]
        );
        assert_eq!(resultados[2].desenlace, Desenlace::Fallo(Some(1)));
        assert!(
            resultados[2].salida.contains("error: mal"),
            "{:?}",
            resultados[2]
        );
        assert!(resultados[3].paso());
        // Sin errores, la salida va sola, sin una línea vacía detrás.
        assert_eq!(
            resultados[3].salida,
            "dijo unshare --net -- cargo test --offline copia/src/lib.rs"
        );
        assert_eq!(sin_aislar.lanzadas, [orden(&["cargo", "fetch"])]);
        assert_eq!(
            aislado.lanzadas[0],
            orden(&["unshare", "--net", "--", "cargo", "fmt"])
        );
        assert_eq!(
            resultados[3].orden,
            orden(&["cargo", "test", "--offline", "copia/src/lib.rs"])
        );
    }

    /// F3: si falla la de formatear, no se sigue.
    #[test]
    fn si_falla_formatear_no_se_sigue() {
        let mut sin_aislar = DeMentira::default();
        let mut aislado = DeMentira {
            fallan: vec!["fmt"],
            ..DeMentira::default()
        };

        let resultados = ejecutar(&rust(), &con(&[]), &mut sin_aislar, &mut aislado);

        assert_eq!(resultados.len(), 2);
        assert!(!resultados[1].paso());
        assert_eq!(aislado.lanzadas.len(), 1);
    }

    /// Una previa que falla tampoco corta: lo que siga lo dirá por su cuenta.
    #[test]
    fn una_previa_que_falla_no_corta() {
        let mut sin_aislar = DeMentira {
            fallan: vec!["fetch"],
            ..DeMentira::default()
        };
        let mut aislado = DeMentira::default();

        let resultados = ejecutar(&rust(), &con(&[]), &mut sin_aislar, &mut aislado);

        assert_eq!(resultados.len(), 4);
        assert!(!resultados[0].paso());
    }

    /// Lo que se vio de la red en cada aislada va con ella; las previas no se vigilan.
    #[test]
    fn cada_aislada_lleva_su_vigilancia() {
        struct QueVigila(DeMentira);
        impl LanzadorConEntorno for QueVigila {
            fn lanzar(
                &mut self,
                argumentos: &[String],
                directorio: &Path,
                entorno: &[(String, String)],
                tope: Duration,
            ) -> DesenlaceDeOrden {
                self.0.lanzar(argumentos, directorio, entorno, tope)
            }
            fn vigilancia_de_la_ultima(&mut self) -> Vigilancia {
                let ultima = self.0.lanzadas.last().cloned().unwrap_or_default();
                Vigilancia {
                    intentos_de_red: vec![ultima.join(" ")],
                    fallo: None,
                }
            }
        }
        let mut sin_aislar = QueVigila(DeMentira::default());
        let mut aislado = QueVigila(DeMentira::default());

        let resultados = ejecutar(&rust(), &con(&[]), &mut sin_aislar, &mut aislado);

        assert_eq!(resultados[0].vigilancia, Vigilancia::default());
        assert_eq!(resultados[1].vigilancia.intentos_de_red, ["cargo fmt"]);
        assert_eq!(
            resultados[3].vigilancia.intentos_de_red,
            ["cargo test --offline copia/src/lib.rs"]
        );
    }

    /// Las previas, con red y sin envoltorio, con su entorno; las aisladas, con el suyo.
    #[test]
    fn cada_clase_lleva_su_entorno() {
        struct QueApunta(Vec<Vec<(String, String)>>);
        impl LanzadorConEntorno for QueApunta {
            fn lanzar(
                &mut self,
                _argumentos: &[String],
                _directorio: &Path,
                entorno: &[(String, String)],
                _tope: Duration,
            ) -> DesenlaceDeOrden {
                self.0.push(entorno.to_vec());
                DesenlaceDeOrden::Terminada {
                    exito: true,
                    codigo: Some(0),
                    estado: String::new(),
                    salida: String::new(),
                    errores: String::new(),
                }
            }
        }
        let previas = [("RED".to_string(), "sí".to_string())];
        let aisladas = [("RED".to_string(), "no".to_string())];
        let ejecucion = Ejecucion {
            entorno_de_las_previas: &previas,
            entorno_de_las_aisladas: &aisladas,
            ..con(&[])
        };
        let mut sin_aislar = QueApunta(Vec::new());
        let mut aislado = QueApunta(Vec::new());

        ejecutar(&rust(), &ejecucion, &mut sin_aislar, &mut aislado);

        assert_eq!(sin_aislar.0, [previas.to_vec()]);
        assert_eq!(aislado.0, vec![aisladas.to_vec(); 3]);
    }

    /// El `CARGO_HOME` de las dos clases es el de la caché, para que lo que baje `cargo fetch` lo
    /// encuentre `cargo build --offline`; las aisladas van sin red y las dos compilan en la copia.
    #[test]
    fn las_previas_llenan_la_cache_que_usan_las_aisladas() {
        // Aunque la configuración la cambie, se compila en la copia.
        let config = ComprobacionLocal {
            variables: BTreeMap::from([("CARGO_TARGET_DIR".to_string(), "/otra".to_string())]),
            ..ComprobacionLocal::default()
        };
        let sistema = BTreeMap::from([("HOME".to_string(), "/home/x".to_string())]);
        let Entornos {
            del_codigo,
            para_preparar,
        } = entornos_de_la_preparacion(
            &config,
            Path::new("/p/cache"),
            Path::new("/p/proyecto"),
            4321,
            &sistema,
        );
        let valor = |entorno: &[(String, String)], nombre: &str| {
            entorno
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, v)| v.clone())
        };

        for entorno in [&del_codigo, &para_preparar] {
            assert_eq!(
                valor(entorno, "CARGO_HOME").as_deref(),
                Some("/p/cache/cargo-home")
            );
            assert_eq!(
                valor(entorno, "CARGO_TARGET_DIR").as_deref(),
                Some("/p/proyecto/target")
            );
            assert_eq!(
                entorno
                    .iter()
                    .filter(|(n, _)| n == "CARGO_TARGET_DIR")
                    .count(),
                1
            );
        }
        assert_eq!(
            valor(&del_codigo, "CARGO_NET_OFFLINE").as_deref(),
            Some("true")
        );
        assert_eq!(valor(&para_preparar, "CARGO_NET_OFFLINE"), None);
        assert!(valor(&del_codigo, "HTTP_PROXY").is_some_and(|v| v.contains("4321")));
    }

    #[test]
    fn sin_ordenes_no_se_lanza_nada() {
        let mut sin_aislar = DeMentira::default();
        let mut aislado = DeMentira::default();
        let resultados = ejecutar(
            &OrdenesDeLaExtension::default(),
            &con(&[]),
            &mut sin_aislar,
            &mut aislado,
        );
        assert!(resultados.is_empty());
        assert!(aislado.lanzadas.is_empty());
    }
}
