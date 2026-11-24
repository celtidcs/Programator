//! Pruebas de la pasada con varios modelos: qué modelo atiende cada encargo según su línea
//! `Clase:`, cuándo se recarga y qué pasa a la cola de pendientes.

use super::pruebas::{
    buzon_propio, carpeta_de_prueba, conversaciones_registradas, escribir_buzon,
    sembrar_registro_no_vacio,
};
use super::*;
use crate::arnes::clase_pedida::{clase_pedida, mensaje_clase_desconocida, ClasePedida};
use crate::arnes::espacio::{ruta_de_los_pendientes, ruta_del_latido};
use crate::arnes::pendientes::{self, Cola};
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
            motor: Box::new(MotorDoble::con_guion(guion)),
            procedencia: self.procedencia,
        })
    }

    fn tras_el_encargo(&mut self) -> Option<String> {
        self.memoria.clone()
    }
}

fn pasar(carpeta: &Path, motores: &mut MotoresDeMentira) -> DesenlacePasada {
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

fn pasar_sin_unwrap(carpeta: &Path, motores: &mut MotoresDeMentira) -> Resultado<DesenlacePasada> {
    let limites = Limites::default();
    let verificacion = Verificacion::default();
    let ajustes = Ajustes::con_limites(&limites, &verificacion, &crate::terminal::TerminalReal);
    ejecutar_pasada_con_motores(carpeta, "Programator", &ajustes, motores, &mut SinRevision)
}

fn resumen(desenlace: DesenlacePasada) -> ResumenPasada {
    match desenlace {
        DesenlacePasada::Atendidos(resumen) => resumen,
        otro => panic!("se esperaba Atendidos: {otro:?}"),
    }
}

fn cola(carpeta: &Path) -> Cola {
    pendientes::leer(&ruta_de_los_pendientes(carpeta, "Programator")).unwrap()
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
    assert_eq!(en_cola.pendientes().len(), 1);
    assert!(en_cola.pendientes()[0].texto().contains("dos"));
    assert!(en_cola.pendientes()[0].motivo().contains("diagnostico"));

    // En la vuelta siguiente, sin nada nuevo en el canal, se atiende solo el pendiente: «uno» no
    // se repite porque el registro avanzó.
    motores.no_carga = None;
    let segunda = resumen(pasar(&carpeta, &mut motores));

    assert_eq!(segunda.atendidos, 1);
    assert!(cola(&carpeta).esta_vacia());
}

#[test]
fn un_pendiente_se_atiende_antes_que_un_encargo_nuevo_y_sale_de_la_cola() {
    let (_dir, carpeta) = carpeta_de_prueba();
    sembrar_registro_no_vacio(&carpeta);
    let mut anterior = Cola::default();
    anterior.anotar_fallo(
        &Encargo {
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
    assert!(cola(&carpeta).esta_vacia());
    let mut ficheros = conversaciones_registradas(&carpeta);
    ficheros.sort();
    let primera = std::fs::read_to_string(&ficheros[0]).unwrap();
    assert!(primera.contains("el pendiente"), "{primera}");
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
    assert_eq!(en_cola.pendientes().len(), 1);
    assert!(en_cola.pendientes()[0].texto().contains("dos"));
    assert_eq!(en_cola.pendientes()[0].intentos(), 1);
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

    assert!(
        cola(&carpeta).pendientes().is_empty(),
        "{:?}",
        cola(&carpeta)
    );
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
    assert!(cola(&carpeta).pendientes().is_empty());
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

    assert!(
        cola(&carpeta).pendientes().is_empty(),
        "{:?}",
        cola(&carpeta)
    );
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
    assert!(cola(&carpeta).pendientes().is_empty());
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
    assert_eq!(cola(&carpeta).pendientes().len(), 1);
    assert_eq!(cola(&carpeta).pendientes()[0].intentos(), 1);

    pasar(&carpeta, &mut motores);
    assert!(
        cola(&carpeta).pendientes().is_empty(),
        "al segundo, cerrado"
    );
    let escrito = todo_lo_escrito(&carpeta);
    assert!(escrito.contains("el motor se cerró"), "{escrito}");
    assert_eq!(escrito.matches("Queda cerrado").count(), 1, "{escrito}");
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
