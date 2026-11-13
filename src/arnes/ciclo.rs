//! El ciclo de un encargo: solicitar, conceder o denegar, y parar cuando toca.
//!
//! Los límites no son sugerencias al modelo: son condiciones del arnés. La Regla de los Dos Intentos
//! del §2.1 y el registro obligatorio en parada del §2.3 dejan de depender de que el modelo se
//! acuerde de ellos.

use super::herramientas::{Decision, Repertorio};
use crate::motor::{Llamada, Mensaje, Motor, Papel, Respuesta, SolicitudHerramienta};

mod mensajes;
mod plazo;
mod reintentos;
mod repeticiones;
use mensajes::estado_entrega_propuestas;
pub use plazo::Plazo;
pub use reintentos::{evaluar_reintento, DecisionReintento};
pub use repeticiones::HistorialRepeticiones;

/// Cómo terminó un encargo.
#[derive(Debug)]
pub enum Desenlace {
    /// El modelo entregó un cuerpo para publicar.
    Publicado(String),
    /// El modelo terminó de hablar sin entregar nada: ni propuestas ni `publicar`.
    SinEntrega(String),
    /// El modelo escribió propuestas en este encargo y cerró con texto sin llamar a `publicar`.
    /// No es lo mismo que `SinEntrega`: decir que «no invocó herramientas de entrega» cuando hay
    /// propuestas en disco es el arnés contando mal lo que hizo (tanda del banco del 11/11/2026).
    SinPublicar {
        texto: String,
        propuestas: Vec<String>,
    },
    /// El motor cortó la respuesta del modelo antes de que terminara: se acabó el contexto o el
    /// tope de tokens. Lleva lo que llegó a escribir y las propuestas que dejó en este encargo.
    Cortado {
        texto: String,
        propuestas: Vec<String>,
    },
    /// El arnés cortó el ciclo. El motivo se publica tal cual.
    Abortado(String),
    /// Se acabó el plazo del encargo antes de que el modelo cerrara. El motivo, que nombra lo que
    /// quedó en disco, se publica tal cual.
    FueraDePlazo(String),
}

/// Topes que impone el arnés.
#[derive(Debug, Clone)]
pub struct Limites {
    pub max_herramientas: u32,
    pub max_denegaciones_seguidas: u32,
    pub max_repeticiones_recordadas: usize,
    pub max_reintentos_motor: u32,
    pub espera_reintento_motor: std::time::Duration,
    /// A partir de cuándo no se vuelve a llamar al motor. Lo pone el banco, con su tiempo máximo
    /// por encargo; en uso normal va vacío.
    pub plazo: Option<Plazo>,
}

impl Default for Limites {
    fn default() -> Self {
        Self {
            max_herramientas: crate::config::max_herramientas_por_defecto(),
            max_denegaciones_seguidas: crate::config::max_denegaciones_seguidas_por_defecto(),
            max_repeticiones_recordadas: crate::config::max_repeticiones_recordadas_por_defecto(),
            max_reintentos_motor: crate::config::max_reintentos_motor_por_defecto(),
            espera_reintento_motor: std::time::Duration::from_millis(0),
            plazo: None,
        }
    }
}

impl Limites {
    /// Los límites tal como los fija `[ciclo]` de la configuración.
    pub fn de_la_config(ciclo: &crate::config::Ciclo) -> Self {
        Self {
            max_herramientas: ciclo.max_herramientas_por_encargo,
            max_denegaciones_seguidas: ciclo.max_denegaciones_seguidas,
            max_repeticiones_recordadas: ciclo.max_repeticiones_recordadas,
            max_reintentos_motor: ciclo.max_reintentos_motor,
            espera_reintento_motor: std::time::Duration::from_secs(
                ciclo.espera_reintento_motor_segundos,
            ),
            plazo: None,
        }
    }
}

/// La etiqueta con que el arnés anotaba en el historial, hasta el 12/11/2026, cada llamada ya
/// resuelta. El historial ya no la usa: las llamadas van enteras, como llamadas de verdad. Se
/// sigue vigilando porque un modelo que la aprendió puede seguir escribiéndola como texto en vez de
/// llamar a la herramienta (visto en uso real).
const PREFIJO_ETIQUETA_SOLICITUD: &str = "[solicito ";

/// Cuántos caracteres lleva el identificador de una llamada. La plantilla de chat de los modelos
/// Mistral, que es la de Devstral, exige identificadores alfanuméricos de exactamente nueve.
const LONGITUD_ID_LLAMADA: usize = 9;
/// El principio fijo del identificador; el resto es el número de la llamada en el encargo.
const PREFIJO_ID_LLAMADA: &str = "ll";

/// La llamada tal como queda en el historial, con un identificador propio del arnés.
///
/// El identificador lo pone el arnés y no el servidor: así es el mismo en cada ejecución, y no
/// depende de que el servidor mande uno ni de cómo lo forme.
fn llamada_hecha(contador: &mut u32, solicitud: &SolicitudHerramienta) -> Llamada {
    *contador += 1;
    let ancho = LONGITUD_ID_LLAMADA - PREFIJO_ID_LLAMADA.len();
    Llamada {
        id: format!("{PREFIJO_ID_LLAMADA}{:0ancho$}", *contador),
        nombre: solicitud.nombre.clone(),
        argumentos: solicitud.argumentos.clone(),
    }
}

/// Ejecuta un encargo de principio a fin.
pub fn atender_encargo(
    motor: &mut dyn Motor,
    repertorio: &mut Repertorio,
    conversacion_inicial: Vec<Mensaje>,
    limites: &Limites,
) -> Desenlace {
    let mut en_curso = EnCurso::nuevo(repertorio, conversacion_inicial, limites);
    loop {
        let respuesta = match en_curso.pedir_respuesta(motor) {
            Ok(respuesta) => respuesta,
            Err(desenlace) => return desenlace,
        };
        let desenlace = match respuesta {
            Respuesta::Texto(texto) => en_curso.atender_texto(texto),
            Respuesta::Cortada(texto) => Some(en_curso.atender_cortada(texto)),
            Respuesta::Herramienta(solicitud) => en_curso.atender_herramienta(solicitud),
        };
        if let Some(desenlace) = desenlace {
            return desenlace;
        }
    }
}

/// Lo que el ciclo cuenta mientras dura un encargo, para saber cuándo cortarlo.
#[derive(Default)]
struct Cuentas {
    /// Solicitudes que gastaron cupo (las repetidas no lo gastan).
    usadas: u32,
    /// Denegaciones seguidas de la misma herramienta.
    denegaciones_seguidas: u32,
    ultima_denegada: Option<String>,
    /// Veces seguidas que el modelo escribió una etiqueta como texto en vez de llamar.
    etiquetas_como_texto: u32,
    /// Llamadas que quedan en el historial, para numerar su identificador.
    llamadas_hechas: u32,
}

impl Cuentas {
    /// Una llamada que salió bien, concedida o repetida: el modelo ni insiste en una denegada ni
    /// escribe etiquetas, así que esas dos cuentas vuelven a cero.
    fn llamada_buena(&mut self) {
        self.denegaciones_seguidas = 0;
        self.ultima_denegada = None;
        self.etiquetas_como_texto = 0;
    }

    /// Una denegación. Es, igualmente, una llamada estructurada de verdad —el modelo ya ha
    /// demostrado saber invocar la herramienta, aunque con un argumento que no cuadra—, así que
    /// las etiquetas vuelven a cero; y solo cuenta como seguida si es la misma herramienta.
    /// Devuelve cuántas van seguidas.
    fn denegada(&mut self, nombre: &str) -> u32 {
        self.etiquetas_como_texto = 0;
        if self.ultima_denegada.as_deref() == Some(nombre) {
            self.denegaciones_seguidas += 1;
        } else {
            self.denegaciones_seguidas = 1;
            self.ultima_denegada = Some(nombre.to_string());
        }
        self.denegaciones_seguidas
    }

    /// Una etiqueta escrita como texto. Devuelve cuántas van seguidas.
    fn etiqueta_como_texto(&mut self) -> u32 {
        self.etiquetas_como_texto += 1;
        self.etiquetas_como_texto
    }

    /// Una solicitud que gasta cupo. Devuelve cuántas van.
    fn gastar_cupo(&mut self) -> u32 {
        self.usadas += 1;
        self.usadas
    }
}

/// Un encargo mientras se atiende: la conversación, lo que se lleva contado y desde dónde son
/// suyas las propuestas del repertorio.
struct EnCurso<'a> {
    repertorio: &'a mut Repertorio,
    conversacion: Vec<Mensaje>,
    limites: &'a Limites,
    historial: HistorialRepeticiones,
    cuentas: Cuentas,
    /// El repertorio es de toda la pasada: lo que escribieron los encargos anteriores no es de este.
    propuestas_al_empezar: usize,
}

impl<'a> EnCurso<'a> {
    fn nuevo(
        repertorio: &'a mut Repertorio,
        conversacion: Vec<Mensaje>,
        limites: &'a Limites,
    ) -> Self {
        let propuestas_al_empezar = repertorio.propuestas().len();
        Self {
            repertorio,
            conversacion,
            limites,
            historial: HistorialRepeticiones::nuevo(limites.max_repeticiones_recordadas),
            cuentas: Cuentas::default(),
            propuestas_al_empezar,
        }
    }

    /// Lo que este encargo dejó en disco, para el motivo de un aborto.
    fn detalle_de_entrega(&self) -> String {
        detalle_de_entrega(self.repertorio, self.propuestas_al_empezar)
    }

    /// Las propuestas que escribió este encargo.
    fn propuestas(&self) -> Vec<String> {
        propuestas_del_encargo(self.repertorio, self.propuestas_al_empezar)
    }

    /// Lo que entregó el modelo si llamó a `publicar`.
    fn publicado(&mut self) -> Option<Desenlace> {
        self.repertorio
            .tomar_pendiente_de_publicar()
            .map(Desenlace::Publicado)
    }

    /// La siguiente respuesta del motor, reintentando lo que diga `evaluar_reintento`. Si se acaba
    /// el plazo o se agotan los reintentos, el desenlace con que se corta el encargo.
    fn pedir_respuesta(&mut self, motor: &mut dyn Motor) -> Result<Respuesta, Desenlace> {
        let mut reintentos_motor = 0u32;
        loop {
            // Antes de cada llamada, también de los reintentos: una respuesta del motor puede
            // tardar minutos, y es ahí donde se va el tiempo.
            if self.limites.plazo.as_ref().is_some_and(Plazo::vencido) {
                return Err(Desenlace::FueraDePlazo(mensajes::fuera_de_plazo(
                    &self.detalle_de_entrega(),
                )));
            }
            let fallo = match motor.responder(&self.conversacion) {
                Ok(respuesta) => return Ok(respuesta),
                Err(fallo) => fallo,
            };
            let salud = motor.comprobar_salud();
            match evaluar_reintento(
                reintentos_motor,
                self.limites.max_reintentos_motor,
                salud,
                &fallo,
                self.limites.espera_reintento_motor,
            ) {
                DecisionReintento::Reintentar {
                    intento_siguiente,
                    espera,
                } => {
                    reintentos_motor = intento_siguiente;
                    if !espera.is_zero() {
                        std::thread::sleep(espera);
                    }
                }
                DecisionReintento::Abortar(motivo) => {
                    return Err(Desenlace::Abortado(mensajes::abortado_por_el_motor(
                        &motivo,
                        &self.detalle_de_entrega(),
                    )));
                }
            }
        }
    }

    /// Una respuesta en texto: cierra el encargo, salvo que sea una etiqueta escrita a mano, que
    /// se le corrige al modelo y se sigue (`None`).
    fn atender_texto(&mut self, texto: String) -> Option<Desenlace> {
        if let Some(publicado) = self.publicado() {
            return Some(publicado);
        }

        // El modelo escribe la etiqueta de una llamada a herramienta como texto plano en vez de
        // invocarla de verdad (visto en uso real): imita el rótulo que el propio arnés deja en
        // el historial tras una llamada ya resuelta. Sin esto, el encargo terminaba en el acto
        // dando SinEntrega aunque el modelo solo necesitara corregir el formato de la llamada, no
        // abandonar el encargo.
        if texto.trim_start().starts_with(PREFIJO_ETIQUETA_SOLICITUD) {
            if self.cuentas.etiqueta_como_texto() >= self.limites.max_denegaciones_seguidas {
                return Some(Desenlace::Abortado(
                    mensajes::abortado_por_etiqueta_como_texto(&self.detalle_de_entrega()),
                ));
            }
            self.conversacion.push(Mensaje::nuevo(Papel::Modelo, texto));
            self.conversacion.push(Mensaje::nuevo(
                Papel::Usuario,
                mensajes::AVISO_ETIQUETA_COMO_TEXTO,
            ));
            return None;
        }

        let propuestas = self.propuestas();
        if propuestas.is_empty() {
            Some(Desenlace::SinEntrega(texto))
        } else {
            Some(Desenlace::SinPublicar { texto, propuestas })
        }
    }

    /// Una respuesta que el motor cortó a medias.
    fn atender_cortada(&mut self, texto: String) -> Desenlace {
        if let Some(publicado) = self.publicado() {
            return publicado;
        }
        Desenlace::Cortado {
            texto,
            propuestas: self.propuestas(),
        }
    }

    /// Una solicitud de herramienta: se concede o se deniega y el encargo sigue (`None`), salvo que
    /// se pase de un límite o que con ella se haya publicado.
    fn atender_herramienta(&mut self, solicitud: SolicitudHerramienta) -> Option<Desenlace> {
        let nombre = solicitud.nombre.clone();

        // Colapsar llamadas idénticas repetidas sin gastar cupo ni volver a ejecutar
        if let Some(salida_previa) = self.historial.buscar_repeticion(&solicitud) {
            self.cuentas.llamada_buena();
            let llamada = llamada_hecha(&mut self.cuentas.llamadas_hechas, &solicitud);
            self.conversacion
                .push(Mensaje::llamada_del_modelo(llamada.clone()));
            self.conversacion.push(Mensaje::resultado_de(
                &llamada,
                mensajes::resultado_repetido(&nombre, salida_previa),
            ));
            return None;
        }

        if self.cuentas.gastar_cupo() > self.limites.max_herramientas {
            return Some(Desenlace::Abortado(mensajes::abortado_por_el_tope(
                self.limites.max_herramientas,
                &self.detalle_de_entrega(),
            )));
        }

        let devuelto = match self.repertorio.atender(&solicitud) {
            Decision::Concedida(salida) => {
                self.cuentas.llamada_buena();
                let devuelto = mensajes::resultado_concedido(&nombre, &salida);
                self.historial.registrar(&solicitud, salida);
                devuelto
            }
            Decision::Denegada(motivo) => {
                if self.cuentas.denegada(&nombre) >= self.limites.max_denegaciones_seguidas {
                    return Some(Desenlace::Abortado(mensajes::abortado_por_insistir(
                        &nombre,
                        &motivo,
                        &self.detalle_de_entrega(),
                    )));
                }
                mensajes::resultado_denegado(&nombre, &motivo)
            }
        };

        let llamada = llamada_hecha(&mut self.cuentas.llamadas_hechas, &solicitud);
        self.conversacion
            .push(Mensaje::llamada_del_modelo(llamada.clone()));
        self.conversacion
            .push(Mensaje::resultado_de(&llamada, devuelto));

        // **`publicar` cierra el encargo.** Es lo que dicen las normas («llama a `publicar` una
        // sola vez», «paras cuando has entregado y publicado»), y dejar el ciclo abierto hacía que
        // el modelo volviera a publicar lo mismo hasta cuatro veces (investigación del
        // 12/11/2026).
        self.publicado()
    }
}

/// Lo que este encargo dejó en disco, para el motivo de un aborto.
fn detalle_de_entrega(repertorio: &Repertorio, al_empezar: usize) -> String {
    estado_entrega_propuestas(&propuestas_del_encargo(repertorio, al_empezar))
}

/// Las propuestas que escribió este encargo, sin repetir y en el orden en que se escribieron.
fn propuestas_del_encargo(repertorio: &Repertorio, al_empezar: usize) -> Vec<String> {
    let todas = repertorio.propuestas();
    deduplicar_conservando_orden(todas.get(al_empezar..).unwrap_or_default())
}

/// Deduplica los nombres de propuestas preservando el orden de aparición.
fn deduplicar_conservando_orden(propuestas: &[String]) -> Vec<String> {
    let mut unicos = Vec::new();
    for p in propuestas {
        if !unicos.contains(p) {
            unicos.push(p.clone());
        }
    }
    unicos
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::arnes::herramientas::Repertorio;
    use crate::arnes::Ambito;
    use crate::error::Error;
    use crate::motor::proceso::EstadoServidor;
    use crate::motor::{MotorDoble, Respuesta, SolicitudHerramienta};

    fn repertorio() -> (tempfile::TempDir, Repertorio) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("nota.txt"), "contenido\n").unwrap();
        for i in 0..30 {
            std::fs::write(dir.path().join(format!("nota_{i}.txt")), "contenido\n").unwrap();
        }
        let ambito = Ambito::nuevo(dir.path()).unwrap();
        let canal = crate::protocolo::Canal::nuevo(dir.path(), "Programator").unwrap();
        (dir, Repertorio::nuevo(ambito, canal))
    }

    fn pedir(nombre: &str, argumentos: serde_json::Value) -> Respuesta {
        Respuesta::Herramienta(SolicitudHerramienta {
            nombre: nombre.to_string(),
            argumentos,
        })
    }

    #[test]
    fn el_modelo_ve_entera_la_llamada_que_hizo_y_su_resultado_enlazado() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "cola.lua", "contenido": "local Cola = {}\n\
                                                                       return Cola\n"}),
            ),
            Respuesta::Texto("Terminado.".to_string()),
        ]);

        atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        let conversacion = motor.ultima_conversacion();
        let turno = conversacion
            .iter()
            .find(|m| m.papel == Papel::Modelo)
            .expect("el turno del modelo debe estar en el historial");
        let llamada = turno.llamada.as_ref().expect("el turno lleva su llamada");
        assert_eq!(llamada.nombre, "escribir_propuesta");
        assert_eq!(
            llamada.argumentos["contenido"],
            "local Cola = {}\n\
             return Cola\n"
        );
        assert_eq!(llamada.id.len(), LONGITUD_ID_LLAMADA);
        assert!(
            llamada.id.chars().all(|c| c.is_ascii_alphanumeric()),
            "{}",
            llamada.id
        );

        let resultado = conversacion
            .iter()
            .find(|m| m.papel == Papel::Herramienta)
            .expect("el resultado debe estar en el historial");
        assert_eq!(resultado.llamada.as_ref().map(|l| &l.id), Some(&llamada.id));
        assert!(
            resultado.contenido.contains("cola.lua"),
            "{}",
            resultado.contenido
        );
    }

    #[test]
    fn cada_llamada_del_encargo_lleva_un_identificador_distinto() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_1.txt"})),
            Respuesta::Texto("Terminado.".to_string()),
        ]);

        atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        let ids: Vec<&str> = motor
            .ultima_conversacion()
            .iter()
            .filter(|m| m.papel == Papel::Modelo)
            .filter_map(|m| m.llamada.as_ref().map(|l| l.id.as_str()))
            .collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn publicar_cierra_el_encargo_sin_volver_a_preguntar_al_modelo() {
        let (_dir, mut r) = repertorio();
        // Un guion de una sola respuesta: si el ciclo volviera a preguntar, el doble daría error.
        let mut motor = MotorDoble::con_guion(vec![pedir(
            "publicar",
            serde_json::json!({"texto": "Hecho y comprobado."}),
        )]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        match desenlace {
            Desenlace::Publicado(cuerpo) => assert!(cuerpo.contains("Hecho y comprobado.")),
            otro => panic!("se esperaba Publicado: {otro:?}"),
        }
    }

    #[test]
    fn un_encargo_normal_termina_publicando() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir(
                "publicar",
                serde_json::json!({"texto": "He leído la nota."}),
            ),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::Publicado(ref t) if t.contains("He leído la nota")));
    }

    #[test]
    fn dos_denegaciones_iguales_seguidas_abortan_el_ciclo() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir(
                "leer_fichero",
                serde_json::json!({"ruta": "../../otra.txt"}),
            ),
            Respuesta::Texto("nunca se llega aquí".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::Abortado(ref m) if m.contains("insistió")));
    }

    #[test]
    fn una_denegacion_aislada_no_aborta_y_el_modelo_puede_rectificar() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir("listar", serde_json::json!({"ruta": "."})),
            pedir("publicar", serde_json::json!({"texto": "Rectifiqué."})),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::Publicado(_)));
    }

    #[test]
    fn el_limite_de_herramientas_corta_un_bucle_improductivo() {
        let (_dir, mut r) = repertorio();
        let guion: Vec<Respuesta> = (0..20)
            .map(|i| {
                pedir(
                    "leer_fichero",
                    serde_json::json!({"ruta": format!("nota_{i}.txt")}),
                )
            })
            .collect();
        let mut motor = MotorDoble::con_guion(guion);
        let limites = Limites {
            max_herramientas: 12,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        assert!(matches!(desenlace, Desenlace::Abortado(ref m) if m.contains("12")));
    }

    #[test]
    fn un_motor_agotado_se_registra_como_parada_y_no_como_exito() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::Abortado(_)));
    }

    #[test]
    fn terminar_sin_publicar_no_cuenta_como_publicado() {
        let (_dir, mut r) = repertorio();
        let mut motor =
            MotorDoble::con_guion(vec![Respuesta::Texto("He pensado mucho.".to_string())]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::SinEntrega(_)));
    }

    #[test]
    fn una_respuesta_cortada_por_el_motor_no_es_sin_entrega() {
        // Tanda del banco del 13/11/2026: el modelo llenó el contexto razonando y el arnés
        // publicaba que «no invocó herramientas», como si hubiera cerrado por su cuenta.
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![Respuesta::Cortada(String::new())]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        match desenlace {
            Desenlace::Cortado { texto, propuestas } => {
                assert!(texto.is_empty());
                assert!(propuestas.is_empty());
            }
            otro => panic!("se esperaba Cortado: {otro:?}"),
        }
    }

    #[test]
    fn una_respuesta_cortada_tras_escribir_nombra_lo_que_quedo_en_disco() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "cola.lua", "contenido": "return {}"}),
            ),
            Respuesta::Cortada("Ahora repaso".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        match desenlace {
            Desenlace::Cortado { texto, propuestas } => {
                assert_eq!(texto, "Ahora repaso");
                assert_eq!(propuestas, vec!["cola.lua".to_string()]);
            }
            otro => panic!("se esperaba Cortado: {otro:?}"),
        }
    }

    #[test]
    fn cerrar_con_texto_tras_escribir_una_propuesta_no_es_sin_entrega() {
        // E03 y E05 de la tanda del 11/11: propuesta escrita, cierre con texto y un aviso que
        // decía que no se había invocado ninguna herramienta de entrega.
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "cola.lua", "contenido": "return {}"}),
            ),
            Respuesta::Texto("Ya está.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        match desenlace {
            Desenlace::SinPublicar { texto, propuestas } => {
                assert_eq!(texto, "Ya está.");
                assert_eq!(propuestas, vec!["cola.lua".to_string()]);
            }
            otro => panic!("se esperaba SinPublicar: {otro:?}"),
        }
    }

    #[test]
    fn las_propuestas_de_un_encargo_anterior_no_cuentan_como_de_este() {
        let (_dir, mut r) = repertorio();
        let mut primero = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "a.rs", "contenido": "fn a() {}"}),
            ),
            pedir("publicar", serde_json::json!({"texto": "Entregado a.rs."})),
        ]);
        atender_encargo(&mut primero, &mut r, vec![], &Limites::default());

        // Una herramienta concedida antes del texto: con el cuerpo del primer encargo todavía
        // pendiente, el segundo cerraba ahí publicando lo del primero.
        let mut segundo = MotorDoble::con_guion(vec![
            pedir("listar", serde_json::json!({"ruta": "."})),
            Respuesta::Texto("Nada que hacer.".to_string()),
        ]);
        let desenlace = atender_encargo(&mut segundo, &mut r, vec![], &Limites::default());

        assert!(
            matches!(desenlace, Desenlace::SinEntrega(_)),
            "{desenlace:?}"
        );
    }

    #[test]
    fn con_el_plazo_vencido_tras_la_segunda_vuelta_no_hay_tercera_llamada_al_motor() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let (_dir, mut r) = repertorio();
        // Tres respuestas en el guion: si hubiera tercera llamada, el encargo publicaría.
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "cola.lua", "contenido": "return {}"}),
            ),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir("publicar", serde_json::json!({"texto": "nunca se llega"})),
        ]);
        // Un reloj de mentira: el plazo vence a la tercera vez que se pregunta, es decir, justo
        // antes de la tercera llamada al motor.
        let preguntas = Arc::new(AtomicU32::new(0));
        let contador = Arc::clone(&preguntas);
        let limites = Limites {
            plazo: Some(Plazo::con_reloj(move || {
                contador.fetch_add(1, Ordering::SeqCst) >= 2
            })),
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::FueraDePlazo(motivo) => {
                assert!(motivo.contains("tiempo máximo"), "{motivo}");
                assert!(motivo.contains("cola.lua"), "{motivo}");
                // El 12/11/2026 una edición perdió la continuación de línea y el mensaje salía
                // con una racha de espacios en mitad de la frase.
                assert!(!motivo.contains("  "), "{motivo}");
            }
            otro => panic!("se esperaba FueraDePlazo: {otro:?}"),
        }
        // La última conversación que vio el motor es la de la segunda llamada: la primera
        // llamada a herramienta y su resultado.
        assert_eq!(motor.ultima_conversacion().len(), 2);
    }

    #[test]
    fn sin_plazo_el_ciclo_sigue_como_siempre() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir("publicar", serde_json::json!({"texto": "Hecho."})),
        ]);
        let limites = Limites {
            plazo: None,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        assert!(
            matches!(desenlace, Desenlace::Publicado(_)),
            "{desenlace:?}"
        );
    }

    #[test]
    fn abortar_por_tope_con_propuesta_escrita_nombra_la_propuesta() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "parcial.rs", "contenido": "fn f() {}"}),
            ),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_0.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_1.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 2,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("superó las 2 solicitudes"));
                assert!(motivo.contains("parcial.rs"));
                assert!(!motivo.contains("No hay entrega"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn abortar_por_tope_sin_propuesta_dice_que_no_hay_entrega() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_0.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_1.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_2.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 2,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("superó las 2 solicitudes"));
                assert!(motivo.contains("No hay entrega"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn abortar_por_insistencia_con_propuesta_escrita_nombra_la_propuesta() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "entrega.rs", "contenido": "struct S;"}),
            ),
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir(
                "leer_fichero",
                serde_json::json!({"ruta": "../../otra.txt"}),
            ),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("insistió"));
                assert!(motivo.contains("entrega.rs"));
                assert!(!motivo.contains("No hay entrega"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn abortar_con_multiples_propuestas_las_nombra_todas() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "primera.rs", "contenido": "struct A;"}),
            ),
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "segunda.rs", "contenido": "struct B;"}),
            ),
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir(
                "leer_fichero",
                serde_json::json!({"ruta": "../../otra.txt"}),
            ),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("insistió"));
                assert!(motivo.contains("primera.rs"));
                assert!(motivo.contains("segunda.rs"));
                assert!(motivo.contains("Quedaron en disco las propuestas"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn dos_llamadas_identicas_gastan_un_solo_cupo() {
        let (_dir, mut r) = repertorio();
        // Con max_herramientas: 1, si la segunda llamada idéntica consumiera cupo,
        // la cuenta llegaría a 2 y abortaría. Como no consume cupo, termina en SinEntrega sin abortar.
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            Respuesta::Texto("Listo tras repetir.".to_string()),
        ]);
        let limites = Limites {
            max_herramientas: 1,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        assert!(
            matches!(desenlace, Desenlace::SinEntrega(ref t) if t.contains("Listo tras repetir"))
        );
    }

    #[test]
    fn segunda_llamada_identica_devuelve_lo_mismo_y_declara_repeticion() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            Respuesta::Texto("Terminado.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());
        assert!(matches!(desenlace, Desenlace::SinEntrega(_)));

        // Verificamos en la conversación del motor qué recibió el modelo en el segundo turno
        let conversacion = motor.ultima_conversacion();
        // El resultado de una llamada es un mensaje de herramienta, enlazado con ella.
        let ultimo_mensaje_usuario = conversacion
            .iter()
            .rfind(|m| m.papel == Papel::Herramienta)
            .expect("debe haber un resultado de herramienta");

        assert!(ultimo_mensaje_usuario.contenido.contains("repetición"));
        assert!(ultimo_mensaje_usuario.contenido.contains("contenido"));
        assert!(ultimo_mensaje_usuario
            .contenido
            .contains("Esta llamada es idéntica a una anterior"));
        assert!(ultimo_mensaje_usuario.contenido.contains("sin gastar cupo"));
    }

    #[test]
    fn dos_llamadas_con_distintos_argumentos_no_se_colapsan() {
        let (_dir, mut r) = repertorio();
        // Con max_herramientas: 1, dos llamadas al mismo verbo con distintos argumentos
        // deben consumir 1 cupo cada una y abortar en la segunda por superar el cupo.
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_0.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota_1.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 1,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("superó las 1 solicitudes de herramienta"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn abortar_por_insistencia_tras_denegacion_sigue_funcionando() {
        let (_dir, mut r) = repertorio();
        // Dos llamadas idénticas que resultan en denegación NO deben colapsarse ni ocultar
        // el mecanismo de insistencia tras denegación.
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
            pedir("leer_fichero", serde_json::json!({"ruta": "../fuera.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 12,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("insistió en «leer_fichero» tras una denegación"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn propuesta_con_mismo_nombre_y_distinto_contenido_no_se_colapsa_y_se_versiona() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "calc.rs", "contenido": "v1"}),
            ),
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "calc.rs", "contenido": "v2"}),
            ),
            pedir("publicar", serde_json::json!({"texto": "Entregado."})),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());
        assert!(matches!(desenlace, Desenlace::Publicado(_)));

        let propuestas = r.propuestas();
        assert_eq!(propuestas.len(), 2);
        assert_eq!(propuestas[0], "calc.rs");
        assert_eq!(propuestas[1], "calc-002.rs");
    }

    #[test]
    fn propuesta_con_mismo_nombre_y_mismo_contenido_se_colapsa_sin_versionar() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "calc.rs", "contenido": "v1"}),
            ),
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "calc.rs", "contenido": "v1"}),
            ),
            pedir("publicar", serde_json::json!({"texto": "Entregado."})),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());
        assert!(matches!(desenlace, Desenlace::Publicado(_)));

        let propuestas = r.propuestas();
        // Solo debe haber una propuesta registrada en disco, no calc-002.rs
        assert_eq!(propuestas.len(), 1);
        assert_eq!(propuestas[0], "calc.rs");
    }

    #[test]
    fn lectura_repetida_tras_mutacion_no_se_colapsa() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "p.rs", "contenido": "fn p(){}"}),
            ),
            pedir("leer_fichero", serde_json::json!({"ruta": "nota.txt"})),
        ]);
        let limites = Limites {
            max_herramientas: 2,
            max_denegaciones_seguidas: 2,
            max_repeticiones_recordadas: 12,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(motivo.contains("superó las 2 solicitudes de herramienta"));
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn el_modelo_que_escribe_la_etiqueta_de_solicitud_como_texto_recibe_un_aviso_y_puede_rectificar(
    ) {
        let (_dir, mut r) = repertorio();
        // El modelo imita «[solicito leer_fichero]» como texto plano (visto en uso real) en vez
        // de invocar la herramienta de verdad. El arnés no debe darlo por terminado: debe avisarle y
        // dejarle rectificar, igual que ante una denegación.
        let mut motor = MotorDoble::con_guion(vec![
            Respuesta::Texto("[solicito leer_fichero]".to_string()),
            pedir(
                "publicar",
                serde_json::json!({"texto": "Rectifiqué tras el aviso."}),
            ),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(
            matches!(desenlace, Desenlace::Publicado(ref t) if t.contains("Rectifiqué tras el aviso"))
        );
    }

    #[test]
    fn el_aviso_por_etiqueta_como_texto_explica_que_hay_que_invocar_la_herramienta() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            Respuesta::Texto("[solicito leer_fichero]".to_string()),
            pedir("publicar", serde_json::json!({"texto": "Ya."})),
            Respuesta::Texto("Listo.".to_string()),
        ]);

        atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        // El primer mensaje de usuario que ve el modelo es el aviso del mimetismo.
        let conversacion = motor.ultima_conversacion();
        let aviso = conversacion
            .iter()
            .find(|m| m.papel == Papel::Usuario)
            .expect("debe haber al menos un mensaje de usuario");
        assert!(aviso.contenido.contains("texto plano"));
        assert!(aviso.contenido.contains("invóca"));
    }

    #[test]
    fn insistir_escribiendo_la_etiqueta_como_texto_aborta_igual_que_insistir_en_una_denegacion() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![
            Respuesta::Texto("[solicito leer_fichero]".to_string()),
            Respuesta::Texto("[solicito leer_fichero]".to_string()),
        ]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(
            matches!(desenlace, Desenlace::Abortado(ref m) if m.contains("etiqueta de una herramienta como texto"))
        );
    }

    #[test]
    fn un_texto_final_que_no_empieza_por_la_etiqueta_de_solicitud_termina_sin_entrega_como_siempre()
    {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_guion(vec![Respuesta::Texto(
            "He terminado de pensar, pero no publico nada.".to_string(),
        )]);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(matches!(desenlace, Desenlace::SinEntrega(_)));
    }

    #[test]
    fn un_fallo_transitorio_del_motor_seguido_de_respuesta_buena_entrega_el_encargo() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_fallos(vec![
            Err(Error::MotorSinRespuesta(
                "contención de GPU con Godot".into(),
            )),
            Ok(pedir(
                "publicar",
                serde_json::json!({"texto": "Completado tras reintento."}),
            )),
            Ok(Respuesta::Texto("Listo.".to_string())),
        ])
        .con_salud_fija(EstadoServidor::Listo);

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &Limites::default());

        assert!(
            matches!(desenlace, Desenlace::Publicado(ref t) if t.contains("Completado tras reintento"))
        );
    }

    #[test]
    fn un_fallo_persistente_aborta_tras_los_reintentos_configurados_y_declara_cuenta_y_salud() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_fallos(vec![
            Err(Error::MotorSinRespuesta("GPU timeout 1".into())),
            Err(Error::MotorSinRespuesta("GPU timeout 2".into())),
            Err(Error::MotorSinRespuesta("GPU timeout 3".into())),
        ])
        .con_salud_fija(EstadoServidor::Listo);

        let limites = Limites {
            max_reintentos_motor: 2,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(
                    motivo.contains("2 reintentos"),
                    "debe indicar 2 reintentos: {motivo}"
                );
                assert!(
                    motivo.contains("3 intentos en total"),
                    "debe indicar 3 intentos: {motivo}"
                );
                assert!(
                    motivo.contains("Listo"),
                    "debe indicar que la salud era Listo: {motivo}"
                );
                assert!(
                    motivo.contains("No hay entrega"),
                    "debe reflejar que no hubo entrega: {motivo}"
                );
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn un_fallo_con_servidor_no_disponible_aborta_en_primer_intento_indicando_salud() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_fallos(vec![Err(Error::MotorSinRespuesta(
            "conexión rechazada".into(),
        ))])
        .con_salud_fija(EstadoServidor::NoDisponible);

        let limites = Limites {
            max_reintentos_motor: 3,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(
                    motivo.contains("1 intento"),
                    "debe indicar 1 intento: {motivo}"
                );
                assert!(
                    motivo.contains("NoDisponible"),
                    "debe indicar NoDisponible: {motivo}"
                );
                assert!(
                    motivo.contains("conexión rechazada"),
                    "debe incluir el error: {motivo}"
                );
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }

    #[test]
    fn fallo_persistente_con_propuesta_escrita_nombra_la_propuesta_y_la_salud() {
        let (_dir, mut r) = repertorio();
        let mut motor = MotorDoble::con_fallos(vec![
            Ok(pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "avance.rs", "contenido": "fn avance(){}"}),
            )),
            Err(Error::MotorSinRespuesta("GPU pegada".into())),
            Err(Error::MotorSinRespuesta("GPU pegada de nuevo".into())),
        ])
        .con_salud_fija(EstadoServidor::Listo);

        let limites = Limites {
            max_reintentos_motor: 1,
            ..Limites::default()
        };

        let desenlace = atender_encargo(&mut motor, &mut r, vec![], &limites);

        match desenlace {
            Desenlace::Abortado(motivo) => {
                assert!(
                    motivo.contains("1 reintento"),
                    "cuenta de reintentos: {motivo}"
                );
                assert!(
                    motivo.contains("2 intentos en total"),
                    "total de intentos: {motivo}"
                );
                assert!(motivo.contains("Listo"), "salud del servidor: {motivo}");
                assert!(
                    motivo.contains("avance.rs"),
                    "debe nombrar la propuesta en disco: {motivo}"
                );
                assert!(
                    !motivo.contains("No hay entrega"),
                    "no debe decir que no hay entrega: {motivo}"
                );
            }
            otro => panic!("se esperaba Abortado: {otro:?}"),
        }
    }
}
