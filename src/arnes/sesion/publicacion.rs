//! Funciones puras de formateo y composición de textos para el canal. Lo que se decide antes de
//! redactar (qué entrega pedía un encargo, si se reatiende tras un fallo de cierre) está en
//! `decisiones`.
//!
//! Separa la lógica de formateo de la persistencia y del ciclo:
//! - Añade la cola de comprobación de propuestas del arnés si las hay.
//! - Añade avisos de contexto si se sobrepasa el umbral configurado.
//! - Compone el resumen de lo leído describiendo hasta qué latido se leyó cada buzón ajeno.

use super::decisiones::{avisa_sin_entrega, EntregaPedida, VERBO_DE_PROPUESTA};
use crate::protocolo::ResumenLeido;

/// Las herramientas con que el modelo entrega, como se nombran en los avisos: «publicar» o
/// «escribir_propuesta», sacadas de sus constantes para que el texto no se quede atrás si cambian.
fn herramientas_de_entrega() -> String {
    format!(
        "«{}» o «{VERBO_DE_PROPUESTA}»",
        crate::arnes::ficha::verbo::PUBLICAR
    )
}

/// El aviso en la terminal de un encargo que terminó sin invocar ninguna herramienta de entrega.
pub(super) fn aviso_sin_entrega_en_la_terminal(de: &str) -> String {
    format!(
        "⚠️ Programator procesó el encargo de «{de}» pero no invocó herramientas de entrega ({}). Se publica la respuesta directa en el canal.",
        herramientas_de_entrega()
    )
}

/// Obtiene el texto base del cuerpo según el desenlace del encargo.
///
/// Si el desenlace es `SinEntrega` y el aviso toca ([`avisa_sin_entrega`]), antepone un aviso
/// visible de que no se invocó ninguna herramienta de entrega (`publicar` o `escribir_propuesta`).
/// Si no toca, el texto del modelo es la entrega.
pub(super) fn cuerpo_base_desenlace(
    desenlace: &crate::arnes::ciclo::Desenlace,
    pedida: EntregaPedida,
) -> String {
    match desenlace {
        crate::arnes::ciclo::Desenlace::Publicado(cuerpo) => cuerpo.clone(),
        crate::arnes::ciclo::Desenlace::SinEntrega(texto) if !avisa_sin_entrega(pedida, texto) => {
            texto.clone()
        }
        crate::arnes::ciclo::Desenlace::SinEntrega(texto) => {
            let aviso = format!(
                "⚠️ Programator no invocó herramientas de entrega ({}).",
                herramientas_de_entrega()
            );
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
        crate::arnes::ciclo::Desenlace::Cancelado {
            a_peticion_de,
            propuestas,
            ..
        } => aviso_cancelado_en_curso(a_peticion_de, propuestas),
    }
}

/// Lo que dejó un encargo que publicó, como lo cuenta el arnés y no el modelo: cada propuesta con
/// su nombre en disco y su veredicto, o que no dejó ninguna. Uso real del 26/11: la respuesta del
/// modelo nombró «fechas.py», que no compilaba, y la entrega buena era «fechas-002.py»; en otro
/// encargo publicó sin dejar el fichero que se le pedía y nadie lo dijo.
pub(super) fn lo_que_dejo_el_encargo(lineas: &[String]) -> String {
    if lineas.is_empty() {
        return "Este encargo no dejó ninguna propuesta: si pedía un fichero, no está.".to_string();
    }
    format!(
        "Lo que dejó este encargo, según el arnés: {}.",
        lineas.join("; ")
    )
}

/// El aviso de que el encargo ya se había atendido y se vuelve a atender porque cambió su texto.
pub(super) fn aviso_de_reatencion() -> String {
    "Segunda atención: el texto de este encargo cambió después de atenderlo, y se ha vuelto a \
     atender entero."
        .to_string()
}

/// La respuesta de un encargo cancelado antes de su primera llamada al modelo: no llegó a empezar.
/// Es la misma que la de una cancelación de lo que aún esperaba.
pub(super) fn aviso_cancelado_sin_empezar(nombre: &str) -> String {
    crate::arnes::cancelacion::respuesta(
        nombre,
        &crate::arnes::cancelacion::Resolucion::Cancelados(vec![nombre.to_string()]),
    )
}

/// Lo leído, con el buzón `buzon` ya leído hasta `contenido`: la respuesta a un encargo cancelado
/// leyó la cancelación, que llegó después de empezar la pasada (uso real del 26/11: su LEÍDO se
/// quedaba en el latido anterior).
pub(super) fn con_lo_leido_despues(
    leido: &ResumenLeido,
    buzon: &str,
    contenido: &str,
) -> ResumenLeido {
    let hasta = hasta_donde_se_leyo(contenido);
    let mut entradas = leido.entradas.clone();
    match entradas.iter_mut().find(|(nombre, _)| nombre == buzon) {
        Some(entrada) => entrada.1 = hasta,
        None => entradas.push((buzon.to_string(), hasta)),
    }
    ResumenLeido { entradas }
}

/// La respuesta de un encargo que se canceló mientras se atendía: quién lo pidió y lo que ya
/// había dejado en disco, que no se borra.
pub(super) fn aviso_cancelado_en_curso(a_peticion_de: &str, propuestas: &[String]) -> String {
    let dejado = if propuestas.is_empty() {
        "No llegó a dejar ninguna propuesta.".to_string()
    } else {
        let lista = crate::arnes::ciclo::lista_entre_comillas(propuestas);
        format!("Lo que ya había dejado sigue en disco, como propuestas de este encargo: {lista}.")
    };
    format!(
        "Cancelado a petición de {a_peticion_de} mientras se atendía: se cortó antes de la \
         siguiente llamada al modelo. {dejado}"
    )
}

/// El aviso de un encargo cuya última respuesta cortó el motor. Dice que no fue el modelo quien
/// cerró, y qué quedó en disco, porque quien lo lee decide con eso si repite el encargo o lo parte.
pub(super) fn aviso_cortado(propuestas: &[String]) -> String {
    let entregado = if propuestas.is_empty() {
        "No llegó a entregar nada en este encargo.".to_string()
    } else {
        let lista = crate::arnes::ciclo::lista_entre_comillas(propuestas);
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
    let lista = crate::arnes::ciclo::lista_entre_comillas(propuestas);
    let (sujeto, verbo) = if propuestas.len() == 1 {
        ("la propuesta", "quedó")
    } else {
        ("las propuestas", "quedaron")
    };
    format!(
        "⚠️ Programator escribió {sujeto} {lista}, que {verbo} en disco, pero cerró el encargo \
         sin llamar a «{}».",
        crate::arnes::ficha::verbo::PUBLICAR
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

/// Añade al cuerpo lo que el arnés denegó al modelo en este encargo. Como los veredictos, lo
/// escribe el arnés: el modelo puede reintentar con otro nombre y no contarlo. Sin denegaciones no
/// se añade nada.
pub(super) fn con_denegaciones(cuerpo: &str, denegadas: &[String]) -> String {
    if denegadas.is_empty() {
        return cuerpo.to_string();
    }
    let lista = denegadas
        .iter()
        .map(|d| format!("- {d}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{cuerpo}\n\n**Denegado por el arnés** (lo pidió el modelo y no se concedió):\n{lista}")
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
         pero «{solicitante}» no recibirá la notificación en su buzón. El encargo queda en la cola \
         de pendientes y volverá a atenderse en el próximo ciclo.\n\
         Qué hacer: comprueba que el archivo del canal y el buzón no estén bloqueados por otro proceso \
         o protegidos contra escritura."
    )
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
/// No se pudo leer el registro de entregas para contarle al modelo lo que ya entregó.
pub(super) fn mensaje_catalogo_sin_leer(fallo: &crate::error::Error) -> String {
    format!(
        "⚠️ No se pudo leer el registro de entregas ({fallo}): el modelo atiende el encargo sin \
         saber lo que ya entregó en este proyecto."
    )
}

/// No se pudo deducir en qué quedaron las propuestas: lo deducido se queda como estaba.
pub(super) fn mensaje_entregas_sin_poner_al_dia(fallo: &crate::error::Error) -> String {
    format!(
        "⚠️ No se pudo poner al día el registro de entregas: {fallo}. Lo que ya estaba anotado \
         sigue igual; se vuelve a intentar en la pasada siguiente."
    )
}

/// No se pudo publicar la respuesta a una cancelación o a una confirmación: lo hecho sigue
/// hecho.
pub(super) fn mensaje_fallo_publicar_lo_resuelto(
    solicitante: &str,
    fallo: &crate::error::Error,
) -> String {
    format!(
        "⚠️ No se pudo publicar en el canal la respuesta a la cancelación o la confirmación de \
         «{solicitante}»: {fallo}.\nConsecuencia: lo cancelado o lo confirmado queda anotado y \
         el encargo no se repite."
    )
}

/// El registro de entregas no se pudo leer o guardar al confirmar una propuesta.
/// No se pudieron borrar las copias de `preparar:` que sobran.
pub(super) fn mensaje_copias_sin_limpiar(fallo: &crate::error::Error) -> String {
    format!(
        "⚠️ No se pudieron borrar las copias de preparar que ya sobran: {fallo}. Se vuelve a \
         intentar en la pasada siguiente."
    )
}

pub(super) fn mensaje_entregas_sin_leer(fallo: &crate::error::Error) -> String {
    format!(
        "⚠️ No se ha anotado nada: el registro de entregas no se pudo leer o guardar ({fallo})."
    )
}

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

/// El aviso en la entrega cuando el encargo pedía una clase y el motor que responde no es de
/// Programator: no se ha podido cambiar de modelo.
pub(super) fn aviso_servidor_ajeno() -> String {
    "⚠️ El llama-server que responde en el puerto no lo arrancó Programator, así que no se ha \
     podido cargar el modelo de la clase pedida: este encargo se ha atendido con el modelo que \
     tenía ese servidor."
        .to_string()
}

/// Por qué queda pendiente un encargo cuyo modelo no terminó de cargar a tiempo.
pub(super) fn motivo_carga_agotada(modelo: Option<&str>) -> String {
    match modelo {
        Some(modelo) => format!("el modelo «{modelo}» no terminó de cargar a tiempo"),
        None => "el motor no terminó de cargar a tiempo".to_string(),
    }
}

/// Por qué queda pendiente un encargo cuyo motor no se pudo preparar.
pub(super) fn motivo_motor_sin_preparar(fallo: &crate::error::Error) -> String {
    format!("no se pudo preparar el motor: {fallo}")
}

/// Por qué queda pendiente un encargo cuya respuesta no se pudo publicar.
pub(super) fn motivo_sin_publicar(fallo: &crate::error::Error) -> String {
    format!("no se pudo publicar la respuesta: {fallo}")
}

/// La línea de la terminal cuando un encargo pasa a la cola de pendientes.
pub(super) fn mensaje_encargo_queda_pendiente(de: &str, motivo: &str) -> String {
    format!(
        "⏳ El encargo de «{de}» queda en la cola de pendientes ({motivo}). Se volverá a atender \
         en la próxima vuelta, antes que los nuevos."
    )
}

/// El aviso que se publica en el canal, una sola vez, cuando un encargo lleva varios intentos
/// fallidos y sigue en la cola.
pub(super) fn aviso_encargo_cerrado(intentos: u32, motivo: &str) -> String {
    format!(
        "⚠️ No he podido atender este encargo tras {intentos} intentos. El último falló porque \
         {motivo}. Queda cerrado y fuera de la cola de pendientes: revisa la causa y vuelve a \
         pedirlo."
    )
}

/// La línea de la terminal cuando un encargo se cierra tras agotar sus intentos.
pub(super) fn mensaje_encargo_cerrado(de: &str, motivo: &str) -> String {
    format!(
        "⛔ El encargo de «{de}» queda cerrado tras agotar sus intentos ({motivo}). Se le ha \
         respondido con el motivo."
    )
}

/// La respuesta a un encargo para el que no hay modelo con el que trabajar.
pub(super) fn aviso_sin_modelo(motivo: &str) -> String {
    format!(
        "⚠️ No he atendido este encargo: {motivo}. Programator no descarga nada por su cuenta. \
         La revisión de modelos busca los que caben en esta tarjeta, los descarga y los prueba en \
         el banco; se hace en la carpeta donde está instalado Programator, con Programator y tu \
         agente abiertos allí, con un encargo «{}». Este encargo queda cerrado: cuando haya un \
         modelo, vuelve a pedirlo.",
        crate::arranque::disponibles::PEDIDO_DE_BUSQUEDA
    )
}

/// La línea de la terminal cuando un encargo se cierra porque no hay modelo con el que trabajar.
pub(super) fn mensaje_encargo_sin_modelo(de: &str, motivo: &str) -> String {
    format!(
        "⛔ El encargo de «{de}» no se atiende: {motivo}. Se le ha respondido que pida «{}» en la \
         carpeta donde está instalado Programator.",
        crate::arranque::disponibles::PEDIDO_DE_BUSQUEDA
    )
}

/// La línea de la terminal cuando el aviso de un pendiente no se pudo publicar.
pub(super) fn mensaje_fallo_publicar_aviso_pendiente(fallo: &crate::error::Error) -> String {
    format!("⚠️ No se pudo publicar en el canal el aviso de un encargo pendiente: {fallo}.")
}

/// La línea de la terminal cuando la cola de pendientes no se pudo guardar: el registro de
/// lectura no avanza, para que los encargos nuevos se vuelvan a detectar.
pub(super) fn mensaje_registro_de_encargos_sin_guardar(fallo: &crate::error::Error) -> String {
    format!(
        "⚠️ No se pudo guardar el registro de encargos: {fallo}.\n\
         Consecuencia: los encargos atendidos en esta vuelta cuyo estado no se guardó siguen \
         pendientes en disco y se volverán a atender; alguno puede recibir dos respuestas.\n\
         Qué hacer: comprueba los permisos de la carpeta «encargos» del agente en `.gestor/`."
    )
}

/// La respuesta con el aviso, al final, de que su buzón no dice quién lo escribe.
pub(super) fn con_aviso_sin_identidad(cuerpo: &str, buzon: &str) -> String {
    format!(
        "{cuerpo}\n\n{}",
        crate::protocolo::respuesta::aviso_sin_identidad(buzon)
    )
}

/// [`aviso_sin_publicar`] en la terminal, con quién hizo el encargo.
pub(super) fn aviso_sin_publicar_en_la_terminal(propuestas: &[String], de: &str) -> String {
    format!("{} (encargo de «{de}»)", aviso_sin_publicar(propuestas))
}

/// [`aviso_cortado`] en la terminal, con quién hizo el encargo.
pub(super) fn aviso_cortado_en_la_terminal(propuestas: &[String], de: &str) -> String {
    format!("{} (encargo de «{de}»)", aviso_cortado(propuestas))
}

/// Lo que se dice en la terminal cuando un encargo llega de un buzón que no dice quién lo escribe.
pub(super) fn mensaje_sin_identidad(buzon: &str) -> String {
    format!(
        "⚠️ El buzón «{buzon}» no empieza por «{}<identidad>»: la respuesta se publica, pero no \
         avisará a quien encargó.",
        crate::protocolo::respuesta::CABECERA_DE_BANDEJA
    )
}

/// El aviso, al pie de la respuesta, de que se dejó una propuesta de poda para `agente`, con las
/// rutas donde se escribió.
pub(super) fn propuesta_de_poda_publicada(agente: &str, podado: &str, archivo: &str) -> String {
    format!(
        "\n\n---\n\n**Propuesta de poda para {agente}:** He dejado una versión limpia de tu buzón en \
         `{podado}` y la tarea cerrada en `{archivo}`. Puedes revisarla y aplicarla a tu buzón cuando \
         gustes."
    )
}

/// `ruta` relativa a `carpeta`, con barras normales, para citarla en el canal; tal cual si no
/// cuelga de ella.
pub(super) fn relativa_a(carpeta: &std::path::Path, ruta: &std::path::Path) -> String {
    ruta.strip_prefix(carpeta)
        .unwrap_or(ruta)
        .to_string_lossy()
        .replace('\\', "/")
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
        let fallo = crate::error::Error::Escritura {
            ruta: std::path::PathBuf::from(".gestor/canal/programator.md"),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "bloqueado"),
        };
        let texto = mensaje_fallo_publicar_desenlace("claude.md", &fallo);
        assert!(texto.starts_with("⚠️ No se pudo publicar el desenlace"));
        assert!(texto.contains("«claude.md»"));
        assert!(texto.contains("Consecuencia:"));
        assert!(texto.contains("las propuestas generadas SÍ quedan a salvo en disco"));
        assert!(texto.contains("Qué hacer:"));
    }

    #[test]
    fn sin_pedir_fichero_la_respuesta_en_texto_es_la_entrega() {
        let desenlace = crate::arnes::ciclo::Desenlace::SinEntrega("Son 42 líneas.".to_string());
        assert_eq!(
            cuerpo_base_desenlace(&desenlace, EntregaPedida::Libre),
            "Son 42 líneas."
        );

        let vacio = crate::arnes::ciclo::Desenlace::SinEntrega("  ".to_string());
        assert!(
            cuerpo_base_desenlace(&vacio, EntregaPedida::Libre).contains("no invocó herramientas")
        );
    }

    #[test]
    fn cuerpo_base_desenlace_publicado_devuelve_cuerpo_tal_cual() {
        let desenlace = crate::arnes::ciclo::Desenlace::Publicado("Solución lista.".to_string());
        assert_eq!(
            cuerpo_base_desenlace(&desenlace, EntregaPedida::Fichero),
            "Solución lista."
        );
    }

    /// Los nombres van escritos aquí a propósito, sin sus constantes: son lo que lee el agente.
    #[test]
    fn los_avisos_sin_entrega_nombran_las_dos_herramientas() {
        let nombres = "«publicar» o «escribir_propuesta»";
        let desenlace = crate::arnes::ciclo::Desenlace::SinEntrega(String::new());

        let terminal = aviso_sin_entrega_en_la_terminal("Claude 1");
        let canal = cuerpo_base_desenlace(&desenlace, EntregaPedida::Libre);

        assert!(terminal.contains(nombres), "{terminal}");
        assert!(canal.contains(nombres), "{canal}");
    }

    #[test]
    fn cuerpo_base_desenlace_sin_entrega_antepone_aviso_visible() {
        let desenlace = crate::arnes::ciclo::Desenlace::SinEntrega("Texto directo".to_string());
        let cuerpo = cuerpo_base_desenlace(&desenlace, EntregaPedida::Fichero);
        assert!(cuerpo.starts_with("⚠️ Programator no invocó herramientas de entrega"));
        assert!(cuerpo.contains("Respuesta directa del modelo:\n\nTexto directo"));

        let desenlace_vacio = crate::arnes::ciclo::Desenlace::SinEntrega(String::new());
        let cuerpo_vacio = cuerpo_base_desenlace(&desenlace_vacio, EntregaPedida::Fichero);
        assert!(cuerpo_vacio.contains("sin producir texto ni invocar herramientas"));
    }

    #[test]
    fn cerrar_con_texto_tras_escribir_nombra_la_propuesta_y_no_niega_la_entrega() {
        let desenlace = crate::arnes::ciclo::Desenlace::SinPublicar {
            texto: "Ya está.".to_string(),
            propuestas: vec!["cola.lua".to_string()],
        };
        let cuerpo = cuerpo_base_desenlace(&desenlace, EntregaPedida::Fichero);
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
        let cuerpo = cuerpo_base_desenlace(&desenlace, EntregaPedida::Fichero);
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
        let cuerpo = cuerpo_base_desenlace(&desenlace, EntregaPedida::Fichero);
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
        let en_la_terminal = aviso_sin_publicar_en_la_terminal(&["a.rs".to_string()], "Claude");
        assert!(
            en_la_terminal.ends_with(" (encargo de «Claude»)"),
            "{en_la_terminal}"
        );
        let cortado = aviso_cortado_en_la_terminal(&["a.rs".to_string()], "Claude");
        assert!(cortado.ends_with(" (encargo de «Claude»)"), "{cortado}");
        let aviso = aviso_sin_publicar(&["a.rs".to_string(), "a-002.rs".to_string()]);
        assert!(
            aviso.contains("las propuestas «a.rs», «a-002.rs»"),
            "{aviso}"
        );
        assert!(aviso.contains("quedaron en disco"), "{aviso}");
    }

    #[test]
    fn mensaje_fallo_publicar_cierre_con_entrega_ya_registrada_explica_que_no_se_pierde_nada() {
        let fallo = crate::error::Error::Escritura {
            ruta: std::path::PathBuf::from(".gestor/canal/programator.md"),
            causa: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "bloqueado"),
        };
        let texto = mensaje_fallo_publicar_cierre_con_entrega_ya_registrada("claude.md", &fallo);
        assert!(texto.contains("«claude.md»"));
        assert!(texto.contains("no se pierde trabajo"));
        assert!(texto.contains("no se reatenderá"));
        assert!(texto.contains("Qué hacer:"));
    }

    #[test]
    fn cuerpo_base_desenlace_abortado_devuelve_motivo() {
        let desenlace =
            crate::arnes::ciclo::Desenlace::Abortado("🔴 Abortado por límite".to_string());
        assert_eq!(
            cuerpo_base_desenlace(&desenlace, EntregaPedida::Fichero),
            "🔴 Abortado por límite"
        );
    }

    #[test]
    fn el_aviso_sin_identidad_de_la_terminal_nombra_el_buzon_y_la_cabecera() {
        let mensaje = mensaje_sin_identidad("claude-1.md");

        assert!(mensaje.contains("«claude-1.md»"), "{mensaje}");
        assert!(mensaje.contains("«# Bandeja de <identidad>»"), "{mensaje}");
    }
}
