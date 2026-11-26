//! Los motores del ciclo: qué modelo atiende cada encargo y cómo se prepara su `llama-server`.
//!
//! Junta las piezas que ya existen por separado: la línea `Clase:` del encargo
//! (`arnes::clase_pedida`), la elección por el registro de modelos (`revision::eleccion`) y la
//! preparación del motor (`motores::preparar`). La pasada solo ve el rasgo [`Motores`].
//!
//! Lo que se mide (registro, tarjeta, ficheros de la carpeta de modelos) se lee una vez al crear
//! [`MotoresDelCiclo`], al principio de cada pasada; lo que se decide con esas medidas está en
//! funciones puras, probadas aparte.

pub mod preparar;

use crate::arnes::clase_pedida::{clase_pedida, mensaje_clase_desconocida, ClasePedida};
use crate::arnes::espacio::ruta_del_encaje_historico;
use crate::arnes::sesion::{Espera, EstadoMotor, ModeloPedido, Motores, Preparacion};
use crate::config::Config;
use crate::error::Resultado;
use crate::motor::cabida::{cabida, por_que_no_se_puede_usar, FicheroDelModelo};
use crate::motor::encaje::encaje_en_tarjeta;
use crate::motor::encaje_historico::{self, Memoria};
use crate::motor::fichero_de_modelo;
use crate::motor::gguf;
use crate::motor::hardware::describir_gpu;
use crate::motor::proceso::{hay_servidor, EstadoServidor, ServidorLocal};
use crate::motores::preparar::{
    asegurar_motor_para, esperar_motor_para, medida_registrada, PedidoDeMotor,
};
use crate::protocolo::Encargo;
use crate::registro::{mensajes, modelo};
use crate::reloj::RelojReal;
use crate::revision::eleccion::{elegir_modelo, Eleccion, Motivo};
use crate::revision::proponer::ficheros_con_tamano;
use crate::terminal::Terminal;
use crate::unidades::BYTES_POR_MIB;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Los motores de una pasada del ciclo, con el servidor propio que vive mientras dura el programa.
pub struct MotoresDelCiclo<'a> {
    config: &'a Config,
    servidor: &'a RefCell<Option<ServidorLocal>>,
    registro: modelo::Registro,
    tarjeta: Option<String>,
    en_modelos: Vec<(String, u64)>,
    /// Dónde se anota la línea de memoria de cada encargo.
    historial: PathBuf,
    /// Por dónde se avisa de lo que no impide atender: un registro ilegible, una memoria sin anotar.
    terminal: &'a dyn Terminal,
}

impl<'a> MotoresDelCiclo<'a> {
    /// Lee el registro, mide la tarjeta y lista la carpeta de modelos. Nada de esto impide
    /// atender: sin registro o sin carpeta, todo va al modelo por defecto.
    pub fn nuevo(
        config: &'a Config,
        servidor: &'a RefCell<Option<ServidorLocal>>,
        carpeta: &Path,
        terminal: &'a dyn Terminal,
    ) -> Self {
        let registro = modelo::leer(&config.resolver(&config.agente.registro_de_modelos))
            .unwrap_or_else(|fallo| {
                terminal.avisar(&mensajes::mensaje_registro_ilegible(&fallo));
                modelo::Registro::default()
            });
        let en_modelos = ficheros_con_tamano(&config.resolver(&config.revision.carpeta_de_modelos))
            .unwrap_or_default();
        Self {
            config,
            servidor,
            registro,
            tarjeta: describir_gpu().map(|gpu| gpu.nombre),
            en_modelos,
            historial: ruta_del_encaje_historico(carpeta, &config.agente.nombre),
            terminal,
        }
    }
}

impl Motores for MotoresDelCiclo<'_> {
    fn modelo_para(&self, encargo: &Encargo) -> ModeloPedido {
        let clase = clase_pedida(&encargo.texto);
        let eleccion = elegir_modelo(
            &self.registro,
            &clase,
            self.tarjeta.as_deref(),
            &self.en_modelos,
        );
        let carpeta = self
            .config
            .resolver(&self.config.revision.carpeta_de_modelos);
        let por_defecto = self
            .config
            .motor
            .modelo
            .as_deref()
            .map(|m| ruta_del_modelo(&self.config.resolver(m)));
        modelo_pedido(
            &eleccion,
            &clase,
            |nombre| {
                fichero_del_modelo(&self.en_modelos, nombre)
                    .map(|fichero| ruta_del_modelo(&carpeta.join(fichero)))
            },
            por_defecto,
        )
    }

    /// Solo si el proceso sigue vivo: uno que se cerró no tiene nada cargado.
    fn cargado(&self) -> Option<String> {
        self.servidor
            .borrow()
            .as_ref()
            .filter(|s| s.sigue_vivo())
            .map(|s| ruta_del_modelo(s.modelo()))
    }

    fn preparar(&mut self, modelo: Option<&str>, espera: Espera) -> Resultado<Preparacion> {
        let config = self.config;
        // Antes de lanzar nada: si el modelo pedido falta o no cabe, no se intenta. El que ya está
        // cargado no se vuelve a pesar, y con un servidor ajeno respondiendo se usa ese, tenga el
        // modelo que tenga.
        let ajeno_listo = || {
            self.servidor.borrow().is_none()
                && matches!(
                    hay_servidor(
                        config.motor.puerto,
                        crate::motor::clave::vigente(config).as_ref()
                    ),
                    EstadoServidor::Listo
                )
        };
        let por_pesar = modelo.filter(|m| self.cargado().as_deref() != Some(*m));
        if let Some(ruta) = por_pesar.filter(|_| !ajeno_listo()) {
            let ruta = Path::new(ruta);
            let cabida =
                || describir_gpu().map(|gpu| cabida(&encaje_en_tarjeta(&config.motor, ruta, &gpu)));
            let (fichero, cabida) = if ruta.is_file() {
                (FicheroDelModelo::Esta, cabida())
            } else {
                (FicheroDelModelo::Falta, None)
            };
            if let Some(motivo) = por_que_no_se_puede_usar(ruta, fichero, cabida.as_ref()) {
                return Ok(Preparacion::SinModelo(motivo));
            }
        }
        let pedido = PedidoDeMotor {
            modelo,
            muestreo: &config.muestreo,
            medida: &|nombre, tarjeta| medida_registrada(config, nombre, tarjeta, self.terminal),
            terminal: self.terminal,
        };
        let mut servidor = self.servidor.borrow_mut();
        let preparado = if espera == Espera::HastaElTope {
            let tope = Duration::from_secs(config.motor.espera_de_carga_segundos);
            let reloj = &mut RelojReal::desde_ahora();
            match esperar_motor_para(config, &pedido, &mut servidor, tope, reloj)? {
                Some(preparado) => preparado,
                None => return Ok(Preparacion::TiempoAgotado),
            }
        } else {
            asegurar_motor_para(config, &pedido, &mut servidor)?
        };
        Ok(match preparado.estado {
            EstadoMotor::Disponible(motor) => Preparacion::Listo {
                motor,
                procedencia: preparado.procedencia,
            },
            EstadoMotor::Esperando => Preparacion::Cargando,
        })
    }

    fn tras_el_encargo(&mut self) -> Option<String> {
        let memoria = {
            let servidor = self.servidor.borrow();
            let propio = servidor.as_ref()?;
            let modelo = nombre_del_fichero(propio.modelo());
            Memoria {
                capas: propio.capas_gpu(),
                capas_medidas: self
                    .tarjeta
                    .as_deref()
                    .and_then(|tarjeta| self.registro.medida_para(&modelo, tarjeta))
                    .map(|medida| medida.capas),
                capas_totales: capas_del_modelo(propio.modelo()),
                libre_antes_mib: propio.vram_libre_antes().map(|b| b / BYTES_POR_MIB),
                motor_mib: propio.vram_ocupada_mib(),
                libre_despues_mib: describir_gpu().map(|gpu| gpu.vram_libre / BYTES_POR_MIB),
                modelo,
            }
        };
        let linea = encaje_historico::linea_de_memoria(&memoria);
        let marca = crate::protocolo::marca_de_tiempo_actual();
        if let Err(fallo) =
            encaje_historico::registrar(&self.historial, &format!("{marca} · {linea}"))
        {
            self.terminal
                .avisar(&encaje_historico::mensaje_fallo_registrar(
                    &self.historial,
                    &fallo,
                ));
        }
        Some(match encaje_historico::aviso_menos_capas(&memoria) {
            Some(aviso) => format!("{linea}\n\n{aviso}"),
            None => linea,
        })
    }
}

/// La ruta de un modelo como texto, con un solo separador: la pasada compara estos textos para
/// agrupar los encargos y para saber si su modelo es el cargado, y en Windows
/// `modelos/devstral.gguf` (del TOML) y `modelos\devstral.gguf` (de la carpeta) son el mismo.
fn ruta_del_modelo(ruta: &Path) -> String {
    ruta.components().collect::<PathBuf>().display().to_string()
}

/// El nombre del modelo tal como lo conoce el registro: el fichero sin carpeta ni extensión.
fn nombre_del_fichero(ruta: &Path) -> String {
    fichero_de_modelo::nombre_de_modelo(ruta).unwrap_or_else(|| ruta.display().to_string())
}

/// Las capas del modelo, de su cabecera GGUF. Si no se deja leer, no se sabe.
fn capas_del_modelo(ruta: &Path) -> Option<u32> {
    gguf::leer_modelo(ruta)
        .ok()?
        .metadatos
        .numero_capas
        .and_then(|capas| u32::try_from(capas).ok())
}

/// El fichero de la carpeta de modelos cuyo nombre sin extensión es `nombre`.
fn fichero_del_modelo<'m>(en_modelos: &'m [(String, u64)], nombre: &str) -> Option<&'m str> {
    en_modelos
        .iter()
        .map(|(fichero, _)| fichero.as_str())
        .find(|fichero| {
            fichero_de_modelo::nombre_de_modelo(Path::new(fichero)).is_some_and(|s| s == nombre)
        })
}

/// Lo que se pide para un encargo a partir de la elección: el fichero del ganador o, si no lo
/// hay (o no se encuentra), el modelo por defecto; el aviso de una clase que no existe; y si pidió
/// una clase válida.
fn modelo_pedido(
    eleccion: &Eleccion,
    clase: &ClasePedida,
    ruta_de: impl Fn(&str) -> Option<String>,
    por_defecto: Option<String>,
) -> ModeloPedido {
    let modelo = eleccion.modelo.as_deref().and_then(ruta_de).or(por_defecto);
    let aviso = match &eleccion.motivo {
        Motivo::ClaseDesconocida(escrita) => Some(mensaje_clase_desconocida(escrita)),
        _ => None,
    };
    ModeloPedido {
        modelo,
        aviso,
        por_clase: matches!(clase, ClasePedida::Valida(_)),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::FilaDeDesempeno;

    const FILA: FilaDeDesempeno = FilaDeDesempeno::Diagnostico;

    fn ganador(nombre: &str) -> Eleccion {
        Eleccion {
            modelo: Some(nombre.to_string()),
            motivo: Motivo::Ganador {
                fila: FILA,
                nota: 5.0,
            },
        }
    }

    fn ruta(nombre: &str) -> Option<String> {
        Some(format!("modelos/{nombre}.gguf"))
    }

    #[test]
    fn el_modelo_del_toml_y_el_de_la_carpeta_son_la_misma_ruta() {
        let raiz = Path::new("portable");
        assert_eq!(
            ruta_del_modelo(&raiz.join("modelos/devstral.gguf")),
            ruta_del_modelo(&raiz.join("modelos").join("devstral.gguf"))
        );
        assert_eq!(
            ruta_del_modelo(&raiz.join("./modelos/devstral.gguf")),
            ruta_del_modelo(&raiz.join("modelos").join("devstral.gguf"))
        );
    }

    #[test]
    fn el_nombre_del_modelo_es_su_fichero_sin_carpeta_ni_extension() {
        assert_eq!(
            nombre_del_fichero(Path::new("modelos/Devstral-Q4.gguf")),
            "Devstral-Q4"
        );
    }

    #[test]
    fn sin_fichero_legible_no_se_saben_las_capas_del_modelo() {
        assert_eq!(capas_del_modelo(Path::new("no/existe.gguf")), None);
    }

    #[test]
    fn el_fichero_del_modelo_se_busca_por_su_nombre_sin_extension() {
        let en_modelos = vec![("a.gguf".to_string(), 1), ("b-Q4.gguf".to_string(), 2)];
        assert_eq!(fichero_del_modelo(&en_modelos, "b-Q4"), Some("b-Q4.gguf"));
        assert_eq!(fichero_del_modelo(&en_modelos, "b"), None);
    }

    #[test]
    fn con_ganador_se_pide_su_fichero_y_se_marca_que_vino_por_clase() {
        let pedido = modelo_pedido(
            &ganador("rapido"),
            &ClasePedida::Valida(FILA),
            ruta,
            Some("defecto".into()),
        );
        assert_eq!(pedido.modelo.as_deref(), Some("modelos/rapido.gguf"));
        assert!(pedido.por_clase);
        assert_eq!(pedido.aviso, None);
    }

    #[test]
    fn si_el_fichero_del_ganador_no_aparece_se_usa_el_de_por_defecto() {
        let pedido = modelo_pedido(
            &ganador("rapido"),
            &ClasePedida::Valida(FILA),
            |_| None,
            Some("defecto".into()),
        );
        assert_eq!(pedido.modelo.as_deref(), Some("defecto"));
    }

    #[test]
    fn una_clase_desconocida_va_al_de_por_defecto_con_su_aviso() {
        let eleccion = Eleccion {
            modelo: None,
            motivo: Motivo::ClaseDesconocida("astrologia".into()),
        };
        let pedido = modelo_pedido(
            &eleccion,
            &ClasePedida::Desconocida("astrologia".into()),
            ruta,
            Some("defecto".into()),
        );
        assert_eq!(pedido.modelo.as_deref(), Some("defecto"));
        assert!(!pedido.por_clase);
        assert!(pedido.aviso.unwrap().contains("astrologia"));
    }

    #[test]
    fn sin_clase_no_hay_aviso_ni_marca_de_clase() {
        let eleccion = Eleccion {
            modelo: None,
            motivo: Motivo::SinClase,
        };
        let pedido = modelo_pedido(
            &eleccion,
            &ClasePedida::Ninguna,
            ruta,
            Some("defecto".into()),
        );
        assert_eq!(pedido.modelo.as_deref(), Some("defecto"));
        assert!(!pedido.por_clase);
        assert_eq!(pedido.aviso, None);
    }
}
