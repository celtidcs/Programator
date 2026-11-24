//! Una pasada completa del ciclo, paso a paso: lee el canal, reparte lo que hay que atender,
//! prepara el motor de cada grupo, atiende y guarda la cola y el registro de lectura.

use super::atencion_del_modelo::atender_con_el_modelo;
use super::instantanea::{tomar_instantanea_inicial, Instantanea};
use super::publicacion::{self, construir_resumen_leido};
use super::reparto::{
    agrupar, deltas_de, encargos_nuevos, encargos_para_el_modelo, separar_los_de_revision, Grupo,
};
use super::revisor::{atender_con_el_revisor, Revisor};
use super::sin_atender::{cerrar_sin_modelo, dejar_pendiente};
use super::{Ajustes, DesenlacePasada, Espera, Motores, Preparacion, ResumenPasada};
use crate::arnes::espacio::{ruta_de_los_pendientes, ruta_del_latido, ruta_del_registro};
use crate::arnes::pendientes::{self, Cola};
use crate::error::Resultado;
use crate::protocolo::{Canal, Delta, Latido, RegistroLectura, ResumenLeido};
use chrono::Local;
use std::path::Path;

/// Lo que comparten los encargos de una pasada.
pub(super) struct Pasada<'a> {
    pub(super) carpeta: &'a Path,
    pub(super) agente: &'a str,
    pub(super) ajustes: &'a Ajustes<'a>,
    pub(super) canal: &'a Canal,
    pub(super) deltas: &'a [(String, Delta)],
    pub(super) leido: &'a ResumenLeido,
    pub(super) ruta_latido: &'a Path,
}

/// Ejecuta una pasada completa del ciclo: lee el canal, detecta encargos nuevos, los atiende con
/// los pendientes de la cola delante y publica el resultado.
///
/// El motor se prepara una vez por grupo de encargos con el mismo modelo, y solo si hay algún
/// encargo para el modelo. Si el primero todavía está cargando, la pasada entera espera a la
/// vuelta siguiente sin guardar nada, como siempre. Si es un grupo posterior el que no termina de
/// cargar dentro de su tope, ese grupo y los que siguen pasan a la cola de pendientes: repetir la
/// pasada entera respondería dos veces a los ya atendidos.
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

    let ruta_cola = ruta_de_los_pendientes(carpeta, agente);
    let cola_leida = pendientes::leer(&ruta_cola)?;
    let mut cola = cola_leida.clone();

    let buzones = canal.buzones_ajenos()?;
    let deltas = deltas_de(&buzones, &mut registro);
    let encargos = encargos_nuevos(agente, &deltas, &cola);

    if encargos.is_empty() && cola.esta_vacia() {
        registro.guardar(&ruta_registro)?;
        return Ok(DesenlacePasada::SinEncargos);
    }

    let (de_revision, nuevos) = separar_los_de_revision(encargos, revisor, agente);
    let para_el_modelo = encargos_para_el_modelo(&cola, nuevos, motores);
    let mut grupos = agrupar(para_el_modelo, motores.cargado().as_deref()).into_iter();

    // El primer grupo decide si la pasada sigue: si su motor todavía carga, se vuelve en la
    // vuelta siguiente. No se guarda el registro: los deltas de arriba solo viven en esta
    // instancia en memoria, así que la pasada siguiente encuentra los mismos encargos. Tampoco se
    // ha atendido aún ningún encargo de revisión.
    let primero = match preparar_el_primer_grupo(&mut grupos, motores, ajustes, &ruta_latido) {
        PrimerGrupo::Cargando => return Ok(DesenlacePasada::MotorCargando),
        PrimerGrupo::Ninguno => None,
        PrimerGrupo::Preparado(grupo, preparacion) => Some((grupo, preparacion)),
    };

    // Rotar el historial anterior del buzón propio a su fichero histórico para que
    // esta pasada mantenga únicamente el último latido.
    canal.rotar_a_historico()?;

    let leido = construir_resumen_leido(&buzones);
    let pasada = Pasada {
        carpeta,
        agente,
        ajustes,
        canal: &canal,
        deltas: &deltas,
        leido: &leido,
        ruta_latido: &ruta_latido,
    };
    let mut resumen = ResumenPasada::default();
    atender_los_grupos(
        &pasada,
        motores,
        primero,
        &mut grupos,
        &mut cola,
        &mut resumen,
    )?;
    atender_con_el_revisor(
        &pasada,
        revisor,
        de_revision,
        &mut resumen,
        motores.cargado(),
    );

    guardar_la_cola_y_el_registro(
        &ruta_cola,
        &cola,
        &cola_leida,
        &ruta_registro,
        &registro,
        ajustes,
    )?;

    Ok(DesenlacePasada::Atendidos(resumen))
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
    cola: &mut Cola,
    resumen: &mut ResumenPasada,
) -> Resultado<()> {
    let Some((grupo, preparacion)) = primero else {
        return Ok(());
    };
    atender_el_grupo(pasada, motores, grupo, preparacion, grupos, cola, resumen)?;
    while let Some(grupo) = grupos.next() {
        let preparacion = motores.preparar(grupo.modelo.as_deref(), Espera::HastaElTope);
        atender_el_grupo(pasada, motores, grupo, preparacion, grupos, cola, resumen)?;
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
    cola: &mut Cola,
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
                cola,
                resumen,
            );
        }
        Ok(Preparacion::SinModelo(motivo)) => {
            for (encargo, _) in grupo.encargos {
                cerrar_sin_modelo(pasada, cola, &encargo, &motivo, resumen);
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
        dejar_pendiente(pasada, cola, &encargo, &motivo, resumen);
    }
    Ok(())
}

/// Guarda la cola si cambió y, solo si quedó guardada, el registro de lectura.
///
/// Lo que no se publicó está en la cola, así que el registro puede avanzar; pero solo si la
/// cola quedó guardada: si no, avanzar perdería los encargos nuevos que no se atendieron.
fn guardar_la_cola_y_el_registro(
    ruta_cola: &Path,
    cola: &Cola,
    cola_leida: &Cola,
    ruta_registro: &Path,
    registro: &RegistroLectura,
    ajustes: &Ajustes,
) -> Resultado<()> {
    let guardada = if cola == cola_leida {
        Ok(())
    } else {
        pendientes::guardar(ruta_cola, cola)
    };
    match guardada {
        Ok(()) => registro.guardar(ruta_registro)?,
        Err(fallo) => ajustes
            .terminal
            .avisar(&publicacion::mensaje_cola_sin_guardar(&fallo)),
    }
    Ok(())
}
