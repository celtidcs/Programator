//! El latido de fin de vuelta: cómo acabó la vuelta y su registro en disco antes de dormir.

use programator::protocolo::Latido;
use programator::terminal::TerminalReal;
use std::path::Path;

/// Estado con el que concluye la vuelta de sondeo antes de entrar en reposo.
pub(super) enum EstadoFinVuelta {
    Reposo,
    Error(String),
}

/// Registra en disco el latido correspondiente al desenlace de la vuelta de sondeo.
pub(super) fn registrar_latido_fin_vuelta(
    ruta_latido: &Path,
    intervalo_segundos: u64,
    estado: &EstadoFinVuelta,
    modelo_cargado: Option<String>,
) {
    let latido = match estado {
        EstadoFinVuelta::Reposo => Latido::en_reposo(chrono::Local::now(), intervalo_segundos),
        EstadoFinVuelta::Error(detalle) => {
            Latido::en_error(chrono::Local::now(), intervalo_segundos, None, detalle)
        }
    };
    latido
        .con_modelo_cargado(modelo_cargado)
        .guardar_con_aviso(ruta_latido, &TerminalReal);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use programator::protocolo::EstadoLatido;

    #[test]
    fn registrar_latido_fin_vuelta_guarda_reposo_y_error() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("latido.json");

        registrar_latido_fin_vuelta(&ruta, 180, &EstadoFinVuelta::Reposo, None);
        let latido_reposo = Latido::cargar(&ruta).unwrap().expect("existe");
        assert_eq!(latido_reposo.estado(), EstadoLatido::Reposo);
        assert!(latido_reposo.encargo().is_none());
        assert_eq!(latido_reposo.modelo_cargado(), None);

        registrar_latido_fin_vuelta(
            &ruta,
            180,
            &EstadoFinVuelta::Error("fallo de prueba".to_string()),
            Some("devstral".to_string()),
        );
        let latido_error = Latido::cargar(&ruta).unwrap().expect("existe");
        assert_eq!(latido_error.estado(), EstadoLatido::Error);
        assert_eq!(latido_error.modelo_cargado(), Some("devstral"));
        assert_eq!(latido_error.detalle_error(), Some("fallo de prueba"));
    }
}
