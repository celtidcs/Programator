//! La puerta que comprueba una propuesta en cuanto se escribe.
//!
//! **Por qué existe.** En la evaluación del 22/09/2026, dos de las siete entregas del modelo no
//! ejecutaban —un módulo Lua con una `ñ` en un identificador y una consulta SQL que usaba el alias
//! de una función de ventana en su propio `WHERE`— y en los dos casos **el modelo publicó que
//! había entregado la solución**. Nadie se enteró hasta que una persona los compiló a mano. Un
//! agente que afirma haber entregado lo que no funciona no se puede dejar solo.
//!
//! **Qué NO hace:** ejecutar lo que el modelo escribe. Comprueba que el lenguaje lo acepta
//! —compilar, o analizar la sintaxis—, que es otra cosa. `python -m py_compile` no corre el
//! fichero.
//!
//! **Quién elige el comprobador:** el Director, en `[verificacion]` de su TOML. Ni el arnés
//! incrusta comandos ni el modelo elige ninguno: el modelo solo pone el nombre del fichero, y ese
//! ya está confinado.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Marcador que se sustituye por la ruta de la propuesta en los argumentos del comprobador.
const MARCADOR_FICHERO: &str = "{fichero}";

/// Cuánta salida del comprobador se le pasa al modelo. Un error de `rustc` puede ocupar pantallas
/// enteras y el contexto es finito: con esto sobra para saber qué falló y dónde.
const MAXIMO_SALIDA: usize = 1200;

/// Cada cuánto se comprueba si el comprobador ya terminó, mientras se espera su tiempo máximo.
const LATIDO_ESPERA: Duration = Duration::from_millis(50);

/// Qué se sabe de una propuesta después de intentar comprobarla.
///
/// Los cuatro desenlaces se distinguen a propósito: llamar «correcta» a una propuesta que no se ha
/// comprobado sería peor que no comprobar nada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Veredicto {
    /// No hay comprobador configurado para esa extensión.
    SinComprobador { extension: String },
    /// Hay comprobador, pero no se pudo ejecutar: no está instalado, o reventó al lanzarse.
    NoSePudo { motivo: String },
    /// El comprobador terminó y aceptó el fichero.
    Pasa { orden: String },
    /// El comprobador terminó y rechazó el fichero.
    NoPasa { orden: String, salida: String },
}

impl Veredicto {
    /// Lo que se le añade al modelo detrás de «escrita la propuesta …».
    ///
    /// Está redactado para que el modelo pueda actuar: cuando falla, dice explícitamente que la
    /// corrija y la vuelva a escribir, porque el fichero ya está en disco y podría darse por
    /// entregado.
    pub fn para_el_modelo(&self) -> String {
        match self {
            Veredicto::SinComprobador { extension } => {
                format!("sin verificar: no hay comprobador configurado para «.{extension}»")
            }
            Veredicto::NoSePudo { motivo } => {
                format!("no se ha podido verificar: {motivo}")
            }
            Veredicto::Pasa { orden } => {
                format!("verificada con «{orden}»: pasa la comprobación")
            }
            Veredicto::NoPasa { orden, salida } => format!(
                "NO pasa la comprobación de «{orden}»:\n{salida}\nCorrígela y vuelve a escribirla."
            ),
        }
    }

    /// La línea corta que el arnés deja en el buzón, junto a lo que el modelo haya redactado.
    ///
    /// **Esto es lo que impide que «he entregado la solución» sea la última palabra.** El cuerpo lo
    /// escribe el modelo; el veredicto, no.
    pub fn para_el_canal(&self, nombre: &str) -> String {
        match self {
            // No dice «sin verificar»: en NatureLand esa frase se leyó como «verificada sin
            // hallazgos» durante cuatro entregas seguidas, y en el único lenguaje de aquel
            // proyecto. Lo que falta es un comprobador, y quien lea el canal tiene que saberlo.
            Veredicto::SinComprobador { extension } => format!(
                "`{nombre}`: ⚠️ SIN COMPROBAR — no hay comprobador configurado para «.{extension}»"
            ),
            Veredicto::NoSePudo { .. } => format!("`{nombre}`: no se pudo verificar"),
            Veredicto::Pasa { .. } => format!("`{nombre}`: ✅ pasa la comprobación"),
            Veredicto::NoPasa { .. } => format!("`{nombre}`: ❌ NO pasa la comprobación"),
        }
    }
}

/// Los comprobadores configurados: extensión (sin punto, en minúsculas) → orden a ejecutar.
pub type Comprobadores = BTreeMap<String, Vec<String>>;

/// Comprueba `fichero` con el comprobador que corresponda a su extensión.
///
/// `directorio_de_trabajo` es donde se ejecuta la orden: conviene que sea temporal, porque
/// comprobadores como `rustc --emit=metadata` dejan artefactos y el canal no es sitio para ellos.
pub fn verificar(
    fichero: &Path,
    comprobadores: &Comprobadores,
    tiempo_maximo: Duration,
    directorio_de_trabajo: &Path,
) -> Veredicto {
    let extension = fichero
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    let Some(orden) = comprobadores.get(&extension) else {
        return Veredicto::SinComprobador { extension };
    };

    let Some((programa, resto)) = orden.split_first() else {
        return Veredicto::NoSePudo {
            motivo: format!("el comprobador de «.{extension}» está configurado vacío"),
        };
    };

    // La ruta se pasa absoluta siempre. El comprobador se ejecuta en un directorio temporal, así
    // que una ruta relativa se resolvería contra ese directorio y el comprobador diría que el
    // fichero no existe. En producción llega absoluta desde `Ambito::resolver`, pero depender de
    // eso sería una trampa esperando a quien llame de otro sitio.
    let ruta = std::fs::canonicalize(fichero)
        .unwrap_or_else(|_| fichero.to_path_buf())
        .display()
        .to_string();
    // `canonicalize` devuelve en Windows el prefijo extendido `\\?\`, que algunos programas no
    // digieren. Se quita: la ruta sigue siendo absoluta y válida sin él.
    let ruta = ruta.strip_prefix(r"\\?\").unwrap_or(&ruta).to_string();

    let argumentos: Vec<String> = resto
        .iter()
        .map(|a| a.replace(MARCADOR_FICHERO, &ruta))
        .collect();
    let descripcion = format!("{programa} {}", resto.join(" "));

    // Sin shell: el programa y sus argumentos van por separado, así que nada de lo que venga en un
    // nombre de fichero puede convertirse en otra orden.
    let hijo = Command::new(programa)
        .args(&argumentos)
        .current_dir(directorio_de_trabajo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut hijo = match hijo {
        Ok(h) => h,
        Err(e) => {
            return Veredicto::NoSePudo {
                motivo: format!("no se pudo lanzar «{programa}»: {e}"),
            }
        }
    };

    // Espera con tope. `Child` no sabe esperar con límite por sí solo, y un comprobador colgado no
    // puede colgar un arnés pensado para vivir días.
    let limite = Instant::now() + tiempo_maximo;
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= limite {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    return Veredicto::NoSePudo {
                        motivo: format!(
                            "«{programa}» seguía trabajando tras {} segundos y se ha cortado",
                            tiempo_maximo.as_secs()
                        ),
                    };
                }
                std::thread::sleep(LATIDO_ESPERA);
            }
            Err(e) => {
                return Veredicto::NoSePudo {
                    motivo: format!("no se pudo esperar a «{programa}»: {e}"),
                }
            }
        }
    }

    let salida = match hijo.wait_with_output() {
        Ok(s) => s,
        Err(e) => {
            return Veredicto::NoSePudo {
                motivo: format!("no se pudo leer la salida de «{programa}»: {e}"),
            }
        }
    };

    if salida.status.success() {
        return Veredicto::Pasa { orden: descripcion };
    }

    // Los comprobadores escriben sus errores por stderr, pero no todos: se juntan las dos salidas
    // para no perder el motivo del rechazo por una cuestión de canal.
    let mut texto = String::from_utf8_lossy(&salida.stderr).trim().to_string();
    if texto.is_empty() {
        texto = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    }
    if texto.is_empty() {
        texto = format!("terminó con {} y sin decir nada", salida.status);
    }

    Veredicto::NoPasa {
        orden: descripcion,
        salida: recortar(&texto, MAXIMO_SALIDA),
    }
}

/// Recorta por caracteres, no por bytes: un error de compilación puede traer acentos y partir un
/// carácter a la mitad dejaría el texto roto justo donde hay que leerlo.
fn recortar(texto: &str, maximo: usize) -> String {
    if texto.chars().count() <= maximo {
        return texto.to_string();
    }
    let recortado: String = texto.chars().take(maximo).collect();
    format!("{recortado}\n[…recortado]")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_comprobador_el_canal_lo_dice_y_nombra_la_extension() {
        // «sin verificar» se leyó en NatureLand como «verificada sin hallazgos», durante cuatro
        // entregas seguidas y en el único lenguaje de aquel proyecto.
        let veredicto = Veredicto::SinComprobador {
            extension: "cs".to_string(),
        };
        let linea = veredicto.para_el_canal("ContrasteGalon.cs");

        assert!(linea.contains("SIN COMPROBAR"), "{linea}");
        assert!(linea.contains(".cs"), "{linea}");
        assert!(
            !linea.contains("sin verificar"),
            "el texto viejo sigue ahí: {linea}"
        );
    }

    /// Una orden que existe en cualquier Windows y termina bien o mal según convenga.
    fn orden_que(pasa: bool) -> Vec<String> {
        let codigo = if pasa { "0" } else { "1" };
        vec![
            "cmd".into(),
            "/C".into(),
            format!("echo lo que dijo el comprobador & exit {codigo}"),
        ]
    }

    fn comprobadores(extension: &str, orden: Vec<String>) -> Comprobadores {
        let mut mapa = Comprobadores::new();
        mapa.insert(extension.to_string(), orden);
        mapa
    }

    fn fichero_de_prueba(dir: &tempfile::TempDir, nombre: &str) -> std::path::PathBuf {
        let ruta = dir.path().join(nombre);
        std::fs::write(&ruta, b"contenido").unwrap();
        ruta
    }

    #[test]
    fn sin_comprobador_para_esa_extension_se_dice_y_no_se_inventa_un_aprobado() {
        let dir = tempfile::tempdir().unwrap();
        let fichero = fichero_de_prueba(&dir, "propuesta.ts");

        let veredicto = verificar(
            &fichero,
            &comprobadores("py", orden_que(true)),
            Duration::from_secs(5),
            dir.path(),
        );

        assert_eq!(
            veredicto,
            Veredicto::SinComprobador {
                extension: "ts".to_string()
            }
        );
        assert!(
            veredicto.para_el_modelo().contains("sin verificar"),
            "no puede sonar a aprobado: {}",
            veredicto.para_el_modelo()
        );
    }

    #[test]
    fn un_comprobador_que_acepta_el_fichero_dice_que_pasa() {
        let dir = tempfile::tempdir().unwrap();
        let fichero = fichero_de_prueba(&dir, "propuesta.py");

        let veredicto = verificar(
            &fichero,
            &comprobadores("py", orden_que(true)),
            Duration::from_secs(10),
            dir.path(),
        );

        assert!(matches!(veredicto, Veredicto::Pasa { .. }), "{veredicto:?}");
    }

    #[test]
    fn un_comprobador_que_rechaza_devuelve_su_salida_para_que_el_modelo_corrija() {
        let dir = tempfile::tempdir().unwrap();
        let fichero = fichero_de_prueba(&dir, "propuesta.py");

        let veredicto = verificar(
            &fichero,
            &comprobadores("py", orden_que(false)),
            Duration::from_secs(10),
            dir.path(),
        );

        let Veredicto::NoPasa { salida, .. } = &veredicto else {
            panic!("debería rechazarlo: {veredicto:?}");
        };
        assert!(
            salida.contains("lo que dijo el comprobador"),
            "la salida del comprobador es lo único que le sirve al modelo: {salida}"
        );
        assert!(veredicto.para_el_modelo().contains("vuelve a escribirla"));
    }

    #[test]
    fn la_extension_no_distingue_mayusculas() {
        let dir = tempfile::tempdir().unwrap();
        let fichero = fichero_de_prueba(&dir, "PROPUESTA.PY");

        let veredicto = verificar(
            &fichero,
            &comprobadores("py", orden_que(true)),
            Duration::from_secs(10),
            dir.path(),
        );

        assert!(matches!(veredicto, Veredicto::Pasa { .. }), "{veredicto:?}");
    }

    #[test]
    fn un_comprobador_que_no_esta_instalado_no_tumba_nada() {
        let dir = tempfile::tempdir().unwrap();
        let fichero = fichero_de_prueba(&dir, "propuesta.lua");

        let veredicto = verificar(
            &fichero,
            &comprobadores(
                "lua",
                vec![
                    "no-existe-este-comprobador-jamas".into(),
                    "{fichero}".into(),
                ],
            ),
            Duration::from_secs(5),
            dir.path(),
        );

        assert!(
            matches!(veredicto, Veredicto::NoSePudo { .. }),
            "{veredicto:?}"
        );
        assert!(veredicto
            .para_el_modelo()
            .contains("no se ha podido verificar"));
    }

    #[test]
    fn un_comprobador_colgado_se_corta_por_el_tiempo_maximo() {
        // Un arnés pensado para vivir días no puede quedarse esperando a un comprobador que no
        // termina nunca.
        let dir = tempfile::tempdir().unwrap();
        let fichero = fichero_de_prueba(&dir, "propuesta.py");

        let inicio = Instant::now();
        let veredicto = verificar(
            &fichero,
            &comprobadores(
                "py",
                vec![
                    "cmd".into(),
                    "/C".into(),
                    "ping -n 30 127.0.0.1 > NUL".into(),
                ],
            ),
            Duration::from_millis(400),
            dir.path(),
        );
        let tardado = inicio.elapsed();

        assert!(
            matches!(veredicto, Veredicto::NoSePudo { .. }),
            "{veredicto:?}"
        );
        assert!(
            tardado < Duration::from_secs(10),
            "debería haber cortado enseguida y tardó {tardado:?}"
        );
    }

    #[test]
    fn el_marcador_del_fichero_se_sustituye_por_su_ruta() {
        let dir = tempfile::tempdir().unwrap();
        let fichero = fichero_de_prueba(&dir, "propuesta.py");

        // `type` falla si el fichero no existe: si la sustitución no ocurriera, buscaría uno
        // llamado literalmente «{fichero}» y esta prueba lo cazaría.
        let veredicto = verificar(
            &fichero,
            &comprobadores(
                "py",
                vec!["cmd".into(), "/C".into(), "type".into(), "{fichero}".into()],
            ),
            Duration::from_secs(10),
            dir.path(),
        );

        assert!(matches!(veredicto, Veredicto::Pasa { .. }), "{veredicto:?}");
    }

    #[test]
    fn una_ruta_relativa_llega_entera_al_comprobador() {
        // El comprobador se ejecuta en un directorio temporal, así que una ruta relativa se
        // resolvería contra ese directorio y el comprobador diría que el fichero no existe. Pasó
        // de verdad al escribir la prueba contra las entregas reales.
        let dir = tempfile::tempdir().unwrap();
        let fichero = fichero_de_prueba(&dir, "propuesta.py");
        let otro_directorio = tempfile::tempdir().unwrap();

        let relativa = pathdiff_simple(&fichero, &std::env::current_dir().unwrap())
            .unwrap_or_else(|| fichero.clone());

        let veredicto = verificar(
            &relativa,
            &comprobadores(
                "py",
                vec!["cmd".into(), "/C".into(), "type".into(), "{fichero}".into()],
            ),
            Duration::from_secs(10),
            otro_directorio.path(),
        );

        assert!(
            matches!(veredicto, Veredicto::Pasa { .. }),
            "el comprobador no encontró el fichero: {veredicto:?}"
        );
    }

    /// Ruta relativa de `objetivo` respecto a `base`, sin depender de ninguna crate. Devuelve
    /// `None` si no comparten raíz, y entonces la prueba usa la absoluta, que también vale.
    fn pathdiff_simple(objetivo: &Path, base: &Path) -> Option<std::path::PathBuf> {
        let objetivo = std::fs::canonicalize(objetivo).ok()?;
        let base = std::fs::canonicalize(base).ok()?;
        objetivo.strip_prefix(&base).ok().map(|p| p.to_path_buf())
    }

    #[test]
    fn una_salida_enorme_se_recorta_antes_de_darsela_al_modelo() {
        let largo = "e".repeat(MAXIMO_SALIDA * 2);
        let recortado = recortar(&largo, MAXIMO_SALIDA);

        assert!(recortado.chars().count() < largo.chars().count());
        assert!(recortado.contains("recortado"));
    }

    #[test]
    fn el_recorte_no_parte_un_caracter_por_la_mitad() {
        // Los errores de compilación traen acentos, y en UTF-8 ocupan más de un byte: recortar por
        // bytes dejaría el texto roto justo donde hay que leerlo.
        let texto = "ñ".repeat(MAXIMO_SALIDA * 2);
        let recortado = recortar(&texto, MAXIMO_SALIDA);

        assert!(recortado.starts_with('ñ'));
        assert_eq!(
            recortado.chars().filter(|c| *c == 'ñ').count(),
            MAXIMO_SALIDA
        );
    }
}
