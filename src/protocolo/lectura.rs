//! Registro de lectura incremental del canal.
//!
//! Es la pieza que hace viable todo el diseño: leer solo lo nuevo desde el último paso baja el coste
//! por ciclo de decenas de miles de tokens a unos cientos de bytes. La huella del prefijo detecta
//! que el dueño de un buzón lo ha podado, en cuyo caso el desplazamiento guardado ya no significa
//! nada y hay que releer entero.

use crate::error::{Error, Resultado};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// El error de un fichero de estado del arnés (el registro de lectura, el latido) que está en
/// disco pero no se deja leer como JSON. Un solo sitio lo redacta para los dos.
pub(crate) fn fichero_corrupto(ruta: &Path, causa: &serde_json::Error) -> Error {
    Error::Configuracion(mensaje_fichero_corrupto(ruta, causa))
}

/// Lo que hay de nuevo en un buzón desde la última vez.
#[derive(Debug)]
pub enum Delta {
    /// Nada ha cambiado.
    Nada,
    /// Solo esto se ha añadido al final.
    Incremento(String),
    /// El fichero cambió por debajo; hay que leerlo entero.
    Completo(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entrada {
    bytes_leidos: usize,
    huella_prefijo: String,
}

/// Cuánto se ha leído de cada buzón y con qué huella.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RegistroLectura {
    entradas: BTreeMap<String, Entrada>,
}

impl RegistroLectura {
    /// Carga el registro. Un fichero que no existe no es un error: es un primer arranque.
    pub fn cargar(ruta: &Path) -> Resultado<Self> {
        if !ruta.exists() {
            return Ok(Self::default());
        }
        let texto = std::fs::read_to_string(ruta).map_err(|causa| Error::Lectura {
            ruta: ruta.to_path_buf(),
            causa,
        })?;
        serde_json::from_str(&texto).map_err(|e| fichero_corrupto(ruta, &e))
    }

    /// Guarda el registro, creando el directorio si hace falta.
    pub fn guardar(&self, ruta: &Path) -> Resultado<()> {
        if let Some(padre) = ruta.parent() {
            std::fs::create_dir_all(padre).map_err(|causa| Error::Escritura {
                ruta: padre.to_path_buf(),
                causa,
            })?;
        }
        let texto = serde_json::to_string_pretty(self)
            .map_err(|e| Error::Configuracion(mensaje_registro_sin_serializar(&e)))?;
        std::fs::write(ruta, texto).map_err(|causa| Error::Escritura {
            ruta: ruta.to_path_buf(),
            causa,
        })
    }

    /// ¿Todavía no se ha leído nada?
    pub fn esta_vacio(&self) -> bool {
        self.entradas.is_empty()
    }

    /// Calcula qué hay de nuevo en un buzón y actualiza el registro.
    pub fn delta(&mut self, nombre: &str, contenido: &str) -> Delta {
        let bytes = contenido.as_bytes();
        let anterior = self.entradas.get(nombre).cloned();

        let resultado = match anterior {
            Some(entrada)
                if entrada.bytes_leidos <= bytes.len()
                    && huella(&bytes[..entrada.bytes_leidos]) == entrada.huella_prefijo =>
            {
                // El prefijo es el mismo que se leyó entero, así que su final cae entre dos
                // caracteres; `get` lo comprueba en vez de suponerlo.
                let nuevo = contenido.get(entrada.bytes_leidos..).unwrap_or_default();
                if nuevo.is_empty() {
                    Delta::Nada
                } else {
                    Delta::Incremento(nuevo.to_string())
                }
            }
            _ => Delta::Completo(contenido.to_string()),
        };

        self.entradas.insert(
            nombre.to_string(),
            Entrada {
                bytes_leidos: bytes.len(),
                huella_prefijo: huella(bytes),
            },
        );

        resultado
    }
}

/// La huella que se guarda en `lectura.json`: el algoritmo delante, para poder cambiarlo.
fn huella(bytes: &[u8]) -> String {
    format!("sha256:{}", crate::ficheros::huella_sha256_de_bytes(bytes))
}

/// Un fichero de estado del arnés que está en disco pero no se deja leer como JSON.
fn mensaje_fichero_corrupto(ruta: &Path, causa: &serde_json::Error) -> String {
    format!("«{}» está corrupto: {causa}", ruta.display())
}

/// El registro de lectura no se pudo pasar a JSON.
fn mensaje_registro_sin_serializar(causa: &serde_json::Error) -> String {
    format!("no se pudo serializar el registro: {causa}")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La ruta, como la escribe el sistema, no con `{:?}`.
    #[test]
    fn el_fichero_corrupto_nombra_la_ruta_tal_cual() {
        let ruta = Path::new("carpeta").join("latido.json");
        let causa = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let texto = fichero_corrupto(&ruta, &causa).to_string();
        assert!(
            texto.contains(&format!("«{}» está corrupto", ruta.display())),
            "{texto}"
        );
        assert!(!texto.contains('"'), "{texto}");
    }

    #[test]
    fn un_buzon_nunca_leido_devuelve_el_contenido_completo() {
        let mut registro = RegistroLectura::default();

        let delta = registro.delta("claude.md", "línea uno\nlínea dos\n");

        assert!(matches!(delta, Delta::Completo(ref t) if t.contains("línea uno")));
    }

    #[test]
    fn un_buzon_que_crece_devuelve_solo_lo_nuevo() {
        let mut registro = RegistroLectura::default();
        registro.delta("claude.md", "viejo\n");

        let delta = registro.delta("claude.md", "viejo\nnuevo\n");

        match delta {
            Delta::Incremento(t) => {
                assert_eq!(t, "nuevo\n", "solo debe devolver lo añadido");
            }
            otro => panic!("se esperaba un incremento y llegó {otro:?}"),
        }
    }

    #[test]
    fn un_buzon_sin_cambios_no_devuelve_nada() {
        let mut registro = RegistroLectura::default();
        registro.delta("claude.md", "igual\n");

        let delta = registro.delta("claude.md", "igual\n");

        assert!(matches!(delta, Delta::Nada));
    }

    #[test]
    fn un_buzon_podado_por_su_dueno_obliga_a_releerlo_entero() {
        let mut registro = RegistroLectura::default();
        registro.delta("claude.md", "bloque viejo largo\nbloque reciente\n");

        // El dueño podó: el prefijo ya no coincide con lo que habíamos leído.
        let delta = registro.delta("claude.md", "bloque reciente\n");

        assert!(
            matches!(delta, Delta::Completo(_)),
            "si el prefijo cambia, la lectura incremental ya no es válida"
        );
    }

    #[test]
    fn el_registro_sobrevive_a_un_viaje_de_ida_y_vuelta_por_disco() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("lectura.json");
        let mut registro = RegistroLectura::default();
        registro.delta("claude.md", "contenido\n");
        registro.guardar(&ruta).unwrap();
        // El formato en disco de la 1.0.0 guarda la huella con su prefijo.
        let en_disco = std::fs::read_to_string(&ruta).unwrap();
        assert!(en_disco.contains("\"sha256:"), "{en_disco}");

        let mut recuperado = RegistroLectura::cargar(&ruta).unwrap();

        assert!(matches!(
            recuperado.delta("claude.md", "contenido\n"),
            Delta::Nada
        ));
    }

    #[test]
    fn cargar_un_registro_inexistente_devuelve_uno_vacio_y_no_falla() {
        let dir = tempfile::tempdir().unwrap();

        let registro = RegistroLectura::cargar(&dir.path().join("no-existe.json")).unwrap();

        assert!(registro.esta_vacio());
    }
}
