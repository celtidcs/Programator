//! Antes de cargar el modelo, se prueba a preparar una carpeta de comprobación como se preparará
//! la de verdad.
//!
//! En uso real (13/11/2026) la etiqueta de integridad baja falló en una unidad donde el usuario
//! solo tiene permiso de modificar, y no se supo hasta terminar la tanda, después de una hora de
//! modelo. Lo que impida aislar se dice ahora al empezar, con el modelo aún sin cargar.

use std::path::Path;

use super::lanzador::LanzadorConEntorno;
use crate::banco::mensajes::mensaje_sin_preparar;
use crate::error::{Error, Resultado};
use crate::ficheros::{borrar_si_existe, crear_carpeta};

/// La carpeta de usar y tirar, dentro de la de resultados del banco, que es donde irá la de
/// comprobación de cada tanda: así se prueba en la misma unidad y con los mismos permisos.
pub const CARPETA_DE_LA_PRUEBA_PREVIA: &str = "prueba-previa-del-aislamiento";

/// Prepara una carpeta de usar y tirar dentro de `resultados` con `lanzador`, y la borra, salga
/// bien o mal. Si no se puede preparar, el error dice qué falló y cómo apagar las pruebas ocultas.
pub fn probar_el_lanzador<L: LanzadorConEntorno>(
    lanzador: &mut L,
    resultados: &Path,
) -> Resultado<()> {
    let prueba = resultados.join(CARPETA_DE_LA_PRUEBA_PREVIA);
    borrar_si_existe(&prueba)?;
    crear_carpeta(&prueba)?;
    let preparada = lanzador.preparar_carpeta(&prueba);
    let borrada = borrar_si_existe(&prueba);
    preparada.map_err(|fallo| Error::Banco(mensaje_sin_preparar(&fallo)))?;
    borrada
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::orden_acotada::DesenlaceDeOrden;
    use std::path::PathBuf;
    use std::time::Duration;

    /// Un lanzador que solo apunta qué carpeta le dieron a preparar, y falla si se le pide.
    struct DeMentira {
        falla: bool,
        preparada: Option<PathBuf>,
        existia_al_prepararla: bool,
        tenia_algo_al_prepararla: bool,
    }

    impl DeMentira {
        fn nuevo(falla: bool) -> Self {
            Self {
                falla,
                preparada: None,
                existia_al_prepararla: false,
                tenia_algo_al_prepararla: false,
            }
        }
    }

    impl LanzadorConEntorno for DeMentira {
        fn preparar_carpeta(&mut self, carpeta: &Path) -> Resultado<()> {
            self.preparada = Some(carpeta.to_path_buf());
            self.existia_al_prepararla = carpeta.is_dir();
            self.tenia_algo_al_prepararla = std::fs::read_dir(carpeta)
                .map(|mut entradas| entradas.next().is_some())
                .unwrap_or(false);
            if self.falla {
                Err(Error::Aislamiento("Acceso denegado. (os error 5)".into()))
            } else {
                Ok(())
            }
        }

        fn lanzar(
            &mut self,
            _argumentos: &[String],
            _directorio: &Path,
            _entorno: &[(String, String)],
            _tope: Duration,
        ) -> DesenlaceDeOrden {
            DesenlaceDeOrden::NoSeLanzo("no se lanza nada en la prueba previa".into())
        }
    }

    #[test]
    fn prepara_una_carpeta_que_ya_existe_dentro_de_resultados_y_la_borra() {
        let dir = tempfile::tempdir().unwrap();
        let resultados = dir.path().join("resultados");
        let mut lanzador = DeMentira::nuevo(false);

        probar_el_lanzador(&mut lanzador, &resultados).unwrap();

        let preparada = lanzador.preparada.unwrap();
        assert_eq!(preparada, resultados.join(CARPETA_DE_LA_PRUEBA_PREVIA));
        assert!(lanzador.existia_al_prepararla);
        assert!(!preparada.exists());
    }

    #[test]
    fn si_no_se_puede_preparar_lo_dice_antes_del_modelo_y_no_deja_la_carpeta() {
        let dir = tempfile::tempdir().unwrap();
        let mut lanzador = DeMentira::nuevo(true);

        let fallo = probar_el_lanzador(&mut lanzador, dir.path())
            .unwrap_err()
            .to_string();

        assert!(fallo.contains("Acceso denegado"), "{fallo}");
        assert!(fallo.contains("antes de cargar el modelo"), "{fallo}");
        assert!(fallo.contains("activa = false"), "{fallo}");
        assert!(!dir.path().join(CARPETA_DE_LA_PRUEBA_PREVIA).exists());
    }

    #[test]
    fn lo_que_quedara_de_una_prueba_anterior_se_borra_antes() {
        let dir = tempfile::tempdir().unwrap();
        let resto = dir
            .path()
            .join(CARPETA_DE_LA_PRUEBA_PREVIA)
            .join("viejo.txt");
        std::fs::create_dir_all(resto.parent().unwrap()).unwrap();
        std::fs::write(&resto, "x").unwrap();

        probar_el_lanzador(&mut DeMentira::nuevo(false), dir.path()).unwrap();

        assert!(!resto.exists());
    }

    #[test]
    fn la_carpeta_se_prepara_vacia_aunque_quedaran_restos() {
        let dir = tempfile::tempdir().unwrap();
        let resto = dir
            .path()
            .join(CARPETA_DE_LA_PRUEBA_PREVIA)
            .join("viejo.txt");
        std::fs::create_dir_all(resto.parent().unwrap()).unwrap();
        std::fs::write(&resto, "x").unwrap();
        let mut lanzador = DeMentira::nuevo(false);

        probar_el_lanzador(&mut lanzador, dir.path()).unwrap();

        assert!(lanzador.existia_al_prepararla);
        assert!(!lanzador.tenia_algo_al_prepararla);
    }
}
