//! La carpeta local: las pruebas ocultas en `<carpeta de la tanda>/comprobacion/`, en este equipo,
//! por las reglas de `[comprobacion_local]`.
//!
//! Al empezar, y antes de ejecutar nada:
//!
//! 1. se crea `comprobacion/`, el lanzador la prepara (con integridad baja, la marca para que el
//!    código aislado pueda escribir dentro), y se crean su carpeta de cachés y el fichero de
//!    funciones de bash que da a cada programa el nombre que usan las fichas;
//! 2. el intérprete tiene que ser un bash que ejecute aquí: el de WSL se rechaza;
//! 3. se toma la foto de las carpetas vigiladas.
//!
//! No se pide autorización: ejecutar en este equipo es para lo que nació Programator. Lo que lo decide es `[comprobacion_local]` en `banco.toml`.
//!
//! Por cada entrega, se copia y se ejecuta cada orden de la ficha dentro de su copia, con las
//! variables que llevan las cachés a la carpeta de la tanda. Al terminar, **siempre**, también si
//! algo falló por el camino, se toma la segunda foto y se borra `comprobacion/`: lo garantiza el
//! `Drop`. Fuera de la carpeta de la tanda no se borra nada.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::lanzador::LanzadorConEntorno;
use super::red::sin_los_programas_del_sistema;
use super::trampa::Trampa;
use super::{recortar, salida_de, LugarDeEjecucion, Salida};
use crate::banco::config::{
    Aislamiento, ComprobacionLocal, MARCADOR_CACHE, MARCADOR_PROXY, MARCADOR_VACIO,
};
use crate::banco::ficha::Herramienta;
use crate::banco::mensajes;
use crate::banco::rastro::{
    carpetas_vigiladas, diferencia, expandir, tomar_foto, CarpetaVigilada, Foto, Rastro,
};
use crate::error::{Error, Resultado};
use crate::ficheros::{borrar_si_existe, copiar_arbol, crear_carpeta, escribir};
use crate::orden_acotada::DesenlaceDeOrden;
use crate::plataforma::sin_prefijo_extendido;

/// La carpeta de la comprobación dentro de la de la tanda.
pub const CARPETA_DE_COMPROBACION: &str = "comprobacion";
/// La de las cachés y los temporales, dentro de la de la comprobación. Empieza por punto para que
/// no se confunda con la copia de una entrega.
const CARPETA_DE_CACHES: &str = ".cache";
/// El fichero de funciones que bash lee antes de cada orden, dentro de la de cachés.
const FICHERO_DE_FUNCIONES: &str = "nombres.sh";
/// La variable con que bash lee un fichero antes de cada orden no interactiva.
const VARIABLE_BASH_ENV: &str = "BASH_ENV";
/// La orden que pregunta al intérprete en qué sistema cree estar.
const ORDEN_DEL_SISTEMA: &str = "echo \"$OSTYPE\"";
/// Lo que responde `$OSTYPE` en un bash de Linux, también en el de WSL.
const PREFIJO_OSTYPE_DE_LINUX: &str = "linux";
/// Cómo llama Rust a Windows en `std::env::consts::OS`.
const SISTEMA_WINDOWS: &str = "windows";
/// Lo que se ejecuta dentro del envoltorio para ver si funciona: el programa de coreutils que sale
/// con 0 sin hacer nada.
const ORDEN_QUE_NO_HACE_NADA: &str = "true";
/// Para preguntar al intérprete basta muy poco; si tarda más, algo va mal.
const TIEMPO_MAXIMO_PREGUNTA_AL_INTERPRETE: Duration = Duration::from_secs(30);

/// ¿Ejecuta este bash en el sistema donde corre Programator? En Windows, `$OSTYPE` es `msys` o
/// `cygwin` con Git for Windows y `linux-gnu` con WSL, que ejecutaría en otro sistema de ficheros
/// y con otras herramientas. Una respuesta vacía es que el intérprete no es bash.
pub fn es_un_bash_valido(ostype: &str, sistema: &str) -> bool {
    let ostype = ostype.trim();
    !ostype.is_empty()
        && !(sistema == SISTEMA_WINDOWS && ostype.starts_with(PREFIJO_OSTYPE_DE_LINUX))
}

/// El fichero de bash con una función por nombre: `python3() { python "$@"; }`.
pub fn fichero_de_funciones(nombres: &BTreeMap<String, String>) -> String {
    nombres
        .iter()
        .map(|(en_la_ficha, aqui)| format!("{en_la_ficha}() {{ {aqui} \"$@\"; }}\n"))
        .collect()
}

/// La orden de la ficha precedida de la carga del fichero de nombres. No basta con `BASH_ENV`: el
/// bash 5.3 de Git for Windows lo ignoró en una máquina Windows de pruebas (12/11/2026), probablemente por ir en
/// modo privilegiado, y python3 y lua5.4 salieron como ausentes. `BASH_ENV` se queda para los bash
/// que lance la propia orden.
fn con_los_nombres(fichero: &str, orden: &str) -> String {
    let entre_comillas = fichero.replace('\'', r"'\''");
    format!(". '{entre_comillas}'\n{orden}")
}

/// Lo que va delante de cada orden para aislarla: el envoltorio con `espacio_de_nombres`, y nada
/// con los demás. La integridad baja de Windows no va por argumentos, sino por el lanzador.
fn prefijo_de_aislamiento(config: &ComprobacionLocal) -> &[String] {
    match config.aislamiento {
        Aislamiento::EspacioDeNombres => &config.envoltorio,
        Aislamiento::IntegridadBaja | Aislamiento::Ninguno => &[],
    }
}

/// Los argumentos completos de una orden de las fichas: el `prefijo` de aislamiento, el intérprete
/// y la orden precedida de la carga de los nombres.
fn argumentos_de_la_orden(
    prefijo: &[String],
    config: &ComprobacionLocal,
    fichero: &str,
    orden: &str,
) -> Vec<String> {
    let mut argumentos = prefijo.to_vec();
    argumentos.extend(config.interprete_efectivo());
    argumentos.push(con_los_nombres(fichero, orden));
    argumentos
}

/// La ruta tal cual si ya es absoluta; si no, desde `actual`. Cada orden se ejecuta en otra carpeta,
/// así que `BASH_ENV` y las cachés con una ruta relativa apuntarían a otro sitio: el 12/11/2026,
/// con `--banco-comprobar banco\resultados\…`, bash no encontró el fichero de nombres y python y
/// lua salieron como ausentes.
fn absoluta(ruta: &Path, actual: &Path) -> PathBuf {
    if ruta.is_absolute() {
        ruta.to_path_buf()
    } else {
        actual.join(ruta)
    }
}

/// Una ruta escrita con barras normales, que entienden igual bash y los programas de Windows.
fn con_barras(ruta: &Path) -> String {
    sin_prefijo_extendido(ruta)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Las variables con `{cache}` sustituido por la carpeta de cachés y `{proxy}`, por el puerto de
/// la trampa, y `{vacio}` por nada: esa variable se pone vacía.
fn variables_sustituidas(
    variables: BTreeMap<String, String>,
    cache: &str,
    puerto_de_la_trampa: u16,
) -> Vec<(String, String)> {
    let puerto = puerto_de_la_trampa.to_string();
    variables
        .into_iter()
        .map(|(nombre, valor)| {
            let valor = valor
                .replace(MARCADOR_CACHE, cache)
                .replace(MARCADOR_PROXY, &puerto)
                .replace(MARCADOR_VACIO, "");
            (nombre, valor)
        })
        .collect()
}

/// Las variables con las `{VARIABLE}` del sistema sustituidas, después de `{cache}` y `{proxy}`.
/// Una que cita las del sistema completa lo que el sistema no trae, así que no se pone si el
/// sistema ya la tiene, ni si cita una que no existe: a medio componer no sirve de nada.
fn con_las_del_sistema(
    variables: Vec<(String, String)>,
    sistema: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    variables
        .into_iter()
        .filter_map(|(nombre, valor)| {
            if !valor.contains('{') {
                return Some((nombre, valor));
            }
            if sistema.contains_key(&nombre) {
                return None;
            }
            expandir(&valor, sistema).map(|valor| (nombre, valor))
        })
        .collect()
}

/// Una carpeta de este equipo donde se comprueban las entregas.
pub struct CarpetaLocal<'a, L: LanzadorConEntorno> {
    config: &'a ComprobacionLocal,
    lanzador: L,
    raiz: PathBuf,
    entorno: Vec<(String, String)>,
    /// El de `[preparar]`: sin las variables de red.
    entorno_para_preparar: Vec<(String, String)>,
    /// El fichero de funciones de bash, con barras normales.
    fichero_de_nombres: String,
    vigiladas: Vec<CarpetaVigilada>,
    foto_inicial: Foto,
    rastro: Vec<Rastro>,
    /// El proxy al que apuntan las variables de proxy. Vive lo que la carpeta.
    trampa: Trampa,
    /// Lo anotado desde la última vez que se preguntó.
    intentos_de_red: Vec<String>,
    red_sin_vigilar_avisada: bool,
    terminado: bool,
}

impl<'a, L: LanzadorConEntorno> CarpetaLocal<'a, L> {
    /// Hace las comprobaciones previas y prepara la carpeta.
    ///
    /// `entorno_del_sistema` son las variables de este proceso, con las que se componen las
    /// carpetas vigiladas.
    pub fn empezar(
        config: &'a ComprobacionLocal,
        carpeta_de_la_tanda: &Path,
        lanzador: L,
        entorno_del_sistema: &BTreeMap<String, String>,
    ) -> Resultado<Self> {
        let actual = std::env::current_dir().map_err(|causa| Error::Lectura {
            ruta: PathBuf::from("."),
            causa,
        })?;
        let raiz = absoluta(carpeta_de_la_tanda, &actual).join(CARPETA_DE_COMPROBACION);
        let cache = raiz.join(CARPETA_DE_CACHES);
        let texto_de_cache = con_barras(&cache);
        let trampa = Trampa::abrir()?;
        let funciones = cache.join(FICHERO_DE_FUNCIONES);
        let con_bash_env = |variables| {
            let mut entorno = con_las_del_sistema(
                variables_sustituidas(variables, &texto_de_cache, trampa.puerto()),
                entorno_del_sistema,
            );
            entorno.push((VARIABLE_BASH_ENV.into(), con_barras(&funciones)));
            entorno
        };
        let entorno = con_bash_env(config.variables_efectivas());
        let entorno_para_preparar = con_bash_env(config.variables_para_preparar());
        let vigiladas = carpetas_vigiladas(
            &config.vigilar,
            &config.temporales_del_sistema,
            entorno_del_sistema,
        );
        // Lo que quedara de un intento anterior se va antes de empezar.
        borrar_si_existe(&raiz)?;
        let mut carpeta = Self {
            config,
            lanzador,
            raiz,
            entorno,
            entorno_para_preparar,
            fichero_de_nombres: con_barras(&funciones),
            vigiladas,
            foto_inicial: Foto::default(),
            rastro: Vec::new(),
            trampa,
            intentos_de_red: Vec::new(),
            red_sin_vigilar_avisada: false,
            terminado: false,
        };
        // La carpeta se crea y se deja lista para el lanzador antes que lo de dentro, para que lo
        // herede: con integridad baja, la etiqueta.
        crear_carpeta(&carpeta.raiz)?;
        carpeta.lanzador.preparar_carpeta(&carpeta.raiz)?;
        carpeta.crear_caches(&cache, &texto_de_cache, &funciones)?;
        carpeta.comprobar_envoltorio()?;
        carpeta.comprobar_interprete()?;
        carpeta.foto_inicial = tomar_foto(&carpeta.vigiladas, config.alcance_del_rastro());
        Ok(carpeta)
    }

    /// Crea la carpeta de cachés, las que nombran las variables y el fichero de funciones.
    fn crear_caches(&self, cache: &Path, texto_de_cache: &str, funciones: &Path) -> Resultado<()> {
        crear_carpeta(cache)?;
        for valor in self.config.variables_efectivas().values() {
            if valor.contains(MARCADOR_CACHE) {
                crear_carpeta(Path::new(&valor.replace(MARCADOR_CACHE, texto_de_cache)))?;
            }
        }
        escribir(funciones, &fichero_de_funciones(&self.config.nombres))
    }

    /// Con `espacio_de_nombres`, prueba el envoltorio solo antes que el intérprete. Si el núcleo
    /// prohíbe los espacios de nombres a los usuarios normales, como Ubuntu 24.04 con AppArmor,
    /// fallaría todo después con un «el intérprete no responde» que mandaría a mirar donde no es.
    fn comprobar_envoltorio(&mut self) -> Resultado<()> {
        let prefijo = prefijo_de_aislamiento(self.config).to_vec();
        if prefijo.is_empty() {
            return Ok(());
        }
        let mut argumentos = prefijo.clone();
        argumentos.push(ORDEN_QUE_NO_HACE_NADA.into());
        let desenlace = self.lanzador.lanzar(
            &argumentos,
            &self.raiz,
            &self.entorno,
            TIEMPO_MAXIMO_PREGUNTA_AL_INTERPRETE,
        );
        match desenlace {
            DesenlaceDeOrden::Terminada { exito: true, .. } => Ok(()),
            otro => Err(Error::Banco(mensajes::mensaje_aislamiento_no_disponible(
                &prefijo.join(" "),
                &recortar(&mensajes::motivo_del_desenlace(&otro)),
            ))),
        }
    }

    fn comprobar_interprete(&mut self) -> Resultado<()> {
        let interprete = self.config.interprete_efectivo().join(" ");
        let raiz = self.raiz.clone();
        let desenlace = self.lanzar(
            ORDEN_DEL_SISTEMA,
            &raiz,
            TIEMPO_MAXIMO_PREGUNTA_AL_INTERPRETE,
        );
        let ostype = match desenlace {
            DesenlaceDeOrden::Terminada {
                exito: true,
                salida,
                ..
            } => salida,
            otro => {
                return Err(Error::Banco(mensajes::mensaje_interprete_no_responde(
                    &interprete,
                    &recortar(&mensajes::motivo_del_desenlace(&otro)),
                    std::env::consts::OS,
                )))
            }
        };
        if es_un_bash_valido(&ostype, std::env::consts::OS) {
            return Ok(());
        }
        Err(Error::Banco(mensajes::mensaje_interprete_no_valido(
            &interprete,
            ostype.trim(),
        )))
    }

    fn lanzar(&mut self, orden: &str, directorio: &Path, tope: Duration) -> DesenlaceDeOrden {
        let argumentos = argumentos_de_la_orden(
            prefijo_de_aislamiento(self.config),
            self.config,
            &self.fichero_de_nombres,
            orden,
        );
        let desenlace = self
            .lanzador
            .lanzar(&argumentos, directorio, &self.entorno, tope);
        self.anotar_la_red();
        desenlace
    }

    /// Junta lo que pidió la trampa y lo que vio el lanzador durante la última orden. Lo dice la
    /// tanda, que sabe de qué encargo y semilla es.
    fn anotar_la_red(&mut self) {
        let vigilancia = self.lanzador.vigilancia_de_la_ultima();
        if let Some(fallo) = vigilancia.fallo {
            if !self.red_sin_vigilar_avisada {
                self.red_sin_vigilar_avisada = true;
                eprintln!("{}", mensajes::mensaje_red_sin_vigilar(&fallo));
            }
        }
        let mut nuevos = self.trampa.recoger();
        nuevos.extend(vigilancia.intentos_de_red);
        self.intentos_de_red.extend(sin_los_programas_del_sistema(
            nuevos,
            &self.config.programas_del_sistema,
        ));
    }

    fn ejecutar_en(&mut self, directorio: &Path, orden: &str) -> Salida {
        let tope = Duration::from_secs(self.config.tiempo_maximo_comprobacion_segundos);
        let desenlace = self.lanzar(orden, directorio, tope);
        salida_de(desenlace, tope)
    }

    /// Toma la segunda foto y borra `comprobacion/`. Solo la primera vez.
    fn cerrar(&mut self) -> Resultado<()> {
        if self.terminado {
            return Ok(());
        }
        self.terminado = true;
        let alcance = self.config.alcance_del_rastro();
        self.rastro = diferencia(&self.foto_inicial, &tomar_foto(&self.vigiladas, alcance));
        borrar_si_existe(&self.raiz)
    }
}

impl<L: LanzadorConEntorno> LugarDeEjecucion for CarpetaLocal<'_, L> {
    fn ordenes_de_herramientas(&self) -> Vec<(Herramienta, String)> {
        self.config.ordenes_de_herramientas()
    }

    fn ejecutar_en_la_base(&mut self, orden: &str) -> Salida {
        let raiz = self.raiz.clone();
        self.ejecutar_en(&raiz, orden)
    }

    fn llevar(&mut self, carpeta_montada: &Path, nombre: &str) -> Result<(), String> {
        let destino = self.raiz.join(nombre);
        borrar_si_existe(&destino)
            .and_then(|()| copiar_arbol(carpeta_montada, &destino))
            .map_err(|fallo| mensajes::mensaje_no_se_pudo_copiar(&fallo))
    }

    fn ejecutar(&mut self, nombre: &str, orden: &str) -> Salida {
        let directorio = self.raiz.join(nombre);
        self.ejecutar_en(&directorio, orden)
    }

    /// Sin envoltorio y sin las variables de red, para que pueda descargar. Lo que vea la
    /// vigilancia es de las herramientas, no del modelo, y no se anota.
    fn preparar(&mut self, nombre: &str, orden: &str) -> Salida {
        let directorio = self.raiz.join(nombre);
        let argumentos = argumentos_de_la_orden(&[], self.config, &self.fichero_de_nombres, orden);
        let tope = Duration::from_secs(self.config.tiempo_maximo_comprobacion_segundos);
        let desenlace =
            self.lanzador
                .lanzar(&argumentos, &directorio, &self.entorno_para_preparar, tope);
        let _de_las_herramientas = self.lanzador.vigilancia_de_la_ultima();
        salida_de(desenlace, tope)
    }

    fn terminar(&mut self) -> Resultado<()> {
        self.cerrar()
    }

    fn rastro_fuera(&self) -> &[Rastro] {
        &self.rastro
    }

    fn intentos_de_red(&mut self) -> Vec<String> {
        let mut intentos = std::mem::take(&mut self.intentos_de_red);
        intentos.sort();
        intentos.dedup();
        intentos
    }
}

impl<L: LanzadorConEntorno> Drop for CarpetaLocal<'_, L> {
    fn drop(&mut self) {
        if let Err(fallo) = self.cerrar() {
            eprintln!("⚠️ {fallo}");
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::comprobacion::lanzador::Vigilancia;
    use std::cell::RefCell;
    use std::rc::Rc;

    type Llamada = (Vec<String>, PathBuf, Vec<(String, String)>);

    /// Anota cada llamada y responde con lo que diga `responder`.
    struct LanzadorFalso {
        llamadas: Rc<RefCell<Vec<Llamada>>>,
        responder: fn(&[String]) -> DesenlaceDeOrden,
        /// Cada carpeta preparada, con cuántas órdenes se habían lanzado ya y si existía.
        preparadas: Rc<RefCell<Vec<(PathBuf, usize, bool)>>>,
        /// Lo que dice haber visto de la red tras cada orden.
        vigilancia: Vigilancia,
    }

    impl LanzadorConEntorno for LanzadorFalso {
        fn vigilancia_de_la_ultima(&mut self) -> Vigilancia {
            self.vigilancia.clone()
        }

        fn preparar_carpeta(&mut self, carpeta: &Path) -> Resultado<()> {
            self.preparadas.borrow_mut().push((
                carpeta.to_path_buf(),
                self.llamadas.borrow().len(),
                carpeta.is_dir(),
            ));
            Ok(())
        }

        fn lanzar(
            &mut self,
            argumentos: &[String],
            directorio: &Path,
            entorno: &[(String, String)],
            _tope: Duration,
        ) -> DesenlaceDeOrden {
            self.llamadas.borrow_mut().push((
                argumentos.to_vec(),
                directorio.to_path_buf(),
                entorno.to_vec(),
            ));
            (self.responder)(argumentos)
        }
    }

    fn termina(salida: &str) -> DesenlaceDeOrden {
        DesenlaceDeOrden::Terminada {
            exito: true,
            codigo: Some(0),
            estado: "código 0".into(),
            salida: salida.into(),
            errores: String::new(),
        }
    }

    /// Un bash que responde como el de este sistema.
    fn bash_de_aqui(_: &[String]) -> DesenlaceDeOrden {
        termina(if cfg!(windows) {
            "msys\n"
        } else {
            "linux-gnu\n"
        })
    }

    fn falso(
        responder: fn(&[String]) -> DesenlaceDeOrden,
    ) -> (LanzadorFalso, Rc<RefCell<Vec<Llamada>>>) {
        let llamadas = Rc::new(RefCell::new(Vec::new()));
        (
            LanzadorFalso {
                llamadas: Rc::clone(&llamadas),
                responder,
                preparadas: Rc::default(),
                vigilancia: Vigilancia::default(),
            },
            llamadas,
        )
    }

    fn config_con_nombres() -> ComprobacionLocal {
        ComprobacionLocal {
            nombres: [("python3".to_string(), "python".to_string())].into(),
            vigilar: Vec::new(),
            temporales_del_sistema: Vec::new(),
            // Estas pruebas miran los nombres y las variables; el aislamiento tiene las suyas.
            aislamiento: Aislamiento::Ninguno,
            programas_del_sistema: Vec::new(),
            ..ComprobacionLocal::default()
        }
    }

    #[test]
    fn el_bash_de_wsl_no_vale_en_windows_y_el_de_linux_si_en_linux() {
        assert!(es_un_bash_valido("msys\n", "windows"));
        assert!(es_un_bash_valido("cygwin", "windows"));
        assert!(!es_un_bash_valido("linux-gnu\n", "windows"));
        assert!(es_un_bash_valido("linux-gnu", "linux"));
        assert!(!es_un_bash_valido("\n", "linux"));
    }

    #[test]
    fn cada_orden_carga_antes_el_fichero_de_nombres() {
        assert_eq!(
            con_los_nombres("C:/tanda/.cache/nombres.sh", "python3 -m unittest"),
            ". 'C:/tanda/.cache/nombres.sh'\npython3 -m unittest"
        );
        // Una comilla en la ruta no puede cortar la orden.
        assert_eq!(
            con_los_nombres("/home/o'neil/nombres.sh", "true"),
            r". '/home/o'\''neil/nombres.sh'".to_string() + "\ntrue"
        );
    }

    #[test]
    fn una_carpeta_relativa_se_hace_absoluta_desde_la_actual() {
        let dir = tempfile::tempdir().unwrap();
        let relativa = Path::new("banco").join("resultados").join("tanda");

        assert_eq!(absoluta(&relativa, dir.path()), dir.path().join(&relativa));
        assert_eq!(absoluta(dir.path(), Path::new("otra")), dir.path());
    }

    #[test]
    fn el_fichero_de_funciones_lleva_cada_nombre() {
        let nombres = [
            ("python3".to_string(), "python".to_string()),
            ("lua5.4".to_string(), "lua".to_string()),
        ]
        .into();

        let texto = fichero_de_funciones(&nombres);

        assert!(texto.contains("python3() { python \"$@\"; }\n"));
        assert!(texto.contains("lua5.4() { lua \"$@\"; }\n"));
    }

    #[test]
    fn la_carpeta_se_prepara_para_el_lanzador_recien_creada_y_antes_de_la_primera_orden() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_con_nombres();
        let (lanzador, _) = falso(bash_de_aqui);
        let preparadas = Rc::clone(&lanzador.preparadas);

        let carpeta = CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new());

        assert!(carpeta.is_ok());
        let raiz = std::env::current_dir()
            .unwrap()
            .join(dir.path())
            .join(CARPETA_DE_COMPROBACION);
        assert_eq!(*preparadas.borrow(), [(raiz, 0, true)]);
    }

    #[test]
    fn el_bash_de_wsl_se_rechaza_y_no_deja_la_carpeta() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_con_nombres();
        let (lanzador, _) = falso(|_| termina("linux-gnu\n"));

        let resultado = CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new());

        if cfg!(windows) {
            let fallo = resultado.err().unwrap();
            assert!(fallo.to_string().contains("WSL"), "{fallo}");
            assert!(!dir.path().join(CARPETA_DE_COMPROBACION).exists());
        } else {
            assert!(resultado.is_ok());
        }
    }

    fn config_en_espacio_de_nombres() -> ComprobacionLocal {
        ComprobacionLocal {
            aislamiento: Aislamiento::EspacioDeNombres,
            envoltorio: vec!["unshare".into(), "--net".into(), "--".into()],
            ..config_con_nombres()
        }
    }

    #[test]
    fn con_espacio_de_nombres_cada_orden_va_dentro_del_envoltorio() {
        let config = config_en_espacio_de_nombres();

        let argumentos = argumentos_de_la_orden(
            prefijo_de_aislamiento(&config),
            &config,
            "/c/nombres.sh",
            "cargo test",
        );

        let mut esperados = config.envoltorio.clone();
        esperados.extend(config.interprete_efectivo());
        esperados.push(con_los_nombres("/c/nombres.sh", "cargo test"));
        assert_eq!(argumentos, esperados);
        assert_eq!(
            argumentos_de_la_orden(
                prefijo_de_aislamiento(&config_con_nombres()),
                &config_con_nombres(),
                "/c/nombres.sh",
                "x"
            )[0],
            config.interprete_efectivo()[0]
        );
    }

    #[test]
    fn el_envoltorio_se_prueba_solo_antes_que_el_interprete() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_en_espacio_de_nombres();
        let (lanzador, llamadas) = falso(bash_de_aqui);

        let carpeta = CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new());

        assert!(carpeta.is_ok());
        let primera = llamadas.borrow()[0].0.clone();
        assert_eq!(primera, ["unshare", "--net", "--", ORDEN_QUE_NO_HACE_NADA]);
    }

    /// Ubuntu 24.04 prohíbe los espacios de nombres a los usuarios normales: tiene que decirse
    /// eso, y no que el intérprete no responde.
    #[test]
    fn si_el_envoltorio_falla_se_explica_y_no_queda_la_carpeta() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_en_espacio_de_nombres();
        let (lanzador, _) = falso(|_| DesenlaceDeOrden::Terminada {
            exito: false,
            codigo: Some(1),
            estado: "código 1".into(),
            salida: String::new(),
            errores: "unshare: write failed /proc/self/uid_map: Operation not permitted".into(),
        });

        let fallo = CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new())
            .err()
            .unwrap()
            .to_string();

        assert!(fallo.contains("no se pudo aislar"), "{fallo}");
        assert!(fallo.contains("Operation not permitted"), "{fallo}");
        assert!(!dir.path().join(CARPETA_DE_COMPROBACION).exists());
    }

    #[test]
    fn el_proxy_de_cada_orden_apunta_a_la_trampa_abierta() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_con_nombres();
        let (lanzador, llamadas) = falso(bash_de_aqui);

        let carpeta =
            CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new()).unwrap();

        let entorno: BTreeMap<_, _> = llamadas.borrow()[0].2.iter().cloned().collect();
        let esperado = format!("http://127.0.0.1:{}", carpeta.trampa.puerto());
        assert_eq!(entorno["HTTPS_PROXY"], esperado);
        assert_eq!(entorno["http_proxy"], esperado);
    }

    #[test]
    fn lo_que_vio_el_lanzador_se_recoge_una_vez_y_sin_repetir() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_con_nombres();
        let (mut lanzador, _) = falso(bash_de_aqui);
        lanzador.vigilancia = Vigilancia {
            intentos_de_red: vec!["151.101.0.223:443".into()],
            fallo: None,
        };
        let mut carpeta =
            CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new()).unwrap();

        carpeta.ejecutar_en_la_base("true");
        carpeta.ejecutar_en_la_base("true");

        assert_eq!(carpeta.intentos_de_red(), ["151.101.0.223:443"]);
        assert!(carpeta.intentos_de_red().is_empty());
    }

    #[test]
    fn lo_que_abre_un_programa_del_sistema_no_se_recoge() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_con_nombres();
        config.programas_del_sistema = vec!["vctip.exe".into()];
        let (mut lanzador, _) = falso(bash_de_aqui);
        lanzador.vigilancia = Vigilancia {
            intentos_de_red: vec![
                "2.20.71.150:443 (vctip.exe)".into(),
                "172.66.147.243:443 (curl.exe)".into(),
            ],
            fallo: None,
        };
        let mut carpeta =
            CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new()).unwrap();

        carpeta.ejecutar_en_la_base("true");

        assert_eq!(carpeta.intentos_de_red(), ["172.66.147.243:443 (curl.exe)"]);
    }

    #[test]
    fn las_del_sistema_completan_lo_que_el_sistema_no_trae() {
        let sistema: BTreeMap<String, String> = [
            ("HOME".to_string(), "/home/tester".to_string()),
            ("RUSTUP_HOME".to_string(), "/opt/rustup".to_string()),
        ]
        .into();
        let variables = vec![
            ("HOME".to_string(), "/tanda/.cache/home".to_string()),
            ("RUSTUP_HOME".to_string(), "{HOME}/.rustup".to_string()),
            ("OTRA_RUTA".to_string(), "{HOME}/otra".to_string()),
            ("NO_EXISTE".to_string(), "{NADA}/x".to_string()),
        ];

        let entorno: BTreeMap<_, _> = con_las_del_sistema(variables, &sistema)
            .into_iter()
            .collect();

        // Sin citas, tal cual; la cita se toma del sistema, no de lo que se pone a la orden.
        assert_eq!(entorno["HOME"], "/tanda/.cache/home");
        assert_eq!(entorno["OTRA_RUTA"], "/home/tester/otra");
        // El sistema ya la tiene: se respeta la suya.
        assert!(!entorno.contains_key("RUSTUP_HOME"));
        assert!(!entorno.contains_key("NO_EXISTE"));
    }

    #[test]
    fn la_preparacion_va_sin_envoltorio_sin_variables_de_red_y_sin_anotar_la_red() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_en_espacio_de_nombres();
        let (mut lanzador, llamadas) = falso(bash_de_aqui);
        lanzador.vigilancia = Vigilancia {
            intentos_de_red: vec!["registry.npmjs.org:443".into()],
            fallo: None,
        };
        let mut carpeta =
            CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new()).unwrap();
        carpeta.intentos_de_red();

        let salida = carpeta.preparar("E04-semilla-42", "npm install");

        assert!(matches!(salida, Salida::Pasa));
        let llamadas = llamadas.borrow();
        let (argumentos, directorio, entorno) = llamadas.last().unwrap();
        assert_eq!(argumentos[0], config.interprete_efectivo()[0]);
        assert!(argumentos.last().unwrap().ends_with("npm install"));
        assert!(directorio.ends_with("E04-semilla-42"));
        let entorno: BTreeMap<_, _> = entorno.iter().cloned().collect();
        assert!(!entorno.contains_key("HTTPS_PROXY"));
        assert!(!entorno.contains_key("npm_config_offline"));
        assert!(entorno.contains_key("npm_config_cache"));
        assert!(entorno.contains_key("BASH_ENV"));
        assert!(carpeta.intentos_de_red().is_empty());
    }

    #[test]
    fn la_entrega_se_copia_y_la_orden_recibe_las_variables_y_bash_env() {
        let dir = tempfile::tempdir().unwrap();
        let montada = dir.path().join("montada");
        std::fs::create_dir(&montada).unwrap();
        std::fs::write(montada.join("prueba.py"), "print(1)").unwrap();
        let tanda = dir.path().join("tanda");
        let config = config_con_nombres();
        let (lanzador, llamadas) = falso(bash_de_aqui);
        let mut carpeta =
            CarpetaLocal::empezar(&config, &tanda, lanzador, &BTreeMap::new()).unwrap();

        carpeta.llevar(&montada, "E01-semilla-42").unwrap();
        let salida = carpeta.ejecutar("E01-semilla-42", "python3 prueba.py");

        assert!(matches!(salida, Salida::Pasa));
        let raiz = tanda.join(CARPETA_DE_COMPROBACION);
        assert!(raiz.join("E01-semilla-42").join("prueba.py").exists());
        let funciones = raiz.join(CARPETA_DE_CACHES).join(FICHERO_DE_FUNCIONES);
        assert!(std::fs::read_to_string(&funciones)
            .unwrap()
            .contains("python3()"));
        assert!(raiz.join(CARPETA_DE_CACHES).join("tmp").is_dir());
        let llamadas = llamadas.borrow();
        let (argumentos, directorio, entorno) = llamadas.last().unwrap();
        let mut esperados = config.interprete_efectivo();
        esperados.push(con_los_nombres(
            &con_barras(&funciones),
            "python3 prueba.py",
        ));
        assert_eq!(argumentos, &esperados);
        assert_eq!(directorio, &raiz.join("E01-semilla-42"));
        let entorno: BTreeMap<_, _> = entorno.iter().cloned().collect();
        assert_eq!(entorno["BASH_ENV"], con_barras(&funciones));
        // Puesta y vacía, para que cargo no use el sccache del sistema.
        assert_eq!(entorno["RUSTC_WRAPPER"], "");
        assert_eq!(
            entorno["npm_config_cache"],
            format!("{}/npm", con_barras(&raiz.join(CARPETA_DE_CACHES)))
        );
    }

    #[test]
    fn al_terminar_la_carpeta_ya_no_existe_tambien_si_se_suelta_sin_terminar() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_con_nombres();
        let raiz = dir.path().join(CARPETA_DE_COMPROBACION);

        let (lanzador, _) = falso(bash_de_aqui);
        let mut carpeta =
            CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new()).unwrap();
        assert!(raiz.exists());
        carpeta.terminar().unwrap();
        assert!(!raiz.exists());
        drop(carpeta);

        let (lanzador, _) = falso(bash_de_aqui);
        let carpeta =
            CarpetaLocal::empezar(&config, dir.path(), lanzador, &BTreeMap::new()).unwrap();
        assert!(raiz.exists());
        drop(carpeta);
        assert!(!raiz.exists());
    }

    #[test]
    fn lo_que_aparece_en_una_carpeta_vigilada_se_cuenta_como_rastro() {
        let dir = tempfile::tempdir().unwrap();
        let perfil = dir.path().join("perfil");
        let config = ComprobacionLocal {
            vigilar: vec!["{PERFIL}/.npm".into()],
            temporales_del_sistema: Vec::new(),
            programas_del_sistema: Vec::new(),
            ..ComprobacionLocal::default()
        };
        let entorno: BTreeMap<String, String> =
            [("PERFIL".to_string(), perfil.display().to_string())].into();
        let (lanzador, _) = falso(bash_de_aqui);
        let mut carpeta =
            CarpetaLocal::empezar(&config, &dir.path().join("tanda"), lanzador, &entorno).unwrap();
        std::fs::create_dir_all(perfil.join(".npm")).unwrap();
        std::fs::write(perfil.join(".npm").join("paquete"), "x").unwrap();

        carpeta.terminar().unwrap();

        let rastro = carpeta.rastro_fuera();
        assert_eq!(rastro.len(), 1);
        assert_eq!(rastro[0].ruta, perfil.join(".npm").join("paquete"));
        // Fuera de la carpeta de la tanda no se borra nada.
        assert!(perfil.join(".npm").join("paquete").exists());
    }
}
