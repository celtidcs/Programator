//! El banco de modelos: quince encargos fijos que se pasan a cualquier modelo a través del ciclo
//! real de Programator y se puntúan siempre de la misma manera, para poder comparar modelos.
//!
//! Los encargos, con su esquema, están en `evaluacion/banco/encargos/`.

pub mod comparacion;
pub mod comparar;
pub mod comprobacion;
pub mod config;
pub mod ejecucion;
pub mod ficha;
pub mod lugar;
pub mod mensajes;
pub mod pasadas;
pub mod puntuacion;
pub mod rastro;
pub mod resultados;
pub mod rubrica;
pub mod tanda;
