//! Cuándo puede el motor reutilizar la caché de prefijos de `llama-server`.
//!
//! **Por qué existe.** `llama-server` guarda en el slot lo último que calculó. Ante una petición
//! nueva, reaprovecha el comienzo que tenga en común y solo calcula el resto. Calcular los mismos
//! tokens en lotes de otro tamaño suma los mismos números en otro orden, y la respuesta varía en los
//! últimos decimales. Con temperatura, eso basta para que la conversación tome otro camino.
//!
//! Medido el 12/11/2026: el mismo
//! encargo, con la misma semilla, dio tres conversaciones distintas según lo que se había pedido
//! antes. Con la primera petición sin caché, dio siempre la misma.
//!
//! **La regla.** La caché solo se reutiliza cuando la petición **continúa** la anterior de este
//! mismo cliente, es decir, cuando es aquella con turnos nuevos detrás. En cualquier otro caso va en
//! frío: un encargo nuevo, un reintento con la misma conversación, una conversación recortada o la
//! petición que sigue a un fallo. Así, lo que hay en la caché depende solo de la conversación en
//! curso.
//!
//! **Límite.** La regla presupone que este cliente es el único que usa el servidor
//! (`--parallel 1`). Si otro programa le pide algo entre dos turnos, el slot ya no tiene lo que esta
//! pieza cree.

use super::Mensaje;

/// Qué hacer con la caché de prefijos en una petición.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cache {
    /// La petición continúa la anterior: se aprovecha lo que el servidor ya calculó.
    Reutilizar,
    /// Se calcula todo desde cero, sin depender de lo que hubiera antes en el slot.
    EnFrio,
}

impl Cache {
    /// El valor de la clave `cache_prompt` de `llama-server`.
    pub fn reutiliza(self) -> bool {
        self == Cache::Reutilizar
    }
}

/// Lo que el cliente sabe de su última petición, y la decisión que se toma con ello.
#[derive(Debug, Clone)]
pub struct Continuidad {
    /// `[motor] empezar_en_frio`. Con `false`, la caché se reutiliza siempre, como antes del
    /// 12/11/2026.
    empezar_en_frio: bool,
    /// La última conversación que se mandó y salió bien. `None` al empezar y tras un fallo.
    ultima: Option<Vec<Mensaje>>,
}

impl Continuidad {
    pub fn nueva(empezar_en_frio: bool) -> Self {
        Self {
            empezar_en_frio,
            ultima: None,
        }
    }

    /// Qué hacer con la caché para mandar `conversacion`.
    pub fn cache_para(&self, conversacion: &[Mensaje]) -> Cache {
        if !self.empezar_en_frio {
            return Cache::Reutilizar;
        }
        match &self.ultima {
            Some(anterior) if continua(anterior, conversacion) => Cache::Reutilizar,
            _ => Cache::EnFrio,
        }
    }

    /// La petición salió bien: el slot del servidor tiene ahora esta conversación.
    pub fn recordar(&mut self, conversacion: &[Mensaje]) {
        self.ultima = Some(conversacion.to_vec());
    }

    /// La petición falló: no se sabe qué quedó en el slot, y la siguiente va en frío.
    pub fn olvidar(&mut self) {
        self.ultima = None;
    }
}

/// `nueva` es `anterior` con al menos un turno más detrás.
fn continua(anterior: &[Mensaje], nueva: &[Mensaje]) -> bool {
    nueva.len() > anterior.len() && nueva.starts_with(anterior)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::Papel;

    fn conversacion(textos: &[&str]) -> Vec<Mensaje> {
        textos
            .iter()
            .map(|t| Mensaje::nuevo(Papel::Usuario, *t))
            .collect()
    }

    fn tras(anterior: &[&str]) -> Continuidad {
        let mut continuidad = Continuidad::nueva(true);
        continuidad.recordar(&conversacion(anterior));
        continuidad
    }

    #[test]
    fn la_primera_peticion_va_en_frio() {
        let continuidad = Continuidad::nueva(true);
        assert_eq!(
            continuidad.cache_para(&conversacion(&["normas", "encargo"])),
            Cache::EnFrio
        );
    }

    #[test]
    fn una_continuacion_reutiliza_la_cache() {
        let continuidad = tras(&["normas", "encargo"]);
        assert_eq!(
            continuidad.cache_para(&conversacion(&[
                "normas",
                "encargo",
                "respuesta",
                "resultado"
            ])),
            Cache::Reutilizar
        );
    }

    #[test]
    fn la_misma_conversacion_repetida_va_en_frio() {
        // Es el caso que divergió en la investigación: con todo el texto ya en el slot, el
        // servidor solo recalcula el último token.
        let continuidad = tras(&["normas", "encargo"]);
        assert_eq!(
            continuidad.cache_para(&conversacion(&["normas", "encargo"])),
            Cache::EnFrio
        );
    }

    #[test]
    fn otro_encargo_con_las_mismas_normas_va_en_frio() {
        let continuidad = tras(&["normas", "encargo uno", "respuesta"]);
        assert_eq!(
            continuidad.cache_para(&conversacion(&["normas", "encargo dos"])),
            Cache::EnFrio
        );
    }

    #[test]
    fn una_conversacion_recortada_va_en_frio() {
        let continuidad = tras(&["normas", "a", "b", "c"]);
        assert_eq!(
            continuidad.cache_para(&conversacion(&["normas", "c", "d"])),
            Cache::EnFrio
        );
    }

    #[test]
    fn tras_un_fallo_la_siguiente_va_en_frio() {
        let mut continuidad = tras(&["normas", "encargo"]);
        continuidad.olvidar();
        assert_eq!(
            continuidad.cache_para(&conversacion(&["normas", "encargo", "respuesta"])),
            Cache::EnFrio
        );
    }

    #[test]
    fn sin_empezar_en_frio_se_reutiliza_siempre() {
        let continuidad = Continuidad::nueva(false);
        assert_eq!(
            continuidad.cache_para(&conversacion(&["normas", "encargo"])),
            Cache::Reutilizar
        );
    }

    #[test]
    fn la_clave_del_servidor_dice_si_se_reutiliza() {
        assert!(Cache::Reutilizar.reutiliza());
        assert!(!Cache::EnFrio.reutiliza());
    }
}
