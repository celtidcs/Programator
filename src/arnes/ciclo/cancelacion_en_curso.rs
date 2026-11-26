//! Si alguien ha cancelado el encargo que se está atendiendo.
//!
//! El ciclo es de un solo hilo y una llamada al motor puede durar minutos: no se puede cortar a
//! mitad de una. Lo que sí se puede es preguntar antes de cada llamada, en el mismo punto que el
//! plazo ([`super::Plazo`]). Quien atiende el encargo pone aquí la comprobación (en producción,
//! releer el buzón de quien lo encargó); el ciclo solo pregunta. Es una función, y no la lectura
//! del canal directamente, para que las pruebas digan cuándo llega la cancelación.

use std::sync::Arc;

/// La pregunta que el ciclo hace antes de cada llamada al motor.
#[derive(Clone)]
pub struct CancelacionEnCurso {
    pedida_por: Arc<dyn Fn() -> Option<String> + Send + Sync>,
}

impl CancelacionEnCurso {
    /// Una comprobación que devuelve quién canceló el encargo, si alguien lo hizo.
    pub fn con(pedida_por: impl Fn() -> Option<String> + Send + Sync + 'static) -> Self {
        Self {
            pedida_por: Arc::new(pedida_por),
        }
    }

    /// Quién ha pedido cancelar el encargo, si alguien lo ha pedido ya.
    pub fn pedida_por(&self) -> Option<String> {
        (self.pedida_por)()
    }
}

impl std::fmt::Debug for CancelacionEnCurso {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelacionEnCurso").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn devuelve_lo_que_diga_la_comprobacion() {
        assert_eq!(CancelacionEnCurso::con(|| None).pedida_por(), None);
        assert_eq!(
            CancelacionEnCurso::con(|| Some("Claude 1".to_string())).pedida_por(),
            Some("Claude 1".to_string())
        );
    }
}
