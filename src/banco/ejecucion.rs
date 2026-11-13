//! Pasar un encargo del banco por el ciclo real de Programator y recoger lo que deja.
//!
//! Nada de atajos: el encargo se escribe en un buzón del canal, como lo haría el agente que dirige,
//! y lo atiende `sesion::ejecutar_pasada`, la misma que usa el bucle de `main`. Así lo que se mide
//! es Programator entero con ese modelo, no el modelo suelto.
//!
//! Cada encargo y cada semilla trabajan en una carpeta nueva, para que nada de uno contamine al
//! siguiente.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::ficha::{Encargo, CARPETA_PARTIDA, NOMBRE_ENUNCIADO};
use crate::arnes::ciclo::{Limites, Plazo};
use crate::arnes::espacio::ruta_de_las_normas;
use crate::arnes::herramientas::CARPETA_DE_CANDIDATOS;
use crate::arnes::sesion::{ejecutar_pasada, Ajustes, DesenlacePasada, EstadoMotor};
use crate::error::{Error, Resultado};
use crate::ficheros::{copiar_arbol, crear_carpeta, entradas, escribir, leer, leer_si_existe};
use crate::motor::proceso::EstadoServidor;
use crate::motor::{Consumo, Mensaje, Motor, Respuesta};

/// El canal dentro de la carpeta de trabajo.
const CARPETA_DEL_CANAL: &str = ".gestor/canal";

/// La carpeta de un encargo en una semilla, dentro de la carpeta de la tanda.
pub fn carpeta_del_encargo(tanda: &Path, id: &str, semilla: i64) -> PathBuf {
    tanda.join(format!("{id}-semilla-{semilla}"))
}

/// El encargo tal como se escribe en el buzón del agente que dirige.
pub fn texto_del_encargo(agente: &str, id: &str, enunciado: &str) -> String {
    format!("## Para {agente}: {id}\n\n{}\n", enunciado.trim_end())
}

/// La primera línea del buzón de quien encarga, antes de que llegue el encargo.
fn encabezado_del_buzon(buzon: &str) -> String {
    format!("# {buzon}\n")
}

/// Lo que se pasa de fuera para ejecutar un encargo.
pub struct Preparacion<'a> {
    /// El nombre de Programator en el canal (`[agente] nombre`).
    pub agente: &'a str,
    /// El buzón desde el que se escribe el encargo (`buzon_que_encarga` del banco).
    pub buzon_que_encarga: &'a str,
    /// El contenido de las instrucciones del proyecto, ya compuesto.
    pub normas: &'a str,
    pub tiempo_maximo: Duration,
}

/// El tiempo, inyectable para probar las esperas sin esperar.
pub trait Reloj {
    fn transcurrido(&self) -> Duration;
    fn esperar(&mut self, cuanto: Duration);
}

/// El reloj de verdad.
pub struct RelojReal {
    inicio: Instant,
}

impl RelojReal {
    pub fn desde_ahora() -> Self {
        Self {
            inicio: Instant::now(),
        }
    }
}

impl Reloj for RelojReal {
    fn transcurrido(&self) -> Duration {
        self.inicio.elapsed()
    }
    fn esperar(&mut self, cuanto: Duration) {
        std::thread::sleep(cuanto);
    }
}

/// La suma del consumo de todas las respuestas de un encargo.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ConsumoAcumulado {
    pub total: Consumo,
    pub respuestas: usize,
    /// Las que no trajeron consumo: si no es 0, el total se queda corto.
    pub respuestas_sin_consumo: usize,
}

impl ConsumoAcumulado {
    fn anotar(&mut self, consumo: Option<Consumo>) {
        self.respuestas += 1;
        match consumo {
            Some(consumo) => self.total = self.total.sumar(&consumo),
            None => self.respuestas_sin_consumo += 1,
        }
    }
}

/// Envuelve el motor que da la sesión y anota el consumo de cada respuesta.
struct MotorMedido {
    interno: Box<dyn Motor>,
    acumulado: Rc<RefCell<ConsumoAcumulado>>,
}

impl Motor for MotorMedido {
    fn responder(&mut self, conversacion: &[Mensaje]) -> Resultado<Respuesta> {
        let respuesta = self.interno.responder(conversacion);
        self.acumulado
            .borrow_mut()
            .anotar(self.interno.ultimo_consumo());
        respuesta
    }

    fn comprobar_salud(&mut self) -> EstadoServidor {
        self.interno.comprobar_salud()
    }

    fn ultimo_consumo(&self) -> Option<Consumo> {
        self.interno.ultimo_consumo()
    }
}

/// Lo que deja un encargo, listo para comprobar y puntuar.
#[derive(Debug, Clone, PartialEq)]
pub struct Recogida {
    /// `false` si se pasó del tiempo máximo: cuenta como no entregado.
    pub a_tiempo: bool,
    /// Lo que publicó Programator en su buzón al atender el encargo.
    pub respuesta: String,
    /// Los ficheros que entregó con `escribir_propuesta`, ordenados.
    pub entregas: Vec<PathBuf>,
    pub consumo: ConsumoAcumulado,
    /// Tiempo de pared desde que se escribió el encargo hasta que se atendió.
    pub duracion: Duration,
}

impl Recogida {
    /// La entrega con ese nombre de fichero, si la hay.
    pub fn entrega(&self, nombre: &str) -> Option<&Path> {
        self.entregas
            .iter()
            .find(|ruta| ruta.file_name().is_some_and(|n| n == nombre))
            .map(PathBuf::as_path)
    }
}

/// Deja la carpeta de trabajo de un encargo lista: canal vacío, ficheros de partida e
/// instrucciones. La carpeta no puede existir: así no hay restos de otro encargo.
pub fn preparar_carpeta(
    carpeta: &Path,
    encargo: &Encargo,
    preparacion: &Preparacion,
) -> Resultado<()> {
    if carpeta.exists() {
        return Err(Error::Banco(format!(
            "la carpeta de trabajo «{}» ya existe; cada encargo empieza en una nueva",
            carpeta.display()
        )));
    }
    crear_carpeta(&carpeta.join(CARPETA_DEL_CANAL))?;
    let partida = encargo.carpeta.join(CARPETA_PARTIDA);
    if partida.is_dir() {
        copiar_arbol(&partida, carpeta)?;
    }
    let normas = ruta_de_las_normas(carpeta, preparacion.agente);
    if let Some(padre) = normas.parent() {
        crear_carpeta(padre)?;
    }
    escribir(&normas, preparacion.normas)
}

/// Pasa el encargo por el ciclo real en `carpeta`, que ya tiene que estar preparada.
///
/// 1. Una primera pasada con el canal vacío, que solo toma el punto de partida.
/// 2. El encargo, escrito en el buzón de quien encarga.
/// 3. Pasadas hasta que se atiende; mientras el motor carga, se espera `intervalo_segundos` de los
///    ajustes, como el bucle de `main`, y si se pasa del tiempo máximo se deja de esperar.
pub fn pasar_encargo(
    carpeta: &Path,
    encargo: &Encargo,
    preparacion: &Preparacion,
    ajustes: &Ajustes,
    asegurar_motor: &mut dyn FnMut() -> Resultado<EstadoMotor>,
    reloj: &mut dyn Reloj,
) -> Resultado<Recogida> {
    let acumulado = Rc::new(RefCell::new(ConsumoAcumulado::default()));
    let mut asegurar_medido = || -> Resultado<EstadoMotor> {
        Ok(match asegurar_motor()? {
            EstadoMotor::Disponible(interno) => EstadoMotor::Disponible(Box::new(MotorMedido {
                interno,
                acumulado: Rc::clone(&acumulado),
            })),
            EstadoMotor::Esperando => EstadoMotor::Esperando,
        })
    };

    // El buzón de quien encarga existe ya, solo con su encabezado, antes de la primera pasada: es
    // lo que esa pasada fotografía. Con el canal vacío no habría nada que fotografiar, el registro
    // seguiría vacío y la pasada siguiente volvería a ser «primera».
    let buzon = carpeta
        .join(CARPETA_DEL_CANAL)
        .join(format!("{}.md", preparacion.buzon_que_encarga));
    let encabezado = encabezado_del_buzon(preparacion.buzon_que_encarga);
    escribir(&buzon, &encabezado)?;

    match ejecutar_pasada(carpeta, preparacion.agente, ajustes, &mut asegurar_medido)? {
        DesenlacePasada::PrimeraPasada => {}
        otro => return Err(no_detectado(encargo, &otro)),
    }

    let enunciado = leer(&encargo.carpeta.join(NOMBRE_ENUNCIADO))?;
    let texto = texto_del_encargo(preparacion.agente, &encargo.ficha.id, &enunciado);
    escribir(
        &buzon,
        &format!(
            "{encabezado}\n\
             {texto}"
        ),
    )?;

    let inicio = reloj.transcurrido();
    // El tiempo máximo corta también el ciclo, no solo la cuenta del final: en la tanda del
    // 11/11/2026 hubo pasadas de 1327 s con un tope de 900 s. El plazo va con el reloj de verdad,
    // que es el que gasta el motor; sin hora representable no hay plazo y solo se cuenta al final.
    let limites = Limites {
        plazo: Instant::now()
            .checked_add(preparacion.tiempo_maximo)
            .map(Plazo::hasta),
        ..ajustes.limites.clone()
    };
    let ajustes = &Ajustes {
        limites: &limites,
        ..*ajustes
    };
    let a_tiempo = loop {
        match ejecutar_pasada(carpeta, preparacion.agente, ajustes, &mut asegurar_medido)? {
            DesenlacePasada::Atendidos(_) => {
                break reloj.transcurrido() - inicio <= preparacion.tiempo_maximo;
            }
            DesenlacePasada::MotorCargando => {
                if reloj.transcurrido() - inicio > preparacion.tiempo_maximo {
                    break false;
                }
                reloj.esperar(Duration::from_secs(ajustes.intervalo_segundos));
            }
            otro => return Err(no_detectado(encargo, &otro)),
        }
    };
    let duracion = reloj.transcurrido() - inicio;

    let consumo = *acumulado.borrow();
    Recogida::de_la_carpeta(carpeta, preparacion.agente, a_tiempo, consumo, duracion)
}

impl Recogida {
    /// Lo que quedó en la carpeta de un encargo ya pasado, con lo que se midió al pasarlo.
    ///
    /// La respuesta y las entregas se leen siempre del disco, también al terminar una pasada: así
    /// `--banco-comprobar`, que las vuelve a leer otro día, recoge exactamente lo mismo.
    pub fn de_la_carpeta(
        carpeta: &Path,
        agente: &str,
        a_tiempo: bool,
        consumo: ConsumoAcumulado,
        duracion: Duration,
    ) -> Resultado<Self> {
        Ok(Self {
            a_tiempo,
            respuesta: leer_si_existe(
                &carpeta
                    .join(CARPETA_DEL_CANAL)
                    .join(format!("{}.md", agente.to_lowercase())),
            )?,
            entregas: ficheros_de(&carpeta.join(CARPETA_DE_CANDIDATOS))?,
            consumo,
            duracion,
        })
    }
}

fn no_detectado(encargo: &Encargo, desenlace: &DesenlacePasada) -> Error {
    Error::Banco(format!(
        "{}: la pasada no fue la esperada ({desenlace:?}); el encargo no llegó al ciclo",
        encargo.ficha.id
    ))
}

fn ficheros_de(carpeta: &Path) -> Resultado<Vec<PathBuf>> {
    if !carpeta.is_dir() {
        return Ok(Vec::new());
    }
    let mut ficheros: Vec<PathBuf> = entradas(carpeta)?
        .into_iter()
        .filter(|ruta| ruta.is_file())
        .collect();
    ficheros.sort();
    Ok(ficheros)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::arnes::ciclo::Limites;
    use crate::banco::ficha::FichaEncargo;
    use crate::config::Verificacion;
    use crate::motor::{MotorDoble, SolicitudHerramienta};

    /// Un reloj que solo avanza cuando se espera.
    struct RelojFalso {
        ahora: Duration,
        esperas: usize,
    }

    impl Reloj for RelojFalso {
        fn transcurrido(&self) -> Duration {
            self.ahora
        }
        fn esperar(&mut self, cuanto: Duration) {
            self.ahora += cuanto;
            self.esperas += 1;
        }
    }

    fn reloj() -> RelojFalso {
        RelojFalso {
            ahora: Duration::ZERO,
            esperas: 0,
        }
    }

    fn encargo_en(dir: &Path, id: &str, partida: &[(&str, &str)]) -> Encargo {
        let carpeta = dir.join("encargos").join(id);
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(carpeta.join(NOMBRE_ENUNCIADO), "Escribe hola.rs.\n").unwrap();
        for (ruta, contenido) in partida {
            let destino = carpeta.join(CARPETA_PARTIDA).join(ruta);
            std::fs::create_dir_all(destino.parent().unwrap()).unwrap();
            std::fs::write(destino, contenido).unwrap();
        }
        let ficha: FichaEncargo = toml::from_str(&format!(
            "id = \"{id}\"\ntitulo = \"t\"\nfamilia = \"criterio\"\n"
        ))
        .unwrap();
        Encargo { ficha, carpeta }
    }

    fn preparacion() -> Preparacion<'static> {
        Preparacion {
            agente: "Programator",
            buzon_que_encarga: "banco",
            normas: "NORMAS DEL BANCO\n",
            tiempo_maximo: Duration::from_secs(900),
        }
    }

    fn pedir(nombre: &str, argumentos: serde_json::Value) -> Respuesta {
        Respuesta::Herramienta(SolicitudHerramienta {
            nombre: nombre.to_string(),
            argumentos,
        })
    }

    /// Da el motor una sola vez, como hace la sesión: se prepara una vez por pasada con encargos.
    fn motor_una_vez(motor: MotorDoble) -> impl FnMut() -> Resultado<EstadoMotor> {
        let mut motor = Some(motor);
        move || match motor.take() {
            Some(motor) => Ok(EstadoMotor::Disponible(Box::new(motor))),
            None => Err(Error::Banco("el motor se pidió dos veces".into())),
        }
    }

    fn pasar(
        dir: &Path,
        encargo: &Encargo,
        asegurar: &mut dyn FnMut() -> Resultado<EstadoMotor>,
        reloj: &mut dyn Reloj,
    ) -> Resultado<Recogida> {
        let carpeta = carpeta_del_encargo(&dir.join("tanda"), &encargo.ficha.id, 42);
        preparar_carpeta(&carpeta, encargo, &preparacion())?;
        let limites = Limites::default();
        let verificacion = Verificacion::default();
        let ajustes = Ajustes::con_limites(&limites, &verificacion);
        pasar_encargo(&carpeta, encargo, &preparacion(), &ajustes, asegurar, reloj)
    }

    #[test]
    fn el_encargo_se_escribe_con_el_encabezado_del_protocolo() {
        assert_eq!(
            texto_del_encargo("Programator", "E03", "Haz esto.\n\n"),
            "## Para Programator: E03\n\nHaz esto.\n"
        );
    }

    #[test]
    fn un_doble_que_entrega_deja_su_fichero_en_la_recogida() {
        let dir = tempfile::tempdir().unwrap();
        let encargo = encargo_en(dir.path(), "E03", &[]);
        let motor = MotorDoble::con_guion(vec![
            pedir(
                "escribir_propuesta",
                serde_json::json!({"nombre": "hola.rs", "contenido": "fn main() {}\n"}),
            ),
            pedir(
                "publicar",
                serde_json::json!({"texto": "Entregado hola.rs."}),
            ),
            Respuesta::Texto("Fin.".into()),
        ]);

        let recogida = pasar(
            dir.path(),
            &encargo,
            &mut motor_una_vez(motor),
            &mut reloj(),
        )
        .unwrap();

        let entrega = recogida.entrega("hola.rs").expect("debería haber entrega");
        assert_eq!(std::fs::read_to_string(entrega).unwrap(), "fn main() {}\n");
        assert!(recogida.respuesta.contains("Entregado hola.rs."));
        assert!(recogida.a_tiempo);
    }

    #[test]
    fn un_doble_que_solo_responde_texto_deja_la_respuesta_y_ninguna_entrega() {
        let dir = tempfile::tempdir().unwrap();
        let encargo = encargo_en(dir.path(), "E01", &[]);
        let motor = MotorDoble::con_guion(vec![Respuesta::Texto("Mi opinión es esta.".into())]);

        let recogida = pasar(
            dir.path(),
            &encargo,
            &mut motor_una_vez(motor),
            &mut reloj(),
        )
        .unwrap();

        assert!(recogida.entregas.is_empty());
        assert!(recogida.respuesta.contains("Mi opinión es esta."));
    }

    #[test]
    fn con_el_tiempo_maximo_agotado_el_ciclo_no_llega_a_llamar_al_motor() {
        let dir = tempfile::tempdir().unwrap();
        let encargo = encargo_en(dir.path(), "E03", &[]);
        let motor = MotorDoble::con_guion(vec![pedir(
            "escribir_propuesta",
            serde_json::json!({"nombre": "hola.rs", "contenido": "fn main() {}\n"}),
        )]);
        let preparacion = Preparacion {
            tiempo_maximo: Duration::ZERO,
            ..preparacion()
        };
        let carpeta = carpeta_del_encargo(&dir.path().join("tanda"), "E03", 42);
        preparar_carpeta(&carpeta, &encargo, &preparacion).unwrap();
        let limites = Limites::default();
        let verificacion = Verificacion::default();
        let ajustes = Ajustes::con_limites(&limites, &verificacion);

        let recogida = pasar_encargo(
            &carpeta,
            &encargo,
            &preparacion,
            &ajustes,
            &mut motor_una_vez(motor),
            &mut reloj(),
        )
        .unwrap();

        assert!(recogida.entregas.is_empty(), "el motor no debió responder");
        assert!(
            recogida.respuesta.contains("se acabó el tiempo máximo"),
            "{}",
            recogida.respuesta
        );
    }

    #[test]
    fn el_consumo_se_suma_a_lo_largo_de_varias_respuestas() {
        let dir = tempfile::tempdir().unwrap();
        let encargo = encargo_en(dir.path(), "E01", &[]);
        let gasto = |generados| {
            Some(Consumo {
                tokens_generados: Some(generados),
                ..Consumo::default()
            })
        };
        // `publicar` cierra el encargo: lo que va antes tiene que ser otra herramienta.
        let motor = MotorDoble::con_guion(vec![
            pedir("listar", serde_json::json!({"ruta": "."})),
            pedir("publicar", serde_json::json!({"texto": "Uno."})),
        ])
        .con_consumos(vec![gasto(10), gasto(5)]);

        let recogida = pasar(
            dir.path(),
            &encargo,
            &mut motor_una_vez(motor),
            &mut reloj(),
        )
        .unwrap();

        assert_eq!(recogida.consumo.respuestas, 2);
        assert_eq!(recogida.consumo.respuestas_sin_consumo, 0);
        assert_eq!(recogida.consumo.total.tokens_generados, Some(15));
    }

    #[test]
    fn mientras_el_motor_carga_se_espera_y_luego_se_atiende() {
        let dir = tempfile::tempdir().unwrap();
        let encargo = encargo_en(dir.path(), "E01", &[]);
        let mut motor = Some(MotorDoble::con_guion(vec![Respuesta::Texto(
            "Hecho.".into(),
        )]));
        let mut veces = 0;
        let mut asegurar = || {
            veces += 1;
            if veces < 3 {
                return Ok(EstadoMotor::Esperando);
            }
            match motor.take() {
                Some(m) => Ok(EstadoMotor::Disponible(Box::new(m) as Box<dyn Motor>)),
                None => Err(Error::Banco("dos veces".into())),
            }
        };
        let mut reloj = reloj();

        let recogida = pasar(dir.path(), &encargo, &mut asegurar, &mut reloj).unwrap();

        assert_eq!(reloj.esperas, 2);
        assert!(recogida.a_tiempo);
        assert!(recogida.respuesta.contains("Hecho."));
    }

    #[test]
    fn si_el_motor_no_termina_de_cargar_a_tiempo_no_se_entrega() {
        let dir = tempfile::tempdir().unwrap();
        let encargo = encargo_en(dir.path(), "E01", &[]);
        let mut asegurar = || Ok(EstadoMotor::Esperando);

        let recogida = pasar(dir.path(), &encargo, &mut asegurar, &mut reloj()).unwrap();

        assert!(!recogida.a_tiempo);
        assert!(recogida.entregas.is_empty());
        assert!(recogida.duracion > Duration::from_secs(900));
    }

    #[test]
    fn la_carpeta_lleva_la_partida_y_las_normas_y_no_ve_nada_del_encargo_anterior() {
        let dir = tempfile::tempdir().unwrap();
        let primero = encargo_en(dir.path(), "U01", &[("src/Pedidos.cs", "class P {}")]);
        let segundo = encargo_en(dir.path(), "U02", &[]);
        let motor = |texto: &str| {
            MotorDoble::con_guion(vec![
                pedir(
                    "escribir_propuesta",
                    serde_json::json!({"nombre": "x.rs", "contenido": texto}),
                ),
                Respuesta::Texto("Fin.".into()),
            ])
        };

        let uno = pasar(
            dir.path(),
            &primero,
            &mut motor_una_vez(motor("uno")),
            &mut reloj(),
        )
        .unwrap();
        let dos = pasar(
            dir.path(),
            &segundo,
            &mut motor_una_vez(motor("dos")),
            &mut reloj(),
        )
        .unwrap();

        let carpeta_uno = carpeta_del_encargo(&dir.path().join("tanda"), "U01", 42);
        let carpeta_dos = carpeta_del_encargo(&dir.path().join("tanda"), "U02", 42);
        assert!(carpeta_uno.join("src/Pedidos.cs").is_file());
        assert!(!carpeta_dos.join("src/Pedidos.cs").exists());
        assert_eq!(
            std::fs::read_to_string(ruta_de_las_normas(&carpeta_dos, "Programator")).unwrap(),
            "NORMAS DEL BANCO\n"
        );
        assert_eq!(uno.entregas.len(), 1);
        assert_eq!(dos.entregas.len(), 1);
        assert_eq!(std::fs::read_to_string(&dos.entregas[0]).unwrap(), "dos");
    }

    #[test]
    fn una_carpeta_que_ya_existe_no_se_reutiliza() {
        let dir = tempfile::tempdir().unwrap();
        let encargo = encargo_en(dir.path(), "E01", &[]);
        let carpeta = dir.path().join("ya-existe");
        std::fs::create_dir_all(&carpeta).unwrap();

        let error = preparar_carpeta(&carpeta, &encargo, &preparacion()).unwrap_err();

        assert!(error.to_string().contains("ya existe"));
    }
}
