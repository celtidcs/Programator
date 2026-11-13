//! El registro de la conversación: lo que el arnés le dice al modelo y lo que el modelo contesta.
//!
//! **Por qué existe.** En la primera tanda del banco (11/11/2026) Devstral reescribió el mismo
//! fichero doce veces seguidas en trece de dieciocho encargos de código, sin llamar nunca a
//! `publicar`. En disco solo quedaban las doce versiones y el aborto: lo que el arnés le había
//! contestado en cada vuelta no lo guardaba nadie, y sin eso la causa no se podía averiguar.
//!
//! Va desactivado por defecto (`[agente] registro_de_conversacion`): la conversación lleva el
//! contenido de los ficheros del proyecto anfitrión y no tiene por qué quedarse en disco.
//!
//! **Nunca tumba un encargo.** Si no se puede escribir, se avisa una vez por la terminal y el
//! encargo sigue como si el registro no existiera.

use std::io::Write;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::error::Resultado;
use crate::motor::{proceso::EstadoServidor, Consumo, Mensaje, Motor, Papel, Respuesta};

/// Un motor que deja por escrito cada vuelta de la conversación antes de devolverla.
pub struct MotorRegistrado<'a> {
    interno: &'a mut dyn Motor,
    fichero: PathBuf,
    /// Cuántos mensajes de la conversación están ya en el fichero: en cada vuelta solo se
    /// escriben los nuevos, para no repetir las normas y el encargo en cada línea.
    escritos: usize,
    vuelta: u32,
    avisado: bool,
}

impl<'a> MotorRegistrado<'a> {
    pub fn nuevo(interno: &'a mut dyn Motor, fichero: PathBuf) -> Self {
        Self {
            interno,
            fichero,
            escritos: 0,
            vuelta: 0,
            avisado: false,
        }
    }

    fn anotar(&mut self, lineas: &[Value]) {
        if let Err(e) = anadir_lineas(&self.fichero, lineas) {
            if !self.avisado {
                eprintln!("{}", aviso_fallo_al_anotar(&self.fichero, &e));
                self.avisado = true;
            }
        }
    }
}

impl Motor for MotorRegistrado<'_> {
    fn responder(&mut self, conversacion: &[Mensaje]) -> Resultado<Respuesta> {
        let respuesta = self.interno.responder(conversacion);
        self.vuelta += 1;
        let lineas = lineas_de_la_vuelta(self.vuelta, conversacion, self.escritos, &respuesta);
        self.anotar(&lineas);
        self.escritos = conversacion.len();
        respuesta
    }

    fn comprobar_salud(&mut self) -> EstadoServidor {
        self.interno.comprobar_salud()
    }

    fn ultimo_consumo(&self) -> Option<Consumo> {
        self.interno.ultimo_consumo()
    }
}

/// Las líneas de una vuelta: los mensajes que no estaban en el fichero y la respuesta del modelo.
///
/// Si la conversación es más corta que lo ya escrito —no debería pasar, el ciclo solo añade— se
/// escribe entera, para que el registro no se salte nada en silencio.
pub fn lineas_de_la_vuelta(
    vuelta: u32,
    conversacion: &[Mensaje],
    escritos: usize,
    respuesta: &Resultado<Respuesta>,
) -> Vec<Value> {
    let desde = if escritos <= conversacion.len() {
        escritos
    } else {
        0
    };
    let mut lineas: Vec<Value> = conversacion[desde..]
        .iter()
        .map(|m| {
            let mut linea = json!({"vuelta": vuelta, "papel": nombre_del_papel(m.papel), "contenido": m.contenido});
            if let Some(llamada) = &m.llamada {
                linea["llamada"] = json!({"id": llamada.id, "nombre": llamada.nombre, "argumentos": llamada.argumentos});
            }
            linea
        })
        .collect();
    lineas.push(match respuesta {
        Ok(Respuesta::Texto(texto)) => {
            json!({"vuelta": vuelta, "papel": "respuesta", "texto": texto})
        }
        Ok(Respuesta::Cortada(texto)) => {
            json!({"vuelta": vuelta, "papel": "respuesta", "texto": texto, "cortada": true})
        }
        Ok(Respuesta::Herramienta(s)) => json!({
            "vuelta": vuelta,
            "papel": "respuesta",
            "herramienta": s.nombre,
            "argumentos": s.argumentos,
        }),
        Err(e) => json!({"vuelta": vuelta, "papel": "respuesta", "error": e.to_string()}),
    });
    lineas
}

fn nombre_del_papel(papel: Papel) -> &'static str {
    match papel {
        Papel::Sistema => "sistema",
        Papel::Usuario => "usuario",
        Papel::Modelo => "modelo",
        Papel::Herramienta => "herramienta",
    }
}

/// El aviso, una sola vez por encargo, de que el registro de la conversación no se pudo escribir.
fn aviso_fallo_al_anotar(fichero: &std::path::Path, fallo: &std::io::Error) -> String {
    format!(
        "⚠️ No se pudo escribir el registro de la conversación en «{}»: {fallo}. \
         El encargo sigue igual; solo falta el registro. Si no lo necesitas, desactiva \
         «registro_de_conversacion» en la sección [agente] del TOML.",
        fichero.display()
    )
}

fn anadir_lineas(fichero: &std::path::Path, lineas: &[Value]) -> std::io::Result<()> {
    if let Some(padre) = fichero.parent() {
        std::fs::create_dir_all(padre)?;
    }
    let mut abierto = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(fichero)?;
    for linea in lineas {
        writeln!(abierto, "{linea}")?;
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::{MotorDoble, SolicitudHerramienta};

    fn mensaje(papel: Papel, contenido: &str) -> Mensaje {
        Mensaje::nuevo(papel, contenido)
    }

    #[test]
    fn la_primera_vuelta_escribe_toda_la_conversacion_y_la_respuesta() {
        let conversacion = [
            mensaje(Papel::Sistema, "normas"),
            mensaje(Papel::Usuario, "encargo"),
        ];
        let lineas =
            lineas_de_la_vuelta(1, &conversacion, 0, &Ok(Respuesta::Texto("hecho".into())));
        assert_eq!(lineas.len(), 3);
        assert_eq!(lineas[0]["papel"], "sistema");
        assert_eq!(lineas[1]["contenido"], "encargo");
        assert_eq!(lineas[2]["texto"], "hecho");
    }

    #[test]
    fn las_vueltas_siguientes_solo_escriben_lo_nuevo() {
        let conversacion = [
            mensaje(Papel::Sistema, "normas"),
            mensaje(Papel::Usuario, "encargo"),
            mensaje(Papel::Modelo, "[solicito escribir_propuesta]"),
            mensaje(Papel::Usuario, "Resultado de «escribir_propuesta»"),
        ];
        let respuesta = Ok(Respuesta::Herramienta(SolicitudHerramienta {
            nombre: "publicar".into(),
            argumentos: json!({"texto": "listo"}),
        }));
        let lineas = lineas_de_la_vuelta(2, &conversacion, 2, &respuesta);
        assert_eq!(lineas.len(), 3);
        assert_eq!(lineas[0]["papel"], "modelo");
        assert_eq!(lineas[2]["herramienta"], "publicar");
        assert_eq!(lineas[2]["argumentos"]["texto"], "listo");
        assert!(lineas.iter().all(|l| l["vuelta"] == 2));
    }

    #[test]
    fn una_respuesta_cortada_queda_marcada_en_la_conversacion() {
        let conversacion = [mensaje(Papel::Usuario, "encargo")];
        let lineas = lineas_de_la_vuelta(1, &conversacion, 0, &Ok(Respuesta::Cortada("".into())));
        assert_eq!(lineas[1]["texto"], "");
        assert_eq!(lineas[1]["cortada"], true);
    }

    #[test]
    fn una_conversacion_mas_corta_que_lo_escrito_se_escribe_entera() {
        let conversacion = [mensaje(Papel::Usuario, "encargo")];
        let lineas = lineas_de_la_vuelta(3, &conversacion, 5, &Ok(Respuesta::Texto("x".into())));
        assert_eq!(lineas.len(), 2);
    }

    #[test]
    fn el_motor_registrado_deja_una_linea_por_mensaje_y_respuesta() {
        let dir = tempfile::tempdir().unwrap();
        let fichero = dir.path().join("sub").join("conversacion.jsonl");
        let mut doble = MotorDoble::con_guion(vec![
            Respuesta::Texto("primera".into()),
            Respuesta::Texto("segunda".into()),
        ]);
        let mut registrado = MotorRegistrado::nuevo(&mut doble, fichero.clone());
        let mut conversacion = vec![mensaje(Papel::Usuario, "encargo")];
        registrado.responder(&conversacion).unwrap();
        conversacion.push(mensaje(Papel::Modelo, "primera"));
        conversacion.push(mensaje(Papel::Usuario, "sigue"));
        registrado.responder(&conversacion).unwrap();

        let texto = std::fs::read_to_string(&fichero).unwrap();
        let lineas: Vec<Value> = texto
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        // Vuelta 1: encargo + respuesta. Vuelta 2: los dos mensajes nuevos + respuesta.
        assert_eq!(lineas.len(), 5);
        assert_eq!(lineas[1]["texto"], "primera");
        assert_eq!(lineas[4]["texto"], "segunda");
        assert_eq!(lineas[4]["vuelta"], 2);
    }

    #[test]
    fn si_no_se_puede_escribir_el_motor_sigue_respondiendo() {
        let dir = tempfile::tempdir().unwrap();
        // Un fichero donde debería ir la carpeta: create_dir_all falla.
        let estorbo = dir.path().join("estorbo");
        std::fs::write(&estorbo, "").unwrap();
        let mut doble = MotorDoble::con_guion(vec![Respuesta::Texto("hecho".into())]);
        let mut registrado = MotorRegistrado::nuevo(&mut doble, estorbo.join("conversacion.jsonl"));
        let respuesta = registrado.responder(&[mensaje(Papel::Usuario, "encargo")]);
        assert!(matches!(respuesta, Ok(Respuesta::Texto(ref t)) if t == "hecho"));
    }
}
