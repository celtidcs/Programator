//! Registro de llamadas a herramientas atendidas para colapsar repeticiones idénticas.
//!
//! En la jornada del 23/09/2026 (incidencia B3 de NatureLand), el modelo repitió cinco
//! veces la misma llamada a «escribir_propuesta» con el mismo contenido exacto, consumiendo
//! el cupo de herramientas y abortando el encargo por agotamiento.
//!
//! Este módulo recuerda las llamadas concedidas previas y devuelve el resultado anterior
//! sin re-ejecutar ni consumir cupo, advirtiendo al modelo para que rectifique.

use crate::motor::SolicitudHerramienta;

/// Historial acotado de solicitudes concedidas durante un encargo.
#[derive(Debug, Clone)]
pub struct HistorialRepeticiones {
    tope: usize,
    entradas: Vec<EntradaHistorial>,
    mutaciones: usize,
}

#[derive(Debug, Clone)]
struct EntradaHistorial {
    nombre: String,
    argumentos: serde_json::Value,
    salida: String,
    mutacion_en_registro: usize,
}

impl HistorialRepeticiones {
    /// Crea un nuevo historial con un tope máximo de solicitudes recordadas.
    pub fn nuevo(tope: usize) -> Self {
        Self {
            tope,
            entradas: Vec::new(),
            mutaciones: 0,
        }
    }

    /// Comprueba si la solicitud actual es idéntica a una concedida previamente.
    ///
    /// Se consideran idénticas si coinciden tanto el nombre del verbo como todos sus argumentos.
    /// Para herramientas de lectura (`leer_fichero`, `listar`), si se ha producido una mutación
    /// en el disco (`escribir_propuesta`) después de registrar la lectura, la entrada previa
    /// se considera obsoleta y no se colapsa, permitiendo al modelo ver el estado actualizado.
    pub fn buscar_repeticion(&self, solicitud: &SolicitudHerramienta) -> Option<&str> {
        if self.tope == 0 {
            return None;
        }

        // Buscar desde la más reciente hacia la más antigua
        for entrada in self.entradas.iter().rev() {
            if entrada.nombre == solicitud.nombre && entrada.argumentos == solicitud.argumentos {
                if es_lectura(&solicitud.nombre) && entrada.mutacion_en_registro < self.mutaciones {
                    // El disco pudo haber cambiado desde esta lectura: no colapsar
                    return None;
                }
                return Some(entrada.salida.as_str());
            }
        }
        None
    }

    /// Registra una solicitud concedida en el historial.
    pub fn registrar(&mut self, solicitud: &SolicitudHerramienta, salida: String) {
        if self.tope == 0 {
            return;
        }

        if es_mutacion(&solicitud.nombre) {
            self.mutaciones += 1;
        }

        if self.entradas.len() >= self.tope {
            self.entradas.remove(0);
        }

        self.entradas.push(EntradaHistorial {
            nombre: solicitud.nombre.clone(),
            argumentos: solicitud.argumentos.clone(),
            salida,
            mutacion_en_registro: self.mutaciones,
        });
    }
}

/// Determina si un verbo es una operación pura de lectura cuyos resultados dependen del estado del disco.
fn es_lectura(nombre: &str) -> bool {
    matches!(nombre, "leer_fichero" | "listar")
}

/// Determina si un verbo introduce cambios en el disco que invaliden lecturas previas.
fn es_mutacion(nombre: &str) -> bool {
    matches!(nombre, "escribir_propuesta")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn solicitud(nombre: &str, args: serde_json::Value) -> SolicitudHerramienta {
        SolicitudHerramienta {
            nombre: nombre.to_string(),
            argumentos: args,
        }
    }

    #[test]
    fn detecta_llamada_identica_con_mismo_nombre_y_argumentos() {
        let mut historial = HistorialRepeticiones::nuevo(10);
        let s = solicitud(
            "escribir_propuesta",
            serde_json::json!({"nombre": "calc.rs", "contenido": "fn f(){}"}),
        );
        historial.registrar(&s, "escrita la propuesta «calc.rs»".to_string());

        let encontrada = historial.buscar_repeticion(&s);
        assert_eq!(encontrada, Some("escrita la propuesta «calc.rs»"));
    }

    #[test]
    fn no_colapsa_llamada_con_mismo_nombre_y_distintos_argumentos() {
        let mut historial = HistorialRepeticiones::nuevo(10);
        let s1 = solicitud(
            "escribir_propuesta",
            serde_json::json!({"nombre": "calc.rs", "contenido": "v1"}),
        );
        historial.registrar(&s1, "escrita v1".to_string());

        let s2 = solicitud(
            "escribir_propuesta",
            serde_json::json!({"nombre": "calc.rs", "contenido": "v2"}),
        );
        assert_eq!(historial.buscar_repeticion(&s2), None);
    }

    #[test]
    fn no_colapsa_llamada_con_distinto_nombre_y_mismos_argumentos() {
        let mut historial = HistorialRepeticiones::nuevo(10);
        let s1 = solicitud("leer_fichero", serde_json::json!({"ruta": "a.txt"}));
        historial.registrar(&s1, "contenido a".to_string());

        let s2 = solicitud("listar", serde_json::json!({"ruta": "a.txt"}));
        assert_eq!(historial.buscar_repeticion(&s2), None);
    }

    #[test]
    fn una_mutacion_invalida_la_cache_de_lecturas_previas() {
        let mut historial = HistorialRepeticiones::nuevo(10);
        let s_leer = solicitud("leer_fichero", serde_json::json!({"ruta": "salida.txt"}));
        historial.registrar(&s_leer, "v1".to_string());

        // Antes de mutación: se colapsa
        assert_eq!(historial.buscar_repeticion(&s_leer), Some("v1"));

        // Mutación intermedia
        let s_escribir = solicitud(
            "escribir_propuesta",
            serde_json::json!({"nombre": "p.rs", "contenido": "code"}),
        );
        historial.registrar(&s_escribir, "escrita".to_string());

        // Después de mutación: la lectura se invalida y no se colapsa
        assert_eq!(historial.buscar_repeticion(&s_leer), None);
    }

    #[test]
    fn una_mutacion_no_invalida_el_colapso_de_otra_propuesta_identica() {
        let mut historial = HistorialRepeticiones::nuevo(10);
        let s1 = solicitud(
            "escribir_propuesta",
            serde_json::json!({"nombre": "a.rs", "contenido": "ca"}),
        );
        historial.registrar(&s1, "escrita a".to_string());

        let s2 = solicitud(
            "escribir_propuesta",
            serde_json::json!({"nombre": "b.rs", "contenido": "cb"}),
        );
        historial.registrar(&s2, "escrita b".to_string());

        // Repetir s1 con argumentos idénticos se colapsa incluso tras registrar s2
        assert_eq!(historial.buscar_repeticion(&s1), Some("escrita a"));
    }

    #[test]
    fn respeta_el_tope_maximo_expulsando_la_mas_antigua() {
        let mut historial = HistorialRepeticiones::nuevo(2);
        let s1 = solicitud("leer_fichero", serde_json::json!({"ruta": "1.txt"}));
        let s2 = solicitud("leer_fichero", serde_json::json!({"ruta": "2.txt"}));
        let s3 = solicitud("leer_fichero", serde_json::json!({"ruta": "3.txt"}));

        historial.registrar(&s1, "1".to_string());
        historial.registrar(&s2, "2".to_string());
        historial.registrar(&s3, "3".to_string());

        // s1 fue expulsada por el tope de 2
        assert_eq!(historial.buscar_repeticion(&s1), None);
        assert_eq!(historial.buscar_repeticion(&s2), Some("2"));
        assert_eq!(historial.buscar_repeticion(&s3), Some("3"));
    }

    #[test]
    fn tope_cero_desactiva_el_historial() {
        let mut historial = HistorialRepeticiones::nuevo(0);
        let s = solicitud("leer_fichero", serde_json::json!({"ruta": "a.txt"}));
        historial.registrar(&s, "a".to_string());
        assert_eq!(historial.buscar_repeticion(&s), None);
    }
}
