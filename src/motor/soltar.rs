//! Cuándo se suelta el modelo cargado para dejar libre la tarjeta (`[motor] soltar_modelo`).
//!
//! Decisión pura: los minutos sin encargos entran por parámetro, sin reloj dentro, y el bucle de
//! `main` es quien mide el tiempo y suelta el servidor.

use crate::config::SoltarModelo;

/// Lo que importa del motor para decidir si se suelta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotorParaSoltar {
    /// No hay servidor propio. Un servidor ajeno nunca se suelta: no es de Programator.
    SinPropio,
    /// Hay propio, pero está cargando o quedan encargos en la cola: soltarlo ahora sería tirar
    /// la carga que se va a necesitar en la vuelta siguiente.
    Ocupado,
    /// Hay propio y no tiene trabajo esperando.
    Libre,
}

/// Lo que se sabe del motor propio. Con nombre, para que no se puedan cruzar como dos `bool`
/// seguidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotorPropio {
    /// Si Programator tiene un servidor propio lanzado.
    pub hay: bool,
    /// Si está cargando o quedan encargos en la cola.
    pub ocupado: bool,
}

/// Lo que importa del motor, a partir de si hay propio y de si está cargando o con cola.
pub fn motor_para_soltar(propio: MotorPropio) -> MotorParaSoltar {
    match (propio.hay, propio.ocupado) {
        (false, _) => MotorParaSoltar::SinPropio,
        (true, true) => MotorParaSoltar::Ocupado,
        (true, false) => MotorParaSoltar::Libre,
    }
}

/// Si toca soltar el modelo: solo un propio libre, y cuando lleva sin encargos los minutos de la
/// política o más. Con `0`, en cuanto termina.
pub fn toca_soltar(
    politica: SoltarModelo,
    minutos_sin_encargos: u64,
    motor: MotorParaSoltar,
) -> bool {
    motor == MotorParaSoltar::Libre && minutos_sin_encargos >= u64::from(politica.minutos())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::config::soltar_modelo::MINUTOS_SIN_ENCARGOS_PARA_SOLTAR;

    #[test]
    fn en_auto_se_suelta_a_los_15_minutos_y_no_antes() {
        assert_eq!(MINUTOS_SIN_ENCARGOS_PARA_SOLTAR, 15);
        let auto = SoltarModelo::Auto;
        assert!(!toca_soltar(auto, 14, MotorParaSoltar::Libre));
        assert!(toca_soltar(auto, 15, MotorParaSoltar::Libre));
    }

    #[test]
    fn con_cero_se_suelta_al_terminar() {
        assert!(toca_soltar(
            SoltarModelo::TrasMinutos(0),
            0,
            MotorParaSoltar::Libre
        ));
    }

    #[test]
    fn con_treinta_minutos_se_espera_a_los_treinta() {
        let treinta = SoltarModelo::TrasMinutos(30);
        assert!(!toca_soltar(treinta, 29, MotorParaSoltar::Libre));
        assert!(toca_soltar(treinta, 30, MotorParaSoltar::Libre));
    }

    #[test]
    fn sin_servidor_propio_nunca_se_suelta_nada() {
        assert!(!toca_soltar(
            SoltarModelo::TrasMinutos(0),
            1000,
            MotorParaSoltar::SinPropio
        ));
    }

    #[test]
    fn el_motor_es_libre_solo_si_hay_propio_y_no_esta_ocupado() {
        assert_eq!(
            motor_para_soltar(MotorPropio {
                hay: false,
                ocupado: false
            }),
            MotorParaSoltar::SinPropio
        );
        assert_eq!(
            motor_para_soltar(MotorPropio {
                hay: false,
                ocupado: true
            }),
            MotorParaSoltar::SinPropio
        );
        assert_eq!(
            motor_para_soltar(MotorPropio {
                hay: true,
                ocupado: true
            }),
            MotorParaSoltar::Ocupado
        );
        assert_eq!(
            motor_para_soltar(MotorPropio {
                hay: true,
                ocupado: false
            }),
            MotorParaSoltar::Libre
        );
    }

    #[test]
    fn un_motor_que_carga_o_tiene_cola_no_se_suelta_ni_con_cero() {
        assert!(!toca_soltar(
            SoltarModelo::TrasMinutos(0),
            1000,
            MotorParaSoltar::Ocupado
        ));
    }
}
