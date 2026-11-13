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
    pub fn cuerpo(&self) -> &str {
        if self.forma != Forma::Linea {
            return &self.texto;
        }
        let primera = self.texto.lines().next().unwrap_or_default();
        let desde = match primera.find(':') {
            Some(dos_puntos) => dos_puntos + 1,
            None => primera.len(),
        };
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub fn detectar(agente: &str, de: &str, contenido: &str) -> Vec<Encargo> {
    let objetivo = agente.to_lowercase();
    let marcador = format!("[→ {}]", objetivo);
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

        if seccion.is_none() {
            if let Some(destinatarios) = destinatarios_de_una_linea_para(recortada) {
                if identidad.esta_entre(&destinatarios) {
                    seccion = Some(vec![recortada.to_string()]);
                    abierta_por_linea = true;
                    continue;
                }
            }
        }

        if minusculas.starts_with("## ") {
            // Un encabezado nuevo cierra la sección anterior.
            let forma = Forma::de_la_seccion(abierta_por_linea);
            abierta_por_linea = false;
            cerrar_seccion(seccion.take(), forma, de, &mut encargos);
            if es_encabezado_para(&minusculas, &objetivo) {
                let texto_inicial = extraer_texto_tras_dos_puntos_en_encabezado(&minusculas);
                if let Some(t) = texto_inicial {
                    seccion = Some(vec![t]);
                } else {
                    seccion = Some(Vec::new());
                }
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

/// Lo que va delante de los destinatarios en una línea de encargo.
const PARA: &str = "para ";
/// Lo que separa a los destinatarios de quienes solo reciben copia, como escriben los agentes del
/// canal («Para Gemini 1, con copia a Claude 2.»). Quien va en copia no tiene encargo.
const CON_COPIA: &str = "con copia";
/// Las conjunciones que unen destinatarios: «Para Claude 1 y Programator», «Gemini e Iris».
const CONJUNCIONES: [&str; 2] = [" y ", " e "];
/// La línea que separa bloques en un buzón.
const SEPARADOR_DE_BLOQUES: &str = "---";
/// Las marcas de énfasis de markdown que pueden envolver el principio de la línea: `**Para …**`.
const ENFASIS: [char; 2] = ['*', '_'];

/// Quién es este agente para los encargos en una línea: la herramienta y el número de instancia.
/// «Programator» es la instancia 1, igual que «Programator 1»; «Programator 2», la 2. Es como
/// el gestor de agentes nombra a sus agentes al revisar el canal.
struct Identidad {
    herramienta: String,
    instancia: u32,
}

/// La instancia que se entiende cuando el nombre no lleva número.
const INSTANCIA_SIN_NUMERO: u32 = 1;

impl Identidad {
    fn de(agente: &str) -> Self {
        let mut palabras = agente.split_whitespace();
        let herramienta = palabras.next().unwrap_or_default().to_lowercase();
        let instancia = palabras
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(INSTANCIA_SIN_NUMERO);
        Self {
            herramienta,
            instancia,
        }
    }

    /// Si alguno de los destinatarios es este agente. Un destinatario es lo que hay entre comas o
    /// conjunciones; vale si su primera palabra es la herramienta y, si lleva número, es el suyo.
    fn esta_entre(&self, destinatarios: &str) -> bool {
        let mut trozos = vec![destinatarios.to_string()];
        for conjuncion in CONJUNCIONES {
            trozos = trozos
                .iter()
                .flat_map(|t| t.split(conjuncion).map(str::to_string).collect::<Vec<_>>())
                .collect();
        }
        trozos
            .iter()
            .flat_map(|t| t.split(',').map(str::to_string).collect::<Vec<_>>())
            .any(|destinatario| {
                let mut palabras = destinatario
                    .split_whitespace()
                    .map(|p| p.trim_matches(|c: char| !c.is_alphanumeric()));
                if palabras.next() != Some(self.herramienta.as_str()) {
                    return false;
                }
                // Sin número detrás («Para Programator», «Para Programator haz…»), la instancia 1.
                match palabras.next().map(str::parse::<u32>) {
                    Some(Ok(numero)) => numero == self.instancia,
                    _ => self.instancia == INSTANCIA_SIN_NUMERO,
                }
            })
    }
}

/// Si la línea empieza por `Para`, los destinatarios, en minúsculas y sin los que van en copia.
/// Una línea citada (`> Para …`) o una mención en medio de una frase no empiezan por `Para`.
fn destinatarios_de_una_linea_para(recortada: &str) -> Option<String> {
    let sin_enfasis = recortada.trim_start_matches(ENFASIS).to_lowercase();
    let resto = sin_enfasis.strip_prefix(PARA)?;
    let (destinatarios, _) = extraer_region_de_destinatarios(resto);
    let sin_copia = match destinatarios.find(CON_COPIA) {
        Some(posicion) => &destinatarios[..posicion],
        None => destinatarios,
    };
    Some(sin_copia.trim().to_string())
}

/// Si la línea termina un encargo que empezó en una línea `Para …`: el siguiente latido, la
/// siguiente línea `Para …` (sea para quien sea) o un separador de bloques.
fn cierra_un_encargo_en_linea(recortada: &str) -> bool {
    crate::protocolo::es_linea_de_latido(recortada)
        || recortada == SEPARADOR_DE_BLOQUES
        || destinatarios_de_una_linea_para(recortada).is_some()
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

/// Extrae la región de destinatarios acumulando caracteres válidos hasta una parada.
/// Los caracteres válidos de destinatarios son: alfanuméricos, comas, espacios,
/// y marcas de énfasis markdown: `*`, `_`, `` ` ``.
/// Se para en el primer carácter que no sea válido:
/// - Si es `:` → devuelve el cuerpo tras los dos puntos
/// - Otro → no hay cuerpo rescatado
///
/// El último token acumulado queda **truncado** —y por tanto se descarta de los
/// destinatarios— si y solo si el carácter de parada tiene un carácter alfanumérico
/// inmediatamente antes y también inmediatamente después: es puntuación interna a una
/// palabra (p. ej. el `-` de «Programator-bis»), no un separador entre destinatarios y
/// el resto de la línea.
fn extraer_region_de_destinatarios(resto_tras_para: &str) -> (&str, Option<&str>) {
    // Se guarda el carácter de parada junto con su posición: quien lo encontró ya lo tenía en la
    // mano, así que no hace falta volver a extraerlo del texto (y no queda ningún camino que
    // pudiera entrar en pánico si la posición no cayera en un límite de carácter).
    let mut parada: Option<(usize, char)> = None;

    for (byte_pos, c) in resto_tras_para.char_indices() {
        if !(c.is_alphanumeric()
            || c == ','
            || c.is_whitespace()
            || c == '*'
            || c == '_'
            || c == '`')
        {
            parada = Some((byte_pos, c));
            break;
        }
    }

    let Some((pos, caracter_parada)) = parada else {
        // Llegó al final de la línea
        return (resto_tras_para.trim(), None);
    };

    let destinatarios_crudo = resto_tras_para[..pos].trim();

    let alfanumerico_antes = resto_tras_para[..pos]
        .chars()
        .next_back()
        .is_some_and(char::is_alphanumeric);
    let alfanumerico_despues = resto_tras_para[pos + caracter_parada.len_utf8()..]
        .chars()
        .next()
        .is_some_and(char::is_alphanumeric);

    let destinatarios = if alfanumerico_antes && alfanumerico_despues {
        match destinatarios_crudo.rfind(|c: char| c == ',' || c.is_whitespace()) {
            Some(ultimo_separador) => destinatarios_crudo[..ultimo_separador].trim(),
            None => "",
        }
    } else {
        destinatarios_crudo
    };

    if caracter_parada == ':' {
        let cuerpo = resto_tras_para[pos + 1..].trim();
        return (
            destinatarios,
            if cuerpo.is_empty() {
                None
            } else {
                Some(cuerpo)
            },
        );
    }

    (destinatarios, None)
}

/// Extrae el texto que va tras los dos puntos en un encabezado de la forma `## Para X: texto`
fn extraer_texto_tras_dos_puntos_en_encabezado(encabezado: &str) -> Option<String> {
    let sin_almohadillas = encabezado.trim_start_matches('#').trim();
    let resto = sin_almohadillas.strip_prefix("para ")?;
    let (_, cuerpo) = extraer_region_de_destinatarios(resto);
    cuerpo.map(|s| s.to_string())
}

/// ¿Es `## Para X` (o `## Para X e Y`) con nuestro agente entre los destinatarios?
fn es_encabezado_para(encabezado_en_minusculas: &str, objetivo: &str) -> bool {
    let sin_almohadillas = encabezado_en_minusculas.trim_start_matches('#').trim();
    let Some(resto) = sin_almohadillas.strip_prefix("para ") else {
        return false;
    };
    let (destinatarios, _) = extraer_region_de_destinatarios(resto);
    destinatarios
        .split(|c: char| c == ',' || c.is_whitespace())
        .any(|palabra| palabra.trim_matches(|c: char| !c.is_alphanumeric()) == objetivo)
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
    fn no_confunde_mayusculas() {
        let texto = "## para programator\n\nen minúsculas.\n";

        assert_eq!(detectar("Programator", "codex.md", texto).len(), 1);
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

        assert_eq!(encargos.len(), 1, "{encargos:?}");
        assert_eq!(
            encargos[0].texto,
            "Para Programator: revisa src/main.rs.\nBusca números sueltos.\n\nY dime cuántos hay."
        );
        assert_eq!(encargos[0].de, "claude.md");
    }

    #[test]
    fn la_mencion_en_medio_de_una_frase_no_es_un_encargo() {
        let encargos = detectar(
            "Programator",
            "claude.md",
            "LATIDO: 2026-11-13 13:41:33\n\nHablé con el Director. Para Programator no hay nada.\n\
             Se lo diré luego a Programator.\n",
        );

        assert!(encargos.is_empty(), "{encargos:?}");
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
            "para programator: haz esto.",
            "**Para Programator:** haz esto.",
            "Para Programator haz esto.",
        ] {
            let encargos = detectar("Programator", "claude.md", linea);
            assert_eq!(encargos.len(), 1, "debía ser un encargo: {linea}");
        }
    }

    #[test]
    fn no_vale_otra_instancia_ni_la_copia_ni_la_cita_ni_otro_nombre() {
        for linea in [
            "Para Programator 2: haz esto.",
            "Para Gemini 1, con copia a Programator. Haz esto.",
            "> Para Programator: haz esto.",
            "Para Programatorcito: haz esto.",
            "Para Programator-bis: haz esto.",
            "Parapente con Programator.",
            "- Para Programator: haz esto.",
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
    fn el_cuerpo_de_un_encargo_en_linea_no_lleva_la_linea_para() {
        let cuerpo = |texto: &str| {
            let encargos = detectar("Programator", "claude.md", texto);
            encargos[0].cuerpo().to_string()
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
