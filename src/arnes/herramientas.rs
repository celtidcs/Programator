//! El repertorio cerrado de acciones.
//!
//! El modelo nunca escribe un fichero: solicita, y aquí se concede o se deniega. Lo que no figura en
//! `Repertorio::nombres()` no puede hacerse. En particular no existe —ni existirá en la v1— ninguna herramienta
//! para firmar, aprobar o aceptar: el §1.1 del protocolo dice que nada se da por bueno porque lo
//! diga quien lo hizo, y Programator no puede violarlo porque no dispone del verbo.

mod candidatos;
mod error;
mod lectura;

use self::candidatos::siguiente_libre;
use self::error::motivo_para_el_modelo;
use self::lectura::preparar_lectura;
use super::verificacion::{verificar, Comprobadores, Veredicto};
use super::Ambito;
use crate::motor::SolicitudHerramienta;
use crate::protocolo::{Canal, ResumenLeido};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

/// Resultado de atender una solicitud.
#[derive(Debug, Clone)]
pub enum Decision {
    /// Concedida, con lo que se le devuelve al modelo.
    Concedida(String),
    /// Denegada, con el motivo que se le explica al modelo.
    Denegada(String),
}

/// Todo lo que el repertorio **reconoce**, derivado de la ficha.
///
/// Se calcula una vez y se reutiliza: la firma sigue siendo `&'static [&'static str]` para que
/// nada de lo que ya llamaba a `nombres()` tenga que cambiar.
fn nombres_reconocidos() -> &'static [&'static str] {
    static CACHE: OnceLock<Vec<&'static str>> = OnceLock::new();
    CACHE
        .get_or_init(|| super::ficha::todas().iter().map(|f| f.nombre).collect())
        .as_slice()
}

/// Lo que hoy puede concederse, y por tanto lo único que se le **declara** al modelo.
///
/// Reconocer no es conceder. Lo que se deniega siempre no se anuncia: anunciarlo solo sirve para
/// que el modelo queme solicitudes, porque insistir dos veces en la misma aborta el encargo (§2.1)
/// y alternarlas agota el tope sin entregar nada. Qué es concedible lo dice `ficha::Estado`, que es
/// también donde está escrita la condición para que `verificar` vuelva.
fn nombres_concedidos() -> &'static [&'static str] {
    static CACHE: OnceLock<Vec<&'static str>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            super::ficha::todas()
                .iter()
                .filter(|f| f.es_concedible())
                .map(|f| f.nombre)
                .collect()
        })
        .as_slice()
}

const VERIFICACIONES: &[&str] = &["formato", "clippy", "pruebas", "estado_git", "diferencias"];

/// Tope de bytes de fichero que se le entrega al modelo de una sola lectura, si la configuración no
/// dice otra cosa.
///
/// Un buzón real puede pasar de 260 KB: devolver el fichero entero desborda la ventana y mata el
/// encargo de golpe, sin diagnóstico. 16 KiB dejan sitio a las normas, a la conversación y a las
/// demás solicitudes del encargo con la ventana por defecto, que son **16.384 tokens** desde el
/// 22/09/2026 (`config::contexto_por_defecto`). Quien cambie la ventana en el TOML probablemente
/// quiera cambiar también este tope, y por eso es configurable.
///
/// Cuando se recorta se le dice al modelo, con el tamaño real: leer de menos sabiéndolo es
/// recuperable, creer que has leído todo no lo es.
///
/// Se deriva de `config::tope_lectura_por_defecto`, que es la única fuente de la cifra: este valor
/// solo se usa cuando nadie ha construido el arnés pasando el del TOML (todo camino real, desde
/// `sesion.rs`, sí lo pasa). Repetir el `16 * 1024` aquí y allá dejó de ser aceptable en la 0.10.0:
/// es la misma fuente múltiple que el resto de la versión elimina.
const TOPE_LECTURA_POR_DEFECTO: usize = crate::config::tope_lectura_por_defecto();

pub struct Repertorio {
    ambito: Ambito,
    canal: Canal,
    /// Cuerpo que el modelo quiere publicar, si ya lo ha entregado.
    pendiente_de_publicar: Option<String>,
    /// Con qué se comprueba cada extensión. Vacío es «no se verifica nada», que es el
    /// comportamiento de siempre.
    comprobadores: Comprobadores,
    /// Cuánto se espera a un comprobador antes de cortarlo.
    tiempo_de_verificacion: Duration,
    /// Qué salió de verificar cada propuesta de esta pasada, en orden. Lo lee `sesion` para dejar
    /// constancia en el canal: **el cuerpo lo redacta el modelo, el veredicto no**.
    veredictos: Vec<String>,
    /// Nombres reales de los ficheros de propuesta escritos en disco durante esta pasada, en orden.
    propuestas: Vec<String>,
    /// Tope de bytes que se le entregan al modelo en una lectura de fichero.
    tope_lectura: usize,
    /// Resumen de los buzones leídos al inicio de la pasada, para notificar entregas al canal.
    leido: ResumenLeido,
    /// Cuántos veredictos ya han sido publicados inmediatamente al entregar propuesta.
    veredictos_publicados: usize,
}

/// Lo que se espera a un comprobador si el TOML no dice otra cosa.
///
/// Se deriva de `config::tiempo_verificacion_por_defecto()`, que es la única fuente de la cifra.
const TIEMPO_VERIFICACION_POR_DEFECTO: Duration =
    Duration::from_secs(crate::config::tiempo_verificacion_por_defecto());

impl Repertorio {
    pub fn nuevo(ambito: Ambito, canal: Canal) -> Self {
        Self {
            ambito,
            canal,
            pendiente_de_publicar: None,
            comprobadores: Comprobadores::new(),
            tiempo_de_verificacion: TIEMPO_VERIFICACION_POR_DEFECTO,
            veredictos: Vec::new(),
            propuestas: Vec::new(),
            tope_lectura: TOPE_LECTURA_POR_DEFECTO,
            leido: ResumenLeido::vacio(),
            veredictos_publicados: 0,
        }
    }

    /// Pone la puerta de verificación: con qué se comprueba cada extensión y cuánto se espera.
    ///
    /// Se construye aparte de `nuevo` a propósito: sin esto, el repertorio se comporta exactamente
    /// como antes de que la puerta existiera, y las pruebas que no van de verificación no tienen
    /// que saber nada de ella.
    pub fn con_verificacion(mut self, comprobadores: Comprobadores, tiempo: Duration) -> Self {
        self.comprobadores = comprobadores;
        self.tiempo_de_verificacion = tiempo;
        self
    }

    /// Establece el tope de bytes para lecturas de fichero desde la configuración.
    ///
    /// Si no se llama, se usa el valor por defecto `TOPE_LECTURA_POR_DEFECTO`.
    pub fn con_tope_de_lectura(mut self, bytes: usize) -> Self {
        self.tope_lectura = bytes;
        self
    }

    /// Establece el resumen de lo leído para publicar entregas en el canal de inmediato.
    pub fn con_leido(mut self, leido: ResumenLeido) -> Self {
        self.leido = leido;
        self
    }

    /// Lo que salió de comprobar las propuestas de esta pasada, para dejarlo escrito en el canal.
    pub fn veredictos(&self) -> &[String] {
        &self.veredictos
    }

    /// Cuántos veredictos de propuestas ya han sido publicados al momento de entrega.
    pub fn veredictos_publicados(&self) -> usize {
        self.veredictos_publicados
    }

    /// Veredictos de propuestas comprobadas que aún no han sido publicados en el canal.
    pub fn veredictos_no_publicados(&self) -> &[String] {
        if self.veredictos_publicados < self.veredictos.len() {
            &self.veredictos[self.veredictos_publicados..]
        } else {
            &[]
        }
    }

    /// Marca todos los veredictos acumulados hasta el momento como publicados.
    pub fn marcar_todos_veredictos_publicados(&mut self) {
        self.veredictos_publicados = self.veredictos.len();
    }

    /// Los nombres reales de los ficheros de propuesta escritos en disco durante esta pasada.
    pub(super) fn propuestas(&self) -> &[String] {
        &self.propuestas
    }

    /// Los nombres de todo lo que el repertorio reconoce. No hay nada más: lo que no está aquí no
    /// puede hacerse. Reconocido no es lo mismo que concedible —para eso está
    /// `nombres_concedibles`—, y cambiarle el sentido a esta rompería a quien la use para
    /// comprobar qué verbos existen.
    pub fn nombres() -> &'static [&'static str] {
        nombres_reconocidos()
    }

    /// Los nombres que se le declaran al modelo como su repertorio: solo lo que hoy puede
    /// concederse. Deja fuera `buscar` y `proponer_poda`, que se deniegan siempre.
    pub fn nombres_concedibles() -> &'static [&'static str] {
        nombres_concedidos()
    }

    /// Lo que el modelo dejó listo para publicar, si lo hizo.
    pub fn pendiente_de_publicar(&self) -> Option<&str> {
        self.pendiente_de_publicar.as_deref()
    }

    /// Atiende una solicitud del modelo.
    pub fn atender(&mut self, solicitud: &SolicitudHerramienta) -> Decision {
        if !nombres_reconocidos().contains(&solicitud.nombre.as_str()) {
            return Decision::Denegada(format!(
                "«{}» no está en el repertorio. Solo puedes usar: {}",
                solicitud.nombre,
                nombres_reconocidos().join(", ")
            ));
        }

        match solicitud.nombre.as_str() {
            "leer_fichero" => self.leer_fichero(solicitud),
            "listar" => self.listar(solicitud),
            "buscar" => Decision::Denegada("«buscar» llega en la v1.1".to_string()),
            "verificar" => self.verificar(solicitud),
            "publicar" => self.publicar(solicitud),
            "escribir_propuesta" => self.escribir_propuesta(solicitud),
            "proponer_poda" => {
                Decision::Denegada("«proponer_poda» la invoca el arnés, no el modelo".to_string())
            }
            "reservar" => self.reservar(solicitud),
            _ => Decision::Denegada("solicitud no reconocida".to_string()),
        }
    }

    /// Saca un argumento de la solicitud, o deniega diciendo **qué falta y qué hay**.
    ///
    /// El modelo no tiene memoria entre encargos ni documentación de sus propios verbos: si la
    /// denegación no le da la firma, solo puede adivinar. El 23/09/2026 adivinó cinco veces
    /// seguidas y costó un encargo entero.
    fn argumento<'a>(&self, s: &'a SolicitudHerramienta, clave: &str) -> Result<&'a str, Decision> {
        match s.argumentos.get(clave) {
            None => Err(Decision::Denegada(format!(
                "falta el argumento «{clave}». {}",
                Self::recordatorio_de_firma(&s.nombre)
            ))),
            Some(valor) => valor.as_str().ok_or_else(|| {
                Decision::Denegada(format!(
                    "el argumento «{clave}» debe ser una cadena de texto. {}",
                    Self::recordatorio_de_firma(&s.nombre)
                ))
            }),
        }
    }

    /// La firma del verbo y para qué sirve cada argumento, en una frase.
    ///
    /// Si el verbo no estuviera en la ficha —cosa que `atender` ya impide— se calla en vez de
    /// inventar: una firma falsa sería peor que ninguna.
    fn recordatorio_de_firma(nombre: &str) -> String {
        let Some(ficha) = crate::arnes::ficha::buscar(nombre) else {
            return String::new();
        };
        let mut texto = format!("La firma es {}.", ficha.firma());
        for argumento in ficha.obligatorios.iter().chain(ficha.opcionales) {
            texto.push_str(&format!(" «{}»: {}.", argumento.nombre, argumento.para_que));
        }
        texto
    }

    fn leer_fichero(&self, s: &SolicitudHerramienta) -> Decision {
        let ruta = match self.argumento(s, "ruta") {
            Ok(r) => r,
            Err(d) => return d,
        };
        match self.ambito.resolver(ruta) {
            Err(e) => Decision::Denegada(motivo_para_el_modelo(ruta, &e)),
            Ok(resuelta) => match std::fs::read_to_string(&resuelta) {
                Ok(contenido) => {
                    Decision::Concedida(preparar_lectura(ruta, &contenido, self.tope_lectura))
                }
                Err(e) => Decision::Denegada(format!("no se pudo leer «{ruta}»: {e}")),
            },
        }
    }

    fn listar(&self, s: &SolicitudHerramienta) -> Decision {
        let ruta = match self.argumento(s, "ruta") {
            Ok(r) => r,
            Err(d) => return d,
        };
        match self.ambito.resolver(ruta) {
            Err(e) => Decision::Denegada(motivo_para_el_modelo(ruta, &e)),
            Ok(resuelta) => match std::fs::read_dir(&resuelta) {
                Err(e) => Decision::Denegada(format!("no se pudo listar «{ruta}»: {e}")),
                Ok(entradas) => {
                    let nombres: Vec<String> = entradas
                        .filter_map(|e| e.ok())
                        .filter_map(|e| e.file_name().into_string().ok())
                        .collect();
                    Decision::Concedida(nombres.join("\n"))
                }
            },
        }
    }

    fn verificar(&self, s: &SolicitudHerramienta) -> Decision {
        let cual = match self.argumento(s, "cual") {
            Ok(c) => c,
            Err(d) => return d,
        };
        if !VERIFICACIONES.contains(&cual) {
            return Decision::Denegada(format!(
                "«{cual}» no está en la lista blanca de verificaciones: {}",
                VERIFICACIONES.join(", ")
            ));
        }
        // El arnés compone el comando entero: el modelo nunca aporta argumentos sueltos. Pero hoy
        // *nadie ejecuta* la verificación: `ciclo.rs` trata toda concesión igual y no mira el
        // nombre de la herramienta. Conceder aquí sería decirle al modelo que su verificación
        // pasó; él lo publicaría como hecho comprobable en un fichero que leen los otros tres
        // agentes, y el §1.1 dice que nada se da por bueno porque lo diga quien lo hizo. Mientras
        // no haya quien la ejecute, lo honrado es denegar.
        Decision::Denegada(format!(
            "la verificación «{cual}» todavía no la ejecuta nadie: el arnés aún no encadena el \
             ciclo completo, así que no hay resultado que darte. Llegará cuando lo encadene. No \
             publiques como verificado nada que no se haya ejecutado de verdad."
        ))
    }

    fn publicar(&mut self, s: &SolicitudHerramienta) -> Decision {
        let texto = match self.argumento(s, "texto") {
            Ok(t) => t.to_string(),
            Err(d) => return d,
        };
        self.pendiente_de_publicar = Some(texto);
        Decision::Concedida("anotado; el arnés lo publicará al cerrar el ciclo".to_string())
    }

    fn escribir_propuesta(&mut self, s: &SolicitudHerramienta) -> Decision {
        let nombre = match self.argumento(s, "nombre") {
            Ok(n) => n,
            Err(d) => return d,
        };
        let contenido = match self.argumento(s, "contenido") {
            Ok(c) => c,
            Err(d) => return d,
        };

        let relativa = format!(".gestor/candidatos/programator/{nombre}");
        let resuelta = match self.ambito.resolver(&relativa) {
            Ok(r) => r,
            Err(e) => return Decision::Denegada(motivo_para_el_modelo(nombre, &e)),
        };

        // Doble cinturón: además del ámbito, la ruta debe seguir bajo el directorio de candidatos.
        let permitido = match self.ambito.resolver(".gestor/candidatos/programator") {
            Ok(p) => p,
            Err(e) => {
                return Decision::Denegada(motivo_para_el_modelo(
                    ".gestor/candidatos/programator",
                    &e,
                ))
            }
        };
        if !resuelta.starts_with(&permitido) {
            return Decision::Denegada(format!(
                "«{nombre}» saldría de .gestor/candidatos/programator/"
            ));
        }

        // **Nunca se borra una entrega anterior.** Si el nombre ya está ocupado se busca el
        // siguiente libre y se le dice al modelo dónde quedó. Sobrescribir costó que en el encargo
        // 006 se revisara una versión distinta de la que estaba en disco.
        let resuelta = siguiente_libre(&resuelta);

        if let Some(padre) = resuelta.parent() {
            if let Err(e) = std::fs::create_dir_all(padre) {
                return Decision::Denegada(format!("no se pudo crear el directorio: {e}"));
            }
        }
        if let Err(e) = std::fs::write(&resuelta, contenido) {
            return Decision::Denegada(format!("no se pudo escribir: {e}"));
        }

        // La propuesta ya está en disco, **pase lo que pase con la comprobación**. Denegar la
        // escritura por un error de compilación perdería el trabajo y gastaría intentos del tope de
        // herramientas, y una propuesta que no compila sigue sirviendo para que una persona vea por
        // dónde iba.
        let veredicto = self.verificar_propuesta(&resuelta);

        // El nombre con el que quedó **de verdad**, no el que se pidió: si `siguiente_libre` movió
        // la entrega a `calculo-002.rs`, tanto el veredicto que se publica como el aviso al modelo
        // tienen que decir ese nombre. Si dijeran el pedido, quien revise buscaría el fichero
        // equivocado — el mismo fallo que costó el encargo 006.
        let nombre_real = resuelta
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| nombre.to_string());

        let veredicto_canal = veredicto.para_el_canal(&nombre_real);
        self.propuestas.push(nombre_real.clone());
        self.veredictos.push(veredicto_canal.clone());

        // Publicar de inmediato en el buzón al entregar la propuesta (incidencia C2 de NatureLand)
        let ahora = crate::protocolo::marca_de_tiempo_actual();
        if let Err(e) =
            self.canal
                .publicar_entrega(&nombre_real, &veredicto_canal, &self.leido, &ahora)
        {
            eprintln!(
                "{}",
                error::mensaje_fallo_publicar_entrega(&nombre_real, &e)
            );
        } else {
            self.veredictos_publicados += 1;
        }

        Decision::Concedida(format!(
            "escrita la propuesta «{nombre_real}» — {}",
            veredicto.para_el_modelo()
        ))
    }

    /// Pasa la propuesta por el comprobador de su extensión, si hay alguno configurado.
    ///
    /// El trabajo se hace en un directorio temporal: comprobadores como `rustc --emit=metadata`
    /// dejan artefactos, y el canal no es sitio para ellos. Si ni siquiera se puede crear ese
    /// directorio, no se verifica y se dice: **quedarse sin comprobar nunca puede tumbar un
    /// encargo**.
    fn verificar_propuesta(&self, fichero: &Path) -> Veredicto {
        if self.comprobadores.is_empty() {
            return Veredicto::SinComprobador {
                extension: fichero
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default(),
            };
        }

        // Directorio temporal propio en vez de `tempfile`: esa crate está en las dependencias de
        // desarrollo y meterla en producción por esto no compensa, cuando lo que hace falta es una
        // carpeta vacía con nombre único.
        let temporal = std::env::temp_dir().join(format!(
            "programator-verificacion-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        if let Err(e) = std::fs::create_dir_all(&temporal) {
            return Veredicto::NoSePudo {
                motivo: format!("no se pudo preparar un directorio de trabajo: {e}"),
            };
        }

        let veredicto = verificar(
            fichero,
            &self.comprobadores,
            self.tiempo_de_verificacion,
            &temporal,
        );

        // Si no se deja borrar, se sigue igual: el veredicto ya está y un temporal huérfano no es
        // motivo para tumbar un encargo.
        let _ = std::fs::remove_dir_all(&temporal);
        veredicto
    }

    fn reservar(&self, s: &SolicitudHerramienta) -> Decision {
        let ruta = match self.argumento(s, "ruta") {
            Ok(r) => r,
            Err(d) => return d,
        };
        let motivo = match self.argumento(s, "motivo") {
            Ok(m) => m,
            Err(d) => return d,
        };

        // `Ambito::resolver` actúa aquí de portero, no de traductor: solo comprueba que la ruta cae
        // dentro del ámbito (admite ficheros que todavía no existen). Lo que se anota en
        // `estado.md` es la ruta relativa tal como la escribió el modelo, nunca la absoluta que
        // devuelve el resolutor — `estado.md` es un fichero compartido y una ruta absoluta de este
        // disco ahí sería ruido y filtraría la estructura de la máquina al canal.
        if let Err(e) = self.ambito.resolver(ruta) {
            return Decision::Denegada(motivo_para_el_modelo(ruta, &e));
        }

        match crate::protocolo::anotar_reserva(
            &self.canal.ruta_estado(),
            "Programator",
            ruta,
            motivo,
        ) {
            Ok(()) => Decision::Concedida("reserva anotada en estado.md".to_string()),
            // Por la misma razón que arriba: el error lleva la ruta absoluta de `estado.md` y esa
            // ruta no puede acabar en el canal si el modelo cita la denegación.
            Err(e) => Decision::Denegada(motivo_para_el_modelo(ruta, &e)),
        }
    }
}

#[cfg(test)]
mod pruebas;
