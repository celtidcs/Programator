//! El recorrido de la fase 1: por cada semilla, todos los encargos, cada uno en su carpeta nueva.
//! Recibe el motor, el reloj y lo que anota cada pasada desde fuera, para probarlo con dobles.

use std::path::Path;

use super::Pasado;
use crate::arnes::sesion::{Ajustes, EstadoMotor};
use crate::banco::ejecucion::{
    carpeta_del_encargo, pasar_encargo, preparar_carpeta, Preparacion, Reloj,
};
use crate::banco::ficha::Encargo;
use crate::banco::mensajes;
use crate::error::Resultado;

/// Qué se pasa en la fase 1, y dónde.
pub struct Recorrido<'a, 'b> {
    pub encargos: &'a [Encargo],
    pub semillas: &'b [i64],
    /// Aquí va la carpeta de cada encargo y semilla.
    pub carpeta_de_trabajo: &'b Path,
}

/// Fase 1: por cada semilla, todos los encargos, cada uno en su carpeta nueva.
///
/// `asegurar` da el motor para una semilla: quien llama fija con ella el muestreo.
pub fn pasar_encargos<'a>(
    recorrido: &Recorrido<'a, '_>,
    preparacion: &Preparacion,
    ajustes: &Ajustes,
    asegurar: &mut dyn FnMut(i64) -> Resultado<EstadoMotor>,
    reloj: &mut dyn Reloj,
    avisar: &mut dyn FnMut(String),
    anotar: &mut dyn FnMut(&Pasado) -> Resultado<()>,
) -> Resultado<Vec<Pasado<'a>>> {
    let total = recorrido.encargos.len() * recorrido.semillas.len();
    let mut pasados = Vec::with_capacity(total);
    for &semilla in recorrido.semillas {
        for encargo in recorrido.encargos {
            let carpeta =
                carpeta_del_encargo(recorrido.carpeta_de_trabajo, &encargo.ficha.id, semilla);
            preparar_carpeta(&carpeta, encargo, preparacion)?;
            let mut asegurar_con_semilla = || asegurar(semilla);
            let recogida = pasar_encargo(
                &carpeta,
                encargo,
                preparacion,
                ajustes,
                &mut asegurar_con_semilla,
                reloj,
            )?;
            avisar(mensajes::mensaje_de_avance(
                pasados.len() + 1,
                total,
                &encargo.ficha.id,
                semilla,
                mensajes::Plazo::segun(recogida.a_tiempo),
                recogida.duracion,
                recogida.entregas.len(),
            ));
            let pasado = Pasado {
                encargo,
                semilla,
                carpeta,
                recogida,
            };
            anotar(&pasado)?;
            pasados.push(pasado);
        }
    }
    Ok(pasados)
}

#[cfg(test)]
mod pruebas {
    use super::super::dobles_de_prueba::{carpeta_tanda, criterio, datos, oficio, pasar, pedir};
    use crate::banco::pasadas;
    use crate::motor::Respuesta;

    #[test]
    fn cada_pasada_queda_en_disco_en_cuanto_termina() {
        let carpeta = tempfile::tempdir().unwrap();
        let encargos = vec![criterio(carpeta.path()), oficio(carpeta.path())];
        pasar(
            carpeta.path(),
            &encargos,
            vec![
                vec![Respuesta::Texto("El protocolo dice esto.".into())],
                vec![
                    pedir(
                        "escribir_propuesta",
                        serde_json::json!({"nombre": "rangos.rs", "contenido": "pub fn r() {}\n"}),
                    ),
                    pedir("publicar", serde_json::json!({"texto": "Entregado."})),
                ],
            ],
        );

        let guardadas = pasadas::cargar(&carpeta_tanda(carpeta.path())).unwrap();

        assert_eq!(guardadas.tanda, datos());
        assert_eq!(guardadas.pasadas.len(), 2);
        assert_eq!(guardadas.pasadas[1].encargo, "E03");
        assert_eq!(guardadas.pasadas[1].entregas, vec!["rangos.rs".to_string()]);
        assert!(guardadas.pasadas.iter().all(|p| p.a_tiempo));
        let texto = pasadas::componer_toml(&guardadas).unwrap();
        assert_eq!(pasadas::leer_toml(&texto).unwrap(), guardadas);
    }
}
