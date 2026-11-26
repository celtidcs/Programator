//! Por qué un fichero GGUF no vale, tal como se le cuenta a una persona. Los lectores deciden; aquí
//! solo se redacta.

use std::fmt::Display;

/// Los cuatro primeros bytes no son `GGUF`.
pub(super) fn sin_numero_magico() -> String {
    "número mágico inválido: no empieza por «GGUF», no es un fichero GGUF".to_string()
}

/// Una versión del formato que este lector no conoce.
pub(super) fn version_desconocida(version: u32) -> String {
    format!("versión de GGUF desconocida: {version}")
}

/// La cabecera declara más tensores de los que se admiten.
pub(super) fn demasiados_tensores(declarados: u64, limite: u64) -> String {
    format!("la cabecera dice tener {declarados} tensores, más del límite de {limite}")
}

/// La cabecera declara más pares clave-valor de los que se admiten.
pub(super) fn demasiadas_claves(declaradas: u64, limite: u64) -> String {
    format!("la cabecera dice tener {declaradas} pares clave-valor, más del límite de {limite}")
}

/// Un valor de los metadatos con un tipo que no existe.
pub(super) fn tipo_de_valor_desconocido(clave: &str, codigo: u32) -> String {
    format!("tipo de valor desconocido para la clave «{clave}»: código {codigo}")
}

/// El inicio del bloque de datos más su tamaño no cabe en un `u64`.
pub(super) fn bloque_de_datos_desbordado() -> String {
    "el bloque de datos desborda al sumar su inicio y su tamaño".to_string()
}

/// Los metadatos pasan del tope de bytes que se leen.
pub(super) fn cabecera_demasiado_grande(limite: u64) -> String {
    format!("la cabecera supera el límite de {limite} bytes admitidos para metadatos")
}

/// El fichero se acabó o no se dejó leer a mitad de la cabecera.
pub(super) fn fichero_truncado(causa: &dyn Display) -> String {
    format!("fichero truncado o ilegible: {causa}")
}

/// Una cadena que dice medir más que el tope.
pub(super) fn cadena_demasiado_larga(longitud: u64, limite: u64) -> String {
    format!("una cadena de la cabecera dice medir {longitud} bytes, más del límite de {limite}")
}

/// Una cadena que dice medir más de lo que cabe en memoria en este sistema.
pub(super) fn cadena_que_no_cabe(longitud: u64) -> String {
    format!("una cadena de la cabecera dice medir {longitud} bytes, más de lo que cabe")
}

/// Una cadena que no es UTF-8.
pub(super) fn cadena_no_utf8(causa: &dyn Display) -> String {
    format!("una cadena de la cabecera no es UTF-8 válido: {causa}")
}

/// Listas dentro de listas más allá del tope.
pub(super) fn listas_demasiado_hondas(limite: u32) -> String {
    format!("arrays anidados más allá del límite de {limite} niveles")
}

/// Los elementos de una lista tienen un tipo que no existe.
pub(super) fn tipo_de_elemento_desconocido(codigo: u32) -> String {
    format!("tipo de elemento de array desconocido: código {codigo}")
}

/// Una lista con más elementos que el tope.
pub(super) fn lista_demasiado_larga(longitud: u64, limite: u64) -> String {
    format!("un array de la cabecera dice tener {longitud} elementos, más del límite de {limite}")
}

/// Un tensor con un tipo que no está en la tabla de ggml.
pub(super) fn tipo_de_tensor_desconocido(nombre: &str, codigo: u32) -> String {
    format!("tipo de tensor desconocido en «{nombre}»: código {codigo}")
}

/// El producto de las dimensiones de un tensor no cabe en un `u64`.
pub(super) fn dimensiones_desbordadas(nombre: &str) -> String {
    format!("las dimensiones de «{nombre}» desbordan al multiplicarse")
}

/// El desplazamiento de un tensor más su tamaño no cabe en un `u64`.
pub(super) fn desplazamiento_desbordado(nombre: &str) -> String {
    format!("el desplazamiento de «{nombre}» más su tamaño desborda")
}

/// Alinear el inicio de los datos se sale de un `u64`.
pub(super) fn alineacion_desbordada(alineacion: u64) -> String {
    format!("la alineación {alineacion} lleva los datos más allá de lo que cabe")
}
