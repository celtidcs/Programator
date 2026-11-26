//! Los encargos pendientes del agente, vistos desde el bucle: solo si tiene trabajo.

use programator::arnes::{espacio, registro_de_encargos::disco};
use std::path::Path;

/// Si hay encargos sin respuesta en el registro de encargos, o una cola de la 1.1.0 que la pasada
/// todavía no ha migrado. Una ficha que no se deja leer cuenta como trabajo: así la vuelta pasa por
/// el canal, la pasada la aparta y avisa, y el modelo no se suelta por un fallo de lectura.
pub(super) fn hay_encargos_en_cola(carpeta: &Path, agente: &str) -> bool {
    espacio::ruta_de_los_pendientes(carpeta, agente).exists()
        || disco::hay_abiertos(&espacio::carpeta_de_los_encargos(carpeta, agente))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    use programator::arnes::pendientes;
    use programator::arnes::registro_de_encargos::{EstadoDelEncargo, RegistroDeEncargos};

    #[test]
    fn una_ficha_pendiente_cuenta_como_trabajo_y_una_atendida_no() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        assert!(!hay_encargos_en_cola(
            carpeta_temporal.path(),
            "Programator"
        ));

        let encargo = programator::protocolo::Encargo {
            latido: None,
            de: "claude.md".into(),
            texto: "Para Programator: algo".into(),
            forma: programator::protocolo::encargo::Forma::Linea,
        };
        let carpeta = espacio::carpeta_de_los_encargos(carpeta_temporal.path(), "Programator");
        let mut registro = RegistroDeEncargos::default();
        registro.anotar(&encargo, "f", EstadoDelEncargo::Atendido);
        disco::guardar_las_cambiadas(&carpeta, &mut registro).unwrap();
        assert!(!hay_encargos_en_cola(
            carpeta_temporal.path(),
            "Programator"
        ));

        registro.anotar_fallo(&encargo, "f", "no cargó", 3);
        disco::guardar_las_cambiadas(&carpeta, &mut registro).unwrap();
        assert!(hay_encargos_en_cola(carpeta_temporal.path(), "Programator"));
    }

    /// Una cola de la 1.1.0 que la pasada aún no ha migrado cuenta como trabajo.
    #[test]
    fn la_cola_de_la_1_1_sin_migrar_cuenta_como_trabajo() {
        let carpeta = tempfile::tempdir().unwrap();

        let mut cola = pendientes::Cola::default();
        cola.anotar_fallo(
            &programator::protocolo::Encargo {
                latido: None,
                de: "claude.md".into(),
                texto: "Para Programator: algo".into(),
                forma: programator::protocolo::encargo::Forma::Linea,
            },
            "2026-11-22 14:00",
            "no cargó",
            3,
        );
        let ruta = espacio::ruta_de_los_pendientes(carpeta.path(), "Programator");
        pendientes::guardar(&ruta, &cola).unwrap();
        assert!(hay_encargos_en_cola(carpeta.path(), "Programator"));
    }

    #[test]
    fn una_cola_ilegible_cuenta_como_trabajo() {
        let carpeta = tempfile::tempdir().unwrap();
        let ruta = espacio::ruta_de_los_pendientes(carpeta.path(), "Programator");
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        std::fs::write(&ruta, "esto no es [toml").unwrap();
        assert!(hay_encargos_en_cola(carpeta.path(), "Programator"));
    }
}
