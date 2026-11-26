//! El ciclo del agente, en el orden en que pasa: arranca (configuración, aviso de versión e
//! informe de lo medido), resuelve y prepara la carpeta de trabajo y vigila el canal hasta que lo
//! paren. Aquí solo está ese orden; cada paso vive en su submódulo.

mod carpeta_de_trabajo;
mod configuracion;
mod guia_del_equipo;
mod historial_de_encaje;
mod informe_de_arranque;
mod modelos_que_faltan;
mod ninguno_cabe;
mod preparacion;
mod version_nueva;
mod vigilancia;

use crate::ruta_de_la_configuracion;
use programator::argumentos::Orden;
use programator::arranque;
use programator::error::Resultado;
use programator::motor::proceso;

/// El ciclo del agente: arranca, se prepara en la carpeta de trabajo y vigila el canal hasta que
/// lo paren. `--diagnostico` se queda en el informe de arranque.
pub(crate) fn ejecutar(orden: Orden) -> Resultado<()> {
    let ruta_config = ruta_de_la_configuracion();
    let config = configuracion::cargar_la_configuracion(&ruta_config, &orden)?;

    // Una sola vez, al arrancar el programa y antes de entrar en el bucle: si Programator llega a
    // arrancar su propio `llama-server`, esto es lo que lo mata al recibir Ctrl+C en vez de
    // dejarlo huérfano con el modelo cargado en VRAM.
    proceso::registrar_cierre_ordenado()?;

    let aviso_de_version = version_nueva::arrancar_el_aviso_de_version(&config);
    let encaje = informe_de_arranque::informar_del_arranque(&config, &orden);
    let registro = guia_del_equipo::leer_el_registro_de_modelos(&config);
    let disponibles = arranque::disponibles::reunir(&config, encaje.gpu.as_ref(), &registro);
    if let Some(disponibles) = &disponibles {
        eprintln!(
            "{}",
            arranque::bloque(&arranque::disponibles::componer(disponibles))
        );
    }
    version_nueva::contar_el_aviso_de_version(aviso_de_version);

    // `--diagnostico` llega hasta aquí y no más allá: sin abrir el diálogo de carpeta, sin
    // instalar normas, sin crear el directorio del canal y sin entrar en el bucle.
    if matches!(orden, Orden::Diagnostico { .. }) {
        return Ok(());
    }

    let carpeta = carpeta_de_trabajo::resolver_la_carpeta(&orden, &config)?;
    historial_de_encaje::registrar_el_encaje(&carpeta, &config, &encaje);
    let (carpeta_del_canal, instantanea) = preparacion::preparar_la_carpeta(
        &carpeta,
        &config,
        &guia_del_equipo::LoMedido {
            registro: &registro,
            disponibles: disponibles.as_ref(),
            tarjeta: encaje.nombre_de_la_tarjeta(),
        },
    )?;
    vigilancia::vigilar_el_canal(
        ruta_config,
        config,
        &carpeta,
        &carpeta_del_canal,
        instantanea,
    )
}
