//! Lo que el bucle decide en cada vuelta, sin tocar disco, canal ni motor: con qué sondeo se
//! decide, si toca atender, qué se dice al empezar a escuchar y al atender, y cómo acaba la vuelta
//! según la pasada.

use super::latido::EstadoFinVuelta;
use programator::arnes::sesion::{self, DesenlacePasada};
use programator::arranque::{self, Sondeo};

/// Si la vuelta es la primera del bucle, que mira el canal aunque no haya cambiado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Vuelta {
    Primera,
    Siguiente,
}

/// Lo que queda por atender aunque el canal no cambie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pendiente {
    Nada,
    /// Hay encargos en la cola, detectados en una pasada anterior.
    EnCola,
    /// La vuelta anterior dejó algo para luego: el motor seguía cargando, o un buzón estaba a
    /// medio escribir.
    Aplazado,
}

/// Si la cola de encargos tiene alguno, según el disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Cola {
    Vacia,
    ConEncargos,
}

impl Cola {
    /// La cola que dice `hay_encargos_en_cola`.
    pub(super) fn segun(hay_encargos: bool) -> Self {
        if hay_encargos {
            Self::ConEncargos
        } else {
            Self::Vacia
        }
    }
}

impl Pendiente {
    /// Lo que queda para esta vuelta: lo aplazado en la anterior y, si no hay nada aplazado, lo
    /// que haya en la cola.
    pub(super) fn con_la_cola(self, cola: Cola) -> Self {
        match (self, cola) {
            (Self::Nada, Cola::ConEncargos) => Self::EnCola,
            (otro, _) => otro,
        }
    }
}

/// Lo que se dice al empezar a escuchar, según lo que hizo la foto inicial. `None` si la foto
/// falló: lo dirá la primera pasada al tomarla.
pub(super) fn mensaje_de_escucha(
    instantanea: Option<sesion::Instantanea>,
    intervalo: u64,
) -> Option<String> {
    match instantanea? {
        sesion::Instantanea::Tomada => Some(arranque::mensaje_listo_primer_arranque(intervalo)),
        sesion::Instantanea::YaHabia | sesion::Instantanea::SinRespuestasPrevias => {
            Some(arranque::mensaje_listo_reinicio(intervalo))
        }
    }
}

/// El sondeo con que se decide la vuelta: en la primera, un canal sin cambios cuenta como novedad,
/// porque «sin cambios» solo significa que no hay foto anterior con que comparar.
pub(super) fn sondeo_de_la_vuelta(sondeo: Sondeo, vuelta: Vuelta) -> Sondeo {
    if vuelta == Vuelta::Primera && sondeo == Sondeo::SinCambios {
        Sondeo::Novedades
    } else {
        sondeo
    }
}

/// La vuelta siguiente: sigue siendo «la primera» solo si esta no pudo leer el canal entero, y por
/// tanto no ha mirado lo que había al arrancar.
pub(super) fn vuelta_siguiente(vuelta: Vuelta, sondeo: Sondeo) -> Vuelta {
    if vuelta == Vuelta::Primera && sondeo == Sondeo::Ignorada {
        Vuelta::Primera
    } else {
        Vuelta::Siguiente
    }
}

/// ¿Toca llamar a `ejecutar_pasada` en esta vuelta del bucle?
///
/// **Por qué no basta con mirar si el canal cambió.** Cuando una pasada termina en
/// `MotorCargando`, el encargo no se ha atendido y el registro de lectura no avanza a propósito,
/// para que la pasada siguiente lo vuelva a encontrar. Pero si nadie escribe nada más en el canal,
/// no hay pasada siguiente: el bucle sondeaba, veía `SinCambios` y se dormía otra vez.
///
/// El resultado era que **el primer encargo de cada sesión se quedaba colgado por defecto**,
/// porque el motor tarda uno o dos minutos en cargar 13 GiB y siempre lo pillaba cargando. Nadie
/// recibía respuesta ni aviso, y solo se reanudaba si alguien volvía a tocar el canal a mano.
/// Detectado el 22/09/2026 en la primera ejecución real del ciclo.
pub(super) fn toca_atender(sondeo: Sondeo, pendiente: Pendiente) -> bool {
    match sondeo {
        Sondeo::Novedades => true,
        // Aquí está el arreglo: sin novedades se atiende igualmente si quedó trabajo esperando a
        // que el motor terminara de cargar.
        Sondeo::SinCambios => pendiente != Pendiente::Nada,
        // Una foto incompleta no vale para comparar, así que no se sabe si hay novedades. Si había
        // trabajo pendiente, se reintenta: el encargo ya estaba detectado antes de esta foto y no
        // depende de ella.
        Sondeo::Ignorada => pendiente != Pendiente::Nada,
    }
}

/// Lo que se dice al ponerse a atender: por novedades en el canal, por encargos que quedaban en
/// la cola o porque se retoma uno que esperaba a que el motor cargara.
pub(super) fn mensaje_al_atender(sondeo: Sondeo, pendiente: Pendiente) -> String {
    if sondeo == Sondeo::Novedades {
        arranque::mensaje_novedades_atendiendo()
    } else if pendiente == Pendiente::Aplazado {
        arranque::mensaje_se_retoma_lo_aplazado()
    } else {
        arranque::mensaje_atendiendo_pendientes()
    }
}

/// Cómo acaba la vuelta según la pasada: en error si alguna publicación falló, en reposo si no.
pub(super) fn estado_tras_la_pasada(desenlace: &DesenlacePasada) -> EstadoFinVuelta {
    match desenlace {
        DesenlacePasada::Atendidos(res) if res.fallidos > 0 => {
            EstadoFinVuelta::Error(arranque::mensaje_publicaciones_fallidas(res.fallidos))
        }
        _ => EstadoFinVuelta::Reposo,
    }
}

/// Si la vuelta siguiente tiene que pasar por el canal aunque no cambie: cuando el motor seguía
/// cargando, o un buzón se estaba escribiendo, no se atendió todo lo que había.
pub(super) fn pendiente_tras(desenlace: &DesenlacePasada) -> Pendiente {
    let aplazado = match desenlace {
        DesenlacePasada::MotorCargando | DesenlacePasada::BuzonAMedias => true,
        DesenlacePasada::Atendidos(resumen) => resumen.buzones_a_medias > 0,
        DesenlacePasada::PrimeraPasada | DesenlacePasada::SinEncargos => false,
    };
    if aplazado {
        Pendiente::Aplazado
    } else {
        Pendiente::Nada
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_vuelve_por_el_motor_cargando_y_por_un_buzon_a_medias() {
        assert_eq!(
            Pendiente::Aplazado,
            pendiente_tras(&DesenlacePasada::MotorCargando)
        );
        assert_eq!(
            Pendiente::Aplazado,
            pendiente_tras(&DesenlacePasada::BuzonAMedias)
        );
        assert_eq!(
            Pendiente::Nada,
            pendiente_tras(&DesenlacePasada::SinEncargos)
        );
        assert_eq!(
            Pendiente::Nada,
            pendiente_tras(&DesenlacePasada::PrimeraPasada)
        );
        let atendidos = |buzones_a_medias| {
            DesenlacePasada::Atendidos(sesion::ResumenPasada {
                atendidos: 1,
                buzones_a_medias,
                ..sesion::ResumenPasada::default()
            })
        };
        assert_eq!(Pendiente::Aplazado, pendiente_tras(&atendidos(1)));
        assert_eq!(Pendiente::Nada, pendiente_tras(&atendidos(0)));
    }

    #[test]
    fn una_primera_vuelta_que_no_pudo_leer_el_canal_no_cuenta() {
        assert!(vuelta_siguiente(Vuelta::Primera, Sondeo::Ignorada) == Vuelta::Primera);
        assert!(Vuelta::Siguiente == vuelta_siguiente(Vuelta::Primera, Sondeo::SinCambios));
        assert!(Vuelta::Siguiente == vuelta_siguiente(Vuelta::Primera, Sondeo::Novedades));
        assert!(Vuelta::Siguiente == vuelta_siguiente(Vuelta::Siguiente, Sondeo::Ignorada));
    }

    #[test]
    fn al_escuchar_se_dice_si_se_dio_por_visto_lo_anterior() {
        let tomada = mensaje_de_escucha(Some(sesion::Instantanea::Tomada), 30).unwrap();
        let sin = mensaje_de_escucha(Some(sesion::Instantanea::SinRespuestasPrevias), 30).unwrap();

        assert_eq!(tomada, arranque::mensaje_listo_primer_arranque(30));
        assert_eq!(sin, arranque::mensaje_listo_reinicio(30));
        assert_eq!(
            mensaje_de_escucha(Some(sesion::Instantanea::YaHabia), 30).unwrap(),
            sin
        );
        assert_eq!(mensaje_de_escucha(None, 30), None);
    }

    #[test]
    fn la_primera_vuelta_mira_el_canal_aunque_no_haya_cambiado() {
        assert_eq!(
            sondeo_de_la_vuelta(Sondeo::SinCambios, Vuelta::Primera),
            Sondeo::Novedades
        );
        assert_eq!(
            sondeo_de_la_vuelta(Sondeo::SinCambios, Vuelta::Siguiente),
            Sondeo::SinCambios
        );
        assert_eq!(
            sondeo_de_la_vuelta(Sondeo::Ignorada, Vuelta::Primera),
            Sondeo::Ignorada
        );
    }

    #[test]
    fn con_novedades_siempre_se_atiende() {
        assert!(toca_atender(Sondeo::Novedades, Pendiente::Nada));
        assert!(toca_atender(Sondeo::Novedades, Pendiente::Aplazado));
    }

    #[test]
    fn sin_novedades_ni_nada_pendiente_no_se_molesta_al_modelo() {
        assert!(!toca_atender(Sondeo::SinCambios, Pendiente::Nada));
    }

    #[test]
    fn un_encargo_aplazado_porque_el_motor_cargaba_se_retoma_sin_tocar_el_canal() {
        // El defecto que esto cierra: el motor tarda uno o dos minutos en cargar el modelo, así
        // que el primer encargo de cada sesión lo pilla cargando. Sin esto, se quedaba colgado
        // hasta que una persona volvía a escribir en el canal.
        assert!(
            toca_atender(Sondeo::SinCambios, Pendiente::Aplazado),
            "el encargo sigue sin atender y el motor ya debería estar listo"
        );
    }

    #[test]
    fn una_foto_incompleta_no_pierde_el_encargo_pendiente() {
        assert!(toca_atender(Sondeo::Ignorada, Pendiente::Aplazado));
        assert!(!toca_atender(Sondeo::Ignorada, Pendiente::Nada));
    }

    #[test]
    fn lo_aplazado_manda_sobre_la_cola_y_la_cola_sobre_nada() {
        assert_eq!(
            Pendiente::Nada.con_la_cola(Cola::ConEncargos),
            Pendiente::EnCola
        );
        assert_eq!(Pendiente::Nada.con_la_cola(Cola::Vacia), Pendiente::Nada);
        assert_eq!(
            Pendiente::Aplazado.con_la_cola(Cola::ConEncargos),
            Pendiente::Aplazado
        );
        assert_eq!(Cola::segun(true), Cola::ConEncargos);
        assert_eq!(Cola::segun(false), Cola::Vacia);
        assert!(toca_atender(Sondeo::SinCambios, Pendiente::EnCola));
        assert_eq!(
            mensaje_al_atender(Sondeo::SinCambios, Pendiente::EnCola),
            arranque::mensaje_atendiendo_pendientes()
        );
    }

    #[test]
    fn al_atender_se_dice_por_que_se_atiende() {
        assert_eq!(
            mensaje_al_atender(Sondeo::Novedades, Pendiente::Aplazado),
            arranque::mensaje_novedades_atendiendo()
        );
        assert_eq!(
            mensaje_al_atender(Sondeo::SinCambios, Pendiente::Nada),
            arranque::mensaje_atendiendo_pendientes()
        );
        assert_eq!(
            mensaje_al_atender(Sondeo::Ignorada, Pendiente::Aplazado),
            arranque::mensaje_se_retoma_lo_aplazado()
        );
    }

    #[test]
    fn la_vuelta_acaba_en_error_solo_si_alguna_publicacion_fallo() {
        let atendidos = |fallidos| {
            DesenlacePasada::Atendidos(sesion::ResumenPasada {
                atendidos: 2,
                fallidos,
                ..Default::default()
            })
        };
        assert!(matches!(
            estado_tras_la_pasada(&atendidos(1)),
            EstadoFinVuelta::Error(detalle) if detalle == arranque::mensaje_publicaciones_fallidas(1)
        ));
        assert!(matches!(
            estado_tras_la_pasada(&atendidos(0)),
            EstadoFinVuelta::Reposo
        ));
        assert!(matches!(
            estado_tras_la_pasada(&DesenlacePasada::MotorCargando),
            EstadoFinVuelta::Reposo
        ));
    }

    #[test]
    fn primera_pasada_utiliza_mensaje_sin_jerga_y_con_cadencia() {
        let mensaje = arranque::mensaje_listo_primer_arranque(180);
        assert!(mensaje.contains("punto de partida sin atender encargos anteriores"));
        assert!(mensaje.contains("Programator ya está escuchando"));
        assert!(mensaje.contains("cada 3 minutos"));
        assert!(!mensaje.contains("registro vacío"));
        assert!(!mensaje.contains("primera pasada"));
    }
}
