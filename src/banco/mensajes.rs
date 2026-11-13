//! Lo que el banco cuenta por la terminal, compuesto en un solo sitio.

use std::path::Path;
use std::time::Duration;

use super::comparacion::{Campo, EstadoDePasada, Veredicto};
use super::config::CLAVE_ACTIVA;
use super::ficha::NOMBRE_RUBRICA;
use super::pasadas::NOMBRE_PASADAS;
use crate::error::Error;
use crate::orden_acotada::DesenlaceDeOrden;

/// Lo que tardó un encargo con Devstral, medido el 22/09/2026: entre dos y tres minutos. Solo sirve
/// para avisar de cuánto durará la tanda; no corta nada.
const MINUTOS_POR_ENCARGO: (u64, u64) = (2, 3);
const MINUTOS_POR_HORA: u64 = 60;

/// Lo que se cuenta antes de empezar.
pub struct Inicio<'a> {
    pub encargos: usize,
    pub semillas: &'a [i64],
    /// La carpeta de este equipo donde se ejecutan las pruebas ocultas; sin ella, no se ejecuta
    /// código del modelo.
    pub carpeta_de_comprobacion: Option<&'a Path>,
    pub hay_codigo_que_comprobar: bool,
    pub resultados: &'a Path,
}

/// Cómo se apagan las pruebas ocultas, con la línea exacta, para los mensajes que lo ofrecen.
fn como_apagarlas() -> String {
    format!("«{CLAVE_ACTIVA} = false» en la sección [comprobacion_local] de banco.toml")
}

/// El principio de todo motivo de «no evaluado» de una entrega: lo que el modelo escribió está, y
/// lo único que falta es saber si funciona (para que el
/// modelo escriba en un lenguaje no hace falta tenerlo instalado; solo para ejecutarlo).
fn escrito_pero_sin_ejecutar(porque: &str) -> String {
    format!(
        "el código está escrito y guardado, pero no se ha ejecutado para comprobar si funciona: \
         {porque}"
    )
}

fn texto_de_minutos(minutos: u64) -> String {
    match (minutos / MINUTOS_POR_HORA, minutos % MINUTOS_POR_HORA) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

pub fn mensaje_de_inicio(inicio: &Inicio) -> String {
    let pasadas = (inicio.encargos * inicio.semillas.len()) as u64;
    let (corto, largo) = MINUTOS_POR_ENCARGO;
    let codigo = match (
        inicio.carpeta_de_comprobacion,
        inicio.hay_codigo_que_comprobar,
    ) {
        (_, false) => "Ningún encargo necesita ejecutar código del modelo.".to_string(),
        (Some(carpeta), true) => format!(
            "⚠️ Al terminar, el código del modelo se ejecutará en este equipo, dentro de {}, con \
             las reglas de [comprobacion_local], y nadie lo ha revisado. Son las pruebas ocultas: \
             si no las quieres, apágalas con {}.",
            carpeta.display(),
            como_apagarlas()
        ),
        (None, true) => format!(
            "Las pruebas ocultas están apagadas ({}): el código del modelo se escribe y se \
             guarda, pero no se ejecuta, y lo que se puntúa ejecutándolo saldrá «no evaluado».",
            como_apagarlas()
        ),
    };
    format!(
        "Banco de modelos: {} encargos × {} semillas {:?} = {pasadas} pasadas.\n\
         Calculado entre {} y {}, a {corto}-{largo} minutos por encargo.\n\
         {codigo}\n\
         Resultados en {}",
        inicio.encargos,
        inicio.semillas.len(),
        inicio.semillas,
        texto_de_minutos(pasadas * corto),
        texto_de_minutos(pasadas * largo),
        inicio.resultados.display(),
    )
}

pub fn mensaje_sin_banco_toml(ruta: &Path) -> String {
    format!(
        "No hay {}: se usan los valores por defecto, con las pruebas ocultas encendidas. Ver \
         banco.ejemplo.toml.",
        ruta.display()
    )
}

/// Un `banco.toml` que todavía trae la sección de la máquina aparte, que ya no existe: Programator
/// solo usa el equipo donde corre.
pub fn mensaje_comprobacion_en_otra_maquina() -> String {
    format!(
        "banco.toml: la sección [comprobacion] llevaba las pruebas ocultas a otra máquina, y \
         Programator ya no usa ninguna: solo el equipo donde corre. Quítala: las pruebas ocultas \
         van en este equipo por defecto, con los ajustes de [comprobacion_local] (ver \
         banco.ejemplo.toml); para no ejecutar código del modelo, pon {}",
        como_apagarlas()
    )
}

/// Por qué no salió bien una orden, para una persona: el 12/11/2026 salía tal cual
/// `NoSeLanzo("program not found")`.
pub fn motivo_del_desenlace(desenlace: &DesenlaceDeOrden) -> String {
    match desenlace {
        DesenlaceDeOrden::Terminada {
            codigo,
            salida,
            errores,
            ..
        } => {
            let codigo = codigo.map_or("una señal".to_string(), |c| format!("el código {c}"));
            format!("terminó con {codigo}: {}{}", salida.trim(), errores.trim())
        }
        DesenlaceDeOrden::Agotada => "no terminó a tiempo".into(),
        DesenlaceDeOrden::NoSeLanzo(causa) => {
            format!("no se pudo lanzar; ¿está instalado y en esa ruta? ({causa})")
        }
        DesenlaceDeOrden::NoSePudoEsperar(causa) | DesenlaceDeOrden::SalidaIlegible(causa) => {
            causa.clone()
        }
    }
}

/// Cuando el bash no responde. `sistema` es `std::env::consts::OS`: en Windows hay que decir de
/// dónde sale el bash, porque una instalación limpia no lo trae.
pub fn mensaje_interprete_no_responde(interprete: &str, detalle: &str, sistema: &str) -> String {
    let como_conseguirlo = if sistema == "windows" {
        "Las fichas necesitan el bash de Git for Windows. Si no lo tienes, instálalo con \
         «winget install Git.Git» o desde https://git-scm.com/download/win; si ya lo tienes en \
         otra carpeta, pon la ruta de su bin\\bash.exe en «interprete»"
    } else {
        "Las fichas necesitan bash: instálalo con el gestor de paquetes del sistema, o pon su \
         ruta en «interprete»"
    };
    format!(
        "el intérprete de [comprobacion_local] («{interprete}») no respondió: {detalle}. \
         {como_conseguirlo}"
    )
}

/// Cuando el envoltorio de `espacio_de_nombres` no funciona en este equipo.
pub fn mensaje_aislamiento_no_disponible(envoltorio: &str, detalle: &str) -> String {
    format!(
        "no se pudo aislar la comprobación local con «{envoltorio}»: {detalle}. Algunas \
         distribuciones, como Ubuntu 24.04, no dejan crear espacios de nombres a los usuarios \
         normales. Puedes permitirlo en el sistema, cambiar «envoltorio» por otro que funcione \
         aquí, o poner aislamiento = \"ninguno\" sabiendo que el código del modelo correrá con red \
         y con todos los permisos de tu usuario"
    )
}

/// Por qué no empieza la tanda cuando la carpeta de comprobación no se puede preparar.
pub fn mensaje_sin_preparar(fallo: &Error) -> String {
    format!(
        "no se puede preparar la carpeta donde se ejecutan las pruebas ocultas: {fallo}. Se \
         dice ahora, antes de cargar el modelo, para no pasar la tanda entera y no poder \
         comprobarla. Si no quieres ejecutar el código del modelo, apaga las pruebas ocultas con {}",
        como_apagarlas()
    )
}

/// Cuando hay que copiar Git para comprobar con integridad baja.
pub fn mensaje_copiando_el_interprete(origen: &Path, copia: &Path) -> String {
    format!(
        "Copiando el bash de Git de {} a {} para ejecutar las pruebas ocultas aisladas. Solo se \
         hace la primera vez y cuando se actualice Git; tarda un poco.",
        origen.display(),
        copia.display()
    )
}

/// Cuando el intérprete no es el bash de una instalación de Git y no se sabe qué copiar.
pub fn mensaje_interprete_sin_copia(interprete: &str) -> String {
    format!(
        "⚠️ El intérprete «{interprete}» no es el bin\\bash.exe de una instalación de Git, así que \
         no se copia (copia_del_interprete en [comprobacion_local]). Se usa tal cual: si hay otro \
         bash de esa misma instalación abierto, el de integridad baja puede no arrancar."
    )
}

/// Al empezar una comprobación local sin aislar.
pub fn mensaje_sin_aislar() -> String {
    "⚠️ La comprobación local va sin aislar (aislamiento = \"ninguno\"): el código del modelo \
     correrá con todos los permisos de tu usuario, y el informe lo dirá."
        .into()
}

/// Cuando el bash de por defecto no estaba en su sitio y se encontró junto a `git`.
pub fn mensaje_bash_junto_a_git(bash: &Path) -> String {
    format!(
        "El bash de Git for Windows no estaba en su carpeta de siempre; se usa el que va con el \
         git del PATH: {}",
        bash.display()
    )
}

pub fn mensaje_interprete_no_valido(interprete: &str, ostype: &str) -> String {
    format!(
        "el intérprete de [comprobacion_local] («{interprete}») dice ser «{ostype}»: no es un \
         bash que ejecute en este sistema. Si es el de WSL, ejecutaría fuera de la carpeta de la \
         tanda y con otras herramientas. En Windows hace falta el bash de Git for Windows: \
         pon su ruta en «interprete»"
    )
}

pub fn mensaje_no_se_pudo_copiar(fallo: &Error) -> String {
    format!("no se pudo copiar la entrega a la carpeta de comprobación: {fallo}")
}

pub fn mensaje_comprobando_en_local(carpeta: &Path) -> String {
    format!(
        "Comprobando las entregas en este equipo, dentro de {}...",
        carpeta.display()
    )
}

pub fn mensaje_herramientas_ausentes(ausentes: &[String]) -> String {
    format!(
        "⚠️ En este equipo no responden: {}. Los encargos que las necesitan se escriben y se \
         guardan igual; lo que no se podrá es ejecutar su código para comprobar si funciona, y eso \
         saldrá «no evaluado». Para comprobarlo, instálalas; si están instaladas con otro nombre, \
         dilo en [comprobacion_local.nombres] o en [comprobacion_local.herramientas] de \
         banco.toml. Si no quieres comprobar ejecutando, apaga las pruebas ocultas con {}.",
        ausentes.join(", "),
        como_apagarlas()
    )
}

/// Por qué no se comprobó una entrega cuyas herramientas no están instaladas.
pub fn motivo_herramientas_no_instaladas(faltan: &[String]) -> String {
    let (cuales, verbo) = match faltan {
        [una] => (una.clone(), "no está instalado"),
        [primeras @ .., ultima] => (
            format!("{} y {ultima}", primeras.join(", ")),
            "no están instalados",
        ),
        [] => (String::new(), "no está instalado"),
    };
    escrito_pero_sin_ejecutar(&format!(
        "{cuales} {verbo} en este equipo. Para comprobarlo, instálalo; si no quieres comprobar \
         ejecutando, apaga las pruebas ocultas con {}",
        como_apagarlas()
    ))
}

/// Por qué no se comprobó una entrega con las pruebas ocultas apagadas.
pub fn motivo_pruebas_ocultas_apagadas() -> String {
    escrito_pero_sin_ejecutar(&format!(
        "las pruebas ocultas están apagadas ({}). Para encenderlas, quita esa línea o pon \
         «{CLAVE_ACTIVA} = true»",
        como_apagarlas()
    ))
}

/// Por qué no se comprobó una entrega cuya orden no se pudo lanzar o esperar: no dice nada del
/// código del modelo, porque no llegó a correr.
pub fn motivo_orden_sin_ejecutar(motivo: &str) -> String {
    escrito_pero_sin_ejecutar(&format!(
        "una orden de la ficha no se pudo ejecutar ({motivo})"
    ))
}

/// Por qué no se comprobó una entrega que no se pudo llevar a la carpeta de comprobación.
pub fn motivo_entrega_sin_llevar(motivo: &str) -> String {
    escrito_pero_sin_ejecutar(&format!(
        "no se pudo llevar a la carpeta donde se comprueba ({motivo})"
    ))
}

/// Por qué no vale el nombre de una herramienta de una ficha o de `banco.toml`.
pub fn motivo_nombre_de_herramienta_no_valido(nombre: &str) -> String {
    format!(
        "«{nombre}» no vale como nombre de herramienta: tiene que ir en minúsculas y llevar solo \
         letras sin tilde, dígitos, guiones y guiones bajos (por ejemplo «php» o «c-sharp»)"
    )
}

/// Las fichas que piden una herramienta que el banco no sabe comprobar: ni la trae de serie ni
/// está en `[comprobacion_local.herramientas]`. Cada par es (encargo, herramienta).
pub fn mensaje_herramientas_sin_orden(sin_orden: &[(String, String)]) -> String {
    let lineas: String = sin_orden
        .iter()
        .map(|(encargo, herramienta)| format!("\n  - {encargo} pide «{herramienta}»"))
        .collect();
    let ejemplo = sin_orden
        .first()
        .map(|(_, herramienta)| herramienta.as_str())
        .unwrap_or("php");
    format!(
        "el banco no sabe comprobar si están instaladas estas herramientas:{lineas}\n\
         Di en banco.toml qué orden lo comprueba, una línea por herramienta, por ejemplo:\n\
         \n  [comprobacion_local.herramientas]\n  {ejemplo} = \"{ejemplo} --version\"\n\n\
         La orden tiene que terminar con 0 si la herramienta está instalada. O, si no quieres \
         comprobar ejecutando, apaga las pruebas ocultas con {}. Lo explica la guía «Como \
         adaptar el banco de modelos».",
        como_apagarlas()
    )
}

/// Lo que apareció fuera de la carpeta de la tanda. No se borra: se dice.
pub fn mensaje_rastro_fuera(rastro: &[String]) -> String {
    let lineas: String = rastro.iter().map(|r| format!("\n  {r}")).collect();
    format!(
        "⚠️ La comprobación dejó esto fuera de la carpeta de la tanda; no se ha borrado nada, \
         revísalo:{lineas}"
    )
}

/// Por qué falla un paso cuyo código intentó usar la red.
pub fn motivo_intento_de_red(destinos: &[String]) -> String {
    format!(
        "el código del modelo intentó conectarse a {}; en la comprobación local no tiene red, y \
         cuenta como fallo",
        destinos.join(", ")
    )
}

/// Lo que se dice en el acto, al terminar de comprobar una semilla cuyo código intentó usar la
/// red, para quien no haya leído que ese código no tiene red.
pub fn mensaje_intento_de_red(encargo: &str, semilla: i64, destinos: &[String]) -> String {
    format!(
        "⚠️ {encargo}, semilla {semilla}: el código del modelo intentó conectarse a {}. En la \
         comprobación local no tiene red: el paso que lo intentó cuenta como fallido. Queda en el \
         informe.",
        destinos.join(", ")
    )
}

/// El `[preparar]` de una ficha falló: no se llegó al código del modelo.
pub fn motivo_preparacion_fallida(salida: &str) -> String {
    escrito_pero_sin_ejecutar(&format!(
        "falló la preparación de la ficha (lo que instala sus dependencias, antes de tocar el \
         código del modelo): {salida}"
    ))
}

/// La vigilancia de la red falló: lo que se haya conectado no se ve. Se dice una vez por tanda.
pub fn mensaje_red_sin_vigilar(motivo: &str) -> String {
    format!(
        "⚠️ No se pudo mirar si el código del modelo usa la red ({motivo}). La comprobación sigue, \
         pero un intento de conexión podría no quedar anotado."
    )
}

/// Lo que se añade al motivo de «no evaluado» de un encargo de código cuyas pruebas ocultas no
/// dieron nota y que no trae rúbrica con la que valorarlo leyendo.
pub fn motivo_sin_rubrica_de_codigo(motivo: &str) -> String {
    format!(
        "{motivo}; y el encargo no trae {NOMBRE_RUBRICA} para valorarlo leyendo el código sin \
         ejecutarlo"
    )
}

/// Lo que se anota en el detalle del informe para un encargo de código con semillas puntuadas
/// con la rúbrica: esa nota no es comparable con la de las pruebas ocultas, y se dice.
pub fn detalle_puntuada_con_rubrica(semillas: usize) -> String {
    let cuantas = match semillas {
        1 => "1 semilla puntuada".to_string(),
        n => format!("{n} semillas puntuadas"),
    };
    format!(
        "{cuantas} con la rúbrica, leyendo el código sin ejecutarlo, porque las pruebas ocultas no \
         dieron nota: no es comparable con una nota de pruebas"
    )
}

/// Lo que se anota en el detalle del informe para un encargo cuyo código intentó usar la red.
pub fn detalle_intentos_de_red(por_semilla: &[(i64, Vec<String>)]) -> String {
    let partes: Vec<String> = por_semilla
        .iter()
        .map(|(semilla, destinos)| format!("{} (semilla {semilla})", destinos.join(", ")))
        .collect();
    format!(
        "⚠️ intentó conectarse a: {}; cuenta como fallida",
        partes.join("; ")
    )
}

pub fn mensaje_calculando_huella(modelo: &Path) -> String {
    format!(
        "Calculando la huella SHA-256 de {} (tarda con un modelo grande)...",
        modelo.display()
    )
}

pub fn mensaje_motor_ajeno() -> String {
    "Ya hay un motor escuchando en el puerto configurado: se usa ese y no se mide su carga.".into()
}

pub fn mensaje_motor_cargado(capas: u32, vram_mib: Option<u64>, carga: Duration) -> String {
    let vram = vram_mib
        .map(|v| format!("{v} MiB de VRAM"))
        .unwrap_or_else(|| "VRAM sin medir".into());
    format!(
        "Motor cargado en {} s: {capas} capas en la GPU, {vram}.",
        carga.as_secs()
    )
}

/// Una línea por encargo pasado.
pub fn mensaje_de_avance(
    numero: usize,
    total: usize,
    id: &str,
    semilla: i64,
    a_tiempo: bool,
    duracion: Duration,
    entregas: usize,
) -> String {
    let como = if a_tiempo {
        format!("{} s, {entregas} ficheros entregados", duracion.as_secs())
    } else {
        format!("sin respuesta a tiempo ({} s)", duracion.as_secs())
    };
    format!("[{numero}/{total}] {id}, semilla {semilla}: {como}")
}

fn texto_de_pendientes(pendientes: usize, resultados: &Path) -> String {
    if pendientes == 0 {
        return "No queda ninguna rúbrica por puntuar.".into();
    }
    format!(
        "Quedan {pendientes} rúbricas por puntuar en {}. Cuando estén, rehaz el informe con \
         «--banco-informe {}».",
        resultados
            .parent()
            .unwrap_or(resultados)
            .join("rubricas")
            .display(),
        resultados.display(),
    )
}

/// Al terminar la tanda. `resultados` es la ruta del TOML de resultados.
pub fn mensaje_de_fin(resultados: &Path, informe: &Path, pendientes: usize) -> String {
    format!(
        "Tanda terminada.\nResultados: {}\nInforme: {}\n{}",
        resultados.display(),
        informe.display(),
        texto_de_pendientes(pendientes, resultados),
    )
}

pub fn mensaje_informe_rehecho(resultados: &Path, pendientes: usize) -> String {
    format!(
        "Informe rehecho a partir de {}.\n{}",
        resultados.display(),
        texto_de_pendientes(pendientes, resultados),
    )
}

/// Cuando la fase 2 de una tanda no termina. Lo pasado con el modelo está ya en disco, y se dice
/// con qué orden se termina sin volver a pasarlo (la tanda del 11/11/2026 perdió cinco horas de
/// GPU por no poder hacerlo).
pub fn mensaje_tanda_sin_cerrar(fallo: &Error, tanda: &Path) -> String {
    format!(
        "no se pudo terminar la tanda: {fallo}. Lo pasado con el modelo está a salvo en «{}». \
         Cuando lo hayas arreglado, termínala sin volver a pasar los encargos con \
         «programator --banco-comprobar {}»",
        tanda.join(NOMBRE_PASADAS).display(),
        tanda.display(),
    )
}

pub fn mensaje_comprobando_guardada(pasadas: usize, tanda: &Path) -> String {
    format!(
        "Se comprueban las {pasadas} pasadas guardadas en {}, sin volver a cargar el modelo.",
        tanda.display()
    )
}

/// El veredicto de `--banco-comparar`: primero si las tandas son comparables, después cada pasada
/// que no coincide, y al final la cuenta.
pub fn mensaje_comparacion(veredicto: &Veredicto, a: &Path, b: &Path) -> String {
    let mut lineas = vec![format!(
        "Comparando las tandas\n  A: {}\n  B: {}",
        a.display(),
        b.display()
    )];
    if !veredicto.diferencias.is_empty() {
        lineas
            .push("⚠️ No son comparables: que salgan distintas no diría nada. Difieren en:".into());
        for diferencia in &veredicto.diferencias {
            lineas.push(format!(
                "  - {}: A {} · B {}",
                nombre_del_campo(diferencia.campo),
                diferencia.en_a,
                diferencia.en_b
            ));
        }
    }
    if veredicto.faltan_ajustes_del_motor {
        lineas.push(
            "⚠️ Alguna de las dos no guardó los ajustes del motor (se pasó antes de que el banco \
             los guardara): no se puede saber si son comparables."
                .into(),
        );
    }
    for pasada in &veredicto.pasadas {
        if let Some(estado) = texto_del_estado(&pasada.estado) {
            let corte = if pasada.cortada_por_tiempo {
                " (la cortó el tiempo máximo, que depende del reloj)"
            } else {
                ""
            };
            lineas.push(format!(
                "  - {} con la semilla {}: {estado}{corte}",
                pasada.encargo, pasada.semilla
            ));
        }
    }
    let cuenta = format!(
        "{} de {} pasadas idénticas.",
        veredicto.identicas(),
        veredicto.pasadas.len()
    );
    lineas.push(if veredicto.es_repeticion() {
        format!("✅ Las dos tandas son repetición una de otra: {cuenta}")
    } else {
        format!("🔴 Las dos tandas no son repetición una de otra: {cuenta}")
    });
    lineas.join("\n")
}

/// Cómo se nombra cada dato de la tanda que también compara el registro de modelos. Un solo
/// sitio, para que el informe de la tanda y el del registro digan lo mismo.
pub const NOMBRE_COMPILACION_DEL_MOTOR: &str = "la compilación de llama-server";
/// Ver [`NOMBRE_COMPILACION_DEL_MOTOR`].
pub const NOMBRE_VERSION_DE_PROGRAMATOR: &str = "la versión de Programator";
/// Ver [`NOMBRE_COMPILACION_DEL_MOTOR`].
pub const NOMBRE_CAPAS_EN_GPU: &str = "las capas en la GPU";

fn nombre_del_campo(campo: Campo) -> &'static str {
    match campo {
        Campo::Modelo => "el modelo",
        Campo::HuellaDelModelo => "la huella del fichero del modelo",
        Campo::CompilacionDelMotor => NOMBRE_COMPILACION_DEL_MOTOR,
        Campo::VersionDeProgramator => NOMBRE_VERSION_DE_PROGRAMATOR,
        Campo::Semillas => "las semillas",
        Campo::Temperatura => "la temperatura",
        Campo::TiempoMaximo => "el tiempo máximo por encargo",
        Campo::CapasEnGpu => NOMBRE_CAPAS_EN_GPU,
        Campo::AjustesDelMotor => "los ajustes del motor",
        Campo::Tarjeta => "la tarjeta gráfica",
    }
}

/// Lo que se dice de una pasada, o `None` si es idéntica y no hace falta nombrarla.
fn texto_del_estado(estado: &EstadoDePasada) -> Option<String> {
    match estado {
        EstadoDePasada::Identica => None,
        EstadoDePasada::Diverge {
            vuelta: Some(vuelta),
            ..
        } => Some(format!("se separan en la vuelta {vuelta}")),
        EstadoDePasada::Diverge {
            linea,
            vuelta: None,
        } => Some(format!("se separan en la línea {linea} del registro")),
        EstadoDePasada::SoloEnA => Some("solo está en A".into()),
        EstadoDePasada::SoloEnB => Some("solo está en B".into()),
        EstadoDePasada::SinConversacion => Some("alguna de las dos no dejó conversación".into()),
    }
}

/// Cuando una orden del banco no termina. No es un fallo del ciclo: el agente del canal no se
/// ve afectado, y decirlo así llevaría a buscar el problema donde no está.
pub fn mensaje_fallo(fallo: &Error) -> String {
    // El texto de `Error::Banco` ya empieza por «banco de modelos:»; aquí sobraría.
    let texto = match fallo {
        Error::Banco(texto) => texto.clone(),
        otro => otro.to_string(),
    };
    format!("🔴 El banco de modelos se detuvo: {texto}.")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::comparacion::{ComparacionDePasada, Diferencia};

    fn inicio(carpeta: Option<&Path>, codigo: bool) -> Inicio<'_> {
        Inicio {
            encargos: 15,
            semillas: &[42, 43, 44],
            carpeta_de_comprobacion: carpeta,
            hay_codigo_que_comprobar: codigo,
            resultados: Path::new("resultados/tanda"),
        }
    }

    #[test]
    fn el_inicio_dice_cuanto_tardara() {
        let texto = mensaje_de_inicio(&inicio(None, true));

        assert!(texto.contains("15 encargos × 3 semillas"));
        assert!(texto.contains("45 pasadas"));
        assert!(texto.contains("entre 1 h 30 min y 2 h 15 min"));
    }

    #[test]
    fn con_las_pruebas_ocultas_apagadas_el_inicio_avisa_de_que_no_se_ejecuta_codigo() {
        let texto = mensaje_de_inicio(&inicio(None, true));

        assert!(texto.contains("no se ejecuta"), "{texto}");
        assert!(texto.contains("[comprobacion_local]"), "{texto}");
        assert!(texto.contains("activa = false"), "{texto}");
        assert!(texto.contains("no evaluado"));
    }

    #[test]
    fn en_local_el_inicio_avisa_de_que_el_codigo_se_ejecuta_en_este_equipo() {
        let carpeta = Path::new("resultados/tanda/comprobacion");
        let texto = mensaje_de_inicio(&inicio(Some(carpeta), true));

        assert!(texto.contains("en este equipo"), "{texto}");
        assert!(texto.contains(&carpeta.display().to_string()));
        assert!(texto.contains("nadie lo ha revisado"));
        assert!(
            texto.contains("activa = false"),
            "dice cómo apagarlo: {texto}"
        );
        assert!(!texto.contains("--maquina-autorizada"), "{texto}");
    }

    #[test]
    fn el_motivo_de_una_orden_se_redacta_sin_formato_de_depuracion() {
        let no_se_lanzo =
            motivo_del_desenlace(&DesenlaceDeOrden::NoSeLanzo("program not found".into()));
        assert!(no_se_lanzo.contains("¿está instalado"), "{no_se_lanzo}");
        assert!(!no_se_lanzo.contains("NoSeLanzo"));

        let fallo = motivo_del_desenlace(&DesenlaceDeOrden::Terminada {
            exito: false,
            codigo: Some(127),
            estado: String::new(),
            salida: String::new(),
            errores: "orden no encontrada\n".into(),
        });
        assert_eq!(fallo, "terminó con el código 127: orden no encontrada");
    }

    #[test]
    fn el_rastro_se_lista_ruta_a_ruta_y_se_dice_que_no_se_borro() {
        let texto = mensaje_rastro_fuera(&["C:/a".to_string(), "C:/b".to_string()]);

        assert!(texto.contains("no se ha borrado nada"));
        assert!(texto.ends_with(":\n  C:/a\n  C:/b"), "{texto}");
    }

    /// Una instalación limpia de Windows no trae bash: el mensaje tiene que decir de dónde sale.
    #[test]
    fn sin_bash_se_dice_como_conseguirlo() {
        let en_windows = mensaje_interprete_no_responde("bash -c", "no está", "windows");
        let en_linux = mensaje_interprete_no_responde("bash -c", "no está", "linux");

        assert!(en_windows.contains("winget install Git.Git"));
        assert!(en_windows.contains("«interprete»"));
        assert!(!en_linux.contains("winget"));
        assert!(en_linux.contains("gestor de paquetes"));
    }

    /// El 12/11/2026 una edición perdió las continuaciones de línea de varios mensajes, que
    /// salían con rachas de espacios en mitad de la frase.
    #[test]
    fn ningun_mensaje_largo_lleva_espacios_de_mas() {
        let carpeta = Path::new("tanda");
        let fallo = Error::Banco("x".into());
        let ausentes = ["lua".to_string()];
        let mensajes = [
            mensaje_comprobacion_en_otra_maquina(),
            mensaje_interprete_no_responde("bash -c", "x", "windows"),
            mensaje_interprete_no_responde("bash -c", "x", "linux"),
            mensaje_bash_junto_a_git(carpeta),
            mensaje_sin_aislar(),
            mensaje_aislamiento_no_disponible("unshare --net --", "x"),
            mensaje_interprete_no_valido("bash -c", "linux-gnu"),
            mensaje_no_se_pudo_copiar(&fallo),
            mensaje_comprobando_en_local(carpeta),
            mensaje_herramientas_ausentes(&ausentes),
            mensaje_de_inicio(&inicio(Some(carpeta), true)),
            mensaje_de_inicio(&inicio(None, true)),
            mensaje_sin_banco_toml(carpeta),
        ];

        for mensaje in mensajes {
            assert!(!mensaje.contains("  "), "{mensaje}");
        }
    }

    #[test]
    fn el_fin_dice_donde_estan_los_resultados_y_cuantas_rubricas_quedan() {
        let resultados = Path::new("tanda").join("resultados.toml");
        let texto = mensaje_de_fin(&resultados, &Path::new("tanda").join("informe.md"), 4);

        assert!(texto.contains("resultados.toml"));
        assert!(texto.contains("Quedan 4 rúbricas"));
        assert!(texto.contains("--banco-informe"));
        assert!(mensaje_de_fin(&resultados, &resultados, 0).contains("ninguna rúbrica"));
    }

    #[test]
    fn el_fallo_del_banco_no_habla_del_canal() {
        let texto = mensaje_fallo(&Error::Banco("algo".into()));

        assert!(texto.contains("El banco de modelos se detuvo"));
        assert!(!texto.contains("canal"));
    }

    /// `Error::Banco` ya lleva «banco de modelos:» en su texto: salía «El banco de modelos se
    /// detuvo: banco de modelos: …» (máquina Windows de pruebas, 12/11/2026).
    #[test]
    fn el_fallo_del_banco_no_repite_el_prefijo() {
        let texto = mensaje_fallo(&Error::Banco("falta algo".into()));

        assert_eq!(texto, "🔴 El banco de modelos se detuvo: falta algo.");
    }

    #[test]
    fn otro_fallo_conserva_su_texto() {
        let fallo = Error::Registro("roto".into());

        assert!(mensaje_fallo(&fallo).contains(&fallo.to_string()));
    }

    #[test]
    fn una_tanda_sin_cerrar_dice_donde_esta_lo_pasado_y_como_terminarla() {
        let tanda = Path::new("resultados").join("t1");
        let texto = mensaje_tanda_sin_cerrar(&Error::Banco("falló la copia".into()), &tanda);

        assert!(texto.contains("falló la copia"), "{texto}");
        assert!(texto.contains("pasadas.toml"), "{texto}");
        assert!(texto.contains("--banco-comprobar"), "{texto}");
        assert!(!texto.contains("--maquina-autorizada"), "{texto}");
    }

    #[test]
    fn las_herramientas_ausentes_se_nombran_y_se_dice_donde_mirar() {
        let texto = mensaje_herramientas_ausentes(&["rust".into(), "lua".into()]);
        assert!(texto.contains("rust, lua"), "{texto}");
        assert!(texto.contains("no evaluado"), "{texto}");
        assert!(
            texto.contains("[comprobacion_local.herramientas]"),
            "{texto}"
        );
        assert!(texto.contains("se escriben y se guardan igual"), "{texto}");
        assert!(texto.contains("activa = false"), "{texto}");
    }

    #[test]
    fn el_motivo_de_una_herramienta_ausente_dice_que_el_codigo_esta_y_que_falta_ejecutarlo() {
        let una = motivo_herramientas_no_instaladas(&["php".into()]);
        assert!(
            una.starts_with("el código está escrito y guardado"),
            "{una}"
        );
        assert!(
            una.contains("php no está instalado en este equipo"),
            "{una}"
        );
        assert!(una.contains("activa = false"), "{una}");

        let varias =
            motivo_herramientas_no_instaladas(&["php".into(), "lua".into(), "node".into()]);
        assert!(
            varias.contains("php, lua y node no están instalados"),
            "{varias}"
        );
    }

    #[test]
    fn el_motivo_con_las_pruebas_apagadas_dice_como_encenderlas() {
        let texto = motivo_pruebas_ocultas_apagadas();
        assert!(
            texto.starts_with("el código está escrito y guardado"),
            "{texto}"
        );
        assert!(texto.contains("apagadas"), "{texto}");
        assert!(texto.contains("activa = true"), "{texto}");
    }

    #[test]
    fn todo_motivo_de_no_evaluado_empieza_por_lo_que_si_esta() {
        for motivo in [
            motivo_preparacion_fallida("npm install falló"),
            motivo_orden_sin_ejecutar("program not found"),
            motivo_entrega_sin_llevar("disco lleno"),
        ] {
            assert!(
                motivo.starts_with("el código está escrito y guardado"),
                "{motivo}"
            );
        }
    }

    #[test]
    fn el_error_de_una_herramienta_sin_orden_ofrece_las_dos_salidas() {
        let texto = mensaje_herramientas_sin_orden(&[("P01".into(), "php".into())]);
        assert!(texto.contains("php = \"php --version\""), "{texto}");
        assert!(texto.contains("activa = false"), "{texto}");
    }

    #[test]
    fn el_avance_distingue_lo_que_no_llego_a_tiempo() {
        let bien = mensaje_de_avance(1, 45, "E03", 42, true, Duration::from_secs(130), 1);
        let mal = mensaje_de_avance(2, 45, "E04", 42, false, Duration::from_secs(901), 0);

        assert_eq!(bien, "[1/45] E03, semilla 42: 130 s, 1 ficheros entregados");
        assert!(mal.contains("sin respuesta a tiempo"));
    }

    fn veredicto(diferencias: Vec<Diferencia>, pasadas: Vec<ComparacionDePasada>) -> Veredicto {
        Veredicto {
            diferencias,
            faltan_ajustes_del_motor: false,
            pasadas,
        }
    }

    fn pasada(encargo: &str, estado: EstadoDePasada, cortada: bool) -> ComparacionDePasada {
        ComparacionDePasada {
            encargo: encargo.into(),
            semilla: 42,
            estado,
            cortada_por_tiempo: cortada,
        }
    }

    #[test]
    fn la_comparacion_de_dos_tandas_iguales_lo_dice_y_no_nombra_pasadas() {
        let texto = mensaje_comparacion(
            &veredicto(
                Vec::new(),
                vec![pasada("E01", EstadoDePasada::Identica, false)],
            ),
            Path::new("a"),
            Path::new("b"),
        );

        assert!(texto.contains("✅"), "{texto}");
        assert!(texto.contains("1 de 1 pasadas idénticas"), "{texto}");
        assert!(!texto.contains("E01"), "{texto}");
    }

    #[test]
    fn la_comparacion_nombra_la_vuelta_donde_se_separan_y_el_corte_por_tiempo() {
        let texto = mensaje_comparacion(
            &veredicto(
                Vec::new(),
                vec![pasada(
                    "E07",
                    EstadoDePasada::Diverge {
                        linea: 5,
                        vuelta: Some(3),
                    },
                    true,
                )],
            ),
            Path::new("a"),
            Path::new("b"),
        );

        assert!(texto.contains("🔴"), "{texto}");
        assert!(
            texto.contains("E07 con la semilla 42: se separan en la vuelta 3"),
            "{texto}"
        );
        assert!(texto.contains("tiempo máximo"), "{texto}");
    }

    #[test]
    fn la_comparacion_dice_primero_por_que_no_son_comparables() {
        let texto = mensaje_comparacion(
            &veredicto(
                vec![Diferencia {
                    campo: Campo::CapasEnGpu,
                    en_a: "34".into(),
                    en_b: "35".into(),
                }],
                vec![pasada("E01", EstadoDePasada::Identica, false)],
            ),
            Path::new("a"),
            Path::new("b"),
        );

        let motivo = texto.find("las capas en la GPU: A 34 · B 35").unwrap();
        let cuenta = texto.find("pasadas idénticas").unwrap();
        assert!(texto.contains("No son comparables"), "{texto}");
        assert!(motivo < cuenta, "{texto}");
    }

    #[test]
    fn los_mensajes_del_aislamiento_no_llevan_rachas_de_espacios() {
        let fallo = Error::Banco("Acceso denegado".into());
        for texto in [
            mensaje_sin_preparar(&fallo),
            mensaje_copiando_el_interprete(Path::new("C:/Git"), Path::new("D:/copia")),
            mensaje_interprete_sin_copia("C:/msys64/usr/bin/bash.exe"),
        ] {
            assert!(!texto.contains("  "), "{texto}");
            assert!(!texto.contains('\n'), "{texto}");
        }
    }
}
