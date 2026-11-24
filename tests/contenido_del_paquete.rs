//! La lista del paquete portable (`empaquetado/contenido.txt`) solo nombra lo que existe, y no se
//! deja ninguna plantilla.
//!
//! El guion `empaquetado/empaquetar.ps1` falla si falta un origen, pero solo corre al publicar:
//! esta prueba lo adelanta a cada `cargo test`.

use std::path::{Path, PathBuf};

/// Una ruta del repositorio, anclada en su raíz y no en la carpeta desde la que se lanza la prueba.
fn del_repositorio(ruta: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(ruta)
}

const LISTA: &str = "empaquetado/contenido.txt";
/// Lo que no está en el repositorio porque lo trae `empaquetado/traer-vigilante.ps1` del release de
/// su proyecto. El empaquetador falla igual si falta.
const TRAIDO_DE_FUERA: &str = "terceros/";
/// La plantilla que va en la raíz del paquete como `LEEME.md` y no dentro de `plantillas/`.
const LEEME_PORTABLE: &str = "plantillas/LEEME-portable.md";

/// Una entrada de la lista: su marca de sistema, si la tiene, el origen y el destino.
struct Entrada {
    marca: Option<String>,
    origen: String,
    destino: String,
}

fn entradas() -> Vec<Entrada> {
    let texto =
        std::fs::read_to_string(del_repositorio(LISTA)).expect("la lista está en el repositorio");
    texto
        .lines()
        .map(str::trim)
        .filter(|linea| !linea.is_empty() && !linea.starts_with('#'))
        .map(|linea| {
            let (marca, resto) = match linea.strip_prefix('[') {
                Some(con_marca) => {
                    let (marca, resto) = con_marca.split_once(']').expect("marca cerrada");
                    (Some(marca.to_string()), resto.trim())
                }
                None => (None, linea),
            };
            let (origen, destino) = resto
                .split_once(" -> ")
                .unwrap_or_else(|| panic!("entrada sin «origen -> destino»: {linea}"));
            Entrada {
                marca,
                origen: origen.trim().to_string(),
                destino: destino.trim().to_string(),
            }
        })
        .collect()
}

#[test]
fn cada_origen_de_la_lista_existe() {
    for entrada in entradas()
        .into_iter()
        .filter(|entrada| !entrada.origen.starts_with(TRAIDO_DE_FUERA))
    {
        let ruta = del_repositorio(&entrada.origen);
        if entrada.origen.ends_with('/') {
            assert!(ruta.is_dir(), "falta la carpeta {}", entrada.origen);
        } else {
            assert!(ruta.is_file(), "falta el fichero {}", entrada.origen);
        }
    }
}

#[test]
fn el_vigilante_va_en_los_dos_sistemas_con_su_licencia_y_su_leeme() {
    let destinos: Vec<(Option<String>, String)> = entradas()
        .into_iter()
        .filter(|entrada| entrada.origen.starts_with(TRAIDO_DE_FUERA))
        .map(|entrada| (entrada.marca, entrada.destino))
        .collect();
    let esta = |marca: Option<&str>, destino: &str| {
        destinos
            .iter()
            .any(|(m, d)| m.as_deref() == marca && d == destino)
    };

    assert!(esta(Some("windows"), "vigilante/gestor-vigilante.exe"));
    assert!(esta(Some("linux"), "vigilante/gestor-vigilante"));
    assert!(esta(None, "vigilante/Licencia.txt"));
    assert!(esta(None, "vigilante/LEEME.txt"));
    let version =
        std::fs::read_to_string(del_repositorio("empaquetado/version-del-vigilante.txt")).unwrap();
    // MAYOR.MENOR.PARCHE, tres números: con solo contar los puntos, «a.b.c» pasaría.
    let partes: Vec<&str> = version.trim().split('.').collect();
    assert!(
        partes.len() == 3 && partes.iter().all(|p| p.parse::<u64>().is_ok()),
        "versión del vigilante: {version}"
    );
}

#[test]
fn las_marcas_son_de_un_sistema_que_existe() {
    for entrada in entradas() {
        if let Some(marca) = &entrada.marca {
            assert!(
                marca == "windows" || marca == "linux",
                "marca desconocida [{marca}] en {}",
                entrada.origen
            );
        }
    }
}

#[test]
fn ninguna_plantilla_se_queda_fuera_del_paquete() {
    let listadas: Vec<String> = entradas().into_iter().map(|e| e.origen).collect();
    for fichero in std::fs::read_dir(del_repositorio("plantillas")).expect("hay plantillas") {
        let ruta = format!(
            "plantillas/{}",
            fichero.unwrap().file_name().to_string_lossy()
        );
        assert!(
            listadas.contains(&ruta),
            "{ruta} no está en {LISTA}: añádela o di por qué no va"
        );
    }
}

#[test]
fn el_leeme_del_portable_va_en_la_raiz_y_no_en_plantillas() {
    let destinos: Vec<(String, String)> = entradas()
        .into_iter()
        .map(|e| (e.origen, e.destino))
        .collect();
    assert!(destinos.contains(&(LEEME_PORTABLE.to_string(), "LEEME.md".to_string())));
    assert_eq!(
        destinos.iter().filter(|(o, _)| o == LEEME_PORTABLE).count(),
        1
    );
}

#[test]
fn cada_sistema_lleva_su_guion_de_preparacion() {
    let entradas = entradas();
    let lleva = |marca: &str, origen: &str| {
        entradas
            .iter()
            .any(|e| e.marca.as_deref() == Some(marca) && e.origen == origen)
    };
    assert!(lleva("windows", "preparar-portable.ps1"));
    assert!(lleva("linux", "preparar-portable.sh"));
    assert!(lleva("windows", "recursos/programator.ico"));
}
