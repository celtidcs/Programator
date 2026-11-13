//! Programator: arnés que convierte un modelo local en un agente del canal.

use programator::argumentos::{self, Orden};
use programator::arnes::ciclo::Limites;
use programator::arnes::sesion::{self, DesenlacePasada, EstadoMotor, ResumenPasada};
use programator::arnes::{espacio, normas};
use programator::arranque::{self, hay_novedades, instalar_normas, publicar_guia, Sondeo};
use programator::carpeta;
use programator::config::{Config, NOMBRE_DEL_FICHERO_DE_CONFIGURACION};
use programator::diagnostico::reunir_datos_de_arranque;
use programator::error::{Error, Resultado};
use programator::informe::{componer_informe, DatosDeArranque};
use programator::motor::hardware::Gpu;
use programator::motor::preparar::asegurar_motor_local;
use programator::motor::proceso;
use programator::protocolo::Latido;
use programator::revision;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// La versión del paquete, para no escribirla a mano en dos sitios.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// El código de salida de una orden mal escrita: el 2 que usan las herramientas de línea de
/// órdenes para «uso incorrecto», distinto del 1 de un fallo al ejecutar.
const CODIGO_DE_USO_INCORRECTO: i32 = 2;

fn main() {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let orden = match argumentos::interpretar(&argumentos) {
        Orden::LanzarAislado { orden } => {
            match programator::ayudante_aislado::lanzar_aislado(&orden) {
                Ok(codigo) => std::process::exit(codigo),
                Err(motivo) => {
                    eprintln!(
                        "{}",
                        programator::ayudante_aislado::mensaje_del_fallo(&motivo)
                    );
                    std::process::exit(programator::ayudante_aislado::CODIGO_AISLAMIENTO_FALLIDO);
                }
            }
        }
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
            std::process::exit(CODIGO_DE_USO_INCORRECTO);
        }
        Orden::FaltaValor(cual) => {
            eprintln!("{}", arranque::mensaje_argumento_falta_valor(&cual));
            std::process::exit(CODIGO_DE_USO_INCORRECTO);
        }
        Orden::BancoInforme { resultados } => {
            salir_si_falla_el_banco(programator::banco::tanda::rehacer(Path::new(&resultados)));
            return;
        }
        Orden::Banco { carpeta, modelo } => {
            salir_si_falla_el_banco(ejecutar_banco(Path::new(&carpeta), modelo.as_deref()));
            return;
        }
        Orden::BancoComprobar { tanda } => {
            salir_si_falla_el_banco(comprobar_banco(Path::new(&tanda)));
            return;
        }
        Orden::BancoComparar { tanda_a, tanda_b } => {
            salir_si_falla_el_banco(comparar_tandas(Path::new(&tanda_a), Path::new(&tanda_b)));
            return;
        }
        Orden::RegistroAnotar { tanda } => {
            anotar_en_el_registro(Path::new(&tanda));
            return;
        }
        Orden::RevisionBuscar => {
            match buscar_en_la_revision() {
                Ok(hecha) => println!("{}", revision::mensajes::busqueda_hecha(&hecha)),
                Err(fallo) => salir_por_la_revision(&fallo),
            }
            return;
        }
        Orden::RevisionProponer => {
            match proponer_en_la_revision() {
                Ok(hecha) => println!("{}", revision::mensajes::propuesta_hecha(&hecha)),
                Err(fallo) => salir_por_la_revision(&fallo),
            }
            return;
        }
        Orden::RetirarModelo { fichero } => {
            match retirar_un_modelo(&fichero) {
                Ok(retirada) => println!("{}", revision::mensajes::retirada_hecha(&retirada)),
                Err(fallo) => {
                    eprintln!("{}", revision::mensajes::fallo_de_la_retirada(&fallo));
                    std::process::exit(1);
                }
            }
            return;
        }
        Orden::DescargarModelo {
            repositorio,
            fichero,
        } => {
            match descargar_un_modelo(&repositorio, &fichero) {
                Ok(revision::traer::Traido::Descargado(hecho)) => {
                    println!("{}", revision::mensajes::descarga_hecha(&hecho))
                }
                Ok(revision::traer::Traido::YaEstaba(ruta)) => {
                    println!("{}", revision::mensajes::ya_estaba(&ruta))
                }
                Err(fallo) => salir_por_la_revision(&fallo),
            }
            return;
        }
        orden @ (Orden::Diagnostico { .. } | Orden::Ejecutar { .. }) => orden,
    };

    if let Err(fallo) = ejecutar(orden) {
        eprintln!("{}", arranque::mensaje_fallo_arranque(&fallo));
        std::process::exit(1);
    }
}

/// `programator.toml`, junto al ejecutable: el único sitio donde se busca.
fn ruta_de_la_configuracion() -> PathBuf {
    directorio_del_ejecutable().join(NOMBRE_DEL_FICHERO_DE_CONFIGURACION)
}

fn salir_si_falla_el_banco(resultado: Resultado<()>) {
    if let Err(fallo) = resultado {
        eprintln!("{}", programator::banco::mensajes::mensaje_fallo(&fallo));
        std::process::exit(1);
    }
}

/// `--banco`: la misma configuración que el ciclo, con el modelo de `--modelo` si se pidió, y el
/// mismo cierre ordenado del motor.
fn ejecutar_banco(carpeta: &Path, modelo: Option<&str>) -> Resultado<()> {
    let config = Config::cargar(&ruta_de_la_configuracion())?.con_modelo(modelo);
    proceso::registrar_cierre_ordenado()?;
    programator::banco::tanda::ejecutar(&config, carpeta, VERSION)
}

/// `--banco-comprobar`: solo las pruebas ocultas, sin motor. Del TOML solo hace falta el nombre
/// del agente, para leer su respuesta en el canal de cada pasada.
fn comprobar_banco(tanda: &Path) -> Resultado<()> {
    let config = Config::cargar(&ruta_de_la_configuracion())?;
    programator::banco::tanda::comprobar(&config, tanda)
}

/// `--banco-comparar`: sin motor. Del TOML solo hace falta el nombre del agente, para
/// encontrar la conversación de cada pasada.
fn comparar_tandas(tanda_a: &Path, tanda_b: &Path) -> Resultado<()> {
    let config = Config::cargar(&ruta_de_la_configuracion())?;
    programator::banco::comparar::comparar(tanda_a, tanda_b, &config.agente.nombre)
}

/// Lo que la guía cuenta del modelo configurado, sacado del registro de modelos. Un registro que
/// no se puede leer no impide arrancar: se avisa, y la guía dice que el modelo no está medido.
fn desempeno_para_la_guia(config: &Config) -> String {
    use programator::registro::{mensajes, modelo, redaccion};
    let nombre = config.motor.nombre_del_modelo().unwrap_or_default();
    let ruta = config.resolver(&config.agente.registro_de_modelos);
    let registro = modelo::leer(&ruta).unwrap_or_else(|fallo| {
        eprintln!("{}", mensajes::mensaje_registro_ilegible(&fallo));
        modelo::Registro::default()
    });
    redaccion::componer_desempeno(
        &nombre,
        redaccion::entrada_del_modelo(&registro, &nombre),
        &config.motor,
    )
}

/// `--registro-anotar`: sin motor ni GPU. Del TOML solo hace falta dónde está el registro.
fn anotar_en_el_registro(tanda: &Path) {
    use programator::registro::{anotar_tanda, mensajes};
    let desenlace = Config::cargar(&ruta_de_la_configuracion()).and_then(|config| {
        let registro = config.resolver(&config.agente.registro_de_modelos);
        anotar_tanda::anotar_tanda(tanda, &registro).map(|desenlace| (desenlace, registro))
    });
    match desenlace {
        Ok((anotar_tanda::Desenlace::Anotada(anotacion, entrada), registro)) => {
            println!(
                "{}",
                mensajes::mensaje_anotada(&anotacion, &entrada, &registro)
            );
        }
        Ok((anotar_tanda::Desenlace::NoSeAnota(motivo), _)) => {
            eprintln!("{}", mensajes::mensaje_no_se_anota(&motivo, tanda));
            std::process::exit(1);
        }
        Err(fallo) => {
            eprintln!("{}", mensajes::mensaje_fallo(&fallo));
            std::process::exit(1);
        }
    }
}

/// `--revision-buscar`: sin motor. Mide la tarjeta, lee el registro y pregunta a Hugging Face.
fn buscar_en_la_revision() -> Resultado<revision::buscar::BusquedaHecha> {
    revision::ordenes::buscar(&Config::cargar(&ruta_de_la_configuracion())?)
}

/// `--revision-proponer`: sin motor ni red. De la tarjeta solo hace falta su nombre.
fn proponer_en_la_revision() -> Resultado<revision::proponer::PropuestaHecha> {
    revision::ordenes::proponer(&Config::cargar(&ruta_de_la_configuracion())?)
}

/// `--retirar-modelo`: sin motor, GPU ni red.
fn retirar_un_modelo(fichero: &str) -> Resultado<revision::retirada::Retirada> {
    revision::ordenes::retirar(&Config::cargar(&ruta_de_la_configuracion())?, fichero)
}

/// `--descargar-modelo`: sin motor ni GPU. Del TOML solo hace falta `[revision]`.
fn descargar_un_modelo(repositorio: &str, fichero: &str) -> Resultado<revision::traer::Traido> {
    let config = Config::cargar(&ruta_de_la_configuracion())?;
    revision::ordenes::descargar(&config, repositorio, fichero, &mut |aviso| {
        println!("{}", revision::mensajes::aviso_de_descarga(&aviso))
    })
}

fn salir_por_la_revision(fallo: &Error) -> ! {
    eprintln!("{}", revision::mensajes::fallo(fallo));
    std::process::exit(1);
}

/// El ciclo del agente: arranca, se prepara en la carpeta de trabajo y vigila el canal hasta que
/// lo paren. `--diagnostico` se queda en el informe de arranque.
fn ejecutar(orden: Orden) -> Resultado<()> {
    let ruta_config = ruta_de_la_configuracion();
    let config = cargar_la_configuracion(&ruta_config, &orden)?;

    // Una sola vez, al arrancar el programa y antes de entrar en el bucle: si Programator llega a
    // arrancar su propio `llama-server`, esto es lo que lo mata al recibir Ctrl+C en vez de
    // dejarlo huérfano con el modelo cargado en VRAM.
    proceso::registrar_cierre_ordenado()?;

    avisar_si_hay_version_nueva(&config);
    let encaje = informar_del_arranque(&config);

    // `--diagnostico` llega hasta aquí y no más allá: sin abrir el diálogo de carpeta, sin
    // instalar normas, sin crear el directorio del canal y sin entrar en el bucle.
    if matches!(orden, Orden::Diagnostico { .. }) {
        return Ok(());
    }

    let carpeta = resolver_la_carpeta(&orden, &config)?;
    registrar_el_encaje(&carpeta, &config, &encaje);
    let canal_dir = preparar_la_carpeta(&carpeta, &config)?;
    vigilar_el_canal(ruta_config, config, &carpeta, &canal_dir)
}

/// La configuración, con el modelo de `--diagnostico --modelo` si se pidió, y sus avisos de claves
/// retiradas impresos.
fn cargar_la_configuracion(ruta_config: &Path, orden: &Orden) -> Resultado<Config> {
    let modelo_pedido = match orden {
        Orden::Diagnostico { modelo } => modelo.as_deref(),
        _ => None,
    };
    let config = Config::cargar(ruta_config)?.con_modelo(modelo_pedido);
    for aviso in config.avisos() {
        eprintln!("{aviso}");
    }
    Ok(config)
}

/// Aviso, solo informativo, de si hay una versión más nueva en GitHub. Nunca puede impedir que
/// Programator arranque: sin red, con GitHub caído o fuera del tiempo de espera, el arranque
/// sigue igual, solo que el aviso dice que no se pudo comprobar en vez de calentarse. Va antes
/// del informe (y no después, donde iba hasta el 2026-10-01) para que no quede metido entre la
/// fila de carpeta del informe y la línea que confirma la carpeta resuelta, justo debajo: esas
/// dos sí tienen que quedar juntas. Se comprueba también en `--diagnostico`, que es justo para
/// preguntar «¿esto cómo está hoy?».
fn avisar_si_hay_version_nueva(config: &Config) {
    if !config.actualizaciones.comprobar {
        return;
    }
    use programator::actualizacion::{mensaje_al_dia, mensaje_no_comprobado, Comprobacion};
    match programator::actualizacion::comprobar_version_mas_reciente(
        &config.actualizaciones.repositorio,
        VERSION,
        std::time::Duration::from_secs(config.actualizaciones.tiempo_espera_segundos),
    ) {
        Comprobacion::VersionNueva(aviso) => eprintln!("{aviso}"),
        Comprobacion::AlDia => eprintln!("{}", mensaje_al_dia(VERSION)),
        Comprobacion::NoComprobado => eprintln!("{}", mensaje_no_comprobado()),
    }
}

/// Lo medido al arrancar que hace falta después, cuando ya se conoce la carpeta de trabajo, para
/// anotar el historial de encaje.
struct EncajeDelArranque {
    gpu: Option<Gpu>,
    /// Las capas que se pedirán a `llama-server`. `None` sin modelo declarado.
    capas_en_gpu: Option<u32>,
    capas_totales: Option<u32>,
}

/// Compone e imprime el informe de arranque.
///
/// El informe se compone **antes** de resolver la carpeta de trabajo, y a propósito no la
/// necesita resuelta: `--diagnostico` existe justo para poder preguntar «¿esto cómo está hoy?»
/// sin dejar un proceso colgado, que es lo que pasaba cuando el diálogo de carpeta se abría
/// primero. Si `[carpeta] ruta` no está fijada, la última fila lo dice y ya; los otros cuatro
/// datos no dependen de la carpeta para nada. Va por `stderr`, como el resto de los avisos
/// desde la 0.4.0. La fila de carpeta va última a propósito (2026-10-01): si el diálogo decide
/// la carpeta más abajo, la línea que lo confirma se imprime justo después, sin nada entre
/// medias.
fn informar_del_arranque(config: &Config) -> EncajeDelArranque {
    let datos = reunir_datos_de_arranque(config);
    // Capturado antes de que `componer_informe` se lleve `datos.modelo` por valor: lo necesita el
    // registro del historial de encaje, una vez se conoce la carpeta de trabajo.
    let capas_totales = match &datos.modelo {
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
    EncajeDelArranque {
        gpu: datos.gpu,
        capas_en_gpu: datos.capas_en_gpu,
        capas_totales,
    }
}

/// La carpeta de trabajo del ciclo. Tres fuentes por delante del diálogo, para que un reinicio no
/// dependa de que haya una persona mirando. Se recuerda en cuanto se sabe que es buena, y no al
/// terminar: un ciclo que vive días no puede esperar a su propio final para dejar anotado dónde
/// trabajaba.
fn resolver_la_carpeta(orden: &Orden, config: &Config) -> Resultado<PathBuf> {
    let pedida_en_la_linea = match orden {
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
                return Err(Error::Configuracion(arranque::mensaje_carpeta_inexistente(
                    &ruta,
                    origen.descripcion(),
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
                .set_title(arranque::TITULO_DEL_DIALOGO_DE_CARPETA)
                .pick_folder()
                .ok_or_else(|| Error::Configuracion(arranque::mensaje_sin_carpeta_elegida()))?;
            println!(
                "{}",
                arranque::mensaje_carpeta_de_trabajo(&ruta, arranque::ORIGEN_VENTANA_EMERGENTE)
            );
            ruta
        }
    };
    carpeta::recordar(&junto_al_toml, &carpeta);
    Ok(carpeta)
}

/// Una línea más al historial de encaje (visto en uso real), ahora que la carpeta de trabajo
/// ya se conoce. `reunir_datos_de_arranque` corre antes de resolverla a propósito (para que
/// `--diagnostico` funcione sin ella), así que este registro va aquí y no allí. Sin modelo
/// declarado no hay nada que registrar; un fallo al escribir solo avisa, nunca aborta.
fn registrar_el_encaje(carpeta: &Path, config: &Config, encaje: &EncajeDelArranque) {
    use programator::motor::encaje_historico;
    let Some(capas_en_gpu) = encaje.capas_en_gpu else {
        return;
    };
    let marca_tiempo = programator::protocolo::marca_de_tiempo_actual();
    let linea = encaje_historico::linea_historico(&encaje_historico::DatosDeEncaje {
        marca_tiempo: &marca_tiempo,
        gpu: encaje.gpu.as_ref(),
        contexto: config.motor.contexto,
        capas_en_gpu,
        capas_totales: encaje.capas_totales,
        version: VERSION,
    });
    let ruta = espacio::ruta_del_encaje_historico(carpeta, &config.agente.nombre);
    if let Err(fallo) = encaje_historico::registrar(&ruta, &linea) {
        eprintln!(
            "{}",
            encaje_historico::mensaje_fallo_registrar(&ruta, &fallo)
        );
    }
}

/// Deja la carpeta de trabajo lista para el ciclo: las instrucciones del agente, el canal y la
/// guía para los demás. Devuelve la carpeta del canal.
fn preparar_la_carpeta(carpeta: &Path, config: &Config) -> Resultado<PathBuf> {
    instalar_las_instrucciones(carpeta, config)?;

    let canal_dir = programator::protocolo::carpetas::carpeta_del_canal(carpeta);
    std::fs::create_dir_all(&canal_dir).map_err(|causa| Error::Escritura {
        ruta: canal_dir.clone(),
        causa,
    })?;

    publicar_la_guia_del_equipo(&canal_dir, config);
    Ok(canal_dir)
}

/// El fichero de instrucciones: se compone con lo que el arnés va a ejecutar de verdad y se
/// instala en el espacio propio del agente, no en la raíz de un repositorio que no es suyo.
fn instalar_las_instrucciones(carpeta: &Path, config: &Config) -> Resultado<()> {
    let plantilla = config.resolver(&config.agente.plantilla_normas);
    let texto_plantilla = std::fs::read_to_string(&plantilla).map_err(|causa| Error::Lectura {
        ruta: plantilla.clone(),
        causa,
    })?;
    let instrucciones = normas::componer(&texto_plantilla, &config.verificacion.comprobadores);
    let destino_normas = espacio::normas_en_uso(carpeta, &config.agente.nombre);
    if instalar_normas(&destino_normas, &instrucciones)? {
        println!(
            "{}",
            arranque::mensaje_instrucciones_instaladas(&destino_normas)
        );
    }
    Ok(())
}

/// La guía para los demás agentes, al día en cada arranque. Si no se puede escribir se avisa y
/// se sigue: quedarse sin guía es malo, pero no arrancar es peor.
fn publicar_la_guia_del_equipo(canal_dir: &Path, config: &Config) {
    let plantilla_guia = config.resolver(&config.agente.guia);
    let desempeno = desempeno_para_la_guia(config);
    match publicar_guia(
        canal_dir,
        &plantilla_guia,
        programator::protocolo::carpetas::NOMBRE_DE_LA_GUIA,
        config.motor.contexto,
        &desempeno,
        &config.verificacion.comprobadores,
    ) {
        Ok(()) => {
            println!("{}", arranque::mensaje_guia_publicada());
            println!("{}", arranque::mensaje_orientacion_modelo_pago());
        }
        Err(fallo) => eprintln!("{}", arranque::mensaje_fallo_publicar_guia(&fallo)),
    }
}

/// El bucle del agente: sondea el canal, atiende cuando toca, escribe el latido y duerme. No
/// termina: lo para Ctrl+C.
fn vigilar_el_canal(
    ruta_config: PathBuf,
    mut config: Config,
    carpeta: &Path,
    canal_dir: &Path,
) -> Resultado<()> {
    // Las instrucciones que ve el modelo en cada encargo. Se resuelve aquí, una vez, y no en cada
    // pasada: es configuración, no algo que cambie mientras el ciclo corre.
    let preambulo = config.resolver(&config.agente.preambulo);
    let recordatorio = config.resolver(&config.agente.recordatorio);

    // Los límites del ciclo se derivan íntegramente de la configuración (§4.3), con valores por
    // defecto medidos y centralizados en config.rs y documentados en programator.ejemplo.toml.
    let limites = Limites::de_la_config(&config.ciclo);

    // El servidor que Programator arranque por su cuenta (si le toca) tiene que sobrevivir a
    // todas las vueltas del bucle: si viviera dentro de él, se cerraría al final de cada
    // iteración y con él el modelo que tanto costó cargar en VRAM. Lo comparten el motor del
    // ciclo y la revisión de modelos, que lo suelta antes de pasar el banco.
    let servidor_local: RefCell<Option<proceso::ServidorLocal>> = RefCell::new(None);

    let ruta_latido =
        espacio::ruta_del_latido(carpeta, &config.agente.nombre, &config.ciclo.fichero_latido);
    let mut revisor = revision::atender::RevisorReal::nuevo(
        ruta_config.clone(),
        ruta_latido.clone(),
        &servidor_local,
        VERSION,
    );

    let ruta_registro = espacio::ruta_del_registro(carpeta, &config.agente.nombre);
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
        // `revisión: cerrar` puede haber cambiado `[motor] modelo`: el motor siguiente tiene que
        // arrancar con el nuevo, y la configuración en memoria todavía dice el viejo.
        if revisor.tomar_cambio_de_modelo() {
            match Config::cargar(&ruta_config) {
                Ok(nueva) => {
                    config = nueva;
                    println!("{}", revision::mensajes::configuracion_releida(&config));
                }
                Err(fallo) => eprintln!("{}", revision::mensajes::configuracion_sin_releer(&fallo)),
            }
        }

        // Un fallo al leer la raíz del canal no puede tumbar el proceso. Esto corre sobre una
        // unidad virtual de Google Drive, donde un directorio puede quedar momentáneamente
        // inaccesible mientras se sincroniza, y Programator está pensado para vivir días: salir
        // con código 1 por un hipo de Drive sería perder al agente hasta que alguien lo note. Se
        // avisa por `stderr` con el motivo y se vuelve a sondear en la siguiente vuelta. La
        // decisión de aguantar es de este bucle: `hay_novedades` sigue devolviendo el error para
        // que cualquier otro que la llame pueda distinguir el fallo.
        let estado_fin_vuelta = match hay_novedades(canal_dir, &mut huella) {
            Ok(sondeo) => {
                if sondeo == Sondeo::Ignorada {
                    // Algo del canal no se dejó leer, así que esta foto no vale para comparar. Se
                    // dice: un canal ilegible es un problema que hay que ver, y callarlo lo
                    // convertiría en un agente que parece despierto y no se entera de nada.
                    eprintln!("{}", arranque::mensaje_canal_parcialmente_ilegible());
                }
                atender_si_toca(
                    sondeo,
                    &mut pendiente_por_motor,
                    config.ciclo.intervalo_segundos,
                    || {
                        let mut asegurar_motor = || -> Resultado<EstadoMotor> {
                            asegurar_motor_local(&config, &mut servidor_local.borrow_mut())
                        };
                        sesion::ejecutar_pasada_con_revisor(
                            carpeta,
                            &config.agente.nombre,
                            &sesion::Ajustes::de_la_config(
                                &config,
                                &limites,
                                &config.verificacion,
                                &preambulo,
                                &recordatorio,
                            ),
                            &mut asegurar_motor,
                            &mut revisor,
                        )
                    },
                )
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

/// Una vuelta con algo que mirar: si toca, pasa por el canal con `pasar` y cuenta cómo acabó.
fn atender_si_toca(
    sondeo: Sondeo,
    pendiente_por_motor: &mut bool,
    intervalo_segundos: u64,
    pasar: impl FnOnce() -> Resultado<DesenlacePasada>,
) -> EstadoFinVuelta {
    if !toca_atender(sondeo, *pendiente_por_motor) {
        return EstadoFinVuelta::Reposo;
    }
    if sondeo == Sondeo::Novedades {
        println!("{}", arranque::mensaje_novedades_atendiendo());
    } else {
        println!(
            "{}",
            arranque::mensaje_reanudacion_encargo_esperando_motor()
        );
    }

    match pasar() {
        Ok(desenlace) => {
            // Si el motor sigue cargando, el encargo no se ha atendido y hay que volver por él
            // aunque el canal no vuelva a moverse.
            *pendiente_por_motor = matches!(desenlace, DesenlacePasada::MotorCargando);
            imprimir_desenlace(&desenlace, intervalo_segundos);

            match desenlace {
                DesenlacePasada::Atendidos(ref res) if res.fallidos > 0 => {
                    EstadoFinVuelta::Error(arranque::mensaje_publicaciones_fallidas(res.fallidos))
                }
                _ => EstadoFinVuelta::Reposo,
            }
        }
        // Un fallo atendiendo el ciclo no puede tumbar el proceso: se avisa y se reintenta en la
        // vuelta siguiente, igual que un canal ilegible. La bandera se deja como estaba: si había
        // trabajo pendiente, lo sigue habiendo.
        Err(fallo) => {
            eprintln!("{}", arranque::mensaje_fallo_pasada_ciclo(&fallo));
            EstadoFinVuelta::Error(fallo.to_string())
        }
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
/// Detectado el 22/09/2026 en la primera ejecución real del ciclo.
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
