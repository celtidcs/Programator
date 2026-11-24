//! Lo que el bucle saca de la configuración: las instrucciones, el recordatorio, los límites del
//! ciclo y la ruta del latido, compuestos juntos para que no se mezclen configuraciones.

use programator::arnes::ciclo::Limites;
use programator::arnes::espacio;
use programator::config::Config;
use std::path::{Path, PathBuf};

/// Lo que el bucle saca de la configuración. Se compone entero cada vez que se lee: así no puede
/// quedar la configuración nueva con algo de la vieja.
pub(super) struct DerivadosDeLaConfig {
    /// Las instrucciones que ve el modelo en cada encargo.
    pub(super) preambulo: PathBuf,
    /// Lo que se le recuerda pegado al encargo.
    pub(super) recordatorio: PathBuf,
    /// Los límites del ciclo, con los valores por defecto de `config.rs`.
    pub(super) limites: Limites,
    /// Dónde se escribe el latido.
    pub(super) ruta_latido: PathBuf,
}

impl DerivadosDeLaConfig {
    pub(super) fn de(config: &Config, carpeta: &Path) -> Self {
        Self {
            preambulo: config.resolver(&config.agente.preambulo),
            recordatorio: config.resolver(&config.agente.recordatorio),
            limites: Limites::de_la_config(&config.ciclo),
            ruta_latido: espacio::ruta_del_latido(
                carpeta,
                &config.agente.nombre,
                &config.ciclo.fichero_latido,
            ),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Tras releer la configuración, todo lo que sale de ella es de la nueva.
    #[test]
    fn los_derivados_salen_de_la_configuracion_que_se_les_da() {
        let dir = tempfile::tempdir().unwrap();
        let cargar = |contenido: &str| {
            let ruta = dir.path().join("programator.toml");
            std::fs::write(&ruta, contenido).unwrap();
            Config::cargar(&ruta).unwrap()
        };
        let vieja = cargar("[motor]\nmodelo = \"x.gguf\"\n");
        let nueva = cargar(
            "[motor]\nmodelo = \"y.gguf\"\n\n[agente]\npreambulo = \"otro.md\"\n\
             recordatorio = \"otro-recordatorio.md\"\n\n[ciclo]\n\
             max_herramientas_por_encargo = 3\nfichero_latido = \"otro.json\"\n",
        );

        let antes = DerivadosDeLaConfig::de(&vieja, dir.path());
        let despues = DerivadosDeLaConfig::de(&nueva, dir.path());

        assert!(despues.preambulo.ends_with("otro.md"));
        assert!(despues.recordatorio.ends_with("otro-recordatorio.md"));
        assert_eq!(despues.limites.max_herramientas, 3);
        assert!(despues.ruta_latido.ends_with("otro.json"));
        assert_ne!(antes.preambulo, despues.preambulo);
        assert_ne!(antes.ruta_latido, despues.ruta_latido);
    }
}
