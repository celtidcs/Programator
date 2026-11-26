//! La línea `Prioridad:` de un encargo: si el agente que dirige quiere que se atienda antes o
//! después que los demás.
//!
//! Es opcional y se lee como toda línea con nombre (`super::linea_con_nombre`). Sin ella, o con un
//! valor que no se entiende, el encargo es `normal`; en el segundo caso la entrega lo avisa. Se lee
//! del encargo que escribió el agente, nunca de lo que diga el modelo.

use super::linea_con_nombre::primer_valor;

/// La palabra que abre la línea, sin los dos puntos. Se compara sin distinguir mayúsculas.
const ETIQUETA: &str = "prioridad";

/// Las tres prioridades. El orden de la declaración es el de atención: primero `Alta`. En la
/// ficha del encargo se guarda con su nombre, en minúsculas; una ficha sin ella es `Normal`.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Prioridad {
    Alta,
    #[default]
    Normal,
    Baja,
}

impl Prioridad {
    /// Todas, en el orden en que se atienden.
    pub const EN_ORDEN: [Prioridad; 3] = [Prioridad::Alta, Prioridad::Normal, Prioridad::Baja];

    /// La palabra con que se escribe en el encargo.
    pub fn nombre(self) -> &'static str {
        match self {
            Prioridad::Alta => "alta",
            Prioridad::Normal => "normal",
            Prioridad::Baja => "baja",
        }
    }

    fn por_nombre(escrito: &str) -> Option<Prioridad> {
        Self::EN_ORDEN
            .into_iter()
            .find(|p| p.nombre().eq_ignore_ascii_case(escrito))
    }
}

/// Lo que pide un encargo sobre su prioridad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrioridadPedida {
    /// No trae línea `Prioridad:`.
    Ninguna,
    Valida(Prioridad),
    /// Trae la línea con un valor que no es ninguna prioridad. Se guarda tal cual para decirle al
    /// agente qué escribió.
    Desconocida(String),
}

impl PrioridadPedida {
    /// La prioridad con que se atiende: la pedida, o `Normal`.
    pub fn efectiva(&self) -> Prioridad {
        match self {
            PrioridadPedida::Valida(prioridad) => *prioridad,
            PrioridadPedida::Ninguna | PrioridadPedida::Desconocida(_) => Prioridad::Normal,
        }
    }
}

/// Lee la primera línea `Prioridad:` del texto de un encargo.
pub fn prioridad_pedida(texto: &str) -> PrioridadPedida {
    match primer_valor(texto, ETIQUETA) {
        None => PrioridadPedida::Ninguna,
        Some(valor) => match Prioridad::por_nombre(valor) {
            Some(prioridad) => PrioridadPedida::Valida(prioridad),
            None => PrioridadPedida::Desconocida(valor.to_string()),
        },
    }
}

/// El aviso que va en la entrega si la prioridad pedida no se entiende.
pub fn aviso_de_la_prioridad(texto: &str) -> Option<String> {
    match prioridad_pedida(texto) {
        PrioridadPedida::Desconocida(escrita) => Some(mensaje_prioridad_desconocida(&escrita)),
        PrioridadPedida::Ninguna | PrioridadPedida::Valida(_) => None,
    }
}

/// Dice lo que se escribió, que se atendió como `normal` y qué valores valen.
pub fn mensaje_prioridad_desconocida(escrita: &str) -> String {
    let validas: Vec<String> = Prioridad::EN_ORDEN
        .iter()
        .map(|p| format!("`{}`", p.nombre()))
        .collect();
    format!(
        "⚠️ La línea `Prioridad: {escrita}` no nombra ninguna prioridad: este encargo se ha \
         atendido como `{}`. Las que valen son {}.",
        Prioridad::Normal.nombre(),
        validas.join(", ")
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_prioridad_se_lee_por_su_nombre() {
        for prioridad in Prioridad::EN_ORDEN {
            let texto = format!("Haz esto.\n- **Prioridad:** {}\n", prioridad.nombre());
            assert_eq!(prioridad_pedida(&texto), PrioridadPedida::Valida(prioridad));
        }
        assert_eq!(
            prioridad_pedida("PRIORIDAD: Alta"),
            PrioridadPedida::Valida(Prioridad::Alta)
        );
    }

    #[test]
    fn sin_linea_es_normal_y_no_avisa() {
        assert_eq!(prioridad_pedida("Haz esto."), PrioridadPedida::Ninguna);
        assert_eq!(prioridad_pedida("Haz esto.").efectiva(), Prioridad::Normal);
        assert_eq!(aviso_de_la_prioridad("Haz esto."), None);
        assert_eq!(aviso_de_la_prioridad("Prioridad: baja"), None);
    }

    /// D2.
    #[test]
    fn una_prioridad_desconocida_avisa_y_cuenta_como_normal() {
        let pedida = prioridad_pedida("Prioridad: urgente");
        assert_eq!(pedida, PrioridadPedida::Desconocida("urgente".to_string()));
        assert_eq!(pedida.efectiva(), Prioridad::Normal);
        let aviso = aviso_de_la_prioridad("Prioridad: urgente").unwrap();
        assert!(aviso.contains("`Prioridad: urgente`"), "{aviso}");
        assert!(aviso.contains("`alta`, `normal`, `baja`"), "{aviso}");
    }

    #[test]
    fn el_orden_de_atencion_es_alta_normal_baja() {
        assert!(Prioridad::Alta < Prioridad::Normal);
        assert!(Prioridad::Normal < Prioridad::Baja);
    }
}
