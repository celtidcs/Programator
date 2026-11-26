//! Los resultados de una tanda: un TOML con todas las cifras y un informe en Markdown para leer.
//!
//! El TOML lleva lo necesario para repetir la tanda (modelo y su huella, motor, versión,
//! configuración, fecha) y, por cada encargo y semilla, la nota, el consumo y en las auditorías los
//! aciertos y los inventados por separado. También guarda, por cada fila de desempeño
//! (`FilaDeDesempeno`), la nota, la velocidad y los inventados: es lo que copia el registro de
//! modelos para saber en qué trabajo puntúa mejor cada modelo, sin volver a medir.
//!
//! Este módulo guarda el esquema del TOML y su paso a texto y vuelta. Lo demás va aparte, una cosa
//! por submódulo: las filas de desempeño (`filas`), las cuentas que se sacan de los encargos
//! (`estadisticas`), la redacción del informe (`redaccion`) y la capa fina que lee y escribe los
//! dos ficheros en el disco (`persistencia`), con [`rehacer_informe`].

mod estadisticas;
mod filas;
mod margen;
mod persistencia;
mod redaccion;

use serde::{Deserialize, Serialize};

use super::ficha::{ClaseDeUso, Familia};
use super::puntuacion::Nota;
use super::rubrica::NotaDeRubrica;
use crate::error::{Error, Resultado};

pub use estadisticas::{calcular_clases, nota_de_familia, resumir_encargos, FAMILIAS};
pub(crate) use estadisticas::{velocidad_de_generacion, velocidad_de_lectura};
pub use filas::{FilaDeDesempeno, FILAS_DE_DESEMPENO};
pub use persistencia::{
    guardar, leer_con_las_rubricas, rehacer_informe, NOMBRE_INFORME, NOMBRE_RESULTADOS,
};
pub(crate) use redaccion::{decimal, nombre_de_familia};

/// Todo lo de una tanda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultadosDeTanda {
    pub tanda: DatosDeTanda,
    /// Las filas de desempeño, en su orden. Se recalculan con
    /// [`calcular_clases`].
    #[serde(default)]
    pub clases: Vec<ResumenDeClase>,
    #[serde(default)]
    pub encargos: Vec<ResultadoDeEncargo>,
}

/// Lo que hace falta para repetir la tanda, y lo que costó cargar el modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatosDeTanda {
    pub modelo: String,
    pub huella_sha256: String,
    pub compilacion_motor: String,
    pub version_programator: String,
    pub fecha: String,
    pub semillas: Vec<i64>,
    pub temperatura: f64,
    pub margen_lineas: usize,
    pub tiempo_maximo_encargo_segundos: u64,
    /// Dónde se ejecutaron las pruebas ocultas: `local`, este equipo. Las tandas anteriores al
    /// 13/11/2026 pueden nombrar la máquina aparte que se usaba entonces. Sin ella, no se
    /// ejecutó código.
    pub maquina_de_comprobacion: Option<String>,
    pub capas_en_gpu: Option<u32>,
    pub vram_ocupada_mib: Option<u64>,
    /// Lo que el encaje calculaba que ocuparía esa carga, en MiB: con `vram_ocupada_mib`, da el
    /// margen que hace falta en esta máquina. Las tandas anteriores a la 1.2.0 no lo guardan.
    #[serde(default)]
    pub vram_calculada_mib: Option<u64>,
    pub segundos_de_carga: Option<f64>,
    /// Lo del motor que cambia la respuesta. Las tandas pasadas antes del 12/11/2026 no lo
    /// guardan.
    #[serde(default)]
    pub ajustes_del_motor: Option<AjustesDelMotor>,
    /// La tarjeta gráfica en que se midió, con el nombre que da el sistema. Con otra tarjeta el
    /// mismo modelo da otras cifras. Las tandas anteriores al 12/11/2026 no la guardan.
    #[serde(default)]
    pub tarjeta: Option<String>,
    /// El sistema operativo donde se comprobó, cuando se comprobó en este equipo: las notas de
    /// oficio en otro sistema pueden salir distintas. Las tandas anteriores al 12/11/2026 no lo
    /// guardan.
    #[serde(default)]
    pub sistema_de_comprobacion: Option<String>,
    /// Lo que la comprobación local dejó fuera de la carpeta de la tanda, ya redactado. Las tandas
    /// anteriores al 12/11/2026 no lo guardan.
    #[serde(default)]
    pub rastro_fuera: Vec<String>,
    /// Cómo se aisló la comprobación local, como se escribe en `banco.toml`. Las tandas
    /// anteriores al 12/11/2026 no lo guardan.
    #[serde(default)]
    pub aislamiento: Option<String>,
}

/// Lo de `programator.toml` que cambia lo que contesta el modelo, y que por eso tiene que
/// coincidir para que dos tandas sean repetición una de otra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AjustesDelMotor {
    pub contexto: u32,
    pub cache_kv: String,
    pub paralelo: u32,
    pub empezar_en_frio: bool,
    pub top_k: u32,
    pub top_p: f64,
    pub min_p: f64,
    pub penalizacion_repeticion: f64,
}

/// Un encargo, con todas sus semillas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultadoDeEncargo {
    pub id: String,
    /// El encargo con el que cuenta: el propio, o aquel del que es variante.
    pub cuenta_como: String,
    pub titulo: String,
    pub familia: Familia,
    pub clase: Option<ClaseDeUso>,
    #[serde(default)]
    pub semillas: Vec<ResultadoDeSemilla>,
}

/// En qué quedó la nota de una semilla.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoDeNota {
    Evaluada,
    /// No se pudo comprobar. Nunca cuenta como 0.
    #[default]
    NoEvaluado,
    /// La puntúa el agente que dirige y aún no lo ha hecho.
    Pendiente,
}

/// Un encargo pasado con una semilla. Los campos que no aplican a su forma de puntuar faltan.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResultadoDeSemilla {
    pub semilla: i64,
    pub estado: EstadoDeNota,
    pub nota: Option<f64>,
    /// Por qué no se evaluó, o lo que dijo la compilación que falló.
    pub detalle: Option<String>,
    /// El fichero de rúbrica, relativo a la carpeta de la tanda.
    pub rubrica: Option<String>,
    pub comprobaciones_superadas: Option<usize>,
    pub comprobaciones_total: Option<usize>,
    pub sembrados: Option<usize>,
    pub aciertos: Option<usize>,
    pub inventados: Option<usize>,
    pub repetidos: Option<usize>,
    pub sin_formato: Option<usize>,
    pub segundos: f64,
    pub tokens_entrada: Option<u64>,
    pub tokens_en_cache: Option<u64>,
    pub tokens_generados: Option<u64>,
    pub ms_entrada: Option<f64>,
    pub ms_generacion: Option<f64>,
    /// Respuestas del motor que no trajeron su consumo: si hay alguna, las cifras se quedan cortas.
    pub respuestas_sin_consumo: usize,
    /// Los destinos de red que intentó alcanzar el código del modelo al comprobarlo. Las tandas de
    /// antes del detector de red no lo traen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intentos_de_red: Vec<String>,
    /// Con qué se puntuó un encargo de código: sus pruebas ocultas, o su rúbrica leyendo el código
    /// porque las pruebas no dieron nota. Las dos notas no son comparables. Si no entregó, lo dice
    /// también. Falta en los demás encargos, en los de código que no se evaluaron y en las tandas
    /// de antes del 13/11/2026 (en estas, también en los que no entregaron).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub puntuada_con: Option<PuntuadaCon>,
}

/// Con qué se puntuó la semilla de un encargo de código.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PuntuadaCon {
    /// Ejecutando sus pruebas ocultas: una nota objetiva.
    PruebasOcultas,
    /// Con la rúbrica, leyendo el código sin ejecutarlo: la nota de quien dirige.
    Rubrica,
    /// No entregó a tiempo: un 0, el mismo con pruebas o con rúbrica, así que se compara con
    /// cualquiera de las dos.
    SinEntrega,
}

impl ResultadoDeSemilla {
    /// La nota para resumir; `None` si está pendiente.
    pub fn nota_para_resumir(&self) -> Option<Nota> {
        match (self.estado, self.nota) {
            (EstadoDeNota::Evaluada, Some(nota)) => Some(Nota::Evaluada(nota)),
            (EstadoDeNota::Pendiente, _) => None,
            _ => Some(Nota::NoEvaluado),
        }
    }

    /// Pone la nota que dice el fichero de rúbrica.
    pub(crate) fn aplicar_rubrica(&mut self, nota: NotaDeRubrica) {
        match nota {
            NotaDeRubrica::Puntuada(nota) => self.poner_nota(nota),
            NotaDeRubrica::Pendiente => self.dejar_pendiente(),
        }
    }

    /// Pone el estado y la nota juntos: solo una semilla evaluada lleva nota (auditoría de
    /// `5a68379`, C1). Es el único sitio, con `dejar_pendiente`, que los escribe.
    pub(crate) fn poner_nota(&mut self, nota: Nota) {
        (self.estado, self.nota) = match nota {
            Nota::Evaluada(valor) => (EstadoDeNota::Evaluada, Some(valor)),
            Nota::NoEvaluado => (EstadoDeNota::NoEvaluado, None),
        };
    }

    /// Deja la semilla a la espera de que quien dirige la puntúe, sin nota.
    pub(crate) fn dejar_pendiente(&mut self) {
        self.estado = EstadoDeNota::Pendiente;
        self.nota = None;
    }
}

/// Una fila de desempeño medida en esta tanda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumenDeClase {
    pub fila: FilaDeDesempeno,
    /// La media de las notas de sus encargos; falta si no hay ninguna o queda alguna pendiente.
    pub nota_media: Option<f64>,
    pub tokens_por_segundo: Option<f64>,
    pub inventados: usize,
    pub pendientes: usize,
}

impl ResultadosDeTanda {
    /// Cuántas semillas quedan por puntuar.
    pub fn pendientes(&self) -> usize {
        self.encargos
            .iter()
            .flat_map(|e| e.semillas.iter())
            .filter(|s| s.estado == EstadoDeNota::Pendiente)
            .count()
    }
}

pub fn componer_toml(resultados: &ResultadosDeTanda) -> Resultado<String> {
    toml::to_string(resultados)
        .map_err(|e| Error::Banco(super::mensajes::resultados_sin_componer(&e)))
}

pub fn leer_toml(texto: &str) -> Resultado<ResultadosDeTanda> {
    toml::from_str(texto).map_err(|e| Error::Banco(super::mensajes::resultados_ilegibles(&e)))
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;
    use crate::banco::rubrica::nombre_del_fichero;

    fn datos() -> DatosDeTanda {
        DatosDeTanda {
            modelo: "Devstral-Small-2-24B".into(),
            huella_sha256: "ab".repeat(32),
            compilacion_motor: "b10993".into(),
            version_programator: "0.11.0".into(),
            fecha: "2026-11-11".into(),
            semillas: vec![42, 43],
            temperatura: 0.2,
            margen_lineas: 2,
            tiempo_maximo_encargo_segundos: 900,
            maquina_de_comprobacion: None,
            capas_en_gpu: Some(41),
            vram_ocupada_mib: Some(13_100),
            vram_calculada_mib: None,
            segundos_de_carga: Some(21.5),
            tarjeta: None,
            sistema_de_comprobacion: None,
            rastro_fuera: Vec::new(),
            aislamiento: None,
            ajustes_del_motor: None,
        }
    }

    pub(super) fn semilla(
        semilla: i64,
        estado: EstadoDeNota,
        nota: Option<f64>,
    ) -> ResultadoDeSemilla {
        ResultadoDeSemilla {
            semilla,
            estado,
            nota,
            segundos: 120.0,
            tokens_generados: Some(500),
            ms_generacion: Some(25_000.0),
            ..ResultadoDeSemilla::default()
        }
    }

    pub(super) fn encargo(
        id: &str,
        familia: Familia,
        clase: Option<ClaseDeUso>,
    ) -> ResultadoDeEncargo {
        ResultadoDeEncargo {
            id: id.into(),
            cuenta_como: id.into(),
            titulo: format!("título de {id}"),
            familia,
            clase,
            semillas: Vec::new(),
        }
    }

    pub(crate) fn tanda_de_ejemplo() -> ResultadosDeTanda {
        let mut oficio = encargo("E03", Familia::Oficio, None);
        oficio.semillas = vec![
            ResultadoDeSemilla {
                comprobaciones_superadas: Some(7),
                comprobaciones_total: Some(10),
                ..semilla(42, EstadoDeNota::Evaluada, Some(3.5))
            },
            ResultadoDeSemilla {
                detalle: Some("falta en este equipo: rust".into()),
                ..semilla(43, EstadoDeNota::NoEvaluado, None)
            },
        ];
        let mut formas = encargo("U01", Familia::Uso, Some(ClaseDeUso::Formas));
        formas.semillas = vec![ResultadoDeSemilla {
            sembrados: Some(6),
            aciertos: Some(5),
            inventados: Some(1),
            repetidos: Some(0),
            sin_formato: Some(0),
            ..semilla(42, EstadoDeNota::Evaluada, Some(4.5))
        }];
        let mut criterio = encargo("E01", Familia::Criterio, None);
        criterio.semillas = vec![ResultadoDeSemilla {
            rubrica: Some(format!("rubricas/{}", nombre_del_fichero("E01", 42))),
            ..semilla(42, EstadoDeNota::Pendiente, None)
        }];
        let encargos = vec![oficio, formas, criterio];
        ResultadosDeTanda {
            tanda: datos(),
            clases: calcular_clases(&encargos),
            encargos,
        }
    }

    #[test]
    fn el_toml_compuesto_se_vuelve_a_leer_igual() {
        let resultados = tanda_de_ejemplo();

        let texto = componer_toml(&resultados).unwrap();

        assert_eq!(leer_toml(&texto).unwrap(), resultados);
    }

    #[test]
    fn la_tarjeta_se_guarda_y_una_tanda_antigua_sin_ella_se_sigue_leyendo() {
        let mut resultados = tanda_de_ejemplo();
        resultados.tanda.tarjeta = Some("NVIDIA GeForce RTX 4070 Ti SUPER".into());
        let texto = componer_toml(&resultados).unwrap();
        assert_eq!(leer_toml(&texto).unwrap(), resultados);

        let sin_tarjeta: String = texto
            .lines()
            .filter(|linea| !linea.starts_with("tarjeta"))
            .map(|linea| format!("{linea}\n"))
            .collect();
        assert_eq!(leer_toml(&sin_tarjeta).unwrap().tanda.tarjeta, None);
    }

    #[test]
    fn una_tanda_antigua_sin_sistema_ni_rastro_se_sigue_leyendo() {
        let mut resultados = tanda_de_ejemplo();
        resultados.tanda.sistema_de_comprobacion = Some("windows".into());
        resultados.tanda.rastro_fuera = vec!["C:/Users/x/.nuget/packages/algo".into()];
        let texto = componer_toml(&resultados).unwrap();
        assert_eq!(leer_toml(&texto).unwrap(), resultados);

        let antigua: String = texto
            .lines()
            .filter(|l| !l.starts_with("sistema_de_comprobacion") && !l.starts_with("rastro_fuera"))
            .map(|linea| format!("{linea}\n"))
            .collect();
        let leida = leer_toml(&antigua).unwrap();
        assert_eq!(leida.tanda.sistema_de_comprobacion, None);
        assert!(leida.tanda.rastro_fuera.is_empty());
    }

    #[test]
    fn con_que_se_puntuo_se_guarda_y_una_tanda_antigua_sin_el_dato_se_sigue_leyendo() {
        let mut resultados = tanda_de_ejemplo();
        assert!(!componer_toml(&resultados).unwrap().contains("puntuada_con"));

        resultados.encargos[0].semillas[0].puntuada_con = Some(PuntuadaCon::PruebasOcultas);
        resultados.encargos[0].semillas[1].puntuada_con = Some(PuntuadaCon::Rubrica);
        let texto = componer_toml(&resultados).unwrap();
        assert!(
            texto.contains("puntuada_con = \"pruebas_ocultas\""),
            "{texto}"
        );
        assert!(texto.contains("puntuada_con = \"rubrica\""), "{texto}");
        assert_eq!(leer_toml(&texto).unwrap(), resultados);

        let antigua: String = texto
            .lines()
            .filter(|linea| !linea.starts_with("puntuada_con"))
            .map(|linea| format!("{linea}\n"))
            .collect();
        let leida = leer_toml(&antigua).unwrap();
        assert!(leida
            .encargos
            .iter()
            .flat_map(|e| e.semillas.iter())
            .all(|s| s.puntuada_con.is_none()));
    }

    #[test]
    fn el_estado_y_la_nota_van_siempre_juntos() {
        let mut semilla = ResultadoDeSemilla {
            nota: Some(3.0),
            ..ResultadoDeSemilla::default()
        };
        semilla.poner_nota(Nota::Evaluada(7.5));
        assert_eq!(
            (semilla.estado, semilla.nota),
            (EstadoDeNota::Evaluada, Some(7.5))
        );
        semilla.poner_nota(Nota::NoEvaluado);
        assert_eq!(
            (semilla.estado, semilla.nota),
            (EstadoDeNota::NoEvaluado, None)
        );
        semilla.poner_nota(Nota::Evaluada(2.0));
        semilla.dejar_pendiente();
        assert_eq!(
            (semilla.estado, semilla.nota),
            (EstadoDeNota::Pendiente, None)
        );
    }
}
