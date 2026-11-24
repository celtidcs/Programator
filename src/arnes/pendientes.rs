//! La cola de encargos pendientes: los que llegaron y todavía no tienen respuesta publicada.
//!
//! **Por qué existe.** Antes que olvidar un encargo, se deja pendiente para recuperarlo y hacer
//! la tarea encomendada. Un encargo que no se pudo atender (el modelo pedido no cargó a tiempo,
//! falló la publicación) entra aquí; cada pasada atiende primero lo que hay, en orden de llegada,
//! y un encargo solo sale cuando su respuesta está publicada. La cola vive en disco, en el espacio
//! del agente, y sobrevive a cerrar Programator.
//!
//! La lógica (qué entra, qué sale, cuándo se avisa) es pura, sobre [`Cola`]; la lectura y el
//! guardado van aparte, en [`leer`] y [`guardar`].

use crate::error::{Error, Resultado};
use crate::ficheros;
use crate::protocolo::encargo::{Encargo, Forma};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Lo que se añade al nombre de la cola para escribirla antes de renombrarla: si el programa se
/// corta a mitad, queda a medias el temporal y no la cola.
const SUFIJO_DEL_TEMPORAL: &str = ".escribiendo";

/// Un encargo en la cola, con lo necesario para atenderlo después y para avisar si se atasca.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pendiente {
    /// Fichero del buzón de origen.
    de: String,
    /// Texto íntegro del encargo.
    texto: String,
    /// Cómo venía escrito.
    forma: Forma,
    /// Cuándo entró en la cola.
    llego_el: String,
    /// Intentos fallidos hasta ahora.
    intentos: u32,
    /// Por qué falló el último intento.
    motivo: String,
    /// Si ya llegó a los intentos que se admiten. Se conserva para leer las colas escritas antes
    /// del cierre por intentos; ya no decide nada.
    #[serde(default)]
    avisado: bool,
}

impl Pendiente {
    /// Intentos fallidos hasta ahora.
    pub fn intentos(&self) -> u32 {
        self.intentos
    }

    /// Por qué falló el último intento.
    pub fn motivo(&self) -> &str {
        &self.motivo
    }

    /// Texto íntegro del encargo.
    pub fn texto(&self) -> &str {
        &self.texto
    }

    /// El encargo, tal como llegó.
    pub fn encargo(&self) -> Encargo {
        Encargo {
            de: self.de.clone(),
            texto: self.texto.clone(),
            forma: self.forma,
        }
    }

    fn es(&self, encargo: &Encargo) -> bool {
        self.de == encargo.de && self.texto == encargo.texto
    }
}

/// La cola entera, en orden de llegada.
///
/// Los campos son privados: la cola solo cambia por `anotar_fallo` y
/// `quitar`, que mantienen el orden de llegada y un solo pendiente por encargo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cola {
    #[serde(default)]
    pendiente: Vec<Pendiente>,
}

impl Cola {
    /// Los pendientes, en orden de llegada, para leerlos.
    pub fn pendientes(&self) -> &[Pendiente] {
        &self.pendiente
    }

    pub fn esta_vacia(&self) -> bool {
        self.pendiente.is_empty()
    }

    /// Los encargos pendientes, en orden de llegada.
    pub fn encargos(&self) -> Vec<Encargo> {
        self.pendiente.iter().map(Pendiente::encargo).collect()
    }

    /// Si el encargo ya está en la cola.
    pub fn contiene(&self, encargo: &Encargo) -> bool {
        self.pendiente.iter().any(|p| p.es(encargo))
    }

    /// Anota un intento fallido. Si el encargo no estaba, entra al final con un intento; si
    /// estaba, conserva su sitio y suma uno. Devuelve el pendiente si ya llegó a `avisar_tras`
    /// intentos: toca cerrarlo. Si el cierre no se pudo publicar, la vuelta siguiente lo vuelve a
    /// devolver.
    pub fn anotar_fallo(
        &mut self,
        encargo: &Encargo,
        fecha: &str,
        motivo: &str,
        avisar_tras: u32,
    ) -> Option<&Pendiente> {
        let posicion = match self.pendiente.iter().position(|p| p.es(encargo)) {
            Some(posicion) => posicion,
            None => {
                self.pendiente.push(Pendiente {
                    de: encargo.de.clone(),
                    texto: encargo.texto.clone(),
                    forma: encargo.forma,
                    llego_el: fecha.to_string(),
                    intentos: 0,
                    motivo: String::new(),
                    avisado: false,
                });
                self.pendiente.len() - 1
            }
        };
        let pendiente = &mut self.pendiente[posicion];
        pendiente.intentos = pendiente.intentos.saturating_add(1);
        pendiente.motivo = motivo.to_string();
        if pendiente.intentos < avisar_tras {
            return None;
        }
        pendiente.avisado = true;
        Some(pendiente)
    }

    /// Saca el encargo de la cola, porque su respuesta ya está publicada. Devuelve si estaba.
    pub fn quitar(&mut self, encargo: &Encargo) -> bool {
        let antes = self.pendiente.len();
        self.pendiente.retain(|p| !p.es(encargo));
        self.pendiente.len() != antes
    }
}

/// Lee la cola. Si el fichero no existe, la cola está vacía. Si no se entiende, es un error que
/// nombra la ruta y no se toca el fichero: ahí puede haber encargos que no se deben perder.
pub fn leer(ruta: &Path) -> Resultado<Cola> {
    let texto = ficheros::leer_si_existe(ruta)?;
    toml::from_str(&texto).map_err(|causa| Error::Pendientes(mensaje_cola_ilegible(ruta, &causa)))
}

/// Guarda la cola entera: primero en un temporal al lado y luego, de una vez, con su nombre.
pub fn guardar(ruta: &Path, cola: &Cola) -> Resultado<()> {
    let texto = toml::to_string(cola)
        .map_err(|causa| Error::Pendientes(mensaje_cola_sin_componer(&causa)))?;
    if let Some(carpeta) = ruta.parent() {
        ficheros::crear_carpeta(carpeta)?;
    }
    let temporal = ruta_del_temporal(ruta);
    ficheros::escribir(&temporal, &texto)?;
    // Si el cambio de nombre falla, el temporal no se queda al lado de la cola: se borra y se
    // devuelve el fallo del cambio de nombre, que es el que importa.
    ficheros::renombrar(&temporal, ruta).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporal);
    })
}

fn ruta_del_temporal(ruta: &Path) -> PathBuf {
    let mut nombre = ruta.as_os_str().to_owned();
    nombre.push(SUFIJO_DEL_TEMPORAL);
    PathBuf::from(nombre)
}

/// El error cuando la cola no se entiende.
fn mensaje_cola_ilegible(ruta: &Path, causa: &toml::de::Error) -> String {
    format!(
        "«{}» no se puede leer ({causa}); no se ha tocado: revísala a mano",
        ruta.display()
    )
}

/// El error cuando la cola no se puede pasar a TOML.
fn mensaje_cola_sin_componer(causa: &toml::ser::Error) -> String {
    format!("no se pudo componer la cola: {causa}")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn encargo(texto: &str) -> Encargo {
        Encargo {
            de: "claude2.md".into(),
            texto: texto.into(),
            forma: Forma::Linea,
        }
    }

    const UMBRAL: u32 = 3;

    #[test]
    fn un_fichero_que_no_existe_es_una_cola_vacia() {
        let dir = tempfile::tempdir().unwrap();
        let cola = leer(&dir.path().join("pendientes.toml")).unwrap();
        assert!(cola.esta_vacia());
    }

    /// Si el cambio de nombre falla, el temporal no se queda al lado de la cola.
    #[test]
    fn si_no_se_puede_poner_su_nombre_no_queda_el_temporal() {
        let dir = tempfile::tempdir().unwrap();
        // Una carpeta con el nombre de la cola: ningún sistema deja poner un fichero encima.
        let ruta = dir.path().join("pendientes.toml");
        std::fs::create_dir(&ruta).unwrap();
        std::fs::write(ruta.join("dentro"), "x").unwrap();

        assert!(guardar(&ruta, &Cola::default()).is_err());
        assert!(!ruta_del_temporal(&ruta).exists());
    }

    #[test]
    fn lo_que_entra_sale_al_quitarlo_y_solo_eso() {
        let mut cola = Cola::default();
        cola.anotar_fallo(&encargo("a"), "f", "m", UMBRAL);
        cola.anotar_fallo(&encargo("b"), "f", "m", UMBRAL);

        assert!(cola.quitar(&encargo("a")));
        assert!(!cola.contiene(&encargo("a")));
        assert!(cola.contiene(&encargo("b")));
        assert!(!cola.quitar(&encargo("a")));
    }

    #[test]
    fn se_atienden_en_orden_de_llegada_aunque_vuelvan_a_fallar() {
        let mut cola = Cola::default();
        for texto in ["primero", "segundo", "tercero"] {
            cola.anotar_fallo(&encargo(texto), "f", "m", UMBRAL);
        }
        cola.anotar_fallo(&encargo("primero"), "f", "m", UMBRAL);

        let textos: Vec<_> = cola.encargos().into_iter().map(|e| e.texto).collect();
        assert_eq!(textos, ["primero", "segundo", "tercero"]);
        let intentos: Vec<_> = cola.pendiente.iter().map(|p| p.intentos).collect();
        assert_eq!(intentos, [2, 1, 1]);
    }

    #[test]
    fn cada_fallo_suma_un_intento_y_guarda_el_ultimo_motivo() {
        let mut cola = Cola::default();
        cola.anotar_fallo(&encargo("a"), "2026-11-22 14:00", "no cargó", UMBRAL);
        cola.anotar_fallo(&encargo("a"), "2026-11-22 14:05", "no publicó", UMBRAL);

        let pendiente = &cola.pendiente[0];
        assert_eq!(pendiente.intentos, 2);
        assert_eq!(pendiente.motivo, "no publicó");
        assert_eq!(pendiente.llego_el, "2026-11-22 14:00");
    }

    #[test]
    fn al_llegar_al_umbral_se_cierra_y_si_no_se_pudo_se_vuelve_a_pedir_el_cierre() {
        // Al llegar al umbral el encargo se cierra, no se queda en la cola tras un solo aviso; y
        // si la respuesta de cierre no se publicó, la vuelta siguiente lo vuelve a intentar.
        let mut cola = Cola::default();
        let avisos: Vec<bool> = (0..5)
            .map(|_| cola.anotar_fallo(&encargo("a"), "f", "m", UMBRAL).is_some())
            .collect();
        assert_eq!(avisos, [false, false, true, true, true]);
    }

    #[test]
    fn una_cola_que_no_se_entiende_da_un_error_que_nombra_la_ruta_y_no_se_borra() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("pendientes.toml");
        std::fs::write(&ruta, "esto no es [toml").unwrap();

        let fallo = leer(&ruta).unwrap_err().to_string();

        assert!(fallo.contains("pendientes.toml"), "el error decía: {fallo}");
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), "esto no es [toml");
    }

    #[test]
    fn la_cola_sobrevive_a_guardarla_y_releerla_sin_dejar_el_temporal() {
        let dir = tempfile::tempdir().unwrap();
        // La carpeta del agente todavía no existe: guardar la crea.
        let ruta = dir.path().join("programator").join("pendientes.toml");
        let mut cola = Cola::default();
        cola.anotar_fallo(&encargo("a\ncon dos líneas"), "f", "m", 1);
        cola.anotar_fallo(&encargo("b"), "f", "m", UMBRAL);

        guardar(&ruta, &cola).unwrap();
        guardar(&ruta, &cola).unwrap();

        assert_eq!(leer(&ruta).unwrap(), cola);
        assert!(!ruta_del_temporal(&ruta).exists());
    }
}
