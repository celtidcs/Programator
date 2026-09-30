//! Acceso al canal `.gestor/canal/`. Este módulo sabe qué ficheros existen y de quién es cada uno;
//! no decide qué escribir en ellos.

use super::saneado::{neutralizar_marcas, sanear_saltos};
use crate::error::{Error, Resultado};
use std::path::{Path, PathBuf};

/// Nombres reservados del canal: ficheros de infraestructura que existen en `.gestor/canal/` pero no
/// son buzones de ningún agente. Se descartan de forma insensible a mayúsculas.
const NOMBRES_RESERVADOS: &[&str] = &[
    "estado.md",
    "readme.md",
    "indice.md",
    // La guía que el arnés publica para los demás agentes. Sin esto, Programator leería su propia
    // guía como si fuera el buzón de un compañero y se la metería en el contexto en cada pasada.
    "como-encargar-a-programator.md",
];

/// Prefijos de ficheros que están en el canal pero **no son buzones de nadie**: los históricos que
/// se generan al archivar.
///
/// Sin esto, archivar un encargo se lo vuelve a encargar: el histórico vive dentro de la carpeta
/// que el arnés sondea, y el texto archivado conserva su encabezado `## Para Programator`. Medido
/// el 23/09/2026: el encargo 009 se archivó a las 20:44 y se entregó a las 20:47.
///
/// Va por prefijo y no por nombre exacto porque el nombre del histórico lo elige el proyecto
/// anfitrión, no el arnés.
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

const ENCABEZADO: &str = "\
# Programator

> **Solo escribe Programator.** Los demás leen y **no editan nada de este archivo**, ni para ordenar
> ni para resumir. Si aquí no está escrito por Programator, no lo ha dicho Programator.
>
> Lo mantiene el arnés, no el modelo: el formato de este fichero no depende de que el modelo se
> acuerde de respetarlo.
";

/// El canal de un proyecto, visto desde un agente concreto.
pub struct Canal {
    directorio: PathBuf,
    fichero_propio: String,
    /// Prefijos que este proyecto usa para sus históricos. Vacío significa los de por defecto.
    prefijos_ignorados: Vec<String>,
}

impl Canal {
    /// Abre el canal de una carpeta de trabajo, creando la estructura si no existe.
    pub fn nuevo(carpeta: &Path, yo: &str) -> Resultado<Self> {
        let directorio = carpeta.join(".gestor").join("canal");
        std::fs::create_dir_all(&directorio).map_err(|causa| Error::Escritura {
            ruta: directorio.clone(),
            causa,
        })?;
        Ok(Self {
            directorio,
            fichero_propio: format!("{}.md", yo.to_lowercase()),
            prefijos_ignorados: PREFIJOS_IGNORADOS.iter().map(|p| p.to_string()).collect(),
        })
    }

    /// Cambia los prefijos que no son buzones por los que declare el proyecto anfitrión.
    ///
    /// Una lista vacía deja los de por defecto: quien no configure nada no se queda sin defensa.
    pub fn con_prefijos_ignorados(mut self, prefijos: Vec<String>) -> Self {
        if !prefijos.is_empty() {
            self.prefijos_ignorados = prefijos.iter().map(|p| p.to_lowercase()).collect();
        }
        self
    }

    /// Ruta del buzón de un agente.
    pub fn ruta_buzon(&self, agente: &str) -> PathBuf {
        self.directorio
            .join(format!("{}.md", agente.to_lowercase()))
    }

    /// Ruta del fichero de hechos comprobables.
    pub fn ruta_estado(&self) -> PathBuf {
        self.directorio.join("estado.md")
    }

    /// Lee los buzones de los demás agentes, en orden alfabético estable.
    pub fn buzones_ajenos(&self) -> Resultado<Vec<(String, String)>> {
        let mut nombres: Vec<String> = std::fs::read_dir(&self.directorio)
            .map_err(|causa| Error::Lectura {
                ruta: self.directorio.clone(),
                causa,
            })?
            .filter_map(|entrada| entrada.ok())
            .filter_map(|entrada| entrada.file_name().into_string().ok())
            .filter(|nombre| {
                let nombre_lower = nombre.to_lowercase();
                nombre_lower.ends_with(".md")
                    && nombre_lower != self.fichero_propio
                    && !NOMBRES_RESERVADOS.contains(&nombre_lower.as_str())
                    && !self
                        .prefijos_ignorados
                        .iter()
                        .any(|prefijo| nombre_lower.starts_with(prefijo))
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
        let previo = extraer_contenido_rotar(&contenido);
        if !previo.trim().is_empty() {
            self.archivar_en_historico(previo.trim())?;
            std::fs::write(&ruta, ENCABEZADO).map_err(|causa| Error::Escritura { ruta, causa })?;
        }
        Ok(())
    }

    /// Añade un bloque al buzón propio, con el formato que exige el protocolo.
    ///
    /// El `cuerpo` lo aporta el modelo. El encabezado, el `LATIDO:` y el `LEÍDO:` los aporta este
    /// método, siempre, aunque el modelo los haya omitido o inventado.
    pub fn publicar(&self, cuerpo: &str, leido: &ResumenLeido, ahora: &str) -> Resultado<()> {
        let ruta = self.directorio.join(&self.fichero_propio);

        let mut contenido = if ruta.exists() {
            std::fs::read_to_string(&ruta).map_err(|causa| Error::Lectura {
                ruta: ruta.clone(),
                causa,
            })?
        } else {
            ENCABEZADO.to_string()
        };

        if !contenido.ends_with('\n') {
            contenido.push('\n');
        }

        // Sanear el cuerpo: neutralizar líneas que parecen marcas del arnés
        let cuerpo_saneado = neutralizar_marcas(cuerpo);

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

        contenido.push_str(&format!(
            "\n**LATIDO:** {ahora} — {resumen_corto}\n**LEÍDO:** {resumen}\n\n---\n\n{cuerpo_saneado}\n",
            resumen_corto = primera_linea(&cuerpo_saneado),
        ));

        std::fs::write(&ruta, contenido).map_err(|causa| Error::Escritura { ruta, causa })
    }

    /// Archiva un bloque anterior en `.gestor/canal/historico-{fichero_propio}`.
    fn archivar_en_historico(&self, bloque: &str) -> Resultado<()> {
        let ruta_historico = self
            .directorio
            .join(format!("historico-{}", self.fichero_propio));

        if !ruta_historico.exists() {
            let agente = self.fichero_propio.trim_end_matches(".md");
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
    /// contra marcas falsas y saneado de cabeceras (§4.8).
    pub fn publicar_entrega(
        &self,
        propuesta: &str,
        veredicto: &str,
        leido: &ResumenLeido,
        ahora: &str,
    ) -> Resultado<()> {
        let cuerpo = format!(
            "He dejado la propuesta «{propuesta}» en disco.\n\n**Comprobación de las propuestas** (la hace el arnés, no el modelo):\n- {veredicto}"
        );
        self.publicar(&cuerpo, leido, ahora)
    }
}

/// La marca de tiempo actual local en el formato que exige el protocolo (§3.2): `2026-09-13 08:30`.
pub fn marca_de_tiempo_actual() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// Longitud máxima de la primera línea en el latido antes de recortarla con elipsis (§4.3).
pub const TOPE_CARACTERES_RESUMEN: usize = 120;
/// Caracteres conservados de la primera línea antes de añadir el carácter de elipsis «…».
pub const CARACTERES_PRE_ELIPSIS: usize = 117;

/// Primera línea del cuerpo, recortada, para que el latido diga de un vistazo en qué se está.
fn primera_linea(cuerpo: &str) -> String {
    let linea = cuerpo.lines().next().unwrap_or("").trim();
    if linea.is_empty() {
        return "sin novedad".to_string();
    }
    if linea.chars().count() > TOPE_CARACTERES_RESUMEN {
        let recortada: String = linea.chars().take(CARACTERES_PRE_ELIPSIS).collect();
        format!("{recortada}…")
    } else {
        linea.to_string()
    }
}

/// Extrae el contenido de mensajes o latidos previos de un buzón para su rotación a histórico.
fn extraer_contenido_rotar(contenido: &str) -> &str {
    if let Some(pos) = contenido.find("**LATIDO:**") {
        &contenido[pos..]
    } else if let Some(resto) = contenido.strip_prefix(ENCABEZADO) {
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
        let dir = tempfile::tempdir().unwrap();
        let canal_dir = dir.path().join(".gestor/canal");
        std::fs::create_dir_all(&canal_dir).unwrap();
        std::fs::write(
            canal_dir.join("claude.md"),
            "# Claude\ncontenido de Claude\n",
        )
        .unwrap();
        std::fs::write(canal_dir.join("codex.md"), "# Codex\ncontenido de Codex\n").unwrap();
        std::fs::write(canal_dir.join("programator.md"), "# Programator\nlo mío\n").unwrap();
        std::fs::write(canal_dir.join("estado.md"), "# Estado\n").unwrap();
        let canal = Canal::nuevo(dir.path(), "Programator").unwrap();
        (dir, canal)
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
        let (dir, canal) = canal_de_prueba();
        std::fs::write(
            dir.path()
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
        let dir = tempfile::tempdir().unwrap();

        let canal = Canal::nuevo(dir.path(), "Programator").unwrap();

        assert!(dir.path().join(".gestor/canal").is_dir());
        assert!(canal.buzones_ajenos().unwrap().is_empty());
    }

    #[test]
    fn un_historico_no_es_un_buzon_y_no_se_vuelve_a_atender() {
        // El 23/09/2026 el encargo 009 se archivó a las 20:44 y se entregó a las 20:47: los
        // históricos viven dentro de la carpeta que el arnés sondea, así que archivar es volver
        // a encargar.
        let dir = tempfile::tempdir().unwrap();
        let canal_dir = dir.path().join(".gestor").join("canal");
        std::fs::create_dir_all(&canal_dir).unwrap();
        std::fs::write(canal_dir.join("claude.md"), "## Para Programator\n\nvivo").unwrap();
        std::fs::write(
            canal_dir.join("historico-actas.md"),
            "## Para Programator\n\nya cerrado",
        )
        .unwrap();

        let canal = Canal::nuevo(dir.path(), "programator").unwrap();
        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(nombres, vec!["claude.md"], "el histórico no es un buzón");
    }

    #[test]
    fn descarta_nombres_reservados_insensiblemente_a_mayusculas() {
        let dir = tempfile::tempdir().unwrap();
        let canal_dir = dir.path().join(".gestor/canal");
        std::fs::create_dir_all(&canal_dir).unwrap();
        std::fs::write(canal_dir.join("README.md"), "# Reglas\n").unwrap();
        std::fs::write(canal_dir.join("INDICE.md"), "# Índice\n").unwrap();
        std::fs::write(canal_dir.join("Claude.md"), "# Claude\ncontenido\n").unwrap();
        let canal = Canal::nuevo(dir.path(), "Programator").unwrap();

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
        let dir = tempfile::tempdir().unwrap();
        let canal_dir = dir.path().join(".gestor/canal");
        std::fs::create_dir_all(&canal_dir).unwrap();
        std::fs::write(canal_dir.join("Programator.md"), "# Programator\nlo mío\n").unwrap();
        std::fs::write(canal_dir.join("Claude.md"), "# Claude\ncontenido\n").unwrap();
        let canal = Canal::nuevo(dir.path(), "Programator").unwrap();

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
    fn con_prefijos_ignorados_vacio_conserva_los_prefijos_por_defecto() {
        // Si una lista vacía dejara al arnés sin prefijos que ignorar, un proyecto sin
        // `prefijos_historico` en el TOML volvería a atender lo ya archivado: el mismo fallo que
        // costó el encargo 009 el 23/09/2026.
        let dir = tempfile::tempdir().unwrap();
        let canal_dir = dir.path().join(".gestor/canal");
        std::fs::create_dir_all(&canal_dir).unwrap();
        std::fs::write(canal_dir.join("claude.md"), "## Para Programator\n\nvivo").unwrap();
        std::fs::write(
            canal_dir.join("historico-actas.md"),
            "## Para Programator\n\nya cerrado",
        )
        .unwrap();

        let canal = Canal::nuevo(dir.path(), "Programator")
            .unwrap()
            .con_prefijos_ignorados(Vec::new());
        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            nombres,
            vec!["claude.md"],
            "una lista vacía debe conservar los prefijos por defecto: {nombres:?}"
        );
    }

    #[test]
    fn con_prefijos_ignorados_personalizados_sustituye_a_los_de_por_defecto() {
        let dir = tempfile::tempdir().unwrap();
        let canal_dir = dir.path().join(".gestor/canal");
        std::fs::create_dir_all(&canal_dir).unwrap();
        std::fs::write(canal_dir.join("claude.md"), "## Para Programator\n\nvivo").unwrap();
        std::fs::write(
            canal_dir.join("historico-actas.md"),
            "## Para Programator\n\nya cerrado, con el prefijo por defecto",
        )
        .unwrap();
        std::fs::write(
            canal_dir.join("archivado-actas.md"),
            "## Para Programator\n\nya cerrado, con el prefijo del proyecto",
        )
        .unwrap();

        let canal = Canal::nuevo(dir.path(), "Programator")
            .unwrap()
            .con_prefijos_ignorados(vec!["archivado-".to_string()]);
        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert!(
            nombres.contains(&"historico-actas.md"),
            "el prefijo por defecto ya no debe aplicar: la lista personalizada lo sustituye, \
             no lo añade: {nombres:?}"
        );
        assert!(
            !nombres.contains(&"archivado-actas.md"),
            "el prefijo personalizado sí debe descartarlo: {nombres:?}"
        );
    }

    #[test]
    fn con_prefijos_ignorados_compara_insensible_a_mayusculas() {
        let dir = tempfile::tempdir().unwrap();
        let canal_dir = dir.path().join(".gestor/canal");
        std::fs::create_dir_all(&canal_dir).unwrap();
        std::fs::write(canal_dir.join("claude.md"), "## Para Programator\n\nvivo").unwrap();
        std::fs::write(
            canal_dir.join("Historico-Actas.md"),
            "## Para Programator\n\nya cerrado, con mayúsculas en el nombre",
        )
        .unwrap();

        let canal = Canal::nuevo(dir.path(), "Programator")
            .unwrap()
            .con_prefijos_ignorados(vec!["HISTORICO-".to_string()]);
        let buzones = canal.buzones_ajenos().unwrap();

        let nombres: Vec<&str> = buzones.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            nombres,
            vec!["claude.md"],
            "un prefijo en mayúsculas debe descartar un fichero cuyo nombre también lleva \
             mayúsculas: {nombres:?}"
        );
    }

    #[test]
    fn la_publicacion_lleva_latido_y_leido_aunque_el_modelo_no_los_escriba() {
        let (dir, canal) = canal_de_prueba();
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
            std::fs::read_to_string(dir.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(buzon.contains("**LATIDO:** 2026-09-13 16:20"));
        assert!(buzon.contains("**LEÍDO:**"));
        assert!(buzon.contains("`claude.md` hasta su latido 14:00"));
        assert!(buzon.contains("446 en verde"));
    }

    #[test]
    fn la_publicacion_conserva_lo_que_ya_habia_en_el_buzon() {
        let (dir, canal) = canal_de_prueba();
        let leido = ResumenLeido { entradas: vec![] };

        canal
            .publicar("Bloque nuevo.", &leido, "2026-09-13 16:20")
            .unwrap();

        let buzon =
            std::fs::read_to_string(dir.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(
            buzon.contains("lo mío"),
            "no puede perder el contenido anterior"
        );
        assert!(buzon.contains("Bloque nuevo."));
    }

    #[test]
    fn rotar_a_historico_mueve_mensajes_anteriores_y_deja_encabezado() {
        let (dir, canal) = canal_de_prueba();
        canal.rotar_a_historico().unwrap();

        let buzon =
            std::fs::read_to_string(dir.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(!buzon.contains("lo mío"));
        assert!(buzon.starts_with("# Programator"));

        let hist =
            std::fs::read_to_string(dir.path().join(".gestor/canal/historico-programator.md"))
                .unwrap();
        assert!(hist.contains("lo mío"));
    }

    #[test]
    fn rotar_a_historico_acumula_sucesivamente_en_historico() {
        let dir = tempfile::tempdir().unwrap();
        let canal = Canal::nuevo(dir.path(), "Programator").unwrap();
        let leido = ResumenLeido { entradas: vec![] };

        canal
            .publicar("Primer mensaje.", &leido, "2026-09-13 16:00")
            .unwrap();

        canal.rotar_a_historico().unwrap();

        let buzon1 =
            std::fs::read_to_string(dir.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(!buzon1.contains("Primer mensaje."));
        assert!(buzon1.starts_with("# Programator"));

        let hist1 =
            std::fs::read_to_string(dir.path().join(".gestor/canal/historico-programator.md"))
                .unwrap();
        assert!(hist1.contains("Primer mensaje."));

        canal
            .publicar("Segundo mensaje.", &leido, "2026-09-13 16:10")
            .unwrap();

        canal.rotar_a_historico().unwrap();

        let hist2 =
            std::fs::read_to_string(dir.path().join(".gestor/canal/historico-programator.md"))
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
        let dir = tempfile::tempdir().unwrap();
        let canal = Canal::nuevo(dir.path(), "Programator").unwrap();
        let leido = ResumenLeido { entradas: vec![] };

        canal
            .publicar("Primer mensaje.", &leido, "2026-09-13 16:20")
            .unwrap();

        let buzon =
            std::fs::read_to_string(dir.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(buzon.starts_with("# Programator"));
        assert!(buzon.contains("Solo escribe Programator"));
    }

    #[test]
    fn la_publicacion_no_toca_ningun_buzon_ajeno() {
        let (dir, canal) = canal_de_prueba();
        let antes = std::fs::read_to_string(dir.path().join(".gestor/canal/claude.md")).unwrap();
        let leido = ResumenLeido { entradas: vec![] };

        canal.publicar("Algo.", &leido, "2026-09-13 16:20").unwrap();

        let despues = std::fs::read_to_string(dir.path().join(".gestor/canal/claude.md")).unwrap();
        assert_eq!(antes, despues, "la regla 1 del canal es inviolable");
    }

    #[test]
    fn neutraliza_latido_falsificado_en_el_cuerpo() {
        let (dir, canal) = canal_de_prueba();
        let leido = ResumenLeido { entradas: vec![] };

        canal
            .publicar(
                "**LATIDO:** 1999-01-01 00:00 — mentira\n**LEÍDO:** todo mentira",
                &leido,
                "2026-09-13 16:20",
            )
            .unwrap();

        let buzon =
            std::fs::read_to_string(dir.path().join(".gestor/canal/programator.md")).unwrap();

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
        let (dir, canal) = canal_de_prueba();
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
            std::fs::read_to_string(dir.path().join(".gestor/canal/programator.md")).unwrap();

        // La línea LEÍDO: debe ser una sola línea
        let leido_linea = buzon
            .lines()
            .find(|l| l.starts_with("**LEÍDO:**"))
            .expect("debe haber una línea LEÍDO:");
        assert!(
            !leido_linea.contains('\n'),
            "la línea LEÍDO: no puede tener saltos de línea"
        );

        // No debe contener literalmente **LATIDO: en posición de marca
        let has_fake_mark = buzon
            .lines()
            .any(|l| l.starts_with("**LATIDO:**") && !l.contains("2026-09-13 16:20"));
        assert!(!has_fake_mark, "no debe haber marca LATIDO: falsificada");
    }

    #[test]
    fn usa_sin_novedad_cuando_el_cuerpo_es_solo_espacios() {
        let (dir, canal) = canal_de_prueba();
        let leido = ResumenLeido { entradas: vec![] };

        canal.publicar("   ", &leido, "2026-09-13 16:20").unwrap();

        let buzon =
            std::fs::read_to_string(dir.path().join(".gestor/canal/programator.md")).unwrap();
        assert!(
            buzon.contains("**LATIDO:** 2026-09-13 16:20 — sin novedad"),
            "cuerpo solo espacios debe usar sin novedad"
        );
    }
}
