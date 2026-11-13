//! Funciones puras de formateo y composición de textos para el canal.
//!
//! Separa la lógica de formateo de la persistencia y del ciclo:
//! - Añade la cola de comprobación de propuestas del arnés si las hay.
//! - Añade avisos de contexto si se sobrepasa el umbral configurado.
//! - Compone el resumen de lo leído describiendo hasta qué latido se leyó cada buzón ajeno.

use crate::protocolo::ResumenLeido;

/// El aviso en la terminal de un encargo que terminó sin invocar ninguna herramienta de entrega.
pub(super) fn aviso_sin_entrega_en_la_terminal(de: &str) -> String {
    format!(
        "⚠️ Programator procesó el encargo de «{de}» pero no invocó herramientas de entrega («publicar» o «escribir_propuesta»). Se publica la respuesta directa en el canal."
    )
}

/// Obtiene el texto base del cuerpo según el desenlace del encargo.
///
/// Si el desenlace es `SinEntrega`, antepone un aviso visible y explícito
/// advirtiendo de que no se invocó ninguna herramienta de entrega (`publicar` o `escribir_propuesta`).
pub(super) fn cuerpo_base_desenlace(desenlace: &crate::arnes::ciclo::Desenlace) -> String {
    match desenlace {
        crate::arnes::ciclo::Desenlace::Publicado(cuerpo) => cuerpo.clone(),
        crate::arnes::ciclo::Desenlace::SinEntrega(texto) => {
            let aviso = "⚠️ Programator no invocó herramientas de entrega («publicar» o «escribir_propuesta»).";
            if texto.trim().is_empty() {
                format!("{aviso}\n\nEl modelo terminó la generación sin producir texto ni invocar herramientas.")
            } else {
                format!("{aviso}\n\nRespuesta directa del modelo:\n\n{texto}")
            }
        }
        crate::arnes::ciclo::Desenlace::SinPublicar { texto, propuestas } => {
            let aviso = aviso_sin_publicar(propuestas);
            if texto.trim().is_empty() {
                aviso
            } else {
                format!("{aviso}\n\nRespuesta directa del modelo:\n\n{texto}")
            }
        }
        crate::arnes::ciclo::Desenlace::Cortado { texto, propuestas } => {
            let aviso = aviso_cortado(propuestas);
            if texto.trim().is_empty() {
                aviso
            } else {
                format!("{aviso}\n\nLo que llegó a escribir:\n\n{texto}")
            }
        }
        crate::arnes::ciclo::Desenlace::Abortado(motivo)
        | crate::arnes::ciclo::Desenlace::FueraDePlazo(motivo) => motivo.clone(),
    }
}

/// El aviso de un encargo cuya última respuesta cortó el motor. Dice que no fue el modelo quien
/// cerró, y qué quedó en disco, porque quien lo lee decide con eso si repite el encargo o lo parte.
pub(super) fn aviso_cortado(propuestas: &[String]) -> String {
    let entregado = if propuestas.is_empty() {
        "No llegó a entregar nada en este encargo.".to_string()
    } else {
        let lista = propuestas
            .iter()
            .map(|p| format!("«{p}»"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("Lo que sí quedó en disco: {lista}.")
    };
    format!(
        "⚠️ El motor cortó la respuesta del modelo antes de que terminara: se acabó el contexto \
         (o el tope de tokens). No es que el modelo cerrara sin entregar. {entregado}"
    )
}

/// El aviso de un encargo que escribió propuestas y cerró sin llamar a `publicar`. Nombra lo que
/// sí quedó en disco: el arnés no niega una entrega que hizo.
pub(super) fn aviso_sin_publicar(propuestas: &[String]) -> String {
    let lista = propuestas
        .iter()
        .map(|p| format!("«{p}»"))
        .collect::<Vec<_>>()
        .join(", ");
    let (sujeto, verbo) = if propuestas.len() == 1 {
        ("la propuesta", "quedó")
    } else {
        ("las propuestas", "quedaron")
    };
    format!(
        "⚠️ Programator escribió {sujeto} {lista}, que {verbo} en disco, pero cerró el encargo \
         sin llamar a «publicar»."
    )
}

/// Añade al cuerpo que publicará el modelo el resultado de comprobar sus propuestas.
///
/// **Quién escribe qué importa aquí más que en ningún otro sitio.** El modelo redacta su cuerpo y
/// dice lo que quiera; esta cola la escribe el arnés a partir de lo que dijo un compilador. Si no
/// hubo propuestas pendientes de publicar, no se añade nada: una sección vacía en cada publicación sería ruido.
pub(super) fn con_veredictos(cuerpo: &str, veredictos: &[String]) -> String {
    if veredictos.is_empty() {
        return cuerpo.to_string();
    }

    let lista = veredictos
        .iter()
        .map(|v| format!("- {v}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{cuerpo}\n\n**Comprobación de las propuestas** (la hace el arnés, no el modelo):\n{lista}"
    )
}

/// Concatena avisos del arnés (como advertencias de ocupación de contexto) al cuerpo final.
pub(super) fn con_avisos(cuerpo: &str, avisos: &[String]) -> String {
    if avisos.is_empty() {
        return cuerpo.to_string();
    }

    let lista = avisos.join("\n\n");
    format!("{cuerpo}\n\n{lista}")
}

/// Construye el acuse de lectura de esta pasada: una entrada por cada buzón ajeno leído, con
/// hasta dónde (decisión 9).
pub(super) fn construir_resumen_leido(buzones: &[(String, String)]) -> ResumenLeido {
    ResumenLeido {
        entradas: buzones
            .iter()
            .map(|(nombre, contenido)| (nombre.clone(), hasta_donde_se_leyo(contenido)))
            .collect(),
    }
}

/// Describe hasta dónde se ha leído un buzón: hasta su último latido propio, con o sin negrita
/// (nunca uno citado con `> `, que sería uno falsificado por el modelo de otro agente), o «leído
/// entero» si no hay ninguno.
pub(super) fn hasta_donde_se_leyo(contenido: &str) -> String {
    let ultimo_latido = contenido
        .lines()
        .rev()
        .find_map(crate::protocolo::tras_la_marca_de_latido);

    match ultimo_latido {
        Some(resto) => {
            let hora = resto.split(" — ").next().unwrap_or(resto).trim();
            format!("hasta su latido {hora}")
        }
        None => "leído entero".to_string(),
    }
}

/// Redacta el aviso emitido cuando no se pudo publicar el desenlace de un encargo en el canal.
pub(super) fn mensaje_fallo_publicar_desenlace(
    solicitante: &str,
    fallo: &crate::error::Error,
) -> String {
    format!(
        "⚠️ No se pudo publicar el desenlace del encargo de «{solicitante}» en el canal: {fallo}.\n\
         Consecuencia: las propuestas generadas SÍ quedan a salvo en disco en la carpeta de candidatos, \
         pero «{solicitante}» no recibirá la notificación en su buzón. El registro no avanzará y el \
         encargo volverá a atenderse en el próximo ciclo.\n\
         Qué hacer: comprueba que el archivo del canal y el buzón no estén bloqueados por otro proceso \
         o protegidos contra escritura."
    )
}

/// ¿Debe el fallo al publicar el **cierre** de un encargo impedir que el registro avance?
///
/// Si el encargo entregó al menos una propuesta y todos sus veredictos ya habían salido al canal
/// por la vía inmediata (`Canal::publicar_entrega` al escribir la propuesta, visto en uso
/// real), el fallo se queda en el resumen de cierre: la entrega ya quedó anotada. Reatender
/// el encargo solo produciría una segunda respuesta que contradice a la primera —exactamente lo que
/// se vio en uso real, dos pasadas sobre el mismo encargo 027-bis, la segunda sin
/// entrega y peor que la primera. Si no hubo ninguna propuesta (p. ej. `SinEntrega` o `Abortado`), o
/// si queda algún veredicto sin publicar, el fallo sí pierde información real y hay que reintentar.
pub(super) fn fallo_de_cierre_pierde_el_encargo(
    propuestas_de_este_encargo: usize,
    veredictos_pendientes: usize,
) -> bool {
    propuestas_de_este_encargo == 0 || veredictos_pendientes > 0
}

/// Redacta el aviso emitido cuando el cierre de un encargo no se pudo publicar, pero su entrega
/// ya había quedado anotada en el canal y por eso el registro avanza de todos modos.
pub(super) fn mensaje_fallo_publicar_cierre_con_entrega_ya_registrada(
    solicitante: &str,
    fallo: &crate::error::Error,
) -> String {
    format!(
        "⚠️ No se pudo publicar el resumen de cierre del encargo de «{solicitante}» en el canal: {fallo}.\n\
         Consecuencia: la propuesta ya había quedado anotada en el canal al entregarse, así que no se \
         pierde trabajo; solo falta el resumen final. El registro avanza igual: el encargo no se \
         reatenderá, para no dar una segunda respuesta que contradiga la primera.\n\
         Qué hacer: no hace falta nada; si el fallo persiste en el siguiente ciclo, comprueba que el \
         archivo del canal no esté bloqueado por otro proceso o protegido contra escritura."
    )
}

/// Redacta el aviso de un encargo de revisión cuyo resultado no se pudo publicar. El registro
/// avanza igual, porque lo hecho está en disco y repetirlo serían horas de banco.
pub(super) fn mensaje_fallo_publicar_revision(
    solicitante: &str,
    fallo: &crate::error::Error,
) -> String {
    format!(
        "⚠️ No se pudo publicar en el canal el resultado del encargo de revisión de «{solicitante}»: {fallo}.\n\
         Consecuencia: lo hecho está en disco, en la carpeta del mes de `revisiones/` (`probados.toml`, \
         la propuesta) y en las tandas del banco. El encargo no se repite, para no volver a descargar \
         ni a pasar el banco.\n\
         Qué hacer: comprueba que el archivo del canal no esté bloqueado o protegido contra escritura, \
         y pide al que encargó la revisión que mire esa carpeta."
    )
}

/// Redacta el aviso de una reserva que no se pudo quitar de `estado.md` al cerrar un encargo.
pub(super) fn mensaje_fallo_liberar_reserva(linea: &str, fallo: &crate::error::Error) -> String {
    format!(
        "⚠️ No se pudo quitar de estado.md la reserva «{linea}»: {fallo}.\n\
         Consecuencia: el encargo está cerrado, pero la reserva sigue anotada y los demás \
         agentes la verán como viva.\n\
         Qué hacer: comprueba que estado.md no esté bloqueado o protegido contra escritura y borra \
         esa línea a mano de «## Reservas activas»."
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_veredicto_se_publica_aunque_el_modelo_diga_otra_cosa() {
        // Esto es el objetivo entero de la puerta: en la evaluación del 22/09/2026 el modelo
        // publicó «he entregado la solución» dos veces con ficheros que no ejecutaban. El cuerpo
        // lo redacta él; esta cola la escribe el arnés a partir de lo que dijo un compilador.
        let cuerpo = con_veredictos(
            "He entregado la solución, ya está lista para revisión.",
            &["`cola.lua`: ❌ NO pasa la comprobación".to_string()],
        );

        assert!(cuerpo.contains("He entregado la solución"), "{cuerpo}");
        assert!(cuerpo.contains("NO pasa la comprobación"), "{cuerpo}");
        assert!(
            cuerpo.contains("la hace el arnés, no el modelo"),
            "quien lee el canal tiene que saber quién firma cada parte: {cuerpo}"
        );
    }

    #[test]
    fn sin_propuestas_no_se_ensucia_la_publicacion() {
        let cuerpo = con_veredictos("Respondido sin escribir ficheros.", &[]);

        assert_eq!(cuerpo, "Respondido sin escribir ficheros.");
    }

    #[test]
    fn hasta_donde_se_leyo_sin_latido_dice_leido_entero() {
        assert_eq!(
            hasta_donde_se_leyo("# Claude\ncontenido sin latido\n"),
            "leído entero"
        );
    }

    #[test]
    fn hasta_donde_se_leyo_toma_el_ultimo_latido_no_citado() {
        let contenido = "**LATIDO:** 2026-01-01 00:00 — viejo\n\n\
                          > **LATIDO:** 1999-01-01 00:00 — falso, citado\n\n\
                          **LATIDO:** 2026-09-14 12:00 — el de verdad\n";
        assert_eq!(
            hasta_donde_se_leyo(contenido),
            "hasta su latido 2026-09-14 12:00"
        );
    }

    #[test]
    fn hasta_donde_se_leyo_reconoce_el_latido_del_canal() {
        let contenido = "LATIDO: 2026-11-13 12:22:06\n\nPara Gemini 1: algo.\n\n\
                          > LATIDO: 1999-01-01 00:00:00\n\n\
                          LATIDO: 2026-11-13 13:19:25\n\nOtra cosa.\n";
        assert_eq!(
            hasta_donde_se_leyo(contenido),
            "hasta su latido 2026-11-13 13:19:25"
        );
    }

    #[test]
    fn mensaje_fallo_publicar_desenlace_explica_fallo_consecuencia_y_accion() {
        let err = crate::error::Error::Escritura {
            ruta: std::path::PathBuf::from(".gestor/canal/programator.md"),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "bloqueado"),
        };
        let texto = mensaje_fallo_publicar_desenlace("claude.md", &err);
        assert!(texto.starts_with("⚠️ No se pudo publicar el desenlace"));
        assert!(texto.contains("«claude.md»"));
        assert!(texto.contains("Consecuencia:"));
        assert!(texto.contains("las propuestas generadas SÍ quedan a salvo en disco"));
        assert!(texto.contains("Qué hacer:"));
    }

    #[test]
    fn cuerpo_base_desenlace_publicado_devuelve_cuerpo_tal_cual() {
        let desenlace = crate::arnes::ciclo::Desenlace::Publicado("Solución lista.".to_string());
        assert_eq!(cuerpo_base_desenlace(&desenlace), "Solución lista.");
    }

    #[test]
    fn cuerpo_base_desenlace_sin_entrega_antepone_aviso_visible() {
        let desenlace = crate::arnes::ciclo::Desenlace::SinEntrega("Texto directo".to_string());
        let cuerpo = cuerpo_base_desenlace(&desenlace);
        assert!(cuerpo.starts_with("⚠️ Programator no invocó herramientas de entrega"));
        assert!(cuerpo.contains("Respuesta directa del modelo:\n\nTexto directo"));

        let desenlace_vacio = crate::arnes::ciclo::Desenlace::SinEntrega(String::new());
        let cuerpo_vacio = cuerpo_base_desenlace(&desenlace_vacio);
        assert!(cuerpo_vacio.contains("sin producir texto ni invocar herramientas"));
    }

    #[test]
    fn cerrar_con_texto_tras_escribir_nombra_la_propuesta_y_no_niega_la_entrega() {
        let desenlace = crate::arnes::ciclo::Desenlace::SinPublicar {
            texto: "Ya está.".to_string(),
            propuestas: vec!["cola.lua".to_string()],
        };
        let cuerpo = cuerpo_base_desenlace(&desenlace);
        assert!(cuerpo.contains("«cola.lua»"), "{cuerpo}");
        assert!(cuerpo.contains("sin llamar a «publicar»"), "{cuerpo}");
        assert!(!cuerpo.contains("no invocó herramientas"), "{cuerpo}");
        assert!(cuerpo.contains("Respuesta directa del modelo:\n\nYa está."));
    }

    #[test]
    fn una_respuesta_cortada_dice_que_la_corto_el_motor_y_no_que_no_entrego() {
        let desenlace = crate::arnes::ciclo::Desenlace::Cortado {
            texto: String::new(),
            propuestas: Vec::new(),
        };
        let cuerpo = cuerpo_base_desenlace(&desenlace);
        assert!(cuerpo.contains("El motor cortó la respuesta"), "{cuerpo}");
        assert!(cuerpo.contains("se acabó el contexto"), "{cuerpo}");
        assert!(cuerpo.contains("No llegó a entregar nada"), "{cuerpo}");
        assert!(!cuerpo.contains("no invocó herramientas"), "{cuerpo}");
    }

    #[test]
    fn una_respuesta_cortada_nombra_lo_que_quedo_y_lo_que_llego_a_escribir() {
        let desenlace = crate::arnes::ciclo::Desenlace::Cortado {
            texto: "Ahora repaso".to_string(),
            propuestas: vec!["cola.lua".to_string()],
        };
        let cuerpo = cuerpo_base_desenlace(&desenlace);
        assert!(
            cuerpo.contains("Lo que sí quedó en disco: «cola.lua»."),
            "{cuerpo}"
        );
        assert!(
            cuerpo.contains("Lo que llegó a escribir:\n\nAhora repaso"),
            "{cuerpo}"
        );
    }

    #[test]
    fn el_aviso_sin_publicar_con_varias_propuestas_las_nombra_todas() {
        let aviso = aviso_sin_publicar(&["a.rs".to_string(), "a-002.rs".to_string()]);
        assert!(
            aviso.contains("las propuestas «a.rs», «a-002.rs»"),
            "{aviso}"
        );
        assert!(aviso.contains("quedaron en disco"), "{aviso}");
    }

    #[test]
    fn un_fallo_de_cierre_sin_propuestas_de_este_encargo_pierde_el_encargo() {
        assert!(fallo_de_cierre_pierde_el_encargo(0, 0));
    }

    #[test]
    fn un_fallo_de_cierre_con_veredictos_sin_publicar_pierde_el_encargo() {
        assert!(fallo_de_cierre_pierde_el_encargo(1, 1));
    }

    #[test]
    fn un_fallo_de_cierre_con_la_propuesta_ya_anotada_no_pierde_el_encargo() {
        assert!(!fallo_de_cierre_pierde_el_encargo(1, 0));
    }

    #[test]
    fn mensaje_fallo_publicar_cierre_con_entrega_ya_registrada_explica_que_no_se_pierde_nada() {
        let err = crate::error::Error::Escritura {
            ruta: std::path::PathBuf::from(".gestor/canal/programator.md"),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "bloqueado"),
        };
        let texto = mensaje_fallo_publicar_cierre_con_entrega_ya_registrada("claude.md", &err);
        assert!(texto.contains("«claude.md»"));
        assert!(texto.contains("no se pierde trabajo"));
        assert!(texto.contains("no se reatenderá"));
        assert!(texto.contains("Qué hacer:"));
    }

    #[test]
    fn cuerpo_base_desenlace_abortado_devuelve_motivo() {
        let desenlace =
            crate::arnes::ciclo::Desenlace::Abortado("🔴 Abortado por límite".to_string());
        assert_eq!(cuerpo_base_desenlace(&desenlace), "🔴 Abortado por límite");
    }
}
