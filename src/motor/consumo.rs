//! Lo que cuesta cada respuesta del motor: tokens y tiempos, tal como los cuenta `llama-server`.
//!
//! Comprobado el 10/11/2026 en el código de `llama-server` b10993 (`tools/server/server-task.cpp`
//! y `server-common.cpp`): cada respuesta de `/v1/chat/completions` trae `usage` con
//! `prompt_tokens`, `completion_tokens` y `prompt_tokens_details.cached_tokens`, y, cuando el
//! servidor tiene estadísticas, `timings` con `prompt_n`, `prompt_ms`, `predicted_n`,
//! `predicted_ms` y `cache_n`. Todo es opcional: otro servidor puede no mandar nada, y entonces no
//! hay consumo, sin que eso sea un error.

use serde_json::Value;

/// Tokens y tiempos de una respuesta, o la suma de varias. Cada campo, si el servidor lo dio.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Consumo {
    /// Tokens de la conversación enviada, incluidos los que el servidor tenía en caché.
    pub tokens_entrada: Option<u64>,
    /// De los de entrada, los que no hubo que procesar porque estaban en caché.
    pub tokens_en_cache: Option<u64>,
    pub tokens_generados: Option<u64>,
    /// Milisegundos procesando la entrada.
    pub ms_entrada: Option<f64>,
    /// Milisegundos generando la respuesta.
    pub ms_generacion: Option<f64>,
}

impl Consumo {
    /// Suma campo a campo. Un campo que falta en uno de los dos vale lo que tenga el otro; quien
    /// necesite saber si el total está completo cuenta aparte las respuestas sin consumo.
    pub fn sumar(&self, otro: &Consumo) -> Consumo {
        fn suma<T: std::ops::Add<Output = T> + Copy>(a: Option<T>, b: Option<T>) -> Option<T> {
            match (a, b) {
                (Some(a), Some(b)) => Some(a + b),
                (a, None) => a,
                (None, b) => b,
            }
        }
        Consumo {
            tokens_entrada: suma(self.tokens_entrada, otro.tokens_entrada),
            tokens_en_cache: suma(self.tokens_en_cache, otro.tokens_en_cache),
            tokens_generados: suma(self.tokens_generados, otro.tokens_generados),
            ms_entrada: suma(self.ms_entrada, otro.ms_entrada),
            ms_generacion: suma(self.ms_generacion, otro.ms_generacion),
        }
    }
}

/// Lee `usage` y `timings` de una respuesta de la API. `None` si no trae ninguno de los dos.
pub fn interpretar_consumo(json: &Value) -> Option<Consumo> {
    let uso = json.get("usage");
    let tiempos = json.get("timings");
    if uso.is_none() && tiempos.is_none() {
        return None;
    }
    let entero = |origen: Option<&Value>, ruta: &[&str]| {
        ruta.iter()
            .try_fold(origen?, |valor, clave| valor.get(clave))
            .and_then(Value::as_u64)
    };
    let real = |clave: &str| tiempos.and_then(|t| t.get(clave)).and_then(Value::as_f64);
    Some(Consumo {
        tokens_entrada: entero(uso, &["prompt_tokens"]),
        tokens_en_cache: entero(uso, &["prompt_tokens_details", "cached_tokens"])
            .or_else(|| entero(tiempos, &["cache_n"])),
        tokens_generados: entero(uso, &["completion_tokens"])
            .or_else(|| entero(tiempos, &["predicted_n"])),
        ms_entrada: real("prompt_ms"),
        ms_generacion: real("predicted_ms"),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn una_respuesta_con_usage_y_timings_da_todos_los_campos() {
        let respuesta = json!({
            "choices": [{ "message": { "content": "hola" } }],
            "usage": {
                "prompt_tokens": 1200,
                "completion_tokens": 85,
                "total_tokens": 1285,
                "prompt_tokens_details": { "cached_tokens": 1000 }
            },
            "timings": {
                "cache_n": 1000,
                "prompt_n": 200,
                "prompt_ms": 150.5,
                "prompt_per_token_ms": 0.75,
                "prompt_per_second": 1328.9,
                "predicted_n": 85,
                "predicted_ms": 2100.25,
                "predicted_per_token_ms": 24.7,
                "predicted_per_second": 40.5
            }
        });

        let consumo = interpretar_consumo(&respuesta);

        assert_eq!(
            consumo,
            Some(Consumo {
                tokens_entrada: Some(1200),
                tokens_en_cache: Some(1000),
                tokens_generados: Some(85),
                ms_entrada: Some(150.5),
                ms_generacion: Some(2100.25),
            })
        );
    }

    #[test]
    fn sumar_junta_campo_a_campo_y_conserva_el_que_solo_tiene_uno() {
        let a = Consumo {
            tokens_generados: Some(5),
            ms_entrada: Some(1.5),
            ..Consumo::default()
        };
        let b = Consumo {
            tokens_generados: Some(7),
            tokens_entrada: Some(100),
            ..Consumo::default()
        };

        let total = a.sumar(&b);

        assert_eq!(total.tokens_generados, Some(12));
        assert_eq!(total.tokens_entrada, Some(100));
        assert_eq!(total.ms_entrada, Some(1.5));
        assert_eq!(total.ms_generacion, None);
    }

    #[test]
    fn una_respuesta_sin_usage_ni_timings_no_tiene_consumo() {
        let respuesta = json!({ "choices": [{ "message": { "content": "hola" } }] });

        assert_eq!(interpretar_consumo(&respuesta), None);
    }

    #[test]
    fn solo_usage_da_los_tokens_y_deja_los_tiempos_vacios() {
        let respuesta = json!({ "usage": { "prompt_tokens": 10, "completion_tokens": 3 } });

        let consumo = interpretar_consumo(&respuesta).unwrap();

        assert_eq!(consumo.tokens_entrada, Some(10));
        assert_eq!(consumo.tokens_generados, Some(3));
        assert_eq!(consumo.tokens_en_cache, None);
        assert_eq!(consumo.ms_generacion, None);
    }

    #[test]
    fn solo_timings_toma_los_tokens_de_timings() {
        let respuesta = json!({ "timings": { "cache_n": 4, "predicted_n": 7, "prompt_ms": 1.0 } });

        let consumo = interpretar_consumo(&respuesta).unwrap();

        assert_eq!(consumo.tokens_generados, Some(7));
        assert_eq!(consumo.tokens_en_cache, Some(4));
        assert_eq!(consumo.tokens_entrada, None);
        assert_eq!(consumo.ms_entrada, Some(1.0));
    }

    #[test]
    fn un_campo_con_un_tipo_inesperado_se_ignora_sin_error() {
        let respuesta = json!({ "usage": { "prompt_tokens": "muchos" } });

        assert_eq!(interpretar_consumo(&respuesta), Some(Consumo::default()));
    }
}
