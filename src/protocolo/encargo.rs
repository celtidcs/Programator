//! Detección de lo que va dirigido a un agente. Son reglas textuales deliberadamente estrictas:
//! mencionar a alguien no es encargarle trabajo.
//!
//! Se reconocen dos formas, y conviene saber que **no valen lo mismo**:
//!
//! 1. **El encabezado `## Para <agente>`** es la forma del protocolo del equipo. Comprobado sobre el
//!    canal real de MMCelt: los mensajes viven bajo encabezados como `## Para Claude y Gemini`, y
//!    por ahí llegará la práctica totalidad de los encargos reales.
//! 2. **El marcador `[→ <agente>]`** es una extensión propia de Programator; **el protocolo del
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
}

/// Extrae los encargos dirigidos a `agente` dentro del contenido de un buzón ajeno.
pub fn detectar(agente: &str, de: &str, contenido: &str) -> Vec<Encargo> {
    let objetivo = agente.to_lowercase();
    let marcador = format!("[→ {}]", objetivo);
    let mut encargos = Vec::new();

    let mut seccion: Option<Vec<String>> = None;

    for linea in contenido.lines() {
        let recortada = linea.trim();
        let minusculas = recortada.to_lowercase();

        if minusculas.starts_with("## ") {
            // Un encabezado nuevo cierra la sección anterior.
            if let Some(acumulado) = seccion.take() {
                let texto_completo = acumulado.join("\n").trim().to_string();
                if !texto_completo.is_empty() {
                    encargos.push(Encargo {
                        de: de.to_string(),
                        texto: texto_completo,
                    });
                }
            }
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
            });
        }
    }

    if let Some(acumulado) = seccion.take() {
        let texto_completo = acumulado.join("\n").trim().to_string();
        if !texto_completo.is_empty() {
            encargos.push(Encargo {
                de: de.to_string(),
                texto: texto_completo,
            });
        }
    }

    encargos
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
}
