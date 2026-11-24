//! Qué hay instalado del motor en el disco: si está el ejecutable y si lleva CUDA al lado.

use std::path::{Path, PathBuf};

/// Qué hay instalado del motor en el disco. No dice si está corriendo —eso es `EstadoServidor`—
/// ni si hay uno disponible para esta pasada —eso es `arnes::sesion::EstadoMotor`—: dice qué
/// ficheros hay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotorInstalado {
    /// El ejecutable está y hay una biblioteca de CUDA junto a él.
    ListoConCuda,
    /// El ejecutable está, pero no se ve ninguna biblioteca de CUDA: descargar capas a la GPU no
    /// tendrá efecto, y el síntoma sería «va lentísimo» sin ninguna causa visible.
    SoloCpu,
    /// No está el ejecutable.
    Falta { ruta: PathBuf },
    /// La configuración no declara ningún motor. No lo devuelve `revisar_motor`, que solo mira el
    /// disco: lo fija quien lee la configuración y ve que no hay nada que mirar.
    NoDeclarado,
}

/// ¿Es `nombre`, el nombre de un fichero de la carpeta del motor, una biblioteca de CUDA de
/// `llama.cpp`? Función pura: decide sin tocar el disco.
fn es_biblioteca_de_cuda(nombre: &str) -> bool {
    // Se busca el nombre dentro y no al principio porque no se escribe igual en todas partes:
    // en Windows el fichero es «ggml-cuda.dll» y en Linux «libggml-cuda.so», con el prefijo
    // que esa plataforma le pone a sus bibliotecas. Buscando solo por el principio, una
    // instalacion de Linux perfectamente valida se anunciaba como «SIN CUDA (solo CPU)».
    nombre.to_ascii_lowercase().contains("ggml-cuda")
}

/// Mira qué hay instalado junto al ejecutable del motor.
///
/// **No comprueba las 55 bibliotecas que trae una distribución de `llama.cpp`**: eso ataría el
/// arnés a una versión concreta y se rompería con la siguiente. Comprueba lo que se puede afirmar
/// con certeza y cambia el comportamiento: que el ejecutable exista, y si hay CUDA a su lado.
pub fn revisar_motor(binario: &Path) -> MotorInstalado {
    if !binario.is_file() {
        return MotorInstalado::Falta {
            ruta: binario.to_path_buf(),
        };
    }

    let Some(carpeta) = binario.parent() else {
        return MotorInstalado::SoloCpu;
    };
    let Ok(entradas) = std::fs::read_dir(carpeta) else {
        // La carpeta no se deja listar: no se puede afirmar que haya CUDA, y decir que no la hay
        // es lo conservador.
        return MotorInstalado::SoloCpu;
    };

    let hay_cuda = entradas
        .flatten()
        .any(|entrada| es_biblioteca_de_cuda(&entrada.file_name().to_string_lossy()));
    if hay_cuda {
        MotorInstalado::ListoConCuda
    } else {
        MotorInstalado::SoloCpu
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_ejecutable_el_motor_falta() {
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");

        assert_eq!(
            revisar_motor(&binario),
            MotorInstalado::Falta { ruta: binario }
        );
    }

    #[test]
    fn con_ejecutable_y_sin_biblioteca_de_cuda_el_motor_es_solo_cpu() {
        // No es hipotético: el llama-server que instala Docker trae veinticuatro bibliotecas y
        // todas son ggml-cpu-*. Con él, --n-gpu-layers se acepta sin protestar y no carga una
        // sola capa.
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");
        std::fs::write(&binario, b"no importa el contenido").unwrap();
        std::fs::write(dir.path().join("ggml-cpu-haswell.dll"), b"x").unwrap();

        assert_eq!(revisar_motor(&binario), MotorInstalado::SoloCpu);
    }

    #[test]
    fn con_biblioteca_de_cuda_al_lado_el_motor_esta_listo() {
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");
        std::fs::write(&binario, b"x").unwrap();
        std::fs::write(dir.path().join("ggml-cuda.dll"), b"x").unwrap();

        assert_eq!(revisar_motor(&binario), MotorInstalado::ListoConCuda);
    }

    #[test]
    fn la_biblioteca_de_cuda_de_linux_tambien_cuenta() {
        // En Linux el fichero es «libggml-cuda.so», con el prefijo que esa plataforma pone a sus
        // bibliotecas. Buscando solo por el principio del nombre, una instalación de Linux
        // perfectamente válida se anunciaba como «SIN CUDA (solo CPU)»: se vio al instalar el
        // motor de verdad dentro de un Linux, y esta prueba existe para que no vuelva a pasar.
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server");
        std::fs::write(&binario, b"x").unwrap();
        std::fs::write(dir.path().join("libggml-cuda.so"), b"x").unwrap();

        assert_eq!(revisar_motor(&binario), MotorInstalado::ListoConCuda);
    }

    #[test]
    fn el_nombre_de_la_biblioteca_no_distingue_mayusculas() {
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");
        std::fs::write(&binario, b"x").unwrap();
        std::fs::write(dir.path().join("GGML-CUDA.DLL"), b"x").unwrap();

        assert_eq!(revisar_motor(&binario), MotorInstalado::ListoConCuda);
    }

    #[test]
    fn el_nombre_de_una_biblioteca_de_cuda_se_reconoce_sin_tocar_el_disco() {
        assert!(es_biblioteca_de_cuda("ggml-cuda.dll"));
        assert!(es_biblioteca_de_cuda("libggml-cuda.so"));
        assert!(es_biblioteca_de_cuda("GGML-CUDA.DLL"));
        assert!(!es_biblioteca_de_cuda("ggml-cpu-haswell.dll"));
    }

    #[test]
    fn una_ruta_que_es_un_directorio_no_es_el_ejecutable() {
        // `is_file()` distingue de verdad: una carpeta llamada como el ejecutable existe, pero no
        // se puede lanzar. Decir «falta» es lo correcto.
        let dir = tempfile::tempdir().unwrap();
        let binario = dir.path().join("llama-server.exe");
        std::fs::create_dir(&binario).unwrap();

        assert_eq!(
            revisar_motor(&binario),
            MotorInstalado::Falta { ruta: binario }
        );
    }
}
