//! Preparar el motor de inferencia para una pasada del ciclo.
//!
//! **Por qué vive aquí y no en `main`.** Decidir si se reutiliza un `llama-server` ajeno, si se
//! espera a que termine de cargar o si se arranca uno propio es el ciclo de vida del motor, no el
//! arranque del programa. `main` se ocupa de atender la línea de órdenes, montar la carpeta y
//! llevar el bucle; mezclar ahí esta decisión hacía que el módulo tuviera dos asuntos.
//!
//! La frontera con `proceso` es otra: allí se mide —qué responde el puerto, qué ficheros hay— y
//! aquí se decide qué hacer con esa medida.
//!
//! **Por qué cuelga de `motores` y no de `motor`.** Lee el registro de modelos, compone las
//! herramientas desde las fichas del arnés y devuelve el estado del motor tal y como lo ve la
//! sesión. Antes vivía en `motor/` y hacía que la capa de abajo importara las de arriba; `motor/`
//! ya no importa ni `arnes` ni `registro`.

use crate::arnes::sesion::EstadoMotor;
use crate::config::{Config, Muestreo};
use crate::error::{Error, Resultado};
use crate::motor::fichero_de_modelo::nombre_de_modelo;
use crate::motor::mensajes::{
    mensaje_arranque_motor, mensaje_motor_terminado, mensaje_puerto_con_otra_clave,
    mensaje_sin_motor_que_arrancar,
};
use crate::motor::{encaje, hardware, proceso, MotorLlama};
use crate::reloj::Reloj;
use crate::terminal::Terminal;
use std::path::Path;
use std::time::Duration;

/// Prepara el motor de inferencia para una pasada que ya sabe que tiene al menos un encargo que
/// atender, con el modelo de `[motor] modelo`. Ver [`asegurar_motor_para`].
pub fn asegurar_motor_local(
    config: &Config,
    servidor_local: &mut Option<proceso::ServidorLocal>,
    terminal: &dyn Terminal,
) -> Resultado<EstadoMotor> {
    let pedido = PedidoDeMotor {
        modelo: config.motor.modelo.as_deref(),
        muestreo: &config.muestreo,
        medida: &|nombre, tarjeta| medida_registrada(config, nombre, tarjeta, terminal),
        terminal,
    };
    Ok(asegurar_motor_para(config, &pedido, servidor_local)?.estado)
}

/// Igual que [`asegurar_motor_local`], pero con un muestreo dado en vez del de la configuración:
/// el banco de modelos fija la semilla y la temperatura de cada tanda sin tocar la `Config`. No
/// usa la medida del registro: el banco está para medir.
pub fn asegurar_motor_con_muestreo(
    config: &Config,
    muestreo: &Muestreo,
    servidor_local: &mut Option<proceso::ServidorLocal>,
    terminal: &dyn Terminal,
) -> Resultado<EstadoMotor> {
    let pedido = PedidoDeMotor {
        modelo: config.motor.modelo.as_deref(),
        muestreo,
        medida: &|_, _| None,
        terminal,
    };
    Ok(asegurar_motor_para(config, &pedido, servidor_local)?.estado)
}

/// Qué motor se pide: el modelo (como lo escribe la configuración, `None` si no hay ninguno), el
/// muestreo y de dónde sale la medida del banco para un nombre de modelo y una tarjeta.
pub struct PedidoDeMotor<'a> {
    pub modelo: Option<&'a str>,
    pub muestreo: &'a Muestreo,
    pub medida: &'a dyn Fn(&str, &str) -> Option<encaje::MedidaDelBanco>,
    /// Por dónde se avisa al arrancar el motor.
    pub terminal: &'a dyn Terminal,
}

/// Quién arrancó el servidor que responde. Con nombre, y no un `bool` llamado `ajeno`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Procedencia {
    /// Lo arrancó Programator y tiene el modelo que se le pidió.
    #[default]
    Propio,
    /// No lo arrancó Programator: se usa tal cual, aunque tenga cargado otro modelo.
    Ajeno,
}

/// El motor preparado y de dónde viene su servidor.
pub struct MotorPreparado {
    pub estado: EstadoMotor,
    pub procedencia: Procedencia,
}

/// El servidor que Programator tiene lanzado, comparado con el modelo pedido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServidorPropio {
    Ninguno,
    /// El mismo modelo. `vivo` dice si el proceso sigue en pie.
    Mismo {
        vivo: bool,
    },
    /// Otro modelo.
    Otro,
}

/// Lo que hay que hacer con el motor antes de un encargo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionDelMotor {
    /// El propio, con el modelo pedido, está listo.
    Usar,
    /// Responde uno que no es de Programator: se usa tal cual y no se cambia de modelo.
    UsarAjeno,
    /// Hay uno cargando (propio con el modelo pedido, o ajeno): todavía no.
    Esperar,
    /// No hay ninguno: se arranca el pedido.
    Arrancar,
    /// El propio tiene otro modelo: se suelta y se arranca el pedido.
    Cambiar,
    /// El propio, con el modelo pedido, terminó: es un fallo, no una espera. Se suelta y no se
    /// relanza en esta vuelta; la siguiente lo intentará de nuevo, y el encargo cuenta un intento.
    Terminado,
    /// En el puerto hay un `llama-server` que no acepta la clave de Programator: ni se usa ni se
    /// arranca otro encima, que no podría escuchar en un puerto ocupado. Lo resuelve la persona.
    PuertoConOtraClave,
}

/// Compara el servidor propio con el modelo pedido. Sin modelo pedido, el propio vale.
pub fn servidor_propio(lanzado: Option<(&Path, bool)>, pedido: Option<&Path>) -> ServidorPropio {
    match (lanzado, pedido) {
        (None, _) => ServidorPropio::Ninguno,
        (Some((_, vivo)), None) => ServidorPropio::Mismo { vivo },
        (Some((modelo, vivo)), Some(pedido)) if pedido == modelo => ServidorPropio::Mismo { vivo },
        (Some(_), Some(_)) => ServidorPropio::Otro,
    }
}

/// Cruza el servidor propio con lo que responde el puerto. Un propio con el modelo pedido que
/// aún no escucha pero sigue vivo está arrancando; si se murió, el encargo falla con el motivo
/// (`Terminado`) y pasa por la cola de pendientes.
pub fn decidir_motor(propio: ServidorPropio, puerto: proceso::EstadoServidor) -> AccionDelMotor {
    use proceso::EstadoServidor::{Cargando, Listo, NoDisponible, RechazaLaClave};
    match (propio, puerto) {
        (ServidorPropio::Otro, _) => AccionDelMotor::Cambiar,
        (ServidorPropio::Mismo { vivo: false }, _) => AccionDelMotor::Terminado,
        (_, RechazaLaClave) => AccionDelMotor::PuertoConOtraClave,
        (ServidorPropio::Ninguno, Listo) => AccionDelMotor::UsarAjeno,
        (ServidorPropio::Ninguno, Cargando) => AccionDelMotor::Esperar,
        (ServidorPropio::Ninguno, NoDisponible) => AccionDelMotor::Arrancar,
        (ServidorPropio::Mismo { vivo: true }, Listo) => AccionDelMotor::Usar,
        (ServidorPropio::Mismo { vivo: true }, Cargando) => AccionDelMotor::Esperar,
        (ServidorPropio::Mismo { vivo: true }, NoDisponible) => AccionDelMotor::Esperar,
    }
}

/// Prepara el motor para el modelo pedido. Consulta el puerto y el servidor propio, y hace lo
/// que diga [`decidir_motor`]:
/// - un ajeno listo se reutiliza y no se cierra nunca: puede ser del usuario;
/// - si hay que cambiar, se suelta el propio y, si el puerto queda libre, se arranca el pedido; si
///   sigue respondiendo alguien, es ajeno y se trata como tal;
/// - recién arrancado, se espera al ciclo siguiente;
/// - sin `motor.binario` o sin modelo, no se puede arrancar: es un error entendible, no un pánico.
pub fn asegurar_motor_para(
    config: &Config,
    pedido: &PedidoDeMotor,
    servidor_local: &mut Option<proceso::ServidorLocal>,
) -> Resultado<MotorPreparado> {
    let ruta_pedida = pedido.modelo.map(|m| config.resolver(m));
    let lanzado = servidor_local
        .as_ref()
        .map(|s| (s.modelo(), s.sigue_vivo()));
    let propio = servidor_propio(lanzado, ruta_pedida.as_deref());
    let mut accion = decidir_motor(
        propio,
        proceso::hay_servidor(config.motor.puerto, clave_vigente(config).as_ref()),
    );
    if accion == AccionDelMotor::Terminado {
        let modelo = servidor_local.take().map(|s| s.modelo().to_path_buf());
        return Err(Error::MotorSinRespuesta(mensaje_motor_terminado(
            modelo.as_deref(),
        )));
    }
    if accion == AccionDelMotor::Cambiar {
        // Al soltarlo se para el proceso y se espera a que salga: así queda libre la tarjeta.
        drop(servidor_local.take());
        accion = decidir_motor(
            ServidorPropio::Ninguno,
            proceso::hay_servidor(config.motor.puerto, clave_vigente(config).as_ref()),
        );
    }
    let nombre = pedido.modelo.unwrap_or("");
    let listo = |procedencia| MotorPreparado {
        estado: EstadoMotor::Disponible(Box::new(construir_motor(config, pedido.muestreo, nombre))),
        procedencia,
    };
    match accion {
        AccionDelMotor::Usar => {
            if let Some(propio) = servidor_local.as_mut() {
                anotar_si_falta_la_vram_ocupada(propio);
            }
            Ok(listo(Procedencia::Propio))
        }
        AccionDelMotor::UsarAjeno => Ok(listo(Procedencia::Ajeno)),
        AccionDelMotor::Esperar => Ok(MotorPreparado {
            estado: EstadoMotor::Esperando,
            procedencia: if servidor_local.is_some() {
                Procedencia::Propio
            } else {
                Procedencia::Ajeno
            },
        }),
        AccionDelMotor::PuertoConOtraClave => Err(Error::MotorSinRespuesta(
            mensaje_puerto_con_otra_clave(config.motor.puerto),
        )),
        AccionDelMotor::Arrancar | AccionDelMotor::Cambiar | AccionDelMotor::Terminado => {
            *servidor_local = Some(arrancar(config, ruta_pedida.as_deref(), pedido)?);
            // Recién arrancado, todavía está cargando el modelo: no tiene sentido lanzarle ya la
            // primera petición. Se espera al ciclo siguiente, que lo encontrará `Cargando` o
            // `Listo` según toque.
            Ok(MotorPreparado {
                estado: EstadoMotor::Esperando,
                procedencia: Procedencia::Propio,
            })
        }
    }
}

/// Como [`asegurar_motor_para`], pero si el motor se queda cargando espera en el sitio, mirando
/// cada [`LATIDO_DE_CARGA`] de `reloj`, hasta `tope`. `None`: no terminó de cargar a tiempo.
pub fn esperar_motor_para(
    config: &Config,
    pedido: &PedidoDeMotor,
    servidor_local: &mut Option<proceso::ServidorLocal>,
    tope: Duration,
    reloj: &mut dyn Reloj,
) -> Resultado<Option<MotorPreparado>> {
    esperar_hasta_disponible(tope, LATIDO_DE_CARGA, reloj, || {
        let preparado = asegurar_motor_para(config, pedido, servidor_local)?;
        Ok(matches!(preparado.estado, EstadoMotor::Disponible(_)).then_some(preparado))
    })
}

/// Repite `intentar` cada `latido` de `reloj` hasta que dé algo, o hasta pasar de `tope`:
/// entonces `None`. El único bucle de espera de carga del motor.
fn esperar_hasta_disponible<T>(
    tope: Duration,
    latido: Duration,
    reloj: &mut dyn Reloj,
    mut intentar: impl FnMut() -> Resultado<Option<T>>,
) -> Resultado<Option<T>> {
    let inicio = reloj.transcurrido();
    loop {
        if let Some(listo) = intentar()? {
            return Ok(Some(listo));
        }
        if reloj.transcurrido().saturating_sub(inicio) > tope {
            return Ok(None);
        }
        reloj.esperar(latido);
    }
}

/// Como [`esperar_hasta_disponible`], y además lo que tardó en `reloj`, desde antes del primer
/// intento hasta el que dio algo.
fn esperar_midiendo<T>(
    tope: Duration,
    latido: Duration,
    reloj: &mut dyn Reloj,
    intentar: impl FnMut() -> Resultado<Option<T>>,
) -> Resultado<Option<(T, Duration)>> {
    let inicio = reloj.transcurrido();
    let listo = esperar_hasta_disponible(tope, latido, reloj, intentar)?;
    Ok(listo.map(|listo| (listo, reloj.transcurrido().saturating_sub(inicio))))
}

/// La primera vez que el propio responde listo se mide lo que ocupa: la VRAM libre de antes de
/// lanzarlo menos la de ahora.
fn anotar_si_falta_la_vram_ocupada(propio: &mut proceso::ServidorLocal) {
    if propio.vram_ocupada_mib().is_some() {
        return;
    }
    let ahora = hardware::describir_gpu().map(|g| g.vram_libre);
    propio.anotar_vram_ocupada(vram_ocupada_mib(propio.vram_libre_antes(), ahora));
}

/// Lo que ocupa el motor, en MiB, a partir de la VRAM libre (en bytes) antes y después de
/// cargarlo.
pub fn vram_ocupada_mib(antes: Option<u64>, despues: Option<u64>) -> Option<u64> {
    antes
        .zip(despues)
        .map(|(antes, despues)| antes.saturating_sub(despues) / crate::unidades::BYTES_POR_MIB)
}

/// Lanza `llama-server` con el modelo pedido y el encaje que toque.
fn arrancar(
    config: &Config,
    modelo: Option<&Path>,
    pedido: &PedidoDeMotor,
) -> Resultado<proceso::ServidorLocal> {
    let (Some(binario), Some(ruta_modelo)) = (&config.motor.binario, modelo) else {
        return Err(Error::ConfiguracionDelMotor(
            mensaje_sin_motor_que_arrancar(),
        ));
    };
    let ruta_binario = config.resolver(binario);
    // `resolver` mide con la GPU real; en las pruebas de `motor::encaje` es una tarjeta
    // inventada la que decide. Aquí, y no en `encaje`, es donde se pregunta al sistema
    // gráfico y donde se avisa: los módulos informan, `main` decide qué se cuenta.
    let gpu = hardware::describir_gpu();
    let medida = nombre_de_modelo(ruta_modelo)
        .zip(gpu.as_ref())
        .and_then(|(nombre, gpu)| (pedido.medida)(&nombre, &gpu.nombre));
    let resolucion = encaje::resolver(&config.motor, ruta_modelo, gpu.as_ref(), medida.as_ref());
    // El informe de arranque ya contó el encaje con el que se pensaba lanzar el motor;
    // este es un segundo aviso, con la situación tal como está justo al arrancarlo, que
    // puede haber cambiado desde entonces.
    pedido
        .terminal
        .avisar(&mensaje_arranque_motor(&resolucion.aviso));
    let clave = crate::motor::clave::para_arrancar(config)?;
    proceso::ServidorLocal::arrancar(
        &config.motor,
        &ruta_binario,
        ruta_modelo,
        resolucion.capas,
        resolucion.en_cpu,
        gpu.map(|g| g.vram_libre),
        &clave,
    )
}

/// La medida que el banco guardó en el registro para ese modelo y esa tarjeta. Un registro que no
/// se puede leer no impide arrancar: se avisa y se calcula sin medida.
pub fn medida_registrada(
    config: &Config,
    nombre: &str,
    tarjeta: &str,
    terminal: &dyn Terminal,
) -> Option<encaje::MedidaDelBanco> {
    use crate::registro::{mensajes, modelo};
    let ruta = config.resolver(&config.agente.registro_de_modelos);
    match modelo::leer(&ruta) {
        Ok(registro) => registro.medida_para(nombre, tarjeta),
        Err(fallo) => {
            terminal.avisar(&mensajes::mensaje_registro_ilegible(&fallo));
            None
        }
    }
}

/// Cada cuánto se mira si el motor terminó de cargar. Fino, porque el tiempo de carga se guarda
/// como resultado; no es el intervalo del ciclo, que es de minutos.
pub const LATIDO_DE_CARGA: Duration = Duration::from_secs(1);

/// Lo que costó cargar el modelo en un motor propio.
#[derive(Debug, Clone, PartialEq)]
pub struct Carga {
    pub capas: u32,
    pub vram_mib: Option<u64>,
    /// Lo que el encaje calculaba que ocuparía esa misma carga, en MiB. Con `vram_mib`, da el
    /// margen que de verdad hace falta en esta máquina.
    pub vram_calculada_mib: Option<u64>,
    pub duracion: Duration,
    pub tarjeta: Option<String>,
}

/// Cómo acabó la espera de [`cargar_midiendo`]. No es un error que el motor tarde: quien llama
/// decide qué hace con un tiempo agotado (el banco lo da por fallo; el ciclo lo deja pendiente).
#[derive(Debug, Clone, PartialEq)]
pub enum DesenlaceDeCarga {
    /// Programator lanzó el motor y quedó listo: se midió lo que costó.
    Listo(Carga),
    /// El motor que responde no es de Programator: se usa, pero no se mide.
    Ajeno,
    /// El motor no terminó de cargar dentro del tope.
    TiempoAgotado,
}

/// Arranca el motor y espera a que esté listo, midiendo con `reloj`. Si ya había uno, no mide
/// nada.
pub fn cargar_midiendo(
    config: &Config,
    muestreo: &Muestreo,
    servidor: &mut Option<proceso::ServidorLocal>,
    tope: Duration,
    reloj: &mut dyn Reloj,
    terminal: &dyn Terminal,
) -> Resultado<DesenlaceDeCarga> {
    if matches!(
        proceso::hay_servidor(config.motor.puerto, clave_vigente(config).as_ref()),
        proceso::EstadoServidor::Listo
    ) {
        return Ok(DesenlaceDeCarga::Ajeno);
    }
    let gpu_antes = hardware::describir_gpu();
    let tarjeta = gpu_antes.as_ref().map(|g| g.nombre.clone());
    let libre_antes = gpu_antes.map(|g| g.vram_libre);
    let listo = esperar_midiendo(tope, LATIDO_DE_CARGA, reloj, || {
        let estado = asegurar_motor_con_muestreo(config, muestreo, servidor, terminal)?;
        Ok(matches!(estado, EstadoMotor::Disponible(_)).then_some(()))
    })?;
    let Some(((), duracion)) = listo else {
        return Ok(DesenlaceDeCarga::TiempoAgotado);
    };
    let libre_despues = hardware::describir_gpu().map(|g| g.vram_libre);
    let vram_mib = vram_ocupada_mib(libre_antes, libre_despues);
    let Some(propio) = servidor.as_mut() else {
        return Ok(DesenlaceDeCarga::Ajeno);
    };
    propio.anotar_vram_ocupada(vram_mib);
    let vram_calculada_mib = encaje::vram_calculada(
        &config.motor,
        propio.modelo(),
        propio.capas_gpu(),
        propio.en_cpu(),
    )
    .map(|bytes| bytes / crate::unidades::BYTES_POR_MIB);
    Ok(DesenlaceDeCarga::Listo(Carga {
        capas: propio.capas_gpu(),
        vram_mib,
        vram_calculada_mib,
        duracion,
        tarjeta,
    }))
}

fn construir_motor(config: &Config, muestreo: &Muestreo, modelo: &str) -> MotorLlama {
    let url_base = crate::motor::url_local(config.motor.puerto);
    MotorLlama::nuevo(&url_base, modelo)
        .con_tiempos_espera(
            std::time::Duration::from_secs(config.motor.tiempo_lectura_segundos),
            std::time::Duration::from_secs(config.motor.tiempo_escritura_segundos),
        )
        .con_herramientas(crate::arnes::ficha::declaraciones_concedibles())
        .con_muestreo(muestreo.clone())
        .con_inicio(crate::motor::continuidad::Inicio::segun(
            config.motor.empezar_en_frio,
        ))
        .con_clave(clave_vigente(config))
}

/// La clave con que se habla ahora con el motor del puerto.
fn clave_vigente(config: &Config) -> Option<crate::motor::clave::ClaveDelMotor> {
    crate::motor::clave::vigente(config)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::reloj::RelojDeMentira;
    use crate::terminal::{Flujo, TerminalDeMentira};

    use proceso::EstadoServidor::{Cargando, Listo, NoDisponible, RechazaLaClave};

    /// Un `llama-server` de mentira en una sola conexión: `/health` siempre contesta, y `/props`
    /// solo con la clave `clave`; sin ella, 401, como b10993.
    fn motor_con_clave(clave: &'static str) -> u16 {
        use std::io::{Read, Write};

        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut conexion, _)) = escucha.accept() else {
                return;
            };
            let mut buffer = [0u8; 4096];
            loop {
                let leidos = conexion.read(&mut buffer).unwrap_or(0);
                if leidos == 0 {
                    return;
                }
                let peticion = String::from_utf8_lossy(&buffer[..leidos]).to_lowercase();
                let con_clave = peticion.contains(&format!("authorization: bearer {clave}"));
                let (codigo, cuerpo) = if peticion.starts_with("get /health") {
                    (200, "{\"status\":\"ok\"}")
                } else if con_clave {
                    (200, "{\"total_slots\":1}")
                } else {
                    (401, "{\"error\":{\"code\":401}}")
                };
                let respuesta = format!(
                    "HTTP/1.1 {codigo} X\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\n\r\n{cuerpo}",
                    cuerpo.len()
                );
                if conexion.write_all(respuesta.as_bytes()).is_err() {
                    return;
                }
            }
        });
        puerto
    }

    /// A6, primera mitad, de punta a punta: con `clave = "auto"` y la clave del arranque anterior
    /// guardada junto a la configuración, el motor que sigue en el puerto se reutiliza.
    #[test]
    fn con_la_clave_guardada_se_reutiliza_el_motor_del_puerto() {
        let carpeta = tempfile::tempdir().unwrap();
        let puerto = motor_con_clave("clave-guardada");
        let ruta_config = carpeta.path().join("programator.toml");
        std::fs::write(
            &ruta_config,
            format!("[motor]\nmodelo = \"m.gguf\"\nclave = \"auto\"\npuerto = {puerto}\n"),
        )
        .unwrap();
        let config = Config::cargar(&ruta_config).unwrap();
        std::fs::write(
            config.resolver(crate::motor::clave::FICHERO_DE_LA_CLAVE),
            "clave-guardada\n",
        )
        .unwrap();
        let muestreo = Muestreo::default();
        let terminal = TerminalDeMentira::default();
        let pedido = PedidoDeMotor {
            modelo: None,
            muestreo: &muestreo,
            medida: &|_, _| None,
            terminal: &terminal,
        };

        let preparado = asegurar_motor_para(&config, &pedido, &mut None).unwrap();

        assert!(matches!(preparado.estado, EstadoMotor::Disponible(_)));
        assert_eq!(preparado.procedencia, Procedencia::Ajeno);
    }

    /// Un motor que no acepta la clave no se usa ni se tapa con otro, sea o no el propio; el propio
    /// con otro modelo se cambia igual, y el propio muerto sigue siendo un fallo.
    #[test]
    fn un_motor_que_rechaza_la_clave_no_se_usa_ni_se_arranca_otro_encima() {
        for propio in [
            ServidorPropio::Ninguno,
            ServidorPropio::Mismo { vivo: true },
        ] {
            assert_eq!(
                decidir_motor(propio, RechazaLaClave),
                AccionDelMotor::PuertoConOtraClave
            );
        }
        assert_eq!(
            decidir_motor(ServidorPropio::Otro, RechazaLaClave),
            AccionDelMotor::Cambiar
        );
        assert_eq!(
            decidir_motor(ServidorPropio::Mismo { vivo: false }, RechazaLaClave),
            AccionDelMotor::Terminado
        );
    }

    #[test]
    fn sin_servidor_propio_lo_que_responde_es_ajeno_y_si_no_hay_nada_se_arranca() {
        let ninguno = ServidorPropio::Ninguno;
        assert_eq!(decidir_motor(ninguno, Listo), AccionDelMotor::UsarAjeno);
        assert_eq!(decidir_motor(ninguno, Cargando), AccionDelMotor::Esperar);
        assert_eq!(
            decidir_motor(ninguno, NoDisponible),
            AccionDelMotor::Arrancar
        );
    }

    #[test]
    fn con_el_mismo_modelo_no_se_toca_nada_y_si_el_proceso_termino_es_un_fallo() {
        let vivo = ServidorPropio::Mismo { vivo: true };
        let muerto = ServidorPropio::Mismo { vivo: false };
        assert_eq!(decidir_motor(vivo, Listo), AccionDelMotor::Usar);
        assert_eq!(decidir_motor(vivo, Cargando), AccionDelMotor::Esperar);
        assert_eq!(decidir_motor(vivo, NoDisponible), AccionDelMotor::Esperar);
        for puerto in [Listo, Cargando, NoDisponible] {
            // Antes se relanzaba en el acto, y un motor que no puede cargar su modelo se
            // relanzaba sin fin.
            assert_eq!(decidir_motor(muerto, puerto), AccionDelMotor::Terminado);
        }
    }

    #[test]
    fn con_otro_modelo_se_cambia_este_como_este_el_puerto() {
        for puerto in [Listo, Cargando, NoDisponible] {
            assert_eq!(
                decidir_motor(ServidorPropio::Otro, puerto),
                AccionDelMotor::Cambiar
            );
        }
    }

    #[test]
    fn el_servidor_propio_se_compara_con_el_modelo_pedido() {
        let a = Path::new("modelos/a.gguf");
        let b = Path::new("modelos/b.gguf");
        assert_eq!(servidor_propio(None, Some(a)), ServidorPropio::Ninguno);
        assert_eq!(
            servidor_propio(Some((a, true)), Some(a)),
            ServidorPropio::Mismo { vivo: true }
        );
        assert_eq!(
            servidor_propio(Some((a, false)), Some(a)),
            ServidorPropio::Mismo { vivo: false }
        );
        assert_eq!(
            servidor_propio(Some((a, true)), Some(b)),
            ServidorPropio::Otro
        );
        assert_eq!(
            servidor_propio(Some((a, true)), None),
            ServidorPropio::Mismo { vivo: true }
        );
    }

    #[test]
    fn la_vram_ocupada_es_la_libre_de_antes_menos_la_de_despues_en_mib() {
        let mib = crate::unidades::BYTES_POR_MIB;
        assert_eq!(vram_ocupada_mib(Some(10 * mib), Some(4 * mib)), Some(6));
        assert_eq!(vram_ocupada_mib(Some(4 * mib), Some(10 * mib)), Some(0));
        assert_eq!(vram_ocupada_mib(None, Some(mib)), None);
        assert_eq!(vram_ocupada_mib(Some(mib), None), None);
    }

    #[test]
    fn el_nombre_del_modelo_es_el_del_fichero_sin_extension() {
        assert_eq!(
            nombre_de_modelo(Path::new("modelos/Devstral-Q4.gguf")).as_deref(),
            Some("Devstral-Q4")
        );
    }

    #[test]
    fn la_medida_registrada_sale_del_registro_y_sin_registro_legible_no_hay() {
        use crate::registro::modelo::{self, pruebas::entrada_de_ejemplo, Registro};
        let carpeta = tempfile::tempdir().unwrap();
        let ruta_config = carpeta.path().join("programator.toml");
        std::fs::write(
            &ruta_config,
            "[motor]\nmodelo = \"m.gguf\"\n[agente]\nnombre = \"Programator\"\n\
             buzon = \"b.md\"\nplantilla_normas = \"n.md\"\n\
             registro_de_modelos = \"registro.toml\"\n",
        )
        .unwrap();
        let config = Config::cargar(&ruta_config).unwrap();
        let entrada = entrada_de_ejemplo();
        let terminal = TerminalDeMentira::default();
        let medida_registrada =
            |config: &Config, nombre: &str, tarjeta: &str| -> Option<encaje::MedidaDelBanco> {
                medida_registrada(config, nombre, tarjeta, &terminal)
            };

        assert_eq!(
            medida_registrada(&config, &entrada.nombre, &entrada.tarjeta),
            None
        );

        let registro = Registro {
            modelo: vec![entrada.clone()],
        };
        modelo::escribir(&carpeta.path().join("registro.toml"), &registro).unwrap();
        let medida = medida_registrada(&config, &entrada.nombre, &entrada.tarjeta).unwrap();
        assert_eq!(medida.capas, 34);
        assert_eq!(medida.vram_ocupada_mib, 13763);

        std::fs::write(carpeta.path().join("registro.toml"), "esto no es toml [").unwrap();
        assert_eq!(
            medida_registrada(&config, &entrada.nombre, &entrada.tarjeta),
            None
        );
    }

    #[test]
    fn un_registro_ilegible_se_avisa_por_la_salida_de_errores_de_la_terminal_inyectada() {
        use crate::registro::{mensajes, modelo};
        let carpeta = tempfile::tempdir().unwrap();
        let ruta_config = carpeta.path().join("programator.toml");
        std::fs::write(
            &ruta_config,
            "[motor]\nmodelo = \"m.gguf\"\n[agente]\nnombre = \"Programator\"\n\
             buzon = \"b.md\"\nplantilla_normas = \"n.md\"\nregistro_de_modelos = \"registro.toml\"\n",
        )
        .unwrap();
        let config = Config::cargar(&ruta_config).unwrap();
        let ruta_registro = carpeta.path().join("registro.toml");
        std::fs::write(&ruta_registro, "esto no es toml [").unwrap();
        let terminal = TerminalDeMentira::default();

        let medida = medida_registrada(&config, "Devstral", "RTX", &terminal);

        assert_eq!(medida, None);
        let fallo = modelo::leer(&ruta_registro).unwrap_err();
        assert_eq!(
            terminal.capturado(),
            vec![(Flujo::Errores, mensajes::mensaje_registro_ilegible(&fallo))]
        );
    }

    #[test]
    fn la_espera_repite_hasta_que_esta_disponible() {
        let mut intentos = 0;
        let mut reloj = RelojDeMentira::default();
        let listo =
            esperar_hasta_disponible(Duration::from_secs(5), Duration::ZERO, &mut reloj, || {
                intentos += 1;
                Ok((intentos == 3).then_some("listo"))
            })
            .unwrap();

        assert_eq!(listo, Some("listo"));
        assert_eq!(intentos, 3);
    }

    #[test]
    fn la_espera_se_rinde_pasado_el_tope_y_un_fallo_corta_en_el_acto() {
        let mut intentos = 0;
        let mut reloj = RelojDeMentira::default();
        let nunca =
            esperar_hasta_disponible(Duration::ZERO, Duration::from_millis(1), &mut reloj, || {
                intentos += 1;
                Ok(None::<()>)
            })
            .unwrap();
        assert_eq!(nunca, None);
        assert!(intentos >= 1);

        let mut tras_el_fallo = 0;
        let fallo = esperar_hasta_disponible(
            Duration::from_secs(5),
            Duration::ZERO,
            &mut RelojDeMentira::default(),
            || {
                tras_el_fallo += 1;
                Err::<Option<()>, _>(crate::error::Error::Configuracion("caído".into()))
            },
        );
        assert!(fallo.is_err());
        assert_eq!(tras_el_fallo, 1);
    }

    /// El tope se mide con el reloj inyectado y vale hasta el instante en que
    /// se cumple, incluido; pasado, se deja de intentar. Sin dormir de verdad.
    #[test]
    fn la_espera_se_mide_con_el_reloj_inyectado_y_se_rinde_al_pasar_del_tope() {
        let mut intentos = 0;
        let mut reloj = RelojDeMentira::en(Duration::from_secs(100));

        let nunca = esperar_hasta_disponible(
            Duration::from_secs(3),
            Duration::from_secs(1),
            &mut reloj,
            || {
                intentos += 1;
                Ok(None::<()>)
            },
        )
        .unwrap();

        assert_eq!(nunca, None);
        // Intentos en los segundos 0, 1, 2, 3 y 4 de la espera: en el 3 aún no se ha pasado.
        assert_eq!(intentos, 5);
        assert_eq!(reloj.esperas(), 4);
        assert_eq!(reloj.transcurrido(), Duration::from_secs(104));
    }

    /// Lo que tarda en cargar se mide con el reloj inyectado, desde antes del
    /// primer intento, y no con la hora real.
    #[test]
    fn lo_que_tarda_la_carga_se_mide_con_el_reloj_inyectado() {
        let mut intentos = 0;
        let mut reloj = RelojDeMentira::en(Duration::from_secs(100));

        let listo = esperar_midiendo(
            Duration::from_secs(60),
            Duration::from_secs(2),
            &mut reloj,
            || {
                intentos += 1;
                Ok((intentos == 4).then_some("listo"))
            },
        )
        .unwrap();

        assert_eq!(listo, Some(("listo", Duration::from_secs(6))));

        let mut reloj = RelojDeMentira::default();
        let agotado = esperar_midiendo(Duration::ZERO, Duration::from_secs(1), &mut reloj, || {
            Ok(None::<()>)
        })
        .unwrap();
        assert_eq!(agotado, None);
    }

    #[test]
    fn mensaje_arranque_motor_incluye_el_aviso_de_encaje() {
        let mensaje = mensaje_arranque_motor("40 de 40 capas en GPU");
        assert_eq!(
            mensaje,
            "Arrancando el motor — encaje: 40 de 40 capas en GPU"
        );
    }
}
