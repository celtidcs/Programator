//! Las filas de desempeño: las clases de trabajo que mide el banco, con su nombre corto, su título
//! y las clases de encargo que entran en cada una. Las comparten el informe, el registro de modelos
//! y la guía, en el orden de [`FILAS_DE_DESEMPENO`].

use serde::{Deserialize, Serialize};

use crate::banco::ficha::ClaseDeUso;

/// Las filas de desempeño: las clases de encargo que el registro guarda y la guía cuenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilaDeDesempeno {
    Formas,
    /// Las auditorías con defectos sembrados, tomadas juntas.
    AuditoriasJuntas,
    FuncionAcotada,
    Validaciones,
    Diagnostico,
    FicheroLimpio,
}

/// Las filas, en el orden en que salen en el informe, el registro y la guía.
pub const FILAS_DE_DESEMPENO: [FilaDeDesempeno; 6] = [
    FilaDeDesempeno::Formas,
    FilaDeDesempeno::AuditoriasJuntas,
    FilaDeDesempeno::FuncionAcotada,
    FilaDeDesempeno::Validaciones,
    FilaDeDesempeno::Diagnostico,
    FilaDeDesempeno::FicheroLimpio,
];

impl FilaDeDesempeno {
    /// El nombre con que el agente que dirige pide esta clase en la línea `Clase:` de un encargo.
    /// Corto, sin tildes y sin espacios, para que escribirlo mal sea difícil.
    pub fn nombre_corto(self) -> &'static str {
        match self {
            Self::Formas => "formas",
            Self::AuditoriasJuntas => "auditorias",
            Self::FuncionAcotada => "funcion-acotada",
            Self::Validaciones => "validaciones",
            Self::Diagnostico => "diagnostico",
            Self::FicheroLimpio => "fichero-limpio",
        }
    }

    /// La fila cuyo nombre corto es `nombre`, sin distinguir mayúsculas ni tildes, y con espacios o
    /// guiones bajos donde va el guion. `None` si no es ninguna.
    pub fn por_nombre(nombre: &str) -> Option<Self> {
        let buscado = normalizar_nombre(nombre);
        FILAS_DE_DESEMPENO
            .into_iter()
            .find(|fila| fila.nombre_corto() == buscado)
    }

    /// Las clases de encargo que entran en la fila.
    pub fn clases(self) -> &'static [ClaseDeUso] {
        match self {
            Self::Formas => &[ClaseDeUso::Formas],
            Self::AuditoriasJuntas => &[ClaseDeUso::Formas, ClaseDeUso::Validaciones],
            Self::FuncionAcotada => &[ClaseDeUso::FuncionAcotada],
            Self::Validaciones => &[ClaseDeUso::Validaciones],
            Self::Diagnostico => &[ClaseDeUso::Diagnostico],
            Self::FicheroLimpio => &[ClaseDeUso::FicheroLimpio],
        }
    }

    /// El texto de la fila, el mismo en el informe y en la guía.
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Formas => "Auditar buscando formas",
            Self::AuditoriasJuntas => "Auditar código ajeno, tomando las auditorías juntas",
            Self::FuncionAcotada => "Función acotada",
            Self::Validaciones => "Auditar buscando validaciones ausentes o errores de significado",
            Self::Diagnostico => "Diagnosticar la causa de un fallo",
            Self::FicheroLimpio => "Auditar un fichero que crees limpio, para confirmarlo",
        }
    }
}

/// Deja un nombre de clase como se escribe su nombre corto: minúsculas, sin tildes y con guiones.
fn normalizar_nombre(nombre: &str) -> String {
    nombre
        .trim()
        .chars()
        .map(|letra| match letra {
            'á' | 'Á' => 'a',
            'é' | 'É' => 'e',
            'í' | 'Í' => 'i',
            'ó' | 'Ó' => 'o',
            'ú' | 'Ú' | 'ü' | 'Ü' => 'u',
            ' ' | '_' => '-',
            otra => otra.to_ascii_lowercase(),
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_fila_tiene_un_nombre_corto_distinto_que_la_encuentra() {
        let mut vistos = std::collections::HashSet::new();
        for fila in FILAS_DE_DESEMPENO {
            assert!(vistos.insert(fila.nombre_corto()), "{fila:?} repite nombre");
            assert_eq!(FilaDeDesempeno::por_nombre(fila.nombre_corto()), Some(fila));
        }
    }

    #[test]
    fn el_nombre_se_busca_sin_mayusculas_tildes_ni_espacios() {
        assert_eq!(
            FilaDeDesempeno::por_nombre("  Diagnóstico "),
            Some(FilaDeDesempeno::Diagnostico)
        );
        assert_eq!(
            FilaDeDesempeno::por_nombre("Función acotada"),
            Some(FilaDeDesempeno::FuncionAcotada)
        );
        assert_eq!(
            FilaDeDesempeno::por_nombre("FICHERO_LIMPIO"),
            Some(FilaDeDesempeno::FicheroLimpio)
        );
        assert_eq!(
            FilaDeDesempeno::por_nombre("Auditorías"),
            Some(FilaDeDesempeno::AuditoriasJuntas)
        );
    }

    #[test]
    fn un_nombre_que_no_es_de_ninguna_fila_no_se_encuentra() {
        assert_eq!(FilaDeDesempeno::por_nombre("traducir"), None);
        assert_eq!(FilaDeDesempeno::por_nombre(""), None);
    }
}
