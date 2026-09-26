//! Maquinaria de bytes para GGUF: contar, acotar y descodificar. No sabe qué es un modelo ni una
//! capa, solo cómo leer un entero, una cadena o saltarse un array sin perder la posición ni superar
//! el presupuesto de bytes de la cabecera.

use super::{
    error_gguf, LIMITE_BYTES_METADATOS, LIMITE_ELEMENTOS_ARRAY, LIMITE_LONGITUD_CADENA,
    LIMITE_PROFUNDIDAD_ARRAY,
};
use crate::error::Resultado;
use std::io::Read;
use std::path::Path;

/// Los doce tipos de valor que define la especificación GGUF (códigos 0 a 12, sin contar el 9 de
/// array, que se representa aparte porque lleva tipo de elemento y longitud propios).
#[derive(Debug, Clone, Copy)]
pub(super) enum TipoValor {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    Bool,
    Cadena,
    Array,
    U64,
    I64,
    F64,
}

impl TipoValor {
    pub(super) fn desde_codigo(codigo: u32) -> Option<Self> {
        Some(match codigo {
            0 => Self::U8,
            1 => Self::I8,
            2 => Self::U16,
            3 => Self::I16,
            4 => Self::U32,
            5 => Self::I32,
            6 => Self::F32,
            7 => Self::Bool,
            8 => Self::Cadena,
            9 => Self::Array,
            10 => Self::U64,
            11 => Self::I64,
            12 => Self::F64,
            _ => return None,
        })
    }
}

/// Lo que resulta de consumir un valor de la cabecera, reducido a lo que interesa: cadena o
/// entero no negativo se pueden usar; flotante, booleano, entero negativo o array quedan
/// descartados, pero ya se han consumido del flujo los bytes exactos que les correspondían.
pub(super) enum ValorLeido {
    Cadena(String),
    Numero(u64),
    Otro,
}

/// Envoltorio sobre cualquier `Read` que cuenta los bytes consumidos y corta con un error al
/// superar `LIMITE_BYTES_METADATOS`, se guarde el valor (una cadena) o solo se descarte. Es la
/// defensa central: ningún tope individual necesita ser perfecto porque este total acaba
/// cortando mucho antes de acercarse a los gigabytes de los pesos de un modelo.
pub(super) struct LectorAcotado<R: Read> {
    interior: R,
    leidos: u64,
}

impl<R: Read> LectorAcotado<R> {
    pub(super) fn nuevo(interior: R) -> Self {
        Self {
            interior,
            leidos: 0,
        }
    }

    fn comprobar_presupuesto(&self, adicionales: u64, ruta: &Path) -> Resultado<()> {
        if self.leidos.saturating_add(adicionales) > LIMITE_BYTES_METADATOS {
            return Err(error_gguf(
                ruta,
                format!(
                    "la cabecera supera el límite de {LIMITE_BYTES_METADATOS} bytes admitidos para metadatos"
                ),
            ));
        }
        Ok(())
    }

    fn leer_exacto(&mut self, buf: &mut [u8], ruta: &Path) -> Resultado<()> {
        self.comprobar_presupuesto(buf.len() as u64, ruta)?;
        self.interior
            .read_exact(buf)
            .map_err(|causa| error_gguf(ruta, format!("fichero truncado o ilegible: {causa}")))?;
        self.leidos += buf.len() as u64;
        Ok(())
    }

    fn leer_u8(&mut self, ruta: &Path) -> Resultado<u8> {
        let mut b = [0u8; 1];
        self.leer_exacto(&mut b, ruta)?;
        Ok(b[0])
    }

    fn leer_u16(&mut self, ruta: &Path) -> Resultado<u16> {
        let mut b = [0u8; 2];
        self.leer_exacto(&mut b, ruta)?;
        Ok(u16::from_le_bytes(b))
    }

    pub(super) fn leer_u32(&mut self, ruta: &Path) -> Resultado<u32> {
        let mut b = [0u8; 4];
        self.leer_exacto(&mut b, ruta)?;
        Ok(u32::from_le_bytes(b))
    }

    pub(super) fn leer_u64(&mut self, ruta: &Path) -> Resultado<u64> {
        let mut b = [0u8; 8];
        self.leer_exacto(&mut b, ruta)?;
        Ok(u64::from_le_bytes(b))
    }

    fn leer_i8(&mut self, ruta: &Path) -> Resultado<i8> {
        Ok(self.leer_u8(ruta)? as i8)
    }

    fn leer_i16(&mut self, ruta: &Path) -> Resultado<i16> {
        Ok(self.leer_u16(ruta)? as i16)
    }

    fn leer_i32(&mut self, ruta: &Path) -> Resultado<i32> {
        Ok(self.leer_u32(ruta)? as i32)
    }

    fn leer_i64(&mut self, ruta: &Path) -> Resultado<i64> {
        Ok(self.leer_u64(ruta)? as i64)
    }

    fn leer_f32(&mut self, ruta: &Path) -> Resultado<f32> {
        Ok(f32::from_bits(self.leer_u32(ruta)?))
    }

    fn leer_f64(&mut self, ruta: &Path) -> Resultado<f64> {
        Ok(f64::from_bits(self.leer_u64(ruta)?))
    }

    /// Lee una cadena GGUF: longitud `u64` seguida de esos bytes en UTF-8, sin terminador nulo.
    pub(super) fn leer_cadena(&mut self, ruta: &Path) -> Resultado<String> {
        let longitud = self.leer_u64(ruta)?;
        if longitud > LIMITE_LONGITUD_CADENA {
            return Err(error_gguf(
                ruta,
                format!(
                    "una cadena de la cabecera dice medir {longitud} bytes, más del límite de {LIMITE_LONGITUD_CADENA}"
                ),
            ));
        }
        let mut bytes = vec![0u8; longitud as usize];
        self.leer_exacto(&mut bytes, ruta)?;
        String::from_utf8(bytes).map_err(|causa| {
            error_gguf(
                ruta,
                format!("una cadena de la cabecera no es UTF-8 válido: {causa}"),
            )
        })
    }

    /// Lee y consume un valor de tipo `tipo`, descartando su contenido si no es una cadena o un
    /// entero no negativo. `profundidad` cuenta los niveles de array anidado ya abiertos.
    pub(super) fn consumir_valor(
        &mut self,
        tipo: TipoValor,
        profundidad: u32,
        ruta: &Path,
    ) -> Resultado<ValorLeido> {
        Ok(match tipo {
            TipoValor::U8 => ValorLeido::Numero(self.leer_u8(ruta)? as u64),
            TipoValor::U16 => ValorLeido::Numero(self.leer_u16(ruta)? as u64),
            TipoValor::U32 => ValorLeido::Numero(self.leer_u32(ruta)? as u64),
            TipoValor::U64 => ValorLeido::Numero(self.leer_u64(ruta)?),
            TipoValor::I8 => numero_si_no_negativo(self.leer_i8(ruta)? as i64),
            TipoValor::I16 => numero_si_no_negativo(self.leer_i16(ruta)? as i64),
            TipoValor::I32 => numero_si_no_negativo(self.leer_i32(ruta)? as i64),
            TipoValor::I64 => numero_si_no_negativo(self.leer_i64(ruta)?),
            TipoValor::F32 => {
                self.leer_f32(ruta)?;
                ValorLeido::Otro
            }
            TipoValor::F64 => {
                self.leer_f64(ruta)?;
                ValorLeido::Otro
            }
            TipoValor::Bool => {
                self.leer_u8(ruta)?;
                ValorLeido::Otro
            }
            TipoValor::Cadena => ValorLeido::Cadena(self.leer_cadena(ruta)?),
            TipoValor::Array => {
                self.consumir_array(profundidad, ruta)?;
                ValorLeido::Otro
            }
        })
    }

    /// Lee un array completo (tipo de elemento + longitud + elementos) y descarta su contenido:
    /// a este lector no le interesa el valor de ningún array, solo saber saltárselo sin perder la
    /// posición de lo que viene después.
    fn consumir_array(&mut self, profundidad: u32, ruta: &Path) -> Resultado<()> {
        if profundidad >= LIMITE_PROFUNDIDAD_ARRAY {
            return Err(error_gguf(
                ruta,
                format!(
                    "arrays anidados más allá del límite de {LIMITE_PROFUNDIDAD_ARRAY} niveles"
                ),
            ));
        }
        let codigo_elemento = self.leer_u32(ruta)?;
        let tipo_elemento = TipoValor::desde_codigo(codigo_elemento).ok_or_else(|| {
            error_gguf(
                ruta,
                format!("tipo de elemento de array desconocido: código {codigo_elemento}"),
            )
        })?;
        let longitud = self.leer_u64(ruta)?;
        if longitud > LIMITE_ELEMENTOS_ARRAY {
            return Err(error_gguf(
                ruta,
                format!(
                    "un array de la cabecera dice tener {longitud} elementos, más del límite de {LIMITE_ELEMENTOS_ARRAY}"
                ),
            ));
        }
        for _ in 0..longitud {
            self.consumir_valor(tipo_elemento, profundidad + 1, ruta)?;
        }
        Ok(())
    }

    /// Bytes consumidos hasta ahora, que es la posición absoluta dentro del fichero porque este
    /// lector nunca salta hacia atrás.
    pub(super) fn posicion(&self) -> u64 {
        self.leidos
    }
}

fn numero_si_no_negativo(valor: i64) -> ValorLeido {
    match u64::try_from(valor) {
        Ok(numero) => ValorLeido::Numero(numero),
        Err(_) => ValorLeido::Otro,
    }
}
