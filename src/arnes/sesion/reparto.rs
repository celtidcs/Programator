//! Lo que una pasada tiene que atender y en qué orden, calculado sin tocar el disco: los encargos
//! nuevos que traen los deltas, los del revisor aparte, y los del modelo agrupados por modelo.

use super::{ModeloPedido, Motores, Revisor};
use crate::arnes::pendientes::Cola;
use crate::protocolo::{detectar, Delta, Encargo, RegistroLectura};

/// Lo nuevo de cada buzón ajeno respecto de lo leído. El registro avanza solo en memoria: quien
/// llama decide si lo guarda.
pub(super) fn deltas_de(
    buzones: &[(String, String)],
    registro: &mut RegistroLectura,
) -> Vec<(String, Delta)> {
    buzones
        .iter()
        .map(|(nombre, contenido)| (nombre.clone(), registro.delta(nombre, contenido)))
        .collect()
}

/// Los encargos para `agente` que traen los deltas y que no esperan ya en la cola.
pub(super) fn encargos_nuevos(
    agente: &str,
    deltas: &[(String, Delta)],
    cola: &Cola,
) -> Vec<Encargo> {
    // Decisión 3: los encargos se buscan solo en lo nuevo. Con `Delta::Nada` no hay nada que
    // mirar, así que un encargo ya atendido no vuelve a aparecer.
    let mut encargos = Vec::new();
    for (nombre, delta) in deltas {
        if let Some(texto) = texto_del_delta(delta) {
            encargos.extend(detectar(agente, nombre, texto));
        }
    }
    encargos.retain(|encargo| !cola.contiene(encargo));
    encargos
}

/// Separa los encargos del revisor (primero) de los del modelo (después).
pub(super) fn separar_los_de_revision(
    encargos: Vec<Encargo>,
    revisor: &dyn Revisor,
    agente: &str,
) -> (Vec<Encargo>, Vec<Encargo>) {
    encargos
        .into_iter()
        .partition(|encargo| revisor.es_suyo(encargo.cuerpo(agente)))
}

/// Los encargos que atiende el modelo, cada uno con el modelo que pide.
pub(super) fn encargos_para_el_modelo(
    cola: &Cola,
    nuevos: Vec<Encargo>,
    motores: &dyn Motores,
) -> Vec<(Encargo, ModeloPedido)> {
    // Los pendientes, que llegaron antes, van delante de los nuevos.
    cola.encargos()
        .into_iter()
        .chain(nuevos)
        .map(|encargo| {
            let pedido = motores.modelo_para(&encargo);
            (encargo, pedido)
        })
        .collect()
}

/// Los encargos que se atienden con un mismo modelo, en orden de llegada.
#[derive(Debug)]
pub(super) struct Grupo {
    pub(super) modelo: Option<String>,
    pub(super) encargos: Vec<(Encargo, ModeloPedido)>,
}

/// Agrupa los encargos por modelo para cambiar de modelo lo menos posible: primero el grupo del
/// modelo ya cargado, luego los demás por orden de aparición. Dentro de cada grupo se respeta el
/// orden de llegada, así que los pendientes, que llegaron antes, van delante.
pub(super) fn agrupar(encargos: Vec<(Encargo, ModeloPedido)>, cargado: Option<&str>) -> Vec<Grupo> {
    let mut grupos: Vec<Grupo> = Vec::new();
    for (encargo, pedido) in encargos {
        match grupos.iter_mut().find(|g| g.modelo == pedido.modelo) {
            Some(grupo) => grupo.encargos.push((encargo, pedido)),
            None => grupos.push(Grupo {
                modelo: pedido.modelo.clone(),
                encargos: vec![(encargo, pedido)],
            }),
        }
    }
    let del_cargado =
        cargado.and_then(|c| grupos.iter().position(|g| g.modelo.as_deref() == Some(c)));
    if let Some(posicion) = del_cargado {
        let grupo = grupos.remove(posicion);
        grupos.insert(0, grupo);
    }
    grupos
}

/// Texto que aporta un `Delta` para buscar encargos, si aporta alguno.
fn texto_del_delta(delta: &Delta) -> Option<&str> {
    match delta {
        Delta::Nada => None,
        Delta::Incremento(texto) | Delta::Completo(texto) => Some(texto.as_str()),
    }
}
