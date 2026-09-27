//! Qué se le ha pedido a Programator en la línea de órdenes.
//!
//! Función pura sobre un vector de cadenas: no lee el entorno ni escribe en pantalla, para que
//! todas las combinaciones se puedan probar sin lanzar el programa.

/// Lo que el Director ha pedido al invocar el ejecutable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Orden {
    /// Arrancar y entrar en el ciclo de trabajo, sobre la carpeta pedida si se pidió alguna.
    Ejecutar { ruta: Option<String> },
    /// Escribir el texto de ayuda y terminar, sin arrancar el ciclo.
    Ayuda,
    /// Escribir la versión del programa y terminar, sin arrancar el ciclo.
    Version,
    /// Escribir el estado de la máquina, el motor y el modelo, y terminar sin arrancar el ciclo.
    Diagnostico,
    /// Un argumento que no se reconoce: el primero de la lista, que es el único que se mira, para
    /// poder nombrarlo.
    NoEntendido(String),
    /// Una opción que necesita un valor y no lo trae detrás.
    ///
    /// Existe aparte de `NoEntendido` porque el remedio es distinto: ahí falta entender qué se
    /// pedía, y aquí se entendió perfectamente y falta el dato.
    FaltaValor(String),
}

/// Interpreta los argumentos, **sin** el nombre del ejecutable.
///
/// Un argumento desconocido no arranca el ciclo: un proceso que va a correr desatendido durante
/// días no debe ponerse en marcha porque no se entendió lo que se le pedía.
///
/// El primer argumento reconocible o irreconocible decide: se mira solo el primero, sin mirar
/// por delante. Así, tanto la ayuda como el primer desconocido ganan si van primero, y solo
/// entonces.
pub fn interpretar(argumentos: &[String]) -> Orden {
    match argumentos.first() {
        None => Orden::Ejecutar { ruta: None },
        // La ayuda gana a lo que venga detrás: quien pregunta, pregunta. Pero solo si es ella
        // la que aparece primero.
        Some(argumento) => match argumento.as_str() {
            "--ayuda" | "-h" => Orden::Ayuda,
            "--version" => Orden::Version,
            "--diagnostico" => Orden::Diagnostico,
            // El valor va detrás, y tiene que parecer una carpeta: cualquier cosa que empiece por
            // «--» es otra opción, no la carpeta que alguien olvidó escribir.
            "--ruta" => match argumentos.get(1) {
                Some(valor) if !valor.starts_with("--") => Orden::Ejecutar {
                    ruta: Some(valor.clone()),
                },
                _ => Orden::FaltaValor("--ruta".to_string()),
            },
            otro => Orden::NoEntendido(otro.to_string()),
        },
    }
}

/// Columna en la que empieza la descripción de cada forma de uso.
///
/// Se compone, no se escribe a mano con espacios, porque el nombre del ejecutable cambia de
/// longitud entre Windows y el resto de sistemas y la tabla quedaría desalineada en uno de los
/// dos.
const COLUMNA_DE_LA_DESCRIPCION: usize = 31;

/// Una línea de la sección USO: la invocación a la izquierda y qué hace a la derecha.
///
/// Si la invocación no cabe en su columna, la descripción baja a la línea siguiente en vez de
/// empujar la tabla hacia la derecha. Así el texto sigue leyéndose en una terminal estrecha.
fn linea_de_uso(invocacion: &str, descripcion: &str) -> String {
    let sangrado = "  ";
    let escrito = sangrado.len() + invocacion.len();
    if escrito < COLUMNA_DE_LA_DESCRIPCION {
        let relleno = COLUMNA_DE_LA_DESCRIPCION - escrito;
        format!(
            "{sangrado}{invocacion}{:relleno$}{descripcion}
",
            ""
        )
    } else {
        format!(
            "{sangrado}{invocacion}
{:COLUMNA_DE_LA_DESCRIPCION$}{descripcion}
",
            ""
        )
    }
}

/// El texto que escribe `--ayuda`.
pub fn texto_de_ayuda(version: &str) -> String {
    let programa = crate::plataforma::NOMBRE_DEL_EJECUTABLE;
    let uso = [
        (
            programa.to_string(),
            "Arranca y entra en el ciclo de trabajo.",
        ),
        (
            format!("{programa} --ruta <carpeta>"),
            "Arranca sobre esa carpeta, sin abrir el diálogo.",
        ),
        (
            format!("{programa} --ayuda"),
            "Muestra esta ayuda y termina.",
        ),
        (
            format!("{programa} --version"),
            "Escribe la versión y termina.",
        ),
        (
            format!("{programa} --diagnostico"),
            "Escribe el estado de la máquina, el motor y el modelo.",
        ),
    ]
    .iter()
    .map(|(invocacion, que_hace)| linea_de_uso(invocacion, que_hace))
    .collect::<String>();

    format!(
        "Programator {version} — arnés que convierte un modelo local en un agente del canal

USO
{uso}
  «--ruta» manda sobre «[carpeta] ruta» del TOML, y «--diagnostico» termina
  sin arrancar el ciclo.

CONFIGURACIÓN
  Se lee de «programator.toml», en la misma carpeta que el ejecutable.
  Las rutas relativas se resuelven contra ese fichero, nunca contra el
  directorio desde el que lances el programa.

DÓNDE MIRAR
  Pruebas manuales:   docs/pruebas-manuales.md
  Defectos conocidos: docs/defectos-conocidos.md
"
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sin_argumentos_se_ejecuta_el_ciclo() {
        assert_eq!(interpretar(&args(&[])), Orden::Ejecutar { ruta: None });
    }

    #[test]
    fn la_ruta_se_recoge_con_su_valor() {
        assert_eq!(
            interpretar(&args(&["--ruta", "C:/proyectos/natureland"])),
            Orden::Ejecutar {
                ruta: Some("C:/proyectos/natureland".to_string())
            }
        );
    }

    #[test]
    fn una_ruta_sin_valor_no_arranca() {
        // Arrancar un proceso desatendido sobre una carpeta que no se sabe cuál es sería peor que
        // no arrancar.
        assert_eq!(
            interpretar(&args(&["--ruta"])),
            Orden::FaltaValor("--ruta".to_string())
        );
    }

    #[test]
    fn una_ruta_seguida_de_otra_orden_tampoco_arranca() {
        // «--ruta --version» es casi seguro un dedo que se saltó la carpeta.
        assert_eq!(
            interpretar(&args(&["--ruta", "--version"])),
            Orden::FaltaValor("--ruta".to_string())
        );
    }

    #[test]
    fn el_texto_de_ayuda_documenta_la_ruta() {
        let texto = texto_de_ayuda("0.9.0");
        assert!(texto.contains("--ruta"), "{texto}");
    }

    #[test]
    fn las_dos_formas_de_pedir_ayuda_valen() {
        assert_eq!(interpretar(&args(&["--ayuda"])), Orden::Ayuda);
        assert_eq!(interpretar(&args(&["-h"])), Orden::Ayuda);
    }

    #[test]
    fn se_reconocen_version_y_diagnostico() {
        assert_eq!(interpretar(&args(&["--version"])), Orden::Version);
        assert_eq!(interpretar(&args(&["--diagnostico"])), Orden::Diagnostico);
    }

    #[test]
    fn un_argumento_desconocido_no_arranca_el_ciclo() {
        // Arrancar un proceso desatendido porque no se entendió lo que se pedía es la peor
        // respuesta posible: se dice qué no se entendió y se termina.
        assert_eq!(
            interpretar(&args(&["--actualizar-motor"])),
            Orden::NoEntendido("--actualizar-motor".to_string())
        );
    }

    #[test]
    fn el_primer_argumento_que_no_se_entiende_es_el_que_se_nombra() {
        assert_eq!(
            interpretar(&args(&["--nada", "--ayuda"])),
            Orden::NoEntendido("--nada".to_string())
        );
    }

    #[test]
    fn la_ayuda_gana_a_lo_que_venga_detras() {
        // Pedir ayuda y otra cosa a la vez no es ambiguo: quien pregunta, pregunta.
        assert_eq!(interpretar(&args(&["--ayuda", "--version"])), Orden::Ayuda);
    }

    #[test]
    fn el_texto_de_ayuda_nombra_las_cuatro_formas_de_uso_y_la_version() {
        let texto = texto_de_ayuda("0.5.0");
        assert!(texto.contains("0.5.0"), "{texto}");
        for esperado in ["--ayuda", "--version", "--diagnostico", "programator.toml"] {
            assert!(
                texto.contains(esperado),
                "falta «{esperado}» en la ayuda:\n{texto}"
            );
        }
    }
}
