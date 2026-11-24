//! La consulta del ámbito: `leer_fichero` y `listar`.
//!
//! Lo que el modelo pide se traduce primero, sin tocar disco, en un pedido de lectura validado
//! (`pedido_de_lectura`); después se resuelve la ruta en el ámbito y se lee o se lista. Ninguno de
//! los dos verbos escribe nada.

use super::error::motivo_para_el_modelo;
use super::lectura::{preparar_lectura, Numeracion};
use super::{argumentos, mensajes, Decision, Repertorio};
use crate::motor::SolicitudHerramienta;

/// Lo que pide una lectura, ya validado: la ruta tal como la escribió el modelo, el rango de
/// líneas (base 1, inclusivo) y si se numeran.
struct PedidoDeLectura<'a> {
    ruta: &'a str,
    desde_linea: Option<usize>,
    hasta_linea: Option<usize>,
    numeracion: Numeracion,
}

/// Traduce los argumentos de `leer_fichero` en un pedido de lectura, o deniega con la firma.
///
/// No toca disco: solo comprueba que los argumentos tienen el tipo que deben y que el rango no
/// está al revés.
fn pedido_de_lectura(s: &SolicitudHerramienta) -> Result<PedidoDeLectura<'_>, Decision> {
    let ruta = argumentos::texto(s, "ruta")?;
    let desde_linea = argumentos::linea(s, "desde_linea")?;
    let hasta_linea = argumentos::linea(s, "hasta_linea")?;
    let numeracion = Numeracion::segun(argumentos::logico(s, "numerar")?);

    if let (Some(d), Some(h)) = (desde_linea, hasta_linea) {
        if d > h {
            return Err(Decision::Denegada(mensajes::rango_al_reves(
                d,
                h,
                &argumentos::recordatorio_de_firma(&s.nombre),
            )));
        }
    }

    Ok(PedidoDeLectura {
        ruta,
        desde_linea,
        hasta_linea,
        numeracion,
    })
}

impl Repertorio<'_> {
    pub(super) fn leer_fichero(&self, s: &SolicitudHerramienta) -> Decision {
        let pedido = match pedido_de_lectura(s) {
            Ok(p) => p,
            Err(d) => return d,
        };
        let ruta = pedido.ruta;

        match self.ambito.resolver(ruta) {
            Err(e) => Decision::Denegada(motivo_para_el_modelo(ruta, &e)),
            Ok(resuelta) => match std::fs::read_to_string(&resuelta) {
                Ok(contenido) => Decision::Concedida(preparar_lectura(
                    ruta,
                    &contenido,
                    self.tope_lectura,
                    pedido.desde_linea,
                    pedido.hasta_linea,
                    pedido.numeracion,
                )),
                Err(e) => Decision::Denegada(mensajes::lectura_fallida(ruta, &e)),
            },
        }
    }

    pub(super) fn listar(&self, s: &SolicitudHerramienta) -> Decision {
        let ruta = match argumentos::texto(s, "ruta") {
            Ok(r) => r,
            Err(d) => return d,
        };
        match self.ambito.resolver(ruta) {
            Err(e) => Decision::Denegada(motivo_para_el_modelo(ruta, &e)),
            Ok(resuelta) => match std::fs::read_dir(&resuelta) {
                Err(e) => Decision::Denegada(mensajes::listado_fallido(ruta, &e)),
                Ok(entradas) => {
                    let nombres: Vec<String> = entradas
                        .filter_map(|e| e.ok())
                        .filter_map(|e| e.file_name().into_string().ok())
                        .collect();
                    Decision::Concedida(nombres.join("\n"))
                }
            },
        }
    }
}
