//! Lo que el motor le declara al modelo de cada herramienta: nombre, para qué sirve y la forma de
//! sus argumentos.
//!
//! **Por qué existe este módulo.** Antes `motor/llama.rs` buscaba cada nombre en `arnes::ficha`:
//! la capa de abajo leía la de arriba. Ahora el arnés compone estas declaraciones desde sus fichas
//! y se las da hechas a `MotorLlama`; el motor solo las traduce al formato de la API.

/// Un argumento declarado, con el nombre **exacto** que el modelo tiene que escribir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parametro {
    pub nombre: &'static str,
    /// Tipo JSON schema de la propiedad ("string", "integer").
    pub tipo: &'static str,
    pub para_que: &'static str,
}

/// Una herramienta tal y como se le anuncia al modelo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaracion {
    pub nombre: &'static str,
    pub para_que: &'static str,
    pub obligatorios: Vec<Parametro>,
    pub opcionales: Vec<Parametro>,
}
