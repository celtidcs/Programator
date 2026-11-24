//! Programator: arnés que convierte un modelo local en un agente del canal `.gestor/canal/`.

#[cfg(windows)]
pub mod aislamiento_windows;
pub mod argumentos;
pub mod arnes;
pub mod arranque;
pub mod aviso_de_version;
pub mod ayudante_aislado;
pub mod banco;
pub mod carpeta;
pub mod concordancia;
pub mod config;
pub mod diagnostico;
pub mod error;
pub(crate) mod ficheros;
pub mod informe;
pub mod motor;
pub mod motores;
pub mod orden_acotada;
pub mod plataforma;
pub mod protocolo;
pub mod red;
pub mod registro;
pub mod reloj;
pub mod revision;
pub mod terminal;
pub mod unidades;
