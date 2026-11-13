//! `[comprobacion_local]`: las pruebas ocultas en una carpeta de este equipo, el único donde corre
//! Programator.
//!
//! Todo lo que depende del equipo —la shell, cómo se llaman los programas, dónde escriben sus
//! cachés— es configuración, con valores por defecto que funcionan en Windows con Git for Windows
//! y en Linux.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{herramientas_por_defecto, orden_de_herramienta_por_defecto};
use crate::banco::ficha::{Encargo, Herramienta};
use crate::banco::rastro::Alcance;

/// El nombre con que los resultados de una tanda dicen que se comprobó en este equipo, en
/// `maquina_de_comprobacion`. Las tandas anteriores al 13/11/2026 pueden traer ahí el nombre de la
/// máquina aparte que se usaba entonces.
pub const MAQUINA_LOCAL: &str = "local";
/// El marcador de la carpeta de cachés en los valores de `variables`.
pub const MARCADOR_CACHE: &str = "{cache}";
/// El marcador del puerto del proxy trampa en los valores de `variables`. El puerto lo elige el
/// sistema al abrir la trampa, así que no se puede escribir de antemano.
pub const MARCADOR_PROXY: &str = "{proxy}";
/// El valor que pone una variable vacía, a diferencia de `""`, que no la pone. Una vacía es como
/// se anula lo que el sistema configura: cargo deja de usar el `rustc-wrapper` de su configuración
/// solo si `CARGO_BUILD_RUSTC_WRAPPER` está puesta y vacía.
pub const MARCADOR_VACIO: &str = "{vacio}";
/// Las órdenes de las fichas están escritas para bash; en Windows, el de Git for Windows, en su
/// carpeta de instalación por defecto. Git solo pone `git\cmd` en el PATH, no `bash`: en
/// una máquina Windows de pruebas, recién instalado con winget, «bash» no se encontraba (12/11/2026).
#[cfg(windows)]
const INTERPRETE_POR_DEFECTO: [&str; 2] = ["C:/Program Files/Git/bin/bash.exe", "-c"];
#[cfg(not(windows))]
const INTERPRETE_POR_DEFECTO: [&str; 2] = ["bash", "-c"];
/// La comprobación más lenta del 10/11/2026 (E06, `dotnet build`) no llegó al minuto.
const TIEMPO_MAXIMO_COMPROBACION_POR_DEFECTO: u64 = 300;
/// Dónde se guarda, dentro de la carpeta del banco, la copia de Git que usa la comprobación con
/// `integridad_baja`. El bash de Git comparte un objeto de Windows entre todos los procesos de
/// una misma instalación: si lo creó un bash normal, uno de integridad baja no puede abrirlo y
/// muere al arrancar. Una copia en otra carpeta tiene el suyo (medido en una máquina Windows de
/// pruebas, 13/11/2026).
const COPIA_DEL_INTERPRETE_POR_DEFECTO: &str = "interprete-aislado";
/// Las pruebas ocultas, encendidas salvo que se apaguen: el
/// banco existe para medir si el código funciona, y para eso hay que ejecutarlo.
const PRUEBAS_OCULTAS_ACTIVAS_POR_DEFECTO: bool = true;
/// La clave de `[comprobacion_local]` que enciende o apaga las pruebas ocultas, para los mensajes
/// que dicen cómo apagarlas.
pub const CLAVE_ACTIVA: &str = "activa";

/// Las variables de entorno que llevan cachés y temporales a la carpeta de la tanda. Cada una,
/// con la herramienta que, sin ella, escribiría en el perfil del usuario.
///
/// Lo que se compila no se desvía: `target/` de cargo y `__pycache__` de Python caen ya dentro de
/// la copia de cada entrega, que se borra con `comprobacion/`. Llevarlos a una carpeta común
/// mezcló entregas en una máquina Windows de pruebas (12/11/2026): cargo reconoce dos copias del mismo paquete como
/// iguales, y `fs::copy` en Windows conserva la fecha del fuente, así que la semilla 44 de E03
/// heredó lo compilado para la 43. `PYTHONPYCACHEPREFIX`, además, repite la ruta entera del
/// fuente bajo el prefijo y pasó de los 260 caracteres de Windows.
const VARIABLES_POR_DEFECTO: [(&str, &str); 17] = [
    // La configuración que se guarda por usuario en Windows: dotnet dejó %APPDATA%\NuGet\NuGet.Config
    // en una máquina Windows de pruebas aun con las variables de NuGet desviadas (12/11/2026). En Linux no se usa.
    ("APPDATA", "{cache}/appdata"),
    // Temporales de compiladores y herramientas: TEMP y TMP en Windows, TMPDIR en Linux.
    ("TEMP", "{cache}/tmp"),
    ("TMP", "{cache}/tmp"),
    ("TMPDIR", "{cache}/tmp"),
    // npm guarda los paquetes descargados en %LOCALAPPDATA%\npm-cache o ~/.npm.
    ("npm_config_cache", "{cache}/npm"),
    // dotnet guarda los paquetes de NuGet en ~/.nuget/packages y su estado en ~/.dotnet.
    ("NUGET_PACKAGES", "{cache}/nuget"),
    ("DOTNET_CLI_HOME", "{cache}/dotnet"),
    // Sin esto, el primer `dotnet` escribe su bienvenida y su telemetría en el perfil.
    ("DOTNET_CLI_TELEMETRY_OPTOUT", "1"),
    ("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1"),
    ("DOTNET_NOLOGO", "1"),
    // Sin esto, `dotnet build` pregunta a api.nuget.org si hay workloads nuevos, aunque vaya con
    // `--no-restore`: en una máquina Windows de pruebas (12/11/2026) contó como intento de red del código de E06.
    ("DOTNET_CLI_WORKLOAD_UPDATE_NOTIFY_DISABLE", "true"),
    // cargo descarga el registro de paquetes en ~/.cargo/registry.
    ("CARGO_HOME", "{cache}/cargo-home"),
    // pip guarda su caché en el perfil.
    ("PIP_CACHE_DIR", "{cache}/pip"),
    // npm escribe también su registro de errores en la caché; con esto queda en la de la tanda.
    ("npm_config_logs_dir", "{cache}/npm-logs"),
    // Sin esto, cualquier `npm`, también `npm --version` al mirar las herramientas, pregunta al
    // registro si hay una versión nueva: una conexión que no hace falta.
    ("npm_config_update_notifier", "false"),
    // Puestas vacías, apagan sccache. Si el sistema compila con él, el código de una entrega no
    // puede pasar por él: compartiría lo compilado entre entregas, como la carpeta común de cargo,
    // y en Linux, sin red, ni siquiera llega a su servidor. En una máquina Linux de pruebas (12/11/2026) la
    // semilla 43 de E03 salió «no compila» por eso. Quitarlas no basta: cargo encuentra el
    // `rustc-wrapper` en el `.cargo/config.toml` de una carpeta superior a la de la tanda.
    ("RUSTC_WRAPPER", MARCADOR_VACIO),
    ("CARGO_BUILD_RUSTC_WRAPPER", MARCADOR_VACIO),
];

/// Las que solo hacen falta en Linux. .NET guarda su almacén de certificados en
/// `$HOME/.dotnet/corefx`, sin otra variable que lo desvíe: en una máquina Linux de pruebas (12/11/2026) quedó
/// ese rastro. Con `HOME` en la tanda, todo lo que va al perfil se queda dentro; y rustup, que
/// busca sus toolchains en `$HOME/.rustup`, recibe la ruta de verdad. Esa cita `{HOME}` del
/// sistema, así que no se pone si el sistema ya tiene su propio `RUSTUP_HOME`. En Windows no hacen
/// falta: allí .NET y rustup usan el perfil, no `HOME`, y `APPDATA` ya va a la tanda.
#[cfg(not(windows))]
const VARIABLES_DE_LA_PLATAFORMA: [(&str, &str); 2] =
    [("HOME", "{cache}/home"), ("RUSTUP_HOME", "{HOME}/.rustup")];
#[cfg(windows)]
const VARIABLES_DE_LA_PLATAFORMA: [(&str, &str); 0] = [];

/// Las variables que cortan las descargas: el código del modelo no tiene nada que bajar, y un
/// intento de hacerlo es lo que el detector de red quiere ver.
const VARIABLES_DE_RED_POR_DEFECTO: [(&str, &str); 11] = [
    // Primero, los gestores de paquetes, que así ni lo intentan.
    ("CARGO_NET_OFFLINE", "true"),
    ("PIP_NO_INDEX", "1"),
    ("npm_config_offline", "true"),
    // Y lo que respete el proxy va a la trampa, que anota el destino y corta. curl solo lee
    // `http_proxy` en minúsculas; en Windows da igual, porque las variables no distinguen.
    ("HTTP_PROXY", "http://127.0.0.1:{proxy}"),
    ("HTTPS_PROXY", "http://127.0.0.1:{proxy}"),
    ("ALL_PROXY", "http://127.0.0.1:{proxy}"),
    ("http_proxy", "http://127.0.0.1:{proxy}"),
    ("https_proxy", "http://127.0.0.1:{proxy}"),
    ("all_proxy", "http://127.0.0.1:{proxy}"),
    // Lo de este mismo equipo no va al proxy: las pruebas de un encargo con red levantan su
    // servidor en 127.0.0.1, y con el proxy puesto su cliente acababa en la trampa
    // (máquina Linux de pruebas, 12/11/2026, que tuvo que saltárselo a mano).
    ("NO_PROXY", "127.0.0.1,localhost,::1"),
    ("no_proxy", "127.0.0.1,localhost,::1"),
];

/// Donde escriben esas herramientas cuando nadie las redirige, para mirar si quedó rastro.
#[cfg(windows)]
const VIGILAR_POR_DEFECTO: [&str; 6] = [
    "{LOCALAPPDATA}/npm-cache",
    "{USERPROFILE}/.nuget/packages",
    "{USERPROFILE}/.dotnet",
    "{APPDATA}/NuGet",
    "{USERPROFILE}/.cargo/registry",
    "{LOCALAPPDATA}/pip/cache",
];
#[cfg(not(windows))]
const VIGILAR_POR_DEFECTO: [&str; 6] = [
    "{HOME}/.npm",
    "{HOME}/.nuget/packages",
    "{HOME}/.dotnet",
    // El NuGet.Config de dotnet en Linux, el equivalente de %APPDATA%\NuGet.
    "{HOME}/.config/NuGet",
    "{HOME}/.cargo/registry",
    "{HOME}/.cache/pip",
];

/// Cómo se aísla cada orden de las fichas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aislamiento {
    /// Windows: integridad baja, que no puede escribir fuera de `comprobacion/`, y un Job que
    /// limita procesos y memoria y mata el árbol entero. La red no se corta.
    IntegridadBaja,
    /// Linux: un espacio de nombres de usuario y de red nuevos, sin red.
    EspacioDeNombres,
    /// Sin aislar: el código del modelo corre con todos los permisos del usuario.
    Ninguno,
}

impl Aislamiento {
    /// Como se escribe en `banco.toml` y se guarda en la tanda.
    pub fn nombre(self) -> &'static str {
        match self {
            Self::IntegridadBaja => "integridad_baja",
            Self::EspacioDeNombres => "espacio_de_nombres",
            Self::Ninguno => "ninguno",
        }
    }

    /// El sistema donde tiene sentido, como lo llama `std::env::consts::OS`; `None` en todos.
    fn sistema_donde_funciona(self) -> Option<&'static str> {
        match self {
            Self::IntegridadBaja => Some("windows"),
            Self::EspacioDeNombres => Some("linux"),
            Self::Ninguno => None,
        }
    }
}

/// El aislamiento de cada plataforma.
#[cfg(windows)]
const AISLAMIENTO_POR_DEFECTO: Aislamiento = Aislamiento::IntegridadBaja;
#[cfg(target_os = "linux")]
const AISLAMIENTO_POR_DEFECTO: Aislamiento = Aislamiento::EspacioDeNombres;
#[cfg(not(any(windows, target_os = "linux")))]
const AISLAMIENTO_POR_DEFECTO: Aislamiento = Aislamiento::Ninguno;
/// Procesos a la vez dentro del Job. Una comprobación lanza bash, cargo o dotnet y sus
/// compiladores en paralelo, unas decenas como mucho; 64 deja trabajar y para una bomba de
/// procesos.
const PROCESOS_MAXIMOS_POR_DEFECTO: u32 = 64;
/// Memoria de todo el Job, en MiB. `dotnet build`, lo más pesado del banco, no llega a 2 GiB; 8 GiB
/// dejan holgura y paran un programa desbocado antes de que se coma el equipo.
const MEMORIA_MAXIMA_MIB_POR_DEFECTO: u64 = 8192;
/// La orden que mete cada orden de las fichas en un espacio de nombres sin red, en Linux: el
/// `unshare` de util-linux, que traen todas las distribuciones. El espacio nace con `lo` apagada, y
/// entonces ni `127.0.0.1` funciona (máquina Linux de pruebas, 12/11/2026): las pruebas de un encargo con
/// red usan un servidor ahí. Se enciende antes de la orden, y si no se puede, el envoltorio falla
/// y se dice al empezar, en vez de fallar cada prueba por su lado.
const ENVOLTORIO_POR_DEFECTO: [&str; 9] = [
    "unshare",
    "--user",
    "--map-root-user",
    "--net",
    "--",
    "sh",
    "-c",
    r#"ip link set lo up && exec "$@""#,
    "envoltorio",
];
/// Cada cuánto se mira la tabla de conexiones: en cada latido de la espera de `orden_acotada`, que
/// es lo más a menudo que se puede. Con 100 no se vio un `curl` directo en una máquina
/// Windows de pruebas (12/11/2026); la tabla sí ve una conexión que dura. Una más breve que esto
/// puede no verse.
const VIGILAR_RED_CADA_MS_POR_DEFECTO: u64 = 50;
/// Programas que lanzan las herramientas por su cuenta y se conectan fuera sin que el código del
/// modelo lo pida. `vctip.exe` es la telemetría del enlazador de MSVC, que `rustc` usa en Windows:
/// en una máquina Windows de pruebas (12/11/2026) hizo perder la nota a una entrega de E03 que compila en cuatro de
/// nueve tandas, siempre hacia Akamai.
const PROGRAMAS_DEL_SISTEMA_POR_DEFECTO: [&str; 1] = ["vctip.exe"];

/// Cuántos niveles se miran dentro de cada carpeta vigilada. Con uno solo no se veía lo que npm
/// añade dentro de subcarpetas que ya existían; cuatro llegan a los paquetes de las cachés de
/// npm, NuGet y cargo.
const PROFUNDIDAD_RASTRO_POR_DEFECTO: usize = 4;
/// Las entradas que se miran como mucho por carpeta. Una caché de NuGet con unos cientos de
/// paquetes no llega a 20000 entradas a cuatro niveles; pasado eso, recorrerla tardaría más que
/// la comprobación y se dice que no se vigiló entera.
const ENTRADAS_MAXIMAS_RASTRO_POR_DEFECTO: usize = 20_000;

/// El temporal del sistema, que también usan otros programas: lo que aparezca ahí se dice como
/// «puede ser de otro programa».
#[cfg(windows)]
const TEMPORALES_DEL_SISTEMA_POR_DEFECTO: [&str; 1] = ["{TEMP}"];
#[cfg(not(windows))]
const TEMPORALES_DEL_SISTEMA_POR_DEFECTO: [&str; 1] = ["/tmp"];

/// El contenido de `[comprobacion_local]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComprobacionLocal {
    /// Si se ejecutan las pruebas ocultas. Con `false` no se ejecuta código del modelo en este
    /// equipo: el modelo escribe igual, y lo que se puntúa ejecutando sale «no evaluado».
    #[serde(default = "activa_por_defecto")]
    pub activa: bool,
    /// La shell con que se lanza cada orden de las fichas, seguida de la orden como último
    /// argumento. Sin escribir, la de por defecto; en Windows, si no está en su sitio, se busca
    /// junto a `git.exe` (`banco::comprobacion::interprete`).
    #[serde(default)]
    pub interprete: Option<Vec<String>>,
    /// La carpeta, relativa a la del banco, donde se copia Git para comprobar con
    /// `integridad_baja`. Vacía, no se copia y se usa el intérprete tal cual.
    #[serde(default = "copia_del_interprete_por_defecto")]
    pub copia_del_interprete: String,
    #[serde(default = "tiempo_maximo_comprobacion_por_defecto")]
    pub tiempo_maximo_comprobacion_segundos: u64,
    /// Los programas que las fichas llaman por su nombre de Linux, y cómo se llaman aquí
    /// (`python3 = "python"`).
    #[serde(default)]
    pub nombres: BTreeMap<String, String>,
    /// La orden que comprueba cada herramienta. Las de serie que no se escriban toman el valor de
    /// [`orden_de_herramienta_por_defecto`]; una que no sea de serie, como `php`, se añade aquí.
    #[serde(default)]
    pub herramientas: BTreeMap<Herramienta, String>,
    /// Las variables de entorno que cambian respecto a [`VARIABLES_POR_DEFECTO`]. Un valor vacío
    /// quita esa variable. Van a todas las órdenes, también a la preparación de las fichas.
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    /// Las que cambian respecto a [`VARIABLES_DE_RED_POR_DEFECTO`]: solo van a las órdenes que
    /// ejecutan el código del modelo, no a la preparación, que descarga con red. Mandan sobre
    /// `variables`.
    #[serde(default)]
    pub variables_de_red: BTreeMap<String, String>,
    /// Las carpetas donde mirar el rastro, con las variables del entorno entre llaves.
    #[serde(default = "vigilar_por_defecto")]
    pub vigilar: Vec<String>,
    #[serde(default = "temporales_del_sistema_por_defecto")]
    pub temporales_del_sistema: Vec<String>,
    /// Cuántos niveles se miran dentro de cada carpeta vigilada.
    #[serde(default = "profundidad_rastro_por_defecto")]
    pub profundidad_rastro: usize,
    /// Cuántas entradas como mucho por carpeta vigilada.
    #[serde(default = "entradas_maximas_rastro_por_defecto")]
    pub entradas_maximas_rastro: usize,
    #[serde(default = "aislamiento_por_defecto")]
    pub aislamiento: Aislamiento,
    /// Procesos a la vez dentro del Job (Windows).
    #[serde(default = "procesos_maximos_por_defecto")]
    pub procesos_maximos: u32,
    /// Memoria de todo el Job, en MiB (Windows).
    #[serde(default = "memoria_maxima_mib_por_defecto")]
    pub memoria_maxima_mib: u64,
    /// Lo que se antepone a cada orden con `espacio_de_nombres` (Linux).
    #[serde(default = "envoltorio_por_defecto")]
    pub envoltorio: Vec<String>,
    /// Cada cuántos milisegundos se mira si algún proceso de la orden tiene una conexión abierta
    /// (Windows, con `integridad_baja`).
    #[serde(default = "vigilar_red_cada_ms_por_defecto")]
    pub vigilar_red_cada_ms: u64,
    /// Los programas cuyas conexiones no cuentan como intento del código del modelo, porque los
    /// lanza una herramienta por su cuenta, como la telemetría del enlazador de MSVC (Windows, con
    /// `integridad_baja`). Se compara el nombre del ejecutable, sin distinguir mayúsculas.
    #[serde(default = "programas_del_sistema_por_defecto")]
    pub programas_del_sistema: Vec<String>,
}

impl ComprobacionLocal {
    /// La orden que comprueba `herramienta`: la configurada, o la de serie. Ninguna si el banco no
    /// la trae de serie y nadie la ha declarado.
    pub fn orden_de_herramienta(&self, herramienta: &Herramienta) -> Option<&str> {
        self.herramientas
            .get(herramienta)
            .map(String::as_str)
            .or_else(|| orden_de_herramienta_por_defecto(herramienta))
    }

    /// Todas las herramientas que este banco sabe comprobar, con su orden: las de serie, con la
    /// orden cambiada si `[comprobacion_local.herramientas]` la cambia, y las que esa sección
    /// añade. En orden alfabético.
    pub fn ordenes_de_herramientas(&self) -> Vec<(Herramienta, String)> {
        let mut todas: BTreeMap<Herramienta, String> =
            herramientas_por_defecto().into_iter().collect();
        todas.extend(self.herramientas.clone());
        todas.into_iter().collect()
    }

    /// Las herramientas que piden los encargos y que este banco no sabe comprobar, como
    /// (encargo, herramienta). Vacío si todas tienen orden.
    pub fn herramientas_sin_orden(&self, encargos: &[Encargo]) -> Vec<(String, String)> {
        encargos
            .iter()
            .flat_map(|encargo| {
                encargo
                    .ficha
                    .herramientas
                    .iter()
                    .filter(|h| self.orden_de_herramienta(h).is_none())
                    .map(|h| (encargo.ficha.id.clone(), h.nombre().to_string()))
            })
            .collect()
    }

    /// Las variables de entorno que se pasan a las órdenes que ejecutan el código del modelo: las
    /// de [`Self::variables_para_preparar`] con las de red encima. `{cache}` y `{proxy}` siguen
    /// sin sustituir.
    pub fn variables_efectivas(&self) -> BTreeMap<String, String> {
        sin_vacias(self.entorno_del_codigo())
    }

    /// Las de la preparación de las fichas, que descarga con red: las de por defecto con los
    /// cambios de `variables` encima, sin las de red y sin las que se dejaron vacías.
    pub fn variables_para_preparar(&self) -> BTreeMap<String, String> {
        sin_vacias(self.entorno_de_la_preparacion())
    }

    /// Las del código del modelo, con las que se dejaron vacías.
    fn entorno_del_codigo(&self) -> BTreeMap<String, String> {
        let mut variables = self.entorno_de_la_preparacion();
        variables.extend(con_cambios(
            &VARIABLES_DE_RED_POR_DEFECTO,
            &self.variables_de_red,
        ));
        variables
    }

    /// Las de `[preparar]`, sin las de red y con las que se dejaron vacías.
    fn entorno_de_la_preparacion(&self) -> BTreeMap<String, String> {
        let por_defecto: Vec<(&str, &str)> = VARIABLES_POR_DEFECTO
            .iter()
            .chain(VARIABLES_DE_LA_PLATAFORMA.iter())
            .copied()
            .collect();
        con_cambios(&por_defecto, &self.variables)
    }

    /// El intérprete escrito, o el de por defecto.
    pub fn interprete_efectivo(&self) -> Vec<String> {
        self.interprete
            .clone()
            .unwrap_or_else(interprete_por_defecto)
    }

    /// Hasta dónde se mira dentro de las carpetas vigiladas.
    pub fn alcance_del_rastro(&self) -> Alcance {
        Alcance {
            profundidad: self.profundidad_rastro,
            entradas_maximas: self.entradas_maximas_rastro,
        }
    }

    /// Lo que no tiene sentido y se rechaza al leer `banco.toml`, o `None`.
    pub fn problema(&self) -> Option<String> {
        if self.interprete.as_ref().is_some_and(Vec::is_empty) {
            return Some("«interprete» no puede estar vacío".into());
        }
        let mayores_que_cero = [
            (
                "tiempo_maximo_comprobacion_segundos",
                self.tiempo_maximo_comprobacion_segundos == 0,
            ),
            ("profundidad_rastro", self.profundidad_rastro == 0),
            ("entradas_maximas_rastro", self.entradas_maximas_rastro == 0),
            ("procesos_maximos", self.procesos_maximos == 0),
            ("memoria_maxima_mib", self.memoria_maxima_mib == 0),
            ("vigilar_red_cada_ms", self.vigilar_red_cada_ms == 0),
        ];
        mayores_que_cero
            .iter()
            .find(|(_, es_cero)| *es_cero)
            .map(|(clave, _)| format!("«{clave}» tiene que ser mayor que 0"))
            .or_else(|| self.problema_del_aislamiento(std::env::consts::OS))
    }

    /// Un aislamiento que no existe en `sistema`, o un envoltorio vacío que no aislaría nada.
    fn problema_del_aislamiento(&self, sistema: &str) -> Option<String> {
        let aislamiento = self.aislamiento;
        if let Some(donde) = aislamiento.sistema_donde_funciona() {
            if donde != sistema {
                return Some(format!(
                    "«aislamiento = \"{}\"» solo existe en {donde}, y esto es {sistema}",
                    aislamiento.nombre()
                ));
            }
        }
        if aislamiento == Aislamiento::EspacioDeNombres && self.envoltorio.is_empty() {
            return Some("«envoltorio» no puede estar vacío con «espacio_de_nombres»".into());
        }
        None
    }
}

impl Default for ComprobacionLocal {
    fn default() -> Self {
        Self {
            activa: activa_por_defecto(),
            interprete: None,
            copia_del_interprete: copia_del_interprete_por_defecto(),
            tiempo_maximo_comprobacion_segundos: tiempo_maximo_comprobacion_por_defecto(),
            nombres: BTreeMap::new(),
            herramientas: BTreeMap::new(),
            variables: BTreeMap::new(),
            variables_de_red: BTreeMap::new(),
            vigilar: vigilar_por_defecto(),
            temporales_del_sistema: temporales_del_sistema_por_defecto(),
            profundidad_rastro: profundidad_rastro_por_defecto(),
            entradas_maximas_rastro: entradas_maximas_rastro_por_defecto(),
            aislamiento: aislamiento_por_defecto(),
            procesos_maximos: procesos_maximos_por_defecto(),
            memoria_maxima_mib: memoria_maxima_mib_por_defecto(),
            envoltorio: envoltorio_por_defecto(),
            vigilar_red_cada_ms: vigilar_red_cada_ms_por_defecto(),
            programas_del_sistema: programas_del_sistema_por_defecto(),
        }
    }
}

/// Una tabla de variables por defecto con `cambios` encima. Las vacías se quedan, para que quiten
/// también lo que venga de otra tabla.
fn con_cambios(
    por_defecto: &[(&str, &str)],
    cambios: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut variables: BTreeMap<String, String> = por_defecto
        .iter()
        .map(|(nombre, valor)| (nombre.to_string(), valor.to_string()))
        .collect();
    variables.extend(cambios.clone());
    variables
}

fn sin_vacias(mut variables: BTreeMap<String, String>) -> BTreeMap<String, String> {
    variables.retain(|_, valor| !valor.is_empty());
    variables
}

fn aislamiento_por_defecto() -> Aislamiento {
    AISLAMIENTO_POR_DEFECTO
}
fn procesos_maximos_por_defecto() -> u32 {
    PROCESOS_MAXIMOS_POR_DEFECTO
}
fn memoria_maxima_mib_por_defecto() -> u64 {
    MEMORIA_MAXIMA_MIB_POR_DEFECTO
}
fn programas_del_sistema_por_defecto() -> Vec<String> {
    PROGRAMAS_DEL_SISTEMA_POR_DEFECTO
        .iter()
        .map(|s| s.to_string())
        .collect()
}
fn vigilar_red_cada_ms_por_defecto() -> u64 {
    VIGILAR_RED_CADA_MS_POR_DEFECTO
}
fn envoltorio_por_defecto() -> Vec<String> {
    ENVOLTORIO_POR_DEFECTO
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn profundidad_rastro_por_defecto() -> usize {
    PROFUNDIDAD_RASTRO_POR_DEFECTO
}
fn entradas_maximas_rastro_por_defecto() -> usize {
    ENTRADAS_MAXIMAS_RASTRO_POR_DEFECTO
}

fn interprete_por_defecto() -> Vec<String> {
    INTERPRETE_POR_DEFECTO
        .iter()
        .map(|s| s.to_string())
        .collect()
}
fn copia_del_interprete_por_defecto() -> String {
    COPIA_DEL_INTERPRETE_POR_DEFECTO.to_string()
}
fn activa_por_defecto() -> bool {
    PRUEBAS_OCULTAS_ACTIVAS_POR_DEFECTO
}
fn tiempo_maximo_comprobacion_por_defecto() -> u64 {
    TIEMPO_MAXIMO_COMPROBACION_POR_DEFECTO
}
fn vigilar_por_defecto() -> Vec<String> {
    VIGILAR_POR_DEFECTO.iter().map(|s| s.to_string()).collect()
}
fn temporales_del_sistema_por_defecto() -> Vec<String> {
    TEMPORALES_DEL_SISTEMA_POR_DEFECTO
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn leer(texto: &str) -> ComprobacionLocal {
        toml::from_str(texto).unwrap()
    }

    #[test]
    fn sin_nada_escrito_valen_los_valores_por_defecto() {
        let local = leer("");

        assert_eq!(local, ComprobacionLocal::default());
        assert_eq!(local.interprete, None);
        assert_eq!(local.copia_del_interprete, COPIA_DEL_INTERPRETE_POR_DEFECTO);
        assert_eq!(local.interprete_efectivo(), INTERPRETE_POR_DEFECTO);
        assert_eq!(local.interprete_efectivo()[1], "-c");
        assert_eq!(local.variables_efectivas()["APPDATA"], "{cache}/appdata");
        assert_eq!(local.tiempo_maximo_comprobacion_segundos, 300);
        assert_eq!(
            local.orden_de_herramienta(&herramienta("python")),
            Some("python3 --version")
        );
        let variables = local.variables_efectivas();
        assert_eq!(variables["npm_config_cache"], "{cache}/npm");
        assert_eq!(variables["TEMP"], "{cache}/tmp");
        assert!(local.problema().is_none());
    }

    #[test]
    fn ninguna_variable_por_defecto_comparte_lo_compilado_entre_entregas() {
        let variables = ComprobacionLocal::default().variables_efectivas();

        // Una carpeta de compilación común mezcla entregas: en una máquina Windows de pruebas (12/11/2026) la
        // semilla 44 de E03 heredó lo compilado para la 43 y sacó su nota.
        assert!(!variables.contains_key("CARGO_TARGET_DIR"));
        // El prefijo repite la ruta entera del fuente y pasa de 260 caracteres en Windows.
        assert!(!variables.contains_key("PYTHONPYCACHEPREFIX"));
    }

    #[test]
    fn una_variable_se_cambia_se_anade_o_se_quita() {
        let local = leer(
            "[variables]\nnpm_config_cache = \"{cache}/otra\"\nNUEVA = \"1\"\nCARGO_HOME = \"\"\n",
        );

        let variables = local.variables_efectivas();

        assert_eq!(variables["npm_config_cache"], "{cache}/otra");
        assert_eq!(variables["NUEVA"], "1");
        assert!(!variables.contains_key("CARGO_HOME"));
        assert_eq!(variables["TEMP"], "{cache}/tmp");
    }

    #[test]
    fn la_preparacion_lleva_las_variables_generales_pero_no_las_de_red() {
        let local = leer(
            "[variables]\nHTTPS_PROXY = \"http://proxy.empresa:3128\"\n\
             [variables_de_red]\nPIP_NO_INDEX = \"\"\nYARN_ENABLE_OFFLINE_MODE = \"1\"\n",
        );

        let preparar = local.variables_para_preparar();
        let efectivas = local.variables_efectivas();

        assert_eq!(preparar["npm_config_cache"], "{cache}/npm");
        // dotnet no busca workloads nuevos al compilar.
        assert_eq!(
            efectivas["DOTNET_CLI_WORKLOAD_UPDATE_NOTIFY_DISABLE"],
            "true"
        );
        // sccache se apaga, también al preparar.
        assert_eq!(preparar["RUSTC_WRAPPER"], MARCADOR_VACIO);
        assert_eq!(efectivas["CARGO_BUILD_RUSTC_WRAPPER"], MARCADOR_VACIO);
        // npm no pregunta por versiones nuevas ni al mirar las herramientas ni al preparar.
        assert_eq!(preparar["npm_config_update_notifier"], "false");
        assert!(!preparar.contains_key("npm_config_offline"));
        // Un proxy de verdad, para descargar, va a la preparación; el código del modelo sigue
        // apuntando a la trampa.
        assert_eq!(preparar["HTTPS_PROXY"], "http://proxy.empresa:3128");
        assert_eq!(efectivas["HTTPS_PROXY"], "http://127.0.0.1:{proxy}");
        assert_eq!(efectivas["npm_config_offline"], "true");
        assert_eq!(efectivas["no_proxy"], "127.0.0.1,localhost,::1");
        assert_eq!(efectivas["YARN_ENABLE_OFFLINE_MODE"], "1");
        assert!(!efectivas.contains_key("PIP_NO_INDEX"));
        assert_eq!(efectivas["TEMP"], "{cache}/tmp");
    }

    /// Solo en Linux: .NET deja su almacén de certificados en `$HOME` (máquina Linux de pruebas, 12/11/2026).
    #[cfg(not(windows))]
    #[test]
    fn en_linux_home_va_a_la_tanda_y_rustup_sigue_encontrando_lo_suyo() {
        let entorno = ComprobacionLocal::default().variables_para_preparar();

        assert_eq!(entorno["HOME"], "{cache}/home");
        assert_eq!(entorno["RUSTUP_HOME"], "{HOME}/.rustup");
    }

    #[test]
    fn los_nombres_y_las_herramientas_se_leen() {
        let local = leer(
            "[nombres]\npython3 = \"python\"\n\"lua5.4\" = \"lua\"\n\
             [herramientas]\nlua = \"lua -v\"\n",
        );

        assert_eq!(local.nombres["python3"], "python");
        assert_eq!(local.nombres["lua5.4"], "lua");
        assert_eq!(
            local.orden_de_herramienta(&herramienta("lua")),
            Some("lua -v")
        );
        assert_eq!(
            local.orden_de_herramienta(&herramienta("rust")),
            Some("cargo --version")
        );
    }

    fn herramienta(nombre: &str) -> Herramienta {
        Herramienta::nueva(nombre).unwrap()
    }

    #[test]
    fn un_lenguaje_nuevo_se_declara_con_su_orden_y_se_suma_a_los_de_serie() {
        let local = leer("[herramientas]\nphp = \"php --version\"\nlua = \"lua -v\"\n");

        assert_eq!(
            local.orden_de_herramienta(&herramienta("php")),
            Some("php --version")
        );
        let ordenes = local.ordenes_de_herramientas();
        let nombres: Vec<&str> = ordenes.iter().map(|(h, _)| h.nombre()).collect();
        assert_eq!(
            nombres,
            ["dotnet", "lua", "node", "php", "python", "rust", "sqlite"]
        );
        assert!(ordenes.contains(&(herramienta("lua"), "lua -v".to_string())));
    }

    #[test]
    fn sin_declararlo_un_lenguaje_nuevo_no_tiene_orden() {
        assert_eq!(leer("").orden_de_herramienta(&herramienta("php")), None);
        assert_eq!(leer("").ordenes_de_herramientas().len(), 6);
    }

    #[test]
    fn un_nombre_no_valido_en_las_herramientas_no_se_lee() {
        let texto = "[herramientas]\nPHP = \"php --version\"\n";
        assert!(toml::from_str::<ComprobacionLocal>(texto).is_err());
    }

    #[test]
    fn un_interprete_vacio_o_un_tiempo_cero_son_un_problema() {
        assert!(leer("interprete = []\n").problema().is_some());
        assert!(leer("tiempo_maximo_comprobacion_segundos = 0\n")
            .problema()
            .is_some());
    }

    #[test]
    fn el_alcance_del_rastro_se_lee_y_no_puede_ser_cero() {
        let local = leer("profundidad_rastro = 2\nentradas_maximas_rastro = 50\n");

        assert_eq!(
            local.alcance_del_rastro(),
            Alcance {
                profundidad: 2,
                entradas_maximas: 50
            }
        );
        assert_eq!(
            ComprobacionLocal::default()
                .alcance_del_rastro()
                .profundidad,
            4
        );
        let problema = leer("profundidad_rastro = 0\n").problema().unwrap();
        assert!(problema.contains("profundidad_rastro"));
        assert!(leer("entradas_maximas_rastro = 0\n").problema().is_some());
    }

    #[test]
    fn por_defecto_se_cortan_las_descargas_y_el_proxy_va_a_la_trampa() {
        let variables = ComprobacionLocal::default().variables_efectivas();

        assert_eq!(variables["CARGO_NET_OFFLINE"], "true");
        assert_eq!(variables["PIP_NO_INDEX"], "1");
        assert_eq!(variables["npm_config_offline"], "true");
        for proxy in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
        ] {
            assert!(variables[proxy].contains(MARCADOR_PROXY), "{proxy}");
        }
    }

    #[test]
    fn el_aislamiento_se_lee_y_por_defecto_es_el_de_la_plataforma() {
        let local = leer(
            "aislamiento = \"ninguno\"\nprocesos_maximos = 8\nmemoria_maxima_mib = 512\n\
             envoltorio = [\"bwrap\", \"--\"]\nvigilar_red_cada_ms = 250\n",
        );

        assert_eq!(local.aislamiento, Aislamiento::Ninguno);
        assert_eq!(local.procesos_maximos, 8);
        assert_eq!(local.memoria_maxima_mib, 512);
        assert_eq!(local.envoltorio, ["bwrap", "--"]);
        assert_eq!(local.vigilar_red_cada_ms, 250);
        assert_eq!(
            ComprobacionLocal::default().aislamiento,
            AISLAMIENTO_POR_DEFECTO
        );
        assert!(ComprobacionLocal::default().problema().is_none());
    }

    #[test]
    fn cada_aislamiento_solo_vale_en_su_sistema() {
        let integridad = leer("aislamiento = \"integridad_baja\"\n");
        let espacio = leer("aislamiento = \"espacio_de_nombres\"\n");
        let ninguno = leer("aislamiento = \"ninguno\"\n");

        assert!(integridad.problema_del_aislamiento("windows").is_none());
        assert!(integridad
            .problema_del_aislamiento("linux")
            .unwrap()
            .contains("solo existe en windows"));
        assert!(espacio.problema_del_aislamiento("linux").is_none());
        assert!(espacio.problema_del_aislamiento("windows").is_some());
        assert!(ninguno.problema_del_aislamiento("windows").is_none());
        assert!(ninguno.problema_del_aislamiento("macos").is_none());
    }

    #[test]
    fn un_envoltorio_vacio_o_un_tope_cero_son_un_problema() {
        let vacio = leer("aislamiento = \"espacio_de_nombres\"\nenvoltorio = []\n");

        assert!(vacio
            .problema_del_aislamiento("linux")
            .unwrap()
            .contains("envoltorio"));
        assert!(leer("procesos_maximos = 0\n").problema().is_some());
        assert!(leer("memoria_maxima_mib = 0\n").problema().is_some());
        assert!(leer("vigilar_red_cada_ms = 0\n").problema().is_some());
    }

    #[test]
    fn una_clave_que_no_existe_se_rechaza() {
        assert!(toml::from_str::<ComprobacionLocal>("maquina = \"x\"\n").is_err());
    }
}
