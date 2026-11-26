//! El paso de la 1.1.0 al registro de encargos, una sola vez.
//!
//! - **`lectura.json`.** En la 1.1.0, lo leído de cada buzón lo decía su desplazamiento en bytes.
//!   Sin migrarlo, todo lo antiguo de los buzones se volvería a atender al cambiar de versión,
//!   porque la instantánea inicial no se toma si `lectura.json` existe. Si todavía no hay carpeta
//!   de encargos, los encargos por debajo del desplazamiento de cada buzón se anotan como vistos,
//!   salvo los que esperan en la cola.
//! - **`pendientes.toml`.** Cada pendiente pasa al registro como pendiente, con sus intentos y su
//!   motivo, y el fichero se renombra a `pendientes.toml.migrado`. Si no se entiende, se deja
//!   intacto y la pasada falla con el error que lo nombra, como en la 1.1.0.
//!
//! Lo que decide va en funciones puras; [`migrar`] solo lee, llama y guarda.

use super::{disco, EstadoDelEncargo, FichaDeEncargo, RegistroDeEncargos};
use crate::arnes::pendientes::{self, Cola};
use crate::error::Resultado;
use crate::ficheros;
use crate::protocolo::{detectar, Encargo, RegistroLectura};
use std::path::{Path, PathBuf};

/// Lo que se añade al nombre de la cola de la 1.1.0 una vez migrada.
const SUFIJO_DE_LA_COLA_MIGRADA: &str = ".migrado";

/// Lo que la migración necesita saber.
pub struct Entrada<'a> {
    /// La carpeta del registro de encargos: si no existe, el canal viene de la 1.1.0.
    pub carpeta_de_los_encargos: &'a Path,
    /// La cola de la 1.1.0.
    pub ruta_de_los_pendientes: &'a Path,
    /// El registro de lectura, tal como se guardó.
    pub lectura: &'a RegistroLectura,
    /// Cada buzón ajeno con su contenido de ahora.
    pub buzones: &'a [(String, String)],
    /// El nombre del agente, para detectar sus encargos.
    pub agente: &'a str,
    /// La fecha que se anota en las fichas nuevas.
    pub fecha: &'a str,
}

/// Migra lo que quede de la 1.1.0 al registro y lo guarda. Sin nada que migrar, no hace nada.
pub fn migrar(registro: &mut RegistroDeEncargos, entrada: &Entrada) -> Resultado<()> {
    let hay_cola = entrada.ruta_de_los_pendientes.exists();
    let viene_de_la_1_1 = !entrada.carpeta_de_los_encargos.exists();
    if !hay_cola && !viene_de_la_1_1 {
        return Ok(());
    }
    // Si la cola no se entiende, se para aquí: sin ella no se sabe qué no dar por visto.
    let cola = pendientes::leer(entrada.ruta_de_los_pendientes)?;

    if viene_de_la_1_1 {
        for encargo in
            vistos_por_la_lectura(entrada.lectura, entrada.buzones, entrada.agente, &cola)
        {
            registro.anotar(&encargo, entrada.fecha, EstadoDelEncargo::Visto);
        }
    }
    let detectados: Vec<Encargo> = entrada
        .buzones
        .iter()
        .flat_map(|(nombre, contenido)| detectar(entrada.agente, nombre, contenido))
        .collect();
    for ficha in fichas_de_la_cola(&cola, &detectados) {
        registro.anotar_ficha(ficha);
    }

    disco::guardar_las_cambiadas(entrada.carpeta_de_los_encargos, registro)?;
    ficheros::crear_carpeta(entrada.carpeta_de_los_encargos)?;
    if hay_cola {
        ficheros::renombrar(
            entrada.ruta_de_los_pendientes,
            &ruta_de_la_cola_migrada(entrada.ruta_de_los_pendientes),
        )?;
    }
    Ok(())
}

/// Los encargos que la 1.1.0 ya había leído: los que caben en el desplazamiento guardado de cada
/// buzón, salvo los que esperan en la cola. Si el buzón encogió desde entonces, todo él.
pub fn vistos_por_la_lectura(
    lectura: &RegistroLectura,
    buzones: &[(String, String)],
    agente: &str,
    cola: &Cola,
) -> Vec<Encargo> {
    buzones
        .iter()
        .filter_map(|(nombre, contenido)| {
            let leidos = lectura.bytes_leidos(nombre)?;
            let leido = contenido.get(..leidos).unwrap_or(contenido);
            Some(detectar(agente, nombre, leido))
        })
        .flatten()
        .filter(|encargo| !cola.contiene(encargo))
        .collect()
}

/// Una ficha pendiente por cada encargo de la cola, con sus intentos, su motivo y su fecha. La
/// cola no guardaba el latido: si el encargo sigue en su buzón, se toma de ahí, para que la
/// detección lo reconozca y no lo atienda dos veces.
pub fn fichas_de_la_cola(cola: &Cola, detectados: &[Encargo]) -> Vec<FichaDeEncargo> {
    cola.pendientes()
        .iter()
        .map(|pendiente| {
            let de_la_cola = pendiente.encargo();
            let encargo = detectados
                .iter()
                .find(|detectado| {
                    detectado.de == de_la_cola.de && detectado.texto == de_la_cola.texto
                })
                .cloned()
                .unwrap_or(de_la_cola);
            let mut ficha =
                FichaDeEncargo::nueva(&encargo, pendiente.llego_el(), EstadoDelEncargo::Pendiente);
            ficha.intentos = pendiente.intentos();
            ficha.motivo = pendiente.motivo().to_string();
            ficha
        })
        .collect()
}

fn ruta_de_la_cola_migrada(ruta: &Path) -> PathBuf {
    let mut nombre = ruta.as_os_str().to_owned();
    nombre.push(SUFIJO_DE_LA_COLA_MIGRADA);
    PathBuf::from(nombre)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::protocolo::encargo::Forma;

    const AGENTE: &str = "Programator";

    /// Un buzón de la 1.1.0 con dos intervenciones ya leídas y, si `con_nuevo`, una tercera escrita
    /// después.
    fn buzon(con_nuevo: bool) -> (String, usize) {
        let leido = "LATIDO: 2026-11-20 10:00:00\nPara Programator: uno\n\n\
                     LATIDO: 2026-11-21 10:00:00\nPara Programator: dos\n\n";
        let mut texto = leido.to_string();
        if con_nuevo {
            texto.push_str("LATIDO: 2026-11-25 10:00:00\nPara Programator: tres\n");
        }
        (texto, leido.len())
    }

    fn lectura_hasta(nombre: &str, leido: &str) -> RegistroLectura {
        let mut lectura = RegistroLectura::default();
        lectura.delta(nombre, leido);
        lectura
    }

    fn encargo_de_la_cola(texto: &str) -> Encargo {
        Encargo {
            de: "claude.md".into(),
            texto: texto.into(),
            forma: Forma::Linea,
            latido: None,
        }
    }

    #[test]
    fn lo_leido_por_la_1_1_se_da_por_visto_y_lo_posterior_no() {
        let (texto, leidos) = buzon(true);
        let lectura = lectura_hasta("claude.md", &texto[..leidos]);
        let buzones = vec![("claude.md".to_string(), texto)];

        let vistos = vistos_por_la_lectura(&lectura, &buzones, AGENTE, &Cola::default());

        let textos: Vec<_> = vistos.iter().map(|e| e.texto.as_str()).collect();
        assert_eq!(textos, ["Para Programator: uno", "Para Programator: dos"]);
    }

    #[test]
    fn lo_que_espera_en_la_cola_no_se_da_por_visto() {
        let (texto, leidos) = buzon(false);
        let lectura = lectura_hasta("claude.md", &texto[..leidos]);
        let buzones = vec![("claude.md".to_string(), texto)];
        let mut cola = Cola::default();
        cola.anotar_fallo(&encargo_de_la_cola("Para Programator: dos"), "f", "m", 9);

        let vistos = vistos_por_la_lectura(&lectura, &buzones, AGENTE, &cola);

        assert_eq!(vistos.len(), 1);
        assert_eq!(vistos[0].texto, "Para Programator: uno");
    }

    #[test]
    fn un_buzon_sin_lectura_no_tiene_nada_visto() {
        let (texto, _) = buzon(false);
        let buzones = vec![("claude.md".to_string(), texto)];
        let vistos = vistos_por_la_lectura(
            &RegistroLectura::default(),
            &buzones,
            AGENTE,
            &Cola::default(),
        );
        assert!(vistos.is_empty());
    }

    #[test]
    fn la_ficha_de_la_cola_toma_el_latido_del_buzon_y_conserva_sus_intentos() {
        let (texto, _) = buzon(false);
        let detectados = detectar(AGENTE, "claude.md", &texto);
        let mut cola = Cola::default();
        let pendiente = encargo_de_la_cola("Para Programator: dos");
        cola.anotar_fallo(&pendiente, "2026-11-21 11:00:00", "no cargó", 9);
        cola.anotar_fallo(&pendiente, "2026-11-21 11:00:00", "no cargó otra vez", 9);

        let fichas = fichas_de_la_cola(&cola, &detectados);

        assert_eq!(fichas.len(), 1);
        assert_eq!(fichas[0].id, detectados[1].id());
        assert_eq!(fichas[0].intentos, 2);
        assert_eq!(fichas[0].motivo, "no cargó otra vez");
        assert_eq!(fichas[0].llego_el, "2026-11-21 11:00:00");
        assert_eq!(fichas[0].estado, EstadoDelEncargo::Pendiente);
    }

    /// B5: una cola escrita por la 1.1.0, tal cual, pasa al registro y se renombra.
    #[test]
    fn una_cola_de_la_1_1_se_migra_con_sus_intentos_y_se_renombra() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let carpeta = carpeta_temporal.path().join("encargos");
        let ruta_cola = carpeta_temporal.path().join("pendientes.toml");
        std::fs::write(
            &ruta_cola,
            "[[pendiente]]\nde = \"claude.md\"\ntexto = \"Para Programator: dos\"\n\
             forma = \"linea\"\nllego_el = \"2026-11-21 11:00:00\"\nintentos = 2\n\
             motivo = \"no cargó\"\navisado = false\n",
        )
        .unwrap();
        let mut registro = RegistroDeEncargos::default();

        migrar(
            &mut registro,
            &Entrada {
                carpeta_de_los_encargos: &carpeta,
                ruta_de_los_pendientes: &ruta_cola,
                lectura: &RegistroLectura::default(),
                buzones: &[],
                agente: AGENTE,
                fecha: "2026-11-25 10:00:00",
            },
        )
        .unwrap();

        let leidas = disco::leer(&carpeta).unwrap().fichas;
        assert_eq!(leidas.len(), 1);
        assert_eq!(leidas[0].intentos, 2);
        assert_eq!(leidas[0].motivo, "no cargó");
        assert!(!ruta_cola.exists());
        assert!(carpeta_temporal
            .path()
            .join("pendientes.toml.migrado")
            .is_file());
    }

    #[test]
    fn una_cola_ilegible_no_se_toca_y_es_un_error() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let carpeta = carpeta_temporal.path().join("encargos");
        let ruta_cola = carpeta_temporal.path().join("pendientes.toml");
        std::fs::write(&ruta_cola, "esto no [es").unwrap();

        let resultado = migrar(
            &mut RegistroDeEncargos::default(),
            &Entrada {
                carpeta_de_los_encargos: &carpeta,
                ruta_de_los_pendientes: &ruta_cola,
                lectura: &RegistroLectura::default(),
                buzones: &[],
                agente: AGENTE,
                fecha: "f",
            },
        );

        assert!(resultado.is_err());
        assert_eq!(std::fs::read_to_string(&ruta_cola).unwrap(), "esto no [es");
        assert!(!carpeta.exists());
    }

    /// Migrar dos veces no duplica: la segunda ya encuentra la carpeta y no hay cola.
    #[test]
    fn la_migracion_repetida_no_duplica() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let carpeta = carpeta_temporal.path().join("encargos");
        let ruta_cola = carpeta_temporal.path().join("pendientes.toml");
        let (texto, leidos) = buzon(false);
        let lectura = lectura_hasta("claude.md", &texto[..leidos]);
        let buzones = vec![("claude.md".to_string(), texto)];
        let entrada = Entrada {
            carpeta_de_los_encargos: &carpeta,
            ruta_de_los_pendientes: &ruta_cola,
            lectura: &lectura,
            buzones: &buzones,
            agente: AGENTE,
            fecha: "f",
        };

        let mut registro = RegistroDeEncargos::default();
        migrar(&mut registro, &entrada).unwrap();
        let mut otra_vez = RegistroDeEncargos::de_fichas(disco::leer(&carpeta).unwrap().fichas);
        migrar(&mut otra_vez, &entrada).unwrap();

        assert_eq!(disco::leer(&carpeta).unwrap().fichas.len(), 2);
        assert!(otra_vez.cambiadas().is_empty());
    }
}
