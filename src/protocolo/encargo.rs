//! Detección de lo que va dirigido a un agente. Son reglas textuales deliberadamente estrictas:
//! mencionar a alguien no es encargarle trabajo.
//!
//! Se reconocen tres formas, y conviene saber que **no valen lo mismo**:
//!
//! 1. **La línea que empieza por `Para <agente>`** es la forma vigente del canal, la que usa el
//!    gestor de agentes que coordina el equipo. Así escriben sus agentes:
//!    `Para Gemini 1: …`, `Para Claude 1 y Programator: …`. El encargo llega hasta el siguiente
//!    `LATIDO:`, la siguiente línea `Para …`, un encabezado `## ` o un `---`. Quien solo va «con
//!    copia» no tiene encargo, y la instancia cuenta: `Para Programator 2` no es para la 1.
//! 2. **El encabezado `## Para <agente>`** es la forma del protocolo anterior. Se
//!    sigue atendiendo para no perder encargos escritos así.
//! 3. **El marcador `[→ <agente>]`** es una extensión propia de Programator; **el protocolo del
//!    equipo no lo contempla y ningún otro agente lo escribe hoy**. Se mantiene porque permite
//!    dirigir una línea suelta a Programator dentro de una sección destinada a otros, cosa que el
//!    encabezado no puede hacer, y porque no reconocerlo costaría perder un encargo escrito así.
//!    No debe presentarse a nadie como parte del protocolo.

use super::destinatarios::{self, Identidad};

/// Un bloque de texto que un agente dirige a otro.
#[derive(Debug, Clone)]
pub struct Encargo {
    /// Fichero del buzón de origen, por ejemplo `codex.md`.
    pub de: String,
    /// Texto íntegro del encargo.
    pub texto: String,
    /// Cómo venía escrito.
    pub forma: Forma,
}

impl Encargo {
    /// Lo que se pide, sin la línea `Para …` que lo dirige: lo que va tras los dos puntos de esa
    /// línea, o, si no los lleva, lo que va debajo. Es lo que mira quien reconoce un pedido por su
    /// primera línea, como la revisión (`revisión: buscar`). En las otras formas, el texto entero.
    /// Los dos puntos son los del «Para» que nombra a `agente`: ni los de una hora delante
    /// («13:00 — Para Programator: …») ni los de otro «Para» de la misma línea.
    pub fn cuerpo(&self, agente: &str) -> &str {
        if self.forma != Forma::Linea {
            return &self.texto;
        }
        let primera = self.texto.lines().next().unwrap_or_default();
        let resto =
            destinatarios::tras_los_dos_puntos(primera, &Identidad::de(agente)).unwrap_or_default();
        let desde = self.texto.len().min(primera.len() - resto.len());
        // `**Para Programator:** revisión: buscar` deja la negrita de cierre delante.
        self.texto[desde..]
            .trim_start()
            .trim_start_matches(ENFASIS)
            .trim()
    }
}

/// Cómo venía escrito un encargo. Importa para lo que se hace después con el buzón de origen: la
/// propuesta de poda trocea por `---`, como los buzones del protocolo anterior, y un buzón de Gestor
/// Agentes no lo usa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Forma {
    /// Una línea que empieza por `Para <agente>` (forma vigente del canal).
    Linea,
    /// Bajo un encabezado `## Para <agente>` (protocolo anterior).
    Encabezado,
    /// Una línea con el marcador `[→ <agente>]`.
    Marcador,
}

impl Forma {
    fn de_la_seccion(abierta_por_linea: bool) -> Self {
        if abierta_por_linea {
            Forma::Linea
        } else {
            Forma::Encabezado
        }
    }
}

/// Extrae los encargos dirigidos a `agente` dentro del contenido de un buzón ajeno.
///
/// Quién es destinatario lo decide [`destinatarios`], con las reglas de Gestor Agentes: un «Para»
/// con mayúscula en cualquier sitio de la línea, también citado. Dónde acaba el encargo es regla
/// propia: el modelo necesita el texto entero, también lo que va tras una línea en blanco.
pub fn detectar(agente: &str, de: &str, contenido: &str) -> Vec<Encargo> {
    let marcador = format!("[→ {}]", agente.to_lowercase());
    let mut encargos = Vec::new();

    let identidad = Identidad::de(agente);
    let mut seccion: Option<Vec<String>> = None;
    // Si la sección abierta la abrió una línea `Para …` y no un encabezado: esas se cierran también
    // con el siguiente latido, la siguiente línea `Para …` o un `---`.
    let mut abierta_por_linea = false;

    for linea in contenido.lines() {
        let recortada = linea.trim();
        let minusculas = recortada.to_lowercase();

        if abierta_por_linea && cierra_un_encargo_en_linea(recortada) {
            cerrar_seccion(seccion.take(), Forma::Linea, de, &mut encargos);
            abierta_por_linea = false;
        }

        let es_encabezado = minusculas.starts_with("## ");
        if seccion.is_none() && !es_encabezado && identidad.es_destinatario_de(recortada) {
            seccion = Some(vec![recortada.to_string()]);
            abierta_por_linea = true;
            continue;
        }

        if es_encabezado {
            // Un encabezado nuevo cierra la sección anterior.
            let forma = Forma::de_la_seccion(abierta_por_linea);
            abierta_por_linea = false;
            cerrar_seccion(seccion.take(), forma, de, &mut encargos);
            if identidad.es_destinatario_de(recortada) {
                let texto_inicial = destinatarios::tras_los_dos_puntos(recortada, &identidad)
                    .map(str::trim)
                    .filter(|texto| !texto.is_empty())
                    .map(str::to_string);
                seccion = Some(texto_inicial.into_iter().collect());
            }
            continue;
        }

        if let Some(acumulado) = seccion.as_mut() {
            acumulado.push(recortada.to_string());
        } else if minusculas.contains(&marcador) {
            encargos.push(Encargo {
                de: de.to_string(),
                texto: recortada.to_string(),
                forma: Forma::Marcador,
            });
        }
    }

    cerrar_seccion(
        seccion.take(),
        Forma::de_la_seccion(abierta_por_linea),
        de,
        &mut encargos,
    );

    encargos
}

/// La línea que separa bloques en un buzón.
const SEPARADOR_DE_BLOQUES: &str = "---";
/// Las marcas de énfasis de markdown que pueden envolver lo que sigue a los dos puntos.
const ENFASIS: [char; 2] = ['*', '_'];

/// Si la línea termina un encargo que empezó en una línea `Para …`: el siguiente latido, un
/// separador de bloques o una línea que encarga algo a otro agente, o a este otra vez, con las
/// reglas de [`destinatarios`] (como `cierra_el_bloque` de Gestor Agentes). Para no cortar el
/// encargo con una frase como «Para ello, lee…», un «Para» solo cierra si su primer destinatario
/// empieza por mayúscula, como todo nombre de agente. La línea en blanco no cierra: es la regla
/// propia del fin del bloque.
fn cierra_un_encargo_en_linea(recortada: &str) -> bool {
    crate::protocolo::es_linea_de_latido(recortada)
        || recortada == SEPARADOR_DE_BLOQUES
        || destinatarios::destinatarios(recortada)
            .iter()
            .any(|destinatario| destinatario.chars().next().is_some_and(char::is_uppercase))
}

/// Si había una sección abierta con algo dentro, la cierra como encargo.
fn cerrar_seccion(
    seccion: Option<Vec<String>>,
    forma: Forma,
    de: &str,
    encargos: &mut Vec<Encargo>,
) {
    let Some(acumulado) = seccion else {
        return;
    };
    let texto = acumulado.join("\n").trim().to_string();
    if !texto.is_empty() {
        encargos.push(Encargo {
            de: de.to_string(),
            texto,
            forma,
        });
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn detecta_un_encabezado_dirigido_solo_a_el() {
        let texto = "# Codex\n\n## Para Programator\n\nRevisa las puertas.\n\n## Para Claude\n\nOtra cosa.\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("Revisa las puertas"));
        assert!(
            !encargos[0].texto.contains("Otra cosa"),
            "no debe invadir la sección siguiente"
        );
        assert_eq!(encargos[0].de, "codex.md");
    }

    #[test]
    fn detecta_un_encabezado_compartido() {
        let texto = "## Para Claude y Programator\n\nAcordamos el formato.\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("Acordamos el formato"));
    }

    #[test]
    fn detecta_un_marcador_en_mitad_del_texto() {
        let texto =
            "## Para Claude\n\nUn párrafo normal.\n\n[→ Programator] ejecuta las pruebas.\n";

        let encargos = detectar("Programator", "gemini.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("ejecuta las pruebas"));
    }

    #[test]
    fn ignora_un_encabezado_dirigido_a_otro() {
        let texto = "## Para Claude\n\nNada mío.\n";

        assert!(detectar("Programator", "codex.md", texto).is_empty());
    }

    #[test]
    fn ignora_una_mencion_suelta_sin_marcador() {
        let texto = "## Para Claude\n\nCreo que Programator debería encargarse algún día.\n";

        assert!(
            detectar("Programator", "codex.md", texto).is_empty(),
            "mencionar a un agente no es encargarle trabajo"
        );
    }

    #[test]
    fn para_en_minuscula_no_es_encargo_como_en_gestor_agentes() {
        // «## para programator» no cuenta: Gestor Agentes exige «Para» con mayúscula, y los dos
        // tienen que leer igual quién es destinatario.
        let texto = "## para programator\n\nen minúsculas.\n";

        assert!(detectar("Programator", "codex.md", texto).is_empty());
    }

    #[test]
    fn detecta_encabezado_con_dos_puntos_seguido_de_cuerpo() {
        let texto = "## Para Programator:\n\nRevisa el código.\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("Revisa el código"));
    }

    #[test]
    fn detecta_encabezado_con_punto_seguido_de_cuerpo() {
        let texto = "## Para Programator.\n\nHaz la compilación.\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("Haz la compilación"));
    }

    #[test]
    fn detecta_texto_tras_dos_puntos_sin_cuerpo_debajo() {
        let texto = "## Para Programator: revisa las puertas.";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("revisa las puertas"));
    }

    #[test]
    fn detecta_texto_tras_dos_puntos_con_cuerpo_debajo() {
        let texto = "## Para Programator: revisa esto\n\nY también comprueba las normas.\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("revisa esto"));
        assert!(encargos[0].texto.contains("comprueba las normas"));
    }

    #[test]
    fn no_detecta_parapente() {
        let texto = "## Parapente\n\nSaltar en las montañas.\n";

        assert!(detectar("Programator", "codex.md", texto).is_empty());
    }

    #[test]
    fn no_detecta_programatorcito() {
        let texto = "## Para Programatorcito\n\nEsto es otro agente.\n";

        assert!(detectar("Programator", "codex.md", texto).is_empty());
    }

    #[test]
    fn no_duplica_marcador_dentro_de_seccion_propia() {
        let texto = "## Para Programator\n\nTarea inicial.\n\n[→ Programator] esto es un marcador.\n\nMás texto.\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1, "debe tener un solo encargo, no dos");
        assert!(encargos[0].texto.contains("Tarea inicial"));
        assert!(encargos[0].texto.contains("esto es un marcador"));
    }

    #[test]
    fn no_detecta_falso_positivo_cuando_nombre_aparece_tras_dos_puntos() {
        let texto = "## Para Claude: recuerda avisar a Programator\n";

        assert!(
            detectar("Programator", "codex.md", texto).is_empty(),
            "no debe detectar si el nombre está tras dos puntos de otro agente"
        );
    }

    #[test]
    fn ignora_contenido_entre_parentesis_y_dos_puntos() {
        let texto = "## Para Programator (ver: detalles abajo)\n\nCuerpo real aquí.\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("Cuerpo real"));
        assert!(
            !encargos[0].texto.contains("detalles abajo"),
            "no debe contaminar el cuerpo con texto entre paréntesis"
        );
    }

    #[test]
    fn detecta_cuando_varios_destinatarios_con_dos_puntos() {
        let texto = "## Para Claude y Programator: revisa esto\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("revisa esto"));
    }

    #[test]
    fn no_detecta_claude_con_programator_en_parentesis() {
        let texto = "## Para Claude (menciona a Programator aquí)\n";

        assert!(
            detectar("Programator", "codex.md", texto).is_empty(),
            "no debe detectar si el nombre está en un paréntesis de otro agente"
        );
    }

    #[test]
    fn detecta_agente_con_enfasis_markdown() {
        let texto = "## Para *Programator*\n\nEjecuta las pruebas.\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("Ejecuta las pruebas"));
    }

    #[test]
    fn no_detecta_guion_interno_a_la_palabra() {
        let texto = "## Para Programator-bis\n\nEsto es para otro agente.\n";

        assert!(
            detectar("Programator", "codex.md", texto).is_empty(),
            "el guion es puntuación interna a la palabra: «programator-bis» no es «programator»"
        );
    }

    #[test]
    fn detecta_cuando_programator_va_primero_entre_varios_destinatarios() {
        let texto = "## Para Programator y Gemini: revisión\n";

        let encargos = detectar("Programator", "codex.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("revisión"));
    }

    // --- La línea `Para <agente>` del canal ---

    /// Un buzón como los del canal: intervenciones que empiezan con `LATIDO:` y encargos
    /// en una línea, para varios agentes.
    const BUZON_DE_GESTOR: &str = "# Claude\n\n\
        LATIDO: 2026-11-13 12:22:06\n\n\
        Para Gemini 1: sigue con VERIF-3.2.0.\n\n\
        LATIDO: 2026-11-13 13:19:25\n\n\
        Para Programator: revisa src/main.rs.\n\
        Busca números sueltos.\n\n\
        Y dime cuántos hay.\n\
        Para Gemini 1: tú no toques nada.\n\n\
        LATIDO: 2026-11-13 13:41:33\n\n\
        Hablé con el Director. Para Programator no hay nada más hoy.\n";

    #[test]
    fn una_linea_para_programator_es_un_encargo_hasta_la_siguiente_linea_para() {
        let encargos = detectar("Programator", "claude.md", BUZON_DE_GESTOR);

        // El segundo es «Para Programator no hay nada más hoy», a mitad de frase: con las reglas de
        // Gestor Agentes también es un encargo (acuerdo del 22/11/2026).
        assert_eq!(encargos.len(), 2, "{encargos:?}");
        assert_eq!(
            encargos[0].texto,
            "Para Programator: revisa src/main.rs.\nBusca números sueltos.\n\nY dime cuántos hay."
        );
        assert_eq!(encargos[0].de, "claude.md");
    }

    #[test]
    fn nombrarlo_sin_para_no_es_encargo_y_para_a_mitad_de_frase_si() {
        // «Para Programator» a mitad de frase cuenta: Gestor Agentes lo lee como encargo, y los dos
        // tienen que leer igual quién es destinatario.
        let sin_para = detectar(
            "Programator",
            "claude.md",
            "LATIDO: 2026-11-13 13:41:33\n\nSe lo diré luego a Programator.\n",
        );
        assert!(sin_para.is_empty(), "{sin_para:?}");

        let a_mitad = detectar(
            "Programator",
            "claude.md",
            "LATIDO: 2026-11-13 13:41:33\n\nHablé con el Director. Para Programator: x.\n",
        );
        assert_eq!(a_mitad.len(), 1, "{a_mitad:?}");
        assert_eq!(a_mitad[0].cuerpo("Programator"), "x.");
    }

    #[test]
    fn el_encargo_en_linea_acaba_en_el_siguiente_latido() {
        let encargos = detectar(
            "Programator",
            "claude.md",
            "Para Programator: primero.\n\nLATIDO: 2026-11-13 14:00:00\n\nOtra cosa.\n",
        );

        assert_eq!(encargos.len(), 1);
        assert_eq!(encargos[0].texto, "Para Programator: primero.");
    }

    #[test]
    fn el_encargo_en_linea_acaba_en_un_separador_o_un_encabezado() {
        let con_separador = detectar(
            "Programator",
            "claude.md",
            "Para Programator: uno.\n---\nResto.\n",
        );
        let con_encabezado = detectar(
            "Programator",
            "claude.md",
            "Para Programator: uno.\n## Notas\nResto.\n",
        );

        assert_eq!(con_separador[0].texto, "Para Programator: uno.");
        assert_eq!(con_encabezado[0].texto, "Para Programator: uno.");
    }

    #[test]
    fn dos_lineas_seguidas_para_programator_son_dos_encargos() {
        let encargos = detectar(
            "Programator",
            "claude.md",
            "Para Programator: uno.\nPara Programator: dos.\n",
        );

        let textos: Vec<&str> = encargos.iter().map(|e| e.texto.as_str()).collect();
        assert_eq!(textos, ["Para Programator: uno.", "Para Programator: dos."]);
    }

    #[test]
    fn vale_con_numero_de_instancia_y_entre_varios_destinatarios() {
        for linea in [
            "Para Programator 1: haz esto.",
            "Para Claude 1 y Programator: haz esto.",
            "Para Programator, Gemini 1: haz esto.",
            "**Para Programator:** haz esto.",
            "Para Programator haz esto.",
            // Con las reglas de Gestor Agentes (22/11/2026), también citado o en una lista.
            "> Para Programator: haz esto.",
            "- Para Programator: haz esto.",
        ] {
            let encargos = detectar("Programator", "claude.md", linea);
            assert_eq!(encargos.len(), 1, "debía ser un encargo: {linea}");
        }
    }

    #[test]
    fn no_vale_otra_instancia_ni_la_copia_ni_minuscula_ni_otro_nombre() {
        for linea in [
            "Para Programator 2: haz esto.",
            "Para Gemini 1, con copia a Programator. Haz esto.",
            "para programator: haz esto.",
            "Para Programatorcito: haz esto.",
            "Para Programator-bis: haz esto.",
            "Parapente con Programator.",
        ] {
            let encargos = detectar("Programator", "claude.md", linea);
            assert!(encargos.is_empty(), "no debía ser un encargo: {linea}");
        }
    }

    #[test]
    fn la_instancia_2_atiende_lo_suyo_y_no_lo_de_la_1() {
        let texto = "Para Programator: para la 1.\nPara Programator 2: para la 2.\n";

        let encargos = detectar("Programator 2", "claude.md", texto);

        assert_eq!(encargos.len(), 1);
        assert_eq!(encargos[0].texto, "Para Programator 2: para la 2.");
    }

    #[test]
    fn una_linea_para_dentro_de_un_encabezado_propio_no_lo_duplica() {
        let texto = "## Para Programator\n\nTarea.\nPara Programator: detalle.\n";

        let encargos = detectar("Programator", "claude.md", texto);

        assert_eq!(encargos.len(), 1);
        assert!(encargos[0].texto.contains("detalle"));
    }

    #[test]
    fn dos_vinetas_seguidas_son_dos_encargos_y_la_de_otro_cierra_la_primera() {
        let dos = detectar(
            "Programator",
            "claude2.md",
            "- **Para Programator (15:00): uno.**\n- **Para Programator (15:01): dos.**\n",
        );
        assert_eq!(dos.len(), 2, "{dos:?}");

        let con_otro = detectar(
            "Programator",
            "claude2.md",
            "- **Para Programator (15:00): uno.**\n- **Para Gemini 1 (15:01): dos.**\n",
        );
        assert_eq!(con_otro.len(), 1, "{con_otro:?}");
        assert!(!con_otro[0].texto.contains("Gemini"), "{con_otro:?}");
    }

    #[test]
    fn una_linea_que_empieza_por_para_en_minuscula_o_para_ello_no_corta_el_encargo() {
        let encargos = detectar(
            "Programator",
            "claude.md",
            "Para Programator: revisa src/main.rs.\npara comprobar si falta algo, mira las pruebas.\n\
             Para ello, lee primero el README.\n",
        );

        assert_eq!(encargos.len(), 1, "{encargos:?}");
        assert!(encargos[0].texto.contains("mira las pruebas"));
        assert!(encargos[0].texto.contains("lee primero el README"));
    }

    #[test]
    fn el_cuerpo_sale_del_para_que_nombra_al_agente() {
        let encargos = detectar(
            "Programator",
            "claude.md",
            "Para Gemini 1: x. Para Programator: revisión: buscar\n",
        );

        assert_eq!(encargos[0].cuerpo("Programator"), "revisión: buscar");
    }

    #[test]
    fn el_cuerpo_de_un_encargo_en_linea_no_lleva_la_linea_para() {
        let cuerpo = |texto: &str| {
            let encargos = detectar("Programator", "claude.md", texto);
            encargos[0].cuerpo("Programator").to_string()
        };

        assert_eq!(
            cuerpo("Para Programator: revisión: buscar\n"),
            "revisión: buscar"
        );
        assert_eq!(
            cuerpo("Para Programator:\nrevisión: probar\nrepo/a a.gguf\n"),
            "revisión: probar\nrepo/a a.gguf"
        );
        assert_eq!(
            cuerpo("**Para Programator:** revisión: cerrar"),
            "revisión: cerrar"
        );
        assert_eq!(
            cuerpo("Para Programator\nrevisión: buscar"),
            "revisión: buscar"
        );
        assert_eq!(
            cuerpo("## Para Programator\n\nrevisión: buscar\n"),
            "revisión: buscar",
            "con encabezado, el texto entero, como antes"
        );
    }
}
