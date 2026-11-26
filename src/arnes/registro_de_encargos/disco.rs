//! El registro de encargos en disco: un fichero TOML por encargo, `<id>.toml`, en la carpeta de
//! los encargos del espacio del agente. Aquí no se decide nada.
//!
//! Cada ficha se escribe de una vez, con un temporal al lado y un cambio de nombre
//! (`ficheros::escribir_de_una_vez`): un corte a mitad deja el temporal, que no cuenta como ficha.
//! Una ficha que no se entiende se aparta con la extensión `.ilegible` y se avisa, sin tumbar la
//! pasada: ahí puede haber algo que una persona quiera mirar.

use super::{FichaDeEncargo, RegistroDeEncargos};
use crate::error::{Error, Resultado};
use crate::ficheros;
use std::path::{Path, PathBuf};

/// La extensión del fichero de cada ficha.
const EXTENSION_DE_LA_FICHA: &str = "toml";

/// Lo que se añade al nombre de una ficha que no se entiende al apartarla.
const SUFIJO_DE_LA_ILEGIBLE: &str = ".ilegible";

/// Lo que se leyó de la carpeta de los encargos.
#[derive(Debug, Default)]
pub struct Leido {
    /// Las fichas que se entendieron, sin orden garantizado.
    pub fichas: Vec<FichaDeEncargo>,
    /// Un aviso por cada ficha que no se entendió y se apartó.
    pub avisos: Vec<String>,
}

/// El fichero de la ficha con identificador `id`.
pub fn ruta_de_la_ficha(carpeta: &Path, id: &str) -> PathBuf {
    carpeta.join(format!("{id}.{EXTENSION_DE_LA_FICHA}"))
}

/// Lee todas las fichas de `carpeta`. Si la carpeta no existe, no hay ninguna. Las que no se
/// entienden se apartan y dejan un aviso.
pub fn leer(carpeta: &Path) -> Resultado<Leido> {
    let mut leido = Leido::default();
    if !carpeta.is_dir() {
        return Ok(leido);
    }
    for ruta in ficheros::entradas(carpeta)? {
        if !es_ficha(&ruta) {
            continue;
        }
        let texto = ficheros::leer(&ruta)?;
        match toml::from_str::<FichaDeEncargo>(&texto) {
            Ok(ficha) => leido.fichas.push(ficha),
            Err(causa) => leido.avisos.push(apartar(&ruta, &causa)?),
        }
    }
    Ok(leido)
}

/// Guarda una ficha, de una vez.
pub fn guardar(carpeta: &Path, ficha: &FichaDeEncargo) -> Resultado<()> {
    let texto = toml::to_string(ficha).map_err(|causa| {
        Error::RegistroDeEncargos(mensaje_ficha_sin_componer(&ficha.id, &causa))
    })?;
    ficheros::escribir_de_una_vez(&ruta_de_la_ficha(carpeta, &ficha.id), &texto)
}

/// Guarda las fichas que cambiaron, una a una; cada una guardada deja de contar como cambiada. Al
/// primer fallo para y lo devuelve: las ya guardadas quedan bien y las demás siguen cambiadas.
pub fn guardar_las_cambiadas(carpeta: &Path, registro: &mut RegistroDeEncargos) -> Resultado<()> {
    let cambiadas: Vec<FichaDeEncargo> = registro.cambiadas().into_iter().cloned().collect();
    for ficha in cambiadas {
        guardar(carpeta, &ficha)?;
        registro.dar_por_guardada(&ficha.id);
    }
    Ok(())
}

/// Si en `carpeta` hay algún encargo sin respuesta, sin tocar nada. Una ficha que no se deja leer
/// cuenta como uno: la pasada la apartará y avisará.
pub fn hay_abiertos(carpeta: &Path) -> bool {
    let Ok(entradas) = ficheros::entradas(carpeta) else {
        return carpeta.exists();
    };
    entradas.iter().filter(|ruta| es_ficha(ruta)).any(|ruta| {
        ficheros::leer(ruta)
            .ok()
            .and_then(|texto| toml::from_str::<FichaDeEncargo>(&texto).ok())
            .is_none_or(|ficha| ficha.estado.esta_abierto())
    })
}

/// Borra la ficha con identificador `id`, si está.
pub fn borrar(carpeta: &Path, id: &str) -> Resultado<()> {
    ficheros::borrar_fichero_si_existe(&ruta_de_la_ficha(carpeta, id))
}

/// Si `ruta` es el fichero de una ficha: `<algo>.toml`, no un temporal ni una apartada.
fn es_ficha(ruta: &Path) -> bool {
    ruta.is_file()
        && ruta
            .extension()
            .is_some_and(|extension| extension == EXTENSION_DE_LA_FICHA)
}

/// Aparta la ficha que no se entiende y devuelve el aviso.
fn apartar(ruta: &Path, causa: &toml::de::Error) -> Resultado<String> {
    let mut apartada = ruta.as_os_str().to_owned();
    apartada.push(SUFIJO_DE_LA_ILEGIBLE);
    let apartada = PathBuf::from(apartada);
    ficheros::renombrar(ruta, &apartada)?;
    Ok(mensaje_ficha_apartada(ruta, &apartada, causa))
}

/// El aviso cuando una ficha no se entiende y se aparta.
fn mensaje_ficha_apartada(ruta: &Path, apartada: &Path, causa: &toml::de::Error) -> String {
    format!(
        "⚠️ La ficha de encargo «{}» no se entiende ({causa}); se ha apartado como «{}». Si era un \
         encargo sin atender, vuelve a pedirlo.",
        ruta.display(),
        apartada.display()
    )
}

/// El error cuando una ficha no se puede pasar a TOML.
fn mensaje_ficha_sin_componer(id: &str, causa: &toml::ser::Error) -> String {
    format!("la ficha del encargo {id} no se pudo componer: {causa}")
}

#[cfg(test)]
mod pruebas {
    use super::super::EstadoDelEncargo;
    use super::*;
    use crate::protocolo::encargo::{Encargo, Forma};

    fn ficha(texto: &str, estado: EstadoDelEncargo) -> FichaDeEncargo {
        let encargo = Encargo {
            de: "claude.md".into(),
            texto: texto.into(),
            forma: Forma::Linea,
            latido: Some("2026-11-25 10:00:00".into()),
        };
        FichaDeEncargo::nueva(&encargo, "2026-11-25 10:00:01", estado)
    }

    #[test]
    fn sin_carpeta_no_hay_fichas() {
        let carpeta = tempfile::tempdir().unwrap();
        let leido = leer(&carpeta.path().join("encargos")).unwrap();
        assert!(leido.fichas.is_empty() && leido.avisos.is_empty());
    }

    #[test]
    fn cada_estado_va_y_vuelve_igual() {
        use EstadoDelEncargo::*;
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let carpeta = carpeta_temporal.path().join("encargos");
        let mut guardadas = Vec::new();
        for (n, estado) in [Pendiente, EnCurso, Atendido, Cancelado, Cerrado, Visto]
            .into_iter()
            .enumerate()
        {
            let mut una = ficha(&format!("encargo {n}"), estado);
            una.intentos = n as u32;
            una.motivo = format!("motivo {n}");
            if !estado.esta_abierto() {
                una.terminar(estado, "2026-11-25 11:00:00");
            }
            guardar(&carpeta, &una).unwrap();
            guardadas.push(una);
        }

        let mut leidas = leer(&carpeta).unwrap().fichas;
        leidas.sort_by(|a, b| a.texto.cmp(&b.texto));
        assert_eq!(leidas, guardadas);
    }

    /// Uso real del 26/11: la ficha dice con qué prioridad se atiende, sin releer el texto. Una
    /// ficha de antes, que no la trae, se lee como normal.
    #[test]
    fn la_ficha_guarda_la_prioridad_y_las_de_antes_son_normales() {
        use crate::arnes::prioridad::Prioridad;
        let carpeta = tempfile::tempdir().unwrap();
        let alta = ficha("uno\nPrioridad: alta", EstadoDelEncargo::Pendiente);
        assert_eq!(alta.prioridad, Prioridad::Alta);
        guardar(carpeta.path(), &alta).unwrap();
        let escrita = std::fs::read_to_string(ruta_de_la_ficha(carpeta.path(), &alta.id)).unwrap();
        assert!(escrita.contains("prioridad = \"alta\""), "{escrita}");

        let sin_prioridad: String = escrita
            .lines()
            .filter(|linea| !linea.starts_with("prioridad"))
            .map(|linea| format!("{linea}\n"))
            .collect();
        std::fs::write(ruta_de_la_ficha(carpeta.path(), &alta.id), sin_prioridad).unwrap();
        let leidas = leer(carpeta.path()).unwrap().fichas;
        assert_eq!(leidas.len(), 1);
        assert_eq!(leidas[0].prioridad, Prioridad::Normal);

        let mal_escrita = ficha("dos\nPrioridad: urgente", EstadoDelEncargo::Pendiente);
        assert_eq!(mal_escrita.prioridad, Prioridad::Normal);
    }

    #[test]
    fn la_ficha_se_guarda_con_su_identificador_por_nombre() {
        let carpeta = tempfile::tempdir().unwrap();
        let una = ficha("x", EstadoDelEncargo::Pendiente);
        guardar(carpeta.path(), &una).unwrap();
        assert!(ruta_de_la_ficha(carpeta.path(), &una.id).is_file());
        assert!(!ficheros::ruta_del_temporal(&ruta_de_la_ficha(carpeta.path(), &una.id)).exists());
    }

    /// Un corte a mitad de escritura deja el temporal: no cuenta como ficha.
    #[test]
    fn un_temporal_sobrante_no_cuenta_como_ficha() {
        let carpeta = tempfile::tempdir().unwrap();
        let una = ficha("x", EstadoDelEncargo::Pendiente);
        let temporal = ficheros::ruta_del_temporal(&ruta_de_la_ficha(carpeta.path(), &una.id));
        std::fs::write(&temporal, "a medias").unwrap();

        let leido = leer(carpeta.path()).unwrap();

        assert!(leido.fichas.is_empty() && leido.avisos.is_empty());
        assert!(temporal.exists());
    }

    #[test]
    fn una_ficha_ilegible_se_aparta_y_avisa_sin_tumbar_las_demas() {
        let carpeta = tempfile::tempdir().unwrap();
        let buena = ficha("buena", EstadoDelEncargo::Pendiente);
        guardar(carpeta.path(), &buena).unwrap();
        let mala = carpeta.path().join("0000000000000000.toml");
        std::fs::write(&mala, "esto = no es [una ficha").unwrap();

        let leido = leer(carpeta.path()).unwrap();

        assert_eq!(leido.fichas, vec![buena]);
        assert_eq!(leido.avisos.len(), 1);
        assert!(leido.avisos[0].contains("0000000000000000.toml"));
        assert!(!mala.exists());
        assert!(carpeta
            .path()
            .join("0000000000000000.toml.ilegible")
            .is_file());

        // Apartada, la siguiente lectura ya no avisa.
        assert!(leer(carpeta.path()).unwrap().avisos.is_empty());
    }

    #[test]
    fn se_guardan_solo_las_cambiadas_y_dejan_de_serlo() {
        let carpeta = tempfile::tempdir().unwrap();
        let mut registro = RegistroDeEncargos::default();
        registro.anotar(
            &ficha("a", EstadoDelEncargo::Pendiente).encargo(),
            "f",
            EstadoDelEncargo::Pendiente,
        );

        guardar_las_cambiadas(carpeta.path(), &mut registro).unwrap();

        assert!(registro.cambiadas().is_empty());
        assert_eq!(leer(carpeta.path()).unwrap().fichas, registro.fichas());
    }

    /// Si una ficha no se puede guardar, sigue cambiada para el intento siguiente.
    #[test]
    fn una_ficha_que_no_se_guarda_sigue_cambiada() {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let mut registro = RegistroDeEncargos::default();
        let encargo = ficha("a", EstadoDelEncargo::Pendiente).encargo();
        registro.anotar(&encargo, "f", EstadoDelEncargo::Pendiente);
        // Una carpeta con el nombre de la ficha: no se puede poner el fichero encima.
        let ruta = ruta_de_la_ficha(carpeta_temporal.path(), &encargo.id());
        std::fs::create_dir(&ruta).unwrap();
        std::fs::write(ruta.join("dentro"), "x").unwrap();

        assert!(guardar_las_cambiadas(carpeta_temporal.path(), &mut registro).is_err());
        assert_eq!(registro.cambiadas().len(), 1);
    }

    #[test]
    fn hay_abiertos_mira_el_estado_sin_tocar_nada() {
        let carpeta = tempfile::tempdir().unwrap();
        assert!(!hay_abiertos(&carpeta.path().join("no-existe")));
        guardar(carpeta.path(), &ficha("hecho", EstadoDelEncargo::Atendido)).unwrap();
        assert!(!hay_abiertos(carpeta.path()));
        guardar(carpeta.path(), &ficha("falta", EstadoDelEncargo::Pendiente)).unwrap();
        assert!(hay_abiertos(carpeta.path()));
    }

    #[test]
    fn una_ficha_ilegible_cuenta_como_abierta_y_no_se_aparta_al_mirar() {
        let carpeta = tempfile::tempdir().unwrap();
        let mala = carpeta.path().join("0000000000000000.toml");
        std::fs::write(&mala, "no [es").unwrap();
        assert!(hay_abiertos(carpeta.path()));
        assert!(mala.exists());
    }

    #[test]
    fn borrar_quita_la_ficha_y_no_falla_si_no_estaba() {
        let carpeta = tempfile::tempdir().unwrap();
        let una = ficha("x", EstadoDelEncargo::Atendido);
        guardar(carpeta.path(), &una).unwrap();
        borrar(carpeta.path(), &una.id).unwrap();
        assert!(leer(carpeta.path()).unwrap().fichas.is_empty());
        borrar(carpeta.path(), &una.id).unwrap();
    }
}
