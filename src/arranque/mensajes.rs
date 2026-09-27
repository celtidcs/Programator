//! Redacción y formateo de avisos y mensajes para la terminal.
//!
//! Cada mensaje orienta al operador indicando qué falló (con su causa), qué consecuencia tiene
//! para el ciclo y qué acción llevar a cabo (o si no se requiere ninguna).

use crate::error::Error;
use crate::plataforma::{EJEMPLO_DE_RUTA, NOMBRE_DEL_EJECUTABLE};
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
            if ruta.ends_with("programator.toml") {
                "Comprueba que «programator.toml» existe junto al ejecutable (puedes copiar y \
                 adaptar «programator.ejemplo.toml») y que tienes permisos de lectura."
                    .to_string()
            } else if ruta.extension().is_some_and(|ext| ext == "md") {
                "Comprueba que el archivo de plantilla configurado existe y es legible, o revisa \
                 la ruta indicada en «programator.toml»."
                    .to_string()
            } else {
                "Comprueba que el archivo indicado existe y que el usuario actual tiene permisos \
                 de lectura."
                    .to_string()
            }
        }
        Error::Configuracion(msg) => {
            let minusculas = msg.to_lowercase();
            if minusculas.contains("carpeta") {
                format!(
                    "Revisa la clave «ruta» en la sección [carpeta] de «programator.toml», o \
                     arranca indicando una carpeta válida con «{NOMBRE_DEL_EJECUTABLE} --ruta \
                     <DIRECTORIO>»."
                )
            } else if minusculas.contains("motor")
                || minusculas.contains("puerto")
                || minusculas.contains("modelo")
            {
                format!(
                    "Revisa la sección [motor] de «programator.toml» (puerto, binario y modelo) o \
                     ejecuta «{NOMBRE_DEL_EJECUTABLE} --diagnostico» para comprobar el encaje de \
                     hardware."
                )
            } else {
                format!(
                    "Revisa los valores y la sintaxis de «programator.toml», o ejecuta \
                     «{NOMBRE_DEL_EJECUTABLE} --diagnostico» para verificar la configuración."
                )
            }
        }
        Error::Escritura { .. } => {
            "Comprueba los permisos de escritura en la ruta indicada y que la unidad de disco no \
             esté llena."
                .to_string()
        }
        Error::MotorSinRespuesta(_) => format!(
            "Comprueba que llama-server esté en ejecución en el puerto configurado o ejecuta \
             «{NOMBRE_DEL_EJECUTABLE} --diagnostico» para revisar la GPU y el motor."
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
             con «{NOMBRE_DEL_EJECUTABLE} --diagnostico» si el servidor de inferencia sigue \
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
    if intervalo_segundos == 0 {
        return "cada 0 segundos".to_string();
    }
    if intervalo_segundos < 60 {
        if intervalo_segundos == 1 {
            "cada segundo".to_string()
        } else {
            format!("cada {intervalo_segundos} segundos")
        }
    } else {
        let minutos = intervalo_segundos / 60;
        let segundos = intervalo_segundos % 60;
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

/// Comprueba si existe un registro de lectura previo con datos en el proyecto.
pub fn hay_registro_previo(ruta_registro: &Path) -> bool {
    crate::protocolo::RegistroLectura::cargar(ruta_registro)
        .map(|reg| !reg.esta_vacio())
        .unwrap_or(false)
}

/// Redacta el mensaje de identificación de versión.
pub fn mensaje_version(version: &str) -> String {
    format!("Programator {version}")
}

/// Redacta el mensaje emitido cuando se pasa un argumento no reconocido por la línea de comandos.
pub fn mensaje_argumento_no_entendido(cual: &str) -> String {
    format!(
        "No entiendo «{cual}». Prueba «{NOMBRE_DEL_EJECUTABLE} --ayuda» para ver qué se le puede \
         pedir a Programator."
    )
}

/// Redacta el mensaje emitido cuando una opción requiere un valor y no se le proporcionó.
pub fn mensaje_argumento_falta_valor(cual: &str) -> String {
    format!(
        "«{cual}» necesita un valor detrás. Ejemplo: {NOMBRE_DEL_EJECUTABLE} --ruta \
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
    "Publicada la guía para el resto del equipo en el canal.".to_string()
}

/// Redacta el aviso cuando el sondeo detecta novedades en el canal para atender.
pub fn mensaje_novedades_atendiendo() -> String {
    "Novedades en el canal: atendiendo…".to_string()
}

/// Redacta el aviso cuando se reanuda un encargo que había quedado en espera de que cargara el motor.
pub fn mensaje_reanudacion_encargo_esperando_motor() -> String {
    "Se retoma el encargo que quedó esperando a que cargara el modelo.".to_string()
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

/// Redacta el resumen de encargos atendidos y fallos de publicación al cierre de una pasada.
pub fn mensaje_desenlace_atendidos(atendidos: usize, fallidos: usize) -> String {
    format!("Encargos atendidos: {atendidos} (fallos al publicar: {fallidos}).")
}
