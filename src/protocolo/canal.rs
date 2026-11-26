//! Acceso al canal `.gestor/canal/`. Este módulo sabe qué ficheros existen y de quién es cada uno;
//! no decide qué escribir en ellos.

use super::carpetas::{carpeta_del_canal, FICHERO_DE_ESTADO, NOMBRE_DE_LA_GUIA};
use super::encargo::Encargo;
use super::respuesta::{neutralizar_para, Cabecera};
use super::saneado::{
    neutralizar_marcas, sanear_saltos, MARCA_DE_LATIDO, MARCA_DE_LATIDO_ANTIGUA, MARCA_DE_LEIDO,
};
use crate::error::{Error, Resultado};
use std::path::{Path, PathBuf};

/// Nombres reservados del canal: ficheros de infraestructura que existen en `.gestor/canal/` pero no
/// son buzones de ningún agente. Se descartan de forma insensible a mayúsculas.
const NOMBRES_RESERVADOS: &[&str] = &[FICHERO_DE_ESTADO, "readme.md", "indice.md"];

/// Prefijos de ficheros que están en el canal pero **no son buzones de nadie**: los históricos que
/// se generan al archivar.
///
/// Sin esto, archivar un encargo se lo vuelve a encargar: el histórico vive dentro de la carpeta
/// que el arnés sondea, y el texto archivado conserva su encabezado `## Para Programator`. Medido
/// el 23/09/2026: el encargo 009 se archivó a las 20:44 y se entregó a las 20:47.
///
/// Va por prefijo y no por nombre exacto porque cada agente nombra el suyo
/// (`historico-programator.md`, `histórico-claude.md`). Son los dos del canal y ninguno
/// más: un prefijo nuevo tiene que incorporarlo también el gestor de agentes que lleve el canal.
/// Antes eran configurables (`[agente] prefijos_ignorados`).
const PREFIJOS_IGNORADOS: &[&str] = &["historico-", "histórico-"];

/// Hasta dónde se ha leído cada buzón ajeno, en forma legible para los compañeros.
#[derive(Debug, Clone, Default)]
pub struct ResumenLeido {
    /// Pares (fichero, descripción de hasta dónde se leyó).
    pub entradas: Vec<(String, String)>,
}

impl ResumenLeido {
    /// Crea un resumen de lectura vacío («nada nuevo»).
    pub fn vacio() -> Self {
        Self {
            entradas: Vec::new(),
        }
    }
}

/// La cabecera del buzón propio, con el nombre del agente de `[agente] nombre`.
fn encabezado(yo: &str) -> String {
    format!(
        "\
# {yo}

> **Solo escribe {yo}.** Los demás leen y **no editan nada de este archivo**, ni para ordenar
> ni para resumir. Si aquí no está escrito por {yo}, no lo ha dicho {yo}.
>
> Lo mantiene el arnés, no el modelo: el formato de este fichero no depende de que el modelo se
> acuerde de respetarlo.
"
    )
}

/// El canal de un proyecto, visto desde un agente concreto.
pub struct Canal {
    directorio: PathBuf,
    /// El nombre del agente que mira el canal, tal como está configurado.
    yo: String,
    fichero_propio: String,
}

impl Canal {
    /// Abre el canal de una carpeta de trabajo, creando la estructura si no existe.
    pub fn nuevo(carpeta: &Path, yo: &str) -> Resultado<Self> {
        let directorio = carpeta_del_canal(carpeta);
        std::fs::create_dir_all(&directorio).map_err(|causa| Error::Escritura {
            ruta: directorio.clone(),
            causa,
        })?;
        Ok(Self {
            directorio,
            yo: yo.to_string(),
            fichero_propio: format!("{}.md", yo.to_lowercase()),
        })
    }

    /// El nombre del agente que mira el canal.
    pub fn yo(&self) -> &str {
        &self.yo
    }

    /// Ruta del buzón de un agente.
    pub fn ruta_buzon(&self, agente: &str) -> PathBuf {
        self.directorio
            .join(format!("{}.md", agente.to_lowercase()))
    }

    /// Ruta del fichero del canal que se llama `nombre`, tal cual, sin cambiarle mayúsculas: en
    /// Linux `Claude.MD` y `claude.md` son ficheros distintos.
    pub fn ruta_del_fichero(&self, nombre: &str) -> PathBuf {
        self.directorio.join(nombre)
    }

    /// Ruta del fichero de hechos comprobables.
    pub fn ruta_estado(&self) -> PathBuf {
        self.directorio.join(FICHERO_DE_ESTADO)
    }

    /// Cuánto hace que cambió el buzón `nombre` (su fecha de modificación). `None` si no se sabe:
    /// no se deja mirar o la fecha cae en el futuro, como con un reloj desfasado.
    pub fn edad_del_buzon(&self, nombre: &str) -> Option<std::time::Duration> {
        std::fs::metadata(self.directorio.join(nombre))
            .ok()?
            .modified()
            .ok()?
            .elapsed()
            .ok()
    }

    /// Lee los buzones de los demás agentes, en orden alfabético estable.
    pub fn buzones_ajenos(&self) -> Resultado<Vec<(String, String)>> {
        let error_de_lectura = |causa| Error::Lectura {
            ruta: self.directorio.clone(),
            causa,
        };
        // Una entrada que no se deja leer es un fallo del canal, no un buzón que no existe; y
        // una carpeta que se llame «x.md» no es un buzón.
        let mut ficheros = Vec::new();
        for entrada in std::fs::read_dir(&self.directorio).map_err(error_de_lectura)? {
            let entrada = entrada.map_err(error_de_lectura)?;
            // `metadata` sigue los enlaces: un buzón que sea un enlace a un fichero se lee.
            if std::fs::metadata(entrada.path())
                .map_err(error_de_lectura)?
                .is_file()
            {
                ficheros.push(entrada.file_name());
            }
        }
        let mut nombres: Vec<String> = ficheros
            .into_iter()
            .filter_map(|nombre| nombre.into_string().ok())
            .filter(|nombre| {
                let nombre_minusculas = nombre.to_lowercase();
                nombre_minusculas.ends_with(".md")
                    && nombre_minusculas != self.fichero_propio
                    && !NOMBRES_RESERVADOS.contains(&nombre_minusculas.as_str())
                    // La guía que el arnés publica para los demás agentes. Sin esto, Programator
                    // leería su propia guía como si fuera el buzón de un compañero y se la
                    // metería en el contexto en cada pasada.
                    && !nombre_minusculas.eq_ignore_ascii_case(NOMBRE_DE_LA_GUIA)
                    && !PREFIJOS_IGNORADOS
                        .iter()
                        .any(|prefijo| nombre_minusculas.starts_with(prefijo))
            })
            .collect();

        nombres.sort();

        nombres
            .into_iter()
            .map(|nombre| {
                let ruta = self.directorio.join(&nombre);
                let contenido = std::fs::read_to_string(&ruta)
                    .map_err(|causa| Error::Lectura { ruta, causa })?;
                Ok((nombre, contenido))
            })
            .collect()
    }

    /// Rota el contenido actual del buzón propio a su fichero histórico
    /// (`historico-{yo}.md`), dejando el buzón limpio con únicamente el encabezado.
    ///
    /// Se invoca al iniciar la atención de nuevos encargos, garantizando que el buzón vivo
    /// mantenga únicamente el último latido y que el historial anterior no se pierda.
    pub fn rotar_a_historico(&self) -> Resultado<()> {
        let ruta = self.directorio.join(&self.fichero_propio);
        if !ruta.exists() {
            return Ok(());
        }
        let contenido = std::fs::read_to_string(&ruta).map_err(|causa| Error::Lectura {
            ruta: ruta.clone(),
            causa,
        })?;
        let previo = extraer_contenido_rotar(&contenido, &encabezado(&self.yo));
        if !previo.trim().is_empty() {
            self.archivar_en_historico(previo.trim())?;
            std::fs::write(&ruta, encabezado(&self.yo))
                .map_err(|causa| Error::Escritura { ruta, causa })?;
        }
        Ok(())
    }

    /// Añade un bloque al buzón propio, con el formato que exige el protocolo.
    ///
    /// El `cuerpo` lo aporta el modelo. El encabezado, el `LATIDO:` y el `LEÍDO:` los aporta este
    /// método, siempre, aunque el modelo los haya omitido o inventado.
    pub fn publicar(&self, cuerpo: &str, leido: &ResumenLeido, ahora: &str) -> Resultado<()> {
        self.publicar_respuesta(cuerpo, leido, ahora, None, ahora)
    }

    /// La cabecera de la respuesta a `encargo`, con la identidad de quien lo escribió leída de la
    /// cabecera de su buzón. `None` si el buzón no se puede leer o no la trae.
    pub fn cabecera_de(&self, encargo: &Encargo) -> Option<Cabecera> {
        let contenido =
            std::fs::read_to_string(self.ruta_del_fichero(&encargo.de)).unwrap_or_default();
        Cabecera::de_encargo(encargo, &contenido, &self.yo)
    }

    /// Publica un aviso que despierta a quien nombra `linea_para` (una línea `Para …` ya compuesta,
    /// como la de [`crate::protocolo::respuesta::linea_para_varios`]). Lo demás, como en
    /// [`Canal::publicar_respuesta`].
    pub fn publicar_dirigido(
        &self,
        cuerpo: &str,
        leido: &ResumenLeido,
        ahora: &str,
        linea_para: &str,
    ) -> Resultado<()> {
        self.escribir_intervencion(cuerpo, leido, ahora, Some(linea_para.to_string()))
    }

    /// Como [`Canal::publicar`], pero, con `cabecera`, la línea que sigue al `LATIDO:` es la suya
    /// (`Para <identidad>: respuesta a …` y el acuse), que despierta a quien encargó. Todo lo demás
    /// se publica con cada «Para» en minúscula, para que no despierte a nadie (por compatibilidad
    /// con Gestor Agentes).
    ///
    /// El `LATIDO:` lleva `ahora`, la hora de la publicación; el acuse lleva `recibido_a`, la marca
    /// de cuando se leyó el encargo, porque `RECIBIDO` dice cuándo llegó, no cuándo se contestó
    /// (en una prueba de uso real salían iguales aunque entre una y otra pasaron 65 s).
    pub fn publicar_respuesta(
        &self,
        cuerpo: &str,
        leido: &ResumenLeido,
        ahora: &str,
        cabecera: Option<&Cabecera>,
        recibido_a: &str,
    ) -> Resultado<()> {
        let lineas = cabecera.map(|cabecera| cabecera.lineas(hora_de(recibido_a)));
        self.escribir_intervencion(cuerpo, leido, ahora, lineas)
    }

    /// Escribe una intervención. Tras el `LATIDO:` va `cabecera` o, sin ella, la primera línea del
    /// cuerpo como resumen.
    fn escribir_intervencion(
        &self,
        cuerpo: &str,
        leido: &ResumenLeido,
        ahora: &str,
        cabecera: Option<String>,
    ) -> Resultado<()> {
        let ruta = self.directorio.join(&self.fichero_propio);

        let mut contenido = if ruta.exists() {
            std::fs::read_to_string(&ruta).map_err(|causa| Error::Lectura {
                ruta: ruta.clone(),
                causa,
            })?
        } else {
            encabezado(&self.yo)
        };

        if !contenido.ends_with('\n') {
            contenido.push('\n');
        }

        // Sanear el cuerpo: neutralizar líneas que parecen marcas del arnés y cada «Para» que
        // despertaría a alguien.
        let cuerpo_saneado = neutralizar_para(&neutralizar_marcas(cuerpo));

        let resumen = if leido.entradas.is_empty() {
            "nada nuevo".to_string()
        } else {
            leido
                .entradas
                .iter()
                .map(|(fichero, hasta)| {
                    // Reemplazar saltos de línea y líneas falsas en ambos campos
                    let fichero_limpio = sanear_saltos(fichero);
                    let hasta_limpio = sanear_saltos(hasta);
                    let hasta_saneado = neutralizar_marcas(&hasta_limpio);
                    format!("`{fichero_limpio}` {hasta_saneado}")
                })
                .collect::<Vec<_>>()
                .join(" · ")
        };
        let resumen = neutralizar_para(&resumen);
        let segunda_linea = cabecera.unwrap_or_else(|| primera_linea(&cuerpo_saneado));

        // Como en el resto del canal: cada intervención empieza con `LATIDO: <AAAA-MM-DD HH:MM:SS>`, sin
        // negrita y sola en su línea, que es lo que lee su revisión del canal. El resumen y el
        // `LEÍDO:` van en las siguientes.
        contenido.push_str(&format!(
            "\n{MARCA_DE_LATIDO} {ahora}\n{segunda_linea}\n{MARCA_DE_LEIDO} {resumen}\n\n---\n\n{cuerpo_saneado}\n",
        ));

        std::fs::write(&ruta, contenido).map_err(|causa| Error::Escritura { ruta, causa })
    }

    /// Archiva un bloque anterior en `.gestor/canal/historico-{fichero_propio}`.
    fn archivar_en_historico(&self, bloque: &str) -> Resultado<()> {
        let ruta_historico = self
            .directorio
            .join(format!("historico-{}", self.fichero_propio));

        if !ruta_historico.exists() {
            let agente = super::poda::nombre_del_buzon(&self.fichero_propio);
            let mut contenido = format!(
                "# Histórico de {}\n\n> Archivo histórico de mensajes y latidos anteriores rotados automáticamente del buzón principal.\n\n",
                agente
            );
            contenido.push_str(bloque);
            contenido.push('\n');
            std::fs::write(&ruta_historico, contenido).map_err(|causa| Error::Escritura {
                ruta: ruta_historico,
                causa,
            })
        } else {
            let mut contenido =
                std::fs::read_to_string(&ruta_historico).map_err(|causa| Error::Lectura {
                    ruta: ruta_historico.clone(),
                    causa,
                })?;
            if !contenido.ends_with('\n') {
                contenido.push('\n');
            }
            if !contenido.ends_with("\n---\n") {
                contenido.push_str("\n---\n\n");
            }
            contenido.push_str(bloque);
            contenido.push('\n');
            std::fs::write(&ruta_historico, contenido).map_err(|causa| Error::Escritura {
                ruta: ruta_historico,
                causa,
            })
        }
    }

    /// Publica la entrega de una propuesta en el buzón propio en el instante en que queda escrita en disco.
    ///
    /// Pasa estrictamente por `publicar`, garantizando que apliquen todas las defensas
    /// contra marcas falsas y saneado de cabeceras.
    pub fn publicar_entrega(
        &self,
        entrega: &EntregaPublicada,
        leido: &ResumenLeido,
        ahora: &str,
    ) -> Resultado<()> {
        self.publicar(&cuerpo_de_la_entrega(entrega), leido, ahora)
    }
}

/// Lo que se publica al dejar una propuesta en disco.
#[derive(Debug, Clone, Copy)]
pub struct EntregaPublicada<'a> {
    /// El nombre con que quedó.
    pub propuesta: &'a str,
    /// La línea de su comprobación.
    pub veredicto: &'a str,
    /// Cómo decir en qué quedó, al final; vacío si no se lleva registro.
    pub confirmacion: &'a str,
}

/// El cuerpo del aviso de una propuesta dejada en disco.
fn cuerpo_de_la_entrega(entrega: &EntregaPublicada) -> String {
    let EntregaPublicada {
        propuesta,
        veredicto,
        confirmacion,
    } = entrega;
    let mut cuerpo = format!(
        "He dejado la propuesta «{propuesta}» en disco.\n\n**Comprobación de las propuestas** (la \
         hace el arnés, no el modelo):\n- {veredicto}"
    );
    if !confirmacion.is_empty() {
        cuerpo.push_str("\n\n");
        cuerpo.push_str(confirmacion);
    }
    cuerpo
}

/// La marca de tiempo actual local en el formato que exige el protocolo: `2026-09-13 08:30`.
/// Es la de las reservas.
pub fn marca_de_tiempo_actual() -> String {
    chrono::Local::now()
        .format(FORMATO_DE_LA_MARCA_DE_TIEMPO)
        .to_string()
}

/// La marca de tiempo del latido, con segundos, como la escriben los demás agentes del canal:
/// `2026-11-13 15:40:07`.
pub fn marca_del_latido() -> String {
    chrono::Local::now()
        .format(FORMATO_DE_LA_HORA_DEL_LATIDO)
        .to_string()
}

/// Cómo va la hora de una reserva y de las demás marcas del protocolo, sin segundos.
pub(crate) const FORMATO_DE_LA_MARCA_DE_TIEMPO: &str = "%Y-%m-%d %H:%M";

/// Cómo va la hora tras `LATIDO:`. El canal acepta también sin segundos; con ellos se
/// distinguen dos intervenciones del mismo minuto. También la usa el latido del arnés en
/// `latido.json`, para que las dos horas se lean igual.
pub(crate) const FORMATO_DE_LA_HORA_DEL_LATIDO: &str = "%Y-%m-%d %H:%M:%S";

/// Si la línea, tal cual (sin quitarle una cita delante), empieza un latido en cualquiera de las
/// dos formas.
pub fn es_linea_de_latido(linea: &str) -> bool {
    linea.starts_with(MARCA_DE_LATIDO) || linea.starts_with(MARCA_DE_LATIDO_ANTIGUA)
}

/// Lo que va tras la marca de latido de una línea que lo es.
pub fn tras_la_marca_de_latido(linea: &str) -> Option<&str> {
    linea
        .strip_prefix(MARCA_DE_LATIDO_ANTIGUA)
        .or_else(|| linea.strip_prefix(MARCA_DE_LATIDO))
        .map(str::trim)
}

/// La hora de una marca `AAAA-MM-DD HH:MM:SS`, para el acuse: la última palabra.
fn hora_de(ahora: &str) -> &str {
    ahora.split_whitespace().last().unwrap_or(ahora)
}

/// Primera línea del cuerpo, recortada, para que el latido diga de un vistazo en qué se está.
fn primera_linea(cuerpo: &str) -> String {
    let linea = cuerpo.lines().next().unwrap_or("").trim();
    if linea.is_empty() {
        return "sin novedad".to_string();
    }
    super::latido::acotar_resumen(linea)
}

/// Dónde empieza la primera línea de latido del buzón, en cualquiera de las dos formas.
fn inicio_del_primer_latido(contenido: &str) -> Option<usize> {
    let mut desplazamiento = 0;
    for linea in contenido.split_inclusive('\n') {
        if es_linea_de_latido(linea) {
            return Some(desplazamiento);
        }
        desplazamiento += linea.len();
    }
    None
}

/// Extrae el contenido de mensajes o latidos previos de un buzón para su rotación a histórico.
fn extraer_contenido_rotar<'a>(contenido: &'a str, encabezado: &str) -> &'a str {
    if let Some(pos) = inicio_del_primer_latido(contenido) {
        &contenido[pos..]
    } else if let Some(resto) = contenido.strip_prefix(encabezado) {
        resto
    } else {
        let mut desplazamiento = 0;
        for linea in contenido.lines() {
            let recortada = linea.trim();
            if recortada.starts_with('#') || recortada.starts_with('>') || recortada.is_empty() {
                desplazamiento += linea.len();
                if desplazamiento < contenido.len() && contenido.as_bytes()[desplazamiento] == b'\r'
                {
                    desplazamiento += 1;
                }
                if desplazamiento < contenido.len() && contenido.as_bytes()[desplazamiento] == b'\n'
                {
                    desplazamiento += 1;
                }
            } else {
                break;
            }
        }
        &contenido[desplazamiento..]
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn canal_de_prueba() -> (tempfile::TempDir, Canal) {
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let carpeta_del_canal = carpeta_temporal.path().join(".gestor/canal");
        std::fs::create_dir_all(&carpeta_del_canal).unwrap();
        std::fs::write(
            carpeta_del_canal.join("claude.md"),
            "# Claude\ncontenido de Claude\n",
        )
        .unwrap();
        std::fs::write(
            carpeta_del_canal.join("codex.md"),
            "# Codex\ncontenido de Codex\n",
        )
        .unwrap();
        std::fs::write(
            carpeta_del_canal.join("programator.md"),
            "# Programator\nlo mío\n",
        )
        .unwrap();
        std::fs::write(carpeta_del_canal.join("estado.md"), "# Estado\n").unwrap();
        let canal = Canal::nuevo(carpeta_temporal.path(), "Programator").unwrap();
        (carpeta_temporal, canal)
    }

    /// Una carpeta «x.md» no se toma por buzón: su lectura tumbaría la de todos.
    #[test]
    fn una_carpeta_con_nombre_de_buzon_no_es_un_buzon() {
        let (carpeta_temporal, canal) = canal_de_prueba();
        std::fs::create_dir(carpeta_temporal.path().join(".gestor/canal/carpeta.md")).unwrap();

        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(nombres, ["claude.md", "codex.md"]);
    }

    /// Un buzón que sea un enlace simbólico a un fichero se sigue leyendo (`0683bd5`). Solo en
    /// Unix: en Windows crear un enlace simbólico pide privilegios que las pruebas no tienen.
    #[cfg(unix)]
    #[test]
    fn un_buzon_que_es_un_enlace_a_un_fichero_se_lee() {
        let (carpeta, canal) = canal_de_prueba();
        let destino = carpeta.path().join("buzon-real.md");
        std::fs::write(&destino, "# Gemini\ncontenido enlazado\n").unwrap();
        std::os::unix::fs::symlink(&destino, carpeta.path().join(".gestor/canal/gemini.md"))
            .unwrap();

        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(nombres, ["claude.md", "codex.md", "gemini.md"]);
        let (_, contenido) = &buzones[2];
        assert!(contenido.contains("contenido enlazado"));
    }

    #[test]
    fn lee_los_buzones_ajenos_y_no_el_propio() {
        let (_dir, canal) = canal_de_prueba();

        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert!(nombres.contains(&"claude.md"));
        assert!(nombres.contains(&"codex.md"));
        assert!(
            !nombres.contains(&"programator.md"),
            "el buzón propio no es ajeno"
        );
        assert!(!nombres.contains(&"estado.md"), "estado.md no es un buzón");
    }

    #[test]
    fn la_guia_del_equipo_no_se_lee_como_si_fuera_un_buzon() {
        // La escribe el propio arnés en el canal. Sin reservarla, Programator leería su propia
        // guía como el buzón de un compañero y se la metería en el contexto en cada pasada:
        // gastaría ventana y se citaría a sí mismo como si fuera información ajena.
        let (carpeta, canal) = canal_de_prueba();
        std::fs::write(
            carpeta
                .path()
                .join(".gestor/canal")
                .join("COMO-ENCARGAR-A-PROGRAMATOR.md"),
            "# Guía\n",
        )
        .unwrap();

        let buzones = canal.buzones_ajenos().unwrap();
        let nombres: Vec<String> = buzones.iter().map(|(n, _)| n.to_lowercase()).collect();

        assert!(
            !nombres.contains(&"como-encargar-a-programator.md".to_string()),
            "la guía es infraestructura del canal, no el buzón de nadie: {nombres:?}"
        );
    }

    #[test]
    fn devuelve_los_buzones_en_orden_estable() {
        let (_dir, canal) = canal_de_prueba();

        let primera = canal.buzones_ajenos().unwrap();
        let segunda = canal.buzones_ajenos().unwrap();

        let n1: Vec<&String> = primera.iter().map(|(n, _)| n).collect();
        let n2: Vec<&String> = segunda.iter().map(|(n, _)| n).collect();
        assert_eq!(n1, n2, "el orden debe ser estable entre lecturas");
    }

    #[test]
    fn crea_la_estructura_del_canal_si_la_carpeta_no_la_tiene() {
        let carpeta = tempfile::tempdir().unwrap();

        let canal = Canal::nuevo(carpeta.path(), "Programator").unwrap();

        assert!(carpeta.path().join(".gestor/canal").is_dir());
        assert!(canal.buzones_ajenos().unwrap().is_empty());
    }

    #[test]
    fn un_historico_no_es_un_buzon_y_no_se_vuelve_a_atender() {
        // El 23/09/2026 el encargo 009 se archivó a las 20:44 y se entregó a las 20:47: los
        // históricos viven dentro de la carpeta que el arnés sondea, así que archivar es volver
        // a encargar.
        let carpeta_temporal = tempfile::tempdir().unwrap();
        let carpeta_del_canal = carpeta_temporal.path().join(".gestor").join("canal");
        std::fs::create_dir_all(&carpeta_del_canal).unwrap();
        std::fs::write(
            carpeta_del_canal.join("claude.md"),
            "## Para Programator\n\nvivo",
        )
        .unwrap();
        std::fs::write(
            carpeta_del_canal.join("historico-actas.md"),
            "## Para Programator\n\nya cerrado",
        )
        .unwrap();

        let canal = Canal::nuevo(carpeta_temporal.path(), "programator").unwrap();
        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(nombres, vec!["claude.md"], "el histórico no es un buzón");
    }

    #[test]
    fn descarta_nombres_reservados_insensiblemente_a_mayusculas() {
        let carpeta = tempfile::tempdir().unwrap();
        let carpeta_del_canal = carpeta.path().join(".gestor/canal");
        std::fs::create_dir_all(&carpeta_del_canal).unwrap();
        std::fs::write(carpeta_del_canal.join("README.md"), "# Reglas\n").unwrap();
        std::fs::write(carpeta_del_canal.join("INDICE.md"), "# Índice\n").unwrap();
        std::fs::write(carpeta_del_canal.join("Claude.md"), "# Claude\ncontenido\n").unwrap();
        let canal = Canal::nuevo(carpeta.path(), "Programator").unwrap();

        let buzones = canal.buzones_ajenos().unwrap();
        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();

        assert!(
            !nombres.contains(&"README.md"),
            "README.md debe ser descartado"
        );
        assert!(
            !nombres.contains(&"INDICE.md"),
            "INDICE.md debe ser descartado"
        );
        assert!(
            nombres.contains(&"Claude.md"),
            "Claude.md es un buzón de agente, debe aparecer"
        );
    }

    #[test]
    fn descarta_el_buzon_propio_insensiblemente_a_mayusculas() {
        let carpeta = tempfile::tempdir().unwrap();
        let carpeta_del_canal = carpeta.path().join(".gestor/canal");
        std::fs::create_dir_all(&carpeta_del_canal).unwrap();
        std::fs::write(
            carpeta_del_canal.join("Programator.md"),
            "# Programator\nlo mío\n",
        )
        .unwrap();
        std::fs::write(carpeta_del_canal.join("Claude.md"), "# Claude\ncontenido\n").unwrap();
        let canal = Canal::nuevo(carpeta.path(), "Programator").unwrap();

        let buzones = canal.buzones_ajenos().unwrap();
        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();

        assert!(
            !nombres.contains(&"Programator.md"),
            "el buzón propio con mayúscula debe ser descartado"
        );
        assert!(
            nombres.contains(&"Claude.md"),
            "Claude.md es un buzón ajeno, debe aparecer"
        );
    }

    #[test]
    fn otro_prefijo_de_archivo_si_es_un_buzon() {
        // Los históricos son los del canal, `historico-` e `histórico-`, y ningún otro. Un proyecto que archive con otro prefijo tiene que
        // renombrar: si no, sus ficheros se leen como buzones.
        let carpeta = tempfile::tempdir().unwrap();
        let carpeta_del_canal = carpeta.path().join(".gestor/canal");
        std::fs::create_dir_all(&carpeta_del_canal).unwrap();
        std::fs::write(
            carpeta_del_canal.join("claude.md"),
            "## Para Programator\n\nvivo",
        )
        .unwrap();
        std::fs::write(carpeta_del_canal.join("historico-actas.md"), "ya cerrado").unwrap();
        std::fs::write(carpeta_del_canal.join("histórico-claude.md"), "ya cerrado").unwrap();
        std::fs::write(carpeta_del_canal.join("archivado-actas.md"), "otro prefijo").unwrap();

        let canal = Canal::nuevo(carpeta.path(), "Programator").unwrap();
        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(nombres, vec!["archivado-actas.md", "claude.md"]);
    }

    #[test]
    fn el_prefijo_del_historico_no_distingue_mayusculas() {
        let carpeta = tempfile::tempdir().unwrap();
        let carpeta_del_canal = carpeta.path().join(".gestor/canal");
        std::fs::create_dir_all(&carpeta_del_canal).unwrap();
        std::fs::write(
            carpeta_del_canal.join("claude.md"),
            "## Para Programator\n\nvivo",
        )
        .unwrap();
        std::fs::write(carpeta_del_canal.join("Historico-Actas.md"), "ya cerrado").unwrap();
        std::fs::write(carpeta_del_canal.join("Histórico-Claude.md"), "ya cerrado").unwrap();

        let canal = Canal::nuevo(carpeta.path(), "Programator").unwrap();
        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(nombres, vec!["claude.md"]);
    }

    #[test]
    fn la_publicacion_lleva_latido_y_leido_aunque_el_modelo_no_los_escriba() {
        let (carpeta, canal) = canal_de_prueba();
        let leido = ResumenLeido {
            entradas: vec![
                ("claude.md".to_string(), "hasta su latido 14:00".to_string()),
                ("codex.md".to_string(), "completo".to_string()),
            ],
        };

        canal
            .publicar(
                "He ejecutado las pruebas: 446 en verde.",
                &leido,
                "2026-09-13 16:20",
            )
            .unwrap();

        let buzon =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(buzon.contains(
            "
LATIDO: 2026-09-13 16:20
"
        ));
        assert!(buzon.contains(
            "
LEÍDO: "
        ));
        assert!(buzon.contains("`claude.md` hasta su latido 14:00"));
        assert!(buzon.contains("446 en verde"));
    }

    #[test]
    fn la_publicacion_conserva_lo_que_ya_habia_en_el_buzon() {
        let (carpeta, canal) = canal_de_prueba();
        let leido = ResumenLeido { entradas: vec![] };

        canal
            .publicar("Bloque nuevo.", &leido, "2026-09-13 16:20")
            .unwrap();

        let buzon =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(
            buzon.contains("lo mío"),
            "no puede perder el contenido anterior"
        );
        assert!(buzon.contains("Bloque nuevo."));
    }

    #[test]
    fn rotar_a_historico_mueve_mensajes_anteriores_y_deja_encabezado() {
        let (carpeta, canal) = canal_de_prueba();
        canal.rotar_a_historico().unwrap();

        let buzon =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(!buzon.contains("lo mío"));
        assert!(buzon.starts_with("# Programator"));

        let hist = std::fs::read_to_string(
            carpeta
                .path()
                .join(".gestor/canal/historico-programator.md"),
        )
        .unwrap();
        assert!(hist.contains("lo mío"));
    }

    #[test]
    fn rotar_a_historico_acumula_sucesivamente_en_historico() {
        let carpeta = tempfile::tempdir().unwrap();
        let canal = Canal::nuevo(carpeta.path(), "Programator").unwrap();
        let leido = ResumenLeido { entradas: vec![] };

        canal
            .publicar("Primer mensaje.", &leido, "2026-09-13 16:00")
            .unwrap();

        canal.rotar_a_historico().unwrap();

        let buzon1 =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(!buzon1.contains("Primer mensaje."));
        assert!(buzon1.starts_with("# Programator"));

        let hist1 = std::fs::read_to_string(
            carpeta
                .path()
                .join(".gestor/canal/historico-programator.md"),
        )
        .unwrap();
        assert!(hist1.contains("Primer mensaje."));

        canal
            .publicar("Segundo mensaje.", &leido, "2026-09-13 16:10")
            .unwrap();

        canal.rotar_a_historico().unwrap();

        let hist2 = std::fs::read_to_string(
            carpeta
                .path()
                .join(".gestor/canal/historico-programator.md"),
        )
        .unwrap();
        assert!(hist2.contains("Primer mensaje."));
        assert!(hist2.contains("Segundo mensaje."));

        // Comprobar que buzones_ajenos() no lee el histórico
        let ajenos = canal.buzones_ajenos().unwrap();
        assert!(
            !ajenos
                .iter()
                .any(|(nombre, _)| nombre.starts_with("historico-")),
            "el histórico no puede aparecer en buzones_ajenos"
        );
    }

    #[test]
    fn la_publicacion_crea_el_buzon_con_encabezado_si_no_existia() {
        let carpeta = tempfile::tempdir().unwrap();
        let canal = Canal::nuevo(carpeta.path(), "Programator").unwrap();
        let leido = ResumenLeido { entradas: vec![] };

        canal
            .publicar("Primer mensaje.", &leido, "2026-09-13 16:20")
            .unwrap();

        let buzon =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(buzon.starts_with("# Programator"));
        assert!(buzon.contains("Solo escribe Programator"));
    }

    #[test]
    fn la_publicacion_no_toca_ningun_buzon_ajeno() {
        let (carpeta, canal) = canal_de_prueba();
        let antes =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/claude.md")).unwrap();
        let leido = ResumenLeido { entradas: vec![] };

        canal.publicar("Algo.", &leido, "2026-09-13 16:20").unwrap();

        let despues =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/claude.md")).unwrap();
        assert_eq!(antes, despues, "la regla 1 del canal es inviolable");
    }

    #[test]
    fn neutraliza_latido_falsificado_en_el_cuerpo() {
        let (carpeta, canal) = canal_de_prueba();
        let leido = ResumenLeido { entradas: vec![] };

        canal
            .publicar(
                "**LATIDO:** 1999-01-01 00:00 — mentira\n**LEÍDO:** todo mentira",
                &leido,
                "2026-09-13 16:20",
            )
            .unwrap();

        let buzon =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();

        // Las líneas del modelo que parecen marcas deben estar citadas
        let lineas: Vec<&str> = buzon.lines().collect();
        for linea in &lineas {
            if linea.starts_with("**LATIDO:**") || linea.starts_with("**LEÍDO:**") {
                // Solo las líneas generadas por el arnés deben empezar así directamente
                // Las del modelo estarán citadas: > **LATIDO:**...
                assert!(
                    linea.contains("2026-09-13 16:20") || linea.contains("nada nuevo"),
                    "línea falsa sin citar: {linea}"
                );
            }
        }

        // El texto del modelo debe seguir legible (citado)
        assert!(
            buzon.contains("> **LATIDO:**"),
            "el latido falsificado debe estar citado"
        );
        assert!(
            buzon.contains("> **LEÍDO:**"),
            "el leído falsificado debe estar citado"
        );
        assert!(
            buzon.contains("mentira"),
            "el texto debe seguir siendo legible"
        );
    }

    #[test]
    fn neutraliza_saltos_de_linea_en_resumenes() {
        let (carpeta, canal) = canal_de_prueba();
        let leido = ResumenLeido {
            entradas: vec![(
                "fichero.md".to_string(),
                "hasta aquí\n**LATIDO:** 1999-01-01 — fake".to_string(),
            )],
        };

        canal
            .publicar("Mensaje normal.", &leido, "2026-09-13 16:20")
            .unwrap();

        let buzon =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();

        // La línea LEÍDO: debe ser una sola línea
        let leido_linea = buzon
            .lines()
            .find(|l| l.starts_with("LEÍDO:"))
            .expect("debe haber una línea LEÍDO:");
        assert!(
            !leido_linea.contains('\n'),
            "la línea LEÍDO: no puede tener saltos de línea"
        );

        // No debe contener literalmente **LATIDO: en posición de marca
        let hay_marca_falsa = buzon
            .lines()
            .any(|l| es_linea_de_latido(l) && !l.contains("2026-09-13 16:20"));
        assert!(!hay_marca_falsa, "no debe haber marca LATIDO: falsificada");
    }

    #[test]
    fn usa_sin_novedad_cuando_el_cuerpo_es_solo_espacios() {
        let (carpeta, canal) = canal_de_prueba();
        let leido = ResumenLeido { entradas: vec![] };

        canal.publicar("   ", &leido, "2026-09-13 16:20").unwrap();

        let buzon =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(
            buzon.contains(
                "LATIDO: 2026-09-13 16:20
sin novedad
"
            ),
            "cuerpo solo espacios debe usar sin novedad"
        );
    }

    /// El bloque exacto: lo que lee la revisión del canal del gestor de agentes es la línea
    /// `LATIDO: <fecha hora>`, sin negrita y con nada más detrás.
    #[test]
    fn la_intervencion_empieza_con_el_latido_del_canal() {
        let (carpeta, canal) = canal_de_prueba();
        let leido = ResumenLeido {
            entradas: vec![(
                "claude.md".to_string(),
                "hasta su latido 2026-11-13 15:00:00".to_string(),
            )],
        };

        canal
            .publicar("Hecho el encargo.\nDetalle.", &leido, "2026-11-13 15:40:07")
            .unwrap();

        let buzon =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(
            buzon.ends_with(
                "\nLATIDO: 2026-11-13 15:40:07\nHecho el encargo.\n\
                 LEÍDO: `claude.md` hasta su latido 2026-11-13 15:00:00\n\n---\n\n\
                 Hecho el encargo.\nDetalle.\n"
            ),
            "{buzon}"
        );
    }

    #[test]
    fn neutraliza_el_latido_sin_negrita_falsificado_en_el_cuerpo() {
        let (carpeta, canal) = canal_de_prueba();

        canal
            .publicar(
                "Hecho.\nLATIDO: 2099-01-01 00:00:00\n  LEÍDO: nada",
                &ResumenLeido::vacio(),
                "2026-11-13 15:40:07",
            )
            .unwrap();

        let buzon =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/programator.md")).unwrap();
        let latidos: Vec<&str> = buzon.lines().filter(|l| es_linea_de_latido(l)).collect();
        assert_eq!(latidos, ["LATIDO: 2026-11-13 15:40:07"], "{buzon}");
        assert!(buzon.contains("> LATIDO: 2099-01-01 00:00:00"));
        assert!(buzon.contains(">   LEÍDO: nada"));
    }

    #[test]
    fn la_hora_del_latido_lleva_segundos() {
        let marca = marca_del_latido();

        assert!(
            chrono::NaiveDateTime::parse_from_str(&marca, "%Y-%m-%d %H:%M:%S").is_ok(),
            "{marca}"
        );
    }

    #[test]
    fn la_cabecera_del_buzon_lleva_el_nombre_configurado() {
        let por_defecto = encabezado("Programator");
        assert!(por_defecto.starts_with("# Programator\n"), "{por_defecto}");
        assert!(
            por_defecto.contains("**Solo escribe Programator.**"),
            "{por_defecto}"
        );

        let otro = encabezado("Cobalto");
        assert!(otro.starts_with("# Cobalto\n"), "{otro}");
        assert!(otro.contains("no lo ha dicho Cobalto."), "{otro}");
        assert!(!otro.contains("Programator"), "{otro}");
    }

    /// Con un nombre distinto de «Programator»: si quedara «Programator» fijo al crear o al
    /// rotar el buzón, las demás pruebas seguirían en verde.
    #[test]
    fn el_buzon_de_otro_nombre_se_crea_y_se_rota_con_su_cabecera() {
        let carpeta = tempfile::tempdir().unwrap();
        let canal = Canal::nuevo(carpeta.path(), "Cobalto").unwrap();
        let leido = ResumenLeido { entradas: vec![] };
        let buzon = carpeta.path().join(".gestor/canal/cobalto.md");

        canal
            .publicar("Primer mensaje.", &leido, "2026-11-23 16:00")
            .unwrap();
        let creado = std::fs::read_to_string(&buzon).unwrap();
        assert!(creado.starts_with(&encabezado("Cobalto")), "{creado}");

        canal.rotar_a_historico().unwrap();
        let rotado = std::fs::read_to_string(&buzon).unwrap();
        assert_eq!(rotado, encabezado("Cobalto"));
        let historico =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/historico-cobalto.md"))
                .unwrap();
        assert!(historico.contains("Primer mensaje."), "{historico}");
        assert!(!historico.contains("Solo escribe Cobalto"), "{historico}");

        // Sin LATIDO, solo la cabecera propia separa lo que se archiva: un título escrito a mano
        // debajo no puede perderse con ella.
        std::fs::write(
            &buzon,
            format!("{}## Apunte sin latido\nTexto.\n", encabezado("Cobalto")),
        )
        .unwrap();
        canal.rotar_a_historico().unwrap();
        let historico =
            std::fs::read_to_string(carpeta.path().join(".gestor/canal/historico-cobalto.md"))
                .unwrap();
        assert!(historico.contains("## Apunte sin latido"), "{historico}");
    }

    #[test]
    fn la_rotacion_reconoce_el_latido_sin_negrita() {
        let contenido = format!(
            "{}\nLATIDO: 2026-11-13 15:40:07\nHecho.\nLEÍDO: nada nuevo\n\n---\n\nHecho.\n",
            encabezado("Programator")
        );

        assert_eq!(
            extraer_contenido_rotar(&contenido, &encabezado("Programator")),
            "LATIDO: 2026-11-13 15:40:07\nHecho.\nLEÍDO: nada nuevo\n\n---\n\nHecho.\n"
        );
    }

    #[test]
    fn tras_la_marca_lee_las_dos_formas_y_no_las_citadas() {
        assert_eq!(
            tras_la_marca_de_latido("LATIDO: 2026-11-13 15:40:07"),
            Some("2026-11-13 15:40:07")
        );
        assert_eq!(
            tras_la_marca_de_latido("**LATIDO:** 2026-09-01 10:00 — algo"),
            Some("2026-09-01 10:00 — algo")
        );
        assert_eq!(tras_la_marca_de_latido("> LATIDO: 2099-01-01"), None);
    }
}
