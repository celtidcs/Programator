//! Pruebas de la biblioteca. Ninguna necesita red, registro de Windows ni variables de entorno.

use crate::{
    comprobar, decidir_aviso, interpretar_respuesta, ultima_version, AjustesDeVersionNueva,
    ConsultaDeVersion, ConsultaHttps, FalloDeComprobacion, OrigenDeLasVersiones,
    ProgramaQueConsulta, VersionSemantica,
};

/// Repositorio neutro de las pruebas: la biblioteca no conoce ningún repositorio real.
const REPOSITORIO_DE_PRUEBA: &str = "dueno/proyecto";

/// Respuesta mínima de releases/latest con la etiqueta indicada.
///
/// Lleva un html_url ajeno a propósito: el enlace del aviso no puede salir de la respuesta.
fn respuesta_de_github(etiqueta: &str) -> String {
    format!(
        r#"{{"tag_name":"{etiqueta}","html_url":"https://ejemplo-malicioso.test/x","body":"<script>"}}"#
    )
}

/// Origen de versiones con los valores por defecto y el repositorio de prueba.
fn origen_por_defecto() -> OrigenDeLasVersiones {
    OrigenDeLasVersiones::desde_ajustes(&AjustesDeVersionNueva::para_repositorio(
        REPOSITORIO_DE_PRUEBA,
    ))
    .expect("los ajustes por defecto tienen que ser válidos")
}

/// Consulta falsa que devuelve siempre lo que se le diga.
struct ConsultaFalsa {
    /// Lo que devuelve cada petición.
    respuesta: Result<String, FalloDeComprobacion>,
}

impl ConsultaDeVersion for ConsultaFalsa {
    /// Devuelve la respuesta preparada, sin mirar la dirección.
    fn pedir(&self, _url: &str) -> Result<String, FalloDeComprobacion> {
        self.respuesta.clone()
    }
}

/// Las versiones se comparan como números, no como texto.
#[test]
fn las_versiones_se_comparan_como_numeros_y_no_como_texto() {
    let v = |texto: &str| VersionSemantica::desde_etiqueta(texto);
    assert!(v("0.12.0") > v("0.9.9"), "0.12.0 es posterior a 0.9.9");
    assert!(v("v0.12.10") > v("0.12.9"), "0.12.10 es posterior a 0.12.9");
    assert!(v("v1.0.0") > v("0.99.99"));
    assert_eq!(
        v("v0.12.0"),
        v("0.12.0"),
        "la «v» delante no cambia la versión"
    );

    for invalida in [
        "",
        "v",
        "0.12",
        "0.12.0.1",
        "v1.0.0-rc1",
        "1.0.0+compilacion",
        " 0.12.0",
        "0.12.x",
        "-1.0.0",
        "99999999999999999999999.0.0",
        "V0.12.0",
    ] {
        assert_eq!(
            v(invalida),
            None,
            "«{invalida}» no es una versión que se pueda comparar"
        );
    }
}

/// Solo se avisa si la publicada es mayor que la local, y el enlace se construye desde los
/// ajustes, nunca desde la respuesta.
#[test]
fn solo_se_avisa_de_una_version_mayor_y_con_enlace_propio() {
    let origen = origen_por_defecto();

    let aviso = decidir_aviso("0.12.0", "v0.13.0", &origen)
        .expect("una etiqueta válida no es un fallo")
        .expect("0.13.0 es mayor que 0.12.0: tiene que avisar");
    assert_eq!(aviso.version, "0.13.0");
    assert_eq!(
        aviso.enlace,
        "https://github.com/dueno/proyecto/releases/tag/v0.13.0"
    );

    for (local, publicada) in [
        ("0.12.0", "v0.12.0"),
        ("0.12.0", "0.12.0"),
        ("0.12.0", "v0.9.0"),
    ] {
        assert_eq!(
            decidir_aviso(local, publicada, &origen).expect("etiqueta válida"),
            None,
            "con la local {local} y la publicada {publicada} no hay nada que avisar"
        );
    }

    assert!(
        decidir_aviso("0.12.0", "v1.0.0-rc1", &origen).is_err(),
        "una etiqueta que no es X.Y.Z es un fallo de la respuesta, no un aviso"
    );
}

/// La respuesta de GitHub es un dato no fiable: solo se toma tag_name.
#[test]
fn la_respuesta_de_github_solo_aporta_la_etiqueta() {
    assert_eq!(
        interpretar_respuesta(&respuesta_de_github("v0.13.0")).expect("respuesta válida"),
        "v0.13.0"
    );
    for ilegible in [
        "",
        "no es json",
        "[]",
        r#"{"name":"sin etiqueta"}"#,
        r#"{"tag_name":13}"#,
    ] {
        assert!(
            interpretar_respuesta(ilegible).is_err(),
            "«{ilegible}» no trae una etiqueta y tiene que ser un fallo"
        );
    }
}

/// Repositorio y direcciones se validan antes de usarlos: solo HTTPS, sin credenciales ni
/// puerto, rutas limpias y un dueño/nombre sin caracteres raros.
#[test]
fn el_origen_de_las_versiones_rechaza_configuraciones_peligrosas() {
    let origen = origen_por_defecto();
    assert_eq!(
        origen.url_de_consulta(),
        "https://api.github.com/repos/dueno/proyecto/releases/latest"
    );

    let con = |cambiar: fn(&mut AjustesDeVersionNueva)| {
        let mut ajustes = AjustesDeVersionNueva::para_repositorio(REPOSITORIO_DE_PRUEBA);
        cambiar(&mut ajustes);
        OrigenDeLasVersiones::desde_ajustes(&ajustes)
    };
    /// Un caso: qué se estropea y cómo.
    type AjusteMalo = (&'static str, fn(&mut AjustesDeVersionNueva));
    // Una ruta como https://api.github.com/x ya no es peligrosa: GitHub Enterprise Server
    // necesita https://servidor/api/v3. Se vigilan las rutas que cambian el sentido de la
    // dirección.
    let malos: [AjusteMalo; 23] = [
        ("API por http", |a| {
            a.url_api = "http://api.github.com".into()
        }),
        ("web por http", |a| a.url_web = "http://github.com".into()),
        ("API con credenciales", |a| {
            a.url_api = format!("https://u:p{}api.github.com", '@')
        }),
        ("API con ruta y credenciales", |a| {
            a.url_api = "https://u:p@servidor/api/v3".into()
        }),
        ("API con puerto y ruta", |a| {
            a.url_api = "https://servidor:8443/api/v3".into()
        }),
        ("API con retroceso", |a| {
            a.url_api = "https://servidor/../x".into()
        }),
        ("API con retroceso al final", |a| {
            a.url_api = "https://servidor/api/..".into()
        }),
        ("API con tramo punto", |a| {
            a.url_api = "https://servidor/./x".into()
        }),
        ("API con barra doble", |a| {
            a.url_api = "https://servidor//x".into()
        }),
        ("API con consulta", |a| {
            a.url_api = "https://servidor/x?y".into()
        }),
        ("API con fragmento", |a| {
            a.url_api = "https://servidor/x#y".into()
        }),
        ("API con retroceso escapado", |a| {
            a.url_api = "https://servidor/%2e%2e".into()
        }),
        ("API con barra invertida", |a| {
            a.url_api = "https://servidor/x\\y".into()
        }),
        ("API con espacio en la ruta", |a| {
            a.url_api = "https://servidor/a b".into()
        }),
        ("API con control en la ruta", |a| {
            a.url_api = "https://servidor/a\ny".into()
        }),
        ("web con retroceso", |a| {
            a.url_web = "https://servidor/../x".into()
        }),
        ("repositorio con ..", |a| {
            a.repositorio = "../proyecto".into()
        }),
        ("repositorio sin dueño", |a| {
            a.repositorio = "proyecto".into()
        }),
        ("repositorio con tres partes", |a| {
            a.repositorio = "a/b/c".into()
        }),
        ("repositorio con espacios", |a| {
            a.repositorio = "dueno/pro yecto".into()
        }),
        ("repositorio vacío", |a| a.repositorio = String::new()),
        ("espera nula", |a| a.segundos_de_espera = 0),
        ("límite nulo", |a| a.limite_de_respuesta_en_bytes = 0),
    ];
    for (caso, cambiar) in malos {
        assert!(con(cambiar).is_err(), "{caso}: tenía que rechazarse");
    }
}

/// El nombre del servidor se valida parte por parte: sin partes vacías, sin guiones en los
/// extremos y dentro de las longitudes del RFC 1035, 63 caracteres por parte y 253 en total.
#[test]
fn el_nombre_del_servidor_se_valida_parte_por_parte() {
    let con_servidor = |servidor: &str| {
        let mut ajustes = AjustesDeVersionNueva::para_repositorio(REPOSITORIO_DE_PRUEBA);
        ajustes.url_api = format!("https://{servidor}/api/v3");
        OrigenDeLasVersiones::desde_ajustes(&ajustes)
    };
    let parte_de = |longitud: usize| "a".repeat(longitud);
    // Cuatro partes de 63 más tres puntos son 255 caracteres; quitando dos, 253 justos.
    let de_253 = format!(
        "{}.{}.{}.{}",
        parte_de(63),
        parte_de(63),
        parte_de(63),
        parte_de(61)
    );
    let de_254 = format!("{de_253}a");

    for bueno in [
        "api.github.com".to_string(),
        "git.empresa.ejemplo".to_string(),
        "servidor".to_string(),
        "a-b.c".to_string(),
        "1.2.3.4".to_string(),
        parte_de(63),
        de_253,
    ] {
        assert!(
            con_servidor(&bueno).is_ok(),
            "«{bueno}» es un nombre de servidor válido"
        );
    }
    for malo in [
        String::new(),
        ".".to_string(),
        "a..b".to_string(),
        "-a.com".to_string(),
        "a-.com".to_string(),
        "a.-b.com".to_string(),
        ".a.com".to_string(),
        "a.com.".to_string(),
        "a_b.com".to_string(),
        parte_de(64),
        de_254,
    ] {
        assert!(
            con_servidor(&malo).is_err(),
            "«{malo}» no es un nombre de servidor válido"
        );
    }
}

/// GitHub Enterprise Server publica su API en el servidor seguido de la ruta api/v3, según la
/// guía de inicio rápido de su API REST. El origen la acepta,
/// quita las barras finales y compone las direcciones sobre esa ruta.
#[test]
fn el_origen_de_las_versiones_admite_la_api_de_github_enterprise_server() {
    let mut ajustes = AjustesDeVersionNueva::para_repositorio(REPOSITORIO_DE_PRUEBA);
    ajustes.url_api = "https://git.empresa.ejemplo/api/v3/".into();
    ajustes.url_web = "https://git.empresa.ejemplo/".into();
    let origen = OrigenDeLasVersiones::desde_ajustes(&ajustes)
        .expect("la API de GitHub Enterprise Server tiene que aceptarse");
    assert_eq!(
        origen.url_de_consulta(),
        "https://git.empresa.ejemplo/api/v3/repos/dueno/proyecto/releases/latest"
    );
    let aviso = decidir_aviso("0.13.1", "v0.14.0", &origen)
        .expect("etiquetas válidas")
        .expect("0.14.0 es mayor que 0.13.1: tiene que avisar");
    assert_eq!(
        aviso.enlace,
        "https://git.empresa.ejemplo/dueno/proyecto/releases/tag/v0.14.0"
    );
}

/// En Windows, ureq lee el proxy de las opciones de internet solo si se compila con su
/// característica win-system-proxy, como hace Proxy::try_from_env. La barrera exige que el
/// manifiesto la active en la sección de Windows y no en la general, para no cambiar nada en
/// Linux ni en macOS.
// Fuente: https://docs.rs/crate/ureq/3.4.2/source/src/proxy.rs
#[test]
fn ureq_lee_el_proxy_del_sistema_solo_en_windows() {
    const CARACTERISTICA: &str = "win-system-proxy";
    let manifiesto: toml::Value =
        toml::from_str(include_str!("../Cargo.toml")).expect("Cargo.toml tiene que ser TOML");
    let caracteristicas_de_ureq = |dependencias: Option<&toml::Value>| -> Vec<String> {
        dependencias
            .and_then(|d| d.get("ureq"))
            .and_then(|u| u.get("features"))
            .and_then(toml::Value::as_array)
            .map(|lista| {
                lista
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let generales = caracteristicas_de_ureq(manifiesto.get("dependencies"));
    let de_windows = caracteristicas_de_ureq(
        manifiesto
            .get("target")
            .and_then(|t| t.get("cfg(windows)"))
            .and_then(|w| w.get("dependencies")),
    );
    assert!(
        de_windows.iter().any(|c| c == CARACTERISTICA),
        "la sección [target.'cfg(windows)'.dependencies] tiene que activar {CARACTERISTICA} en ureq \
         (tiene {de_windows:?})"
    );
    assert!(
        !generales.iter().any(|c| c == CARACTERISTICA),
        "{CARACTERISTICA} no puede ir en las dependencias generales (tiene {generales:?})"
    );
}

/// La consulta real sale por el proxy que se le da, y sin proxy conecta directamente. Se
/// comprueba sobre la configuración del cliente, sin red, sin registro de Windows y sin tocar
/// variables de entorno, que las pruebas comparten.
#[test]
fn la_consulta_https_sale_por_el_proxy_que_se_le_da() {
    use crate::consulta::proxy_configurado;

    let origen = origen_por_defecto();
    let proxy = ureq::Proxy::new("http://proxy.ejemplo:3128").expect("dirección de proxy válida");
    let con_proxy = ConsultaHttps::nueva(&origen, Some(proxy));
    let configurado =
        proxy_configurado(&con_proxy).expect("el cliente tiene que llevar el proxy que se le dio");
    assert_eq!(
        (configurado.host(), configurado.port()),
        ("proxy.ejemplo", 3128)
    );
    assert!(
        proxy_configurado(&ConsultaHttps::nueva(&origen, None)).is_none(),
        "sin proxy, el cliente tiene que conectar directamente"
    );
}

/// La consulta HTTPS toma del origen validado el tiempo de espera y el límite de respuesta, y
/// su cabecera User-Agent nombra a la biblioteca y, si se identifica, antes al programa.
#[test]
fn la_consulta_https_se_prepara_desde_el_origen_validado() {
    use crate::consulta::configuracion_de;

    let mut ajustes = AjustesDeVersionNueva::para_repositorio(REPOSITORIO_DE_PRUEBA);
    ajustes.segundos_de_espera = 7;
    ajustes.limite_de_respuesta_en_bytes = 4096;
    let origen = OrigenDeLasVersiones::desde_ajustes(&ajustes).expect("ajustes válidos");
    let biblioteca = format!("aviso-version-github/{}", env!("CARGO_PKG_VERSION"));

    assert_eq!(
        configuracion_de(&ConsultaHttps::nueva(&origen, None)),
        (
            Some(biblioteca.clone()),
            Some(std::time::Duration::from_secs(7)),
            4096
        )
    );
    let programa = ProgramaQueConsulta::nuevo("mi-programa", "v1.2.0").expect("válido");
    let (identificada, _, _) =
        configuracion_de(&ConsultaHttps::para_programa(&origen, None, &programa));
    assert_eq!(
        identificada,
        Some(format!("mi-programa/1.2.0 {biblioteca}"))
    );
}

/// El programa solo puede presentarse con un nombre seguro para la cabecera User-Agent y una
/// versión X.Y.Z: nada de espacios, barras ni saltos de línea que colarían otra cabecera.
#[test]
fn el_programa_que_consulta_se_valida_antes_de_presentarse() {
    for (nombre, version) in [
        ("mi-programa", "1.2.0"),
        ("Programa_2.beta", "v0.1.0"),
        ("x", "0.0.1"),
    ] {
        assert!(
            ProgramaQueConsulta::nuevo(nombre, version).is_ok(),
            "«{nombre}» con «{version}» es una identificación válida"
        );
    }
    let largo = "a".repeat(65);
    for (nombre, version) in [
        ("", "1.0.0"),
        ("mi programa", "1.0.0"),
        ("mi/programa", "1.0.0"),
        ("mi-programa\r\nX-Otra: 1", "1.0.0"),
        ("mi-programa\n", "1.0.0"),
        ("-programa", "1.0.0"),
        (".programa", "1.0.0"),
        ("programa(1)", "1.0.0"),
        ("año", "1.0.0"),
        (largo.as_str(), "1.0.0"),
        ("mi-programa", "1.0"),
        ("mi-programa", "1.0.0 X"),
        ("mi-programa", "1.0.0\r\n"),
    ] {
        assert!(
            ProgramaQueConsulta::nuevo(nombre, version).is_err(),
            "«{nombre}» con «{version}» no es una identificación válida"
        );
    }
    assert!(
        ProgramaQueConsulta::nuevo(&"a".repeat(64), "1.0.0").is_ok(),
        "64 caracteres es el máximo y todavía vale"
    );
}

/// comprobar devuelve cada fallo como error, sin pánico.
///
/// Compilado con panic = "abort", un pánico en el hilo de trabajo no se quedaría en ese hilo:
/// cerraría el programa entero. Por eso cada fallo tiene que llegar como valor.
#[test]
fn comprobar_devuelve_cada_fallo_como_valor_sin_panico() {
    let origen = origen_por_defecto();
    let consulta = |respuesta| ConsultaFalsa { respuesta };
    let casos = [
        (Err(FalloDeComprobacion::Red("sin conexión".into())), "red"),
        (Ok("no es json".to_string()), "respuesta"),
        (Ok(respuesta_de_github("v1.0.0-rc1")), "etiqueta"),
    ];
    for (respuesta, caso) in casos {
        assert!(
            comprobar("0.12.0", &origen, &consulta(respuesta)).is_err(),
            "fallo de {caso}: tenía que devolverse como Err"
        );
    }
    assert!(
        comprobar(
            "no-es-version",
            &origen,
            &consulta(Ok(respuesta_de_github("v0.13.0")))
        )
        .is_err(),
        "una versión local ilegible también es un fallo, no un pánico"
    );
    assert_eq!(
        comprobar(
            "0.12.0",
            &origen,
            &consulta(Ok(respuesta_de_github("v0.13.0")))
        )
        .expect("una respuesta válida no es un fallo")
        .map(|aviso| aviso.version),
        Some("0.13.0".to_string())
    );
}

/// La raíz de la biblioteca prohíbe el código inseguro y, fuera de las pruebas, las llamadas
/// que pueden acabar en pánico. Así lo vigila el compilador también cuando la biblioteca se
/// compila sola, fuera del programa que la usa.
#[test]
fn la_raiz_prohibe_el_codigo_inseguro_y_los_panicos() {
    // Se descarta lo que va detrás de dos barras, para que un atributo comentado no cuente.
    let sin_espacios: String = include_str!("lib.rs")
        .lines()
        .map(|linea| linea.split("//").next().unwrap_or_default())
        .flat_map(str::chars)
        .filter(|c| !c.is_whitespace())
        .collect();
    for atributo in [
        "#![forbid(unsafe_code)]",
        "#![cfg_attr(not(test),deny(clippy::unwrap_used,clippy::expect_used,clippy::panic))]",
    ] {
        assert!(
            sin_espacios.contains(atributo),
            "src/lib.rs tiene que declarar {atributo}"
        );
    }
}

/// Consulta falsa que apunta cada dirección que se le pide y responde con una etiqueta fija.
struct ConsultaQueApunta {
    /// Direcciones pedidas, en orden.
    pedidas: std::sync::Mutex<Vec<String>>,
    /// Etiqueta de la respuesta.
    etiqueta: &'static str,
}

impl ConsultaDeVersion for ConsultaQueApunta {
    /// Apunta la dirección y devuelve una respuesta con la etiqueta preparada.
    fn pedir(&self, url: &str) -> Result<String, FalloDeComprobacion> {
        self.pedidas
            .lock()
            .expect("el registro de direcciones no puede estar envenenado")
            .push(url.to_string());
        Ok(respuesta_de_github(self.etiqueta))
    }
}

/// ultima_version pide la dirección de la última versión y devuelve la versión publicada sin
/// compararla con nada; cada fallo llega como valor.
#[test]
fn ultima_version_devuelve_la_publicada_sin_comparar() {
    let origen = origen_por_defecto();
    let consulta = ConsultaQueApunta {
        pedidas: std::sync::Mutex::new(Vec::new()),
        etiqueta: "v0.13.0",
    };
    assert_eq!(
        ultima_version(&origen, &consulta).expect("una respuesta válida no es un fallo"),
        VersionSemantica::desde_etiqueta("0.13.0").expect("versión válida")
    );
    assert_eq!(
        *consulta.pedidas.lock().expect("registro legible"),
        vec![origen.url_de_consulta()],
        "tiene que pedir una sola vez la dirección de la última versión"
    );

    let consulta = |respuesta| ConsultaFalsa { respuesta };
    assert!(matches!(
        ultima_version(
            &origen,
            &consulta(Err(FalloDeComprobacion::Red("sin conexión".into())))
        ),
        Err(FalloDeComprobacion::Red(_))
    ));
    assert!(matches!(
        ultima_version(&origen, &consulta(Ok("no es json".to_string()))),
        Err(FalloDeComprobacion::RespuestaIlegible(_))
    ));
    assert!(matches!(
        ultima_version(&origen, &consulta(Ok(respuesta_de_github("v1.0.0-rc1")))),
        Err(FalloDeComprobacion::EtiquetaNoValida(_))
    ));
}

/// Unos ajustes leídos de un archivo toman los valores por defecto en los campos que falten,
/// salvo el repositorio, que queda vacío y no supera la validación.
#[test]
fn los_ajustes_leidos_completan_lo_que_falta_salvo_el_repositorio() {
    let completos: AjustesDeVersionNueva =
        serde_json::from_str(r#"{"repositorio":"dueno/proyecto"}"#).expect("JSON válido");
    assert_eq!(
        completos,
        AjustesDeVersionNueva::para_repositorio(REPOSITORIO_DE_PRUEBA)
    );
    let sin_repositorio: AjustesDeVersionNueva = serde_json::from_str("{}").expect("JSON válido");
    assert!(OrigenDeLasVersiones::desde_ajustes(&sin_repositorio).is_err());
}

/// Palabras que no pueden aparecer en la documentación: delatan el proceso interno con que se
/// escribió o nombran a quien la escribió en vez de lo que hace la biblioteca. Se comparan en
/// minúsculas y como palabra completa, para no confundirlas con otras más largas que empiezan
/// igual.
const PALABRAS_DEL_PROCESO_INTERNO: [&str; 6] = [
    "agente",
    "agentes",
    "mutación",
    "mutaciones",
    "claude",
    "codex",
];

/// Qué marca de Markdown lleva un texto, si lleva alguna.
fn marca_markdown(texto: &str) -> Option<&'static str> {
    let inicio = texto.trim_start();
    let cifras = inicio.chars().take_while(char::is_ascii_digit).count();
    if inicio.starts_with('#') {
        Some("título con almohadilla")
    } else if ["- ", "* ", "+ "].iter().any(|v| inicio.starts_with(v)) {
        Some("viñeta")
    } else if cifras > 0 && inicio[cifras..].starts_with(". ") {
        Some("lista numerada")
    } else if inicio.starts_with('>') {
        Some("cita")
    } else if inicio.starts_with('|') {
        Some("tabla")
    } else if inicio.trim_end() == "---" {
        Some("separador")
    } else if texto.contains("**") {
        Some("negrita")
    } else if texto.contains('`') {
        Some("comillas invertidas")
    } else if texto.contains("](") {
        Some("enlace con corchetes")
    } else {
        None
    }
}

/// Qué rastro interno lleva un texto, si lleva alguno.
fn rastro_interno(texto: &str) -> Option<String> {
    texto
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .find(|palabra| PALABRAS_DEL_PROCESO_INTERNO.contains(palabra))
        .map(|palabra| format!("la palabra «{palabra}»"))
}

/// Las líneas de documentación de un archivo de código: las que empiezan por tres barras o
/// por dos barras y una exclamación, sin ese prefijo.
fn lineas_de_documentacion(codigo: &str) -> Vec<(usize, String)> {
    codigo
        .lines()
        .enumerate()
        .filter_map(|(indice, linea)| {
            let recortada = linea.trim_start();
            recortada
                .strip_prefix("///")
                .or_else(|| recortada.strip_prefix("//!"))
                .map(|resto| (indice + 1, resto.to_string()))
        })
        .collect()
}

/// Las quejas de un texto de documentación, con su origen y su línea.
fn quejas_de(origen: &str, lineas: Vec<(usize, String)>) -> Vec<String> {
    lineas
        .into_iter()
        .filter_map(|(numero, linea)| {
            marca_markdown(&linea)
                .map(str::to_string)
                .or_else(|| rastro_interno(&linea))
                .map(|motivo| format!("{origen}:{numero}: {motivo}"))
        })
        .collect()
}

/// Los detectores distinguen lo prohibido de la prosa normal, para que la barrera no pase en
/// verde por no saber reconocer nada.
#[test]
fn los_detectores_de_la_documentacion_distinguen_lo_prohibido() {
    for malo in [
        "# Título",
        "- viñeta",
        "1. paso",
        "> cita",
        "| a | b |",
        "texto **fuerte**",
        "usa `tal`",
        "[texto](ruta)",
    ] {
        assert!(marca_markdown(malo).is_some(), "«{malo}» lleva Markdown");
    }
    for malo in [
        "Implementado por Claude.",
        "Pasó dos mutaciones.",
        "Lo escribió un agente.",
    ] {
        assert!(
            rastro_interno(malo).is_some(),
            "«{malo}» delata el proceso interno"
        );
    }
    for bueno in [
        "La biblioteca consulta la API de GitHub.",
        "Se guarda en el directorio del programa.",
        "cargo test",
    ] {
        assert!(marca_markdown(bueno).is_none() && rastro_interno(bueno).is_none());
    }
}

/// La documentación de la biblioteca está escrita para personas, en prosa y sin rastros del
/// proceso interno: el README y todos los comentarios de documentación del código.
#[test]
fn la_documentacion_es_prosa_sin_markdown_ni_rastros_internos() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut quejas = Vec::new();

    let leer = |ruta: &std::path::Path| {
        std::fs::read_to_string(ruta)
            .unwrap_or_else(|error| panic!("no se puede leer {}: {error}", ruta.display()))
    };
    let readme = leer(&raiz.join("README.md"));
    quejas.extend(quejas_de(
        "README.md",
        readme
            .lines()
            .enumerate()
            .map(|(i, linea)| (i + 1, linea.to_string()))
            .collect(),
    ));

    let carpeta = raiz.join("src");
    let mut fuentes: Vec<_> = std::fs::read_dir(&carpeta)
        .expect("la carpeta src tiene que poder leerse")
        .map(|entrada| entrada.expect("entrada legible").path())
        .filter(|ruta| ruta.extension().is_some_and(|e| e == "rs"))
        .collect();
    fuentes.sort();
    assert!(fuentes.len() >= 5, "faltan archivos de código: {fuentes:?}");
    for fuente in fuentes {
        let nombre = format!(
            "src/{}",
            fuente
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        );
        quejas.extend(quejas_de(&nombre, lineas_de_documentacion(&leer(&fuente))));
    }

    assert!(
        quejas.is_empty(),
        "la documentación tiene que ser prosa sin Markdown ni rastros internos:\n  - {}",
        quejas.join("\n  - ")
    );
}

use std::path::{Path, PathBuf};

/// Archivos y carpetas de la biblioteca que se versionan y viajan con ella: los que revisa la
/// prueba de datos privados. Lo demás (lo compilado, el git, lo que nunca se versiona) no cuenta.
const LO_QUE_SE_VERSIONA: [&str; 9] = [
    "Cargo.toml",
    "Cargo.lock",
    "README.md",
    "mejoras.md",
    "LICENSE",
    ".gitignore",
    "src",
    "examples",
    "documentacion",
];

/// Único dominio de correo admitido: el que asigna GitHub para no publicar el correo real.
const DOMINIO_DE_CORREO_ADMITIDO: &str = "users.noreply.github.com";

/// Lista privada de fragmentos prohibidos, en la carpeta que contiene la biblioteca. Solo existe
/// junto al original: en las copias de los programas no está, y la prueba se queda con las formas
/// genéricas. Nunca se copia dentro de la biblioteca.
const LISTA_PRIVADA: &str = "fragmentos-privados.txt";

/// Todos los archivos de una carpeta, recorrida entera.
fn archivos_de(carpeta: &Path, encontrados: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(carpeta) else {
        return;
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta.is_dir() {
            archivos_de(&ruta, encontrados);
        } else {
            encontrados.push(ruta);
        }
    }
}

/// Si el texto contiene una ruta con letra de unidad: una letra suelta, dos puntos y una barra.
/// La letra no puede ir pegada a otra, para no confundirla con el esquema de una dirección web.
fn tiene_letra_de_unidad(texto: &str) -> bool {
    let letras: Vec<char> = texto.chars().collect();
    letras.windows(3).enumerate().any(|(i, trozo)| {
        let suelta = i == 0 || !letras[i - 1].is_alphanumeric();
        suelta
            && trozo[0].is_ascii_alphabetic()
            && trozo[1] == ':'
            && matches!(trozo[2], '/' | '\\')
    })
}

/// Si el texto contiene una carpeta personal o una dirección IP de una red privada. Las formas se
/// componen aquí para que este archivo no las contenga tal cual.
fn tiene_ruta_personal_o_red_privada(texto: &str) -> bool {
    let formas = [
        format!("{0}home{0}", '/'),
        format!("{0}users{0}", '/'),
        format!("{0}users{0}", '\\'),
        format!("{}.{}.", 192, 168),
    ];
    formas.iter().any(|forma| texto.contains(forma.as_str()))
}

/// Correos del texto cuyo dominio no es el admitido.
fn correos_no_admitidos(texto: &str) -> Vec<String> {
    let de_correo = |c: char| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '+');
    let mut correos = Vec::new();
    for (posicion, _) in texto.match_indices('@') {
        let antes: String = texto[..posicion]
            .chars()
            .rev()
            .take_while(|c| de_correo(*c))
            .collect();
        let despues: String = texto[posicion + 1..]
            .chars()
            .take_while(|c| de_correo(*c))
            .collect();
        let despues = despues.trim_end_matches('.');
        if !antes.is_empty() && despues.contains('.') && despues != DOMINIO_DE_CORREO_ADMITIDO {
            correos.push(format!(
                "{}@{despues}",
                antes.chars().rev().collect::<String>()
            ));
        }
    }
    correos
}

/// Si el fragmento aparece como tal: cuando empieza o termina en letra o cifra, no puede ir pegado
/// a otra, para que un fragmento no salte dentro de una palabra legítima más larga.
fn contiene_fragmento(texto: &str, fragmento: &str) -> bool {
    let pegado = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    texto.match_indices(fragmento).any(|(posicion, _)| {
        let antes = texto[..posicion].chars().next_back();
        let despues = texto[posicion + fragmento.len()..].chars().next();
        let primera = fragmento.chars().next();
        let ultima = fragmento.chars().next_back();
        !(pegado(primera) && pegado(antes)) && !(pegado(ultima) && pegado(despues))
    })
}

/// La biblioteca viaja con la copia a cada programa y puede publicarse con él: ningún archivo que
/// se versiona contiene rutas del equipo, carpetas personales, direcciones de redes privadas,
/// correos distintos del que asigna GitHub ni los fragmentos de la lista privada, si existe.
#[test]
fn la_biblioteca_no_contiene_datos_privados() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !raiz.join(LISTA_PRIVADA).exists(),
        "la lista privada no puede ir dentro de la biblioteca"
    );
    let privados: Vec<String> = raiz
        .parent()
        .and_then(|carpeta| std::fs::read_to_string(carpeta.join(LISTA_PRIVADA)).ok())
        .unwrap_or_default()
        .lines()
        .map(|linea| linea.trim().to_lowercase())
        .filter(|linea| !linea.is_empty() && !linea.starts_with('#'))
        .collect();
    let mut archivos = Vec::new();
    for nombre in LO_QUE_SE_VERSIONA {
        let ruta = raiz.join(nombre);
        if ruta.is_dir() {
            archivos_de(&ruta, &mut archivos);
        } else if ruta.is_file() {
            archivos.push(ruta);
        }
    }
    archivos.sort();
    assert!(archivos.len() >= 10, "faltan archivos: {archivos:?}");
    let mut quejas = Vec::new();
    for archivo in archivos {
        let Ok(texto) = std::fs::read_to_string(&archivo) else {
            continue;
        };
        for (indice, linea) in texto.lines().enumerate() {
            let minusculas = linea.to_lowercase();
            let mut motivos = Vec::new();
            if tiene_letra_de_unidad(&minusculas) {
                motivos.push("una ruta con letra de unidad".to_string());
            }
            if tiene_ruta_personal_o_red_privada(&minusculas) {
                motivos.push("una carpeta personal o una red privada".to_string());
            }
            for correo in correos_no_admitidos(&minusculas) {
                motivos.push(format!("el correo {correo}"));
            }
            for fragmento in privados
                .iter()
                .filter(|f| contiene_fragmento(&minusculas, f))
            {
                motivos.push(format!("un fragmento de la lista privada («{fragmento}»)"));
            }
            for motivo in motivos {
                quejas.push(format!("{}:{}: {motivo}", archivo.display(), indice + 1));
            }
        }
    }
    assert!(
        quejas.is_empty(),
        "la biblioteca no puede contener datos privados:\n  - {}",
        quejas.join("\n  - ")
    );
}

/// Las formas genéricas saltan con lo que tienen que saltar y no con lo legítimo.
#[test]
fn las_formas_genericas_de_datos_privados_distinguen_lo_legitimo() {
    let unidad = format!("{}:{}carpeta", 'm', '\\');
    assert!(tiene_letra_de_unidad(&format!("está en {unidad}")));
    assert!(!tiene_letra_de_unidad(
        "https://code.claude.com/docs/en/settings"
    ));
    assert!(tiene_ruta_personal_o_red_privada(&format!(
        "{0}home{0}x",
        '/'
    )));
    assert!(!tiene_ruta_personal_o_red_privada(
        "~/.claude/settings.json"
    ));
    let ajeno = format!("alguien{}ejemplo.com", '@');
    assert_eq!(correos_no_admitidos(&ajeno), vec![ajeno]);
    let admitido = format!("celtidcs{}{DOMINIO_DE_CORREO_ADMITIDO}", '@');
    assert!(correos_no_admitidos(&admitido).is_empty());
    assert!(contiene_fragmento("lo dijo el vecino.", "el vecino"));
    assert!(!contiene_fragmento("los vecinos", "vecino"));
}
