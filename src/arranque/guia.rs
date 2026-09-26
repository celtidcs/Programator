//! Preparación y publicación de instrucciones y guía para los demás agentes.

use crate::error::{Error, Resultado};
use std::path::Path;

/// Lo que la plantilla de la guía trae escrito donde va la ventana de contexto.
const MARCADOR_CONTEXTO: &str = "{contexto}";

/// Deja en el canal la guía que explica a los demás agentes cómo encargarle trabajo.
///
/// **Se sobrescribe siempre, a diferencia de las normas.** La guía dice qué sabe hacer este modelo,
/// qué no, y dónde miente —se inventa APIs cuando suenan plausibles—. Eso cambia con cada versión
/// del arnés y con cada modelo que se ponga detrás: una guía vieja sería peor que ninguna, porque
/// se leería como vigente.
///
/// La escribe el arnés y no el modelo, que es la única forma de que el aviso sea fiable: si
/// dependiera de que el modelo se acuerde de declarar sus límites, no serviría de nada.
pub fn publicar_guia(
    canal: &Path,
    plantilla: &Path,
    destino: &str,
    contexto: u32,
    desempeno: &Path,
    comprobadores: &crate::arnes::verificacion::Comprobadores,
) -> Resultado<()> {
    let contenido = std::fs::read_to_string(plantilla).map_err(|causa| Error::Lectura {
        ruta: plantilla.to_path_buf(),
        causa,
    })?;
    let cifras = std::fs::read_to_string(desempeno).map_err(|causa| Error::Lectura {
        ruta: desempeno.to_path_buf(),
        causa,
    })?;

    // El repertorio y el desempeño se componen antes que el contexto: la guía es la única
    // plantilla que trae los tres marcadores, y componer en este orden es lo que la mantiene
    // fiable pieza a pieza en vez de a base de reescribirla a mano cada vez que algo cambia.
    let contenido = crate::arnes::redaccion::componer_guia(&contenido, &cifras);
    let contenido = crate::arnes::normas::componer(&contenido, comprobadores);

    // La ventana la decide el TOML, así que escribirla a mano en la plantilla condena a las dos
    // cifras a divergir: en NatureLand la guía decía 16.384 y el TOML 32.768, las dos eran ciertas
    // y ninguna decía cuál mandaba. Quien dimensione un encargo lee aquí la que está en vigor.
    let contenido = contenido.replace(MARCADOR_CONTEXTO, &contexto.to_string());

    let ruta_destino = canal.join(destino);
    std::fs::write(&ruta_destino, contenido).map_err(|causa| Error::Escritura {
        ruta: ruta_destino,
        causa,
    })
}

/// Deja el fichero de instrucciones en su sitio, si aún no está.
///
/// Devuelve `true` si lo escribió. **Nunca sobrescribe**: si el proyecto ya tiene el suyo, ese
/// manda, porque puede llevar las normas que alguien escribió a mano para este proyecto concreto.
/// Es justo al revés que `publicar_guia`, y por un motivo: las instrucciones las acuerda el equipo,
/// la guía la mantiene el arnés.
///
/// Recibe el contenido ya compuesto y la ruta ya calculada: componer es asunto de `arnes::normas` y
/// decidir dónde, de `arnes::espacio`. Aquí solo se escribe.
pub fn instalar_normas(destino: &Path, contenido: &str) -> Resultado<bool> {
    if destino.exists() {
        return Ok(false);
    }

    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre).map_err(|causa| Error::Escritura {
            ruta: padre.to_path_buf(),
            causa,
        })?;
    }

    std::fs::write(destino, contenido).map_err(|causa| Error::Escritura {
        ruta: destino.to_path_buf(),
        causa,
    })?;

    Ok(true)
}
