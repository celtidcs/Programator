//! Lo que una pasada tiene que atender y en qué orden, calculado sin tocar el disco: los encargos
//! nuevos de los buzones que cambiaron, los del revisor aparte, y los del modelo agrupados por
//! modelo.

use super::{ModeloPedido, Motores, Revisor};
use crate::arnes::prioridad::{prioridad_pedida, Prioridad};
use crate::arnes::registro_de_encargos::RegistroDeEncargos;
use crate::protocolo::{detectar, Delta, Encargo, RegistroLectura};
use std::collections::BTreeSet;

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

/// Los buzones con novedades que cambiaron hace menos de `estable_tras`: pueden estar a medio
/// escribir, y lo cortado daría un encargo que nadie pidió. Lo nuevo de estos se deja para la
/// vuelta siguiente; los demás se atienden. Uno cuya edad no se sabe (`edad` da `None`) cuenta
/// como quieto: esperarlo podría no acabar nunca.
pub(super) fn buzones_a_medias(
    buzones: &[(String, String)],
    registro: &RegistroLectura,
    edad: impl Fn(&str) -> Option<std::time::Duration>,
    estable_tras: std::time::Duration,
) -> BTreeSet<String> {
    buzones
        .iter()
        .filter(|(nombre, contenido)| registro.tiene_novedades(nombre, contenido))
        .filter(|(nombre, _)| edad(nombre).is_some_and(|edad| edad < estable_tras))
        .map(|(nombre, _)| nombre.clone())
        .collect()
}

/// Los encargos para `agente` que no tienen ficha en el registro. Se buscan en los buzones que
/// cambiaron, leídos enteros: lo que decide si un encargo es nuevo es su identificador, no dónde
/// cae en el fichero. Así, editar lo anterior o cambiar sus saltos de línea no reatiende nada, y
/// una línea `Para …` añadida dentro de una intervención ya leída se ve.
pub(super) fn encargos_nuevos(
    agente: &str,
    buzones: &[(String, String)],
    deltas: &[(String, Delta)],
    registro: &RegistroDeEncargos,
) -> Vec<Encargo> {
    let cambio = |nombre: &str| {
        deltas
            .iter()
            .any(|(de, delta)| de == nombre && !matches!(delta, Delta::Nada))
    };
    buzones
        .iter()
        .filter(|(nombre, _)| cambio(nombre))
        .flat_map(|(nombre, contenido)| detectar(agente, nombre, contenido))
        .filter(|encargo| !registro.tiene(encargo))
        .collect()
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

/// Los encargos que atiende el modelo, cada uno con el modelo que pide, en el orden en que llegan.
pub(super) fn encargos_para_el_modelo(
    encargos: Vec<Encargo>,
    motores: &dyn Motores,
) -> Vec<(Encargo, ModeloPedido)> {
    encargos
        .into_iter()
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

/// Ordena los encargos por su prioridad (`Prioridad:`), primero `alta`, y dentro de cada una los
/// agrupa por modelo con [`agrupar_por_modelo`]. Un encargo `alta` que pide otro modelo obliga a
/// cambiarlo: es lo que se pide.
pub(super) fn agrupar(encargos: Vec<(Encargo, ModeloPedido)>, cargado: Option<&str>) -> Vec<Grupo> {
    let mut por_prioridad: Vec<(Prioridad, Vec<(Encargo, ModeloPedido)>)> = Prioridad::EN_ORDEN
        .into_iter()
        .map(|prioridad| (prioridad, Vec::new()))
        .collect();
    for (encargo, pedido) in encargos {
        let prioridad = prioridad_pedida(&encargo.texto).efectiva();
        if let Some((_, de_esta)) = por_prioridad.iter_mut().find(|(p, _)| *p == prioridad) {
            de_esta.push((encargo, pedido));
        }
    }
    por_prioridad
        .into_iter()
        .flat_map(|(_, de_esta)| agrupar_por_modelo(de_esta, cargado))
        .collect()
}

/// Agrupa los encargos por modelo para cambiar de modelo lo menos posible: primero el grupo del
/// modelo ya cargado, luego los demás por orden de aparición. Dentro de cada grupo se respeta el
/// orden de llegada, así que los pendientes, que llegaron antes, van delante.
fn agrupar_por_modelo(encargos: Vec<(Encargo, ModeloPedido)>, cargado: Option<&str>) -> Vec<Grupo> {
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

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::protocolo::encargo::Forma;
    use std::time::Duration;

    /// Solo cuentan los buzones con novedades, y solo si son más recientes que el plazo.
    #[test]
    fn un_buzon_con_novedades_recien_tocado_esta_a_medias() {
        let buzones = vec![
            ("quieto.md".to_string(), "leído\n".to_string()),
            ("nuevo.md".to_string(), "leído\nx".to_string()),
        ];
        let mut registro = RegistroLectura::default();
        registro.delta("quieto.md", "leído\n");
        registro.delta("nuevo.md", "leído\n");
        let plazo = Duration::from_millis(1500);
        let a_medias = |edad: &dyn Fn(&str) -> Option<Duration>, plazo| {
            buzones_a_medias(&buzones, &registro, edad, plazo)
        };
        let edades = |reciente: u64, otro: u64| {
            move |nombre: &str| {
                Some(Duration::from_millis(if nombre == "nuevo.md" {
                    reciente
                } else {
                    otro
                }))
            }
        };

        let solo_el_nuevo = BTreeSet::from(["nuevo.md".to_string()]);
        assert_eq!(a_medias(&edades(1499, 9999), plazo), solo_el_nuevo);
        // El quieto, recién tocado, no cuenta: no tiene nada nuevo.
        assert_eq!(a_medias(&edades(1499, 0), plazo), solo_el_nuevo);
        assert!(a_medias(&edades(1500, 0), plazo).is_empty());
        assert!(a_medias(&|_| None, plazo).is_empty());
        assert!(a_medias(&edades(0, 0), Duration::ZERO).is_empty());
        // Mirar no da nada por leído.
        assert!(registro.tiene_novedades("nuevo.md", "leído\nx"));
    }

    fn encargo(texto: &str) -> (Encargo, ModeloPedido) {
        let encargo = Encargo {
            latido: None,
            de: "claude.md".to_string(),
            texto: texto.to_string(),
            forma: Forma::Linea,
        };
        (encargo, ModeloPedido::default())
    }

    fn con_modelo(texto: &str, modelo: &str) -> (Encargo, ModeloPedido) {
        let (encargo, mut pedido) = encargo(texto);
        pedido.modelo = Some(modelo.to_string());
        (encargo, pedido)
    }

    fn textos(grupos: &[Grupo]) -> Vec<Vec<&str>> {
        grupos
            .iter()
            .map(|g| g.encargos.iter().map(|(e, _)| e.texto.as_str()).collect())
            .collect()
    }

    /// D1.
    #[test]
    fn baja_normal_y_alta_se_atienden_al_reves() {
        let grupos = agrupar(
            vec![
                encargo("uno\nPrioridad: baja"),
                encargo("dos"),
                encargo("tres\nPrioridad: alta"),
            ],
            None,
        );

        assert_eq!(
            textos(&grupos),
            vec![
                vec!["tres\nPrioridad: alta"],
                vec!["dos"],
                vec!["uno\nPrioridad: baja"],
            ]
        );
    }

    #[test]
    fn en_la_misma_prioridad_manda_el_modelo_cargado_y_luego_la_llegada() {
        let grupos = agrupar(
            vec![
                con_modelo("a", "otro"),
                con_modelo("b", "cargado"),
                con_modelo("c", "otro"),
            ],
            Some("cargado"),
        );

        assert_eq!(textos(&grupos), vec![vec!["b"], vec!["a", "c"]]);
    }

    #[test]
    fn un_encargo_alta_con_otro_modelo_va_antes_que_el_modelo_cargado() {
        let grupos = agrupar(
            vec![
                con_modelo("normal", "cargado"),
                con_modelo("urge\nPrioridad: alta", "otro"),
            ],
            Some("cargado"),
        );

        assert_eq!(grupos[0].modelo.as_deref(), Some("otro"));
        assert_eq!(grupos[1].modelo.as_deref(), Some("cargado"));
    }

    #[test]
    fn una_prioridad_desconocida_se_ordena_como_normal() {
        let grupos = agrupar(
            vec![
                encargo("uno\nPrioridad: baja"),
                encargo("dos\nPrioridad: urgente"),
            ],
            None,
        );

        assert_eq!(
            textos(&grupos),
            vec![
                vec!["dos\nPrioridad: urgente"],
                vec!["uno\nPrioridad: baja"]
            ]
        );
    }
}
