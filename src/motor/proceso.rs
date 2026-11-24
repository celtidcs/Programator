//! Ciclo de vida de `llama-server`.
//!
//! Si ya hay un servidor escuchando en el puerto, se reutiliza y no se mata al terminar: puede ser
//! del usuario. El que arranca Programator sí es suyo, y lo cierra al salir.
//!
//! Aquí vive `ServidorLocal`, el proceso propio que se cierra al soltarlo. El resto se reparte por
//! submódulos: la línea de argumentos (`linea_de_arranque`), el sondeo del puerto (`salud`), lo
//! instalado en el disco (`instalacion`) y el cierre ante Ctrl+C (`cierre`).

mod cierre;
mod instalacion;
mod linea_de_arranque;
mod salud;

pub use cierre::registrar_cierre_ordenado;
pub use instalacion::{revisar_motor, MotorInstalado};
pub use salud::{hay_servidor, hay_servidor_url, EstadoServidor};

use crate::config::MotorConfig;
use crate::error::{Error, Resultado};
use crate::motor::mensajes;
use cierre::{matar_y_esperar, registrar_para_cierre_ordenado, tomar_hijo, Compartido};
use linea_de_arranque::linea_de_arranque;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};

/// Un `llama-server` lanzado por Programator, que se cierra al soltarlo.
#[derive(Debug)]
pub struct ServidorLocal {
    hijo: Compartido,
    modelo: PathBuf,
    capas_gpu: u32,
    vram_libre_antes: Option<u64>,
    vram_ocupada_mib: Option<u64>,
}

impl ServidorLocal {
    /// Lanza el servidor. Si el binario no existe, falla nombrando la ruta buscada.
    ///
    /// `capas_gpu` es la resolución ya calculada por `encaje::resolver`, no `config.capas_gpu`:
    /// ver la documentación de `linea_de_arranque`. `vram_libre_antes` es la VRAM libre, en
    /// bytes, medida justo antes de lanzarlo; con ella se calcula después lo que ocupa.
    pub fn arrancar(
        config: &MotorConfig,
        binario: &Path,
        modelo: &Path,
        capas_gpu: u32,
        vram_libre_antes: Option<u64>,
    ) -> Resultado<Self> {
        if !binario.exists() {
            return Err(Error::ConfiguracionDelMotor(mensajes::sin_binario(binario)));
        }

        // La línea de arranque empieza siempre por el binario: se lanza el binario con el resto.
        // Antes se partía la lista y su rama vacía, que no puede darse, repetía el texto de «no se
        // encontró el binario».
        let argumentos = linea_de_arranque(config, binario, modelo, capas_gpu);
        let hijo = Command::new(binario)
            .args(argumentos.iter().skip(1))
            .spawn()
            .map_err(|e| Error::ConfiguracionDelMotor(mensajes::no_se_lanzo(binario, &e)))?;

        let servidor = Self::desde_hijo(hijo, modelo, capas_gpu, vram_libre_antes);
        registrar_para_cierre_ordenado(&servidor.hijo);
        Ok(servidor)
    }

    /// Envuelve un proceso ya lanzado. No lo registra para el cierre ordenado: eso es de
    /// `arrancar`, y así las pruebas no pisan el registro compartido.
    fn desde_hijo(
        hijo: Child,
        modelo: &Path,
        capas_gpu: u32,
        vram_libre_antes: Option<u64>,
    ) -> Self {
        Self {
            hijo: Arc::new(Mutex::new(Some(hijo))),
            modelo: modelo.to_path_buf(),
            capas_gpu,
            vram_libre_antes,
            vram_ocupada_mib: None,
        }
    }

    /// Si el proceso sigue vivo. Un `llama-server` recién lanzado puede no escuchar aún en el
    /// puerto: esto distingue «todavía arrancando» de «se murió».
    pub fn sigue_vivo(&self) -> bool {
        let mut guardia = match self.hijo.lock() {
            Ok(guardia) => guardia,
            Err(envenenado) => envenenado.into_inner(),
        };
        match guardia.as_mut() {
            Some(hijo) => matches!(hijo.try_wait(), Ok(None)),
            None => false,
        }
    }

    /// El fichero del modelo con el que se lanzó.
    pub fn modelo(&self) -> &Path {
        &self.modelo
    }

    /// Las capas que se pidieron a la GPU al lanzarlo.
    pub fn capas_gpu(&self) -> u32 {
        self.capas_gpu
    }

    /// La VRAM libre, en bytes, justo antes de lanzarlo; `None` si no se pudo medir.
    pub fn vram_libre_antes(&self) -> Option<u64> {
        self.vram_libre_antes
    }

    /// La VRAM que ocupa una vez cargado, en MiB; `None` hasta que se mide al quedar listo.
    pub fn vram_ocupada_mib(&self) -> Option<u64> {
        self.vram_ocupada_mib
    }

    /// Anota la VRAM que ocupa, medida al pasar de cargando a listo.
    pub fn anotar_vram_ocupada(&mut self, mib: Option<u64>) {
        self.vram_ocupada_mib = mib;
    }
}

impl Drop for ServidorLocal {
    fn drop(&mut self) {
        if let Some(hijo) = tomar_hijo(&self.hijo) {
            matar_y_esperar(hijo);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La configuración del motor de las pruebas de este módulo y de sus submódulos.
    pub(super) fn config_de_prueba() -> crate::config::MotorConfig {
        crate::config::MotorConfig {
            binario: Some("herramientas/llama-server.exe".into()),
            modelo: Some("modelos/devstral.gguf".into()),
            puerto: 8080,
            contexto: 32768,
            capas_gpu: crate::config::CapasGpu::Fijas(99),
            margen_vram_mib: 1024,
            cache_kv: "q8_0".into(),
            paralelo: 1,
            empezar_en_frio: true,
            umbral_aviso_contexto: 70,
            bytes_por_token: 4,
            tiempo_lectura_segundos: crate::config::tiempo_lectura_motor_segundos_por_defecto(),
            tiempo_escritura_segundos: crate::config::tiempo_escritura_motor_segundos_por_defecto(),
            soltar_modelo: crate::config::SoltarModelo::Auto,
            vram_maxima_mib: None,
            espera_de_carga_segundos: crate::config::ESPERA_DE_CARGA_POR_DEFECTO_SEGUNDOS,
            atencion_rapida: crate::config::AtencionRapida::default(),
        }
    }

    /// Sin el binario del motor, el fallo es del motor y sale antes de lanzar nada.
    #[test]
    fn sin_binario_el_fallo_es_del_motor() {
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("no-existe-llama-server");

        let fallo = ServidorLocal::arrancar(
            &config_de_prueba(),
            &binario,
            &dir.path().join("m.gguf"),
            0,
            None,
        )
        .err()
        .unwrap();

        assert!(
            matches!(fallo, Error::ConfiguracionDelMotor(_)),
            "{fallo:?}"
        );
        assert!(
            fallo.to_string().starts_with("configuración inválida: "),
            "{fallo}"
        );
    }

    #[test]
    fn arrancar_con_un_binario_inexistente_da_un_error_que_nombra_la_ruta() {
        let config = config_de_prueba();

        let fallo = ServidorLocal::arrancar(
            &config,
            std::path::Path::new("no/existe/llama-server.exe"),
            std::path::Path::new("x.gguf"),
            99,
            None,
        )
        .unwrap_err();

        assert!(
            fallo.to_string().contains("no/existe"),
            "el error decía: {fallo}"
        );
    }

    #[test]
    fn un_servidor_que_termino_no_sigue_vivo_y_uno_que_tarda_si() {
        let rapido = crate::plataforma::ordenes_de_prueba::eco_y_codigo("x", 0);
        let mut hijo = std::process::Command::new(&rapido[0])
            .args(&rapido[1..])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        hijo.wait().unwrap();
        let terminado = ServidorLocal::desde_hijo(hijo, Path::new("m.gguf"), 1, None);
        assert!(!terminado.sigue_vivo());

        let lento = crate::plataforma::ordenes_de_prueba::tardar_mucho();
        let hijo = std::process::Command::new(&lento[0])
            .args(&lento[1..])
            .spawn()
            .unwrap();
        let _guardia = MataAlSoltar(hijo.id());
        let vivo = ServidorLocal::desde_hijo(hijo, Path::new("m.gguf"), 1, None);
        assert!(vivo.sigue_vivo());
        assert_eq!(vivo.modelo(), Path::new("m.gguf"));
    }

    /// Best-effort: mata el proceso de prueba por identificador aunque la prueba entre en pánico
    /// antes de llegar al final. Sin esto, un `assert!` fallido a mitad dejaría un proceso de
    /// sobra colgado en la máquina de quien ejecutó la suite.
    pub(super) struct MataAlSoltar(pub(super) u32);

    impl Drop for MataAlSoltar {
        fn drop(&mut self) {
            crate::plataforma::ordenes_de_prueba::matar_proceso(self.0);
        }
    }
}
