//! Cierre ordenado ante Ctrl+C: mata el `llama-server` que arrancó Programator antes de salir.
//!
//! Guarda en un registro global el servidor propio vivo y, al llegar la señal, lo mata y espera.

use crate::error::{Error, Resultado};
use std::process::Child;
use std::sync::{Arc, Mutex, Weak};

/// El hueco donde vive el `Child` mientras dura el servidor: lo comparten `ServidorLocal`, que es
/// su dueño, y el registro global de cierre ordenado, que solo necesita poder alcanzarlo.
pub(super) type Compartido = Arc<Mutex<Option<Child>>>;

/// Qué servidor arrancado por Programator hay que matar si llega un Ctrl+C antes de que
/// `ServidorLocal` se suelte por las buenas.
///
/// Guarda un `Weak`, nunca un `Arc` fuerte, y esa elección es la que hace que la garantía de tipos
/// de este módulo («un servidor que Programator no arrancó, no se toca nunca») siga intacta pase lo
/// que pase con Ctrl+C: un `Arc` fuerte aquí mantendría vivo el `Mutex<Option<Child>>` de un
/// servidor que ya se soltó por las buenas, y si esta ranura conservara una referencia a él después
/// de que `ServidorLocal::drop` ya lo hubiera matado y vaciado, en el mejor de los casos no haría
/// nada (el hueco ya está vacío) pero seguiría habiendo una lectura de estado ajeno a la instancia
/// que lo posee. Con `Weak`, en cuanto el único `Arc` fuerte —el que vive dentro de
/// `ServidorLocal`— se suelta, `upgrade()` empieza a devolver `None` sin que nadie tenga que
/// acordarse de limpiar esta ranura a mano.
///
/// Vive como estático porque el manejador de Ctrl+C (instalado una sola vez, en
/// `registrar_cierre_ordenado`) se registra antes de que exista ningún `ServidorLocal`, y necesita
/// un sitio fijo desde el que alcanzar al que se arranque después, sin que nadie tenga que
/// pasárselo a mano por en medio de código que no tiene por qué saber que existe.
static REGISTRO_CIERRE: Mutex<Option<Weak<Mutex<Option<Child>>>>> = Mutex::new(None);

/// Apunta el registro de cierre al servidor recién arrancado. Visible solo dentro de
/// `motor::proceso` a propósito: la única forma de llegar hasta aquí desde fuera es
/// `ServidorLocal::arrancar`, así que un servidor que Programator no arrancó jamás puede acabar en
/// esta ranura.
pub(super) fn registrar_para_cierre_ordenado(compartido: &Compartido) {
    let debil = Arc::downgrade(compartido);
    match REGISTRO_CIERRE.lock() {
        Ok(mut ranura) => *ranura = Some(debil),
        Err(envenenado) => *envenenado.into_inner() = Some(debil),
    }
}

/// Saca el `Child`, si sigue ahí, del hueco compartido con quien lo posee. Usada tanto por
/// `ServidorLocal::drop` (el camino normal) como por `manejar_cierre_ordenado` (Ctrl+C): sea cual
/// sea el que llegue primero, deja el hueco vacío y el otro no encuentra nada que matar dos veces.
pub(super) fn tomar_hijo(compartido: &Compartido) -> Option<Child> {
    match compartido.lock() {
        Ok(mut guardia) => guardia.take(),
        Err(envenenado) => envenenado.into_inner().take(),
    }
}

/// Mata y espera a `hijo`. Si ya había muerto por su cuenta, matarlo falla y no importa: el
/// objetivo es no dejarlo huérfano, no que la llamada a `kill` tenga éxito.
pub(super) fn matar_y_esperar(mut hijo: Child) {
    let _ = hijo.kill();
    let _ = hijo.wait();
}

/// Lo que ocurre de verdad al recibir Ctrl+C: si Programator arrancó un servidor y sigue vivo
/// (no se soltó ya por las buenas), lo mata y espera a que termine antes de devolver el control.
///
/// Separada de `registrar_cierre_ordenado` para poder probarla llamándola directamente, sin
/// necesidad de disparar una señal real ni de depender de que un proceso solo pueda instalar un
/// manejador de `ctrlc` una vez: eso haría que, en la suite de pruebas, la primera prueba que lo
/// registrara dejara sin poder registrarlo a todas las demás.
fn manejar_cierre_ordenado() {
    let debil = match REGISTRO_CIERRE.lock() {
        Ok(guardia) => guardia.clone(),
        Err(envenenado) => envenenado.into_inner().clone(),
    };

    let Some(debil) = debil else { return };
    let Some(compartido) = debil.upgrade() else {
        return;
    };

    if let Some(hijo) = tomar_hijo(&compartido) {
        matar_y_esperar(hijo);
    }
}

/// El código con que sale Programator tras Ctrl+C: 128 + 2 (SIGINT), la convención de las shells
/// para «terminado por una interrupción», que es como lo leen los guiones que lo lanzan.
const CODIGO_DE_SALIDA_POR_CTRL_C: i32 = 130;

/// Instala el manejador de Ctrl+C, una sola vez por proceso (una segunda llamada falla: así lo hace
/// `ctrlc`, y es la señal de que algo en el arranque de Programator lo está llamando dos veces).
///
/// **Por qué `ctrlc` y no `SetConsoleCtrlHandler` a mano:** esta última es una API de Win32 que solo
/// se puede llamar con `unsafe`, prohibido en todo el proyecto salvo dentro de dependencias ya
/// auditadas; `ctrlc` hace exactamente esa llamada por dentro, con una interfaz seguro y además
/// portable a Unix, por si Programator corre alguna vez fuera de Windows.
///
/// **Qué no cubre esto:** `ctrlc` solo engancha Ctrl+C (y Ctrl+Break) en la consola. Cerrar la
/// ventana de la consola con el ratón, matar el proceso desde el Administrador de tareas, o un
/// apagado/cierre de sesión de Windows mandan `CTRL_CLOSE_EVENT`, `CTRL_LOGOFF_EVENT` o
/// `CTRL_SHUTDOWN_EVENT` (o, en el caso del Administrador de tareas, ninguna señal en absoluto:
/// termina el proceso directamente). `ctrlc` 3.x no engancha esos otros eventos, así que por esos
/// caminos el servidor arrancado por Programator seguiría sobreviviendo con el modelo cargado en
/// VRAM. Queda dicho aquí y en el informe de la tarea: esta solución cierra el camino más común
/// (Ctrl+C en la terminal), no todos los caminos posibles.
pub fn registrar_cierre_ordenado() -> Resultado<()> {
    ctrlc::set_handler(|| {
        manejar_cierre_ordenado();
        // El manejador de `ctrlc` sustituye al de Windows, que por defecto sí termina el proceso;
        // sin esta llamada, Ctrl+C dejaría de cerrar Programator y solo correría esta limpieza.
        std::process::exit(CODIGO_DE_SALIDA_POR_CTRL_C);
    })
    .map_err(|causa| {
        Error::Configuracion(crate::motor::mensajes::manejador_de_ctrl_c_sin_instalar(
            &causa,
        ))
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::proceso::pruebas::MataAlSoltar;

    // Los tres casos de `manejar_cierre_ordenado` viven en una sola prueba, a propósito: la
    // función toca `REGISTRO_CIERRE`, que es un estático compartido por todo el binario de
    // pruebas. `cargo test` corre las pruebas en paralelo, así que dos pruebas separadas que
    // registraran ahí cada una lo suyo podrían pisarse la una a la otra (la segunda sobrescribe
    // la ranura antes de que la primera la lea) y producir un fallo que no depende del código,
    // sino de qué hilo gane la carrera. Con todo secuencial en una única prueba, esa carrera no
    // puede darse.
    #[test]
    fn el_cierre_ordenado_distingue_nada_registrado_servidor_ya_suelto_y_servidor_vivo() {
        // Caso 1: nada registrado todavía. No debe entrar en pánico ni fallar.
        manejar_cierre_ordenado();

        // Caso 2: un servidor que ya se soltó por las buenas (su `Drop` ya corrió). El `Weak` no
        // puede subir a `Arc`, así que no hay nada que matar.
        {
            let compartido: Compartido = std::sync::Arc::new(std::sync::Mutex::new(None));
            registrar_para_cierre_ordenado(&compartido);
            drop(compartido);

            manejar_cierre_ordenado();
        }

        // Caso 3: un servidor de verdad, todavía vivo. Un proceso que tarda treinta segundos vive
        // de sobra para dar tiempo a que el cierre ordenado actúe antes de que termine solo.
        let orden = crate::plataforma::ordenes_de_prueba::tardar_mucho();
        let hijo = std::process::Command::new(&orden[0])
            .args(&orden[1..])
            .spawn()
            .unwrap();
        let pid = hijo.id();
        let _guardia = MataAlSoltar(pid);

        let compartido: Compartido = std::sync::Arc::new(std::sync::Mutex::new(Some(hijo)));
        registrar_para_cierre_ordenado(&compartido);

        manejar_cierre_ordenado();

        assert!(
            compartido.lock().unwrap().is_none(),
            "el hueco compartido debía quedar vacío tras el cierre ordenado"
        );
        assert!(
            !crate::plataforma::ordenes_de_prueba::proceso_vive(pid),
            "el proceso registrado debía estar muerto tras el cierre ordenado"
        );
    }
}
