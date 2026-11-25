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
//! **Quién elige el comprobador:** quien usa Programator, en `[verificacion]` de su TOML. Ni el
//! arnés incrusta comandos ni el modelo elige ninguno: el modelo solo pone el nombre del fichero,
//! y ese ya está confinado.

pub mod python;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use crate::orden_acotada::{ejecutar_con_tope_y_temporales_dentro, DesenlaceDeOrden};

/// Marcador que se sustituye por la ruta de la propuesta en los argumentos del comprobador.
const MARCADOR_FICHERO: &str = "{fichero}";

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
            // No dice «sin verificar»: en uso real esa frase se leyó como «verificada sin
            // hallazgos» durante cuatro entregas seguidas, y en el único lenguaje de aquel
            // proyecto. Lo que falta es un comprobador, y quien lea el canal tiene que saberlo.
            Veredicto::SinComprobador { extension } => format!(
                "`{nombre}`: ⚠️ SIN COMPROBAR — no hay comprobador configurado para «.{extension}»"
            ),
            Veredicto::NoSePudo { .. } => format!("`{nombre}`: no se pudo verificar"),
            Veredicto::Pasa { .. } => format!("`{nombre}`: ✅ pasa la comprobación"),
            // Con el motivo: en una prueba de uso real la línea solo decía que no
            // pasaba, y quien encargó tuvo que recompilar para saber por qué.
            Veredicto::NoPasa { salida, .. } => format!(
                "`{nombre}`: ❌ NO pasa la comprobación — {}",
                motivo_en_una_linea(salida)
            ),
        }
    }
}

/// Lo más largo que se copia al canal del motivo de un rechazo o de una denegación. Es una línea
/// para personas, no el texto entero: ese ya lo recibe el modelo.
pub(crate) const MAXIMO_EN_UNA_LINEA_DEL_CANAL: usize = 160;

/// `linea` tal como va en una línea del canal: recortada a [`MAXIMO_EN_UNA_LINEA_DEL_CANAL`]
/// caracteres y sin saltos. La usan el motivo de un rechazo y el de una denegación.
pub(crate) fn en_una_linea_del_canal(linea: &str) -> String {
    recortar(linea, MAXIMO_EN_UNA_LINEA_DEL_CANAL).replace('\n', " ")
}

/// Cómo termina la palabra con que empieza la línea del error en los comprobadores habituales:
/// `rustc` escribe «error[E0308]: …», `cargo` antepone líneas «Compiling …» y Python termina con
/// «SyntaxError: …» tras varias de contexto.
const SENAL_DE_ERROR: &str = "error";

/// Lo que cierra esa primera palabra: «error[E0308]» o «SyntaxError:».
const TRAS_LA_SENAL: [char; 2] = ['[', ':'];

/// La línea de la salida que mejor dice por qué se rechazó: la primera que empieza por una
/// palabra terminada en «error» seguida de «[» o «:», o si ninguna, la primera que no está vacía.
/// Recortada, para que no se coma el canal. Buscar «error» en cualquier sitio de la línea
/// confundía un `File "error_handler.py"` de Python con el error.
fn motivo_en_una_linea(salida: &str) -> String {
    let lineas = || salida.lines().map(str::trim).filter(|l| !l.is_empty());
    let linea = lineas()
        .find(|l| empieza_por_un_error(l))
        .or_else(|| lineas().next())
        .unwrap_or_default();
    en_una_linea_del_canal(linea)
}

/// Si `linea` empieza por una palabra terminada en «error» (en cualquier mayúscula) seguida de
/// «[» o «:».
fn empieza_por_un_error(linea: &str) -> bool {
    linea
        .split_once(TRAS_LA_SENAL)
        .map(|(palabra, _)| palabra)
        .filter(|palabra| !palabra.contains(char::is_whitespace))
        .is_some_and(|palabra| palabra.to_lowercase().ends_with(SENAL_DE_ERROR))
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
    maximo_salida: usize,
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

    // La ruta se pasa absoluta siempre. El comprobador se ejecuta en su propia carpeta de trabajo,
    // así que una ruta relativa se resolvería contra ese directorio y el comprobador diría que el
    // fichero no existe. En producción llega absoluta desde `Ambito::resolver`, pero depender de
    // eso sería una trampa esperando a quien llame de otro sitio.
    let ruta = crate::plataforma::sin_prefijo_extendido(
        &std::fs::canonicalize(fichero).unwrap_or_else(|_| fichero.to_path_buf()),
    )
    .display()
    .to_string();

    let argumentos: Vec<String> = resto
        .iter()
        .map(|a| a.replace(MARCADOR_FICHERO, &ruta))
        .collect();
    let descripcion = format!("{programa} {}", resto.join(" "));

    // Sin shell: el programa y sus argumentos van por separado, así que nada de lo que venga en un
    // nombre de fichero puede convertirse en otra orden.
    let (exito, estado, salida, errores) = match ejecutar_con_tope_y_temporales_dentro(
        programa,
        &argumentos,
        directorio_de_trabajo,
        tiempo_maximo,
    ) {
        DesenlaceDeOrden::Terminada {
            exito,
            estado,
            salida,
            errores,
            ..
        } => (exito, estado, salida, errores),
        DesenlaceDeOrden::NoSeLanzo(e) => {
            return Veredicto::NoSePudo {
                motivo: format!("no se pudo lanzar «{programa}»: {e}"),
            }
        }
        DesenlaceDeOrden::Agotada => {
            return Veredicto::NoSePudo {
                motivo: format!(
                    "«{programa}» seguía trabajando tras {} segundos y se ha cortado",
                    tiempo_maximo.as_secs()
                ),
            }
        }
        DesenlaceDeOrden::NoSePudoEsperar(e) => {
            return Veredicto::NoSePudo {
                motivo: format!("no se pudo esperar a «{programa}»: {e}"),
            }
        }
        DesenlaceDeOrden::SalidaIlegible(e) => {
            return Veredicto::NoSePudo {
                motivo: format!("no se pudo leer la salida de «{programa}»: {e}"),
            }
        }
    };

    if exito {
        return Veredicto::Pasa { orden: descripcion };
    }

    Veredicto::NoPasa {
        orden: descripcion,
        salida: recortar(&juntar_salidas(&errores, &salida, &estado), maximo_salida),
    }
}

/// Lo que dijo un comprobador que rechazó. Los comprobadores escriben sus errores por stderr, pero
/// no todos: `cargo test` escribe allí «Compiling…» y el fallo por stdout. Se juntan las dos, la de
/// errores primero, para no perder el motivo del rechazo por una cuestión de canal: con algo en
/// stderr, no se descarta stdout.
fn juntar_salidas(errores: &str, salida: &str, estado: &impl std::fmt::Display) -> String {
    let partes: Vec<&str> = [errores.trim(), salida.trim()]
        .into_iter()
        .filter(|parte| !parte.is_empty())
        .collect();
    if partes.is_empty() {
        format!("terminó con {estado} y sin decir nada")
    } else {
        partes.join("\n")
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
    use std::time::Instant;

    /// El tope de salida de fábrica, el que usan las pruebas.
    const MAXIMO_SALIDA: usize = crate::config::MAXIMO_SALIDA_AL_MODELO_POR_DEFECTO;

    #[test]
    fn sin_comprobador_el_canal_lo_dice_y_nombra_la_extension() {
        // «sin verificar» se leyó en uso real como «verificada sin hallazgos», durante cuatro
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

    fn no_pasa(salida: &str) -> Veredicto {
        Veredicto::NoPasa {
            orden: "rustc".to_string(),
            salida: salida.to_string(),
        }
    }

    #[test]
    fn un_rechazo_dice_en_el_canal_la_linea_del_error() {
        // Lo que dijo rustc de verdad en una prueba de uso real, con el aviso que a veces lo precede.
        let salida = "warning: unused variable\nerror[E0308]: mismatched types\n --> roto.rs:1:21";
        let linea = no_pasa(salida).para_el_canal("roto.rs");

        assert_eq!(
            linea,
            "`roto.rs`: ❌ NO pasa la comprobación — error[E0308]: mismatched types"
        );
    }

    #[test]
    fn de_python_coge_la_linea_del_error_y_no_la_del_fichero() {
        let salida =
            "  File \"x.py\", line 1\n    print(\n         ^\nSyntaxError: '(' was never closed";
        let linea = no_pasa(salida).para_el_canal("x.py");

        assert!(
            linea.ends_with("— SyntaxError: '(' was never closed"),
            "{linea}"
        );
    }

    #[test]
    fn un_fichero_con_error_en_el_nombre_no_es_la_linea_del_error() {
        let salida =
            "  File \"error_handler.py\", line 1\n    def f(:\n          ^\nSyntaxError: invalid syntax";
        let linea = no_pasa(salida).para_el_canal("error_handler.py");

        assert!(linea.ends_with("— SyntaxError: invalid syntax"), "{linea}");
    }

    #[test]
    fn sin_linea_de_error_se_dice_la_primera_que_haya() {
        let linea = no_pasa("\n  algo salió mal\notra cosa").para_el_canal("a.lua");

        assert!(linea.ends_with("— algo salió mal"), "{linea}");
    }

    #[test]
    fn el_motivo_del_canal_es_una_sola_linea_corta() {
        let larga = format!("error: {}", "x".repeat(MAXIMO_EN_UNA_LINEA_DEL_CANAL * 2));
        let linea = no_pasa(&larga).para_el_canal("a.rs");

        assert!(!linea.contains('\n'), "{linea}");
        assert!(linea.contains("[…recortado]"), "{linea}");
        assert!(
            linea.chars().count() < MAXIMO_EN_UNA_LINEA_DEL_CANAL + 80,
            "{linea}"
        );
    }

    /// Una orden que existe en cualquier Windows y termina bien o mal según convenga.
    fn orden_que(pasa: bool) -> Vec<String> {
        let codigo = if pasa { 0 } else { 1 };
        crate::plataforma::ordenes_de_prueba::eco_y_codigo("lo que dijo el comprobador", codigo)
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
            MAXIMO_SALIDA,
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
            MAXIMO_SALIDA,
        );

        assert!(matches!(veredicto, Veredicto::Pasa { .. }), "{veredicto:?}");
    }

    #[test]
    fn un_tope_menor_que_el_de_fabrica_tambien_recorta() {
        let dir = tempfile::tempdir().unwrap();
        let fichero = fichero_de_prueba(&dir, "propuesta.py");

        let veredicto = verificar(
            &fichero,
            &comprobadores("py", orden_que(false)),
            Duration::from_secs(10),
            dir.path(),
            10,
        );

        let Veredicto::NoPasa { salida, .. } = veredicto else {
            panic!("{veredicto:?}");
        };
        assert_eq!(salida, "lo que dij\n[…recortado]");
    }

    #[test]
    fn el_motivo_del_rechazo_junta_las_dos_salidas() {
        assert_eq!(
            juntar_salidas("Compiling x\n", "test falla en la línea 3\n", &1),
            "Compiling x\ntest falla en la línea 3"
        );
        assert_eq!(juntar_salidas("", " solo stdout ", &1), "solo stdout");
        assert_eq!(juntar_salidas("solo stderr", "\n", &1), "solo stderr");
        assert_eq!(
            juntar_salidas(" ", "", &2),
            "terminó con 2 y sin decir nada"
        );
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
            MAXIMO_SALIDA,
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
            MAXIMO_SALIDA,
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
            MAXIMO_SALIDA,
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
            &comprobadores("py", crate::plataforma::ordenes_de_prueba::tardar_mucho()),
            Duration::from_millis(400),
            dir.path(),
            MAXIMO_SALIDA,
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

        // La orden falla si el fichero no existe: si la sustitución no ocurriera, buscaría uno
        // llamado literalmente «{fichero}» y esta prueba lo cazaría.
        let veredicto = verificar(
            &fichero,
            &comprobadores("py", crate::plataforma::ordenes_de_prueba::volcar_fichero()),
            Duration::from_secs(10),
            dir.path(),
            MAXIMO_SALIDA,
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

        let relativa = ruta_relativa_simple(&fichero, &std::env::current_dir().unwrap())
            .unwrap_or_else(|| fichero.clone());

        let veredicto = verificar(
            &relativa,
            &comprobadores("py", crate::plataforma::ordenes_de_prueba::volcar_fichero()),
            Duration::from_secs(10),
            otro_directorio.path(),
            MAXIMO_SALIDA,
        );

        assert!(
            matches!(veredicto, Veredicto::Pasa { .. }),
            "el comprobador no encontró el fichero: {veredicto:?}"
        );
    }

    /// Ruta relativa de `objetivo` respecto a `base`, sin depender de ninguna crate. Devuelve
    /// `None` si no comparten raíz, y entonces la prueba usa la absoluta, que también vale.
    fn ruta_relativa_simple(objetivo: &Path, base: &Path) -> Option<std::path::PathBuf> {
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
