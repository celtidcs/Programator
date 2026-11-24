//! Las órdenes de la línea de órdenes que no arrancan el ciclo del agente: la ayuda, la versión,
//! los errores de uso, el lanzador aislado, el banco y la revisión de modelos. Cada una hace lo
//! suyo y termina; las del ciclo se devuelven a `main` sin tocar.

mod banco;
mod revision;

use crate::{CODIGO_DE_USO_INCORRECTO, VERSION};
use programator::argumentos::{self, Orden};
use programator::arranque;
use programator::ayudante_aislado;
use std::path::Path;

/// Atiende la orden si no es del ciclo. Devuelve la orden cuando sí lo es (`--diagnostico` o el
/// ciclo normal), para que la ejecute `main`; `None` cuando ya está atendida. Las que fallan
/// terminan el proceso con su código de salida.
pub(crate) fn atender(orden: Orden) -> Option<Orden> {
    match orden {
        Orden::LanzarAislado { orden } => lanzar_aislado(&orden),
        Orden::Ayuda => {
            // `print!`, no `println!`: `texto_de_ayuda` ya termina en salto de línea, y
            // `println!` añadiría uno más, dejando una línea en blanco de sobra al final.
            print!("{}", argumentos::texto_de_ayuda(VERSION));
        }
        Orden::Version => println!("{}", arranque::mensaje_version(VERSION)),
        Orden::NoEntendido(cual) => {
            eprintln!("{}", arranque::mensaje_argumento_no_entendido(&cual));
            std::process::exit(CODIGO_DE_USO_INCORRECTO);
        }
        Orden::FaltaValor(cual) => {
            eprintln!("{}", arranque::mensaje_argumento_falta_valor(&cual));
            std::process::exit(CODIGO_DE_USO_INCORRECTO);
        }
        Orden::BancoInforme { resultados } => banco::rehacer_el_informe(Path::new(&resultados)),
        Orden::Banco { carpeta, modelo } => banco::ejecutar(Path::new(&carpeta), modelo.as_deref()),
        Orden::BancoComprobar { tanda } => banco::comprobar(Path::new(&tanda)),
        Orden::BancoComparar { tanda_a, tanda_b } => {
            banco::comparar(Path::new(&tanda_a), Path::new(&tanda_b))
        }
        Orden::RegistroAnotar { tanda } => banco::anotar_en_el_registro(Path::new(&tanda)),
        Orden::RevisionBuscar => revision::buscar(),
        Orden::RevisionProponer => revision::proponer(),
        Orden::RetirarModelo { fichero } => revision::retirar(&fichero),
        Orden::DescargarModelo {
            repositorio,
            fichero,
        } => revision::descargar(&repositorio, &fichero),
        orden @ (Orden::Diagnostico { .. } | Orden::Ejecutar { .. }) => return Some(orden),
    }
    None
}

/// `--lanzar-aislado`: lanza la orden con el aislamiento y sale con su código.
fn lanzar_aislado(orden: &[String]) -> ! {
    match ayudante_aislado::lanzar_aislado(orden) {
        Ok(codigo) => std::process::exit(codigo),
        Err(motivo) => {
            eprintln!("{}", ayudante_aislado::mensaje_del_fallo(&motivo));
            std::process::exit(ayudante_aislado::CODIGO_AISLAMIENTO_FALLIDO);
        }
    }
}
