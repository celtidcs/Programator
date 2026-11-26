//! Pruebas de la pasada con varios modelos: qué modelo atiende cada encargo según su línea
//! `Clase:`, cuándo se recarga y qué pasa a la cola de pendientes.

use super::pruebas::{
    buzon_propio, carpeta_de_prueba, conversaciones_registradas, escribir_buzon,
    sembrar_registro_no_vacio,
};
use super::*;
use crate::arnes::clase_pedida::{clase_pedida, mensaje_clase_desconocida, ClasePedida};
use crate::arnes::espacio::{carpeta_de_los_encargos, ruta_de_los_pendientes, ruta_del_latido};
use crate::arnes::pendientes::{self, Cola};
use crate::arnes::registro_de_encargos::{disco, FichaDeEncargo};
use crate::motor::{MotorDoble, Respuesta};
use crate::protocolo::Latido;

/// Respuestas de sobra para cualquier grupo: cada encargo gasta una.
const RESPUESTAS_POR_MOTOR: usize = 10;
const POR_DEFECTO: &str = "defecto";

/// Motores de mentira: cada clase pide el modelo con su nombre corto y, sin clase o con una que
/// no existe, el de por defecto. Apunta cada carga.
#[derive(Default)]
pub(super) struct MotoresDeMentira {
    pub(super) cargado: Option<String>,
    pub(super) cargas: Vec<String>,
    /// El modelo que, si hay que esperarlo en el sitio, no termina de cargar.
    pub(super) no_carga: Option<String>,
    pub(super) procedencia: Procedencia,
    /// Se ejecuta al preparar cada modelo que no es el primero que se prepara.
    pub(super) al_cambiar: Option<Box<dyn FnMut()>>,
    /// Lo que cuenta de la memoria tras cada encargo.
    pub(super) memoria: Option<String>,
    /// Lo que responde el modelo a cada encargo; sin ella, «Hecho.».
    pub(super) respuesta: Option<String>,
    /// El modelo que no se puede usar (falta o no cabe), con el motivo.
    pub(super) sin_modelo: Option<(String, String)>,
    /// El modelo cuyo motor falla al prepararlo, con el motivo.
    pub(super) falla: Option<(String, String)>,
    /// El modelo que, si no se espera, sigue cargando.
    pub(super) sigue_cargando: Option<String>,
    /// El motor que se entrega en la primera preparación, en lugar del que responde «Hecho.».
    pub(super) motor_a_medida: Option<Box<dyn crate::motor::Motor>>,
}

impl Motores for MotoresDeMentira {
    fn modelo_para(&self, encargo: &Encargo) -> ModeloPedido {
        match clase_pedida(&encargo.texto) {
            ClasePedida::Valida(fila) => ModeloPedido {
                modelo: Some(fila.nombre_corto().to_string()),
                aviso: None,
                por_clase: true,
            },
            ClasePedida::Ninguna => ModeloPedido {
                modelo: Some(POR_DEFECTO.to_string()),
                ..ModeloPedido::default()
            },
            ClasePedida::Desconocida(escrita) => ModeloPedido {
                modelo: Some(POR_DEFECTO.to_string()),
                aviso: Some(mensaje_clase_desconocida(&escrita)),
                por_clase: false,
            },
        }
    }

    fn cargado(&self) -> Option<String> {
        self.cargado.clone()
    }

    fn preparar(&mut self, modelo: Option<&str>, espera: Espera) -> Resultado<Preparacion> {
        let modelo = modelo.unwrap_or(POR_DEFECTO).to_string();
        if !self.cargas.is_empty() || self.cargado.is_some() {
            if let Some(al_cambiar) = self.al_cambiar.as_mut() {
                al_cambiar();
            }
        }
        if let Some((_, motivo)) = self.sin_modelo.as_ref().filter(|(m, _)| *m == modelo) {
            return Ok(Preparacion::SinModelo(motivo.clone()));
        }
        if let Some((_, motivo)) = self.falla.as_ref().filter(|(m, _)| *m == modelo) {
            return Err(crate::error::Error::MotorSinRespuesta(motivo.clone()));
        }
        if espera == Espera::HastaElTope && self.no_carga.as_deref() == Some(modelo.as_str()) {
            return Ok(Preparacion::TiempoAgotado);
        }
        if espera == Espera::Ninguna && self.sigue_cargando.as_deref() == Some(modelo.as_str()) {
            return Ok(Preparacion::Cargando);
        }
        if self.cargado.as_deref() != Some(modelo.as_str()) {
            self.cargas.push(modelo.clone());
            self.cargado = Some(modelo);
        }
        let texto = self
            .respuesta
            .clone()
            .unwrap_or_else(|| "Hecho.".to_string());
        let guion = vec![Respuesta::Texto(texto); RESPUESTAS_POR_MOTOR];
        Ok(Preparacion::Listo {
            motor: self
                .motor_a_medida
                .take()
                .unwrap_or_else(|| Box::new(MotorDoble::con_guion(guion))),
            procedencia: self.procedencia,
        })
    }

    fn tras_el_encargo(&mut self) -> Option<String> {
        self.memoria.clone()
    }
}

pub(super) fn pasar(carpeta: &Path, motores: &mut MotoresDeMentira) -> DesenlacePasada {
    pasar_con(carpeta, motores, false)
}

fn pasar_con(
    carpeta: &Path,
    motores: &mut MotoresDeMentira,
    registro_de_conversacion: bool,
) -> DesenlacePasada {
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let ajustes = Ajustes {
        registro_de_conversacion,
        ..Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal)
    };
    ejecutar_pasada_con_motores(carpeta, "Programator", &ajustes, motores, &mut SinRevision)
        .unwrap()
}

pub(super) fn pasar_sin_unwrap(
    carpeta: &Path,
    motores: &mut MotoresDeMentira,
) -> Resultado<DesenlacePasada> {
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let ajustes = Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal);
    ejecutar_pasada_con_motores(carpeta, "Programator", &ajustes, motores, &mut SinRevision)
}

pub(super) fn resumen(desenlace: DesenlacePasada) -> ResumenPasada {
    match desenlace {
        DesenlacePasada::Atendidos(resumen) => resumen,
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }
}

/// Las fichas que siguen pendientes en el registro de encargos.
pub(super) fn cola(carpeta: &Path) -> Vec<FichaDeEncargo> {
    disco::leer(&carpeta_de_los_encargos(carpeta, "Programator"))
        .unwrap()
        .fichas
        .into_iter()
        .filter(|ficha| ficha.estado.esta_abierto())
        .collect()
}

/// Un buzón con un encargo por línea `Para`, cada uno con su línea `Clase:` si la trae.
fn encargos(carpeta: &Path, lineas: &[(&str, Option<&str>)]) {
    let mut texto = String::from("# Bandeja de Codex\n\n");
    for (pedido, clase) in lineas {
        texto.push_str(&format!("Para Programator: {pedido}\n"));
        if let Some(clase) = clase {
            texto.push_str(&format!("Clase: {clase}\n"));
        }
    }
    escribir_buzon(carpeta, "codex.md", &texto);
}

#[test]
fn dos_encargos_de_la_misma_clase_cargan_el_modelo_una_sola_vez() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(
        &carpeta,
        &[("uno", Some("diagnostico")), ("dos", Some("diagnostico"))],
    );
    let mut motores = MotoresDeMentira::default();

    let resumen = resumen(pasar(&carpeta, &mut motores));

    assert_eq!(resumen.atendidos, 2);
    assert_eq!(motores.cargas, ["diagnostico"]);
}

#[test]
fn encargos_de_clases_distintas_empiezan_por_el_modelo_ya_cargado() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(
        &carpeta,
        &[
            ("uno", Some("diagnostico")),
            ("dos", Some("formas")),
            ("tres", Some("diagnostico")),
        ],
    );
    let mut motores = MotoresDeMentira {
        cargado: Some("formas".to_string()),
        ..MotoresDeMentira::default()
    };

    let resumen = resumen(pasar(&carpeta, &mut motores));

    assert_eq!(resumen.atendidos, 3);
    assert_eq!(motores.cargas, ["diagnostico"], "una sola recarga");
}

/// El latido de reposo tiene que decir el modelo cargado.
#[test]
fn publicada_la_respuesta_el_latido_de_reposo_dice_el_modelo_cargado() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(&carpeta, &[("uno", None)]);
    let mut motores = MotoresDeMentira::default();

    resumen(pasar(&carpeta, &mut motores));

    let latido = Latido::cargar(&ruta_del_latido(&carpeta, "Programator", "latido.json"))
        .unwrap()
        .expect("latido escrito");
    assert_eq!(latido.estado(), crate::protocolo::EstadoLatido::Reposo);
    assert_eq!(latido.modelo_cargado(), Some(POR_DEFECTO));
}

#[test]
fn sin_linea_de_clase_se_usa_el_modelo_por_defecto() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(&carpeta, &[("uno", None)]);
    let mut motores = MotoresDeMentira::default();

    resumen(pasar(&carpeta, &mut motores));

    assert_eq!(motores.cargas, [POR_DEFECTO]);
}

#[test]
fn una_clase_desconocida_se_atiende_con_el_de_por_defecto_y_la_entrega_lo_dice() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(&carpeta, &[("uno", Some("astrologia"))]);
    let mut motores = MotoresDeMentira::default();

    let resumen = resumen(pasar(&carpeta, &mut motores));

    assert_eq!(resumen.atendidos, 1);
    assert_eq!(motores.cargas, [POR_DEFECTO]);
    let buzon = buzon_propio(&carpeta);
    assert!(buzon.contains("`Clase: astrologia` no nombra"), "{buzon}");
    assert!(buzon.contains("`diagnostico`"), "{buzon}");
}

#[test]
fn con_un_servidor_ajeno_la_entrega_dice_que_no_se_pudo_cambiar_de_modelo() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(&carpeta, &[("uno", Some("diagnostico"))]);
    let mut motores = MotoresDeMentira {
        procedencia: Procedencia::Ajeno,
        ..MotoresDeMentira::default()
    };

    resumen(pasar(&carpeta, &mut motores));

    let buzon = buzon_propio(&carpeta);
    assert!(buzon.contains("no lo arrancó Programator"), "{buzon}");
}

#[test]
fn con_un_servidor_ajeno_y_sin_clase_no_hay_aviso() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(&carpeta, &[("uno", None)]);
    let mut motores = MotoresDeMentira {
        procedencia: Procedencia::Ajeno,
        ..MotoresDeMentira::default()
    };

    resumen(pasar(&carpeta, &mut motores));

    assert!(!buzon_propio(&carpeta).contains("no lo arrancó Programator"));
}

#[test]
fn si_un_modelo_no_carga_a_tiempo_su_encargo_queda_en_la_cola_y_el_registro_avanza() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(
        &carpeta,
        &[("uno", Some("formas")), ("dos", Some("diagnostico"))],
    );
    let mut motores = MotoresDeMentira {
        no_carga: Some("diagnostico".to_string()),
        ..MotoresDeMentira::default()
    };

    let primera = resumen(pasar(&carpeta, &mut motores));

    assert_eq!(primera.atendidos, 1);
    assert_eq!(primera.pendientes, 1);
    let en_cola = cola(&carpeta);
    assert_eq!(en_cola.len(), 1);
    assert!(en_cola[0].texto.contains("dos"));
    assert!(en_cola[0].motivo.contains("diagnostico"));

    // En la vuelta siguiente, sin nada nuevo en el canal, se atiende solo el pendiente: «uno» no
    // se repite porque el registro avanzó.
    motores.no_carga = None;
    let segunda = resumen(pasar(&carpeta, &mut motores));

    assert_eq!(segunda.atendidos, 1);
    assert!(cola(&carpeta).is_empty());
}

#[test]
fn un_pendiente_se_atiende_antes_que_un_encargo_nuevo_y_sale_de_la_cola() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let mut anterior = Cola::default();
    anterior.anotar_fallo(
        &Encargo {
            latido: None,
            de: "claude.md".into(),
            texto: "Para Programator: el pendiente".into(),
            forma: crate::protocolo::encargo::Forma::Linea,
        },
        "2026-11-22 12:00",
        "no cargó",
        3,
    );
    pendientes::guardar(&ruta_de_los_pendientes(&carpeta, "Programator"), &anterior).unwrap();
    encargos(&carpeta, &[("el nuevo", None)]);
    let mut motores = MotoresDeMentira::default();

    let resumen = resumen(pasar_con(&carpeta, &mut motores, true));

    assert_eq!(resumen.atendidos, 2);
    assert!(cola(&carpeta).is_empty());
    let mut ficheros = conversaciones_registradas(&carpeta);
    ficheros.sort();
    let primera = std::fs::read_to_string(&ficheros[0]).unwrap();
    assert!(primera.contains("el pendiente"), "{primera}");
}

/// Un encargo respondido queda atendido en el disco en ese momento, no al final de la pasada: si
/// Programator muere antes de acabarla, no se vuelve a atender (visto en la prueba manual O, en
/// Windows, el 25/11/2026: tres respuestas al mismo encargo).
#[test]
fn un_encargo_respondido_queda_atendido_en_el_disco_antes_del_siguiente() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(
        &carpeta,
        &[("uno", Some("formas")), ("dos", Some("diagnostico"))],
    );
    let (vistos, mut motores) = mirar_el_disco_al_cambiar(&carpeta, None);

    assert_eq!(resumen(pasar(&carpeta, &mut motores)).atendidos, 2);

    let vistos = vistos.borrow();
    assert_eq!(vistos.len(), 1);
    assert_eq!(cerradas(&vistos[0]), 1);
}

/// Un encargo que no se pudo atender queda en el disco con su intento antes de lo siguiente.
#[test]
fn un_fallo_queda_en_el_disco_antes_de_lo_siguiente() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "# Bandeja de Codex\n\nPara Programator: uno\nClase: formas\n\
         Para Programator: dos\nClase: diagnostico\nPara Programator: mira el disco\n",
    );
    let mut motores = MotoresDeMentira {
        falla: Some(("diagnostico".to_string(), "roto".to_string())),
        ..MotoresDeMentira::default()
    };

    let (desenlace, vistos) = pasar_mirando(&carpeta, &mut motores);

    assert_eq!(resumen(desenlace).atendidos, 2);
    assert_eq!(vistos.len(), 1);
    let dos = vistos[0]
        .iter()
        .find(|ficha| ficha.texto.contains("dos"))
        .unwrap();
    assert_eq!(dos.intentos, 1, "{:?}", vistos[0]);
}

/// Lo que resuelve el arnés sin el modelo, como una cancelación, queda en el disco antes de lo
/// siguiente.
#[test]
fn una_cancelacion_queda_en_el_disco_antes_de_lo_siguiente() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "# Bandeja de Codex\n\nPara Programator: PG-7 contar líneas\n\
         Para Programator: cancelar: PG-7\nPara Programator: mira el disco\n",
    );

    let (desenlace, vistos) = pasar_mirando(&carpeta, &mut MotoresDeMentira::default());

    assert_eq!(resumen(desenlace).atendidos, 2);
    assert_eq!(vistos.len(), 1);
    assert_eq!(cerradas(&vistos[0]), 2, "{:?}", vistos[0]);
}

/// Un revisor que, al atender «mira el disco», apunta las fichas que hay en el disco y hace lo que
/// se le pida. Atiende después de los grupos del modelo y de lo que resuelve el arnés.
struct RevisorQueMira {
    carpeta: std::path::PathBuf,
    vistos: Vec<Vec<FichaDeEncargo>>,
    al_atender: Option<Box<dyn FnOnce()>>,
}

impl Revisor for RevisorQueMira {
    fn es_suyo(&self, texto: &str) -> bool {
        texto.contains("mira el disco")
    }
    fn necesita_el_motor(&self, _texto: &str) -> bool {
        false
    }
    fn atender(
        &mut self,
        _de: &str,
        _quien: Option<&crate::protocolo::destinatarios::Identidad>,
        _texto: &str,
    ) -> String {
        let fichas = disco::leer(&carpeta_de_los_encargos(&self.carpeta, "Programator"))
            .unwrap()
            .fichas;
        self.vistos.push(fichas);
        if let Some(al_atender) = self.al_atender.take() {
            al_atender();
        }
        "Mirado.".to_string()
    }
}

/// Lo que cambia de una pasada con el revisor que mira el disco.
#[derive(Default)]
struct Mirando {
    intentos_por_encargo: Option<u32>,
    al_atender: Option<Box<dyn FnOnce()>>,
}

/// Una pasada con el revisor que mira el disco: su desenlace y lo que vio.
fn pasar_mirando(
    carpeta: &Path,
    motores: &mut MotoresDeMentira,
) -> (DesenlacePasada, Vec<Vec<FichaDeEncargo>>) {
    pasar_mirando_con(carpeta, motores, Mirando::default())
}

fn pasar_mirando_con(
    carpeta: &Path,
    motores: &mut MotoresDeMentira,
    mirando: Mirando,
) -> (DesenlacePasada, Vec<Vec<FichaDeEncargo>>) {
    let mut revisor = RevisorQueMira {
        carpeta: carpeta.to_path_buf(),
        vistos: Vec::new(),
        al_atender: mirando.al_atender,
    };
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let por_defecto = Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal);
    let ajustes = Ajustes {
        intentos_por_encargo: mirando
            .intentos_por_encargo
            .unwrap_or(por_defecto.intentos_por_encargo),
        ..por_defecto
    };
    let desenlace =
        ejecutar_pasada_con_motores(carpeta, "Programator", &ajustes, motores, &mut revisor)
            .unwrap();
    (desenlace, revisor.vistos)
}

/// La ficha del disco cuyo texto contiene `trozo`.
fn ficha_con<'a>(fichas: &'a [FichaDeEncargo], trozo: &str) -> &'a FichaDeEncargo {
    fichas
        .iter()
        .find(|ficha| ficha.texto.contains(trozo))
        .unwrap_or_else(|| panic!("ninguna ficha con «{trozo}»: {fichas:?}"))
}

/// Un motor de guion que, al recibir la llamada número `en_la`, hace `efecto`.
struct MotorConEfecto {
    doble: MotorDoble,
    llamadas: usize,
    en_la: usize,
    efecto: Option<Box<dyn FnOnce()>>,
}

impl crate::motor::Motor for MotorConEfecto {
    fn responder(&mut self, conversacion: &[crate::motor::Mensaje]) -> Resultado<Respuesta> {
        self.llamadas += 1;
        if self.llamadas == self.en_la {
            if let Some(efecto) = self.efecto.take() {
                efecto();
            }
        }
        self.doble.responder(conversacion)
    }
}

/// Un encargo que ya entregó su propuesta y cuyo cierre no se pudo publicar queda atendido en el
/// disco antes de lo siguiente: reatenderlo contestaría dos veces.
#[test]
fn un_cierre_sin_publicar_con_la_entrega_ya_publicada_queda_en_el_disco() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "# Bandeja de Codex\n\nPara Programator: escribe la cola\nPara Programator: mira el disco\n",
    );
    // Entregada la propuesta, el buzón propio deja de poder escribirse: el cierre no sale.
    let buzon = carpeta.join(".gestor/canal/programator.md");
    let mut motores = MotoresDeMentira {
        motor_a_medida: Some(Box::new(MotorConEfecto {
            doble: MotorDoble::con_guion(vec![
                Respuesta::Herramienta(crate::motor::SolicitudHerramienta {
                    nombre: "escribir_propuesta".to_string(),
                    argumentos: serde_json::json!({"nombre": "cola.lua", "contenido": "return {}"}),
                }),
                Respuesta::Texto("Hecho.".to_string()),
            ]),
            llamadas: 0,
            en_la: 2,
            efecto: Some(Box::new(move || {
                let _ = std::fs::remove_file(&buzon);
                std::fs::create_dir_all(&buzon).unwrap();
            })),
        })),
        ..MotoresDeMentira::default()
    };

    let (desenlace, vistos) = pasar_mirando(&carpeta, &mut motores);

    let resumen = resumen(desenlace);
    // Tampoco sale la respuesta del revisor: dos fallos al publicar.
    assert_eq!(resumen.fallidos, 2, "{resumen:?} {vistos:?}");
    assert_eq!(vistos.len(), 1);
    let cola = ficha_con(&vistos[0], "la cola");
    assert!(!cola.estado.esta_abierto(), "{:?}", vistos[0]);
}

/// Un encargo que agota sus intentos queda cerrado en el disco antes de lo siguiente: reatenderlo
/// volvería a contestar que no se pudo.
#[test]
fn un_encargo_cerrado_por_agotar_los_intentos_queda_en_el_disco() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "# Bandeja de Codex\n\nPara Programator: uno\nClase: formas\n\
         Para Programator: dos\nClase: diagnostico\nPara Programator: mira el disco\n",
    );
    let mut motores = MotoresDeMentira {
        falla: Some(("diagnostico".to_string(), "roto".to_string())),
        ..MotoresDeMentira::default()
    };
    let mirando = Mirando {
        intentos_por_encargo: Some(1),
        ..Mirando::default()
    };

    let (_, vistos) = pasar_mirando_con(&carpeta, &mut motores, mirando);

    assert_eq!(vistos.len(), 1);
    let dos = ficha_con(&vistos[0], "dos");
    assert_eq!(
        dos.estado,
        crate::arnes::registro_de_encargos::EstadoDelEncargo::Cerrado,
        "{:?}",
        vistos[0]
    );
}

/// Con el motor cargando, lo que resolvió el arnés y no se pudo guardar al momento se guarda en el
/// guardado siguiente de la rama (aquí, el del revisor). El del final de la rama es un respaldo:
/// solo cuenta si fallan todos los anteriores y él no, y no hay forma de provocarlo en una prueba.
#[test]
fn con_el_motor_cargando_lo_que_fallo_al_momento_se_guarda_despues() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let texto = "# Bandeja de Codex\n\nPara Programator: PG-7 contar líneas\n\
                 Para Programator: cancelar: PG-7\nPara Programator: dos\nClase: formas\n\
                 Para Programator: mira el disco\n";
    escribir_buzon(&carpeta, "codex.md", texto);
    let cancelacion = crate::protocolo::detectar("Programator", "codex.md", texto)
        .into_iter()
        .find(|encargo| encargo.texto.contains("cancelar"))
        .unwrap()
        .id();
    let estorbo = disco::ruta_de_la_ficha(
        &carpeta_de_los_encargos(&carpeta, "Programator"),
        &cancelacion,
    );
    // Al preparar el motor, la ficha de la cancelación deja de poder escribirse; al atender el
    // revisor, vuelve a poder.
    let poner = estorbo.clone();
    let mut motores = MotoresDeMentira {
        cargado: Some("otro".to_string()),
        sigue_cargando: Some("formas".to_string()),
        al_cambiar: Some(Box::new(move || {
            std::fs::remove_file(&poner).unwrap();
            std::fs::create_dir_all(poner.join("dentro")).unwrap();
        })),
        ..MotoresDeMentira::default()
    };
    let mirando = Mirando {
        al_atender: Some(Box::new(move || std::fs::remove_dir_all(&estorbo).unwrap())),
        ..Mirando::default()
    };

    let (desenlace, vistos) = pasar_mirando_con(&carpeta, &mut motores, mirando);

    assert!(
        matches!(desenlace, DesenlacePasada::MotorCargando),
        "{desenlace:?}"
    );
    assert_eq!(vistos.len(), 1);
    let en_disco = disco::leer(&carpeta_de_los_encargos(&carpeta, "Programator"))
        .unwrap()
        .fichas;
    assert!(
        !ficha_con(&en_disco, "cancelar").estado.esta_abierto(),
        "{en_disco:?}"
    );
    assert!(
        !ficha_con(&en_disco, "contar líneas").estado.esta_abierto(),
        "{en_disco:?}"
    );
}

/// Si el guardado al momento falla, la ficha sigue contando como cambiada y el final de la pasada
/// la guarda.
#[test]
fn si_el_guardado_al_momento_falla_lo_guarda_el_final_de_la_pasada() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(
        &carpeta,
        &[("uno", Some("formas")), ("dos", Some("diagnostico"))],
    );
    let texto = std::fs::read_to_string(carpeta.join(".gestor/canal/codex.md")).unwrap();
    let primero = crate::protocolo::detectar("Programator", "codex.md", &texto)[0].id();
    let estorbo =
        disco::ruta_de_la_ficha(&carpeta_de_los_encargos(&carpeta, "Programator"), &primero);
    // Al preparar el primer motor, la ficha del primero deja de poder escribirse; al preparar el
    // segundo, vuelve a poder.
    let mut veces = 0;
    let mut motores = MotoresDeMentira {
        cargado: Some("otro".to_string()),
        al_cambiar: Some(Box::new(move || {
            veces += 1;
            if veces == 1 {
                std::fs::remove_file(&estorbo).unwrap();
                std::fs::create_dir_all(estorbo.join("dentro")).unwrap();
            } else {
                std::fs::remove_dir_all(&estorbo).unwrap();
            }
        })),
        ..MotoresDeMentira::default()
    };

    assert_eq!(resumen(pasar(&carpeta, &mut motores)).atendidos, 2);

    let en_disco = disco::leer(&carpeta_de_los_encargos(&carpeta, "Programator"))
        .unwrap()
        .fichas;
    assert_eq!(en_disco.len(), 2, "{en_disco:?}");
    assert_eq!(cerradas(&en_disco), 2, "{en_disco:?}");
}

/// Un encargo cuyo modelo no se puede usar queda respondido en el disco antes de lo siguiente.
#[test]
fn un_encargo_sin_modelo_queda_en_el_disco_antes_de_lo_siguiente() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    escribir_buzon(
        &carpeta,
        "codex.md",
        "# Bandeja de Codex\n\nPara Programator: uno\nClase: formas\n\
         Para Programator: dos\nClase: diagnostico\nPara Programator: mira el disco\n",
    );
    let mut motores = MotoresDeMentira {
        sin_modelo: Some(("diagnostico".to_string(), "no cabe".to_string())),
        ..MotoresDeMentira::default()
    };

    let (_, vistos) = pasar_mirando(&carpeta, &mut motores);

    assert_eq!(vistos.len(), 1);
    assert_eq!(cerradas(&vistos[0]), 2, "{:?}", vistos[0]);
}

/// Motores que, cada vez que preparan un modelo con otro ya cargado, apuntan las fichas que hay en
/// el disco en ese momento.
fn mirar_el_disco_al_cambiar(
    carpeta: &Path,
    cargado: Option<&str>,
) -> (
    std::rc::Rc<std::cell::RefCell<Vec<Vec<FichaDeEncargo>>>>,
    MotoresDeMentira,
) {
    let vistos = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let anotar = std::rc::Rc::clone(&vistos);
    let carpeta = carpeta.to_path_buf();
    let motores = MotoresDeMentira {
        cargado: cargado.map(str::to_string),
        al_cambiar: Some(Box::new(move || {
            let fichas = disco::leer(&carpeta_de_los_encargos(&carpeta, "Programator"))
                .unwrap()
                .fichas;
            anotar.borrow_mut().push(fichas);
        })),
        ..MotoresDeMentira::default()
    };
    (vistos, motores)
}

fn cerradas(fichas: &[FichaDeEncargo]) -> usize {
    fichas
        .iter()
        .filter(|ficha| !ficha.estado.esta_abierto())
        .count()
}

#[test]
fn si_falla_la_publicacion_el_encargo_sigue_en_la_cola_con_un_intento_mas() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(
        &carpeta,
        &[("uno", Some("formas")), ("dos", Some("diagnostico"))],
    );
    // Al cambiar al segundo modelo, el buzón propio deja de poder escribirse: el segundo
    // encargo se atiende pero su respuesta no sale.
    let buzon = carpeta.join(".gestor/canal/programator.md");
    let mut motores = MotoresDeMentira {
        al_cambiar: Some(Box::new(move || {
            let _ = std::fs::remove_file(&buzon);
            std::fs::create_dir_all(&buzon).unwrap();
        })),
        ..MotoresDeMentira::default()
    };

    let resumen = resumen(pasar(&carpeta, &mut motores));

    assert_eq!(resumen.atendidos, 1);
    assert_eq!(resumen.fallidos, 1);
    let en_cola = cola(&carpeta);
    assert_eq!(en_cola.len(), 1);
    assert!(en_cola[0].texto.contains("dos"));
    assert_eq!(en_cola[0].intentos, 1);
}

/// Todo lo que hay escrito en `.gestor/`, para contar avisos aunque el buzón se haya rotado.
fn todo_lo_escrito(carpeta: &Path) -> String {
    fn recorrer(ruta: &Path, texto: &mut String) {
        for entrada in std::fs::read_dir(ruta).unwrap().filter_map(|e| e.ok()) {
            let ruta = entrada.path();
            if ruta.is_dir() {
                recorrer(&ruta, texto);
            } else if let Ok(contenido) = std::fs::read_to_string(&ruta) {
                texto.push_str(&contenido);
            }
        }
    }
    let mut texto = String::new();
    recorrer(&carpeta.join(".gestor"), &mut texto);
    texto
}

#[test]
fn al_llegar_a_los_intentos_el_encargo_se_cierra_con_el_motivo_y_sale_de_la_cola() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let pendiente = Encargo {
        latido: None,
        de: "claude.md".into(),
        texto: "Para Programator: dos
Clase: diagnostico"
            .into(),
        forma: crate::protocolo::encargo::Forma::Linea,
    };
    let mut anterior = Cola::default();
    anterior.anotar_fallo(&pendiente, "2026-11-22 12:00", "no cargó", 2);
    pendientes::guardar(&ruta_de_los_pendientes(&carpeta, "Programator"), &anterior).unwrap();
    let mut motores = MotoresDeMentira {
        no_carga: Some("diagnostico".to_string()),
        cargado: Some("formas".to_string()),
        ..MotoresDeMentira::default()
    };

    encargos(&carpeta, &[("uno", Some("formas"))]);
    resumen(pasar(&carpeta, &mut motores));

    assert!(cola(&carpeta).is_empty(), "{:?}", cola(&carpeta));
    let escrito = todo_lo_escrito(&carpeta);
    assert_eq!(escrito.matches("Queda cerrado").count(), 1, "{escrito}");
    assert!(escrito.contains("no terminó de cargar"), "{escrito}");
}

#[test]
fn sin_modelo_utilizable_el_encargo_se_cierra_en_el_acto_y_no_entra_en_la_cola() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let mut motores = MotoresDeMentira {
        sin_modelo: Some((POR_DEFECTO.to_string(), "falta el fichero".to_string())),
        ..MotoresDeMentira::default()
    };

    encargos(&carpeta, &[("uno", None)]);
    let resumen = resumen(pasar(&carpeta, &mut motores));

    assert_eq!(resumen.pendientes, 0);
    assert!(cola(&carpeta).is_empty());
    assert!(motores.cargas.is_empty(), "no se carga nada");
    let buzon = buzon_propio(&carpeta);
    assert!(buzon.contains("falta el fichero"), "{buzon}");
    assert!(buzon.contains("revisión: buscar"), "{buzon}");
    assert!(
        buzon.contains("en la carpeta donde está instalado Programator"),
        "{buzon}"
    );
}

/// Un encargo que ya estaba en la cola y cuyo modelo ya no se puede usar también se cierra y sale
/// de ella.
#[test]
fn un_pendiente_cuyo_modelo_ya_no_se_puede_usar_se_cierra_y_sale_de_la_cola() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let pendiente = Encargo {
        latido: None,
        de: "claude.md".into(),
        texto: "Para Programator: dos".into(),
        forma: crate::protocolo::encargo::Forma::Linea,
    };
    let mut anterior = Cola::default();
    anterior.anotar_fallo(&pendiente, "2026-11-22 12:00", "no cargó", 2);
    pendientes::guardar(&ruta_de_los_pendientes(&carpeta, "Programator"), &anterior).unwrap();
    let mut motores = MotoresDeMentira {
        sin_modelo: Some((POR_DEFECTO.to_string(), "falta el fichero".to_string())),
        ..MotoresDeMentira::default()
    };

    pasar(&carpeta, &mut motores);

    assert!(cola(&carpeta).is_empty(), "{:?}", cola(&carpeta));
    assert!(buzon_propio(&carpeta).contains("falta el fichero"));
}

#[test]
fn sin_modelo_para_una_clase_las_demas_se_atienden() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let mut motores = MotoresDeMentira {
        sin_modelo: Some(("diagnostico".to_string(), "no cabe".to_string())),
        ..MotoresDeMentira::default()
    };

    encargos(
        &carpeta,
        &[("uno", Some("diagnostico")), ("dos", Some("formas"))],
    );
    let resumen = resumen(pasar(&carpeta, &mut motores));

    assert_eq!(resumen.atendidos, 1);
    assert!(cola(&carpeta).is_empty());
}

#[test]
fn si_falla_el_motor_del_primer_grupo_sus_encargos_entran_en_la_cola() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let mut motores = MotoresDeMentira {
        falla: Some((POR_DEFECTO.to_string(), "el motor se cerró".to_string())),
        ..MotoresDeMentira::default()
    };

    encargos(&carpeta, &[("uno", None)]);
    let primera = pasar_sin_unwrap(&carpeta, &mut motores);
    assert!(primera.is_ok(), "{primera:?}");
    assert_eq!(cola(&carpeta).len(), 1);
    assert_eq!(cola(&carpeta)[0].intentos, 1);

    pasar(&carpeta, &mut motores);
    assert!(cola(&carpeta).is_empty(), "al segundo, cerrado");
    let escrito = todo_lo_escrito(&carpeta);
    assert!(escrito.contains("el motor se cerró"), "{escrito}");
    assert_eq!(escrito.matches("Queda cerrado").count(), 1, "{escrito}");
}

/// Uso real del 26/11: un encargo que espera en la cola y se cancela mientras se atiende el de
/// delante no llegó a empezar. Su respuesta es la de lo cancelado sin atender, no la de lo cortado
/// en curso, y no lleva la línea de la memoria, porque no se usó el modelo para él.
#[test]
fn cancelar_un_encargo_de_la_cola_mientras_se_atiende_otro_lo_da_por_no_empezado() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let ruta_buzon = carpeta.join(".gestor/canal/codex.md");
    std::fs::write(
        &ruta_buzon,
        "# Bandeja de Codex\n\nLATIDO: 2026-11-25 10:00:00\nPara Programator: PG-3 escribir la cola\nPara Programator: PG-4 escribir la pila\n",
    )
    .unwrap();
    let motor = super::pruebas::MotorQueCancelaAlResponder {
        doble: MotorDoble::con_guion(vec![Respuesta::Texto("la cola, hecha".to_string())]),
        ruta: ruta_buzon,
        cancelacion: Some(
            "\nLATIDO: 2026-11-25 10:00:40\nPara Programator: cancelar: PG-4\n".to_string(),
        ),
    };
    let mut motores = MotoresDeMentira {
        memoria: Some("Memoria: de prueba · 40 capas".to_string()),
        motor_a_medida: Some(Box::new(motor)),
        ..MotoresDeMentira::default()
    };

    pasar(&carpeta, &mut motores);

    let buzon = buzon_propio(&carpeta);
    assert!(buzon.contains("la cola, hecha"), "{buzon}");
    assert!(
        buzon.contains("Cancelado, sin atender: «escribir la pila»"),
        "{buzon}"
    );
    assert!(!buzon.contains("mientras se atendía"), "{buzon}");
    assert_eq!(buzon.matches("Memoria: de prueba").count(), 1, "{buzon}");
}

#[test]
fn cada_entrega_lleva_lo_que_cuentan_los_motores_de_la_memoria() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    encargos(&carpeta, &[("uno", None), ("dos", None)]);
    let mut motores = MotoresDeMentira {
        memoria: Some("Memoria: de prueba · 40 capas".to_string()),
        ..MotoresDeMentira::default()
    };

    resumen(pasar(&carpeta, &mut motores));

    let buzon = buzon_propio(&carpeta);
    assert_eq!(buzon.matches("Memoria: de prueba").count(), 2, "{buzon}");
}
