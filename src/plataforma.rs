//! Lo que cambia de un sistema operativo a otro y el usuario ve escrito en la terminal.
//!
//! **Por qué existe este módulo.** Programator nació en Windows y sus mensajes decían
//! «programator.exe» en trece sitios distintos. En Linux el programa no se llama así, de modo que
//! cada uno de esos trece sitios habría pasado a dar una instrucción que no funciona. Eso no es un
//! detalle cosmético: es el arnés contando de sí mismo algo que no es cierto, que es justo el
//! defecto que persigue esta versión.
//!
//! La norma de la casa ya lo pedía por otra vía: las cadenas que ve el usuario se componen en un
//! solo sitio, no repartidas en literales gemelos. Aquí está ese sitio para lo que depende de la
//! plataforma.

/// Cómo se escribe el nombre del programa al decirle a alguien que lo ejecute.
///
/// En Windows el ejecutable lleva su extensión y se invoca tal cual. En cualquier otro sistema no
/// hay extensión, y quien esté en la carpeta tendrá que escribir `./programator`; el prefijo no se
/// pone aquí porque el nombre también aparece en frases donde no se está invocando nada.
#[cfg(windows)]
pub const NOMBRE_DEL_EJECUTABLE: &str = "programator.exe";

/// Cómo se escribe el nombre del programa fuera de Windows.
#[cfg(not(windows))]
pub const NOMBRE_DEL_EJECUTABLE: &str = "programator";

/// Una ruta de ejemplo con la forma que tienen las rutas en este sistema.
///
/// Sale en los mensajes que enseñan a usar «--ruta». Un ejemplo con letra de unidad en Linux, o
/// uno que empieza por barra en Windows, le dice a quien lo lee que el programa no sabe dónde
/// está, y a partir de ahí ya no se fía del resto.
#[cfg(windows)]
pub const EJEMPLO_DE_RUTA: &str = "C:/ruta/a/tu/proyecto";

/// Una ruta de ejemplo fuera de Windows.
#[cfg(not(windows))]
pub const EJEMPLO_DE_RUTA: &str = "/ruta/a/tu/proyecto";

/// Órdenes de consola equivalentes en cada sistema, para las pruebas que necesitan un
/// comprobador de mentira.
///
/// **Por qué está aquí y no en cada prueba.** La puerta de verificación ejecuta la orden que diga
/// el TOML, sea cual sea: esa parte es agnóstica y no hay nada que arreglar en ella. Lo que no era
/// agnóstico eran las pruebas, que llamaban a `cmd /C` directamente y por tanto solo podían
/// ejecutarse en Windows. Se descubrió al pasar la suite dentro de un Linux de verdad: siete
/// pruebas fallaban, todas por lo mismo.
///
/// Vive en este módulo porque es exactamente su asunto —lo que cambia de un sistema a otro—, y
/// solo se compila al construir las pruebas, así que no entra en el binario de nadie.
#[cfg(test)]
pub mod ordenes_de_prueba {
    /// Una orden que escribe ese texto y termina con ese código de salida.
    ///
    /// Es el comprobador de mentira que usan las pruebas de la puerta: con código cero simula uno
    /// que acepta el fichero, y con cualquier otro uno que lo rechaza devolviendo su salida.
    pub fn eco_y_codigo(texto: &str, codigo: i32) -> Vec<String> {
        #[cfg(windows)]
        {
            vec![
                "cmd".into(),
                "/C".into(),
                format!("echo {texto} & exit {codigo}"),
            ]
        }
        #[cfg(not(windows))]
        {
            vec![
                "sh".into(),
                "-c".into(),
                format!("echo '{texto}'; exit {codigo}"),
            ]
        }
    }

    /// Una orden que vuelca el fichero que se le pase, y falla si no existe.
    ///
    /// Sirve para comprobar que el marcador `{fichero}` se sustituye de verdad: si no se
    /// sustituyera, la orden buscaría uno llamado literalmente «{fichero}» y fallaría.
    pub fn volcar_fichero() -> Vec<String> {
        #[cfg(windows)]
        {
            vec!["cmd".into(), "/C".into(), "type".into(), "{fichero}".into()]
        }
        #[cfg(not(windows))]
        {
            vec!["cat".into(), "{fichero}".into()]
        }
    }

    /// Mata sin contemplaciones el proceso con ese identificador.
    ///
    /// Se usa como red de seguridad: si una prueba entra en pánico a mitad, su proceso auxiliar
    /// se queda colgado en la máquina de quien ejecutó la suite. No devuelve si lo consiguió,
    /// porque lo normal es que ya estuviera muerto y eso no es un fallo.
    pub fn matar_proceso(identificador: u32) {
        #[cfg(windows)]
        let mut orden = {
            let mut o = std::process::Command::new("taskkill");
            o.args(["/F", "/PID", &identificador.to_string()]);
            o
        };
        #[cfg(not(windows))]
        let mut orden = {
            let mut o = std::process::Command::new("kill");
            o.args(["-9", &identificador.to_string()]);
            o
        };
        let _ = orden.output();
    }

    /// ¿Sigue vivo el proceso con ese identificador?
    ///
    /// Se apoya en programas del sistema en vez de en llamadas de bajo nivel, para no tener que
    /// escribir interfaz nativa a mano en una ayuda de pruebas. En Windows se pregunta a
    /// `tasklist`; fuera de Windows se manda la señal cero, que no hace nada salvo fallar si el
    /// proceso ya no está.
    ///
    /// **Cuidado con los procesos zombi.** En los sistemas de tipo Unix, un hijo muerto al que
    /// nadie ha recogido sigue existiendo para la señal cero. Aquí no molesta porque quien lo mata
    /// es `matar_y_esperar`, que espera por él y por tanto lo recoge; si alguna vez esta ayuda se
    /// usa en otro sitio, conviene recordarlo.
    pub fn proceso_vive(identificador: u32) -> bool {
        #[cfg(windows)]
        {
            let salida = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {identificador}"), "/NH"])
                .output();
            match salida {
                Ok(salida) => {
                    String::from_utf8_lossy(&salida.stdout).contains(&identificador.to_string())
                }
                Err(_) => false,
            }
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("kill")
                .args(["-0", &identificador.to_string()])
                .output()
                .map(|salida| salida.status.success())
                .unwrap_or(false)
        }
    }

    /// Una orden que tarda mucho más de lo que ninguna prueba va a esperar.
    ///
    /// Es la que comprueba que el plazo de la puerta se respeta y que un comprobador colgado no
    /// deja el ciclo parado para siempre. También sirve para lanzar un proceso de mentira que
    /// siga vivo el rato suficiente: el primer elemento es el programa y el resto sus argumentos.
    pub fn tardar_mucho() -> Vec<String> {
        #[cfg(windows)]
        {
            vec![
                "cmd".into(),
                "/C".into(),
                "ping -n 30 127.0.0.1 > NUL".into(),
            ]
        }
        #[cfg(not(windows))]
        {
            vec!["sleep".into(), "30".into()]
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_nombre_del_ejecutable_nunca_esta_vacio_ni_lleva_espacios() {
        // Se compone dentro de frases y se le pegan argumentos detrás. Un nombre vacío dejaría
        // instrucciones sin verbo, y uno con espacios las partiría en dos.
        assert!(!NOMBRE_DEL_EJECUTABLE.is_empty());
        assert!(!NOMBRE_DEL_EJECUTABLE.contains(' '));
    }

    #[test]
    #[cfg(windows)]
    fn en_windows_lleva_su_extension() {
        assert_eq!(NOMBRE_DEL_EJECUTABLE, "programator.exe");
    }

    #[test]
    #[cfg(not(windows))]
    fn fuera_de_windows_no_lleva_extension() {
        assert_eq!(NOMBRE_DEL_EJECUTABLE, "programator");
        assert!(!NOMBRE_DEL_EJECUTABLE.ends_with(".exe"));
    }
}
