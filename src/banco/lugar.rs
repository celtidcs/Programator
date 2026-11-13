//! Dónde se ejecutan las pruebas ocultas de una tanda: en una carpeta de este equipo o en ninguna
//! parte.
//!
//! Programator solo usa el equipo donde corre, y ejecutar ahí es para lo que nació: no se pide
//! autorización. Lo decide `banco.toml`: se comprueba aquí
//! salvo que `[comprobacion_local]` diga `activa = false`.

use std::path::Path;

use super::config::{ComprobacionLocal, ConfigBanco, ModoDeComprobacion, MAQUINA_LOCAL};
use super::ficha::{Encargo, FormaDePuntuar};

/// El lugar elegido para las pruebas ocultas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LugarElegido<'b> {
    /// No se ejecuta código del modelo: las pruebas ocultas están apagadas, o ningún encargo las
    /// necesita.
    Ninguno,
    Local(&'b ComprobacionLocal),
}

impl LugarElegido<'_> {
    /// Lo que se guarda en `maquina_de_comprobacion`.
    pub fn nombre(&self) -> Option<String> {
        match self {
            Self::Ninguno => None,
            Self::Local(_) => Some(MAQUINA_LOCAL.into()),
        }
    }

    /// Lo que se guarda en `sistema_de_comprobacion`: el de este equipo, si se comprueba.
    pub fn sistema(&self, sistema_de_este_equipo: &str) -> Option<String> {
        match self {
            Self::Local(_) => Some(sistema_de_este_equipo.into()),
            Self::Ninguno => None,
        }
    }

    /// Lo que se guarda en `aislamiento`.
    pub fn aislamiento(&self) -> Option<String> {
        match self {
            Self::Local(local) => Some(local.aislamiento.nombre().into()),
            Self::Ninguno => None,
        }
    }

    /// La carpeta donde se comprobará, para contarlo al empezar.
    pub fn para_contar<'c>(&self, carpeta_de_comprobacion: &'c Path) -> Option<&'c Path> {
        match self {
            Self::Ninguno => None,
            Self::Local(_) => Some(carpeta_de_comprobacion),
        }
    }
}

pub fn hay_codigo_que_comprobar(encargos: &[Encargo]) -> bool {
    encargos
        .iter()
        .any(|e| e.ficha.forma_de_puntuar() == Some(FormaDePuntuar::Comprobaciones))
}

/// El lugar que dice `banco.toml`, si hay código que comprobar.
pub fn elegir_lugar<'b>(banco: &'b ConfigBanco, encargos: &[Encargo]) -> LugarElegido<'b> {
    if !hay_codigo_que_comprobar(encargos) {
        return LugarElegido::Ninguno;
    }
    match banco.modo_de_comprobacion() {
        ModoDeComprobacion::NoEjecutaCodigo => LugarElegido::Ninguno,
        ModoDeComprobacion::Local(local) => LugarElegido::Local(local),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::PathBuf;

    const OFICIO: &str = r#"
id = "E03"
titulo = "Rust"
familia = "oficio"
entrega = "rangos.rs"
colocar_en = "src/rangos.rs"
herramientas = ["rust"]
[[comprobaciones]]
nombre = "caso"
orden = "cargo test caso"
"#;

    const CRITERIO: &str = r#"
id = "E01"
titulo = "Protocolo"
familia = "criterio"
"#;

    const LOCAL: &str = r#"
[comprobacion_local]
"#;

    fn encargo(ficha: &str) -> Encargo {
        Encargo {
            ficha: toml::from_str(ficha).unwrap(),
            carpeta: PathBuf::new(),
        }
    }

    fn banco(texto: &str) -> ConfigBanco {
        ConfigBanco::desde_texto(texto).unwrap()
    }

    #[test]
    fn con_comprobacion_local_se_comprueba_en_este_equipo_sin_pedir_nada() {
        let banco = banco(LOCAL);

        let lugar = elegir_lugar(&banco, &[encargo(OFICIO)]);

        assert!(matches!(lugar, LugarElegido::Local(_)));
        assert_eq!(lugar.nombre().as_deref(), Some("local"));
        assert_eq!(lugar.sistema("windows").as_deref(), Some("windows"));
        let carpeta = Path::new("tanda/comprobacion");
        assert_eq!(lugar.para_contar(carpeta), Some(carpeta));
    }

    #[test]
    fn sin_codigo_que_comprobar_no_hace_falta_lugar() {
        for texto in [LOCAL, ""] {
            let banco = banco(texto);

            let lugar = elegir_lugar(&banco, &[encargo(CRITERIO)]);

            assert_eq!(lugar, LugarElegido::Ninguno);
            assert_eq!(lugar.nombre(), None);
            assert_eq!(lugar.para_contar(Path::new("x")), None);
        }
    }

    #[test]
    fn sin_seccion_de_comprobacion_se_comprueba_en_este_equipo() {
        // Las pruebas ocultas van encendidas por defecto.
        let banco = banco("");

        let lugar = elegir_lugar(&banco, &[encargo(OFICIO)]);

        assert!(matches!(lugar, LugarElegido::Local(_)), "{lugar:?}");
    }

    #[test]
    fn con_las_pruebas_ocultas_apagadas_no_se_ejecuta_codigo() {
        let banco = banco("[comprobacion_local]\nactiva = false\n");

        let lugar = elegir_lugar(&banco, &[encargo(OFICIO)]);

        assert_eq!(lugar, LugarElegido::Ninguno);
    }
}
