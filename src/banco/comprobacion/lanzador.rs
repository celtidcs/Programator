//! Cómo se lanza cada orden de la comprobación local en este equipo.
//!
//! La carpeta local decide qué se ejecuta y dónde; el lanzador, cómo se crea el proceso. Así la
//! carpeta se prueba con un lanzador de mentira, sin crear procesos.
//!
//! Con `aislamiento = "integridad_baja"`, en Windows, cada orden va a través del ayudante
//! (`ayudante_aislado`): Programator se lanza a sí mismo con `--lanzar-aislado`, lo mete en un Job
//! y solo entonces le da la señal de arrancar. Mientras la orden corre, se mira cada poco si algún
//! proceso del Job tiene una conexión que sale del equipo.

use std::path::Path;
use std::time::Duration;

use crate::banco::config::{Aislamiento, ComprobacionLocal};
use crate::error::Resultado;
use crate::orden_acotada::{ejecutar_con_tope_y_entorno, DesenlaceDeOrden};

/// Lanza una orden en un directorio, con variables de entorno añadidas.
pub trait LanzadorConEntorno {
    /// Deja `carpeta` lista para que las órdenes trabajen en ella. Se llama una vez, recién creada
    /// y antes de la primera orden.
    fn preparar_carpeta(&mut self, _carpeta: &Path) -> Resultado<()> {
        Ok(())
    }

    fn lanzar(
        &mut self,
        argumentos: &[String],
        directorio: &Path,
        entorno: &[(String, String)],
        tope: Duration,
    ) -> DesenlaceDeOrden;

    /// Los destinos fuera del equipo a los que se vio conectarse a la última orden, y lo que
    /// impidió vigilarla, si algo lo impidió.
    fn vigilancia_de_la_ultima(&mut self) -> Vigilancia {
        Vigilancia::default()
    }
}

/// Lo que se vio de la red mientras corría una orden.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vigilancia {
    pub intentos_de_red: Vec<String>,
    /// Por qué no se pudo mirar, la primera vez que falló.
    pub fallo: Option<String>,
}

/// Los límites de una orden aislada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Topes {
    pub procesos_maximos: u32,
    pub memoria_maxima_mib: u64,
    pub vigilar_red_cada: Duration,
}

/// Cómo se crea cada proceso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormaDeLanzar {
    /// La orden tal cual. El envoltorio de Linux, si lo hay, ya va en los argumentos.
    Directa,
    /// Windows: con integridad baja y dentro de un Job con estos topes.
    Aislada(Topes),
}

impl FormaDeLanzar {
    /// La que pide la configuración.
    pub fn para(config: &ComprobacionLocal) -> Self {
        match config.aislamiento {
            Aislamiento::IntegridadBaja => Self::Aislada(Topes {
                procesos_maximos: config.procesos_maximos,
                memoria_maxima_mib: config.memoria_maxima_mib,
                vigilar_red_cada: Duration::from_millis(config.vigilar_red_cada_ms),
            }),
            Aislamiento::EspacioDeNombres | Aislamiento::Ninguno => Self::Directa,
        }
    }
}

/// Lanza de verdad, sin shell de por medio salvo el intérprete configurado.
pub struct LanzadorLocal {
    forma: FormaDeLanzar,
    ultima: Vigilancia,
}

impl LanzadorLocal {
    pub fn nuevo(forma: FormaDeLanzar) -> Self {
        Self {
            forma,
            ultima: Vigilancia::default(),
        }
    }

    /// El que pide la configuración.
    pub fn para(config: &ComprobacionLocal) -> Self {
        Self::nuevo(FormaDeLanzar::para(config))
    }
}

impl LanzadorConEntorno for LanzadorLocal {
    fn preparar_carpeta(&mut self, carpeta: &Path) -> Resultado<()> {
        match self.forma {
            FormaDeLanzar::Directa => Ok(()),
            FormaDeLanzar::Aislada(_) => aislado::marcar(carpeta),
        }
    }

    fn lanzar(
        &mut self,
        argumentos: &[String],
        directorio: &Path,
        entorno: &[(String, String)],
        tope: Duration,
    ) -> DesenlaceDeOrden {
        let Some((programa, resto)) = argumentos.split_first() else {
            return DesenlaceDeOrden::NoSeLanzo("la orden está vacía".into());
        };
        match self.forma {
            FormaDeLanzar::Directa => {
                ejecutar_con_tope_y_entorno(programa, resto, directorio, tope, entorno)
            }
            FormaDeLanzar::Aislada(topes) => {
                let (desenlace, vigilancia) =
                    aislado::lanzar(argumentos, directorio, entorno, tope, topes);
                self.ultima = vigilancia;
                desenlace
            }
        }
    }

    fn vigilancia_de_la_ultima(&mut self) -> Vigilancia {
        std::mem::take(&mut self.ultima)
    }
}

/// La orden a través del ayudante, en un Job.
#[cfg(windows)]
mod aislado {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::net::SocketAddr;
    use std::path::Path;
    use std::process::Child;
    use std::time::{Duration, Instant};

    use super::{Topes, Vigilancia};
    use crate::aislamiento_windows::{
        marcar_carpeta_de_integridad_baja, nombre_del_proceso, tabla_de_conexiones, Trabajo,
    };
    use crate::argumentos::orden_del_ayudante;
    use crate::ayudante_aislado::SENAL_DE_ARRANQUE;
    use crate::banco::comprobacion::red::{de_los_procesos, intentos_de_red};
    use crate::error::Resultado;
    use crate::orden_acotada::{
        ejecutar_con_tope_entorno_y_contencion, Contencion, DesenlaceDeOrden,
    };

    pub fn marcar(carpeta: &Path) -> Resultado<()> {
        marcar_carpeta_de_integridad_baja(carpeta)
    }

    pub fn lanzar(
        argumentos: &[String],
        directorio: &Path,
        entorno: &[(String, String)],
        tope: Duration,
        topes: Topes,
    ) -> (DesenlaceDeOrden, Vigilancia) {
        let ejecutable = match std::env::current_exe() {
            Ok(ruta) => ruta.display().to_string(),
            Err(causa) => {
                let motivo = format!("no se sabe dónde está el propio Programator: {causa}");
                return (DesenlaceDeOrden::NoSeLanzo(motivo), Vigilancia::default());
            }
        };
        let trabajo = match Trabajo::nuevo(topes.procesos_maximos, topes.memoria_maxima_mib) {
            Ok(trabajo) => trabajo,
            Err(fallo) => {
                return (
                    DesenlaceDeOrden::NoSeLanzo(fallo.to_string()),
                    Vigilancia::default(),
                )
            }
        };
        let orden = orden_del_ayudante(&ejecutable, argumentos);
        let mut contencion = EnUnTrabajo::nuevo(trabajo, topes.vigilar_red_cada);
        let desenlace = ejecutar_con_tope_entorno_y_contencion(
            &orden[0],
            &orden[1..],
            directorio,
            tope,
            entorno,
            &mut contencion,
        );
        (desenlace, contencion.vigilancia())
    }

    /// Mete al ayudante en el Job antes de dejarle arrancar; mientras espera, mira las conexiones
    /// de los procesos del Job; y lo corta entero al vencer el plazo y también al terminar, para
    /// que no quede nada vivo de la orden. Al soltarse, el Job se cierra y mata lo que quedara.
    struct EnUnTrabajo {
        trabajo: Trabajo,
        vigilar_cada: Duration,
        ultima_mirada: Instant,
        /// Cada destino visto, con el programa que lo abrió si se supo.
        remotas: Vec<(SocketAddr, Option<String>)>,
        /// El programa de cada proceso ya preguntado, para no preguntarlo en cada mirada.
        programas: BTreeMap<u32, Option<String>>,
        fallo: Option<String>,
    }

    impl EnUnTrabajo {
        fn nuevo(trabajo: Trabajo, vigilar_cada: Duration) -> Self {
            Self {
                trabajo,
                vigilar_cada,
                ultima_mirada: Instant::now(),
                remotas: Vec::new(),
                programas: BTreeMap::new(),
                fallo: None,
            }
        }

        fn mirar_la_red(&mut self) {
            self.ultima_mirada = Instant::now();
            let vistas = self.trabajo.procesos().and_then(|antes| {
                let tabla = tabla_de_conexiones()?;
                let despues = self.trabajo.procesos()?;
                Ok(de_los_procesos(&tabla, &antes, &despues))
            });
            match vistas {
                Ok(vistas) => {
                    for (remota, proceso) in vistas {
                        let programa = self
                            .programas
                            .entry(proceso)
                            .or_insert_with(|| nombre_del_proceso(proceso))
                            .clone();
                        self.remotas.push((remota, programa));
                    }
                }
                Err(fallo) => {
                    self.fallo.get_or_insert(fallo.to_string());
                }
            }
        }

        fn vigilancia(self) -> Vigilancia {
            Vigilancia {
                intentos_de_red: intentos_de_red(&self.remotas),
                fallo: self.fallo,
            }
        }
    }

    impl Contencion for EnUnTrabajo {
        fn tras_lanzar(&mut self, hijo: &mut Child) -> Result<(), String> {
            self.trabajo
                .meter(hijo)
                .map_err(|fallo| fallo.to_string())?;
            let Some(mut entrada) = hijo.stdin.take() else {
                return Err("el ayudante no tiene entrada por donde darle la señal".into());
            };
            // Al soltar `entrada` se cierra, y lo que lance el ayudante lee el final al momento.
            entrada
                .write_all(&[SENAL_DE_ARRANQUE])
                .map_err(|causa| format!("no se pudo dar la señal al ayudante: {causa}"))
        }

        fn mientras_espera(&mut self) {
            if self.ultima_mirada.elapsed() >= self.vigilar_cada {
                self.mirar_la_red();
            }
        }

        fn cortar(&mut self, hijo: &mut Child) {
            self.trabajo.cortar();
            let _ = hijo.kill();
        }

        fn al_terminar(&mut self) {
            // Una última mirada antes de cortar: lo que quede vivo puede tener algo abierto.
            self.mirar_la_red();
            self.trabajo.cortar();
        }
    }
}

/// Fuera de Windows no hay integridad baja: la configuración la rechaza antes de llegar aquí.
#[cfg(not(windows))]
mod aislado {
    use std::path::Path;
    use std::time::Duration;

    use super::{Topes, Vigilancia};
    use crate::error::{Error, Resultado};
    use crate::orden_acotada::DesenlaceDeOrden;

    const SOLO_EN_WINDOWS: &str = "la integridad baja solo existe en Windows";

    pub fn marcar(_carpeta: &Path) -> Resultado<()> {
        Err(Error::Aislamiento(SOLO_EN_WINDOWS.into()))
    }

    pub fn lanzar(
        _argumentos: &[String],
        _directorio: &Path,
        _entorno: &[(String, String)],
        _tope: Duration,
        _topes: Topes,
    ) -> (DesenlaceDeOrden, Vigilancia) {
        (
            DesenlaceDeOrden::NoSeLanzo(SOLO_EN_WINDOWS.into()),
            Vigilancia::default(),
        )
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_integridad_baja_lanza_aislado_con_los_topes_y_lo_demas_directo() {
        let config = ComprobacionLocal {
            aislamiento: Aislamiento::IntegridadBaja,
            procesos_maximos: 8,
            memoria_maxima_mib: 512,
            vigilar_red_cada_ms: 250,
            ..ComprobacionLocal::default()
        };
        assert_eq!(
            FormaDeLanzar::para(&config),
            FormaDeLanzar::Aislada(Topes {
                procesos_maximos: 8,
                memoria_maxima_mib: 512,
                vigilar_red_cada: Duration::from_millis(250),
            })
        );
        for aislamiento in [Aislamiento::EspacioDeNombres, Aislamiento::Ninguno] {
            let config = ComprobacionLocal {
                aislamiento,
                ..config.clone()
            };
            assert_eq!(FormaDeLanzar::para(&config), FormaDeLanzar::Directa);
        }
    }

    /// Con el lanzador real y una orden inocua, como las pruebas de `orden_acotada`.
    #[test]
    fn el_lanzador_local_pasa_las_variables_a_la_orden() {
        let dir = tempfile::tempdir().unwrap();
        let orden = crate::plataforma::ordenes_de_prueba::eco_de_variable("PROGRAMATOR_LOCAL");
        let mut lanzador = LanzadorLocal::nuevo(FormaDeLanzar::Directa);

        let desenlace = lanzador.lanzar(
            &orden,
            dir.path(),
            &[("PROGRAMATOR_LOCAL".into(), "desde-la-tanda".into())],
            Duration::from_secs(30),
        );

        match desenlace {
            DesenlaceDeOrden::Terminada { exito, salida, .. } => {
                assert!(exito);
                assert!(salida.contains("desde-la-tanda"), "{salida}");
            }
            otro => panic!("{otro:?}"),
        }
        assert_eq!(lanzador.vigilancia_de_la_ultima(), Vigilancia::default());
    }
}
