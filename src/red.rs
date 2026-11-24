//! Lo común a los clientes HTTP de Programator (`ureq` 3): el motor, `/health`, Hugging Face y
//! la descarga de modelos. El aviso de versión no pasa por aquí: usa su biblioteca.

use ureq::config::ConfigBuilder;
use ureq::typestate::AgentScope;

/// La configuración de partida de todo agente HTTP de Programator.
///
/// Sin proxy, como con `ureq` 2, que solo lo usaba si se le pedía. La 3 toma por defecto el de
/// las variables de entorno (`Proxy::try_from_env()` en `Config::default`, `ureq-3.5.2/src/config.rs`),
/// y eso mandaría las llamadas a `127.0.0.1` del motor a un proxy del sistema o al proxy trampa del
/// banco.
pub(crate) fn configuracion() -> ConfigBuilder<AgentScope> {
    sin_proxy(ureq::Agent::config_builder())
}

fn sin_proxy(configuracion: ConfigBuilder<AgentScope>) -> ConfigBuilder<AgentScope> {
    configuracion.proxy(None)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_configuracion_de_partida_quita_cualquier_proxy() {
        let con_proxy = ureq::Agent::config_builder()
            .proxy(Some(ureq::Proxy::new("http://127.0.0.1:9").unwrap()));
        assert!(con_proxy.build().proxy().is_some());

        let con_proxy = ureq::Agent::config_builder()
            .proxy(Some(ureq::Proxy::new("http://127.0.0.1:9").unwrap()));
        assert!(sin_proxy(con_proxy).build().proxy().is_none());
        assert!(configuracion().build().proxy().is_none());
    }
}
