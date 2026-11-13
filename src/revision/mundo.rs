//! El taller y la mesa de verdad de la revisión autónoma: la descarga de Hugging Face, el motor
//! propio que se suelta, el banco con sus pruebas ocultas, el registro y `programator.toml`.
//!
//! Aquí no se decide nada: el recorrido está en `probar` y en `cerrar`, que se prueban con un
//! taller y una mesa falsos. Lo que sí se decide al montar el mundo (qué rúbricas faltan, cómo se
//! escribe la ruta del sustituto) va en funciones puras con sus pruebas.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use super::cerrar::Mesa;
use super::encargo::CandidatoPedido;
use super::mensajes;
use super::ordenes;
use super::probados::{self, Probados};
use super::probar::Taller;
use super::propuesta::Propuesta;
use super::traer::Traido;
use crate::banco::resultados::{
    leer_con_las_rubricas, rehacer_informe, EstadoDeNota, ResultadosDeTanda, NOMBRE_RESULTADOS,
};
use crate::banco::tanda;
use crate::config::{modelo_por_defecto, Config};
use crate::error::{Error, Resultado};
use crate::motor::proceso::{self, EstadoServidor, ServidorLocal};
use crate::protocolo::{DetalleEncargo, Latido};
use crate::registro::anotar_tanda::{anotar_tanda, Desenlace};
use crate::registro::mensajes::mensaje_no_se_anota;

/// La extensión de los modelos: el nombre del registro es el del fichero sin ella.
const EXTENSION_DEL_MODELO: &str = ".gguf";

/// Lo que comparten el taller y la mesa.
pub struct Mundo<'a> {
    /// La configuración de ahora. Si `cerrar` cambia el modelo por defecto, se cambia también aquí.
    pub config: Config,
    pub ruta_de_la_configuracion: PathBuf,
    /// El servidor propio del ciclo, que la revisión suelta antes de pasar el banco.
    pub servidor: &'a RefCell<Option<ServidorLocal>>,
    pub version: &'a str,
    pub carpeta_del_mes: PathBuf,
    /// Si se cambió `[motor] modelo`: quien lleve el ciclo tiene que volver a leer la configuración.
    pub cambio_el_modelo: bool,
    /// Dónde se dice lo que se va haciendo, para que quien mire el canal sepa que no está colgado.
    pub latido: AvisoEnElLatido,
}

/// El latido del ciclo, que la revisión va poniendo al día.
pub struct AvisoEnElLatido {
    pub ruta: PathBuf,
    pub intervalo_segundos: u64,
    /// El buzón que pidió la revisión.
    pub buzon: String,
}

impl Mundo<'_> {
    fn comprobar(&self, tanda: &Path) -> Resultado<()> {
        tanda::comprobar(&self.config, tanda)
    }

    fn guardar(&self, probados: &Probados) -> Resultado<()> {
        probados::guardar(&self.carpeta_del_mes, probados)
    }
}

impl Taller for Mundo<'_> {
    fn descargar(&mut self, candidato: &CandidatoPedido) -> Resultado<PathBuf> {
        let traido = ordenes::descargar(
            &self.config,
            &candidato.repositorio,
            &candidato.fichero,
            &mut |aviso| println!("{}", mensajes::aviso_de_descarga(&aviso)),
        )?;
        Ok(match traido {
            Traido::Descargado(hecho) => hecho.ruta,
            Traido::YaEstaba(ruta) => ruta,
        })
    }

    fn soltar_motor(&mut self) -> Resultado<()> {
        // Al soltarlo se para el proceso y se espera a que salga: así queda libre la tarjeta.
        drop(self.servidor.borrow_mut().take());
        let puerto = self.config.motor.puerto;
        match proceso::hay_servidor(puerto) {
            EstadoServidor::NoDisponible => Ok(()),
            EstadoServidor::Listo | EstadoServidor::Cargando => {
                Err(Error::Revision(mensajes::motor_ajeno(puerto)))
            }
        }
    }

    fn pasar_tanda(&mut self, modelo: &Path) -> Resultado<PathBuf> {
        let config = self
            .config
            .clone()
            .con_modelo(Some(&modelo.to_string_lossy()));
        let carpeta_del_banco = self
            .config
            .resolver(&self.config.revision.carpeta_del_banco);
        tanda::pasar(&config, &carpeta_del_banco, self.version)
    }

    fn comprobar_tanda(&mut self, tanda: &Path) -> Resultado<()> {
        self.comprobar(tanda)
    }

    fn avisar(&mut self, linea: &str) {
        println!("{linea}");
        Latido::en_atencion(
            chrono::Local::now(),
            self.latido.intervalo_segundos,
            DetalleEncargo::nuevo(&self.latido.buzon, linea),
        )
        .guardar_con_aviso(&self.latido.ruta);
    }

    fn guardar_lo_probado(&mut self, probados: &Probados) -> Resultado<()> {
        self.guardar(probados)
    }
}

impl Mesa for Mundo<'_> {
    fn comprobar_tanda(&mut self, tanda: &Path) -> Resultado<()> {
        self.comprobar(tanda)
    }

    fn rubricas_sin_puntuar(&mut self, tanda: &Path) -> Resultado<Vec<PathBuf>> {
        rubricas_sin_puntuar(tanda)
    }

    fn rehacer_informe(&mut self, tanda: &Path) -> Resultado<()> {
        rehacer_informe(&tanda.join(NOMBRE_RESULTADOS)).map(|_| ())
    }

    fn anotar(&mut self, tanda: &Path) -> Resultado<Result<(), String>> {
        let registro = self
            .config
            .resolver(&self.config.agente.registro_de_modelos);
        Ok(match anotar_tanda(tanda, &registro)? {
            Desenlace::Anotada(..) => Ok(()),
            Desenlace::NoSeAnota(motivo) => Err(mensaje_no_se_anota(&motivo, tanda)),
        })
    }

    fn proponer(&mut self, modelo_en_uso: Option<&str>) -> Resultado<Propuesta> {
        ordenes::proponer_con(&self.config, modelo_en_uso.map(str::to_string))
            .map(|hecha| hecha.propuesta)
    }

    fn cambiar_el_modelo_por_defecto(&mut self, nombre: &str) -> Resultado<()> {
        let ruta = ruta_del_sustituto(
            self.config.motor.modelo.as_deref(),
            &self.config.revision.carpeta_de_modelos,
            nombre,
        );
        modelo_por_defecto::cambiar_en(&self.ruta_de_la_configuracion, &ruta)?;
        self.config.motor.modelo = Some(ruta);
        self.cambio_el_modelo = true;
        // El motor propio, si lo hay, tiene cargado el viejo: el siguiente encargo arranca el nuevo.
        drop(self.servidor.borrow_mut().take());
        Ok(())
    }

    fn retirar(&mut self, nombre: &str, modelo_en_uso: Option<&str>) -> Resultado<u64> {
        let fichero = format!("{nombre}{EXTENSION_DEL_MODELO}");
        ordenes::retirar_con(&self.config, &fichero, modelo_en_uso.map(str::to_string))
            .map(|retirada| retirada.liberados)
    }

    fn guardar_lo_probado(&mut self, probados: &Probados) -> Resultado<()> {
        self.guardar(probados)
    }
}

/// Las rúbricas de una tanda que siguen sin nota, con su ruta.
pub fn rubricas_sin_puntuar(tanda: &Path) -> Resultado<Vec<PathBuf>> {
    let resultados = leer_con_las_rubricas(&tanda.join(NOMBRE_RESULTADOS))?;
    Ok(pendientes(&resultados, tanda))
}

fn pendientes(resultados: &ResultadosDeTanda, tanda: &Path) -> Vec<PathBuf> {
    resultados
        .encargos
        .iter()
        .flat_map(|e| e.semillas.iter())
        .filter(|s| s.estado == EstadoDeNota::Pendiente)
        .filter_map(|s| s.rubrica.as_deref())
        .map(|rubrica| tanda.join(rubrica))
        .collect()
}

/// Cómo se escribe en `programator.toml` el modelo nuevo: en la misma carpeta que el actual, tal
/// como esté escrita; sin modelo actual, en la carpeta de modelos de `[revision]`.
fn ruta_del_sustituto(actual: Option<&str>, carpeta_de_modelos: &str, nombre: &str) -> String {
    let fichero = format!("{nombre}{EXTENSION_DEL_MODELO}");
    match actual.and_then(|ruta| ruta.rfind(['/', '\\']).map(|corte| &ruta[..=corte])) {
        Some(carpeta) => format!("{carpeta}{fichero}"),
        None if actual.is_some() => fichero,
        None => format!(
            "{}/{fichero}",
            carpeta_de_modelos.trim_end_matches(['/', '\\'])
        ),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::pruebas::tanda_de_ejemplo;

    #[test]
    fn el_sustituto_va_a_la_carpeta_del_modelo_actual_escrita_igual() {
        assert_eq!(
            ruta_del_sustituto(Some("modelos/devstral.gguf"), "otra", "nuevo"),
            "modelos/nuevo.gguf"
        );
        assert_eq!(
            ruta_del_sustituto(Some(r"D:\ia\devstral.gguf"), "otra", "nuevo"),
            r"D:\ia\nuevo.gguf"
        );
        assert_eq!(
            ruta_del_sustituto(Some("devstral.gguf"), "otra", "nuevo"),
            "nuevo.gguf"
        );
        assert_eq!(
            ruta_del_sustituto(None, "modelos/", "nuevo"),
            "modelos/nuevo.gguf"
        );
    }

    #[test]
    fn las_rubricas_pendientes_salen_con_la_ruta_de_su_tanda() {
        let resultados = tanda_de_ejemplo();
        let tanda = Path::new("banco/resultados/t1");

        let faltan = pendientes(&resultados, tanda);

        assert!(!faltan.is_empty(), "la tanda de ejemplo deja una pendiente");
        assert!(faltan.iter().all(|r| r.starts_with(tanda)), "{faltan:?}");
    }
}
