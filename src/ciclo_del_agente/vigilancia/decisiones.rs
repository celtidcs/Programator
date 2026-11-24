//! Lo que el bucle decide en cada vuelta, sin tocar disco, canal ni motor: con qué sondeo se
//! decide, si toca atender, qué se dice al empezar a escuchar y al atender, y cómo acaba la vuelta
//! según la pasada.

use super::latido::EstadoFinVuelta;
use programator::arnes::sesion::{self, DesenlacePasada};
use programator::arranque::{self, Sondeo};

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
pub(super) fn sondeo_de_la_vuelta(sondeo: Sondeo, primera_vuelta: bool) -> Sondeo {
    if primera_vuelta && sondeo == Sondeo::SinCambios {
        Sondeo::Novedades
    } else {
        sondeo
    }
}

/// Si la vuelta siguiente sigue siendo «la primera»: solo si esta no pudo leer el canal entero, y
/// por tanto no ha mirado lo que había al arrancar.
pub(super) fn sigue_siendo_la_primera(primera_vuelta: bool, sondeo: Sondeo) -> bool {
    primera_vuelta && sondeo == Sondeo::Ignorada
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
pub(super) fn toca_atender(sondeo: Sondeo, pendiente_por_motor: bool) -> bool {
    match sondeo {
        Sondeo::Novedades => true,
        // Aquí está el arreglo: sin novedades se atiende igualmente si quedó trabajo esperando a
        // que el motor terminara de cargar.
        Sondeo::SinCambios => pendiente_por_motor,
        // Una foto incompleta no vale para comparar, así que no se sabe si hay novedades. Si había
        // trabajo pendiente, se reintenta: el encargo ya estaba detectado antes de esta foto y no
        // depende de ella.
        Sondeo::Ignorada => pendiente_por_motor,
    }
}

/// Lo que se dice al ponerse a atender: por novedades en el canal, por encargos que quedaban en
/// la cola o porque se retoma uno que esperaba a que el motor cargara.
pub(super) fn mensaje_al_atender(sondeo: Sondeo, pendiente_por_motor: bool) -> String {
    if sondeo == Sondeo::Novedades {
        arranque::mensaje_novedades_atendiendo()
    } else if !pendiente_por_motor {
        arranque::mensaje_atendiendo_pendientes()
    } else {
        arranque::mensaje_reanudacion_encargo_esperando_motor()
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

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_primera_vuelta_que_no_pudo_leer_el_canal_no_cuenta() {
        assert!(sigue_siendo_la_primera(true, Sondeo::Ignorada));
        assert!(!sigue_siendo_la_primera(true, Sondeo::SinCambios));
        assert!(!sigue_siendo_la_primera(true, Sondeo::Novedades));
        assert!(!sigue_siendo_la_primera(false, Sondeo::Ignorada));
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
            sondeo_de_la_vuelta(Sondeo::SinCambios, true),
            Sondeo::Novedades
        );
        assert_eq!(
            sondeo_de_la_vuelta(Sondeo::SinCambios, false),
            Sondeo::SinCambios
        );
        assert_eq!(
            sondeo_de_la_vuelta(Sondeo::Ignorada, true),
            Sondeo::Ignorada
        );
    }

    #[test]
    fn con_novedades_siempre_se_atiende() {
        assert!(toca_atender(Sondeo::Novedades, false));
        assert!(toca_atender(Sondeo::Novedades, true));
    }

    #[test]
    fn sin_novedades_ni_nada_pendiente_no_se_molesta_al_modelo() {
        assert!(!toca_atender(Sondeo::SinCambios, false));
    }

    #[test]
    fn un_encargo_aplazado_porque_el_motor_cargaba_se_retoma_sin_tocar_el_canal() {
        // El defecto que esto cierra: el motor tarda uno o dos minutos en cargar el modelo, así
        // que el primer encargo de cada sesión lo pilla cargando. Sin esto, se quedaba colgado
        // hasta que una persona volvía a escribir en el canal.
        assert!(
            toca_atender(Sondeo::SinCambios, true),
            "el encargo sigue sin atender y el motor ya debería estar listo"
        );
    }

    #[test]
    fn una_foto_incompleta_no_pierde_el_encargo_pendiente() {
        assert!(toca_atender(Sondeo::Ignorada, true));
        assert!(!toca_atender(Sondeo::Ignorada, false));
    }

    #[test]
    fn al_atender_se_dice_por_que_se_atiende() {
        assert_eq!(
            mensaje_al_atender(Sondeo::Novedades, true),
            arranque::mensaje_novedades_atendiendo()
        );
        assert_eq!(
            mensaje_al_atender(Sondeo::SinCambios, false),
            arranque::mensaje_atendiendo_pendientes()
        );
        assert_eq!(
            mensaje_al_atender(Sondeo::Ignorada, true),
            arranque::mensaje_reanudacion_encargo_esperando_motor()
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
