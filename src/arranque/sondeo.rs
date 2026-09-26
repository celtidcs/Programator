//! Detección de novedades en el directorio del canal mediante cálculo de huellas.

use crate::error::{Error, Resultado};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Cómo salió una pasada de sondeo del canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sondeo {
    /// El canal cambió respecto a la pasada anterior: hay algo que atender.
    Novedades,
    /// El canal está igual que estaba.
    SinCambios,
    /// Alguna lectura falló, así que la foto del canal está incompleta y **no se ha comparado**.
    ///
    /// Existe porque lo contrario costó un fallo real en la primera ejecución sobre disco: si un
    /// fichero no se puede leer durante un instante —un antivirus que lo abre, una unidad virtual
    /// que aún no lo tiene local— desaparece de la huella exactamente igual que si lo hubieran
    /// borrado, y esa diferencia se anunciaba como novedad. Luego, al volver a leerse, se anunciaba
    /// otra. Dos avisos falsos por un parpadeo del disco.
    ///
    /// Hoy la pasada se descarta entera y la referencia anterior se conserva intacta. Quien llame
    /// debería decirlo por `stderr`: un canal que no se puede leer es un problema que hay que ver,
    /// no un silencio que disimular.
    Ignorada,
}

/// ¿Ha cambiado algo en el directorio del canal, o en cualquiera de sus subcarpetas, desde la
/// última comprobación?
///
/// Por sondeo de ruta relativa, tamaño y **huella del contenido**: los vigilantes de eventos del
/// sistema operativo pierden avisos sobre unidades virtuales como Google Drive, donde vive este
/// proyecto, así que la única alternativa fiable es comparar el estado del árbol entero en cada
/// ciclo.
///
/// **El recorrido baja a las subcarpetas**, aunque el canal de MMCelt —comprobado sobre el canal
/// real— sea plano: cuatro ficheros (`estado.md` y un buzón por agente) y ninguna carpeta. Se hace
/// así porque el error no es simétrico: bajar de más solo cuesta una comprobación que no encuentra
/// nada, mientras que no bajar dejaría a Programator dormido ante cualquier proyecto que sí
/// organice el canal en carpetas. Una versión anterior de este comentario afirmaba que el canal
/// tenía una subcarpeta `msg/`; era falso, y venía de un documento de otro proyecto.
///
/// **Por qué la huella del contenido y no la fecha de modificación:** una primera versión comparaba
/// nombre, tamaño y fecha en segundos, y no detectaba dos escrituras del mismo tamaño dentro del
/// mismo segundo. Cambiar a la fecha completa en nanosegundos parecía arreglarlo por lectura del
/// código, pero **ejecutado en bucle fallaba en torno al 60% de las veces**: en Windows la marca de
/// tiempo de escritura de un fichero no se actualiza con precisión de nanosegundo en cada llamada,
/// sino al ritmo del temporizador del sistema (del orden de milisegundos), así que dos escrituras
/// seguidas caen a menudo en la misma marca. Un hash SHA-256 del contenido no depende de ninguna
/// resolución de reloj: dos contenidos distintos casi nunca producen el mismo hash,
/// independientemente de cuándo se escribieran.
///
/// La primera llamada nunca da novedad (no hay huella previa con la que comparar); dentro deja
/// grabada la huella recién calculada para la siguiente.
///
/// **Ante la duda, no se inventa una novedad.** Ver `Sondeo::Ignorada`.
pub fn hay_novedades(canal_dir: &Path, huella_anterior: &mut Option<String>) -> Resultado<Sondeo> {
    // Verificamos la lectura de la raíz explícitamente para devolver un error que nombre la ruta
    // si el directorio no existe. Los fallos de lectura de más abajo son otra cosa: no tumban el
    // sondeo, pero tampoco se ignoran en silencio (ver `Sondeo::Ignorada`).
    std::fs::read_dir(canal_dir).map_err(|causa| Error::Lectura {
        ruta: canal_dir.to_path_buf(),
        causa,
    })?;

    let mut partes: Vec<String> = Vec::new();
    let mut ilegibles = 0usize;
    recolectar_huellas(canal_dir, canal_dir, &mut partes, &mut ilegibles);

    // Si algo del canal no se pudo leer, la foto está incompleta y **no se compara**: un fichero
    // ilegible durante un instante saldría de la huella igual que uno borrado, y la diferencia se
    // anunciaría como novedad. Conservamos la referencia buena y esperamos a la vuelta siguiente;
    // un cambio real no se pierde, solo se retrasa un ciclo.
    if ilegibles > 0 {
        return Ok(Sondeo::Ignorada);
    }

    partes.sort();
    let huella = partes.join("|");

    let cambio = match huella_anterior.as_deref() {
        Some(previa) => previa != huella,
        None => false,
    };

    *huella_anterior = Some(huella);

    Ok(if cambio {
        Sondeo::Novedades
    } else {
        Sondeo::SinCambios
    })
}

/// Recorre `dir_actual` y añade a `partes` una línea por cada fichero que encuentra, bajando
/// recursivamente a las subcarpetas. Cada línea lleva la ruta relativa a `raiz` (con `/` como
/// separador, para que la huella no dependa de la plataforma), el tamaño en bytes y el hash
/// SHA-256 hexadecimal de su contenido.
///
/// Usar la ruta relativa completa, y no solo el nombre suelto del fichero, es lo que evita que dos
/// ficheros con el mismo nombre en carpetas distintas se confundan entre sí.
///
/// **No sigue enlaces simbólicos ni uniones de directorio** (`file_type().is_symlink()` los
/// detecta sin necesidad de abrirlos): seguirlos podría entrar en un ciclo infinito si el enlace
/// apunta a un antepasado suyo, o sacar el sondeo fuera del ámbito del canal.
///
/// Cualquier entrada que no se pueda leer (permisos, un antivirus que la tiene abierta, un fichero
/// que la unidad virtual aún no ha traído) **no tumba el recorrido, pero se cuenta** en `ilegibles`.
/// Descartarla en silencio sería peor que fallar: el fichero saldría de la huella igual que si lo
/// hubieran borrado, y `hay_novedades` anunciaría una novedad que no existe.
fn recolectar_huellas(
    raiz: &Path,
    dir_actual: &Path,
    partes: &mut Vec<String>,
    ilegibles: &mut usize,
) {
    let Ok(entradas) = std::fs::read_dir(dir_actual) else {
        // Una subcarpeta que no se deja listar deja la foto incompleta, igual que un fichero que no
        // se deja leer: se cuenta y quien decide es `hay_novedades`.
        *ilegibles += 1;
        return;
    };

    for entrada in entradas {
        let Ok(entrada) = entrada else {
            *ilegibles += 1;
            continue;
        };
        let Ok(tipo) = entrada.file_type() else {
            *ilegibles += 1;
            continue;
        };

        if tipo.is_symlink() {
            continue;
        }

        let ruta = entrada.path();

        if tipo.is_dir() {
            recolectar_huellas(raiz, &ruta, partes, ilegibles);
            continue;
        }

        if !tipo.is_file() {
            continue;
        }

        let Ok(contenido) = std::fs::read(&ruta) else {
            // El fichero está ahí pero no se deja leer. Omitirlo sin más lo haría indistinguible de
            // uno borrado, que es justo el falso positivo que `Sondeo::Ignorada` existe para evitar.
            *ilegibles += 1;
            continue;
        };
        let relativa = ruta
            .strip_prefix(raiz)
            .unwrap_or(&ruta)
            .to_string_lossy()
            .replace('\\', "/");
        partes.push(format!(
            "{relativa}:{}:{}",
            contenido.len(),
            huella_contenido(&contenido)
        ));
    }
}

/// Hash SHA-256 hexadecimal del contenido, para distinguir ficheros que cambian de contenido sin
/// cambiar de tamaño. No depende de ninguna resolución de reloj del sistema de ficheros.
fn huella_contenido(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
