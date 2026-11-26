//! El texto de `--ayuda`: las formas de uso, alineadas en dos columnas, y lo que conviene saber de
//! cada opción.

use super::{
    OPCION_AYUDA, OPCION_BANCO, OPCION_BANCO_COMPARAR, OPCION_BANCO_COMPROBAR,
    OPCION_BANCO_INFORME, OPCION_DESCARGAR_MODELO, OPCION_DIAGNOSTICO, OPCION_MODELO,
    OPCION_REGISTRO_ANOTAR, OPCION_RETIRAR_MODELO, OPCION_REVISION_BUSCAR,
    OPCION_REVISION_PROPONER, OPCION_RUTA, OPCION_VERSION,
};

/// Columna en la que empieza la descripción de cada forma de uso.
///
/// Se compone, no se escribe a mano con espacios, porque el nombre del ejecutable cambia de
/// longitud entre Windows y el resto de sistemas y la tabla quedaría desalineada en uno de los
/// dos.
pub(super) const COLUMNA_DE_LA_DESCRIPCION: usize = 31;

/// Una línea de la sección USO: la invocación a la izquierda y qué hace a la derecha.
///
/// Si la invocación no cabe en su columna, la descripción baja a la línea siguiente en vez de
/// empujar la tabla hacia la derecha. Así el texto sigue leyéndose en una terminal estrecha.
pub(super) fn linea_de_uso(invocacion: &str, descripcion: &str) -> String {
    let sangrado = "  ";
    // La columna se mide en caracteres, no en bytes: una invocación con «ñ» o con acento
    // ocupa en la terminal menos de lo que pesa.
    let escrito = sangrado.chars().count() + invocacion.chars().count();
    if escrito < COLUMNA_DE_LA_DESCRIPCION {
        let relleno = COLUMNA_DE_LA_DESCRIPCION - escrito;
        format!("{sangrado}{invocacion}{:relleno$}{descripcion}\n", "")
    } else {
        format!(
            "{sangrado}{invocacion}\n{:COLUMNA_DE_LA_DESCRIPCION$}{descripcion}\n",
            ""
        )
    }
}

/// El texto que escribe `--ayuda`.
pub fn texto_de_ayuda(version: &str) -> String {
    let programa = crate::plataforma::NOMBRE_DEL_EJECUTABLE;
    let uso = [
        (
            programa.to_string(),
            "Arranca y entra en el ciclo de trabajo.",
        ),
        (
            format!("{programa} {OPCION_RUTA} <carpeta>"),
            "Arranca sobre esa carpeta, sin abrir el diálogo.",
        ),
        (
            format!("{programa} {OPCION_AYUDA}"),
            "Muestra esta ayuda y termina.",
        ),
        (
            format!("{programa} {OPCION_VERSION}"),
            "Escribe la versión y termina.",
        ),
        (
            format!("{programa} {OPCION_DIAGNOSTICO} [{OPCION_MODELO} <fichero>]"),
            "Escribe el estado de la máquina, el motor y el modelo.",
        ),
        (
            format!("{programa} {OPCION_BANCO} <carpeta> [{OPCION_MODELO} <fichero>]"),
            "Pasa al modelo una tanda del banco de modelos.",
        ),
        (
            format!("{programa} {OPCION_BANCO_COMPROBAR} <tanda>"),
            "Hace solo las pruebas ocultas de una tanda ya pasada.",
        ),
        (
            format!("{programa} {OPCION_BANCO_INFORME} <resultados.toml>"),
            "Rehace el informe de una tanda con las rúbricas puntuadas.",
        ),
        (
            format!("{programa} {OPCION_BANCO_COMPARAR} <tanda-a> <tanda-b>"),
            "Dice si dos tandas guardadas son repetición una de otra.",
        ),
        (
            format!("{programa} {OPCION_REGISTRO_ANOTAR} <tanda>"),
            "Anota en el registro de modelos lo que midió una tanda terminada.",
        ),
        (
            format!("{programa} {OPCION_REVISION_BUSCAR}"),
            "Busca en Hugging Face modelos que quepan en la tarjeta.",
        ),
        (
            format!("{programa} {OPCION_DESCARGAR_MODELO} <repositorio> <fichero>"),
            "Descarga un candidato de la última búsqueda y comprueba su huella.",
        ),
        (
            format!("{programa} {OPCION_REVISION_PROPONER}"),
            "Propone qué modelos se quedan y cuáles se retiran, según el registro.",
        ),
        (
            format!("{programa} {OPCION_RETIRAR_MODELO} <fichero>"),
            "Anota la retirada en el registro y borra el fichero del modelo.",
        ),
    ]
    .iter()
    .map(|(invocacion, que_hace)| linea_de_uso(invocacion, que_hace))
    .collect::<String>();

    format!(
        "Programator {version} — arnés que convierte un modelo local en un agente del canal

USO
{uso}
  «{OPCION_RUTA}» manda sobre «[carpeta] ruta» del TOML, y «{OPCION_DIAGNOSTICO}» termina
  sin arrancar el ciclo.
  «{OPCION_BANCO}» lee el banco.toml de esa carpeta. Ejecuta en este equipo las pruebas
  ocultas, salvo que [comprobacion_local] diga «activa = false». Si fallan las
  pruebas, «{OPCION_BANCO_COMPROBAR}» termina la tanda con lo que dejó en «pasadas.toml», sin
  volver a cargar el modelo.
  «{OPCION_MODELO}» prueba otro fichero GGUF en lugar de «[motor] modelo», solo en esa
  ejecución y sin tocar el TOML; se resuelve igual que el del TOML.
  La revisión mensual va por pasos: buscar, descargar, pasar el banco, proponer y
  retirar. Se configura en «[revision]» y deja lo suyo en «revisiones/AAAA-MM/».
  También se puede pedir por el canal, con «revisión: buscar», «revisión: probar» y
  «revisión: cerrar»: la atiende el arnés, no el modelo, y solo de las identidades de
  «[revision] identidades_que_ordenan». El procedimiento está en
  «plantillas/revision-mensual.md».

CONFIGURACIÓN
  Se lee de «programator.toml», en la misma carpeta que el ejecutable.
  Las rutas relativas se resuelven contra ese fichero, nunca contra el
  directorio desde el que lances el programa.

DÓNDE MIRAR
  Pruebas manuales:   docs/pruebas-manuales.md
  Defectos conocidos: docs/defectos-conocidos.md
"
    )
}
