//! El texto que Programator escribe al arrancar.
//!
//! **No mide, no lee ficheros y no calcula nada**: recibe lo que ya se sabe y devuelve texto. Esa
//! pobreza deliberada es lo que permite probar todos los desenlaces —sin GPU, sin motor, sin
//! modelo— con cuatro literales y sin tocar el hardware.

use crate::motor::hardware::Gpu;
use crate::motor::proceso::MotorInstalado;
use std::path::{Path, PathBuf};

/// Qué se ha podido averiguar del modelo configurado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumenModelo {
    Pesado {
        nombre: String,
        capas: Option<u64>,
        bytes: u64,
    },
    /// La configuración no declara ningún modelo.
    NoDeclarado,
    Falta {
        ruta: PathBuf,
    },
    Ilegible {
        nombre: String,
        causa: String,
    },
}

/// Todo lo que el informe necesita saber. Lo reúne `main`, que es quien mide.
pub struct DatosDeArranque<'a> {
    pub version: &'a str,
    /// `None` cuando `[carpeta] ruta` no está fijada en `programator.toml`. Con `--diagnostico`
    /// esto pasa a propósito: el informe se compone antes de abrir ningún diálogo, así que la
    /// carpeta puede no conocerse todavía.
    pub carpeta: Option<&'a Path>,
    pub gpu: Option<&'a Gpu>,
    pub motor: &'a MotorInstalado,
    /// La cadena tal cual viene de `config.motor.binario` (relativa, tal como está en el TOML),
    /// para decir junto a qué CUDA se encontró o no se encontró, cuando hay más de un
    /// `llama-server.exe` instalado en la máquina.
    pub ruta_motor: Option<&'a str>,
    pub modelo: ResumenModelo,
    /// El aviso que produce `motor::encaje::resolver`, tal cual.
    pub encaje: &'a str,
    /// `[carpeta] preguntar_siempre` del TOML, tal cual.
    ///
    /// Hace falta aquí porque cambia qué fuentes siguen en juego cuando la carpeta no está fijada:
    /// con la bandera puesta, la última carpeta usada deja de contar, y nombrarla en el informe
    /// sería ofrecer una salida que el arnés ya no va a tomar.
    pub preguntar_siempre: bool,
}

/// Ancho al que se alinean las etiquetas, para que las cifras queden en columna y se puedan
/// comparar de un vistazo.
const ANCHO_ETIQUETA: usize = 20;

/// Compone el informe de arranque o diagnóstico formateado para la terminal.
///
/// Función pura: recibe los datos consolidados del arranque (versión, carpeta, GPU,
/// binario del motor, modelo GGUF y encaje de capas resuelto) y redacta un bloque
/// alineado en columnas, legible de un vistazo para el operador o scripts de diagnóstico.
pub fn componer_informe(datos: &DatosDeArranque) -> String {
    let mut lineas = Vec::new();
    lineas.push(format!(
        "Programator {} — asistente de código local en segundo plano\n",
        datos.version
    ));
    lineas.push(fila(
        "Carpeta de trabajo:",
        &describir_carpeta(datos.carpeta, datos.preguntar_siempre),
    ));
    lineas.push(fila("GPU:", &describir_gpu(datos.gpu)));
    lineas.push(fila(
        "Motor:",
        &describir_motor(datos.motor, datos.ruta_motor),
    ));
    lineas.push(fila("Modelo:", &describir_modelo(&datos.modelo)));
    lineas.push(fila(
        "Encaje:",
        if datos.encaje.is_empty() {
            "no calculado"
        } else {
            datos.encaje
        },
    ));
    lineas.join("\n")
}

fn fila(etiqueta: &str, valor: &str) -> String {
    format!("{etiqueta:<ANCHO_ETIQUETA$}{valor}")
}

fn describir_carpeta(carpeta: Option<&Path>, preguntar_siempre: bool) -> String {
    match carpeta {
        Some(c) => c.display().to_string(),
        None if preguntar_siempre => SIN_CARPETA_PREGUNTANDO.to_string(),
        None => SIN_CARPETA_CON_MEMORIA.to_string(),
    }
}

/// La línea del informe cuando no hay carpeta fijada y `preguntar_siempre` está puesto.
const SIN_CARPETA_PREGUNTANDO: &str =
    "no fijada (elige la carpeta de trabajo en la ventana emergente)";

/// La línea del informe cuando no hay carpeta fijada y la memoria de la última sigue en juego.
const SIN_CARPETA_CON_MEMORIA: &str =
    "no fijada (usará la última carpeta guardada o la que elijas en la ventana emergente)";

fn describir_gpu(gpu: Option<&Gpu>) -> String {
    match gpu {
        Some(g) => format!(
            "{} · {} libres de {}",
            g.nombre,
            en_gib(g.vram_libre),
            en_gib(g.vram_total)
        ),
        None => "no se ha encontrado ninguna GPU dedicada".to_string(),
    }
}

fn describir_motor(motor: &MotorInstalado, ruta_motor: Option<&str>) -> String {
    match motor {
        // Sin la extension a proposito: el nombre real del fichero ya va detrás, en la ruta que
        // añade «con_ruta», y escribirlo aquí con «.exe» era dar por hecho el sistema.
        MotorInstalado::ListoConCuda => con_ruta("llama-server con CUDA", ruta_motor),
        MotorInstalado::SoloCpu => con_ruta(
            "llama-server SIN CUDA (solo CPU) — las capas en GPU no tendrán efecto",
            ruta_motor,
        ),
        MotorInstalado::Falta { ruta } => format!("FALTA «{}»", ruta.display()),
        MotorInstalado::NoDeclarado => "la configuración no declara ninguno".to_string(),
    }
}

/// Añade «, en {ruta}» cuando se conoce la ruta configurada del motor. Solo tiene sentido para los
/// dos desenlaces que no nombran ya una ruta por su cuenta (`Falta` y `NoDeclarado` la nombran o
/// explican su ausencia a su manera).
fn con_ruta(texto: &str, ruta_motor: Option<&str>) -> String {
    match ruta_motor {
        Some(ruta) => format!("{texto}, en {ruta}"),
        None => texto.to_string(),
    }
}

fn describir_modelo(modelo: &ResumenModelo) -> String {
    match modelo {
        ResumenModelo::Pesado {
            nombre,
            capas,
            bytes,
        } => match capas {
            Some(n) => format!("{nombre} · {n} capas · {}", en_gib(*bytes)),
            None => format!(
                "{nombre} · {} (no declara cuántas capas tiene)",
                en_gib(*bytes)
            ),
        },
        ResumenModelo::NoDeclarado => "la configuración no declara ninguno".to_string(),
        ResumenModelo::Falta { ruta } => format!("FALTA «{}»", ruta.display()),
        ResumenModelo::Ilegible { nombre, causa } => {
            format!("{nombre} — no se ha podido pesar: {causa}")
        }
    }
}

/// Factor de conversión de bytes a gibibytes en coma flotante (§4.3).
const BYTES_POR_GIB_F64: f64 = 1024.0 * 1024.0 * 1024.0;

/// Bytes a GiB con un decimal y **coma** decimal, que es como se escribe en español.
///
/// Deliberadamente distinta de la `en_gib` de `motor::encaje`, que usa punto: aquella cifra la lee
/// una máquina (se compara con otras cifras internas) y esta la lee una persona. No se unifican.
fn en_gib(bytes: u64) -> String {
    let gib = bytes as f64 / BYTES_POR_GIB_F64;
    format!("{gib:.1} GiB").replace('.', ",")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::hardware::Gpu;
    use std::path::Path;

    fn gpu() -> Gpu {
        Gpu {
            nombre: "NVIDIA GeForce RTX 4070 Ti SUPER".to_string(),
            vram_total: 16_841_179_136,
            vram_libre: 16_035_872_768,
        }
    }

    fn datos_completos() -> DatosDeArranque<'static> {
        DatosDeArranque {
            version: "0.5.0",
            carpeta: Some(Path::new("O:/proyecto/prueba1")),
            gpu: None,
            motor: &MotorInstalado::ListoConCuda,
            ruta_motor: None,
            modelo: ResumenModelo::NoDeclarado,
            encaje: "",
            preguntar_siempre: false,
        }
    }

    #[test]
    fn el_informe_presenta_la_aplicacion_y_la_version() {
        let texto = componer_informe(&datos_completos());
        assert!(texto.contains("Programator 0.5.0"), "{texto}");
        assert!(!texto.contains("--ayuda"), "{texto}");
    }

    #[test]
    fn ninguna_linea_falta_aunque_no_se_sepa_el_dato() {
        // Una línea ausente se confunde con un dato que no importa. Las cinco están siempre.
        let texto = componer_informe(&datos_completos());
        for etiqueta in [
            "Carpeta de trabajo:",
            "GPU:",
            "Motor:",
            "Modelo:",
            "Encaje:",
        ] {
            assert!(texto.contains(etiqueta), "falta «{etiqueta}»:\n{texto}");
        }
    }

    #[test]
    fn sin_carpeta_fijada_lo_dice_en_vez_de_dar_una_ruta_vacia() {
        let datos = DatosDeArranque {
            carpeta: None,
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(texto.contains("no fijada"), "{texto}");
        assert!(texto.contains("ventana emergente"), "{texto}");
        assert!(
            !texto.contains("se decidirá al arrancar"),
            "el informe no debe incluir jerga interna:\n{texto}"
        );
    }

    #[test]
    fn con_preguntar_siempre_el_informe_no_ofrece_la_ultima_carpeta_usada() {
        let datos = DatosDeArranque {
            carpeta: None,
            preguntar_siempre: true,
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(texto.contains("no fijada"), "{texto}");
        assert!(texto.contains("ventana emergente"), "{texto}");
        assert!(
            !texto.contains("última carpeta guardada"),
            "el informe no ofrece la carpeta guardada si preguntar_siempre está activo:\n{texto}"
        );
    }

    #[test]
    fn con_gpu_se_dicen_los_gib_libres_y_los_totales() {
        let g = gpu();
        let datos = DatosDeArranque {
            gpu: Some(&g),
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(texto.contains("RTX 4070 Ti SUPER"), "{texto}");
        assert!(
            texto.contains("14,9 GiB"),
            "los libres, con coma decimal:\n{texto}"
        );
        assert!(texto.contains("15,7 GiB"), "y los totales:\n{texto}");
    }

    #[test]
    fn sin_gpu_lo_dice_en_vez_de_callarse() {
        let texto = componer_informe(&datos_completos());
        assert!(texto.contains("no se ha encontrado"), "{texto}");
    }

    #[test]
    fn un_motor_sin_cuda_se_avisa_con_su_consecuencia() {
        let datos = DatosDeArranque {
            motor: &MotorInstalado::SoloCpu,
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(texto.contains("SIN CUDA"), "{texto}");
        assert!(
            texto.contains("no tendrán efecto"),
            "decir que no hay CUDA sin decir qué implica no sirve de nada:\n{texto}"
        );
    }

    #[test]
    fn con_ruta_de_motor_conocida_se_dice_cual_es() {
        // En una máquina con dos `llama-server.exe` (Tarea A-2), decir «con CUDA» sin decir cuál
        // deja al Director sin saber si está mirando el correcto.
        let datos = DatosDeArranque {
            motor: &MotorInstalado::ListoConCuda,
            ruta_motor: Some("herramientas/llama-server.exe"),
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(
            texto.contains("con CUDA, en herramientas/llama-server.exe"),
            "{texto}"
        );
    }

    #[test]
    fn un_motor_que_falta_dice_que_ruta_se_esperaba() {
        let falta = MotorInstalado::Falta {
            ruta: "herramientas/llama-server.exe".into(),
        };
        let datos = DatosDeArranque {
            motor: &falta,
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(texto.contains("FALTA"), "{texto}");
        assert!(texto.contains("llama-server.exe"), "{texto}");
    }

    #[test]
    fn un_motor_no_declarado_no_se_confunde_con_uno_que_falta() {
        // Un hueco de configuración («no hay nada que mirar») no es lo mismo que una pieza
        // perdida («debería estar y no está»): confundirlos le haría pensar al Director que hay
        // algo roto en el disco cuando en realidad es que `motor.binario` no está en el TOML.
        let datos = DatosDeArranque {
            motor: &MotorInstalado::NoDeclarado,
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(
            texto.contains("no declara ninguno"),
            "tiene que decir que no se declaró motor:\n{texto}"
        );
        assert!(
            !texto.contains("FALTA"),
            "no puede sonar a que falta un fichero que sí se esperaba:\n{texto}"
        );
    }

    #[test]
    fn un_modelo_pesado_dice_sus_capas_y_su_tamano() {
        let datos = DatosDeArranque {
            modelo: ResumenModelo::Pesado {
                nombre: "devstral-small-2-24b-Q4_K_M.gguf".to_string(),
                capas: Some(40),
                bytes: 14_334_446_752,
            },
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(texto.contains("devstral"), "{texto}");
        assert!(texto.contains("40 capas"), "{texto}");
        // Corrección vinculante del encargo: 14.334.446.752 bytes ÷ 1024³ da 13,349993… GiB, que
        // redondeado a un decimal es 13,3 GiB, no 13,4 GiB. El «13,4 GiB» de la especificación es
        // un redondeo humano de 13,35 hecho a mano; ningún redondeo correcto a un decimal da 13,4.
        assert!(texto.contains("13,3 GiB"), "{texto}");
    }

    #[test]
    fn un_modelo_pesado_sin_capas_declaradas_lo_dice_en_vez_de_inventar_un_numero() {
        // El único desenlace de `ResumenModelo` que quedaba sin literal propio (Tarea A-7): la
        // cabecera del GGUF no trae `block_count`, y el informe tiene que decirlo en vez de
        // callarse la cifra o fingir que no hay capas.
        let datos = DatosDeArranque {
            modelo: ResumenModelo::Pesado {
                nombre: "modelo-sin-metadatos.gguf".to_string(),
                capas: None,
                bytes: 1_073_741_824,
            },
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(texto.contains("modelo-sin-metadatos.gguf"), "{texto}");
        assert!(texto.contains("no declara cuántas capas tiene"), "{texto}");
        assert!(texto.contains("1,0 GiB"), "{texto}");
    }

    #[test]
    fn un_modelo_que_falta_dice_que_ruta_se_esperaba() {
        // El §3 de la especificación pone este desenlace como uno de sus dos ejemplos canónicos
        // del informe («la carpeta a medio montar»): motor y modelo faltando a la vez, cada uno
        // nombrando la ruta que se esperaba. En la línea de
        // `un_motor_que_falta_dice_que_ruta_se_esperaba`.
        let datos = DatosDeArranque {
            modelo: ResumenModelo::Falta {
                ruta: "modelos/devstral-small-2-24b-Q4_K_M.gguf".into(),
            },
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(texto.contains("FALTA"), "{texto}");
        assert!(
            texto.contains("devstral-small-2-24b-Q4_K_M.gguf"),
            "{texto}"
        );
    }

    #[test]
    fn un_modelo_ilegible_dice_la_causa() {
        let datos = DatosDeArranque {
            modelo: ResumenModelo::Ilegible {
                nombre: "roto.gguf".to_string(),
                causa: "número mágico inválido".to_string(),
            },
            ..datos_completos()
        };

        let texto = componer_informe(&datos);

        assert!(texto.contains("número mágico inválido"), "{texto}");
    }
}
