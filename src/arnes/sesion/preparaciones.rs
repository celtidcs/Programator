//! Los `preparar:` de una pasada y la limpieza de sus copias. Los atiende el arnés, sin el
//! modelo, con el mismo aislamiento que las comprobaciones del banco.

use super::publicacion;
use super::Ajustes;
use crate::arnes::entregas::RegistroDeEntregas;
use crate::arnes::preparacion::atencion::{atender, Entorno};
use crate::arnes::preparacion::caducidad::limpiar;
use crate::arnes::preparacion::pedido::reconocer;
use crate::banco::comprobacion::lanzador::{FormaDeLanzar, LanzadorLocal};
use crate::banco::config::ComprobacionLocal;
use crate::protocolo::Encargo;
use chrono::Local;
use std::collections::BTreeMap;
use std::path::Path;

/// La respuesta de cada `preparar:`. Si el registro de entregas no se deja leer, todos dicen por
/// qué.
pub(super) fn preparar_las_propuestas(
    encargos: &[Encargo],
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
) -> Vec<(Encargo, String)> {
    if encargos.is_empty() {
        return Vec::new();
    }
    let entregas = match leer_las_entregas(carpeta, agente, ajustes) {
        Ok(entregas) => entregas,
        Err(texto) => {
            return encargos
                .iter()
                .map(|encargo| (encargo.clone(), texto.clone()))
                .collect()
        }
    };
    // El aislamiento de fábrica de cada plataforma, el de las comprobaciones del banco, con el
    // envoltorio de `[preparacion]` si lo trae.
    let comprobacion = comprobacion_de(ajustes.preparacion);
    let sistema: BTreeMap<String, String> = std::env::vars().collect();
    let candidatos = carpeta.join(crate::arnes::herramientas::carpeta_de_candidatos(agente));
    let entorno = Entorno {
        raiz: carpeta,
        candidatos: &candidatos,
        entregas: &entregas,
        preparacion: ajustes.preparacion,
        comprobacion: &comprobacion,
        sistema: &sistema,
        maximo_salida: ajustes.verificacion.maximo_salida_al_modelo,
        ahora: Local::now().naive_local(),
    };
    let mut sin_aislar = LanzadorLocal::nuevo(FormaDeLanzar::Directa);
    let mut aislado = LanzadorLocal::para(&comprobacion);
    encargos
        .iter()
        .filter_map(|encargo| {
            let pedido = reconocer(encargo.cuerpo(agente))?;
            let respuesta = atender(
                &pedido,
                &encargo.id(),
                &entorno,
                &mut sin_aislar,
                &mut aislado,
            );
            Some((encargo.clone(), respuesta))
        })
        .collect()
}

/// Cómo se aíslan las órdenes de preparar.
fn comprobacion_de(preparacion: &crate::config::Preparacion) -> ComprobacionLocal {
    let mut comprobacion = ComprobacionLocal::default();
    if let Some(envoltorio) = &preparacion.envoltorio {
        comprobacion.envoltorio = envoltorio.clone();
    }
    comprobacion
}

/// Borra las copias que sobran. Un fallo se avisa y se reintenta en la pasada siguiente.
pub(super) fn limpiar_las_copias(carpeta: &Path, agente: &str, ajustes: &Ajustes) {
    let Ok(entregas) = leer_las_entregas(carpeta, agente, ajustes) else {
        return;
    };
    if let Err(fallo) = limpiar(
        &carpeta.join(&ajustes.preparacion.carpeta),
        &entregas,
        Local::now().naive_local(),
        ajustes.preparacion.dias_de_vida,
    ) {
        ajustes
            .terminal
            .avisar(&publicacion::mensaje_copias_sin_limpiar(&fallo));
    }
}

/// El registro de entregas, o el texto que dice por qué no se pudo leer (ya avisado).
fn leer_las_entregas(
    carpeta: &Path,
    agente: &str,
    ajustes: &Ajustes,
) -> Result<RegistroDeEntregas, String> {
    let ruta = crate::arnes::espacio::ruta_de_las_entregas(carpeta, agente);
    match crate::arnes::entregas::disco::leer(&ruta) {
        Ok(leido) => {
            if let Some(aviso) = leido.aviso {
                ajustes.terminal.avisar(&aviso);
            }
            Ok(leido.registro)
        }
        Err(fallo) => {
            let texto = publicacion::mensaje_entregas_sin_leer(&fallo);
            ajustes.terminal.avisar(&texto);
            Err(texto)
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// En Ubuntu 24.04 y posteriores, `[preparacion] envoltorio` es lo que hace funcionar
    /// `preparar:` (F7): si se ignorara, las órdenes saldrían con el de fábrica, que allí no aísla.
    #[test]
    fn las_ordenes_de_preparar_se_aislan_con_el_envoltorio_de_la_configuracion() {
        let propio = vec!["bwrap".to_string(), "--unshare-net".to_string()];
        let preparacion = crate::config::Preparacion {
            envoltorio: Some(propio.clone()),
            ..Default::default()
        };
        assert_eq!(comprobacion_de(&preparacion).envoltorio, propio);
    }

    #[test]
    fn sin_envoltorio_en_la_configuracion_va_el_de_fabrica() {
        assert_eq!(
            comprobacion_de(&crate::config::Preparacion::default()),
            ComprobacionLocal::default()
        );
    }
}
