//! Quién es destinatario de una línea del canal, con las mismas reglas que Gestor Agentes.
//!
//! Es una copia fiel de `crates/canal/src/encargos.rs` de Gestor Agentes (3.6.0 y 3.7.0), que es
//! quien despierta a los agentes: si Programator leyera los destinatarios de otra forma, un mismo
//! texto sería encargo para uno y no para el otro. Si Gestor cambia una regla, se cambia aquí a
//! la vez.
//!
//! Reglas:
//! - «Para» va con mayúscula y no forma parte de otra palabra. Puede ir a mitad de línea, detrás de
//!   la fecha, de un título de markdown o de la marca de cita `>`.
//! - Los destinatarios van detrás, separados por comas, «y» o «e». La lista acaba en el primer
//!   «:», «;», «.» o «—». Lo que va entre paréntesis no cuenta.
//! - Un destinatario vale con su número («Programator 1») y, si es la instancia 1, también sin él
//!   («Programator»). Basta con que la primera palabra sea la herramienta: «Programator haz…» es la
//!   instancia 1.

/// Palabra que abre un encargo.
pub(crate) const PALABRA_DEL_ENCARGO: &str = "Para ";
/// Caracteres que cierran la lista de destinatarios.
pub(crate) const FIN_DE_LISTA: [char; 4] = [':', ';', '.', '—'];
/// Separadores de destinatarios dentro de la lista, además de la coma.
const CONJUNCIONES: [&str; 2] = [" y ", " e "];
/// La instancia que se entiende cuando el nombre no lleva número.
const INSTANCIA_SIN_NUMERO: u32 = 1;

/// Un agente tal como lo nombra el canal: la herramienta y su número de instancia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identidad {
    herramienta: String,
    instancia: u32,
}

impl Identidad {
    /// «Programator» o «Programator 2». Sin número, o con uno que no se lee, la instancia 1.
    pub fn de(agente: &str) -> Self {
        let mut palabras = agente.split_whitespace();
        let herramienta = palabras.next().unwrap_or_default().to_string();
        let instancia = palabras
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(INSTANCIA_SIN_NUMERO);
        Self {
            herramienta,
            instancia,
        }
    }

    /// Lee una identidad con la regla estricta de Gestor Agentes (`Identidad::leer` de
    /// `crates/canal/src/identidad.rs`): «Herramienta» o «Herramienta N», con la herramienta en
    /// letras y cifras ASCII empezando por letra y N de 1 en adelante. `None` si no la cumple.
    pub fn leer(texto: &str) -> Option<Self> {
        let partes: Vec<&str> = texto.split_whitespace().collect();
        let (herramienta, instancia) = match partes.as_slice() {
            [herramienta] => (*herramienta, INSTANCIA_SIN_NUMERO),
            [herramienta, numero] => (
                *herramienta,
                numero
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n >= INSTANCIA_SIN_NUMERO)?,
            ),
            _ => return None,
        };
        let valida = herramienta
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
            && herramienta.chars().all(|c| c.is_ascii_alphanumeric());
        valida.then(|| Self {
            herramienta: herramienta.to_string(),
            instancia,
        })
    }

    /// Si la línea encarga algo a este agente, en cualquiera de sus «Para».
    pub fn es_destinatario_de(&self, linea: &str) -> bool {
        destinatarios(linea)
            .iter()
            .any(|destinatario| self.nombrado_en(destinatario))
    }

    /// Si un destinatario, como se escribió, nombra a este agente.
    fn nombrado_en(&self, destinatario: &str) -> bool {
        let mut palabras = destinatario.split_whitespace();
        let Some(herramienta) = palabras.next() else {
            return false;
        };
        if !herramienta.eq_ignore_ascii_case(&self.herramienta) {
            return false;
        }
        match palabras.next().map(str::parse::<u32>) {
            Some(Ok(numero)) => numero == self.instancia,
            _ => self.instancia == INSTANCIA_SIN_NUMERO,
        }
    }
}

impl std::fmt::Display for Identidad {
    /// Siempre con su número, como «Claude 1»: así la escribe Gestor Agentes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.herramienta, self.instancia)
    }
}

/// Dónde empieza cada «Para » de un texto y si va suelto, es decir, sin letra ni cifra pegada
/// delante: «Para» en «SepáraPara » no lo va. La única búsqueda de la palabra del encargo, para
/// no repetir el mismo bucle en cada sitio que la necesita.
pub(crate) fn apariciones_del_para(texto: &str) -> impl Iterator<Item = (usize, bool)> + '_ {
    texto
        .match_indices(PALABRA_DEL_ENCARGO)
        .map(move |(posicion, _)| {
            let suelto = texto[..posicion]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric());
            (posicion, suelto)
        })
}

/// Lo que sigue a cada «Para» suelto de la línea, en orden.
fn tras_cada_para(linea: &str) -> impl Iterator<Item = &str> {
    apariciones_del_para(linea)
        .filter(|(_, suelto)| *suelto)
        .map(move |(posicion, _)| &linea[posicion + PALABRA_DEL_ENCARGO.len()..])
}

/// Destinatarios de todos los «Para» de una línea, como se escribieron y en orden.
pub fn destinatarios(linea: &str) -> Vec<String> {
    tras_cada_para(linea).flat_map(lista_de).collect()
}

/// Lo que va tras los dos puntos del primer «Para» de la línea que nombra a `identidad`: lo que se
/// pide en esa misma línea. `None` si la lista no acaba en dos puntos o no la nombra.
pub fn tras_los_dos_puntos<'a>(linea: &'a str, identidad: &Identidad) -> Option<&'a str> {
    tras_cada_para(linea)
        .find(|tras| {
            lista_de(tras)
                .iter()
                .any(|destinatario| identidad.nombrado_en(destinatario))
        })
        .and_then(tras_la_lista)
}

/// Lo que va tras los dos puntos del primer «Para» de la línea, sea para quien sea. `None` si no
/// hay «Para» o su lista no acaba en dos puntos.
pub fn tras_los_dos_puntos_del_primer_para(linea: &str) -> Option<&str> {
    tras_cada_para(linea).next().and_then(tras_la_lista)
}

/// Lo que sigue a la lista de destinatarios si acaba en dos puntos. Como en [`lista_de`], lo que va
/// entre paréntesis no cuenta: «Programator (ver: abajo)» no acaba en esos dos puntos.
fn tras_la_lista(texto: &str) -> Option<&str> {
    let mut profundidad = 0_usize;
    for (posicion, c) in texto.char_indices() {
        match c {
            '(' => profundidad += 1,
            ')' => profundidad = profundidad.saturating_sub(1),
            _ if profundidad == 0 && FIN_DE_LISTA.contains(&c) => {
                return texto[posicion..].strip_prefix(':');
            }
            _ => {}
        }
    }
    None
}

/// Destinatarios del texto que sigue a «Para ».
fn lista_de(texto: &str) -> Vec<String> {
    let sin_parentesis = quitar_parentesis(texto);
    let lista = sin_parentesis
        .split(|c| FIN_DE_LISTA.contains(&c))
        .next()
        .unwrap_or_default();
    lista
        .split(',')
        .flat_map(partir_por_conjunciones)
        .map(|destinatario| destinatario.trim().trim_matches('*').trim().to_owned())
        .filter(|destinatario| !destinatario.is_empty())
        .collect()
}

/// Parte un trozo de la lista por las conjunciones.
fn partir_por_conjunciones(trozo: &str) -> Vec<&str> {
    let mut partes = vec![trozo];
    for conjuncion in CONJUNCIONES {
        partes = partes
            .into_iter()
            .flat_map(|parte| parte.split(conjuncion))
            .collect();
    }
    partes
}

/// El texto sin lo que va entre paréntesis.
fn quitar_parentesis(texto: &str) -> String {
    let mut profundidad = 0_usize;
    texto
        .chars()
        .filter(|c| match c {
            '(' => {
                profundidad += 1;
                false
            }
            ')' => {
                profundidad = profundidad.saturating_sub(1);
                false
            }
            _ => profundidad == 0,
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    //! Los casos de `crates/canal/tests/encargos.rs` de Gestor Agentes, con Programator como
    //! destinatario, más los que solo importan aquí.
    use super::*;

    #[test]
    fn cada_para_se_encuentra_con_su_posicion_y_si_va_suelto() {
        let texto = "Para A: x. SepáraPara B (Para C)";
        let apariciones: Vec<(usize, bool)> = apariciones_del_para(texto).collect();
        let pegada = texto.find("Para B").unwrap();
        let entre_parentesis = texto.find("Para C").unwrap();

        assert_eq!(
            apariciones,
            vec![(0, true), (pegada, false), (entre_parentesis, true)]
        );
        assert_eq!(apariciones_del_para("para nadie").count(), 0);
    }

    fn programator() -> Identidad {
        Identidad::de("Programator")
    }

    #[test]
    fn un_destinatario_con_numero() {
        assert!(programator().es_destinatario_de("Para Programator 1: haz algo"));
        assert!(Identidad::de("Programator 2").es_destinatario_de("Para Programator 2: haz algo"));
    }

    #[test]
    fn leer_sigue_la_regla_estricta_de_gestor_y_se_escribe_con_numero() {
        let leida = |texto: &str| Identidad::leer(texto).map(|id| id.to_string());
        assert_eq!(leida("Claude 2").as_deref(), Some("Claude 2"));
        assert_eq!(leida("Codex").as_deref(), Some("Codex 1"));
        for mala in [
            "",
            "Claude 0",
            "Claude dos",
            "Claude 2 bis",
            "2Claude",
            "Clau-de",
        ] {
            assert_eq!(leida(mala), None, "{mala}");
        }
    }

    #[test]
    fn la_instancia_1_vale_sin_numero() {
        assert!(programator().es_destinatario_de("Para Programator: haz algo"));
    }

    #[test]
    fn sin_numero_no_vale_para_otra_instancia() {
        assert!(!Identidad::de("Programator 2").es_destinatario_de("Para Programator: haz algo"));
    }

    #[test]
    fn otro_numero_no_vale() {
        assert!(!programator().es_destinatario_de("Para Programator 2: haz algo"));
    }

    #[test]
    fn varios_destinatarios_despiertan_a_todos() {
        let linea = "Para Claude 1, Codex e Programator y Gemini 1: revisad";
        assert!(programator().es_destinatario_de(linea));
        assert!(Identidad::de("Codex").es_destinatario_de(linea));
        assert!(Identidad::de("Gemini 1").es_destinatario_de(linea));
    }

    #[test]
    fn dos_para_en_la_misma_linea_nombran_a_los_dos() {
        let linea = "Para Claude 1: hecho. Para Programator: sigue";
        assert!(Identidad::de("Claude 1").es_destinatario_de(linea));
        assert!(programator().es_destinatario_de(linea));
    }

    #[test]
    fn lo_que_va_tras_los_dos_puntos_no_es_destinatario() {
        assert!(!programator().es_destinatario_de("Para Claude 1: díselo a Programator"));
    }

    #[test]
    fn los_parentesis_no_cortan_la_lista() {
        assert!(programator().es_destinatario_de("Para Claude 1 (urgente) y Programator: x"));
        assert!(!programator().es_destinatario_de("Para Claude 1 (y Programator): x"));
        assert!(programator().es_destinatario_de("Para Claude 1 (15:48) y Programator: x"));
        assert!(!programator().es_destinatario_de("Para Claude 1 (y Programator, en copia): x"));
    }

    #[test]
    fn las_negritas_no_estorban() {
        assert!(programator().es_destinatario_de("**Para Programator:** haz algo"));
    }

    #[test]
    fn para_en_minuscula_o_dentro_de_otra_palabra_no_es_encargo() {
        assert!(!programator().es_destinatario_de("para Programator: haz algo"));
        assert!(!programator().es_destinatario_de("ParaProgramator: haz algo"));
        assert!(!programator().es_destinatario_de("SepáraPara Programator: haz algo"));
    }

    #[test]
    fn la_copia_no_es_un_destinatario() {
        assert!(!programator().es_destinatario_de("Para Gemini 1, con copia a Programator."));
    }

    #[test]
    fn para_a_mitad_de_linea_tras_la_fecha_el_titulo_o_la_cita() {
        assert!(programator().es_destinatario_de("2026-11-22 13:00 — Para Programator: x"));
        assert!(programator().es_destinatario_de("## Para Programator: x"));
        assert!(programator().es_destinatario_de("> Para Programator: x"));
    }

    #[test]
    fn lo_pedido_en_la_linea_va_tras_los_dos_puntos_del_para_que_le_nombra() {
        let id = programator();
        assert_eq!(
            tras_los_dos_puntos("13:00 — Para Programator: PG-A contar", &id),
            Some(" PG-A contar")
        );
        assert_eq!(
            tras_los_dos_puntos("Para Gemini 1: x. Para Programator: y", &id),
            Some(" y")
        );
        assert_eq!(tras_los_dos_puntos("Para Programator. Haz x", &id), None);
        assert_eq!(
            tras_los_dos_puntos("Para Programator (ver: abajo): x", &id),
            Some(" x")
        );
        assert_eq!(
            tras_los_dos_puntos_del_primer_para("13:00 — Para Gemini 1: x"),
            Some(" x")
        );
        assert_eq!(tras_los_dos_puntos("Para Gemini 1: x", &id), None);
    }
}
