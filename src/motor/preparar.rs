//! Preparar el motor de inferencia para una pasada del ciclo.
//!
//! **Por qué vive aquí y no en `main`.** Decidir si se reutiliza un `llama-server` ajeno, si se
//! espera a que termine de cargar o si se arranca uno propio es el ciclo de vida del motor, no el
//! arranque del programa. `main` se ocupa de atender la línea de órdenes, montar la carpeta y
//! llevar el bucle; mezclar ahí esta decisión hacía que el módulo tuviera dos asuntos.
//!
//! La frontera con `proceso` es otra: allí se mide —qué responde el puerto, qué ficheros hay— y
//! aquí se decide qué hacer con esa medida.

use crate::arnes::herramientas::Repertorio;
use crate::arnes::sesion::EstadoMotor;
use crate::config::Config;
use crate::error::{Error, Resultado};
use crate::motor::{encaje, hardware, proceso, MotorLlama};

/// Prepara el motor de inferencia para una pasada que ya sabe que tiene al menos un encargo que
/// atender. Consulta el estado real del puerto configurado y decide, según la Tarea 16.2:
/// - **Listo**: se reutiliza tal cual. No se arranca nada, y no se cierra nunca al terminar: puede
///   ser del Director.
/// - **Cargando**: se espera al ciclo siguiente. Arrancar otro fallaría contra un puerto ocupado.
/// - **NoDisponible**: si la configuración trae `motor.binario` y `motor.modelo`, se arranca el
///   propio y se conserva vivo en `servidor_local` mientras dure el programa; si no, es un error
///   entendible («no hay motor y no sé arrancar uno»), no un pánico.
pub fn asegurar_motor_local(
    config: &Config,
    servidor_local: &mut Option<proceso::ServidorLocal>,
) -> Resultado<EstadoMotor> {
    match proceso::hay_servidor(config.motor.puerto) {
        proceso::EstadoServidor::Listo => {
            Ok(EstadoMotor::Disponible(Box::new(construir_motor(config))))
        }
        proceso::EstadoServidor::Cargando => Ok(EstadoMotor::Esperando),
        proceso::EstadoServidor::NoDisponible => {
            let (Some(binario), Some(modelo)) = (&config.motor.binario, &config.motor.modelo)
            else {
                return Err(Error::Configuracion(
                    "no hay ningún motor escuchando en el puerto configurado, y la \
                     configuración no trae «motor.binario»/«motor.modelo» con los que arrancar \
                     uno propio"
                        .to_string(),
                ));
            };
            let ruta_binario = config.resolver(binario);
            let ruta_modelo = config.resolver(modelo);
            // `resolver` mide con la GPU real; en las pruebas de `motor::encaje` es una tarjeta
            // inventada la que decide. Aquí, y no en `encaje`, es donde se pregunta al sistema
            // gráfico y donde se avisa: los módulos informan, `main` decide qué se cuenta.
            let resolucion = encaje::resolver(
                &config.motor,
                &ruta_modelo,
                hardware::describir_gpu().as_ref(),
            );
            // El informe de arranque ya contó el encaje con el que se pensaba lanzar el motor;
            // este es un segundo aviso, con la situación tal como está justo al arrancarlo, que
            // puede haber cambiado desde entonces.
            eprintln!("{}", mensaje_arranque_motor(&resolucion.aviso));
            let nuevo = proceso::ServidorLocal::arrancar(
                &config.motor,
                &ruta_binario,
                &ruta_modelo,
                resolucion.capas,
            )?;
            *servidor_local = Some(nuevo);
            // Recién arrancado, todavía está cargando el modelo: no tiene sentido lanzarle ya la
            // primera petición. Se espera al ciclo siguiente, que lo encontrará `Cargando` o
            // `Listo` según toque.
            Ok(EstadoMotor::Esperando)
        }
    }
}

/// Redacta el aviso emitido por terminal al arrancar el motor con el encaje de capas resuelto.
pub fn mensaje_arranque_motor(aviso_encaje: &str) -> String {
    format!("Arrancando el motor — encaje: {aviso_encaje}")
}

fn construir_motor(config: &Config) -> MotorLlama {
    let base_url = format!("http://{}:{}", super::HOST_LOCAL, config.motor.puerto);
    let modelo = config.motor.modelo.as_deref().unwrap_or("");
    MotorLlama::nuevo(&base_url, modelo)
        .con_tiempos_espera(
            std::time::Duration::from_secs(config.motor.tiempo_lectura_segundos),
            std::time::Duration::from_secs(config.motor.tiempo_escritura_segundos),
        )
        .con_herramientas(Repertorio::nombres_concedibles())
        .con_muestreo(config.muestreo.clone())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn mensaje_arranque_motor_incluye_el_aviso_de_encaje() {
        let msg = mensaje_arranque_motor("40 de 40 capas en GPU");
        assert_eq!(msg, "Arrancando el motor — encaje: 40 de 40 capas en GPU");
    }
}
