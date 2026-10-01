//! Programator: arnés que convierte un modelo local en un agente del canal.

use programator::argumentos::{self, Orden};
use programator::arnes::ciclo::Limites;
use programator::arnes::sesion::{self, DesenlacePasada, EstadoMotor, ResumenPasada};
use programator::arnes::{espacio, normas};
use programator::arranque::{self, hay_novedades, instalar_normas, publicar_guia, Sondeo};
use programator::carpeta;
use programator::config::Config;
use programator::diagnostico::reunir_datos_de_arranque;
use programator::error::{Error, Resultado};
use programator::informe::{componer_informe, DatosDeArranque};
use programator::motor::preparar::asegurar_motor_local;
use programator::motor::proceso;
use programator::protocolo::Latido;
use std::path::{Path, PathBuf};

/// La versión del paquete, para no escribirla a mano en dos sitios.
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let orden = match argumentos::interpretar(&argumentos) {
        Orden::Ayuda => {
            // `print!`, no `println!`: `texto_de_ayuda` ya termina en salto de línea, y
            // `println!` añadiría uno más, dejando una línea en blanco de sobra al final.
            print!("{}", argumentos::texto_de_ayuda(VERSION));
            return;
        }
        Orden::Version => {
            println!("{}", arranque::mensaje_version(VERSION));
            return;
        }
        Orden::NoEntendido(cual) => {
            eprintln!("{}", arranque::mensaje_argumento_no_entendido(&cual));
            std::process::exit(2);
        }
        Orden::FaltaValor(cual) => {
            eprintln!("{}", arranque::mensaje_argumento_falta_valor(&cual));
            std::process::exit(2);
        }
        orden @ (Orden::Diagnostico | Orden::Ejecutar { .. }) => orden,
    };

    if let Err(fallo) = ejecutar(orden) {
        eprintln!("{}", arranque::mensaje_fallo_arranque(&fallo));
        std::process::exit(1);
    }
}

fn ejecutar(orden: Orden) -> Resultado<()> {
    let ruta_config = directorio_del_ejecutable().join("programator.toml");
    let config = Config::cargar(&ruta_config)?;

    // Una sola vez, al arrancar el programa y antes de entrar en el bucle: si Programator llega a
    // arrancar su propio `llama-server`, esto es lo que lo mata al recibir Ctrl+C en vez de
    // dejarlo huérfano con el modelo cargado en VRAM.
    proceso::registrar_cierre_ordenado()?;

    // El informe se compone **antes** de resolver la carpeta de trabajo, y a propósito no la
    // necesita resuelta: `--diagnostico` existe justo para poder preguntar «¿esto cómo está hoy?»
    // sin dejar un proceso colgado, que es lo que pasaba cuando el diálogo de carpeta se abría
    // primero. Si `[carpeta] ruta` no está fijada, la línea lo dice y ya; los otros cuatro datos
    // no dependen de la carpeta para nada. Va por `stderr`, como el resto de los avisos desde la
    // 0.4.0.
    let datos = reunir_datos_de_arranque(&config);
    // Capturado antes de que `componer_informe` se lleve `datos.modelo` por valor: lo necesita el
    // registro del historial de encaje, más abajo, una vez se conoce la carpeta de trabajo.
    let capas_totales_modelo = match &datos.modelo {
        programator::informe::ResumenModelo::Pesado { capas, .. } => {
            capas.and_then(|c| u32::try_from(c).ok())
        }
        _ => None,
    };
    let informe = componer_informe(&DatosDeArranque {
        version: VERSION,
        carpeta: config.carpeta.ruta.as_deref().map(Path::new),
        gpu: datos.gpu.as_ref(),
        motor: &datos.motor,
        ruta_motor: config.motor.binario.as_deref(),
        modelo: datos.modelo,
        encaje: &datos.encaje,
        preguntar_siempre: config.carpeta.preguntar_siempre,
    });
    eprintln!("{informe}");

    // Aviso, solo informativo, de si hay una versión más nueva en GitHub. Nunca puede impedir que
    // Programator arranque: sin red, con GitHub caído o fuera del tiempo de espera, simplemente no
    // hay nada que avisar. Se comprueba también en `--diagnostico`, que es justo para preguntar
    // «¿esto cómo está hoy?».
    if config.actualizaciones.comprobar {
        if let Some(aviso) = programator::actualizacion::comprobar_version_mas_reciente(
            &config.actualizaciones.repositorio,
            VERSION,
            std::time::Duration::from_secs(config.actualizaciones.tiempo_espera_segundos),
        ) {
            eprintln!("{aviso}");
        }
    }

    // `--diagnostico` llega hasta aquí y no más allá: sin abrir el diálogo de carpeta, sin
    // instalar normas, sin crear el directorio del canal y sin entrar en el bucle.
    if orden == Orden::Diagnostico {
        return Ok(());
    }

    // El ciclo sí necesita la carpeta resuelta. Tres fuentes por delante del diálogo, para que un
    // reinicio no dependa de que haya una persona mirando.
    let pedida_en_la_linea = match &orden {
        Orden::Ejecutar { ruta } => ruta.clone(),
        _ => None,
    };
    let junto_al_toml = directorio_del_ejecutable();
    let recordada = carpeta::recordada(&junto_al_toml);
    let carpeta = match carpeta::decidir(
        pedida_en_la_linea.as_deref(),
        config.carpeta.ruta.as_deref(),
        recordada.as_deref(),
        config.carpeta.preguntar_siempre,
    ) {
        Some((ruta, origen)) => {
            if !ruta.is_dir() {
                return Err(Error::Configuracion(format!(
                    "la carpeta de trabajo {} ({}) no existe",
                    ruta.display(),
                    origen.descripcion()
                )));
            }
            println!(
                "{}",
                arranque::mensaje_carpeta_de_trabajo(&ruta, origen.descripcion())
            );
            ruta
        }
        None => {
            let ruta = rfd::FileDialog::new()
                .set_title("Elige la carpeta de trabajo de Programator")
                .pick_folder()
                .ok_or_else(|| {
                    Error::Configuracion("no se eligió carpeta de trabajo".to_string())
                })?;
            println!(
                "{}",
                arranque::mensaje_carpeta_de_trabajo(&ruta, "elegida en la ventana emergente")
            );
            ruta
        }
    };

    // Recordada en cuanto se sabe que es buena, y no al terminar: un ciclo que vive días no puede
    // esperar a su propio final para dejar anotado dónde trabajaba.
    carpeta::recordar(&junto_al_toml, &carpeta);

    // Una línea más al historial de encaje (INC-N07 de NatureLand), ahora que la carpeta de
    // trabajo ya se conoce. `reunir_datos_de_arranque` corre antes de resolverla a propósito (para
    // que `--diagnostico` funcione sin ella), así que este registro va aquí y no allí. Sin modelo
    // declarado no hay nada que registrar; un fallo al escribir solo avisa, nunca aborta.
    if let Some(capas_en_gpu) = datos.capas_en_gpu {
        let marca_tiempo = programator::protocolo::marca_de_tiempo_actual();
        let linea = programator::motor::encaje_historico::linea_historico(
            &programator::motor::encaje_historico::DatosDeEncaje {
                marca_tiempo: &marca_tiempo,
                gpu: datos.gpu.as_ref(),
                contexto: config.motor.contexto,
                capas_en_gpu,
                capas_totales: capas_totales_modelo,
                version: VERSION,
            },
        );
        let ruta_encaje_historico =
            espacio::ruta_del_encaje_historico(&carpeta, &config.agente.nombre);
        if let Err(fallo) =
            programator::motor::encaje_historico::registrar(&ruta_encaje_historico, &linea)
        {
            eprintln!(
                "{}",
                programator::motor::encaje_historico::mensaje_fallo_registrar(
                    &ruta_encaje_historico,
                    &fallo
                )
            );
        }
    }

    // Las instrucciones que ve el modelo en cada encargo. Se resuelve aquí, una vez, y no en cada
    // pasada: es configuración, no algo que cambie mientras el ciclo corre.
    let preambulo = config.resolver(&config.agente.preambulo);
    let recordatorio = config.resolver(&config.agente.recordatorio);

    // El fichero de instrucciones: se compone con lo que el arnés va a ejecutar de verdad y se
    // instala en el espacio propio del agente, no en la raíz de un repositorio que no es suyo.
    let plantilla = config.resolver(&config.agente.plantilla_normas);
    let texto_plantilla = std::fs::read_to_string(&plantilla).map_err(|causa| Error::Lectura {
        ruta: plantilla.clone(),
        causa,
    })?;
    let instrucciones = normas::componer(&texto_plantilla, &config.verificacion.comprobadores);
    let destino_normas = espacio::normas_en_uso(&carpeta, &config.agente.nombre);
    if instalar_normas(&destino_normas, &instrucciones)? {
        println!(
            "{}",
            arranque::mensaje_instrucciones_instaladas(&destino_normas)
        );
    }

    let canal_dir = carpeta.join(".gestor").join("canal");
    std::fs::create_dir_all(&canal_dir).map_err(|causa| Error::Escritura {
        ruta: canal_dir.clone(),
        causa,
    })?;

    // La guía para los demás agentes, al día en cada arranque. Si no se puede escribir se avisa y
    // se sigue: quedarse sin guía es malo, pero no arrancar es peor.
    let plantilla_guia = config.resolver(&config.agente.guia);
    let plantilla_desempeno = config.resolver(&config.agente.desempeno);
    match publicar_guia(
        &canal_dir,
        &plantilla_guia,
        "COMO-ENCARGAR-A-PROGRAMATOR.md",
        config.motor.contexto,
        &plantilla_desempeno,
        &config.verificacion.comprobadores,
    ) {
        Ok(()) => {
            println!("{}", arranque::mensaje_guia_publicada());
            println!("{}", arranque::mensaje_orientacion_modelo_pago());
        }
        Err(fallo) => eprintln!("{}", arranque::mensaje_fallo_publicar_guia(&fallo)),
    }

    // Los límites del ciclo se derivan íntegramente de la configuración (§4.3), con valores por
    // defecto medidos y centralizados en config.rs y documentados en programator.ejemplo.toml.
    let limites = Limites {
        max_herramientas: config.ciclo.max_herramientas_por_encargo,
        max_denegaciones_seguidas: config.ciclo.max_denegaciones_seguidas,
        max_repeticiones_recordadas: config.ciclo.max_repeticiones_recordadas,
        max_reintentos_motor: config.ciclo.max_reintentos_motor,
        espera_reintento_motor: std::time::Duration::from_secs(
            config.ciclo.espera_reintento_motor_segundos,
        ),
    };

    // El servidor que Programator arranque por su cuenta (si le toca) tiene que sobrevivir a
    // todas las vueltas del bucle: si viviera dentro de él, se cerraría al final de cada
    // iteración y con él el modelo que tanto costó cargar en VRAM.
    let mut servidor_local: Option<proceso::ServidorLocal> = None;

    let ruta_latido = espacio::ruta_del_latido(
        &carpeta,
        &config.agente.nombre,
        &config.ciclo.fichero_latido,
    );

    let ruta_registro = espacio::ruta_del_registro(&carpeta, &config.agente.nombre);
    if arranque::hay_registro_previo(&ruta_registro) {
        println!(
            "{}",
            arranque::mensaje_listo_reinicio(config.ciclo.intervalo_segundos)
        );
    }

    let mut huella = None;
    // Hay un encargo detectado que no se pudo atender porque el motor seguía cargando el modelo.
    // Mientras esto sea cierto, el bucle vuelve a intentarlo aunque nadie escriba en el canal.
    let mut pendiente_por_motor = false;
    loop {
        // Un fallo al leer la raíz del canal no puede tumbar el proceso. Esto corre sobre una
        // unidad virtual de Google Drive, donde un directorio puede quedar momentáneamente
        // inaccesible mientras se sincroniza, y Programator está pensado para vivir días: salir
        // con código 1 por un hipo de Drive sería perder al agente hasta que alguien lo note. Se
        // avisa por `stderr` con el motivo y se vuelve a sondear en la siguiente vuelta. La
        // decisión de aguantar es de este bucle: `hay_novedades` sigue devolviendo el error para
        // que cualquier otro que la llame pueda distinguir el fallo.
        let estado_fin_vuelta = match hay_novedades(&canal_dir, &mut huella) {
            Ok(sondeo) => {
                if sondeo == Sondeo::Ignorada {
                    // Algo del canal no se dejó leer, así que esta foto no vale para comparar. Se
                    // dice: un canal ilegible es un problema que hay que ver, y callarlo lo
                    // convertiría en un agente que parece despierto y no se entera de nada.
                    eprintln!("{}", arranque::mensaje_canal_parcialmente_ilegible());
                }

                if toca_atender(sondeo, pendiente_por_motor) {
                    if sondeo == Sondeo::Novedades {
                        println!("{}", arranque::mensaje_novedades_atendiendo());
                    } else {
                        println!(
                            "{}",
                            arranque::mensaje_reanudacion_encargo_esperando_motor()
                        );
                    }

                    let mut asegurar_motor = || -> Resultado<EstadoMotor> {
                        asegurar_motor_local(&config, &mut servidor_local)
                    };
                    match sesion::ejecutar_pasada(
                        &carpeta,
                        &config.agente.nombre,
                        &sesion::Ajustes {
                            limites: &limites,
                            verificacion: &config.verificacion,
                            preambulo: Some(&preambulo),
                            recordatorio: Some(&recordatorio),
                            prefijos_ignorados: &config.agente.prefijos_ignorados,
                            contexto: config.motor.contexto,
                            umbral_aviso_contexto: config.motor.umbral_aviso_contexto,
                            bytes_por_token: config.motor.bytes_por_token,
                            tope_lectura_bytes: config.ciclo.tope_lectura_bytes,
                            fichero_latido: &config.ciclo.fichero_latido,
                            intervalo_segundos: config.ciclo.intervalo_segundos,
                        },
                        &mut asegurar_motor,
                    ) {
                        Ok(desenlace) => {
                            // Si el motor sigue cargando, el encargo no se ha atendido y hay que
                            // volver por él aunque el canal no vuelva a moverse.
                            pendiente_por_motor =
                                matches!(desenlace, DesenlacePasada::MotorCargando);
                            imprimir_desenlace(&desenlace, config.ciclo.intervalo_segundos);

                            match desenlace {
                                DesenlacePasada::Atendidos(ref res) if res.fallidos > 0 => {
                                    EstadoFinVuelta::Error(format!(
                                        "falló la publicación de {} encargo(s)",
                                        res.fallidos
                                    ))
                                }
                                _ => EstadoFinVuelta::Reposo,
                            }
                        }
                        // Un fallo atendiendo el ciclo no puede tumbar el proceso: se avisa y se
                        // reintenta en la vuelta siguiente, igual que un canal ilegible. La
                        // bandera se deja como estaba: si había trabajo pendiente, lo sigue
                        // habiendo.
                        Err(fallo) => {
                            eprintln!("{}", arranque::mensaje_fallo_pasada_ciclo(&fallo));
                            EstadoFinVuelta::Error(fallo.to_string())
                        }
                    }
                } else {
                    EstadoFinVuelta::Reposo
                }
            }
            Err(fallo) => {
                eprintln!("{}", arranque::mensaje_fallo_sondeo_canal(&fallo));
                EstadoFinVuelta::Error(fallo.to_string())
            }
        };

        // El latido se escribe exactamente una vez por vuelta al terminar el sondeo y antes de
        // dormir, garantizando por tipos (Rust match exhaustivo) que ninguna rama futura omita
        // el registro en disco.
        registrar_latido_fin_vuelta(
            &ruta_latido,
            config.ciclo.intervalo_segundos,
            &estado_fin_vuelta,
        );

        std::thread::sleep(std::time::Duration::from_secs(
            config.ciclo.intervalo_segundos,
        ));
    }
}

/// Estado con el que concluye la vuelta de sondeo antes de entrar en reposo.
enum EstadoFinVuelta {
    Reposo,
    Error(String),
}

/// Registra en disco el latido correspondiente al desenlace de la vuelta de sondeo.
fn registrar_latido_fin_vuelta(
    ruta_latido: &Path,
    intervalo_segundos: u64,
    estado: &EstadoFinVuelta,
) {
    let latido = match estado {
        EstadoFinVuelta::Reposo => Latido::en_reposo(chrono::Local::now(), intervalo_segundos),
        EstadoFinVuelta::Error(detalle) => {
            Latido::en_error(chrono::Local::now(), intervalo_segundos, None, detalle)
        }
    };
    latido.guardar_con_aviso(ruta_latido);
}

/// `main` decide qué imprimir a partir del desenlace de la pasada: `ejecutar_pasada` solo informa.
fn imprimir_desenlace(desenlace: &DesenlacePasada, intervalo_segundos: u64) {
    match desenlace {
        DesenlacePasada::PrimeraPasada => {
            println!(
                "{}",
                arranque::mensaje_listo_primer_arranque(intervalo_segundos)
            );
        }
        DesenlacePasada::SinEncargos => {
            println!("{}", arranque::mensaje_desenlace_sin_encargos());
        }
        DesenlacePasada::MotorCargando => {
            println!("{}", arranque::mensaje_desenlace_motor_cargando());
        }
        DesenlacePasada::Atendidos(ResumenPasada {
            atendidos,
            fallidos,
            ..
        }) => println!(
            "{}",
            arranque::mensaje_desenlace_atendidos(*atendidos, *fallidos)
        ),
    }
}

fn directorio_del_ejecutable() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
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
/// Detectado el 22/09/2026 en la primera ejecución real del ciclo (INC-04 de la evaluación).
fn toca_atender(sondeo: Sondeo, pendiente_por_motor: bool) -> bool {
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

#[cfg(test)]
mod pruebas {
    use super::*;
    use programator::protocolo::EstadoLatido;

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
    fn registrar_latido_fin_vuelta_guarda_reposo_y_error() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("latido.json");

        registrar_latido_fin_vuelta(&ruta, 180, &EstadoFinVuelta::Reposo);
        let latido_reposo = Latido::cargar(&ruta).unwrap().expect("existe");
        assert_eq!(latido_reposo.estado, EstadoLatido::Reposo);
        assert!(latido_reposo.encargo.is_none());

        registrar_latido_fin_vuelta(
            &ruta,
            180,
            &EstadoFinVuelta::Error("fallo de prueba".to_string()),
        );
        let latido_error = Latido::cargar(&ruta).unwrap().expect("existe");
        assert_eq!(latido_error.estado, EstadoLatido::Error);
        assert_eq!(
            latido_error.detalle_error.as_deref(),
            Some("fallo de prueba")
        );
    }

    #[test]
    fn primera_pasada_utiliza_mensaje_sin_jerga_y_con_cadencia() {
        let msg = arranque::mensaje_listo_primer_arranque(180);
        assert!(msg.contains("punto de partida sin atender encargos anteriores"));
        assert!(msg.contains("Programator ya está escuchando"));
        assert!(msg.contains("cada 3 minutos"));
        assert!(!msg.contains("registro vacío"));
        assert!(!msg.contains("primera pasada"));
    }
}
