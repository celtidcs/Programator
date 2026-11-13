//! Las pruebas ocultas: compilar y ejecutar en este equipo el código que entregó el modelo.
//!
//! Lo común está aquí: mirar qué herramientas hay, y por cada entrega llevarla, compilarla y
//! ejecutar cada comprobación de la ficha. **Dónde y cómo** se ejecuta cada orden es un
//! [`LugarDeEjecucion`]: el de verdad es [`local::CarpetaLocal`], una carpeta de la tanda con las
//! reglas de `[comprobacion_local]`. El rasgo separa el recorrido, que se prueba con lugares
//! falsos, de los procesos de verdad.

pub mod copia_del_interprete;
pub mod interprete;
pub mod lanzador;
pub mod local;
pub mod prueba_previa;
pub mod red;
pub mod trampa;

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use super::ficha::{Encargo, FichaEncargo, Herramienta};
use super::mensajes;
use super::puntuacion::HechosDeComprobaciones;
use super::rastro::Rastro;
use crate::error::{Error, Resultado};
use crate::ficheros::{copiar_arbol, crear_carpeta};
use crate::orden_acotada::DesenlaceDeOrden;

/// Las pruebas de cada encargo, junto a la ficha.
const CARPETA_DE_PRUEBAS: &str = "pruebas";
/// Cuánta salida de una orden fallida se guarda para el informe.
const MAXIMO_SALIDA_GUARDADA: usize = 2000;

/// Lo que dicen las pruebas ocultas de una entrega.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultadoDePruebas {
    /// No se pudo comprobar: falta una herramienta, no se ejecuta código o una orden no se pudo
    /// lanzar. Nunca cuenta como 0.
    NoEvaluado(String),
    NoCompila(String),
    Compila {
        superadas: Vec<String>,
        fallidas: Vec<String>,
    },
}

/// Lo que dicen las pruebas ocultas de una entrega, y adónde intentó conectarse su código.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comprobado {
    pub resultado: ResultadoDePruebas,
    pub intentos_de_red: Vec<String>,
}

impl From<ResultadoDePruebas> for Comprobado {
    /// Sin intentos de red: lo que no ejecutó nada, o no lo vigiló.
    fn from(resultado: ResultadoDePruebas) -> Self {
        Self {
            resultado,
            intentos_de_red: Vec::new(),
        }
    }
}

impl ResultadoDePruebas {
    /// Los hechos que puntúa `puntuacion`, sumando los requisitos de texto como comprobaciones.
    pub fn hechos(&self, texto_cumplidos: usize, texto_total: usize) -> HechosDeComprobaciones {
        match self {
            Self::NoEvaluado(_) => HechosDeComprobaciones::NoEvaluado,
            Self::NoCompila(_) => HechosDeComprobaciones::NoCompila,
            Self::Compila {
                superadas,
                fallidas,
            } => HechosDeComprobaciones::Compila {
                superadas: superadas.len() + texto_cumplidos,
                total: superadas.len() + fallidas.len() + texto_total,
            },
        }
    }
}

/// Cómo salió una orden, desde el punto de vista del banco.
pub enum Salida {
    Pasa,
    Falla(String),
    /// No se pudo lanzar o esperar: no dice nada de la entrega.
    NoSePudoEjecutar(String),
}

/// Deja en `destino` las pruebas del encargo con la entrega colocada donde dice la ficha.
pub fn montar_carpeta_de_pruebas(
    encargo: &Encargo,
    entrega: &Path,
    destino: &Path,
) -> Resultado<()> {
    let Some(colocar_en) = &encargo.ficha.colocar_en else {
        return Err(Error::Banco(format!(
            "{}: la ficha no dice dónde colocar la entrega",
            encargo.ficha.id
        )));
    };
    copiar_arbol(&encargo.carpeta.join(CARPETA_DE_PRUEBAS), destino)?;
    let objetivo = destino.join(colocar_en);
    if let Some(padre) = objetivo.parent() {
        crear_carpeta(padre)?;
    }
    std::fs::copy(entrega, &objetivo).map_err(|causa| Error::Escritura {
        ruta: objetivo.clone(),
        causa,
    })?;
    Ok(())
}

fn recortar(texto: &str) -> String {
    let caracteres = texto.chars().count();
    if caracteres <= MAXIMO_SALIDA_GUARDADA {
        return texto.to_string();
    }
    texto
        .chars()
        .skip(caracteres - MAXIMO_SALIDA_GUARDADA)
        .collect()
}

/// Cómo cuenta el banco el desenlace de una orden.
fn salida_de(desenlace: DesenlaceDeOrden, tope: Duration) -> Salida {
    match desenlace {
        DesenlaceDeOrden::Terminada { exito: true, .. } => Salida::Pasa,
        DesenlaceDeOrden::Terminada {
            salida, errores, ..
        } => Salida::Falla(recortar(&format!("{salida}{errores}"))),
        DesenlaceDeOrden::Agotada => {
            Salida::Falla(format!("se cortó al pasar de {} segundos", tope.as_secs()))
        }
        DesenlaceDeOrden::NoSeLanzo(motivo)
        | DesenlaceDeOrden::NoSePudoEsperar(motivo)
        | DesenlaceDeOrden::SalidaIlegible(motivo) => Salida::NoSePudoEjecutar(motivo),
    }
}

/// Donde se ejecutan las pruebas ocultas. Todo lo demás —qué orden toca, qué cuenta como
/// superada— es común y está en [`Comprobador`].
pub trait LugarDeEjecucion {
    /// Todas las herramientas que el lugar sabe comprobar, cada una con la orden que mira si está
    /// instalada. Se miran todas al empezar.
    fn ordenes_de_herramientas(&self) -> Vec<(Herramienta, String)>;
    /// Lanza una orden en la carpeta base del lugar, sin entrega: para mirar las herramientas.
    fn ejecutar_en_la_base(&mut self, orden: &str) -> Salida;
    /// Lleva la carpeta ya montada de una entrega al lugar, con ese nombre. El error es el motivo,
    /// ya redactado.
    fn llevar(&mut self, carpeta_montada: &Path, nombre: &str) -> Result<(), String>;
    /// Lanza una orden de la ficha dentro de la carpeta de esa entrega.
    fn ejecutar(&mut self, nombre: &str, orden: &str) -> Salida;
    /// Lanza el `[preparar]` de la ficha, que no toca el código del modelo y puede necesitar red.
    /// Donde no se vigila la red, es una orden más.
    fn preparar(&mut self, nombre: &str, orden: &str) -> Salida {
        self.ejecutar(nombre, orden)
    }
    /// Deja el lugar como estaba. Si no se llama, lo hace el `Drop` del lugar.
    fn terminar(&mut self) -> Resultado<()>;
    /// Lo que, después de terminar, apareció fuera de la carpeta donde se comprobó. Un lugar
    /// que no lo vigila no da nada.
    fn rastro_fuera(&self) -> &[Rastro] {
        &[]
    }
    /// Los destinos de red que intentó alcanzar lo ejecutado desde la última vez que se preguntó,
    /// sin repetir. Un lugar que no vigila la red no da ninguno.
    fn intentos_de_red(&mut self) -> Vec<String> {
        Vec::new()
    }
}

/// Una tanda de comprobaciones en un lugar, de principio a fin.
pub struct Comprobador<G: LugarDeEjecucion> {
    lugar: G,
    /// Las que se miraron al empezar, en el orden en que se miraron.
    miradas: Vec<Herramienta>,
    disponibles: BTreeSet<Herramienta>,
}

impl<G: LugarDeEjecucion> Comprobador<G> {
    /// Con el lugar ya preparado, mira qué herramientas responden.
    pub fn nuevo(lugar: G) -> Self {
        let mut comprobador = Self {
            lugar,
            miradas: Vec::new(),
            disponibles: BTreeSet::new(),
        };
        comprobador.mirar_herramientas();
        comprobador
    }

    /// Las herramientas que respondieron al empezar.
    pub fn herramientas_disponibles(&self) -> &BTreeSet<Herramienta> {
        &self.disponibles
    }

    /// Las que no respondieron, con su nombre de las fichas.
    pub fn herramientas_ausentes(&self) -> Vec<String> {
        self.miradas
            .iter()
            .filter(|h| !self.disponibles.contains(h))
            .map(|h| h.nombre().to_string())
            .collect()
    }

    fn mirar_herramientas(&mut self) {
        for (herramienta, orden) in self.lugar.ordenes_de_herramientas() {
            if matches!(self.lugar.ejecutar_en_la_base(&orden), Salida::Pasa) {
                self.disponibles.insert(herramienta.clone());
            }
            self.miradas.push(herramienta);
        }
    }

    /// Lleva la carpeta ya montada al lugar y ejecuta la compilación y cada comprobación de la
    /// ficha.
    pub fn comprobar(&mut self, ficha: &FichaEncargo, carpeta_local: &Path) -> ResultadoDePruebas {
        self.comprobar_y_vigilar(ficha, carpeta_local).resultado
    }

    /// Lleva y comprueba una entrega, como [`Self::comprobar`], y dice además adónde intentó
    /// conectarse su código. Un paso que lo intentó falla aunque la orden saliera bien: compilar
    /// da «no compila» y una comprobación cuenta como fallida.
    pub fn comprobar_y_vigilar(
        &mut self,
        ficha: &FichaEncargo,
        carpeta_local: &Path,
    ) -> Comprobado {
        // Lo que quedara anotado es de las herramientas, al mirarlas, no de esta entrega.
        let _de_antes = self.lugar.intentos_de_red();
        let mut intentos_de_red = Vec::new();
        let resultado = self.comprobar_anotando(ficha, carpeta_local, &mut intentos_de_red);
        intentos_de_red.sort();
        intentos_de_red.dedup();
        Comprobado {
            resultado,
            intentos_de_red,
        }
    }

    fn comprobar_anotando(
        &mut self,
        ficha: &FichaEncargo,
        carpeta_local: &Path,
        intentos_de_red: &mut Vec<String>,
    ) -> ResultadoDePruebas {
        let faltan: Vec<String> = ficha
            .herramientas
            .iter()
            .filter(|h| !self.disponibles.contains(h))
            .map(|h| h.nombre().to_string())
            .collect();
        if !faltan.is_empty() {
            return ResultadoDePruebas::NoEvaluado(mensajes::motivo_herramientas_no_instaladas(
                &faltan,
            ));
        }
        let nombre = carpeta_local
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ficha.id.clone());
        if let Err(motivo) = self.lugar.llevar(carpeta_local, &nombre) {
            return ResultadoDePruebas::NoEvaluado(mensajes::motivo_entrega_sin_llevar(&motivo));
        }
        if let Some(preparar) = &ficha.preparar {
            match self.lugar.preparar(&nombre, &preparar.orden) {
                Salida::Pasa => {}
                // Lo que falla aquí no es del modelo: no se ha llegado a su código.
                Salida::Falla(salida) => {
                    return ResultadoDePruebas::NoEvaluado(mensajes::motivo_preparacion_fallida(
                        &salida,
                    ))
                }
                Salida::NoSePudoEjecutar(motivo) => {
                    return ResultadoDePruebas::NoEvaluado(mensajes::motivo_orden_sin_ejecutar(
                        &motivo,
                    ))
                }
            }
        }
        if let Some(compilar) = &ficha.compilar {
            match self.ejecutar_vigilado(&nombre, &compilar.orden, intentos_de_red) {
                Salida::Pasa => {}
                Salida::Falla(salida) => return ResultadoDePruebas::NoCompila(salida),
                Salida::NoSePudoEjecutar(motivo) => {
                    return ResultadoDePruebas::NoEvaluado(mensajes::motivo_orden_sin_ejecutar(
                        &motivo,
                    ))
                }
            }
        }
        let mut superadas = Vec::new();
        let mut fallidas = Vec::new();
        for comprobacion in &ficha.comprobaciones {
            match self.ejecutar_vigilado(&nombre, &comprobacion.orden, intentos_de_red) {
                Salida::Pasa => superadas.push(comprobacion.nombre.clone()),
                Salida::Falla(_) => fallidas.push(comprobacion.nombre.clone()),
                Salida::NoSePudoEjecutar(motivo) => {
                    return ResultadoDePruebas::NoEvaluado(mensajes::motivo_orden_sin_ejecutar(
                        &motivo,
                    ))
                }
            }
        }
        ResultadoDePruebas::Compila {
            superadas,
            fallidas,
        }
    }

    /// Lanza una orden que ejecuta el código del modelo y la da por fallida si intentó usar la
    /// red, anotando los destinos en `intentos_de_red`. Una orden que no se pudo ejecutar sigue
    /// siendo eso: no dice nada de la entrega.
    fn ejecutar_vigilado(
        &mut self,
        nombre: &str,
        orden: &str,
        intentos_de_red: &mut Vec<String>,
    ) -> Salida {
        let salida = self.lugar.ejecutar(nombre, orden);
        let nuevos = self.lugar.intentos_de_red();
        if nuevos.is_empty() {
            return salida;
        }
        let motivo = mensajes::motivo_intento_de_red(&nuevos);
        intentos_de_red.extend(nuevos);
        match salida {
            Salida::NoSePudoEjecutar(motivo) => Salida::NoSePudoEjecutar(motivo),
            Salida::Pasa => Salida::Falla(motivo),
            Salida::Falla(salida) => Salida::Falla(format!("{motivo}\n{salida}")),
        }
    }

    /// Deja el lugar como estaba y da lo que apareció fuera de él. Si no se llama, limpia el
    /// `Drop` del lugar.
    pub fn terminar(mut self) -> Resultado<Vec<Rastro>> {
        self.lugar.terminar()?;
        Ok(self.lugar.rastro_fuera().to_vec())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::config::herramientas_por_defecto;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// El tope con que el lugar falso cuenta una orden agotada.
    const TOPE: Duration = Duration::from_secs(30);

    type Respuesta = dyn Fn(&str) -> DesenlaceDeOrden;

    /// Un lugar que anota lo que se le pide y responde a cada orden lo que diga `responder`, con
    /// la misma traducción de desenlaces que el lugar de verdad ([`salida_de`]).
    struct LugarFalso {
        llamadas: Rc<RefCell<Vec<String>>>,
        responder: Box<Respuesta>,
        ordenes: Vec<(Herramienta, String)>,
    }

    impl LugarDeEjecucion for LugarFalso {
        fn ordenes_de_herramientas(&self) -> Vec<(Herramienta, String)> {
            self.ordenes.clone()
        }
        fn ejecutar_en_la_base(&mut self, orden: &str) -> Salida {
            self.ejecutar("", orden)
        }
        fn llevar(&mut self, _carpeta_montada: &Path, nombre: &str) -> Result<(), String> {
            self.llamadas.borrow_mut().push(format!("llevar {nombre}"));
            Ok(())
        }
        fn ejecutar(&mut self, _nombre: &str, orden: &str) -> Salida {
            self.llamadas.borrow_mut().push(orden.to_string());
            salida_de((self.responder)(orden), TOPE)
        }
        fn terminar(&mut self) -> Resultado<()> {
            self.llamadas.borrow_mut().push("terminar".into());
            Ok(())
        }
    }

    fn termina(codigo: i32) -> DesenlaceDeOrden {
        DesenlaceDeOrden::Terminada {
            exito: codigo == 0,
            codigo: Some(codigo),
            estado: format!("código {codigo}"),
            salida: String::new(),
            errores: if codigo == 0 {
                String::new()
            } else {
                "mal".into()
            },
        }
    }

    /// El comprobador sobre un lugar falso, ya con las herramientas miradas, y sus llamadas.
    fn comprobador(
        responder: impl Fn(&str) -> DesenlaceDeOrden + 'static,
    ) -> (Comprobador<LugarFalso>, Rc<RefCell<Vec<String>>>) {
        comprobador_con(herramientas_por_defecto(), responder)
    }

    /// Como [`comprobador`], con las herramientas que se le digan.
    fn comprobador_con(
        ordenes: Vec<(Herramienta, String)>,
        responder: impl Fn(&str) -> DesenlaceDeOrden + 'static,
    ) -> (Comprobador<LugarFalso>, Rc<RefCell<Vec<String>>>) {
        let llamadas = Rc::new(RefCell::new(Vec::new()));
        let lugar = LugarFalso {
            llamadas: Rc::clone(&llamadas),
            responder: Box::new(responder),
            ordenes,
        };
        (Comprobador::nuevo(lugar), llamadas)
    }

    /// Las herramientas de serie y además PHP, como quien lo declara en banco.toml.
    fn de_serie_y_php() -> Vec<(Herramienta, String)> {
        let mut ordenes = herramientas_por_defecto();
        ordenes.push((Herramienta::nueva("php").unwrap(), "php --version".into()));
        ordenes
    }

    fn ficha_en_php() -> FichaEncargo {
        let mut ficha = ficha_de_oficio();
        ficha.herramientas = vec![Herramienta::nueva("php").unwrap()];
        ficha
    }

    /// Falla con 1 la orden `que_falla` y pasa todas las demás.
    fn falla_solo(que_falla: &'static str) -> impl Fn(&str) -> DesenlaceDeOrden {
        move |orden| termina(if orden == que_falla { 1 } else { 0 })
    }

    fn ficha_de_oficio() -> FichaEncargo {
        toml::from_str(
            "id = \"E03\"\ntitulo = \"t\"\nfamilia = \"oficio\"\nentrega = \"a.rs\"\n\
             colocar_en = \"src/a.rs\"\nherramientas = [\"rust\"]\n\
             [compilar]\norden = \"compila\"\n\
             [[comprobaciones]]\nnombre = \"buena\"\norden = \"prueba buena\"\n\
             [[comprobaciones]]\nnombre = \"mala\"\norden = \"prueba mala\"\n",
        )
        .unwrap()
    }

    fn ficha_con_preparacion() -> FichaEncargo {
        let mut ficha = ficha_de_oficio();
        ficha.preparar = Some(super::super::ficha::Orden {
            orden: "descarga".into(),
        });
        ficha
    }

    /// Las órdenes lanzadas desde la llamada número `desde`, sin las de llevar la entrega.
    fn ordenes_desde(llamadas: &Rc<RefCell<Vec<String>>>, desde: usize) -> Vec<String> {
        llamadas.borrow()[desde..]
            .iter()
            .filter(|l| !l.starts_with("llevar "))
            .cloned()
            .collect()
    }

    #[test]
    fn una_entrega_se_lleva_se_compila_se_comprueba_y_el_lugar_se_deja_como_estaba() {
        let (mut comprobador, llamadas) = comprobador(falla_solo("prueba mala"));
        assert_eq!(comprobador.herramientas_disponibles().len(), 6);
        assert!(comprobador.herramientas_ausentes().is_empty());

        let resultado = comprobador.comprobar(&ficha_de_oficio(), Path::new("E03-semilla-42"));
        comprobador.terminar().unwrap();

        assert_eq!(
            resultado,
            ResultadoDePruebas::Compila {
                superadas: vec!["buena".into()],
                fallidas: vec!["mala".into()],
            }
        );
        let llamadas = llamadas.borrow();
        assert!(llamadas.contains(&"llevar E03-semilla-42".to_string()));
        assert_eq!(llamadas.last().unwrap(), "terminar");
    }

    #[test]
    fn sin_la_herramienta_sale_no_evaluado_sin_llevar_nada() {
        let (mut comprobador, llamadas) = comprobador(falla_solo("cargo --version"));
        let antes = llamadas.borrow().len();

        let resultado = comprobador.comprobar(&ficha_de_oficio(), Path::new("E03"));

        assert!(matches!(resultado, ResultadoDePruebas::NoEvaluado(ref m) if m.contains("rust")));
        assert_eq!(comprobador.herramientas_ausentes(), ["rust"]);
        assert_eq!(llamadas.borrow().len(), antes);
    }

    #[test]
    fn un_lenguaje_declarado_se_mira_al_empezar_y_se_comprueba_como_los_de_serie() {
        let (mut comprobador, llamadas) =
            comprobador_con(de_serie_y_php(), falla_solo("prueba mala"));
        assert!(llamadas.borrow().contains(&"php --version".to_string()));
        assert!(comprobador.herramientas_ausentes().is_empty());

        let resultado = comprobador.comprobar(&ficha_en_php(), Path::new("P01"));

        assert!(
            matches!(resultado, ResultadoDePruebas::Compila { .. }),
            "{resultado:?}"
        );
    }

    #[test]
    fn un_lenguaje_declarado_que_no_esta_instalado_sale_no_evaluado_con_su_nombre() {
        let (mut comprobador, llamadas) =
            comprobador_con(de_serie_y_php(), falla_solo("php --version"));
        let antes = llamadas.borrow().len();

        let resultado = comprobador.comprobar(&ficha_en_php(), Path::new("P01"));

        assert!(matches!(resultado, ResultadoDePruebas::NoEvaluado(ref m) if m.contains("php")));
        assert_eq!(comprobador.herramientas_ausentes(), ["php"]);
        assert_eq!(
            llamadas.borrow().len(),
            antes,
            "no se lleva ni se ejecuta nada"
        );
    }

    #[test]
    fn si_no_compila_no_se_lanzan_las_comprobaciones() {
        let (mut comprobador, llamadas) = comprobador(falla_solo("compila"));

        let resultado = comprobador.comprobar(&ficha_de_oficio(), Path::new("E03"));

        assert!(matches!(resultado, ResultadoDePruebas::NoCompila(_)));
        assert!(!llamadas.borrow().iter().any(|l| l.starts_with("prueba")));
    }

    #[test]
    fn la_preparacion_va_antes_de_compilar() {
        let (mut comprobador, llamadas) = comprobador(|_| termina(0));
        let antes = llamadas.borrow().len();

        comprobador.comprobar(&ficha_con_preparacion(), Path::new("E03"));

        assert_eq!(
            ordenes_desde(&llamadas, antes)[..2],
            ["descarga".to_string(), "compila".to_string()]
        );
    }

    #[test]
    fn si_la_preparacion_falla_es_no_evaluado_y_no_se_compila() {
        let (mut comprobador, llamadas) = comprobador(falla_solo("descarga"));
        let antes = llamadas.borrow().len();

        let resultado = comprobador.comprobar(&ficha_con_preparacion(), Path::new("E03"));

        assert!(
            matches!(resultado, ResultadoDePruebas::NoEvaluado(ref m) if m.contains("preparación")),
            "{resultado:?}"
        );
        assert_eq!(ordenes_desde(&llamadas, antes), ["descarga"]);
    }

    #[test]
    fn una_orden_que_se_pasa_de_tiempo_no_pasa() {
        let (mut comprobador, _) = comprobador(|orden| {
            if orden == "prueba buena" {
                DesenlaceDeOrden::Agotada
            } else {
                termina(0)
            }
        });

        let resultado = comprobador.comprobar(&ficha_de_oficio(), Path::new("E03"));

        assert_eq!(
            resultado,
            ResultadoDePruebas::Compila {
                superadas: vec!["mala".into()],
                fallidas: vec!["buena".into()],
            }
        );
    }

    #[test]
    fn una_orden_que_no_se_pudo_lanzar_es_no_evaluado_y_no_un_fallo() {
        let (mut comprobador, _) = comprobador(|orden| {
            if orden == "prueba mala" {
                DesenlaceDeOrden::NoSeLanzo("program not found".into())
            } else {
                termina(0)
            }
        });

        let resultado = comprobador.comprobar(&ficha_de_oficio(), Path::new("E03"));

        assert!(matches!(resultado, ResultadoDePruebas::NoEvaluado(_)));
    }

    /// Un lugar donde todo pasa y cada orden de `con_red` intenta conectarse a su destino.
    struct LugarConRed {
        con_red: Vec<(&'static str, &'static str)>,
        pendientes: Vec<String>,
    }

    impl LugarConRed {
        fn nuevo(con_red: Vec<(&'static str, &'static str)>) -> Self {
            Self {
                con_red,
                pendientes: Vec::new(),
            }
        }
    }

    impl LugarDeEjecucion for LugarConRed {
        fn ordenes_de_herramientas(&self) -> Vec<(Herramienta, String)> {
            herramientas_por_defecto()
                .into_iter()
                .map(|(h, _)| (h, "herramienta".into()))
                .collect()
        }
        fn ejecutar_en_la_base(&mut self, orden: &str) -> Salida {
            self.ejecutar("", orden)
        }
        fn llevar(&mut self, _carpeta_montada: &Path, _nombre: &str) -> Result<(), String> {
            Ok(())
        }
        fn ejecutar(&mut self, _nombre: &str, orden: &str) -> Salida {
            for (con_red, destino) in &self.con_red {
                if *con_red == orden {
                    self.pendientes.push((*destino).into());
                }
            }
            Salida::Pasa
        }
        fn terminar(&mut self) -> Resultado<()> {
            Ok(())
        }
        fn intentos_de_red(&mut self) -> Vec<String> {
            std::mem::take(&mut self.pendientes)
        }
    }

    #[test]
    fn si_compilar_intenta_usar_la_red_no_compila_aunque_saliera_bien() {
        let mut comprobador =
            Comprobador::nuevo(LugarConRed::nuevo(vec![("compila", "nuget.org:443")]));

        let comprobado = comprobador.comprobar_y_vigilar(&ficha_de_oficio(), Path::new("E03"));

        assert!(
            matches!(comprobado.resultado, ResultadoDePruebas::NoCompila(ref m)
                if m.contains("nuget.org:443") && m.contains("cuenta como fallo")),
            "{comprobado:?}"
        );
        assert_eq!(comprobado.intentos_de_red, ["nuget.org:443"]);
    }

    #[test]
    fn la_comprobacion_que_intenta_usar_la_red_cuenta_como_fallida() {
        let mut comprobador = Comprobador::nuevo(LugarConRed::nuevo(vec![
            ("prueba buena", "pypi.org:443"),
            ("prueba buena", "pypi.org:443"),
            ("prueba mala", "1.2.3.4:80"),
        ]));

        let comprobado = comprobador.comprobar_y_vigilar(&ficha_de_oficio(), Path::new("E03"));

        assert_eq!(
            comprobado.resultado,
            ResultadoDePruebas::Compila {
                superadas: vec![],
                fallidas: vec!["buena".into(), "mala".into()],
            }
        );
        assert_eq!(comprobado.intentos_de_red, ["1.2.3.4:80", "pypi.org:443"]);
    }

    #[test]
    fn lo_que_hicieron_las_herramientas_al_mirarlas_no_se_achaca_a_la_entrega() {
        let mut comprobador = Comprobador::nuevo(LugarConRed::nuevo(vec![(
            "herramienta",
            "registry.npmjs.org:443",
        )]));

        let comprobado = comprobador.comprobar_y_vigilar(&ficha_de_oficio(), Path::new("E03"));

        assert!(comprobado.intentos_de_red.is_empty());
        assert_eq!(
            comprobado.resultado,
            ResultadoDePruebas::Compila {
                superadas: vec!["buena".into(), "mala".into()],
                fallidas: vec![],
            }
        );
    }

    #[test]
    fn los_hechos_suman_los_requisitos_de_texto() {
        let resultado = ResultadoDePruebas::Compila {
            superadas: vec!["a".into(), "b".into()],
            fallidas: vec!["c".into()],
        };

        assert_eq!(
            resultado.hechos(1, 2),
            HechosDeComprobaciones::Compila {
                superadas: 3,
                total: 5
            }
        );
        assert_eq!(
            ResultadoDePruebas::NoEvaluado("x".into()).hechos(2, 2),
            HechosDeComprobaciones::NoEvaluado
        );
    }

    #[test]
    fn la_carpeta_de_pruebas_lleva_la_entrega_en_su_sitio() {
        let dir = tempfile::tempdir().unwrap();
        let carpeta = dir.path().join("E03");
        std::fs::create_dir_all(carpeta.join("pruebas/tests")).unwrap();
        std::fs::write(carpeta.join("pruebas/tests/t.rs"), "// prueba").unwrap();
        let entrega = dir.path().join("a.rs");
        std::fs::write(&entrega, "fn a() {}").unwrap();
        let encargo = Encargo {
            ficha: ficha_de_oficio(),
            carpeta,
        };
        let destino = dir.path().join("montada");

        montar_carpeta_de_pruebas(&encargo, &entrega, &destino).unwrap();

        assert_eq!(
            std::fs::read_to_string(destino.join("src/a.rs")).unwrap(),
            "fn a() {}"
        );
        assert!(destino.join("tests/t.rs").is_file());
    }
}
