//! Una tanda completa del banco: pasar todos los encargos con todas las semillas, comprobar,
//! puntuar y dejar los resultados.
//!
//! Va en dos fases, y a propósito: primero se pasan todos los encargos con el motor cargado, y
//! después, con el motor ya apagado, se comprueban las entregas. Así las pruebas ocultas no
//! compiten con el motor por el equipo, y si la fase 2 falla, la 1 queda en disco y se termina
//! sin volver a cargar el modelo.
//!
//! [`pasar_encargos`], [`puntuar`] y [`puntuar_tanda`] reciben el motor y las pruebas desde fuera,
//! para probarlos con dobles. [`ejecutar`] y [`rehacer`] son la capa fina que los conecta con el
//! motor de verdad, la configuración y la terminal.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::comprobacion::lanzador::LanzadorLocal;
use super::comprobacion::local::{CarpetaLocal, CARPETA_DE_COMPROBACION};
use super::comprobacion::prueba_previa::probar_el_lanzador;
use super::comprobacion::{copia_del_interprete, interprete};
use super::comprobacion::{
    montar_carpeta_de_pruebas, Comprobado, Comprobador, LugarDeEjecucion, ResultadoDePruebas,
};
use super::config::{Aislamiento, ComprobacionLocal};
use super::config::{ConfigBanco, ModoDeComprobacion};
use super::ejecucion::{
    carpeta_del_encargo, pasar_encargo, preparar_carpeta, Preparacion, Recogida, Reloj, RelojReal,
};
use super::ficha::{cargar_encargos, DefectoSembrado, Encargo, FormaDePuntuar, NOMBRE_RUBRICA};
use super::lugar::{elegir_lugar, hay_codigo_que_comprobar, LugarElegido};
use super::mensajes;
use super::pasadas::{self, PasadaGuardada, PasadasDeTanda};
use super::puntuacion::{
    contar_auditoria, leer_hallazgos, nota_de_auditoria, nota_de_comprobaciones,
    nota_de_fichero_limpio, HechosDeComprobaciones, Nota, RecuentoDeAuditoria,
};
use super::rastro;
use super::resultados::{
    calcular_clases, guardar, rehacer_informe, AjustesDelMotor, DatosDeTanda, EstadoDeNota,
    PuntuadaCon, ResultadoDeEncargo, ResultadoDeSemilla, ResultadosDeTanda, NOMBRE_INFORME,
    NOMBRE_RESULTADOS,
};
use super::rubrica::{
    componer_fichero, componer_fichero_de_codigo, leer_fichero as leer_fichero_de_rubrica,
    nombre_del_fichero,
};
use crate::arnes::ciclo::Limites;
use crate::arnes::normas;
use crate::arnes::sesion::{Ajustes, EstadoMotor};
use crate::config::{Config, Muestreo, Verificacion};
use crate::error::{Error, Resultado};
use crate::ficheros::{borrar_si_existe, crear_carpeta, escribir, huella_sha256, leer};
use crate::motor::encaje::BYTES_POR_MIB;
use crate::motor::preparar::asegurar_motor_con_muestreo;
use crate::motor::{hardware, proceso};
use crate::orden_acotada::{ejecutar_con_tope, DesenlaceDeOrden};

/// La configuración del banco, dentro de su carpeta.
pub const NOMBRE_BANCO_TOML: &str = "banco.toml";
/// Los encargos, uno por subcarpeta.
pub const CARPETA_DE_ENCARGOS: &str = "encargos";
/// Una subcarpeta por tanda.
pub const CARPETA_DE_RESULTADOS: &str = "resultados";
/// Dentro de la tanda: los ficheros que puntúa el agente que dirige.
pub const CARPETA_DE_RUBRICAS: &str = "rubricas";
/// Dentro de la tanda: la carpeta de trabajo de cada encargo y semilla.
pub const CARPETA_DE_TRABAJO: &str = "trabajo";
/// Detrás del nombre de la carpeta de un encargo, la de sus pruebas con la entrega colocada.
const SUFIJO_DE_PRUEBAS: &str = "pruebas";

/// Cómo se nombra la carpeta de una tanda delante del modelo: ordena por fecha al listarlas y no
/// lleva espacios ni dos puntos, que Windows no admite en un nombre de carpeta.
const FORMATO_DE_LA_CARPETA_DE_LA_TANDA: &str = "%Y-%m-%d-%H%M";

/// Cómo se escribe la fecha de la tanda en `resultados.toml`: la misma forma que las marcas del
/// protocolo, para que se lea igual que el resto del canal.
const FORMATO_DE_LA_FECHA_DE_LA_TANDA: &str = "%Y-%m-%d %H:%M";
/// Cada cuánto se mira si el motor terminó de cargar. Fino, porque el tiempo de carga se guarda
/// como resultado; no es el intervalo del ciclo, que es de minutos.
const LATIDO_DE_CARGA: Duration = Duration::from_secs(1);
/// Lo que puede tardar `llama-server --version`.
const TIEMPO_MAXIMO_VERSION_MOTOR: Duration = Duration::from_secs(30);
/// Lo que se guarda como compilación del motor si no se puede preguntar.
const COMPILACION_DESCONOCIDA: &str = "desconocida";

/// Un encargo pasado con una semilla, a falta de comprobar y puntuar.
pub struct Pasado<'a> {
    pub encargo: &'a Encargo,
    pub semilla: i64,
    pub carpeta: PathBuf,
    pub recogida: Recogida,
}

/// Lanza las pruebas ocultas sobre una carpeta ya montada.
pub type Pruebas<'p> = dyn FnMut(&Encargo, &Path) -> Comprobado + 'p;

/// Qué se pasa en la fase 1, y dónde.
pub struct Recorrido<'a, 'b> {
    pub encargos: &'a [Encargo],
    pub semillas: &'b [i64],
    /// Aquí va la carpeta de cada encargo y semilla.
    pub carpeta_de_trabajo: &'b Path,
}

/// Fase 1: por cada semilla, todos los encargos, cada uno en su carpeta nueva.
///
/// `asegurar` da el motor para una semilla: quien llama fija con ella el muestreo.
pub fn pasar_encargos<'a>(
    recorrido: &Recorrido<'a, '_>,
    preparacion: &Preparacion,
    ajustes: &Ajustes,
    asegurar: &mut dyn FnMut(i64) -> Resultado<EstadoMotor>,
    reloj: &mut dyn Reloj,
    avisar: &mut dyn FnMut(String),
    anotar: &mut dyn FnMut(&Pasado) -> Resultado<()>,
) -> Resultado<Vec<Pasado<'a>>> {
    let total = recorrido.encargos.len() * recorrido.semillas.len();
    let mut pasados = Vec::with_capacity(total);
    for &semilla in recorrido.semillas {
        for encargo in recorrido.encargos {
            let carpeta =
                carpeta_del_encargo(recorrido.carpeta_de_trabajo, &encargo.ficha.id, semilla);
            preparar_carpeta(&carpeta, encargo, preparacion)?;
            let mut asegurar_con_semilla = || asegurar(semilla);
            let recogida = pasar_encargo(
                &carpeta,
                encargo,
                preparacion,
                ajustes,
                &mut asegurar_con_semilla,
                reloj,
            )?;
            avisar(mensajes::mensaje_de_avance(
                pasados.len() + 1,
                total,
                &encargo.ficha.id,
                semilla,
                recogida.a_tiempo,
                recogida.duracion,
                recogida.entregas.len(),
            ));
            let pasado = Pasado {
                encargo,
                semilla,
                carpeta,
                recogida,
            };
            anotar(&pasado)?;
            pasados.push(pasado);
        }
    }
    Ok(pasados)
}

/// La semilla con lo que se midió al pasarla, sin nota todavía.
fn semilla_medida(pasado: &Pasado) -> ResultadoDeSemilla {
    let consumo = &pasado.recogida.consumo;
    ResultadoDeSemilla {
        semilla: pasado.semilla,
        segundos: pasado.recogida.duracion.as_secs_f64(),
        tokens_entrada: consumo.total.tokens_entrada,
        tokens_en_cache: consumo.total.tokens_en_cache,
        tokens_generados: consumo.total.tokens_generados,
        ms_entrada: consumo.total.ms_entrada,
        ms_generacion: consumo.total.ms_generacion,
        respuestas_sin_consumo: consumo.respuestas_sin_consumo,
        ..ResultadoDeSemilla::default()
    }
}

fn poner_nota(semilla: &mut ResultadoDeSemilla, nota: Nota) {
    (semilla.estado, semilla.nota) = match nota {
        Nota::Evaluada(valor) => (EstadoDeNota::Evaluada, Some(valor)),
        Nota::NoEvaluado => (EstadoDeNota::NoEvaluado, None),
    };
}

fn poner_recuento(semilla: &mut ResultadoDeSemilla, recuento: &RecuentoDeAuditoria) {
    semilla.sembrados = Some(recuento.sembrados);
    semilla.aciertos = Some(recuento.aciertos);
    semilla.inventados = Some(recuento.inventados);
    semilla.repetidos = Some(recuento.repetidos);
    semilla.sin_formato = Some(recuento.sin_formato);
}

const NO_RESPONDIO: &str = "no respondió a tiempo";

/// Comprueba y puntúa un encargo pasado. Los de rúbrica dejan su fichero en `carpeta_rubricas`.
pub fn puntuar(
    pasado: &Pasado,
    margen: usize,
    pruebas: Option<&mut Pruebas>,
    carpeta_rubricas: &Path,
) -> Resultado<ResultadoDeSemilla> {
    let ficha = &pasado.encargo.ficha;
    let recogida = &pasado.recogida;
    let mut semilla = semilla_medida(pasado);
    let forma = ficha
        .forma_de_puntuar()
        .ok_or_else(|| Error::Banco(format!("{}: familia y clase no casan", ficha.id)))?;
    // Lo que no llegó a tiempo no cuenta, aunque algo se escribiera.
    let respuesta = if recogida.a_tiempo {
        recogida.respuesta.as_str()
    } else {
        semilla.detalle = Some(NO_RESPONDIO.into());
        ""
    };

    match forma {
        FormaDePuntuar::Comprobaciones => {
            puntuar_comprobaciones(pasado, pruebas, carpeta_rubricas, &mut semilla)?
        }
        FormaDePuntuar::Auditoria => {
            puntuar_auditoria(respuesta, &ficha.defectos, margen, &mut semilla)
        }
        FormaDePuntuar::FicheroLimpio => {
            puntuar_fichero_limpio(respuesta, margen, recogida.a_tiempo, &mut semilla)
        }
        FormaDePuntuar::Rubrica => {
            puntuar_con_rubrica(pasado, respuesta, carpeta_rubricas, &mut semilla)?
        }
    }
    Ok(semilla)
}

/// Un encargo de código: se comprueba lo que entregó, si lo entregó a tiempo. Si las pruebas
/// ocultas no dan nota y el encargo trae rúbrica, se valora con ella leyendo el código.
fn puntuar_comprobaciones(
    pasado: &Pasado,
    pruebas: Option<&mut Pruebas>,
    carpeta_rubricas: &Path,
    semilla: &mut ResultadoDeSemilla,
) -> Resultado<()> {
    let recogida = &pasado.recogida;
    let nombre = pasado.encargo.ficha.entrega.as_deref().unwrap_or_default();
    let hechos = match recogida.entrega(nombre).filter(|_| recogida.a_tiempo) {
        None => {
            semilla
                .detalle
                .get_or_insert(format!("no entregó {nombre}"));
            semilla.puntuada_con = Some(PuntuadaCon::SinEntrega);
            HechosDeComprobaciones::NoEntregado
        }
        Some(ruta) => {
            let hechos = comprobar_la_entrega(pasado, ruta, pruebas, semilla)?;
            if hechos == HechosDeComprobaciones::NoEvaluado {
                return valorar_leyendo_el_codigo(pasado, ruta, carpeta_rubricas, semilla);
            }
            semilla.puntuada_con = Some(PuntuadaCon::PruebasOcultas);
            hechos
        }
    };
    if let HechosDeComprobaciones::Compila { superadas, total } = hechos {
        semilla.comprobaciones_superadas = Some(superadas);
        semilla.comprobaciones_total = Some(total);
    }
    poner_nota(semilla, nota_de_comprobaciones(hechos));
    Ok(())
}

/// Un encargo de código cuyas pruebas ocultas no dieron nota: con rúbrica, deja su fichero con el
/// código entregado para que quien dirige lo valore leyéndolo; sin ella, queda «no evaluado» y el
/// motivo lo dice. `semilla.detalle` trae ya por qué las pruebas no dieron nota.
fn valorar_leyendo_el_codigo(
    pasado: &Pasado,
    entregado: &Path,
    carpeta_rubricas: &Path,
    semilla: &mut ResultadoDeSemilla,
) -> Resultado<()> {
    let encargo = pasado.encargo;
    let motivo = semilla.detalle.clone().unwrap_or_default();
    let ruta_rubrica = encargo.carpeta.join(NOMBRE_RUBRICA);
    if !ruta_rubrica.is_file() {
        semilla.detalle = Some(mensajes::motivo_sin_rubrica_de_codigo(&motivo));
        poner_nota(semilla, Nota::NoEvaluado);
        return Ok(());
    }
    let nombre_entrega = encargo.ficha.entrega.as_deref().unwrap_or_default();
    let contenido = componer_fichero_de_codigo(
        &encargo.ficha.id,
        pasado.semilla,
        nombre_entrega,
        &leer(entregado)?,
        &motivo,
        &leer(&ruta_rubrica)?,
    );
    dejar_rubrica(pasado, &contenido, carpeta_rubricas, semilla)?;
    semilla.puntuada_con = Some(PuntuadaCon::Rubrica);
    Ok(())
}

/// Lo que cumple una entrega: los requisitos de texto y, si hay dónde, las pruebas ocultas.
fn comprobar_la_entrega(
    pasado: &Pasado,
    ruta: &Path,
    pruebas: Option<&mut Pruebas>,
    semilla: &mut ResultadoDeSemilla,
) -> Resultado<HechosDeComprobaciones> {
    let encargo = pasado.encargo;
    let ficha = &encargo.ficha;
    let contenido = leer(ruta)?;
    let cumplidos = ficha
        .texto
        .iter()
        .filter(|r| r.se_cumple_en(&contenido))
        .count();
    let comprobado = match pruebas {
        None => Comprobado::from(ResultadoDePruebas::NoEvaluado(
            mensajes::motivo_pruebas_ocultas_apagadas(),
        )),
        Some(pruebas) => {
            let montada = carpeta_de_pruebas(&pasado.carpeta);
            // Solo existe si ya se intentó comprobar esta tanda: se monta de nuevo para que no
            // quede nada del intento anterior.
            borrar_si_existe(&montada)?;
            montar_carpeta_de_pruebas(encargo, ruta, &montada)?;
            pruebas(encargo, &montada)
        }
    };
    let resultado = comprobado.resultado;
    if !comprobado.intentos_de_red.is_empty() {
        eprintln!(
            "{}",
            mensajes::mensaje_intento_de_red(
                &ficha.id,
                pasado.semilla,
                &comprobado.intentos_de_red
            )
        );
    }
    semilla.intentos_de_red = comprobado.intentos_de_red;
    match &resultado {
        ResultadoDePruebas::NoEvaluado(motivo) | ResultadoDePruebas::NoCompila(motivo) => {
            semilla.detalle = Some(motivo.clone())
        }
        ResultadoDePruebas::Compila { .. } => {}
    }
    Ok(resultado.hechos(cumplidos, ficha.texto.len()))
}

/// Un encargo de auditoría: los defectos sembrados que encontró y los que se inventó.
fn puntuar_auditoria(
    respuesta: &str,
    defectos: &[DefectoSembrado],
    margen: usize,
    semilla: &mut ResultadoDeSemilla,
) {
    let recuento = contar_auditoria(&leer_hallazgos(respuesta), defectos, margen);
    poner_recuento(semilla, &recuento);
    poner_nota(semilla, nota_de_auditoria(&recuento));
}

/// Un fichero sin defectos: cuenta lo que se inventó.
fn puntuar_fichero_limpio(
    respuesta: &str,
    margen: usize,
    a_tiempo: bool,
    semilla: &mut ResultadoDeSemilla,
) {
    let recuento = contar_auditoria(&leer_hallazgos(respuesta), &[], margen);
    poner_recuento(semilla, &recuento);
    // Callar no es confirmar que está limpio: sin respuesta, un 0.
    let nota = if a_tiempo {
        nota_de_fichero_limpio(recuento.inventados)
    } else {
        Nota::Evaluada(0.0)
    };
    poner_nota(semilla, nota);
}

/// Un encargo de criterio: deja la rúbrica para que la puntúe una persona, o recoge la nota si ya
/// está puesta.
fn puntuar_con_rubrica(
    pasado: &Pasado,
    respuesta: &str,
    carpeta_rubricas: &Path,
    semilla: &mut ResultadoDeSemilla,
) -> Resultado<()> {
    let encargo = pasado.encargo;
    let rubrica = leer(&encargo.carpeta.join(NOMBRE_RUBRICA))?;
    let respuesta = if pasado.recogida.a_tiempo {
        respuesta
    } else {
        NO_RESPONDIO
    };
    let contenido = componer_fichero(&encargo.ficha.id, pasado.semilla, respuesta, &rubrica);
    dejar_rubrica(pasado, &contenido, carpeta_rubricas, semilla)
}

/// Deja el fichero de rúbrica de esta semilla con `contenido`, o recoge su nota si ya estaba.
fn dejar_rubrica(
    pasado: &Pasado,
    contenido: &str,
    carpeta_rubricas: &Path,
    semilla: &mut ResultadoDeSemilla,
) -> Resultado<()> {
    let nombre = nombre_del_fichero(&pasado.encargo.ficha.id, pasado.semilla);
    let ruta = carpeta_rubricas.join(&nombre);
    // Ya existe si se comprueba otra vez una tanda (`--banco-comprobar`), y puede llevar la nota
    // puesta a mano: se respeta. Lo que se valora es lo mismo, lo de `pasadas.toml`.
    if ruta.is_file() {
        semilla.aplicar_rubrica(leer_fichero_de_rubrica(&ruta)?);
    } else {
        escribir(&ruta, contenido)?;
        semilla.estado = EstadoDeNota::Pendiente;
        semilla.nota = None;
    }
    semilla.rubrica = Some(format!("{CARPETA_DE_RUBRICAS}/{nombre}"));
    Ok(())
}

/// La carpeta de pruebas montadas de un encargo, junto a su carpeta de trabajo.
fn carpeta_de_pruebas(carpeta_del_encargo: &Path) -> PathBuf {
    let nombre = carpeta_del_encargo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    carpeta_del_encargo.with_file_name(format!("{nombre}-{SUFIJO_DE_PRUEBAS}"))
}

/// Fase 2: comprueba y puntúa todo lo pasado y lo agrupa por encargo, en el orden en que se pasó.
pub fn puntuar_tanda(
    pasados: &[Pasado],
    datos: DatosDeTanda,
    margen: usize,
    mut pruebas: Option<&mut Pruebas>,
    carpeta_de_la_tanda: &Path,
) -> Resultado<ResultadosDeTanda> {
    let carpeta_rubricas = carpeta_de_la_tanda.join(CARPETA_DE_RUBRICAS);
    crear_carpeta(&carpeta_rubricas)?;
    let mut encargos: Vec<ResultadoDeEncargo> = Vec::new();
    for pasado in pasados {
        let semilla = puntuar(pasado, margen, pruebas.as_deref_mut(), &carpeta_rubricas)?;
        let ficha = &pasado.encargo.ficha;
        let posicion = match encargos.iter().position(|e| e.id == ficha.id) {
            Some(posicion) => posicion,
            None => {
                encargos.push(ResultadoDeEncargo {
                    id: ficha.id.clone(),
                    cuenta_como: ficha.encargo_que_cuenta().to_string(),
                    titulo: ficha.titulo.clone(),
                    familia: ficha.familia,
                    clase: ficha.clase,
                    semillas: Vec::new(),
                });
                encargos.len() - 1
            }
        };
        encargos[posicion].semillas.push(semilla);
    }
    Ok(ResultadosDeTanda {
        tanda: datos,
        clases: calcular_clases(&encargos),
        encargos,
    })
}

/// Lo que costó cargar el modelo, si lo cargó el banco.
struct Carga {
    capas: u32,
    vram_mib: Option<u64>,
    duracion: Duration,
    tarjeta: Option<String>,
}

/// Arranca el motor y espera a que esté listo, midiendo. Si ya había uno, no mide nada.
fn cargar_midiendo(
    config: &Config,
    muestreo: &Muestreo,
    servidor: &mut Option<proceso::ServidorLocal>,
    tope: Duration,
) -> Resultado<Option<Carga>> {
    if matches!(
        proceso::hay_servidor(config.motor.puerto),
        proceso::EstadoServidor::Listo
    ) {
        println!("{}", mensajes::mensaje_motor_ajeno());
        return Ok(None);
    }
    let gpu_antes = hardware::describir_gpu();
    let tarjeta = gpu_antes.as_ref().map(|g| g.nombre.clone());
    let libre_antes = gpu_antes.map(|g| g.vram_libre);
    let inicio = Instant::now();
    loop {
        match asegurar_motor_con_muestreo(config, muestreo, servidor)? {
            EstadoMotor::Disponible(_) => break,
            EstadoMotor::Esperando if inicio.elapsed() > tope => {
                return Err(Error::Banco(format!(
                    "el motor no terminó de cargar en {} s",
                    tope.as_secs()
                )))
            }
            EstadoMotor::Esperando => std::thread::sleep(LATIDO_DE_CARGA),
        }
    }
    let duracion = inicio.elapsed();
    let libre_despues = hardware::describir_gpu().map(|g| g.vram_libre);
    let vram_mib = libre_antes
        .zip(libre_despues)
        .map(|(antes, despues)| antes.saturating_sub(despues) / BYTES_POR_MIB);
    Ok(servidor.as_ref().map(|s| Carga {
        capas: s.capas_gpu(),
        vram_mib,
        duracion,
        tarjeta,
    }))
}

/// La compilación de `llama-server`, tal como la cuenta `--version`.
fn compilacion_del_motor(binario: Option<&Path>) -> String {
    let Some(binario) = binario else {
        return COMPILACION_DESCONOCIDA.into();
    };
    let directorio = binario.parent().unwrap_or_else(|| Path::new("."));
    let programa = binario.display().to_string();
    match ejecutar_con_tope(
        &programa,
        &["--version".to_string()],
        directorio,
        TIEMPO_MAXIMO_VERSION_MOTOR,
    ) {
        DesenlaceDeOrden::Terminada {
            salida, errores, ..
        } => format!("{salida}\n{errores}")
            .lines()
            .find(|l| l.contains("version"))
            .map(|l| l.trim().to_string())
            .unwrap_or_else(|| COMPILACION_DESCONOCIDA.into()),
        _ => COMPILACION_DESCONOCIDA.into(),
    }
}

/// Un `f32` de la configuración como el `f64` que se escribiría a mano: 0.2, no 0.2000000029.
fn decimal_de_la_configuracion(valor: f32) -> f64 {
    valor.to_string().parse().unwrap_or(f64::from(valor))
}

/// Lo de la configuración del motor que se guarda con la tanda para poder compararla.
fn ajustes_del_motor(config: &Config) -> AjustesDelMotor {
    AjustesDelMotor {
        contexto: config.motor.contexto,
        cache_kv: config.motor.cache_kv.clone(),
        paralelo: config.motor.paralelo,
        empezar_en_frio: config.motor.empezar_en_frio,
        top_k: config.muestreo.top_k,
        top_p: decimal_de_la_configuracion(config.muestreo.top_p),
        min_p: decimal_de_la_configuracion(config.muestreo.min_p),
        penalizacion_repeticion: decimal_de_la_configuracion(
            config.muestreo.penalizacion_repeticion,
        ),
    }
}

/// El `banco.toml` y los encargos de la carpeta del banco.
fn cargar_banco(carpeta_banco: &Path) -> Resultado<(ConfigBanco, Vec<Encargo>)> {
    let ruta_toml = carpeta_banco.join(NOMBRE_BANCO_TOML);
    let banco = if ruta_toml.is_file() {
        ConfigBanco::cargar(&ruta_toml)?
    } else {
        eprintln!("{}", mensajes::mensaje_sin_banco_toml(&ruta_toml));
        ConfigBanco::desde_texto("")?
    };
    let encargos = cargar_encargos(&carpeta_banco.join(CARPETA_DE_ENCARGOS))?;
    exigir_ordenes_de_herramientas(&banco, &encargos)?;
    Ok((banco, encargos))
}

/// Si el banco ejecuta código, cada herramienta que pidan las fichas tiene que tener una orden
/// que mire si está instalada: sin ella, el banco no sabría distinguir «no está instalada» de «no
/// se sabe comprobar», y se dice antes de cargar el modelo. Con las pruebas ocultas apagadas no
/// se ejecuta nada y no hace falta.
fn exigir_ordenes_de_herramientas(banco: &ConfigBanco, encargos: &[Encargo]) -> Resultado<()> {
    let ModoDeComprobacion::Local(local) = banco.modo_de_comprobacion() else {
        return Ok(());
    };
    let sin_orden = local.herramientas_sin_orden(encargos);
    if sin_orden.is_empty() {
        Ok(())
    } else {
        Err(Error::Banco(mensajes::mensaje_herramientas_sin_orden(
            &sin_orden,
        )))
    }
}

/// Si se van a ejecutar pruebas ocultas, deja lista antes de cargar el modelo la copia de Git, si
/// hace falta, y prueba que la carpeta de comprobación se puede preparar en la de resultados del
/// banco.
fn probar_el_lugar(lugar: &LugarElegido, carpeta_banco: &Path) -> Resultado<()> {
    match lugar {
        LugarElegido::Ninguno => Ok(()),
        LugarElegido::Local(local) => {
            interprete_de_este_equipo(local, carpeta_banco)?;
            probar_el_lanzador(
                &mut LanzadorLocal::para(local),
                &carpeta_banco.join(CARPETA_DE_RESULTADOS),
            )
        }
    }
}

/// Lo que se hace antes de cargar el modelo al pasar el banco: leerlo y probar dónde se
/// comprobará, para que lo que impida comprobar se sepa sin haber pasado la tanda.
fn cargar_y_probar(carpeta_banco: &Path) -> Resultado<(ConfigBanco, Vec<Encargo>)> {
    let (banco, encargos) = cargar_banco(carpeta_banco)?;
    probar_el_lugar(&elegir_lugar(&banco, &encargos), carpeta_banco)?;
    Ok((banco, encargos))
}

/// `local` con el intérprete que se usará en este equipo: el que encuentre junto a Git y, si
/// comprueba con integridad baja, el de la copia. Dice en la terminal lo que haya que avisar.
fn interprete_de_este_equipo(
    local: &ComprobacionLocal,
    carpeta_banco: &Path,
) -> Resultado<ComprobacionLocal> {
    let (local, aviso) = interprete::en_este_equipo(local);
    let (local, aviso_de_la_copia) = copia_del_interprete::con_la_copia(&local, carpeta_banco)?;
    for aviso in [aviso, aviso_de_la_copia].into_iter().flatten() {
        println!("{aviso}");
    }
    Ok(local)
}

/// La carpeta del banco al que pertenece una tanda, que vive en `<banco>/resultados/<tanda>`.
pub(super) fn carpeta_del_banco(carpeta_de_la_tanda: &Path) -> Resultado<PathBuf> {
    carpeta_de_la_tanda
        .parent()
        .filter(|resultados| {
            resultados
                .file_name()
                .is_some_and(|n| n == CARPETA_DE_RESULTADOS)
        })
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            Error::Banco(format!(
                "«{}» no está en la carpeta «{CARPETA_DE_RESULTADOS}» de un banco",
                carpeta_de_la_tanda.display()
            ))
        })
}

/// La fase 2: las pruebas ocultas en el lugar elegido, si lo hay, y la puntuación.
fn comprobar_y_puntuar(
    pasados: &[Pasado],
    datos: DatosDeTanda,
    lugar: LugarElegido,
    carpeta: &Path,
) -> Resultado<ResultadosDeTanda> {
    match lugar {
        LugarElegido::Ninguno => {
            let margen = datos.margen_lineas;
            puntuar_tanda(pasados, datos, margen, None, carpeta)
        }
        LugarElegido::Local(local) => {
            println!(
                "{}",
                mensajes::mensaje_comprobando_en_local(&carpeta.join(CARPETA_DE_COMPROBACION))
            );
            let entorno_del_sistema = std::env::vars().collect();
            let local = interprete_de_este_equipo(local, &carpeta_del_banco(carpeta)?)?;
            if local.aislamiento == Aislamiento::Ninguno {
                println!("{}", mensajes::mensaje_sin_aislar());
            }
            let comprobador = Comprobador::nuevo(CarpetaLocal::empezar(
                &local,
                carpeta,
                LanzadorLocal::para(&local),
                &entorno_del_sistema,
            )?);
            comprobar_en(comprobador, pasados, datos, carpeta)
        }
    }
}

/// Puntúa la tanda comprobando en un lugar ya preparado, y lo deja como estaba.
fn comprobar_en<G: LugarDeEjecucion>(
    mut comprobador: Comprobador<G>,
    pasados: &[Pasado],
    datos: DatosDeTanda,
    carpeta: &Path,
) -> Resultado<ResultadosDeTanda> {
    let margen = datos.margen_lineas;
    // Se dice en el acto: el 12/11/2026 faltaron todas por un fallo del adaptador y la tanda
    // terminó sin avisar, con todo el código «no evaluado».
    let ausentes = comprobador.herramientas_ausentes();
    if !ausentes.is_empty() {
        eprintln!("{}", mensajes::mensaje_herramientas_ausentes(&ausentes));
    }
    let mut pruebas = |encargo: &Encargo, montada: &Path| {
        comprobador.comprobar_y_vigilar(&encargo.ficha, montada)
    };
    let puntuada = puntuar_tanda(pasados, datos, margen, Some(&mut pruebas), carpeta);
    // Deja el lugar como estaba también si puntuar falló.
    let terminada = comprobador.terminar();
    let mut puntuada = puntuada?;
    let rastro: Vec<String> = terminada?.iter().map(rastro::describir).collect();
    if !rastro.is_empty() {
        eprintln!("{}", mensajes::mensaje_rastro_fuera(&rastro));
    }
    puntuada.tanda.rastro_fuera = rastro;
    Ok(puntuada)
}

/// La fase 2 y los resultados en disco. La usan `--banco` y `--banco-comprobar`.
///
/// Si algo falla aquí, la fase 1 ya está en `pasadas.toml`, y el error dice con qué orden se
/// termina la tanda sin volver a pasar los encargos por el modelo.
pub fn cerrar_tanda(
    pasados: &[Pasado],
    datos: DatosDeTanda,
    lugar: LugarElegido,
    carpeta: &Path,
) -> Resultado<ResultadosDeTanda> {
    comprobar_y_puntuar(pasados, datos, lugar, carpeta)
        .and_then(|resultados| {
            guardar(&resultados, carpeta)?;
            Ok(resultados)
        })
        .map_err(|fallo| Error::Banco(mensajes::mensaje_tanda_sin_cerrar(&fallo, carpeta)))
}

fn contar_el_fin(resultados: &ResultadosDeTanda, carpeta: &Path) {
    println!(
        "{}",
        mensajes::mensaje_de_fin(
            &carpeta.join(NOMBRE_RESULTADOS),
            &carpeta.join(NOMBRE_INFORME),
            resultados.pendientes(),
        )
    );
}

/// `programator --banco <carpeta>`: la tanda entera con el motor de verdad.
pub fn ejecutar(config: &Config, carpeta_banco: &Path, version: &str) -> Resultado<()> {
    let (banco, encargos) = cargar_y_probar(carpeta_banco)?;
    let lugar = elegir_lugar(&banco, &encargos);
    let pasada = pasar_con_el_motor(config, carpeta_banco, &banco, &encargos, &lugar, version)?;
    let resultados = cerrar_tanda(&pasada.pasados, pasada.datos, lugar, &pasada.carpeta)?;
    contar_el_fin(&resultados, &pasada.carpeta);
    Ok(())
}

/// Solo la fase 1 de una tanda: los encargos con el motor, guardados en disco. Devuelve la carpeta
/// de la tanda, que la fase 2 retoma con [`comprobar_guardada`].
pub fn pasar(config: &Config, carpeta_banco: &Path, version: &str) -> Resultado<PathBuf> {
    let (banco, encargos) = cargar_y_probar(carpeta_banco)?;
    let lugar = elegir_lugar(&banco, &encargos);
    pasar_con_el_motor(config, carpeta_banco, &banco, &encargos, &lugar, version)
        .map(|pasada| pasada.carpeta)
}

/// Cómo lleva el arnés al modelo durante la fase 1: con las mismas instrucciones y límites que el
/// ciclo, pero sin ejecutar su código y guardando siempre la conversación.
struct ArnesDelBanco {
    verificacion: Verificacion,
    limites: Limites,
    preambulo: PathBuf,
    recordatorio: PathBuf,
    normas: String,
}

impl ArnesDelBanco {
    fn de_la_config(config: &Config) -> Resultado<Self> {
        // Mientras se pasa, el código del modelo no se ejecuta: sin comprobadores en la
        // verificación. Lo que se ejecuta son las pruebas ocultas de la fase 2, con las reglas de
        // su lugar.
        let verificacion = Verificacion::default();
        let plantilla = leer(&config.resolver(&config.agente.plantilla_normas))?;
        let normas = normas::componer(&plantilla, &verificacion.comprobadores);
        Ok(Self {
            limites: Limites::de_la_config(&config.ciclo),
            preambulo: config.resolver(&config.agente.preambulo),
            recordatorio: config.resolver(&config.agente.recordatorio),
            verificacion,
            normas,
        })
    }

    /// El banco guarda siempre la conversación de cada pasada: una nota sin el porqué no sirve
    /// para corregir nada (el bucle de reescrituras del 11/11/2026 no se pudo explicar sin ella).
    fn ajustes<'a>(&'a self, config: &'a Config) -> Ajustes<'a> {
        Ajustes {
            registro_de_conversacion: true,
            ..Ajustes::de_la_config(
                config,
                &self.limites,
                &self.verificacion,
                &self.preambulo,
                &self.recordatorio,
            )
        }
    }
}

/// Lo que deja la fase 1 para la fase 2.
struct TandaPasada<'a> {
    carpeta: PathBuf,
    pasados: Vec<Pasado<'a>>,
    datos: DatosDeTanda,
}

/// La fase 1: carga el motor, pasa cada encargo con cada semilla, lo guarda en disco y apaga el
/// motor si lo arrancó.
fn pasar_con_el_motor<'a>(
    config: &Config,
    carpeta_banco: &Path,
    banco: &ConfigBanco,
    encargos: &'a [Encargo],
    lugar: &LugarElegido,
    version: &str,
) -> Resultado<TandaPasada<'a>> {
    let config = &banco.para_la_tanda(config);
    let Some(modelo) = &config.motor.modelo else {
        return Err(Error::Configuracion(
            "el banco necesita «motor.modelo» para saber qué modelo mide".into(),
        ));
    };
    let ruta_modelo = config.resolver(modelo);
    let nombre_modelo = ruta_modelo
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| modelo.clone());

    let ahora = chrono::Local::now();
    let carpeta = carpeta_banco.join(CARPETA_DE_RESULTADOS).join(format!(
        "{}-{nombre_modelo}",
        ahora.format(FORMATO_DE_LA_CARPETA_DE_LA_TANDA)
    ));
    let trabajo = carpeta.join(CARPETA_DE_TRABAJO);
    crear_carpeta(&trabajo)?;
    println!(
        "{}",
        mensajes::mensaje_de_inicio(&mensajes::Inicio {
            encargos: encargos.len(),
            semillas: &banco.semillas,
            carpeta_de_comprobacion: lugar.para_contar(&carpeta.join(CARPETA_DE_COMPROBACION)),
            hay_codigo_que_comprobar: hay_codigo_que_comprobar(encargos),
            resultados: &carpeta,
        })
    );

    eprintln!("{}", mensajes::mensaje_calculando_huella(&ruta_modelo));
    let huella = huella_sha256(&ruta_modelo)?;
    let binario = config.motor.binario.as_ref().map(|b| config.resolver(b));
    let compilacion = compilacion_del_motor(binario.as_deref());

    let muestreo = |semilla: i64| Muestreo {
        semilla,
        temperatura: banco.temperatura,
        ..config.muestreo.clone()
    };
    let tope = Duration::from_secs(banco.tiempo_maximo_encargo_segundos);
    let mut servidor = None;
    let carga = cargar_midiendo(
        config,
        &muestreo(config.muestreo.semilla),
        &mut servidor,
        tope,
    )?;
    if let Some(carga) = &carga {
        println!(
            "{}",
            mensajes::mensaje_motor_cargado(carga.capas, carga.vram_mib, carga.duracion)
        );
    }

    let datos = DatosDeTanda {
        modelo: nombre_modelo,
        huella_sha256: huella,
        compilacion_motor: compilacion,
        version_programator: version.to_string(),
        fecha: ahora.format(FORMATO_DE_LA_FECHA_DE_LA_TANDA).to_string(),
        semillas: banco.semillas.clone(),
        temperatura: decimal_de_la_configuracion(banco.temperatura),
        margen_lineas: banco.margen_lineas,
        tiempo_maximo_encargo_segundos: banco.tiempo_maximo_encargo_segundos,
        maquina_de_comprobacion: lugar.nombre(),
        capas_en_gpu: carga.as_ref().map(|c| c.capas),
        vram_ocupada_mib: carga.as_ref().and_then(|c| c.vram_mib),
        segundos_de_carga: carga.as_ref().map(|c| c.duracion.as_secs_f64()),
        ajustes_del_motor: Some(ajustes_del_motor(config)),
        tarjeta: carga.as_ref().and_then(|c| c.tarjeta.clone()),
        sistema_de_comprobacion: lugar.sistema(std::env::consts::OS),
        rastro_fuera: Vec::new(),
        aislamiento: lugar.aislamiento(),
    };
    // Lo de la GPU va a disco en cuanto se mide: si la fase 2 falla, no se pierde.
    let mut guardadas = PasadasDeTanda {
        tanda: datos.clone(),
        pasadas: Vec::new(),
    };
    pasadas::guardar(&guardadas, &carpeta)?;

    let arnes = ArnesDelBanco::de_la_config(config)?;
    let ajustes = arnes.ajustes(config);
    let preparacion = Preparacion {
        agente: &config.agente.nombre,
        buzon_que_encarga: &banco.buzon_que_encarga,
        normas: &arnes.normas,
        tiempo_maximo: tope,
    };

    let pasados = pasar_encargos(
        &Recorrido {
            encargos,
            semillas: &banco.semillas,
            carpeta_de_trabajo: &trabajo,
        },
        &preparacion,
        &ajustes,
        &mut |semilla| asegurar_motor_con_muestreo(config, &muestreo(semilla), &mut servidor),
        &mut RelojReal::desde_ahora(),
        &mut |linea| println!("{linea}"),
        &mut |pasado| {
            guardadas.pasadas.push(PasadaGuardada::de(pasado));
            pasadas::guardar(&guardadas, &carpeta)
        },
    )?;
    // Se apaga antes de las pruebas, si lo arrancó el banco: la tarjeta ya no hace falta.
    drop(servidor.take());

    Ok(TandaPasada {
        carpeta,
        pasados,
        datos,
    })
}

/// La fase 2 de una tanda cuya fase 1 quedó en disco, sin motor ni modelo.
pub fn comprobar_guardada(
    carpeta_de_la_tanda: &Path,
    agente: &str,
) -> Resultado<ResultadosDeTanda> {
    let guardadas = pasadas::cargar(carpeta_de_la_tanda)?;
    let (banco, encargos) = cargar_banco(&carpeta_del_banco(carpeta_de_la_tanda)?)?;
    let lugar = elegir_lugar(&banco, &encargos);
    let pasados = pasadas::recuperar(
        &guardadas.pasadas,
        &encargos,
        &carpeta_de_la_tanda.join(CARPETA_DE_TRABAJO),
        agente,
    )?;
    println!(
        "{}",
        mensajes::mensaje_comprobando_guardada(pasados.len(), carpeta_de_la_tanda)
    );
    let datos = DatosDeTanda {
        maquina_de_comprobacion: lugar.nombre(),
        sistema_de_comprobacion: lugar.sistema(std::env::consts::OS),
        aislamiento: lugar.aislamiento(),
        ..guardadas.tanda
    };
    cerrar_tanda(&pasados, datos, lugar, carpeta_de_la_tanda)
}

/// `programator --banco-comprobar <carpeta de la tanda>`: solo la fase 2.
pub fn comprobar(config: &Config, carpeta_de_la_tanda: &Path) -> Resultado<()> {
    let resultados = comprobar_guardada(carpeta_de_la_tanda, &config.agente.nombre)?;
    contar_el_fin(&resultados, carpeta_de_la_tanda);
    Ok(())
}

/// `programator --banco-informe <resultados.toml>`.
pub fn rehacer(ruta_resultados: &Path) -> Resultado<()> {
    let pendientes = rehacer_informe(ruta_resultados)?;
    println!(
        "{}",
        mensajes::mensaje_informe_rehecho(ruta_resultados, pendientes)
    );
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::ficha::FichaEncargo;
    use crate::banco::resultados::{leer_toml, FilaDeDesempeno};
    use crate::motor::{MotorDoble, Respuesta, SolicitudHerramienta};
    use std::collections::VecDeque;

    struct RelojFalso(Duration);

    impl Reloj for RelojFalso {
        fn transcurrido(&self) -> Duration {
            self.0
        }
        fn esperar(&mut self, cuanto: Duration) {
            self.0 += cuanto;
        }
    }

    /// El encargo, también en disco, como lo lee `cargar_encargos`.
    fn encargo(dir: &Path, texto_ficha: &str, rubrica: Option<&str>) -> Encargo {
        let ficha: FichaEncargo = toml::from_str(texto_ficha).unwrap();
        let carpeta = dir.join(CARPETA_DE_ENCARGOS).join(&ficha.id);
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(carpeta.join(crate::banco::ficha::NOMBRE_FICHA), texto_ficha).unwrap();
        std::fs::write(carpeta.join("enunciado.md"), "Haz el encargo.\n").unwrap();
        if let Some(rubrica) = rubrica {
            std::fs::write(carpeta.join(NOMBRE_RUBRICA), rubrica).unwrap();
        }
        Encargo { ficha, carpeta }
    }

    fn criterio(dir: &Path) -> Encargo {
        encargo(
            dir,
            "id = \"E01\"\ntitulo = \"Protocolo\"\nfamilia = \"criterio\"\n",
            Some("- **5**: lo explica todo\n"),
        )
    }

    fn formas(dir: &Path) -> Encargo {
        let partida = dir.join(CARPETA_DE_ENCARGOS).join("U01").join("partida");
        std::fs::create_dir_all(&partida).unwrap();
        std::fs::write(partida.join("Pedidos.cs"), "class Pedidos {}\n").unwrap();
        encargo(
            dir,
            "id = \"U01\"\ntitulo = \"Formas\"\nfamilia = \"uso\"\nclase = \"formas\"\n\
             auditado = \"Pedidos.cs\"\n\
             [[defectos]]\nlineas = [10]\ntipo = \"NUMERO_MAGICO\"\ndescripcion = \"un 0.21\"\n\
             [[defectos]]\nlineas = [40]\ntipo = \"METODO_LARGO\"\ndescripcion = \"Procesar\"\n",
            None,
        )
    }

    fn oficio(dir: &Path) -> Encargo {
        encargo(
            dir,
            "id = \"E03\"\ntitulo = \"Rangos\"\nfamilia = \"oficio\"\nentrega = \"rangos.rs\"\n\
             colocar_en = \"src/rangos.rs\"\nherramientas = [\"rust\"]\n\
             [[comprobaciones]]\nnombre = \"basico\"\norden = \"cargo test\"\n\
             [[texto]]\nnombre = \"sin_unwrap\"\npatron = \"unwrap()\"\nmaximo = 0\n",
            None,
        )
    }

    fn datos() -> DatosDeTanda {
        DatosDeTanda {
            modelo: "doble".into(),
            huella_sha256: "00".repeat(32),
            compilacion_motor: COMPILACION_DESCONOCIDA.into(),
            version_programator: "0.11.0".into(),
            fecha: "2026-11-11 10:00".into(),
            semillas: vec![42],
            temperatura: 0.2,
            margen_lineas: 2,
            tiempo_maximo_encargo_segundos: 900,
            maquina_de_comprobacion: None,
            capas_en_gpu: None,
            vram_ocupada_mib: None,
            segundos_de_carga: None,
            tarjeta: None,
            sistema_de_comprobacion: None,
            rastro_fuera: Vec::new(),
            aislamiento: None,
            ajustes_del_motor: None,
        }
    }

    /// La tanda vive donde la pone `ejecutar`: en `<banco>/resultados/<tanda>`, con el banco en `dir`.
    fn carpeta_tanda(dir: &Path) -> PathBuf {
        dir.join(CARPETA_DE_RESULTADOS).join("tanda")
    }

    fn preparacion() -> Preparacion<'static> {
        Preparacion {
            agente: "Programator",
            buzon_que_encarga: "banco",
            normas: "NORMAS\n",
            tiempo_maximo: Duration::from_secs(900),
        }
    }

    /// Pasa los encargos con un motor doble por encargo, en orden, y una semilla.
    fn pasar<'a>(
        dir: &Path,
        encargos: &'a [Encargo],
        guiones: Vec<Vec<Respuesta>>,
    ) -> Vec<Pasado<'a>> {
        let mut motores: VecDeque<MotorDoble> =
            guiones.into_iter().map(MotorDoble::con_guion).collect();
        let mut semillas_pedidas = Vec::new();
        let limites = Limites::default();
        let verificacion = Verificacion::default();
        let ajustes = Ajustes::con_limites(&limites, &verificacion);
        let mut avisos = Vec::new();
        let tanda = carpeta_tanda(dir);
        let trabajo = tanda.join(CARPETA_DE_TRABAJO);
        let mut guardadas = PasadasDeTanda {
            tanda: datos(),
            pasadas: Vec::new(),
        };
        let pasados = pasar_encargos(
            &Recorrido {
                encargos,
                semillas: &[42],
                carpeta_de_trabajo: &trabajo,
            },
            &preparacion(),
            &ajustes,
            &mut |semilla| {
                semillas_pedidas.push(semilla);
                let motor = motores.pop_front().unwrap();
                Ok(EstadoMotor::Disponible(Box::new(motor)))
            },
            &mut RelojFalso(Duration::ZERO),
            &mut |linea| avisos.push(linea),
            &mut |pasado| {
                guardadas.pasadas.push(PasadaGuardada::de(pasado));
                pasadas::guardar(&guardadas, &tanda)
            },
        )
        .unwrap();
        assert!(semillas_pedidas.iter().all(|&s| s == 42));
        assert_eq!(avisos.len(), encargos.len());
        pasados
    }

    fn pedir(nombre: &str, argumentos: serde_json::Value) -> Respuesta {
        Respuesta::Herramienta(SolicitudHerramienta {
            nombre: nombre.to_string(),
            argumentos,
        })
    }

    #[test]
    fn una_tanda_completa_sin_comprobacion_deja_un_toml_y_un_informe_validos() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![criterio(dir.path()), formas(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![
                vec![Respuesta::Texto("El protocolo dice esto.".into())],
                vec![Respuesta::Texto(
                    "NUMERO_MAGICO | Pedidos.cs | Calcular | 11 | 0.21 | sin nombre\n\
                     METODO_LARGO | Pedidos.cs | Otro | 90 | Otro | inventado"
                        .into(),
                )],
            ],
        );
        let carpeta = carpeta_tanda(dir.path());

        let resultados = puntuar_tanda(&pasados, datos(), 2, None, &carpeta).unwrap();
        guardar(&resultados, &carpeta).unwrap();

        let leido =
            leer_toml(&std::fs::read_to_string(carpeta.join(NOMBRE_RESULTADOS)).unwrap()).unwrap();
        assert_eq!(leido, resultados);
        let rubrica = &leido.encargos[0].semillas[0];
        assert_eq!(rubrica.estado, EstadoDeNota::Pendiente);
        let fichero = carpeta.join(rubrica.rubrica.as_ref().unwrap());
        assert!(std::fs::read_to_string(fichero)
            .unwrap()
            .contains("El protocolo dice esto."));
        let auditoria = &leido.encargos[1].semillas[0];
        assert_eq!(auditoria.aciertos, Some(1));
        assert_eq!(auditoria.inventados, Some(1));
        // 5 · 1 acierto / (2 sembrados + 1 inventado) = 1,67, a medio punto: 1,5.
        assert_eq!(auditoria.nota, Some(1.5));
        let formas = leido
            .clases
            .iter()
            .find(|c| c.fila == FilaDeDesempeno::Formas)
            .unwrap();
        assert_eq!(formas.nota_media, Some(1.5));
        let informe = std::fs::read_to_string(carpeta.join(NOMBRE_INFORME)).unwrap();
        assert!(informe.contains("Quedan 1 rúbricas por puntuar"));
    }

    #[test]
    fn con_las_pruebas_apagadas_una_entrega_de_oficio_sale_no_evaluada_y_no_con_cero() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![
                pedir(
                    "escribir_propuesta",
                    serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                ),
                Respuesta::Texto("Entregado.".into()),
            ]],
        );

        let semilla = puntuar(&pasados[0], 2, None, dir.path()).unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::NoEvaluado);
        let detalle = semilla.detalle.unwrap();
        assert!(detalle.contains("no se ha ejecutado"), "{detalle}");
        assert!(detalle.contains("escrito y guardado"), "{detalle}");
        assert!(detalle.contains("apagadas"), "{detalle}");
    }

    #[test]
    fn con_las_pruebas_apagadas_el_motivo_nombra_la_seccion_que_sigue_existiendo() {
        // Desde el 13/11/2026 no hay máquina aparte: la sección que falta es [comprobacion_local], y nombrar la
        // vieja [comprobacion] mandaría a escribir una sección que hace que el banco no arranque.
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![
                pedir(
                    "escribir_propuesta",
                    serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                ),
                Respuesta::Texto("Entregado.".into()),
            ]],
        );

        let detalle = puntuar(&pasados[0], 2, None, dir.path())
            .unwrap()
            .detalle
            .unwrap();

        assert!(detalle.contains("[comprobacion_local]"), "{detalle}");
    }

    #[test]
    fn con_pruebas_se_monta_la_carpeta_y_se_suman_los_requisitos_de_texto() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        std::fs::create_dir_all(encargos[0].carpeta.join("pruebas/tests")).unwrap();
        std::fs::write(encargos[0].carpeta.join("pruebas/tests/t.rs"), "").unwrap();
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![
                pedir(
                    "escribir_propuesta",
                    serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                ),
                Respuesta::Texto("Entregado.".into()),
            ]],
        );
        let mut montadas = Vec::new();
        let mut pruebas = |_: &Encargo, montada: &Path| {
            montadas.push(montada.to_path_buf());
            Comprobado {
                resultado: ResultadoDePruebas::Compila {
                    superadas: vec![],
                    fallidas: vec!["basico".into()],
                },
                intentos_de_red: vec!["pypi.org:443".into()],
            }
        };

        let semilla = puntuar(&pasados[0], 2, Some(&mut pruebas), dir.path()).unwrap();

        assert_eq!(semilla.intentos_de_red, ["pypi.org:443"]);
        assert_eq!(semilla.comprobaciones_superadas, Some(1));
        assert_eq!(semilla.comprobaciones_total, Some(2));
        assert_eq!(semilla.nota, Some(2.5));
        assert!(montadas[0].join("src/rangos.rs").is_file());
        assert!(montadas[0].join("tests/t.rs").is_file());
        assert!(montadas[0].ends_with("E03-semilla-42-pruebas"));
    }

    #[test]
    fn sin_entrega_el_oficio_saca_un_cero_sin_lanzar_pruebas() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![Respuesta::Texto("No sé hacerlo.".into())]],
        );
        let mut lanzadas = 0;
        let mut pruebas = |_: &Encargo, _: &Path| {
            lanzadas += 1;
            ResultadoDePruebas::NoEvaluado("no".into()).into()
        };

        let semilla = puntuar(&pasados[0], 2, Some(&mut pruebas), dir.path()).unwrap();

        assert_eq!(semilla.nota, Some(0.0));
        assert!(semilla.detalle.unwrap().contains("no entregó rangos.rs"));
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::SinEntrega));
        assert_eq!(lanzadas, 0);
    }

    /// Un encargo de oficio con rúbrica, para valorarlo leyendo cuando las pruebas no dan nota.
    fn oficio_con_rubrica(dir: &Path) -> Encargo {
        let encargo = oficio(dir);
        std::fs::write(
            encargo.carpeta.join(NOMBRE_RUBRICA),
            "- **5**: cubre los casos\n",
        )
        .unwrap();
        // Las pruebas ocultas se montan desde aquí, aunque el doble no las ejecute.
        std::fs::create_dir_all(encargo.carpeta.join("pruebas")).unwrap();
        encargo
    }

    /// Pasa un encargo de oficio que entrega `rangos.rs` con `pub fn r() {}`.
    fn pasar_entregando<'a>(dir: &Path, encargos: &'a [Encargo]) -> Vec<Pasado<'a>> {
        pasar(
            dir,
            encargos,
            vec![vec![
                pedir(
                    "escribir_propuesta",
                    serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                ),
                Respuesta::Texto("Entregado.".into()),
            ]],
        )
    }

    #[test]
    fn con_las_pruebas_apagadas_y_rubrica_el_codigo_se_deja_para_valorarlo_leyendo() {
        // Sin pruebas ocultas, el código lo valora quien dirige
        // con la rúbrica, como los encargos de criterio, y no se queda en «no evaluado».
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);

        let semilla = puntuar(&pasados[0], 2, None, dir.path()).unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::Pendiente);
        assert_eq!(semilla.nota, None);
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::Rubrica));
        let ruta = dir.path().join(nombre_del_fichero("E03", 42));
        assert_eq!(
            semilla.rubrica.as_deref(),
            Some(format!("{CARPETA_DE_RUBRICAS}/{}", nombre_del_fichero("E03", 42)).as_str())
        );
        let fichero = std::fs::read_to_string(ruta).unwrap();
        assert!(fichero.contains("```\npub fn r() {}\n```"), "{fichero}");
        assert!(fichero.contains("**sin ejecutarlo**"), "{fichero}");
        assert!(
            fichero.contains("apagadas"),
            "el motivo va en el fichero:\n{fichero}"
        );
        assert!(fichero.contains("- **5**: cubre los casos"), "{fichero}");
    }

    #[test]
    fn una_rubrica_de_codigo_ya_puntuada_da_su_nota_y_dice_que_es_de_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);
        std::fs::write(
            dir.path().join(nombre_del_fichero("E03", 42)),
            "# E03\n\nNota: 4\n",
        )
        .unwrap();

        let semilla = puntuar(&pasados[0], 2, None, dir.path()).unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::Evaluada);
        assert_eq!(semilla.nota, Some(4.0));
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::Rubrica));
    }

    #[test]
    fn si_las_pruebas_no_se_pueden_ejecutar_tambien_se_valora_con_la_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);
        let mut pruebas = |_: &Encargo, _: &Path| {
            ResultadoDePruebas::NoEvaluado("rust no está instalado en este equipo".into()).into()
        };

        let semilla = puntuar(&pasados[0], 2, Some(&mut pruebas), dir.path()).unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::Pendiente);
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::Rubrica));
        let fichero =
            std::fs::read_to_string(dir.path().join(nombre_del_fichero("E03", 42))).unwrap();
        assert!(
            fichero.contains("rust no está instalado en este equipo"),
            "{fichero}"
        );
    }

    #[test]
    fn si_las_pruebas_dan_nota_manda_esa_y_no_se_deja_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);
        let mut pruebas = |_: &Encargo, _: &Path| {
            ResultadoDePruebas::Compila {
                superadas: vec!["basico".into()],
                fallidas: vec![],
            }
            .into()
        };

        let semilla = puntuar(&pasados[0], 2, Some(&mut pruebas), dir.path()).unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::Evaluada);
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::PruebasOcultas));
        assert_eq!(semilla.rubrica, None);
        assert!(!dir.path().join(nombre_del_fichero("E03", 42)).exists());
    }

    #[test]
    fn lo_que_no_compila_es_nota_de_las_pruebas_y_no_va_a_la_rubrica() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);
        let mut pruebas =
            |_: &Encargo, _: &Path| ResultadoDePruebas::NoCompila("error[E0425]".into()).into();

        let semilla = puntuar(&pasados[0], 2, Some(&mut pruebas), dir.path()).unwrap();

        assert_eq!(semilla.nota, Some(0.0));
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::PruebasOcultas));
        assert!(!dir.path().join(nombre_del_fichero("E03", 42)).exists());
    }

    #[test]
    fn sin_rubrica_el_motivo_dice_que_falta_para_valorarlo_leyendo() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio(dir.path())];
        let pasados = pasar_entregando(dir.path(), &encargos);

        let semilla = puntuar(&pasados[0], 2, None, dir.path()).unwrap();

        assert_eq!(semilla.estado, EstadoDeNota::NoEvaluado);
        assert_eq!(semilla.puntuada_con, None);
        let detalle = semilla.detalle.unwrap();
        assert!(detalle.contains("apagadas"), "{detalle}");
        assert!(detalle.contains(NOMBRE_RUBRICA), "{detalle}");
    }

    #[test]
    fn sin_entrega_no_se_deja_rubrica_y_se_dice_que_no_entrego() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![oficio_con_rubrica(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![Respuesta::Texto("No sé hacerlo.".into())]],
        );

        let semilla = puntuar(&pasados[0], 2, None, dir.path()).unwrap();

        assert_eq!(semilla.nota, Some(0.0));
        assert_eq!(semilla.puntuada_con, Some(PuntuadaCon::SinEntrega));
        assert!(!dir.path().join(nombre_del_fichero("E03", 42)).exists());
    }

    #[test]
    fn cada_pasada_queda_en_disco_en_cuanto_termina() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![criterio(dir.path()), oficio(dir.path())];
        pasar(
            dir.path(),
            &encargos,
            vec![
                vec![Respuesta::Texto("El protocolo dice esto.".into())],
                vec![
                    pedir(
                        "escribir_propuesta",
                        serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                    ),
                    pedir("publicar", serde_json::json!({"texto": "Entregado."})),
                ],
            ],
        );

        let guardadas = pasadas::cargar(&carpeta_tanda(dir.path())).unwrap();

        assert_eq!(guardadas.tanda, datos());
        assert_eq!(guardadas.pasadas.len(), 2);
        assert_eq!(guardadas.pasadas[1].encargo, "E03");
        assert_eq!(guardadas.pasadas[1].entregas, vec!["rangos.rs".to_string()]);
        assert!(guardadas.pasadas.iter().all(|p| p.a_tiempo));
        let texto = pasadas::componer_toml(&guardadas).unwrap();
        assert_eq!(pasadas::leer_toml(&texto).unwrap(), guardadas);
    }

    #[test]
    fn comprobar_una_tanda_guardada_da_las_mismas_notas_que_la_tanda_completa() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![criterio(dir.path()), formas(dir.path()), oficio(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![
                vec![Respuesta::Texto("El protocolo dice esto.".into())],
                vec![Respuesta::Texto(
                    "NUMERO_MAGICO | Pedidos.cs | Calcular | 11 | 0.21 | sin nombre".into(),
                )],
                vec![
                    pedir(
                        "escribir_propuesta",
                        serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                    ),
                    pedir("publicar", serde_json::json!({"texto": "Entregado."})),
                ],
            ],
        );
        let carpeta = carpeta_tanda(dir.path());
        let completa = cerrar_tanda(&pasados, datos(), LugarElegido::Ninguno, &carpeta).unwrap();
        std::fs::remove_file(carpeta.join(NOMBRE_RESULTADOS)).unwrap();
        // Como la tanda completa, sin ejecutar código: desde el 13/11/2026 las pruebas ocultas van
        // encendidas por defecto, y una prueba no puede lanzar nada de verdad.
        std::fs::write(
            dir.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\nactiva = false\n",
        )
        .unwrap();

        let aparte = comprobar_guardada(&carpeta, "Programator").unwrap();

        assert_eq!(aparte, completa);
        assert!(carpeta.join(NOMBRE_RESULTADOS).is_file());
    }

    /// El 12/11/2026, `--banco-comprobar` sobre una tanda con las rúbricas ya puntuadas las
    /// escribió de nuevo en blanco y se perdieron las 21 notas puestas a mano (en una copia).
    #[test]
    fn comprobar_de_nuevo_no_pisa_las_rubricas_ya_puntuadas() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![criterio(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![Respuesta::Texto("El protocolo dice esto.".into())]],
        );
        let carpeta = carpeta_tanda(dir.path());
        let primera = cerrar_tanda(&pasados, datos(), LugarElegido::Ninguno, &carpeta).unwrap();
        assert_eq!(primera.pendientes(), 1);
        let rubrica = carpeta
            .join(CARPETA_DE_RUBRICAS)
            .join(nombre_del_fichero("E01", 42));
        let puntuada = std::fs::read_to_string(&rubrica)
            .unwrap()
            .trim_end()
            .to_string()
            + " 4\n";
        std::fs::write(&rubrica, &puntuada).unwrap();

        let otra_vez = comprobar_guardada(&carpeta, "Programator").unwrap();

        assert_eq!(std::fs::read_to_string(&rubrica).unwrap(), puntuada);
        assert_eq!(otra_vez.pendientes(), 0);
        assert_eq!(otra_vez.encargos[0].semillas[0].nota, Some(4.0));
    }

    #[test]
    fn si_falla_la_fase_2_quedan_las_pasadas_y_se_dice_como_terminar() {
        let dir = tempfile::tempdir().unwrap();
        let encargos = vec![criterio(dir.path())];
        let pasados = pasar(
            dir.path(),
            &encargos,
            vec![vec![Respuesta::Texto("El protocolo dice esto.".into())]],
        );
        let carpeta = carpeta_tanda(dir.path());
        // Una carpeta donde va el fichero de resultados: guardarlos falla.
        std::fs::create_dir_all(carpeta.join(NOMBRE_RESULTADOS)).unwrap();

        let fallo = cerrar_tanda(&pasados, datos(), LugarElegido::Ninguno, &carpeta)
            .unwrap_err()
            .to_string();

        assert!(fallo.contains("--banco-comprobar"), "{fallo}");
        assert!(fallo.contains(pasadas::NOMBRE_PASADAS), "{fallo}");
        assert!(!fallo.contains("--maquina-autorizada"), "{fallo}");
        assert!(carpeta.join(pasadas::NOMBRE_PASADAS).is_file());
    }

    #[test]
    fn una_carpeta_fuera_de_resultados_no_se_toma_por_una_tanda() {
        let dir = tempfile::tempdir().unwrap();
        let fallo = carpeta_del_banco(&dir.path().join("otra").join("t1")).unwrap_err();
        assert!(fallo.to_string().contains("resultados"), "{fallo}");
        assert_eq!(
            carpeta_del_banco(&carpeta_tanda(dir.path())).unwrap(),
            dir.path()
        );
    }

    #[test]
    fn la_temperatura_de_la_configuracion_se_guarda_como_se_escribio() {
        assert_eq!(decimal_de_la_configuracion(0.2), 0.2);
    }

    /// Un encargo de oficio en PHP, un lenguaje que el banco no trae de serie.
    fn oficio_en_php(dir: &Path) -> Encargo {
        encargo(
            dir,
            "id = \"P01\"\ntitulo = \"Rangos en PHP\"\nfamilia = \"oficio\"\n\
             entrega = \"rangos.php\"\ncolocar_en = \"rangos.php\"\nherramientas = [\"php\"]\n\
             [[comprobaciones]]\nnombre = \"basico\"\norden = \"php pruebas.php\"\n",
            None,
        )
    }

    #[test]
    fn un_lenguaje_sin_orden_para_el_banco_antes_de_cargar_el_modelo_y_dice_que_anadir() {
        let dir = tempfile::tempdir().unwrap();
        oficio_en_php(dir.path());
        oficio(dir.path());
        std::fs::write(dir.path().join(NOMBRE_BANCO_TOML), "[comprobacion_local]\n").unwrap();

        let fallo = cargar_banco(dir.path()).unwrap_err().to_string();
        // Sin banco.toml van encendidas, así que también se exige.
        std::fs::remove_file(dir.path().join(NOMBRE_BANCO_TOML)).unwrap();
        let sin_toml = cargar_banco(dir.path()).unwrap_err().to_string();
        assert_eq!(sin_toml, fallo);

        assert!(fallo.contains("P01 pide «php»"), "{fallo}");
        assert!(!fallo.contains("«rust»"), "rust es de serie: {fallo}");
        assert!(
            fallo.contains("[comprobacion_local.herramientas]"),
            "{fallo}"
        );
        assert!(fallo.contains("php = \"php --version\""), "{fallo}");
    }

    #[test]
    fn un_lenguaje_declarado_en_banco_toml_se_acepta() {
        let dir = tempfile::tempdir().unwrap();
        oficio_en_php(dir.path());
        std::fs::write(
            dir.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\n[comprobacion_local.herramientas]\nphp = \"php --version\"\n",
        )
        .unwrap();

        let (_, encargos) = cargar_banco(dir.path()).unwrap();

        assert_eq!(encargos.len(), 1);
    }

    #[test]
    fn con_las_pruebas_ocultas_apagadas_un_lenguaje_sin_orden_no_impide_pasar_el_banco() {
        // Apagadas no se ejecuta código: el modelo escribe el PHP igual, y lo que no se puede es
        // comprobar si funciona, que sale «no evaluado» con su motivo.
        let dir = tempfile::tempdir().unwrap();
        oficio_en_php(dir.path());
        std::fs::write(
            dir.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\nactiva = false\n",
        )
        .unwrap();

        let (_, encargos) = cargar_banco(dir.path()).unwrap();

        assert_eq!(encargos.len(), 1);
    }

    #[test]
    fn antes_de_cargar_el_modelo_se_prueba_donde_se_comprobara() {
        // La carpeta de resultados es un fichero: no se puede preparar la de comprobación, y eso
        // se sabe antes del motor. Si no se probara, `cargar_y_probar` saldría bien.
        let dir = tempfile::tempdir().unwrap();
        oficio_en_php(dir.path());
        std::fs::write(
            dir.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\naislamiento = \"ninguno\"\n\
             [comprobacion_local.herramientas]\nphp = \"php --version\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join(CARPETA_DE_RESULTADOS), "no es una carpeta").unwrap();

        assert!(cargar_banco(dir.path()).is_ok());
        assert!(cargar_y_probar(dir.path()).is_err());
    }

    #[test]
    fn pasar_y_ejecutar_el_banco_hacen_la_prueba_previa_antes_que_nada() {
        // Sin «motor.modelo», el motor diría que le falta: si sale el error de la prueba previa es
        // que se probó antes, y el motor no llega a arrancar nunca, se pruebe o no.
        let dir = tempfile::tempdir().unwrap();
        oficio_en_php(dir.path());
        std::fs::write(
            dir.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\naislamiento = \"ninguno\"\n\
             [comprobacion_local.herramientas]\nphp = \"php --version\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join(CARPETA_DE_RESULTADOS), "no es una carpeta").unwrap();
        let config: Config = toml::from_str("[motor]\n").unwrap();

        let fallos = [
            super::pasar(&config, dir.path(), "0.0.0")
                .unwrap_err()
                .to_string(),
            ejecutar(&config, dir.path(), "0.0.0")
                .unwrap_err()
                .to_string(),
        ];

        for fallo in fallos {
            assert!(
                fallo.contains(
                    crate::banco::comprobacion::prueba_previa::CARPETA_DE_LA_PRUEBA_PREVIA
                ),
                "{fallo}"
            );
        }
    }

    #[test]
    fn sin_codigo_que_comprobar_no_se_prueba_nada() {
        let dir = tempfile::tempdir().unwrap();
        oficio_en_php(dir.path());
        std::fs::write(
            dir.path().join(NOMBRE_BANCO_TOML),
            "[comprobacion_local]\nactiva = false\n",
        )
        .unwrap();
        std::fs::write(dir.path().join(CARPETA_DE_RESULTADOS), "no es una carpeta").unwrap();

        assert!(cargar_y_probar(dir.path()).is_ok());
    }
}
