//! Lo que comparten las pruebas de la tanda: encargos de ejemplo escritos en disco, los datos de
//! una tanda de mentira y una fase 1 pasada con motores dobles. Solo existe al compilar las pruebas.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::recorrido::{pasar_encargos, Recorrido};
use super::version_del_motor::COMPILACION_DESCONOCIDA;
use super::{Pasado, CARPETA_DE_ENCARGOS, CARPETA_DE_RESULTADOS, CARPETA_DE_TRABAJO};
use crate::arnes::ciclo::Limites;
use crate::arnes::sesion::{Ajustes, EstadoMotor};
use crate::banco::ejecucion::Preparacion;
use crate::banco::ficha::{Encargo, FichaEncargo, NOMBRE_RUBRICA};
use crate::banco::pasadas::{self, PasadaGuardada, PasadasDeTanda};
use crate::banco::resultados::DatosDeTanda;
use crate::config::Verificacion;
use crate::motor::{MotorDoble, Respuesta, SolicitudHerramienta};
use crate::reloj::RelojDeMentira;

/// El encargo, también en disco, como lo lee `cargar_encargos`.
pub(super) fn encargo(dir: &Path, texto_ficha: &str, rubrica: Option<&str>) -> Encargo {
    let ficha: FichaEncargo = toml::from_str(texto_ficha).unwrap();
    let carpeta = dir.join(CARPETA_DE_ENCARGOS).join(&ficha.id);
    std::fs::create_dir_all(&carpeta).unwrap();
    std::fs::write(carpeta.join(crate::banco::ficha::NOMBRE_FICHA), texto_ficha).unwrap();
    std::fs::write(carpeta.join("enunciado.md"), "Haz el encargo.\n").unwrap();
    if let Some(rubrica) = rubrica {
        std::fs::write(carpeta.join(NOMBRE_RUBRICA), rubrica).unwrap();
    }
    Encargo { ficha, carpeta }
}

pub(super) fn criterio(dir: &Path) -> Encargo {
    encargo(
        dir,
        "id = \"E01\"\ntitulo = \"Protocolo\"\nfamilia = \"criterio\"\n",
        Some("- **5**: lo explica todo\n"),
    )
}

pub(super) fn formas(dir: &Path) -> Encargo {
    let partida = dir.join(CARPETA_DE_ENCARGOS).join("U01").join("partida");
    std::fs::create_dir_all(&partida).unwrap();
    std::fs::write(partida.join("Pedidos.cs"), "class Pedidos {}\n").unwrap();
    encargo(
        dir,
        "id = \"U01\"\ntitulo = \"Formas\"\nfamilia = \"uso\"\nclase = \"formas\"\n\
         auditado = \"Pedidos.cs\"\n\
         [[defectos]]\nlineas = [10]\ntipo = \"NUMERO_MAGICO\"\ndescripcion = \"un 0.21\"\n\
         [[defectos]]\nlineas = [40]\ntipo = \"METODO_LARGO\"\ndescripcion = \"Procesar\"\n",
        None,
    )
}

pub(super) fn oficio(dir: &Path) -> Encargo {
    encargo(
        dir,
        "id = \"E03\"\ntitulo = \"Rangos\"\nfamilia = \"oficio\"\nentrega = \"rangos.rs\"\n\
         colocar_en = \"src/rangos.rs\"\nherramientas = [\"rust\"]\n\
         [[comprobaciones]]\nnombre = \"basico\"\norden = \"cargo test\"\n\
         [[texto]]\nnombre = \"sin_unwrap\"\npatron = \"unwrap()\"\nmaximo = 0\n",
        None,
    )
}

/// Un encargo de oficio en PHP, un lenguaje que el banco no trae de serie.
pub(super) fn oficio_en_php(dir: &Path) -> Encargo {
    encargo(
        dir,
        "id = \"P01\"\ntitulo = \"Rangos en PHP\"\nfamilia = \"oficio\"\n\
         entrega = \"rangos.php\"\ncolocar_en = \"rangos.php\"\nherramientas = [\"php\"]\n\
         [[comprobaciones]]\nnombre = \"basico\"\norden = \"php pruebas.php\"\n",
        None,
    )
}

pub(super) fn datos() -> DatosDeTanda {
    DatosDeTanda {
        modelo: "doble".into(),
        huella_sha256: "00".repeat(32),
        compilacion_motor: COMPILACION_DESCONOCIDA.into(),
        version_programator: "0.11.0".into(),
        fecha: "2026-11-11 10:00".into(),
        semillas: vec![42],
        temperatura: 0.2,
        margen_lineas: 2,
        tiempo_maximo_encargo_segundos: 900,
        maquina_de_comprobacion: None,
        capas_en_gpu: None,
        vram_ocupada_mib: None,
        segundos_de_carga: None,
        tarjeta: None,
        sistema_de_comprobacion: None,
        rastro_fuera: Vec::new(),
        aislamiento: None,
        ajustes_del_motor: None,
    }
}

/// La tanda vive donde la pone `ejecutar`: en `<banco>/resultados/<tanda>`, con el banco en `dir`.
pub(super) fn carpeta_tanda(dir: &Path) -> PathBuf {
    dir.join(CARPETA_DE_RESULTADOS).join("tanda")
}

pub(super) fn preparacion() -> Preparacion<'static> {
    Preparacion {
        agente: "Programator",
        buzon_que_encarga: "banco",
        normas: "NORMAS\n",
        tiempo_maximo: Duration::from_secs(900),
    }
}

/// Pasa los encargos con un motor doble por encargo, en orden, y una semilla.
pub(super) fn pasar<'a>(
    dir: &Path,
    encargos: &'a [Encargo],
    guiones: Vec<Vec<Respuesta>>,
) -> Vec<Pasado<'a>> {
    let mut motores: VecDeque<MotorDoble> =
        guiones.into_iter().map(MotorDoble::con_guion).collect();
    let mut semillas_pedidas = Vec::new();
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let ajustes = Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal);
    let mut avisos = Vec::new();
    let tanda = carpeta_tanda(dir);
    let trabajo = tanda.join(CARPETA_DE_TRABAJO);
    let mut guardadas = PasadasDeTanda {
        tanda: datos(),
        pasadas: Vec::new(),
    };
    let pasados = pasar_encargos(
        &Recorrido {
            encargos,
            semillas: &[42],
            carpeta_de_trabajo: &trabajo,
        },
        &preparacion(),
        &ajustes,
        &mut |semilla| {
            semillas_pedidas.push(semilla);
            let motor = motores.pop_front().unwrap();
            Ok(EstadoMotor::Disponible(Box::new(motor)))
        },
        &mut RelojDeMentira::default(),
        &mut |linea| avisos.push(linea),
        &mut |pasado| {
            guardadas.pasadas.push(PasadaGuardada::de(pasado));
            pasadas::guardar(&guardadas, &tanda)
        },
    )
    .unwrap();
    assert!(semillas_pedidas.iter().all(|&s| s == 42));
    assert_eq!(avisos.len(), encargos.len());
    pasados
}

pub(super) fn pedir(nombre: &str, argumentos: serde_json::Value) -> Respuesta {
    Respuesta::Herramienta(SolicitudHerramienta {
        nombre: nombre.to_string(),
        argumentos,
    })
}
