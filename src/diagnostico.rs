//! Lo que Programator averigua de la máquina al arrancar: qué GPU hay, qué motor está instalado,
//! qué modelo y cuántas capas caben.
//!
//! **Es la contraparte impura de `informe`.** Aquí se mide, se abre el GGUF y se mira el disco;
//! allí solo se redacta el texto con lo que ya se sabe. Separarlos es lo que permite probar todos
//! los desenlaces del informe sin GPU, sin motor y sin modelo, y es la misma frontera que hay
//! entre `encaje::decidir` —aritmética pura— y `encaje::resolver`, que orquesta.

use crate::config::Config;
use crate::informe::ResumenModelo;
use crate::motor::hardware::Gpu;
use crate::motor::proceso::MotorInstalado;
use crate::motor::{encaje, gguf, hardware, proceso};
use std::path::Path;

/// Lo que se ha podido averiguar de la máquina, el motor y el modelo al arrancar, ya reunido pero
/// todavía sin componer en texto: `ejecutar` lo pasa a `componer_informe`.
pub struct DatosReunidos {
    pub gpu: Option<Gpu>,
    pub motor: MotorInstalado,
    pub modelo: ResumenModelo,
    /// El texto listo para la línea «Encaje:»: el aviso de `encaje::resolver` con las capas que
    /// de verdad se pedirán, o una explicación de por qué no hay nada que calcular si no hay
    /// modelo declarado.
    pub encaje: String,
    /// Las capas que de verdad se van a pedir a `llama-server` (`resolucion.capas`), para quien
    /// quiera registrar el historial de encaje (INC-N07) sin tener que volver a calcularlas ni
    /// extraerlas del texto del aviso. `None` cuando no hay modelo declarado: no hay nada que
    /// registrar.
    pub capas_en_gpu: Option<u32>,
}

/// Reúne lo que se sabe de la máquina, el motor y el modelo. Cada dato que falte se convierte en
/// su desenlace correspondiente: **nada de esto aborta el arranque**, porque el canal se puede
/// sondear sin motor y el Director puede colocar las piezas con el programa ya corriendo.
///
/// `describir_gpu()` se llama **una sola vez** aquí, y el mismo valor sirve para el informe y para
/// `encaje::resolver`: medirla dos veces arriesgaría dos respuestas distintas en el mismo arranque.
pub fn reunir_datos_de_arranque(config: &Config) -> DatosReunidos {
    let gpu = hardware::describir_gpu();

    let motor = match &config.motor.binario {
        Some(binario) => proceso::revisar_motor(&config.resolver(binario)),
        None => MotorInstalado::NoDeclarado,
    };

    let (modelo, encaje, capas_en_gpu) = match &config.motor.modelo {
        None => (
            ResumenModelo::NoDeclarado,
            "no calculado: la configuración no declara ningún modelo".to_string(),
            None,
        ),
        Some(cadena) => {
            let ruta_modelo = config.resolver(cadena);
            let modelo = resumir_modelo(&ruta_modelo);
            let resolucion = encaje::resolver(&config.motor, &ruta_modelo, gpu.as_ref());
            // `resolucion.capas` es lo que de verdad se le pasa a `llama-server`: con
            // `capas_gpu` fija en el TOML, puede no coincidir con el número que menciona el
            // aviso (redactado siempre desde el cálculo automático), así que en principio se dice
            // aparte y no se da por sobreentendido. Pero el aviso ya nombra ese mismo número en
            // sus propias palabras cuando coincide —por ejemplo, «se piden 99 capas como hasta
            // ahora» cuando `capas_gpu = "auto"` no pudo calcular nada—, y repetirlo entonces
            // queda como un eco ridículo. Se comprueba con una búsqueda de texto, no comparando
            // con el cálculo interno de `encaje` (que no sale de `Resolucion`), así que basta con
            // que el aviso mencione la cifra en la forma «N capas» para no repetirla.
            let cifra_ya_mencionada = resolucion
                .aviso
                .contains(&format!("{} capas", resolucion.capas));
            let encaje = if cifra_ya_mencionada {
                resolucion.aviso
            } else {
                format!(
                    "{}; se pedirán {} capas",
                    resolucion.aviso, resolucion.capas
                )
            };
            (modelo, encaje, Some(resolucion.capas))
        }
    };

    DatosReunidos {
        gpu,
        motor,
        modelo,
        encaje,
        capas_en_gpu,
    }
}

/// Pesa y describe el modelo configurado, sin abortar si falta o si no se puede leer.
fn resumir_modelo(ruta: &Path) -> ResumenModelo {
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| ruta.display().to_string());

    if !ruta.is_file() {
        return ResumenModelo::Falta {
            ruta: ruta.to_path_buf(),
        };
    }

    match gguf::leer_modelo(ruta) {
        Ok(informe) => {
            // El tamaño en disco no sale de la cabecera del GGUF: `leer_modelo` no llega a leer
            // los pesos, así que se pregunta al sistema de ficheros aparte. Si esa pregunta
            // falla, el modelo pasa a `Ilegible`: un dato que no se sabe no puede aparecer como
            // si pesara 0,0 GiB.
            match std::fs::metadata(ruta) {
                Ok(metadatos_fichero) => ResumenModelo::Pesado {
                    nombre,
                    capas: informe.metadatos.numero_capas,
                    bytes: metadatos_fichero.len(),
                },
                Err(causa) => ResumenModelo::Ilegible {
                    nombre,
                    causa: format!("no se ha podido pesar el fichero: {causa}"),
                },
            }
        }
        Err(fallo) => ResumenModelo::Ilegible {
            nombre,
            causa: fallo.to_string(),
        },
    }
}
