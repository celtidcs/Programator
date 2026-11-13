//! El plazo de un encargo: la hora a partir de la cual el ciclo no vuelve a llamar al motor.
//!
//! **Por qué existe.** En la tanda del banco del 11/11/2026 el tiempo máximo por encargo era de
//! 900 s y hubo pasadas de 1327 s: el banco solo miraba el reloj al terminar la pasada, y el ciclo
//! seguía mientras le quedaran herramientas. En uso normal no hay plazo y nada cambia.
//!
//! Se pregunta a una función, y no a `Instant::now()` directamente, para que las pruebas usen un
//! reloj de mentira.

use std::sync::Arc;
use std::time::Instant;

/// Cuándo se acaba el tiempo de un encargo.
#[derive(Clone)]
pub struct Plazo {
    vencido: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl Plazo {
    /// Un plazo que vence a la hora `limite` del reloj de verdad.
    pub fn hasta(limite: Instant) -> Self {
        Self::con_reloj(move || Instant::now() >= limite)
    }

    /// Un plazo que vence cuando `vencido` lo diga.
    pub fn con_reloj(vencido: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self {
            vencido: Arc::new(vencido),
        }
    }

    pub fn vencido(&self) -> bool {
        (self.vencido)()
    }
}

impl std::fmt::Debug for Plazo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plazo")
            .field("vencido", &self.vencido())
            .finish()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::time::Duration;

    #[test]
    fn un_plazo_ya_pasado_esta_vencido_y_uno_lejano_no() {
        assert!(Plazo::hasta(Instant::now()).vencido());
        let lejano = Instant::now() + Duration::from_secs(3600);
        assert!(!Plazo::hasta(lejano).vencido());
    }
}
