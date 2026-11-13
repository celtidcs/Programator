use std::fmt;

/// Por qué no se pudo expandir la entrada, y en qué parte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorRango {
    CadenaVacia,
    SeparadorSobrante { posicion: usize },
    NumeroNoValido { posicion: usize, texto: String },
    RangoInvertido { posicion: usize, inicio: u32, fin: u32 },
}

impl fmt::Display for ErrorRango {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CadenaVacia => write!(f, "la cadena está vacía"),
            Self::SeparadorSobrante { posicion } => {
                write!(f, "separador sobrante en la parte {posicion}")
            }
            Self::NumeroNoValido { posicion, texto } => {
                write!(f, "«{texto}» no es un número válido (parte {posicion})")
            }
            Self::RangoInvertido { posicion, inicio, fin } => {
                write!(f, "rango invertido {inicio}-{fin} (parte {posicion})")
            }
        }
    }
}

impl std::error::Error for ErrorRango {}

pub fn expandir(entrada: &str) -> Result<Vec<u32>, ErrorRango> {
    if entrada.trim().is_empty() {
        return Err(ErrorRango::CadenaVacia);
    }
    let mut numeros = Vec::new();
    for (posicion, parte) in entrada.split(',').enumerate() {
        let parte = parte.trim();
        if parte.is_empty() {
            return Err(ErrorRango::SeparadorSobrante { posicion });
        }
        match parte.split_once('-') {
            Some((inicio, fin)) => {
                let inicio = numero(inicio, posicion)?;
                let fin = numero(fin, posicion)?;
                if inicio > fin {
                    return Err(ErrorRango::RangoInvertido { posicion, inicio, fin });
                }
                numeros.extend(inicio..=fin);
            }
            None => numeros.push(numero(parte, posicion)?),
        }
    }
    Ok(numeros)
}

fn numero(texto: &str, posicion: usize) -> Result<u32, ErrorRango> {
    texto
        .trim()
        .parse::<u32>()
        .map_err(|_| ErrorRango::NumeroNoValido {
            posicion,
            texto: texto.trim().to_string(),
        })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_caso_del_enunciado() {
        assert_eq!(expandir("1-3,7,10-12"), Ok(vec![1, 2, 3, 7, 10, 11, 12]));
    }

    #[test]
    fn vacia() {
        assert_eq!(expandir(""), Err(ErrorRango::CadenaVacia));
    }

    #[test]
    fn invertido() {
        assert!(matches!(expandir("5-2"), Err(ErrorRango::RangoInvertido { .. })));
    }

    #[test]
    fn no_valido() {
        assert!(matches!(expandir("1,x"), Err(ErrorRango::NumeroNoValido { posicion: 1, .. })));
    }

    #[test]
    fn sobrante() {
        assert_eq!(expandir("1,,2"), Err(ErrorRango::SeparadorSobrante { posicion: 1 }));
    }
}
