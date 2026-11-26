//! Una pasada completa del ciclo, paso a paso: lee el canal, reparte lo que hay que atender,
//! prepara el motor de cada grupo, atiende y guarda el registro de encargos y el de lectura.

use super::atencion_del_modelo::atender_con_el_modelo;
use super::instantanea::{tomar_instantanea_inicial, Instantanea};
use super::preparaciones::{limpiar_las_copias, preparar_las_propuestas};
use super::publicacion::{self, construir_resumen_leido};
use super::reparto::{
    agrupar, buzones_a_medias, deltas_de, encargos_nuevos, encargos_para_el_modelo,
    separar_los_de_revision, Grupo,
};
use super::revisor::{atender_con_el_revisor, responder_lo_resuelto, Revisor};
use super::sin_atender::{cerrar_sin_modelo, dejar_pendiente, fecha_de_ahora};
use super::{Ajustes, DesenlacePasada, Espera, Motores, Preparacion, ResumenPasada};
use crate::arnes::cancelacion::{cancelar, es_cancelacion};
use crate::arnes::entregas::confirmacion::{confirmar, es_confirmacion, reconocer};
use crate::arnes::espacio::{
    carpeta_de_los_encargos, ruta_de_los_pendientes, ruta_del_latido, ruta_del_registro,
};
use crate::arnes::preparacion::pedido::es_preparacion;
use crate::arnes::registro_de_encargos::{disco, migracion, EstadoDelEncargo, RegistroDeEncargos};
use crate::error::Resultado;
use crate::protocolo::{Canal, Delta, Encargo, Latido, RegistroLectura, ResumenLeido};
use chrono::Local;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Lo que comparten los encargos de una pasada.
pub(super) struct Pasada<'a> {
    pub(super) carpeta: &'a Path,
    pub(super) agente: &'a str,
    pub(super) ajustes: &'a Ajustes<'a>,
    pub(super) canal: &'a Canal,
    pub(super) deltas: &'a [(String, Delta)],
    pub(super) leido: &'a ResumenLeido,
    /// Cuándo leyó esta pasada el canal, con la marca del latido: la hora del `RECIBIDO`.
    pub(super) recibido_a: &'a str,
    pub(super) ruta_latido: &'a Path,
}

/// Ejecuta una pasada completa del ciclo: lee el canal, anota en el registro de encargos los que
/// no tienen ficha, atiende todos los pendientes en orden de llegada y publica el resultado.
///
/// Lo nuevo se anota como pendiente y se guarda antes de atender nada: un corte a partir de ahí no
/// lo pierde, y la pasada siguiente lo reconoce por su identificador y no lo duplica.
///
/// El motor se prepara una vez por grupo de encargos con el mismo modelo, y solo si hay algún
/// encargo para el modelo. Si el primero todavía está cargando, la pasada espera a la vuelta
/// siguiente. Si es un grupo posterior el que no termina de cargar dentro de su tope, ese grupo y
/// los que siguen suman un intento y siguen pendientes: repetir la pasada entera respondería dos
/// veces a los ya atendidos.
///
/// Los encargos del `revisor` se atienden **después** de los del modelo: la revisión suelta el
/// motor para pasar el banco, y así no se lo quita a un encargo que espera en la misma pasada.
pub fn ejecutar_pasada_con_motores(
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
    motores: &mut dyn Motores,
    revisor: &mut dyn Revisor,
) -> Resultado<DesenlacePasada> {
    let canal = Canal::nuevo(carpeta, agente)?;
    let ruta_registro = ruta_del_registro(carpeta, agente);
    let ruta_latido = ruta_del_latido(carpeta, agente, ajustes.fichero_latido);
    // Decisión 2: si nadie tomó la foto inicial al arrancar, la toma esta pasada; si da por visto
    // lo que hay, no atiende nada.
    if tomar_instantanea_inicial(carpeta, agente)? == Instantanea::Tomada {
        return Ok(DesenlacePasada::PrimeraPasada);
    }
    let mut registro = RegistroLectura::cargar(&ruta_registro)?;
    let buzones = canal.buzones_ajenos()?;
    let carpeta_encargos = carpeta_de_los_encargos(carpeta, agente);
    let fecha = fecha_de_ahora();
    let mut fichas = leer_el_registro_de_encargos(&carpeta_encargos, ajustes)?;
    migracion::migrar(
        &mut fichas,
        &migracion::Entrada {
            carpeta_de_los_encargos: &carpeta_encargos,
            ruta_de_los_pendientes: &ruta_de_los_pendientes(carpeta, agente),
            lectura: &registro,
            buzones: &buzones,
            agente,
            fecha: &fecha,
        },
    )?;

    let recibido_a = crate::protocolo::marca_del_latido();
    let (a_medias, deltas) = leer_los_quietos(&canal, &buzones, &mut registro, ajustes);
    let nuevo = Novedades {
        agente,
        buzones: &buzones,
        deltas: &deltas,
    };
    registrar_lo_nuevo(&nuevo, &mut fichas, &carpeta_encargos, &fecha, ajustes)?;

    poner_al_dia_las_entregas(carpeta, agente, ajustes);
    limpiar_las_copias(carpeta, agente, ajustes);

    if !fichas.hay_pendientes() {
        registro.guardar(&ruta_registro)?;
        return Ok(if a_medias.is_empty() {
            DesenlacePasada::SinEncargos
        } else {
            DesenlacePasada::BuzonAMedias
        });
    }

    let Reparto {
        resueltas,
        de_revision,
        del_modelo,
    } = repartir_los_pendientes(
        &canal,
        carpeta,
        agente,
        ajustes,
        &*revisor,
        &mut fichas,
        &fecha,
    );
    let para_el_modelo = encargos_para_el_modelo(del_modelo, motores);
    let mut grupos = agrupar(para_el_modelo, motores.cargado().as_deref()).into_iter();

    let leido = construir_resumen_leido(&buzones);
    let pasada = Pasada {
        carpeta,
        agente,
        ajustes,
        canal: &canal,
        deltas: &deltas,
        leido: &leido,
        recibido_a: &recibido_a,
        ruta_latido: &ruta_latido,
    };
    let mut resumen = ResumenPasada {
        buzones_a_medias: a_medias.len(),
        ..ResumenPasada::default()
    };

    // El primer grupo decide si la pasada sigue: si su motor todavía carga, se vuelve en la
    // vuelta siguiente. Lo nuevo ya está en el registro de encargos; el de lectura no se guarda,
    // para que la pasada siguiente vuelva a dar al modelo las mismas novedades del canal. Las
    // revisiones que no tocan el motor no esperan a la carga: se atienden ya.
    let primero = match preparar_el_primer_grupo(&mut grupos, motores, ajustes, &ruta_latido) {
        PrimerGrupo::Cargando => {
            let lo_que_no_espera = LoQueNoEspera {
                resueltas: &resueltas,
                de_revision,
                modelo_cargado: motores.cargado(),
                carpeta_encargos: &carpeta_encargos,
            };
            atender_mientras_carga(
                &pasada,
                revisor,
                lo_que_no_espera,
                &mut fichas,
                &mut resumen,
            )?;
            return Ok(DesenlacePasada::MotorCargando);
        }
        PrimerGrupo::Ninguno => None,
        PrimerGrupo::Preparado(grupo, preparacion) => Some((grupo, preparacion)),
    };

    // Rotar el historial anterior del buzón propio a su fichero histórico para que
    // esta pasada mantenga únicamente el último latido.
    canal.rotar_a_historico()?;

    responder_lo_resuelto(
        &pasada,
        &resueltas,
        &mut fichas,
        &mut resumen,
        &motores.cargado(),
    );
    atender_los_grupos(
        &pasada,
        motores,
        primero,
        &mut grupos,
        &mut fichas,
        &mut resumen,
    )?;
    atender_con_el_revisor(
        &pasada,
        revisor,
        de_revision,
        &mut fichas,
        &mut resumen,
        motores.cargado(),
    );

    guardar_los_encargos_y_la_lectura(
        &carpeta_encargos,
        &mut fichas,
        &ruta_registro,
        &registro,
        ajustes,
    )?;

    Ok(DesenlacePasada::Atendidos(resumen))
}

/// Lo nuevo de los buzones ajenos en esta pasada, para anotarlo y podar el registro de encargos.
struct Novedades<'a> {
    agente: &'a str,
    buzones: &'a [(String, String)],
    deltas: &'a [(String, Delta)],
}

/// Los buzones a medio escribir y lo nuevo de los demás. Lo de un buzón a medio escribir no se
/// lee: sin delta, ni se buscan sus encargos ni se poda lo suyo, y el registro de lectura no avanza
/// para él, así que la vuelta siguiente lo ve.
fn leer_los_quietos(
    canal: &Canal,
    buzones: &[(String, String)],
    registro: &mut RegistroLectura,
    ajustes: &Ajustes,
) -> (BTreeSet<String>, Vec<(String, Delta)>) {
    let a_medias = buzones_a_medias(
        buzones,
        registro,
        |nombre| canal.edad_del_buzon(nombre),
        ajustes.buzon_estable_tras,
    );
    let quietos: Vec<(String, String)> = buzones
        .iter()
        .filter(|(nombre, _)| !a_medias.contains(nombre))
        .cloned()
        .collect();
    let deltas = deltas_de(&quietos, registro);
    (a_medias, deltas)
}

/// Anota como pendiente cada encargo nuevo y olvida las fichas de lo que lleva el plazo sin verse,
/// guardando el registro de encargos tras cada paso: lo nuevo queda en el disco antes de atender
/// nada.
fn registrar_lo_nuevo(
    nuevo: &Novedades,
    fichas: &mut RegistroDeEncargos,
    carpeta_encargos: &Path,
    fecha: &str,
    ajustes: &Ajustes,
) -> Resultado<()> {
    for encargo in encargos_nuevos(nuevo.agente, nuevo.buzones, nuevo.deltas, fichas) {
        fichas.anotar(&encargo, fecha, EstadoDelEncargo::Pendiente);
    }
    disco::guardar_las_cambiadas(carpeta_encargos, fichas)?;
    let (presentes, existentes) = encargos_presentes(nuevo.agente, nuevo.buzones, nuevo.deltas);
    let plazo = i64::try_from(ajustes.olvidar_lo_cerrado_tras_minutos)
        .ok()
        .and_then(chrono::Duration::try_minutes)
        .unwrap_or(chrono::Duration::MAX);
    for id in fichas.podar(&presentes, &existentes, Local::now().naive_local(), plazo) {
        disco::borrar(carpeta_encargos, &id)?;
    }
    disco::guardar_las_cambiadas(carpeta_encargos, fichas)
}

/// Los pendientes, repartidos por quién los atiende.
struct Reparto {
    /// Los que resolvió el arnés sin el modelo, con su respuesta.
    resueltas: Vec<(Encargo, String)>,
    /// Los del revisor.
    de_revision: Vec<Encargo>,
    /// Los del modelo.
    del_modelo: Vec<Encargo>,
}

/// Resuelve lo que no necesita al modelo y separa el resto. Las cancelaciones van antes que nada:
/// lo que cancelan ya no se atiende en esta pasada. Después, las confirmaciones de las propuestas
/// y los `preparar:`, que también resuelve el arnés. De lo que queda abierto, los del revisor van
/// aparte.
fn repartir_los_pendientes(
    canal: &Canal,
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
    revisor: &dyn Revisor,
    fichas: &mut RegistroDeEncargos,
    fecha: &str,
) -> Reparto {
    let (cancelaciones, demas): (Vec<Encargo>, Vec<Encargo>) = fichas
        .pendientes()
        .into_iter()
        .partition(|encargo| es_cancelacion(encargo.cuerpo(agente)));
    let mut resueltas = cancelar(&cancelaciones, fichas, agente, fecha);
    let (confirmaciones, demas): (Vec<Encargo>, Vec<Encargo>) = demas
        .into_iter()
        .partition(|encargo| es_confirmacion(encargo.cuerpo(agente)));
    resueltas.extend(confirmar_las_propuestas(
        &confirmaciones,
        canal,
        carpeta,
        agente,
        ajustes,
    ));
    let (preparaciones, demas): (Vec<Encargo>, Vec<Encargo>) = demas
        .into_iter()
        .partition(|encargo| es_preparacion(encargo.cuerpo(agente)));
    resueltas.extend(preparar_las_propuestas(
        &preparaciones,
        carpeta,
        agente,
        ajustes,
    ));
    let demas: Vec<Encargo> = demas
        .into_iter()
        .filter(|encargo| fichas.esta_abierto(encargo))
        .collect();
    let (de_revision, del_modelo) = separar_los_de_revision(demas, revisor, agente);
    Reparto {
        resueltas,
        de_revision,
        del_modelo,
    }
}

/// Lo que se atiende aunque el motor siga cargando.
struct LoQueNoEspera<'a> {
    resueltas: &'a [(Encargo, String)],
    de_revision: Vec<Encargo>,
    modelo_cargado: Option<String>,
    carpeta_encargos: &'a Path,
}

/// Con el motor cargando, responde lo que resolvió el arnés y atiende las revisiones que no tocan
/// el motor; las demás esperan a la vuelta siguiente. Si el registro de encargos no se guarda, se
/// avisa: lo atendido sigue pendiente en el disco y se reatiende, que es mejor que perderlo.
fn atender_mientras_carga(
    pasada: &Pasada,
    revisor: &mut dyn Revisor,
    lo_que_no_espera: LoQueNoEspera,
    fichas: &mut RegistroDeEncargos,
    resumen: &mut ResumenPasada,
) -> Resultado<()> {
    let LoQueNoEspera {
        resueltas,
        de_revision,
        modelo_cargado,
        carpeta_encargos,
    } = lo_que_no_espera;
    let (sin_motor, _con_motor): (Vec<Encargo>, Vec<Encargo>) = de_revision
        .into_iter()
        .partition(|encargo| !revisor.necesita_el_motor(encargo.cuerpo(pasada.agente)));
    if sin_motor.is_empty() && resueltas.is_empty() {
        return Ok(());
    }
    pasada.canal.rotar_a_historico()?;
    responder_lo_resuelto(pasada, resueltas, fichas, resumen, &modelo_cargado);
    atender_con_el_revisor(pasada, revisor, sin_motor, fichas, resumen, modelo_cargado);
    if let Err(fallo) = disco::guardar_las_cambiadas(carpeta_encargos, fichas) {
        pasada
            .ajustes
            .terminal
            .avisar(&publicacion::mensaje_registro_de_encargos_sin_guardar(
                &fallo,
            ));
    }
    Ok(())
}

/// Aplica al registro de entregas lo que dicen las confirmaciones (`aceptada:`, `rechazada:`) y
/// devuelve la respuesta de cada una. Si el registro no se puede leer o guardar, se dice en la
/// respuesta y en la terminal.
fn confirmar_las_propuestas(
    confirmaciones: &[Encargo],
    canal: &Canal,
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
) -> Vec<(Encargo, String)> {
    if confirmaciones.is_empty() {
        return Vec::new();
    }
    let ruta = crate::arnes::espacio::ruta_de_las_entregas(carpeta, agente);
    let leido = match crate::arnes::entregas::disco::leer(&ruta) {
        Ok(leido) => leido,
        Err(fallo) => {
            let texto = publicacion::mensaje_entregas_sin_leer(&fallo);
            ajustes.terminal.avisar(&texto);
            return confirmaciones
                .iter()
                .map(|encargo| (encargo.clone(), texto.clone()))
                .collect();
        }
    };
    if let Some(aviso) = leido.aviso {
        ajustes.terminal.avisar(&aviso);
    }
    let mut registro = leido.registro;
    let mut cambio = false;
    let mut respuestas: Vec<(Encargo, String)> = confirmaciones
        .iter()
        .filter_map(|encargo| {
            let confirmacion = reconocer(encargo.cuerpo(agente))?;
            let quien = canal
                .cabecera_de(encargo)
                .map(|cabecera| cabecera.identidad().to_string())
                .unwrap_or_else(|| encargo.de.clone());
            let (texto, cambio_esta) = confirmar(&confirmacion, &encargo.de, &quien, &mut registro);
            cambio |= cambio_esta;
            Some((encargo.clone(), texto))
        })
        .collect();
    if cambio {
        if let Err(fallo) = crate::arnes::entregas::disco::guardar(&ruta, &registro) {
            let texto = publicacion::mensaje_entregas_sin_leer(&fallo);
            ajustes.terminal.avisar(&texto);
            for (_, respuesta) in &mut respuestas {
                *respuesta = texto.clone();
            }
        }
    }
    respuestas
}

/// Deduce en qué quedó cada propuesta (`arnes::entregas::al_dia`). Lo que falle se avisa y la
/// pasada sigue: es información, no trabajo pendiente.
fn poner_al_dia_las_entregas(carpeta: &Path, agente: &str, ajustes: &Ajustes) {
    let ruta = crate::arnes::espacio::ruta_de_las_entregas(carpeta, agente);
    let ahora = Local::now().naive_local();
    let candidatos = carpeta.join(crate::arnes::herramientas::carpeta_de_candidatos(agente));
    match crate::arnes::entregas::al_dia::poner_al_dia(
        &ruta,
        carpeta,
        &candidatos,
        &ajustes.entregas,
        ahora,
    ) {
        Ok(avisos) => {
            for aviso in avisos {
                ajustes.terminal.avisar(&aviso);
            }
        }
        Err(fallo) => ajustes
            .terminal
            .avisar(&publicacion::mensaje_entregas_sin_poner_al_dia(&fallo)),
    }
}

/// El primer grupo con su motor ya preparado, si hay encargos para el modelo.
type Primero = Option<(Grupo, Resultado<Preparacion>)>;

/// Cómo quedó el motor del primer grupo, que decide si la pasada sigue.
enum PrimerGrupo {
    /// No hay encargos para el modelo: no se prepara nada.
    Ninguno,
    /// Su motor todavía carga: la pasada entera se repite en la vuelta siguiente.
    Cargando,
    /// Listo, sin modelo o con un fallo al prepararlo: se atiende en esta pasada.
    Preparado(Grupo, Resultado<Preparacion>),
}

/// Prepara el motor del primer grupo sin esperar. Un fallo al prepararlo no corta la pasada: sus
/// encargos van a la cola y cuentan un intento; se deja el latido en error.
fn preparar_el_primer_grupo(
    grupos: &mut std::vec::IntoIter<Grupo>,
    motores: &mut dyn Motores,
    ajustes: &Ajustes,
    ruta_latido: &Path,
) -> PrimerGrupo {
    let Some(grupo) = grupos.next() else {
        return PrimerGrupo::Ninguno;
    };
    match motores.preparar(grupo.modelo.as_deref(), Espera::Ninguna) {
        Ok(Preparacion::Cargando | Preparacion::TiempoAgotado) => PrimerGrupo::Cargando,
        Ok(preparacion) => PrimerGrupo::Preparado(grupo, Ok(preparacion)),
        Err(fallo) => {
            Latido::en_error(
                Local::now(),
                ajustes.intervalo_segundos,
                None,
                fallo.to_string(),
            )
            .guardar_con_aviso(ruta_latido, ajustes.terminal);
            PrimerGrupo::Preparado(grupo, Err(fallo))
        }
    }
}

/// Atiende el primer grupo con el motor que ya se preparó y, después, cada grupo siguiente,
/// esperando en el sitio a que cargue su modelo.
fn atender_los_grupos(
    pasada: &Pasada,
    motores: &mut dyn Motores,
    primero: Primero,
    grupos: &mut std::vec::IntoIter<Grupo>,
    fichas: &mut RegistroDeEncargos,
    resumen: &mut ResumenPasada,
) -> Resultado<()> {
    let Some((grupo, preparacion)) = primero else {
        return Ok(());
    };
    atender_el_grupo(pasada, motores, grupo, preparacion, grupos, fichas, resumen)?;
    while let Some(grupo) = grupos.next() {
        let preparacion = motores.preparar(grupo.modelo.as_deref(), Espera::HastaElTope);
        atender_el_grupo(pasada, motores, grupo, preparacion, grupos, fichas, resumen)?;
    }
    Ok(())
}

/// Atiende un grupo según cómo quedó su motor. Sin modelo utilizable, sus encargos se cierran
/// en el acto. Si el motor no está a punto, ese grupo y los que siguen esperan en la cola.
fn atender_el_grupo(
    pasada: &Pasada,
    motores: &mut dyn Motores,
    grupo: Grupo,
    preparacion: Resultado<Preparacion>,
    siguientes: &mut std::vec::IntoIter<Grupo>,
    fichas: &mut RegistroDeEncargos,
    resumen: &mut ResumenPasada,
) -> Resultado<()> {
    let motivo = match preparacion {
        Ok(Preparacion::Listo {
            mut motor,
            procedencia,
        }) => {
            return atender_con_el_modelo(
                pasada,
                motores,
                motor.as_mut(),
                procedencia,
                grupo.encargos,
                fichas,
                resumen,
            );
        }
        Ok(Preparacion::SinModelo(motivo)) => {
            for (encargo, _) in grupo.encargos {
                cerrar_sin_modelo(pasada, fichas, &encargo, &motivo, resumen);
            }
            return Ok(());
        }
        Ok(Preparacion::Cargando | Preparacion::TiempoAgotado) => {
            publicacion::motivo_carga_agotada(grupo.modelo.as_deref())
        }
        Err(fallo) => publicacion::motivo_motor_sin_preparar(&fallo),
    };
    let resto = grupo
        .encargos
        .into_iter()
        .chain(siguientes.flat_map(|g| g.encargos));
    for (encargo, _) in resto {
        dejar_pendiente(pasada, fichas, &encargo, &motivo, resumen);
    }
    Ok(())
}

/// Los identificadores de los encargos que hay ahora en cada buzón que cambió, y los nombres de
/// todos los buzones ajenos: lo que necesita la poda del registro.
fn encargos_presentes(
    agente: &str,
    buzones: &[(String, String)],
    deltas: &[(String, crate::protocolo::Delta)],
) -> (BTreeMap<String, BTreeSet<String>>, BTreeSet<String>) {
    let cambiados: BTreeSet<&str> = deltas
        .iter()
        .filter(|(_, delta)| !matches!(delta, crate::protocolo::Delta::Nada))
        .map(|(nombre, _)| nombre.as_str())
        .collect();
    let presentes = buzones
        .iter()
        .filter(|(nombre, _)| cambiados.contains(nombre.as_str()))
        .map(|(nombre, contenido)| {
            let ids = crate::protocolo::detectar(agente, nombre, contenido)
                .iter()
                .map(crate::protocolo::Encargo::id)
                .collect();
            (nombre.clone(), ids)
        })
        .collect();
    let existentes = buzones.iter().map(|(nombre, _)| nombre.clone()).collect();
    (presentes, existentes)
}

/// Lee el registro de encargos y avisa de cada ficha que no se entendió y se apartó.
fn leer_el_registro_de_encargos(
    carpeta_encargos: &Path,
    ajustes: &Ajustes,
) -> Resultado<RegistroDeEncargos> {
    let leido = disco::leer(carpeta_encargos)?;
    for aviso in &leido.avisos {
        ajustes.terminal.avisar(aviso);
    }
    Ok(RegistroDeEncargos::de_fichas(leido.fichas))
}

/// Guarda las fichas que cambiaron y, solo si quedaron guardadas, el registro de lectura. Si una
/// no se guarda, se avisa: en disco sigue pendiente y la pasada siguiente la vuelve a atender, que
/// es preferible a perderla.
fn guardar_los_encargos_y_la_lectura(
    carpeta_encargos: &Path,
    fichas: &mut RegistroDeEncargos,
    ruta_registro: &Path,
    registro: &RegistroLectura,
    ajustes: &Ajustes,
) -> Resultado<()> {
    match disco::guardar_las_cambiadas(carpeta_encargos, fichas) {
        Ok(()) => registro.guardar(ruta_registro)?,
        Err(fallo) => {
            ajustes
                .terminal
                .avisar(&publicacion::mensaje_registro_de_encargos_sin_guardar(
                    &fallo,
                ))
        }
    }
    Ok(())
}
