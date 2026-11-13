//! Utilidades de texto para los nombres que escriben los usuarios en un formulario.
//!
//! Cada función hace una sola cosa y ninguna reserva memoria de más: las que pueden devolver el
//! texto tal cual devuelven un `Cow`.

use std::borrow::Cow;

/// Por qué un nombre no es aceptable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorNombre {
    Vacio,
    CaracterNoPermitido(char),
}

impl std::fmt::Display for ErrorNombre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vacio => write!(f, "el nombre está vacío"),
            Self::CaracterNoPermitido(c) => write!(f, "el carácter «{c}» no está permitido"),
        }
    }
}

impl std::error::Error for ErrorNombre {}

/// Sustituye cada racha de espacios en blanco por un único espacio y quita los de los extremos.
pub fn normalizar_espacios(texto: &str) -> Cow<'_, str> {
    let normalizado = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalizado == texto {
        Cow::Borrowed(texto)
    } else {
        Cow::Owned(normalizado)
    }
}

/// Lo que lleva un nombre de persona además de letras.
const SIGNOS_PERMITIDOS: &[char] = &[' ', '-', '\''];

fn caracter_permitido(c: char) -> bool {
    c.is_alphabetic() || SIGNOS_PERMITIDOS.contains(&c)
}

/// Normaliza el nombre y comprueba que no está vacío y que solo lleva caracteres permitidos.
pub fn validar_nombre(nombre: &str) -> Result<Cow<'_, str>, ErrorNombre> {
    let normalizado = normalizar_espacios(nombre);
    if normalizado.is_empty() {
        return Err(ErrorNombre::Vacio);
    }
    match normalizado.chars().find(|&c| !caracter_permitido(c)) {
        Some(c) => Err(ErrorNombre::CaracterNoPermitido(c)),
        None => Ok(normalizado),
    }
}

/// Las iniciales de cada palabra, en mayúsculas y seguidas de punto: «ana maría» da «A. M.».
pub fn iniciales(nombre: &str) -> String {
    nombre
        .split_whitespace()
        .filter_map(|palabra| palabra.chars().next())
        .map(|inicial| format!("{}.", inicial.to_uppercase()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn normalizar_deja_tal_cual_lo_que_ya_esta_bien() {
        assert!(matches!(normalizar_espacios("Ana López"), Cow::Borrowed(_)));
    }

    #[test]
    fn normalizar_junta_rachas_y_recorta_extremos() {
        assert_eq!(normalizar_espacios("  Ana   López "), "Ana López");
    }

    #[test]
    fn un_nombre_vacio_se_rechaza() {
        assert_eq!(validar_nombre("   "), Err(ErrorNombre::Vacio));
    }

    #[test]
    fn un_digito_se_rechaza_y_se_nombra() {
        assert_eq!(validar_nombre("Ana2"), Err(ErrorNombre::CaracterNoPermitido('2')));
    }

    #[test]
    fn guiones_y_apostrofos_se_aceptan() {
        assert!(validar_nombre("Jean-Luc O'Neill").is_ok());
    }

    #[test]
    fn iniciales_de_un_nombre_compuesto() {
        assert_eq!(iniciales("ana maría"), "A. M.");
    }
}
