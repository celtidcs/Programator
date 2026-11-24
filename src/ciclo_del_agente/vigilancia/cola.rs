//! La cola de pendientes del agente, vista desde el bucle: solo si tiene trabajo.

use programator::arnes::{espacio, pendientes};
use std::path::Path;

/// Si hay encargos en la cola de pendientes. Una cola que no se deja leer cuenta como con trabajo:
/// así la vuelta pasa por el canal, la pasada la vuelve a leer y avisa del error, y el modelo no se
/// suelta por un fallo de lectura.
pub(super) fn hay_encargos_en_cola(carpeta: &Path, agente: &str) -> bool {
    pendientes::leer(&espacio::ruta_de_los_pendientes(carpeta, agente))
        .map(|cola| !cola.esta_vacia())
        .unwrap_or(true)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_cola_de_pendientes_cuenta_como_trabajo_solo_si_tiene_algo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!hay_encargos_en_cola(dir.path(), "Programator"));

        let mut cola = pendientes::Cola::default();
        cola.anotar_fallo(
            &programator::protocolo::Encargo {
                de: "claude.md".into(),
                texto: "Para Programator: algo".into(),
                forma: programator::protocolo::encargo::Forma::Linea,
            },
            "2026-11-22 14:00",
            "no cargó",
            3,
        );
        let ruta = espacio::ruta_de_los_pendientes(dir.path(), "Programator");
        pendientes::guardar(&ruta, &cola).unwrap();
        assert!(hay_encargos_en_cola(dir.path(), "Programator"));
    }

    #[test]
    fn una_cola_ilegible_cuenta_como_trabajo() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = espacio::ruta_de_los_pendientes(dir.path(), "Programator");
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        std::fs::write(&ruta, "esto no es [toml").unwrap();
        assert!(hay_encargos_en_cola(dir.path(), "Programator"));
    }
}
