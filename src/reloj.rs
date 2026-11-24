//! El tiempo, inyectable: lo que mide, espera o pone plazos lo pide a un [`Reloj`] que recibe de
//! fuera, para que las pruebas usen uno de mentira y no esperen ni dependan de la hora real.
//!
//! **Por qué vive aquí y no en `banco::ejecucion`.** Lo usan el banco, la espera de carga del
//! motor (`motores::preparar`) y la tanda, y `motores` no debe depender del banco.

use std::time::{Duration, Instant};

use crate::arnes::ciclo::Plazo;

/// El tiempo, inyectable para probar las esperas sin esperar.
pub trait Reloj {
    /// Lo que ha pasado desde que se creó el reloj. Solo valen las diferencias entre lecturas.
    fn transcurrido(&self) -> Duration;
    /// Deja pasar `cuanto` antes de volver.
    fn esperar(&mut self, cuanto: Duration);
    /// Un plazo que vence cuando hayan pasado `cuanto` desde ahora en este mismo reloj. `None` si
    /// esa hora no se puede representar: entonces no hay plazo.
    fn plazo_dentro_de(&self, cuanto: Duration) -> Option<Plazo>;
}

/// El reloj de verdad.
pub struct RelojReal {
    inicio: Instant,
}

impl RelojReal {
    pub fn desde_ahora() -> Self {
        Self {
            inicio: Instant::now(),
        }
    }
}

impl Reloj for RelojReal {
    fn transcurrido(&self) -> Duration {
        self.inicio.elapsed()
    }
    fn esperar(&mut self, cuanto: Duration) {
        std::thread::sleep(cuanto);
    }
    fn plazo_dentro_de(&self, cuanto: Duration) -> Option<Plazo> {
        Instant::now().checked_add(cuanto).map(Plazo::hasta)
    }
}

/// Un reloj que solo avanza cuando se le dice: al esperar o con [`RelojDeMentira::adelantar`].
/// Sus copias comparten la hora, para que un doble (un motor que tarda, por ejemplo) la adelante
/// mientras otro la lee. Solo existe al compilar las pruebas.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct RelojDeMentira {
    ahora: std::sync::Arc<std::sync::Mutex<Duration>>,
    esperas: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

#[cfg(test)]
impl RelojDeMentira {
    /// Un reloj que empieza en `ahora`, para comprobar que solo cuentan las diferencias.
    pub fn en(ahora: Duration) -> Self {
        let reloj = Self::default();
        reloj.adelantar(ahora);
        reloj
    }

    /// Hace pasar `cuanto` sin contarlo como espera.
    pub fn adelantar(&self, cuanto: Duration) {
        *self.ahora.lock().unwrap() += cuanto;
    }

    /// Cuántas veces se ha esperado.
    pub fn esperas(&self) -> usize {
        self.esperas.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
impl Reloj for RelojDeMentira {
    fn transcurrido(&self) -> Duration {
        *self.ahora.lock().unwrap()
    }
    fn esperar(&mut self, cuanto: Duration) {
        self.adelantar(cuanto);
        self.esperas
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn plazo_dentro_de(&self, cuanto: Duration) -> Option<Plazo> {
        let limite = self.transcurrido().checked_add(cuanto)?;
        let reloj = self.clone();
        Some(Plazo::con_reloj(move || reloj.transcurrido() >= limite))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_plazo_del_reloj_de_mentira_vence_al_llegar_a_su_hora_y_no_antes() {
        let reloj = RelojDeMentira::en(Duration::from_secs(10));
        let plazo = reloj.plazo_dentro_de(Duration::from_secs(5)).unwrap();

        reloj.adelantar(Duration::from_secs(4));
        assert!(!plazo.vencido());
        reloj.adelantar(Duration::from_secs(1));
        assert!(plazo.vencido());
    }

    #[test]
    fn el_plazo_del_reloj_de_verdad_sin_margen_ya_esta_vencido_y_uno_lejano_no() {
        let reloj = RelojReal::desde_ahora();
        assert!(reloj.plazo_dentro_de(Duration::ZERO).unwrap().vencido());
        assert!(!reloj
            .plazo_dentro_de(Duration::from_secs(3600))
            .unwrap()
            .vencido());
    }

    #[test]
    fn un_plazo_que_no_se_puede_representar_no_es_plazo() {
        assert!(RelojReal::desde_ahora()
            .plazo_dentro_de(Duration::MAX)
            .is_none());
        assert!(RelojDeMentira::en(Duration::from_secs(1))
            .plazo_dentro_de(Duration::MAX)
            .is_none());
    }
}
