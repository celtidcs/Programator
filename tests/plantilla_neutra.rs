//! La plantilla de instrucciones no puede volver a hablar de un proyecto concreto.
//!
//! **Por qué existe esta prueba.** Entre el 14 y el 22 de septiembre de 2026, cuatro plantillas se
//! generalizaron y esta se quedó atrás. Nadie se enteró hasta que Programator arrancó en un
//! proyecto de Godot y .NET y le entregó al modelo siete menciones de `cargo`, cuatro de `egui` y
//! ninguna de los tres lenguajes que ese proyecto usa.

/// El tope de tamaño. La versión que un agente reescribió a mano para un proyecto real cumplía la misma
/// función en 6.688 bytes; 8.000 deja margen sin permitir que vuelva a crecer sin control.
const TOPE_DE_BYTES: usize = 8_000;

/// Lo que no puede aparecer, y por qué. Cada una es un préstamo de un proyecto concreto.
const PROHIBIDAS: &[(&str, &str)] = &[
    (
        "cargo",
        "es el gestor de Rust, y el proyecto anfitrión puede no usar Rust",
    ),
    ("egui", "es la biblioteca de interfaz de otro proyecto"),
    (
        "documentacion/",
        "es la estructura documental de otro proyecto, no la de todos",
    ),
    ("Codex", "es un agente de otro equipo"),
    ("audit_checks", "es un fichero de otro repositorio"),
];

/// Lo que no puede aparecer y tampoco se puede escribir aquí, porque es el nombre de un producto
/// privado: va su huella SHA-256 en minúsculas, y se compara con la de cada palabra.
const PROHIBIDAS_POR_HUELLA: &[(&str, &str)] = &[(
    "572492d015c725a6402f7c450319e5d050e6f8cefa72034873ff5e1030ed3ee4",
    "es el nombre de otro producto",
)];

fn huella(palabra: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(palabra.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// La huella de la primera palabra de `texto` que la tenga entre `huellas`.
fn palabra_con_huella<'a>(
    texto: &str,
    huellas: &[(&'a str, &'a str)],
) -> Option<(&'a str, &'a str)> {
    texto
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|palabra| !palabra.is_empty())
        .find_map(|palabra| {
            let suya = huella(palabra);
            huellas.iter().find(|(h, _)| *h == suya).copied()
        })
}

fn plantilla() -> String {
    std::fs::read_to_string("plantillas/instrucciones-del-proyecto.md")
        .expect("la plantilla de instrucciones tiene que estar en «plantillas/»")
}

/// ¿Aparece `palabra` como palabra entera, y no dentro de otra?
///
/// **Comparar subcadenas no vale aquí, y costó una prueba en rojo al escribirla:** «encargo»
/// contiene «cargo», y «encargo» es la palabra central de este dominio. Se exige que los caracteres
/// de alrededor no sean alfanuméricos, así que «cargo test» se caza y «un encargo» no.
fn contiene_palabra(texto: &str, palabra: &str) -> bool {
    let mut desde = 0;
    while let Some(posicion) = texto[desde..].find(palabra) {
        let inicio = desde + posicion;
        let fin = inicio + palabra.len();
        let antes_es_letra = texto[..inicio]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric);
        let despues_es_letra = texto[fin..]
            .chars()
            .next()
            .is_some_and(char::is_alphanumeric);
        if !antes_es_letra && !despues_es_letra {
            return true;
        }
        desde = fin;
    }
    false
}

#[test]
fn la_plantilla_no_nombra_ningun_proyecto_concreto() {
    let texto = plantilla().to_lowercase();
    for (prohibida, motivo) in PROHIBIDAS {
        assert!(
            !contiene_palabra(&texto, &prohibida.to_lowercase()),
            "la plantilla dice «{prohibida}», y {motivo}"
        );
    }
    if let Some((suya, motivo)) = palabra_con_huella(&texto, PROHIBIDAS_POR_HUELLA) {
        panic!("la plantilla dice la palabra de huella {suya}, y {motivo}");
    }
}

#[test]
fn la_barrera_por_huella_caza_la_palabra_entera_y_no_un_trozo() {
    let prueba = [(
        "3928d2fb50c11791fed429ae38189f8bb36c8fdcefc57328d04b1381c39de380",
        "ejemplo",
    )];
    assert_eq!(prueba[0].0, huella("encargo"));
    assert!(palabra_con_huella("Atiende un ENCARGO.", &prueba).is_some());
    assert!(palabra_con_huella("atiende los encargos", &prueba).is_none());
}

#[test]
fn la_barrera_distingue_una_palabra_de_un_trozo_de_otra() {
    // La prueba de la prueba. Sin esto, «encargo» dispararía la barrera de «cargo» y alguien
    // acabaría debilitando la lista de prohibidas para que dejara de molestar.
    assert!(contiene_palabra("ejecuta cargo test", "cargo"));
    assert!(!contiene_palabra("atiende un encargo", "cargo"));
    assert!(contiene_palabra("cargo", "cargo"));
    assert!(!contiene_palabra("encargos nuevos", "cargo"));
}

#[test]
fn la_plantilla_cabe_en_el_tope() {
    let bytes = plantilla().len();
    assert!(
        bytes <= TOPE_DE_BYTES,
        "la plantilla ocupa {bytes} bytes y el tope son {TOPE_DE_BYTES}"
    );
}

#[test]
fn la_plantilla_trae_el_marcador_y_el_hueco() {
    let texto = plantilla();
    assert!(
        texto.contains("{comprobadores}"),
        "sin el marcador, el arnés no puede escribir qué comprueba"
    );
    assert!(
        texto.contains("PENDIENTE DE RELLENAR"),
        "sin el hueco, el arnés estaría inventando las normas del proyecto"
    );
}
