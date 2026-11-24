//! El fichero del registro: qué se guarda de cada modelo medido, y cómo se lee y se escribe.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::banco::resultados::FilaDeDesempeno;
use crate::error::{Error, Resultado};
use crate::ficheros;
use crate::motor::encaje::MedidaDelBanco;

/// Las conversaciones en paralelo de una entrada que no las guarda. Las primeras versiones del
/// registro no las guardaban, y el encaje daba por hecho que el banco medía con una; `[motor]
/// paralelo` vale 1 de fábrica.
const PARALELO_SIN_ANOTAR: u32 = 1;

/// Todo el registro: una entrada por modelo y tarjeta.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Registro {
    #[serde(default)]
    pub modelo: Vec<EntradaDeModelo>,
}

/// Lo que el banco midió de un modelo en una tarjeta, y la tanda de la que sale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntradaDeModelo {
    /// El nombre del modelo: el del fichero GGUF sin carpeta ni extensión, como lo guarda la
    /// tanda. Es por lo que la guía busca la entrada.
    pub nombre: String,
    pub huella_sha256: String,
    pub tarjeta: String,
    pub compilacion_motor: String,
    pub version_programator: String,
    /// La fecha y hora de la tanda, tal como la guarda el banco.
    pub medido_el: String,
    /// El nombre de la carpeta de la tanda, dentro de los resultados del banco.
    pub tanda: String,
    pub semillas: Vec<i64>,
    pub capas_en_gpu: Option<u32>,
    pub contexto: u32,
    pub cache_kv: String,
    /// Las conversaciones en paralelo de la tanda. Opcional, para que los registros de antes del
    /// 23/11/2026, que no lo guardan, se lean: valen como [`PARALELO_SIN_ANOTAR`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paralelo: Option<u32>,
    pub vram_ocupada_mib: Option<u64>,
    pub generando_tokens_por_segundo: Option<f64>,
    pub leyendo_tokens_por_segundo: Option<f64>,
    pub familias: NotasDeFamilia,
    /// Una por fila de desempeño, en el orden de las filas.
    pub clase: Vec<NotaDeClase>,
    /// El día en que se retiró el modelo («AAAA-MM-DD»), si se retiró. El registro conserva sus
    /// cifras, pero la guía ya no lo ofrece. Opcional, para que los registros de antes se lean.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retirado_el: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo_de_retirada: Option<String>,
    /// Cómo se puntuaron los encargos de código de la tanda: ejecutando sus pruebas ocultas,
    /// leyéndolos con su rúbrica, o mezclado. Sin él, no se sabe: las entradas de antes del
    /// 13/11/2026 no lo traen, y la revisión no compara sus cifras de código con las de otras.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codigo_puntuado_con: Option<CodigoPuntuadoCon>,
    /// De dónde se descargó el fichero, para poder reponerlo si falta de la carpeta de modelos.
    /// Opcional: los registros de antes del 24/11/2026 no lo guardan, y la revisión lo completa
    /// por la huella cuando la encuentra en una búsqueda o en una lista de candidatos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origen: Option<Origen>,
}

/// De dónde salió el fichero de un modelo: el repositorio de Hugging Face y el nombre del fichero
/// allí, que no tiene por qué ser el de la carpeta de modelos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origen {
    /// El repositorio, como «autor/nombre».
    pub repositorio: String,
    /// El nombre del fichero dentro del repositorio.
    pub fichero: String,
    /// Los bytes del fichero, si se sabían al anotarlo. Sirve para desempatar sin tenerlo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tamano: Option<u64>,
}

/// Cómo se puntuaron los encargos de código de una tanda. Una nota de pruebas y una de rúbrica no
/// son comparables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodigoPuntuadoCon {
    /// Todas sus semillas de código, ejecutando las pruebas ocultas.
    PruebasOcultas,
    /// Todas, leyendo el código con la rúbrica.
    Rubrica,
    /// Unas de una forma y otras de la otra.
    Mixto,
    /// Ninguna semilla de código entregó: todas valen 0, se puntúe como se puntúe, y se comparan
    /// con cualquier forma.
    SinEntregas,
}

/// La nota media de cada familia del banco, sobre 5. Falta si no se pudo evaluar.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NotasDeFamilia {
    pub oficio: Option<f64>,
    pub criterio: Option<f64>,
    pub uso: Option<f64>,
}

/// Una fila de desempeño medida. Sin `nota`, la guía la da como «sin medir».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotaDeClase {
    pub clase: FilaDeDesempeno,
    pub nota: Option<f64>,
    pub inventados: usize,
    pub tokens_por_segundo: Option<f64>,
}

impl Registro {
    /// Lo que midió el banco de un modelo en una tarjeta, para que `capas_gpu = "auto"` lo use.
    /// Se busca por nombre, como la guía y la revisión: la huella exige leer el fichero entero.
    /// Si hay varias tandas, vale la más reciente; si no guardó capas o VRAM, no hay medida.
    pub fn medida_para(&self, nombre: &str, tarjeta: &str) -> Option<MedidaDelBanco> {
        let entrada = self
            .modelo
            .iter()
            .filter(|e| e.nombre == nombre && e.tarjeta == tarjeta)
            .max_by(|a, b| a.medido_el.cmp(&b.medido_el))?;
        Some(MedidaDelBanco {
            capas: entrada.capas_en_gpu?,
            vram_ocupada_mib: entrada.vram_ocupada_mib?,
            contexto: entrada.contexto,
            cache_kv: entrada.cache_kv.clone(),
            paralelo: entrada.paralelo.unwrap_or(PARALELO_SIN_ANOTAR),
        })
    }
}

impl EntradaDeModelo {
    /// Lo que identifica una entrada: el mismo modelo en otra tarjeta da otras cifras.
    pub fn misma_clave(&self, otra: &EntradaDeModelo) -> bool {
        self.huella_sha256 == otra.huella_sha256 && self.tarjeta == otra.tarjeta
    }
}

/// Lee el registro. Un fichero que no existe es un registro vacío: todavía no se ha anotado nada.
pub fn leer(ruta: &Path) -> Resultado<Registro> {
    let texto = ficheros::leer_si_existe(ruta)?;
    toml::from_str(&texto)
        .map_err(|causa| Error::Registro(super::mensajes::registro_que_no_se_lee(ruta, &causa)))
}

/// Escribe el registro entero.
pub fn escribir(ruta: &Path, registro: &Registro) -> Resultado<()> {
    let texto = toml::to_string(registro)
        .map_err(|causa| Error::Registro(super::mensajes::registro_sin_componer(&causa)))?;
    ficheros::escribir(ruta, &texto)
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;

    /// Una entrada completa, con las cifras de la tanda del 12/11/2026.
    pub(crate) fn entrada_de_ejemplo() -> EntradaDeModelo {
        EntradaDeModelo {
            nombre: "devstral-small-2-24b-Q4_K_M".into(),
            huella_sha256: "abc123".into(),
            tarjeta: "NVIDIA GeForce RTX 4070 Ti SUPER".into(),
            compilacion_motor: "b10993".into(),
            version_programator: "0.11.0".into(),
            medido_el: "2026-11-12 12:19".into(),
            tanda: "2026-11-12-1219-devstral-small-2-24b-Q4_K_M".into(),
            semillas: vec![42, 43, 44],
            capas_en_gpu: Some(34),
            contexto: 32768,
            cache_kv: "q8_0".into(),
            paralelo: None,
            vram_ocupada_mib: Some(13763),
            generando_tokens_por_segundo: Some(10.5),
            leyendo_tokens_por_segundo: Some(1013.9),
            familias: NotasDeFamilia {
                oficio: Some(1.8),
                criterio: Some(2.1),
                uso: Some(2.8),
            },
            clase: vec![
                NotaDeClase {
                    clase: FilaDeDesempeno::FuncionAcotada,
                    nota: Some(5.0),
                    inventados: 0,
                    tokens_por_segundo: Some(10.6),
                },
                NotaDeClase {
                    clase: FilaDeDesempeno::Diagnostico,
                    nota: None,
                    inventados: 0,
                    tokens_por_segundo: None,
                },
            ],
            retirado_el: None,
            motivo_de_retirada: None,
            codigo_puntuado_con: None,
            origen: None,
        }
    }

    #[test]
    fn la_medida_se_busca_por_nombre_y_tarjeta_y_vale_la_tanda_mas_reciente() {
        let vieja = entrada_de_ejemplo();
        let mut nueva = entrada_de_ejemplo();
        nueva.medido_el = "2026-11-20 10:00".into();
        nueva.capas_en_gpu = Some(36);
        let mut otra_tarjeta = entrada_de_ejemplo();
        otra_tarjeta.tarjeta = "otra".into();
        otra_tarjeta.capas_en_gpu = Some(1);
        let registro = Registro {
            modelo: vec![vieja.clone(), nueva, otra_tarjeta],
        };

        let medida = registro.medida_para(&vieja.nombre, &vieja.tarjeta).unwrap();

        assert_eq!(medida.capas, 36);
        assert_eq!(medida.vram_ocupada_mib, 13763);
        assert_eq!(medida.contexto, 32768);
        assert_eq!(medida.cache_kv, "q8_0");
        assert_eq!(registro.medida_para("otro-modelo", &vieja.tarjeta), None);
    }

    /// La medida lleva las conversaciones en paralelo con que se tomó; las
    /// entradas de antes, que no las guardan, cuentan como una.
    #[test]
    fn la_medida_lleva_las_conversaciones_en_paralelo_y_sin_anotar_son_una() {
        let antigua = entrada_de_ejemplo();
        let mut con_dos = entrada_de_ejemplo();
        con_dos.tarjeta = "otra".into();
        con_dos.paralelo = Some(2);
        let registro = Registro {
            modelo: vec![antigua.clone(), con_dos.clone()],
        };

        let de_la_antigua = registro.medida_para(&antigua.nombre, &antigua.tarjeta);
        let de_la_nueva = registro.medida_para(&con_dos.nombre, &con_dos.tarjeta);

        assert_eq!(de_la_antigua.map(|m| m.paralelo), Some(1));
        assert_eq!(de_la_nueva.map(|m| m.paralelo), Some(2));
    }

    #[test]
    fn una_entrada_sin_paralelo_se_escribe_sin_la_clave_y_se_lee_igual() {
        let antigua = Registro {
            modelo: vec![entrada_de_ejemplo()],
        };

        let texto = toml::to_string(&antigua).unwrap();
        let leida: Registro = toml::from_str(&texto).unwrap();

        assert!(!texto.contains("paralelo"), "{texto}");
        assert_eq!(leida, antigua);
    }

    #[test]
    fn sin_capas_o_sin_vram_no_hay_medida() {
        let mut sin_capas = entrada_de_ejemplo();
        sin_capas.capas_en_gpu = None;
        let mut sin_vram = entrada_de_ejemplo();
        sin_vram.vram_ocupada_mib = None;
        for entrada in [sin_capas, sin_vram] {
            let registro = Registro {
                modelo: vec![entrada.clone()],
            };
            assert_eq!(
                registro.medida_para(&entrada.nombre, &entrada.tarjeta),
                None
            );
        }
    }

    #[test]
    fn un_registro_sin_las_claves_de_la_retirada_se_lee_y_se_escribe_igual() {
        let registro = Registro {
            modelo: vec![entrada_de_ejemplo()],
        };
        let de_antes = toml::to_string(&registro).unwrap();
        assert!(!de_antes.contains("retirad"), "{de_antes}");

        let leido: Registro = toml::from_str(&de_antes).unwrap();
        assert_eq!(leido, registro);
        assert_eq!(toml::to_string(&leido).unwrap(), de_antes);
    }

    #[test]
    fn como_se_puntuo_el_codigo_se_guarda_y_una_entrada_antigua_sin_el_dato_se_lee() {
        let mut entrada = entrada_de_ejemplo();
        let antigua = toml::to_string(&Registro {
            modelo: vec![entrada.clone()],
        })
        .unwrap();
        assert!(!antigua.contains("codigo_puntuado_con"), "{antigua}");
        assert_eq!(
            toml::from_str::<Registro>(&antigua).unwrap().modelo[0].codigo_puntuado_con,
            None
        );

        entrada.codigo_puntuado_con = Some(CodigoPuntuadoCon::Rubrica);
        let registro = Registro {
            modelo: vec![entrada],
        };
        let texto = toml::to_string(&registro).unwrap();
        assert!(
            texto.contains("codigo_puntuado_con = \"rubrica\""),
            "{texto}"
        );
        assert_eq!(toml::from_str::<Registro>(&texto).unwrap(), registro);
    }

    /// Un registro de antes del 24/11/2026 no guarda de dónde salió cada modelo: se lee, y al
    /// escribirlo no gana ninguna clave.
    #[test]
    fn un_registro_sin_origen_se_lee_y_se_escribe_igual() {
        let registro = Registro {
            modelo: vec![entrada_de_ejemplo()],
        };
        let de_antes = toml::to_string(&registro).unwrap();
        assert!(!de_antes.contains("origen"), "{de_antes}");

        let leido: Registro = toml::from_str(&de_antes).unwrap();

        assert_eq!(leido.modelo[0].origen, None);
        assert_eq!(toml::to_string(&leido).unwrap(), de_antes);
    }

    #[test]
    fn el_origen_hace_ida_y_vuelta() {
        let mut entrada = entrada_de_ejemplo();
        entrada.origen = Some(Origen {
            repositorio: "unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF".into(),
            fichero: "Devstral-Small-2-24B-Instruct-2512-Q4_K_M.gguf".into(),
            tamano: Some(14_334_000_000),
        });
        let registro = Registro {
            modelo: vec![entrada],
        };

        let texto = toml::to_string(&registro).unwrap();

        assert!(texto.contains("[modelo.origen]"), "{texto}");
        assert_eq!(toml::from_str::<Registro>(&texto).unwrap(), registro);
    }

    #[test]
    fn la_retirada_se_guarda_y_se_vuelve_a_leer() {
        let mut entrada = entrada_de_ejemplo();
        entrada.retirado_el = Some("2026-12-01".into());
        entrada.motivo_de_retirada = Some("superado en funcion_acotada".into());
        let registro = Registro {
            modelo: vec![entrada],
        };

        let texto = toml::to_string(&registro).unwrap();
        assert_eq!(toml::from_str::<Registro>(&texto).unwrap(), registro);
    }

    #[test]
    fn el_registro_escrito_se_vuelve_a_leer_igual() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("registro-de-modelos.toml");
        let registro = Registro {
            modelo: vec![entrada_de_ejemplo()],
        };

        escribir(&ruta, &registro).unwrap();

        assert_eq!(leer(&ruta).unwrap(), registro);
    }

    #[test]
    fn un_registro_que_no_existe_esta_vacio() {
        let dir = tempfile::tempdir().unwrap();

        let registro = leer(&dir.path().join("no-existe.toml")).unwrap();

        assert!(registro.modelo.is_empty());
    }

    #[test]
    fn un_registro_roto_da_un_error_que_dice_la_ruta() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("registro-de-modelos.toml");
        std::fs::write(&ruta, "[[modelo]]\nnombre = 3\n").unwrap();

        let error = leer(&ruta).unwrap_err().to_string();

        assert!(error.contains("registro-de-modelos.toml"), "{error}");
    }

    #[test]
    fn la_clave_es_el_modelo_y_la_tarjeta() {
        let a = entrada_de_ejemplo();
        let mut otra_tanda = a.clone();
        otra_tanda.tanda = "otra".into();
        let mut otra_tarjeta = a.clone();
        otra_tarjeta.tarjeta = "otra tarjeta".into();

        assert!(a.misma_clave(&otra_tanda));
        assert!(!a.misma_clave(&otra_tarjeta));
    }
}
