//! Redacción y formateo de avisos y mensajes para la terminal.
//!
//! Cada mensaje orienta al operador indicando qué falló (con su causa), qué consecuencia tiene
//! para el ciclo y qué acción llevar a cabo (o si no se requiere ninguna).

use crate::argumentos::{OPCION_AYUDA, OPCION_DIAGNOSTICO, OPCION_REGISTRO_ANOTAR, OPCION_RUTA};
use crate::config::NOMBRE_DEL_FICHERO_DE_CONFIGURACION;
use crate::error::Error;
use crate::plataforma::{EJEMPLO_DE_RUTA, NOMBRE_DEL_EJECUTABLE};
use crate::protocolo::carpetas::ruta_legible_de_la_guia;
use std::path::Path;

/// Redacta el mensaje de error fatal cuando Programator no puede ponerse en marcha.
///
/// Explica:
/// 1. Qué ha fallado (con su error subyacente).
/// 2. Consecuencia: el proceso se detiene antes de entrar al bucle y el agente no atenderá el canal.
/// 3. Qué hacer: orientación contextual según la variante del error para que el operador sepa qué corregir.
pub fn mensaje_fallo_arranque(fallo: &Error) -> String {
    let detalle_accion = match fallo {
        Error::Lectura { ruta, .. } => {
            if ruta.ends_with(NOMBRE_DEL_FICHERO_DE_CONFIGURACION) {
                "Comprueba que «programator.toml» existe junto al ejecutable (puedes copiar y \
                 adaptar «programator.ejemplo.toml») y que tienes permisos de lectura."
                    .to_string()
            } else if ruta.extension().is_some_and(|extension| extension == "md") {
                "Comprueba que el archivo de plantilla configurado existe y es legible, o revisa \
                 la ruta indicada en «programator.toml»."
                    .to_string()
            } else {
                "Comprueba que el archivo indicado existe y que el usuario actual tiene permisos \
                 de lectura."
                    .to_string()
            }
        }
        Error::ConfiguracionDeLaCarpeta(_) => format!(
            "Revisa la clave «ruta» en la sección [carpeta] de «programator.toml», o \
             arranca indicando una carpeta válida con «{NOMBRE_DEL_EJECUTABLE} \
             {OPCION_RUTA} \
             <DIRECTORIO>»."
        ),
        Error::ConfiguracionDelMotor(_) => format!(
            "Revisa la sección [motor] de «programator.toml» (puerto, binario y modelo) o \
             ejecuta «{NOMBRE_DEL_EJECUTABLE} {OPCION_DIAGNOSTICO}» para comprobar el \
             encaje de \
             hardware."
        ),
        Error::Configuracion(_) => format!(
            "Revisa los valores y la sintaxis de «programator.toml», o ejecuta \
             «{NOMBRE_DEL_EJECUTABLE} {OPCION_DIAGNOSTICO}» para verificar la \
             configuración."
        ),
        Error::Escritura { .. } => {
            "Comprueba los permisos de escritura en la ruta indicada y que la unidad de disco no \
             esté llena."
                .to_string()
        }
        Error::MotorSinRespuesta(_) => format!(
            "Comprueba que llama-server esté en ejecución en el puerto configurado o ejecuta \
             «{NOMBRE_DEL_EJECUTABLE} {OPCION_DIAGNOSTICO}» para revisar la GPU y el motor."
        ),
        Error::GgufInvalido { .. } => {
            "Comprueba que el fichero del modelo configurado no esté dañado o incompleto y sea \
             compatible."
                .to_string()
        }
        Error::FueraDeAmbito(_) => {
            "Asegúrate de que la ruta solicitada se encuentra confinada dentro de la carpeta de \
             trabajo del proyecto."
                .to_string()
        }
        Error::PodaInvalida(_) => {
            "Revisa el formato del buzón o documento que se intentaba podar.".to_string()
        }
        Error::Banco(_) => {
            "Revisa la ficha o la configuración del banco que nombra el mensaje; el esquema está \
             en «evaluacion/banco/encargos/README.md»."
                .to_string()
        }
        Error::Registro(_) => {
            format!(
                "El registro de modelos solo lo escribe «{OPCION_REGISTRO_ANOTAR}»: si está \
                 roto, bórralo \
                 y vuelve a anotar las tandas, que conservan todas las cifras."
            )
        }
        Error::Aislamiento(_) => {
            "Windows no dejó aislar la comprobación local. Si no se puede arreglar, «aislamiento \
             = \"ninguno\"» en [comprobacion_local] la deja correr sin aislar, con todos los \
             permisos del usuario."
                .to_string()
        }
        Error::Revision(_) => {
            "Revisa la sección [revision] de «programator.toml» y que Hugging Face responda \
             desde este equipo."
                .to_string()
        }
        Error::Pendientes(_) => {
            "La cola de encargos pendientes guarda encargos que aún no tienen respuesta: no la \
             borres sin mirarla. Corrige el fichero que nombra el mensaje o apártalo y vuelve a \
             publicar esos encargos en el canal."
                .to_string()
        }
        Error::RegistroDeEncargos(_) => {
            "El registro de encargos, en la carpeta «encargos» del espacio del agente, guarda en \
             qué está cada encargo: no lo borres sin mirarlo. Corrige o aparta la ficha que nombra \
             el mensaje."
                .to_string()
        }
        Error::RegistroDeEntregas(_) => {
            "El registro de entregas, «entregas.toml» en el espacio del agente, guarda lo que \
             Programator propuso y en qué quedó: no lo borres sin mirarlo."
                .to_string()
        }
    };

    format!(
        "🔴 Programator no pudo arrancar: {fallo}.\n\
         Consecuencia: el proceso se ha detenido y el agente no estará disponible en el canal.\n\
         Qué hacer: {detalle_accion}"
    )
}

/// Redacta el aviso emitido cuando la guía para el equipo no se pudo publicar en el canal al arrancar.
pub fn mensaje_fallo_publicar_guia(fallo: &Error) -> String {
    format!(
        "⚠️ No se pudo publicar la guía para el equipo en el canal: {fallo}.\n\
         Consecuencia: el ciclo de trabajo continúa y atenderá encargos, pero los demás agentes \
         no tendrán visibles las instrucciones actualizadas sobre qué encargarle a Programator.\n\
         Qué hacer: comprueba los permisos de escritura en la carpeta «.gestor/canal/» o que la \
         plantilla configurada en [agente.guia] sea accesible."
    )
}

/// Redacta el aviso emitido cuando una pasada de sondeo encuentra ficheros bloqueados o ilegibles.
pub fn mensaje_canal_parcialmente_ilegible() -> String {
    "⚠️ Parte del canal no se pudo leer en esta vuelta, así que la comprobación se descarta.\n\
     Consecuencia: se conserva intacta la referencia anterior para evitar falsos positivos y no se \
     atienden encargos en este ciclo. Se reintentará en la siguiente vuelta.\n\
     Qué hacer: no hace falta hacer nada si es una pausa puntual de sincronización (por ejemplo en \
     Google Drive); si el aviso se repite continuamente, comprueba que ningún archivo del canal esté \
     bloqueado en modo exclusivo por otro editor o proceso."
        .to_string()
}

/// Redacta el aviso emitido cuando la ejecución de una pasada de atención falla.
pub fn mensaje_fallo_pasada_ciclo(fallo: &Error) -> String {
    let accion = match fallo {
        Error::MotorSinRespuesta(_) => format!(
            "Si se debe a contención temporal de GPU o carga del modelo, el arnés reintentará \
             automáticamente en la siguiente vuelta. Si persiste tras varios ciclos, comprueba \
             con «{NOMBRE_DEL_EJECUTABLE} {OPCION_DIAGNOSTICO}» si el servidor de inferencia sigue \
             activo."
        ),
        Error::Lectura { .. } | Error::Escritura { .. } => {
            "Comprueba que los ficheros del proyecto y del canal sigan siendo accesibles y \
             dispongan de permisos de lectura y escritura."
                .to_string()
        }
        _ => "El arnés reintentará en el próximo ciclo; no es necesario intervenir a menos que el \
             fallo se repita de forma continuada."
            .to_string(),
    };

    format!(
        "⚠️ La pasada de este ciclo falló: {fallo}.\n\
         Consecuencia: el encargo no se ha completado y se reintentará en el siguiente ciclo.\n\
         Qué hacer: {accion}"
    )
}

/// Redacta el aviso emitido cuando falla la comprobación de novedades en la raíz del canal.
pub fn mensaje_fallo_sondeo_canal(fallo: &Error) -> String {
    format!(
        "⚠️ No se pudo sondear el canal en esta vuelta: {fallo}.\n\
         Consecuencia: no se han podido comprobar novedades y se pospone la revisión al siguiente \
         intervalo de sondeo.\n\
         Qué hacer: si el canal está en una unidad virtual o sincronizada, es normal durante una \
         sincronización y no requiere acción; si el fallo persiste, comprueba que la carpeta del \
         canal exista y sea accesible."
    )
}

/// Describe la cadencia de comprobación del canal para los avisos al operador en lenguaje natural.
///
/// Se expresa en la unidad más legible: minutos exactos cuando no hay residuo, minutos y segundos
/// cuando ambos son distintos de cero, y segundos cuando el intervalo es inferior a un minuto.
pub fn cadencia_sondeo(intervalo_segundos: u64) -> String {
    use crate::unidades::SEGUNDOS_POR_MINUTO;
    if intervalo_segundos == 0 {
        return "cada 0 segundos".to_string();
    }
    if intervalo_segundos < SEGUNDOS_POR_MINUTO {
        if intervalo_segundos == 1 {
            "cada segundo".to_string()
        } else {
            format!("cada {intervalo_segundos} segundos")
        }
    } else {
        let minutos = intervalo_segundos / SEGUNDOS_POR_MINUTO;
        let segundos = intervalo_segundos % SEGUNDOS_POR_MINUTO;
        if segundos == 0 {
            if minutos == 1 {
                "cada minuto".to_string()
            } else {
                format!("cada {minutos} minutos")
            }
        } else {
            let texto_minutos = if minutos == 1 {
                "1 minuto".to_string()
            } else {
                format!("{minutos} minutos")
            };
            let texto_segundos = if segundos == 1 {
                "1 segundo".to_string()
            } else {
                format!("{segundos} segundos")
            };
            format!("cada {texto_minutos} y {texto_segundos}")
        }
    }
}

/// Redacta el mensaje tras el primer contacto con el proyecto (cuando se toma la instantánea inicial).
///
/// Explica que el contenido previo se toma como punto de partida sin atender encargos anteriores,
/// que Programator ya está escuchando, que lo publicado a partir de ahora será atendido y la
/// cadencia de sondeo en segundos, sin recurrir a jerga interna («registro vacío», «primera pasada»).
pub fn mensaje_listo_primer_arranque(intervalo_segundos: u64) -> String {
    let cadencia = cadencia_sondeo(intervalo_segundos);
    format!(
        "Se ha tomado el canal existente como punto de partida sin atender encargos anteriores. \
         Programator ya está escuchando: cualquier encargo que se publique a partir de ahora \
         será atendido (comprobación del canal {cadencia})."
    )
}

/// Redacta el mensaje de arranque cuando ya existe un registro previo de lecturas en el proyecto.
///
/// Avisa al operador de que el agente está vivo y escuchando el canal, indicando la cadencia de
/// comprobación en segundos.
pub fn mensaje_listo_reinicio(intervalo_segundos: u64) -> String {
    let cadencia = cadencia_sondeo(intervalo_segundos);
    format!(
        "Programator está escuchando el canal: cualquier encargo nuevo será atendido \
         (comprobación del canal {cadencia})."
    )
}

/// Un bloque del texto de arranque, listo para `println!` o `eprintln!`: sin los saltos de línea
/// que traiga al final y con uno solo añadido, de modo que al imprimirlo queda una línea en blanco
/// debajo. Todo seguido costaba leerlo.
pub fn bloque(texto: &str) -> String {
    format!("{}\n", texto.trim_end_matches('\n'))
}

/// Redacta el mensaje de identificación de versión.
pub fn mensaje_version(version: &str) -> String {
    format!("Programator {version}")
}

/// Redacta el mensaje emitido cuando se pasa un argumento no reconocido por la línea de comandos.
pub fn mensaje_argumento_no_entendido(cual: &str) -> String {
    format!(
        "No entiendo «{cual}». Prueba «{NOMBRE_DEL_EJECUTABLE} {OPCION_AYUDA}» para ver qué se \
         le puede \
         pedir a Programator."
    )
}

/// Redacta el mensaje emitido cuando una opción requiere un valor y no se le proporcionó.
pub fn mensaje_argumento_falta_valor(cual: &str) -> String {
    format!(
        "«{cual}» necesita un valor detrás. Ejemplo: {NOMBRE_DEL_EJECUTABLE} {OPCION_RUTA} \
         \"{EJEMPLO_DE_RUTA}\"."
    )
}

/// Redacta la confirmación de la carpeta de trabajo seleccionada y su origen de resolución.
pub fn mensaje_carpeta_de_trabajo(ruta: &Path, descripcion_origen: &str) -> String {
    format!(
        "Carpeta de trabajo: {} — {}",
        ruta.display(),
        descripcion_origen
    )
}

/// Redacta el aviso emitido cuando se instalan las normas iniciales en el espacio propio del agente.
pub fn mensaje_instrucciones_instaladas(destino_normas: &Path) -> String {
    format!(
        "Instaladas las instrucciones del proyecto en {}",
        destino_normas.display()
    )
}

/// Redacta el aviso de publicación exitosa de la guía para el equipo en el canal.
pub fn mensaje_guia_publicada() -> String {
    format!(
        "Publicada la guía para el resto del equipo en {}.",
        ruta_legible_de_la_guia()
    )
}

/// La guía para personas en el paquete portable: el LEEME, junto al ejecutable
/// (`empaquetado/contenido.txt`).
const GUIA_PARA_PERSONAS_EN_EL_PAQUETE: &str = "LEEME.md";
/// La guía para personas en el repositorio.
const GUIA_PARA_PERSONAS_EN_EL_REPOSITORIO: &str = "docs/Manual de uso.md";

/// Dónde está la guía para personas, y que la del canal es para el agente que dirige.
pub fn mensaje_guia_para_personas() -> String {
    format!(
        "📖 La guía para ti es {GUIA_PARA_PERSONAS_EN_EL_PAQUETE}, junto al ejecutable (en el \
         repositorio, {GUIA_PARA_PERSONAS_EN_EL_REPOSITORIO}). La del canal, {}, es para el agente \
         que dirige a Programator: que la lea antes de encargarle nada.",
        ruta_legible_de_la_guia()
    )
}

/// Redacta la orientación para el usuario sobre cómo comenzar la colaboración desde su modelo de pago habitual.
pub fn mensaje_orientacion_modelo_pago() -> String {
    format!(
        "💡 Cómo empezar:\n   \
     Abre tu modelo de pago habitual (Claude, ChatGPT, etc.) en esta carpeta de trabajo y pídele:\n   \
     «Accede a la ayuda de Programator en {}, \
     infórmame de sus capacidades y coordínate con él para gestionar el trabajo. Antes de \
     encargarle nada, elige tu identidad, de la forma «Herramienta N» (por ejemplo {}), y \
     escríbela en la primera línea de tu buzón en .gestor/canal/: «{}<tu identidad>». Después \
     de cada encargo, lanza en segundo plano {} con esa misma identidad, y vuelve a lanzarlo tras \
     cada aviso.»",
        ruta_legible_de_la_guia(),
        crate::protocolo::respuesta::ejemplos_de_identidad(),
        crate::protocolo::respuesta::CABECERA_DE_BANDEJA,
        super::vigilante::orden_para_el_agente()
    )
}

/// Redacta el aviso cuando el sondeo detecta novedades en el canal para atender.
pub fn mensaje_novedades_atendiendo() -> String {
    "Novedades en el canal: atendiendo…".to_string()
}

/// Redacta el aviso cuando se retoma lo que la vuelta anterior dejó para luego: un encargo que
/// esperaba a que cargara el modelo, o un buzón que se estaba escribiendo.
pub fn mensaje_se_retoma_lo_aplazado() -> String {
    "Se retoma lo que quedó para esta vuelta: el modelo cargando o un buzón a medio escribir."
        .to_string()
}

/// Redacta el desenlace cuando un buzón acababa de cambiar y podía estar a medio escribir.
pub fn mensaje_desenlace_buzon_a_medias() -> String {
    "Un buzón se estaba escribiendo: lo nuevo se lee en el ciclo siguiente.".to_string()
}

/// Redacta el desenlace cuando hubo novedades en el canal pero ninguna contenía un encargo para el agente.
pub fn mensaje_desenlace_sin_encargos() -> String {
    "Había novedades, pero ninguna era un encargo para Programator.".to_string()
}

/// Redacta el desenlace cuando el motor todavía está cargando el modelo en VRAM/RAM.
pub fn mensaje_desenlace_motor_cargando() -> String {
    "El motor local todavía está cargando el modelo: se reintenta en el ciclo siguiente."
        .to_string()
}

/// El título de la ventana emergente que pide la carpeta de trabajo.
pub const TITULO_DEL_DIALOGO_DE_CARPETA: &str = "Elige la carpeta de trabajo de Programator";

/// De dónde salió la carpeta de trabajo cuando la eligió una persona en la ventana emergente,
/// como lo dicen las demás procedencias en `carpeta::Origen::descripcion`.
pub const ORIGEN_VENTANA_EMERGENTE: &str = "elegida en la ventana emergente";

/// El error de una carpeta de trabajo que no existe, con de dónde salió.
pub fn mensaje_carpeta_inexistente(ruta: &Path, origen: &str) -> String {
    format!(
        "la carpeta de trabajo {} ({origen}) no existe",
        ruta.display()
    )
}

/// El error de cerrar la ventana emergente sin elegir carpeta.
pub fn mensaje_sin_carpeta_elegida() -> String {
    "no se eligió carpeta de trabajo".to_string()
}

/// Lo que anota el latido cuando una pasada no pudo publicar algún encargo.
pub fn mensaje_publicaciones_fallidas(fallidos: usize) -> String {
    format!("falló la publicación de {fallidos} encargo(s)")
}

/// Redacta el resumen de encargos atendidos y fallos de publicación al cierre de una pasada.
/// Lo que se dice al volver por los encargos de la cola de pendientes sin novedades en el canal.
pub fn mensaje_atendiendo_pendientes() -> String {
    "Se retoman los encargos que quedaron en la cola de pendientes.".to_string()
}

/// Cuántos encargos quedaron en la cola de pendientes al terminar la pasada.
pub fn mensaje_encargos_en_cola(pendientes: usize) -> String {
    format!(
        "Encargos en la cola de pendientes: {pendientes}. Se volverán a atender en la próxima vuelta."
    )
}

pub fn mensaje_desenlace_atendidos(atendidos: usize, fallidos: usize) -> String {
    format!("Encargos atendidos: {atendidos} (fallos al publicar: {fallidos}).")
}
