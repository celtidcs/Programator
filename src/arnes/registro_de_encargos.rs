//! El registro de encargos: una ficha por encargo, reconocido por su identificador
//! (`protocolo::identificador`), con el estado en que está.
//!
//! **Por qué existe.** Hasta la 1.1.0 un encargo no tenía identidad propia: se detectaba en el
//! trozo nuevo de cada buzón, por desplazamiento en bytes, y la cola de pendientes lo reconocía
//! por su buzón y su texto. Editar lo anterior del buzón o cambiar sus saltos de línea hacía
//! reatender encargos ya atendidos, y lo añadido dentro de una sección ya leída se perdía
//! (limitación 1 de `docs/defectos-conocidos.md`). Con el registro, es nuevo lo que no tiene ficha.
//!
//! Aquí está lo que decide, sobre [`FichaDeEncargo`]; la lectura y el guardado van en [`disco`].

pub mod disco;
pub mod migracion;

use crate::arnes::prioridad::Prioridad;
use crate::protocolo::encargo::{Encargo, Forma};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// En qué está un encargo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoDelEncargo {
    /// Llegó y todavía no tiene respuesta publicada.
    Pendiente,
    /// Se está atendiendo ahora.
    EnCurso,
    /// Su respuesta está publicada.
    Atendido,
    /// Quien lo encargó lo canceló.
    Cancelado,
    /// Agotó los intentos que se admiten (`[ciclo] intentos_por_encargo`) y se cerró con aviso.
    Cerrado,
    /// Ya estaba en el canal al tomar la instantánea inicial: se da por leído sin atenderlo.
    Visto,
}

impl EstadoDelEncargo {
    /// Si al encargo todavía le queda algo por hacer.
    pub fn esta_abierto(self) -> bool {
        matches!(self, Self::Pendiente | Self::EnCurso)
    }
}

/// Lo que se sabe de un encargo: con qué se reconoce, lo que pide y en qué está.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FichaDeEncargo {
    /// El identificador del encargo; también da nombre a su fichero.
    pub id: String,
    /// Fichero del buzón de origen.
    pub de: String,
    /// El latido de la intervención que lo contiene, si la hay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latido: Option<String>,
    /// Cómo venía escrito.
    pub forma: Forma,
    /// Texto íntegro del encargo.
    pub texto: String,
    /// Cuándo se detectó.
    pub llego_el: String,
    /// En qué está.
    pub estado: EstadoDelEncargo,
    /// Intentos fallidos hasta ahora.
    #[serde(default)]
    pub intentos: u32,
    /// Por qué falló el último intento.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub motivo: String,
    /// Cuándo dejó de estar abierto (atendido, cancelado, cerrado), si ya lo dejó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atendido_el: Option<String>,
    /// Su puesto en el orden de llegada: los encargos de una misma pasada llegan a la vez y hay que
    /// atenderlos en el orden en que estaban en el canal.
    #[serde(default)]
    pub orden: u64,
    /// Desde cuándo no está en su buzón, si ya cerrada dejó de verse. Se olvida cuando sigue sin
    /// verse pasado el plazo; si vuelve a verse, se borra la marca.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub falta_desde: Option<String>,
    /// Con qué prioridad se atiende, sacada de su línea `Prioridad:`: queda a la vista en la ficha
    /// sin releer el texto. Las fichas que no la traen son `normal`.
    #[serde(default)]
    pub prioridad: Prioridad,
}

/// El formato de [`FichaDeEncargo::falta_desde`]: el del latido, con segundos.
const FORMATO_DE_FALTA_DESDE: &str = crate::protocolo::canal::FORMATO_DE_LA_HORA_DEL_LATIDO;

impl FichaDeEncargo {
    /// La ficha de un encargo recién detectado, en el estado que se le da al detectarlo.
    pub fn nueva(encargo: &Encargo, llego_el: &str, estado: EstadoDelEncargo) -> Self {
        Self {
            id: encargo.id(),
            de: encargo.de.clone(),
            latido: encargo.latido.clone(),
            forma: encargo.forma,
            texto: encargo.texto.clone(),
            llego_el: llego_el.to_string(),
            estado,
            intentos: 0,
            motivo: String::new(),
            atendido_el: None,
            orden: 0,
            falta_desde: None,
            prioridad: crate::arnes::prioridad::prioridad_pedida(&encargo.texto).efectiva(),
        }
    }

    /// El encargo, tal como llegó.
    pub fn encargo(&self) -> Encargo {
        Encargo {
            de: self.de.clone(),
            texto: self.texto.clone(),
            forma: self.forma,
            latido: self.latido.clone(),
        }
    }

    /// Anota un intento fallido y su motivo; el encargo vuelve a `Pendiente`. Devuelve si ya llegó
    /// a `intentos_admitidos`: toca cerrarlo con aviso.
    pub fn anotar_fallo(&mut self, motivo: &str, intentos_admitidos: u32) -> bool {
        self.intentos = self.intentos.saturating_add(1);
        self.motivo = motivo.to_string();
        self.estado = EstadoDelEncargo::Pendiente;
        self.intentos >= intentos_admitidos
    }

    /// Deja el encargo en un estado final (atendido, cancelado o cerrado), con su fecha.
    pub fn terminar(&mut self, estado: EstadoDelEncargo, fecha: &str) {
        self.estado = estado;
        self.atendido_el = Some(fecha.to_string());
    }
}

/// El registro entero en memoria, en orden de llegada, con lo que cambió desde que se leyó: es lo
/// que hay que guardar.
///
/// Los campos son privados: solo cambia por sus métodos, que mantienen el orden de llegada y una
/// sola ficha por encargo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RegistroDeEncargos {
    fichas: Vec<FichaDeEncargo>,
    cambiadas: BTreeSet<String>,
}

/// Desde cuándo falta la ficha. Una marca que no se entiende cuenta como ninguna: se vuelve a
/// anotar y el plazo empieza otra vez, que es lo prudente.
fn desde_cuando(ficha: &FichaDeEncargo) -> Option<NaiveDateTime> {
    ficha
        .falta_desde
        .as_deref()
        .and_then(|texto| NaiveDateTime::parse_from_str(texto, FORMATO_DE_FALTA_DESDE).ok())
}

impl RegistroDeEncargos {
    /// El registro con las fichas leídas, puestas en orden de llegada.
    pub fn de_fichas(mut fichas: Vec<FichaDeEncargo>) -> Self {
        fichas.sort_by(|a, b| {
            a.orden
                .cmp(&b.orden)
                .then_with(|| a.llego_el.cmp(&b.llego_el))
                .then_with(|| a.id.cmp(&b.id))
        });
        Self {
            fichas,
            cambiadas: BTreeSet::new(),
        }
    }

    /// Todas las fichas, en orden de llegada.
    pub fn fichas(&self) -> &[FichaDeEncargo] {
        &self.fichas
    }

    /// Si el encargo ya tiene ficha, esté como esté.
    pub fn tiene(&self, encargo: &Encargo) -> bool {
        self.posicion(&encargo.id()).is_some()
    }

    /// Anota un encargo que no tenía ficha, al final, en `estado`. Devuelve si era nuevo; si ya
    /// tenía ficha, no la toca.
    pub fn anotar(&mut self, encargo: &Encargo, fecha: &str, estado: EstadoDelEncargo) -> bool {
        if self.tiene(encargo) {
            return false;
        }
        let mut ficha = FichaDeEncargo::nueva(encargo, fecha, estado);
        ficha.orden = self.siguiente_orden();
        self.cambiadas.insert(ficha.id.clone());
        self.fichas.push(ficha);
        true
    }

    /// Añade una ficha ya hecha (la de una migración), al final y con el orden siguiente, si su
    /// identificador no estaba.
    pub fn anotar_ficha(&mut self, mut ficha: FichaDeEncargo) -> bool {
        if self.posicion(&ficha.id).is_some() {
            return false;
        }
        ficha.orden = self.siguiente_orden();
        self.cambiadas.insert(ficha.id.clone());
        self.fichas.push(ficha);
        true
    }

    /// Si `encargo` es uno ya atendido que se vuelve a atender porque cambió su texto: hay otra
    /// ficha atendida del mismo buzón, de la misma intervención y con la misma primera línea. Otro
    /// encargo de esa intervención empieza por otra línea `Para …`, y no lo es.
    pub fn es_reatencion(&self, encargo: &Encargo) -> bool {
        let id = encargo.id();
        let primera = encargo.texto.lines().next();
        self.fichas.iter().any(|ficha| {
            ficha.id != id
                && ficha.estado == EstadoDelEncargo::Atendido
                && ficha.de == encargo.de
                && ficha.latido == encargo.latido
                && ficha.texto.lines().next() == primera
        })
    }

    /// Los encargos que todavía no tienen respuesta, en orden de llegada.
    pub fn pendientes(&self) -> Vec<Encargo> {
        self.fichas
            .iter()
            .filter(|ficha| ficha.estado.esta_abierto())
            .map(FichaDeEncargo::encargo)
            .collect()
    }

    /// Si el encargo tiene ficha y sigue sin respuesta.
    pub fn esta_abierto(&self, encargo: &Encargo) -> bool {
        self.posicion(&encargo.id())
            .is_some_and(|posicion| self.fichas[posicion].estado.esta_abierto())
    }

    /// Si queda algún encargo sin respuesta.
    pub fn hay_pendientes(&self) -> bool {
        self.fichas.iter().any(|ficha| ficha.estado.esta_abierto())
    }

    /// Anota un intento fallido del encargo, que queda pendiente; si no tenía ficha, entra al
    /// final con fecha `fecha`. Devuelve la ficha si ya llegó a `intentos_admitidos`: toca cerrarlo.
    /// Si el cierre no se pudo publicar, la vuelta siguiente la vuelve a devolver.
    pub fn anotar_fallo(
        &mut self,
        encargo: &Encargo,
        fecha: &str,
        motivo: &str,
        intentos_admitidos: u32,
    ) -> Option<&FichaDeEncargo> {
        self.anotar(encargo, fecha, EstadoDelEncargo::Pendiente);
        let posicion = self.posicion(&encargo.id())?;
        self.cambiadas.insert(self.fichas[posicion].id.clone());
        let toca_cerrar = self.fichas[posicion].anotar_fallo(motivo, intentos_admitidos);
        toca_cerrar.then(|| &self.fichas[posicion])
    }

    /// Deja el encargo en un estado final con su fecha. Devuelve si tenía ficha.
    pub fn terminar(&mut self, encargo: &Encargo, estado: EstadoDelEncargo, fecha: &str) -> bool {
        let Some(posicion) = self.posicion(&encargo.id()) else {
            return false;
        };
        self.fichas[posicion].terminar(estado, fecha);
        self.cambiadas.insert(self.fichas[posicion].id.clone());
        true
    }

    /// Las fichas que cambiaron desde que se leyó el registro o se guardó por última vez.
    pub fn cambiadas(&self) -> Vec<&FichaDeEncargo> {
        self.fichas
            .iter()
            .filter(|ficha| self.cambiadas.contains(&ficha.id))
            .collect()
    }

    /// Da por guardada la ficha `id`: ya no cuenta entre las cambiadas.
    pub fn dar_por_guardada(&mut self, id: &str) {
        self.cambiadas.remove(id);
    }

    /// Olvida las fichas cerradas (ni pendientes ni en curso) cuyo encargo ya no está en su buzón:
    /// el agente lo pasó a su histórico. `presentes` dice, de cada buzón mirado, qué
    /// identificadores tiene ahora; un buzón que no está en `existentes` ya no existe. Un buzón que
    /// existe pero no se miró no cambió desde la última lectura: lo suyo sigue como estaba.
    ///
    /// No basta con no verla una vez: un buzón leído mientras su dueño lo reescribe puede salir
    /// vacío o cortado, y olvidar la ficha haría atender otra vez lo ya atendido cuando se lea
    /// entero. La primera vez que falta se anota desde
    /// cuándo; se olvida si sigue faltando pasado `plazo`, y si vuelve a verse se borra la marca.
    /// Devuelve los identificadores olvidados, para borrar sus ficheros.
    pub fn podar(
        &mut self,
        presentes: &BTreeMap<String, BTreeSet<String>>,
        existentes: &BTreeSet<String>,
        ahora: NaiveDateTime,
        plazo: chrono::Duration,
    ) -> Vec<String> {
        // `None`: no se miró, así que no hay nada nuevo que saber de ella.
        let falta = |ficha: &FichaDeEncargo| match presentes.get(&ficha.de) {
            Some(ids) => Some(!ids.contains(&ficha.id)),
            None if !existentes.contains(&ficha.de) => Some(true),
            None => None,
        };
        let mut quitados = Vec::new();
        for ficha in self.fichas.iter_mut().filter(|f| !f.estado.esta_abierto()) {
            match (falta(ficha), desde_cuando(ficha)) {
                (Some(false), Some(_)) => {
                    ficha.falta_desde = None;
                    self.cambiadas.insert(ficha.id.clone());
                }
                (Some(true) | None, Some(desde)) if ahora.signed_duration_since(desde) >= plazo => {
                    quitados.push(ficha.id.clone());
                }
                (Some(false) | None, None) | (Some(true) | None, Some(_)) => {}
                (Some(true), None) => {
                    ficha.falta_desde = Some(ahora.format(FORMATO_DE_FALTA_DESDE).to_string());
                    self.cambiadas.insert(ficha.id.clone());
                }
            }
        }
        self.fichas.retain(|ficha| !quitados.contains(&ficha.id));
        for id in &quitados {
            self.cambiadas.remove(id);
        }
        quitados
    }

    fn posicion(&self, id: &str) -> Option<usize> {
        self.fichas.iter().position(|ficha| ficha.id == id)
    }

    fn siguiente_orden(&self) -> u64 {
        self.fichas
            .iter()
            .map(|ficha| ficha.orden.saturating_add(1))
            .max()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn otro(texto: &str) -> Encargo {
        Encargo {
            texto: texto.into(),
            ..encargo()
        }
    }

    #[test]
    fn el_registro_anota_cada_encargo_una_vez_y_en_orden_de_llegada() {
        let mut registro = RegistroDeEncargos::default();
        assert!(registro.anotar(&otro("a"), "f", EstadoDelEncargo::Pendiente));
        assert!(registro.anotar(&otro("b"), "f", EstadoDelEncargo::Pendiente));
        assert!(!registro.anotar(&otro("a"), "g", EstadoDelEncargo::Visto));

        let textos: Vec<_> = registro.pendientes().into_iter().map(|e| e.texto).collect();
        assert_eq!(textos, ["a", "b"]);
        assert_eq!(registro.cambiadas().len(), 2);
    }

    /// Leídas del disco llegan sin orden: el registro las pone en el de llegada.
    #[test]
    fn las_fichas_leidas_se_ordenan_por_su_puesto() {
        let mut registro = RegistroDeEncargos::default();
        for texto in ["a", "b", "c"] {
            registro.anotar(&otro(texto), "f", EstadoDelEncargo::Pendiente);
        }
        let mut fichas = registro.fichas().to_vec();
        fichas.reverse();

        let leido = RegistroDeEncargos::de_fichas(fichas);

        let textos: Vec<_> = leido.pendientes().into_iter().map(|e| e.texto).collect();
        assert_eq!(textos, ["a", "b", "c"]);
        assert!(leido.cambiadas().is_empty());
    }

    #[test]
    fn solo_los_abiertos_quedan_pendientes() {
        let mut registro = RegistroDeEncargos::default();
        registro.anotar(&otro("visto"), "f", EstadoDelEncargo::Visto);
        registro.anotar(&otro("hecho"), "f", EstadoDelEncargo::Pendiente);
        assert!(registro.terminar(&otro("hecho"), EstadoDelEncargo::Atendido, "g"));
        assert!(!registro.hay_pendientes());
        assert!(!registro.terminar(&otro("nunca"), EstadoDelEncargo::Atendido, "g"));
    }

    #[test]
    fn un_fallo_sin_ficha_la_crea_y_al_tope_la_devuelve() {
        let mut registro = RegistroDeEncargos::default();
        assert!(registro.anotar_fallo(&otro("a"), "f", "uno", 2).is_none());
        let cerrada = registro.anotar_fallo(&otro("a"), "f", "dos", 2).unwrap();
        assert_eq!((cerrada.intentos, cerrada.motivo.as_str()), (2, "dos"));
        assert_eq!(registro.fichas().len(), 1);
    }

    /// B7: lo que el agente poda de su buzón sale del registro; lo abierto y lo de buzones que no
    /// se miraron se queda, y lo de un buzón que desapareció también sale.
    #[test]
    fn la_poda_quita_lo_cerrado_que_ya_no_esta_en_su_buzon() {
        let en = |de: &str, texto: &str| Encargo {
            de: de.into(),
            ..otro(texto)
        };
        let mut registro = RegistroDeEncargos::default();
        registro.anotar(&en("a.md", "sigue"), "f", EstadoDelEncargo::Atendido);
        registro.anotar(&en("a.md", "podado"), "f", EstadoDelEncargo::Atendido);
        registro.anotar(&en("a.md", "debido"), "f", EstadoDelEncargo::Pendiente);
        registro.anotar(&en("b.md", "sin mirar"), "f", EstadoDelEncargo::Visto);
        registro.anotar(&en("c.md", "borrado"), "f", EstadoDelEncargo::Cerrado);
        let presentes = BTreeMap::from([(
            "a.md".to_string(),
            BTreeSet::from([en("a.md", "sigue").id()]),
        )]);
        let existentes = BTreeSet::from(["a.md".to_string(), "b.md".to_string()]);

        let hora =
            |texto: &str| NaiveDateTime::parse_from_str(texto, FORMATO_DE_FALTA_DESDE).unwrap();
        let plazo = chrono::Duration::minutes(60);

        // La primera vez que faltan solo se anota desde cuándo.
        assert!(registro
            .podar(&presentes, &existentes, hora("2026-11-25 10:00:00"), plazo)
            .is_empty());
        assert_eq!(registro.fichas().len(), 5);
        // Dentro del plazo, siguen.
        assert!(registro
            .podar(&presentes, &existentes, hora("2026-11-25 10:59:00"), plazo)
            .is_empty());

        let quitados = registro.podar(&presentes, &existentes, hora("2026-11-25 11:00:00"), plazo);

        assert_eq!(
            quitados,
            [en("a.md", "podado").id(), en("c.md", "borrado").id()]
        );
        let quedan: Vec<_> = registro.fichas().iter().map(|f| f.texto.as_str()).collect();
        assert_eq!(quedan, ["sigue", "debido", "sin mirar"]);
    }

    /// Lo que falta una vez y vuelve a verse pierde la marca: el plazo empieza otra vez.
    #[test]
    fn lo_que_vuelve_a_verse_no_se_olvida() {
        let mut registro = RegistroDeEncargos::default();
        registro.anotar(&otro("a"), "f", EstadoDelEncargo::Atendido);
        let id = otro("a").id();
        let existentes = BTreeSet::from([otro("a").de.clone()]);
        let vacio = BTreeMap::from([(otro("a").de.clone(), BTreeSet::new())]);
        let con_el = BTreeMap::from([(otro("a").de.clone(), BTreeSet::from([id.clone()]))]);
        let hora =
            |texto: &str| NaiveDateTime::parse_from_str(texto, FORMATO_DE_FALTA_DESDE).unwrap();
        let plazo = chrono::Duration::minutes(60);

        registro.podar(&vacio, &existentes, hora("2026-11-25 10:00:00"), plazo);
        assert!(registro.fichas()[0].falta_desde.is_some());
        registro.podar(&con_el, &existentes, hora("2026-11-25 10:30:00"), plazo);
        assert_eq!(registro.fichas()[0].falta_desde, None);
        assert!(registro.cambiadas().iter().any(|f| f.id == id));

        assert!(registro
            .podar(&vacio, &existentes, hora("2026-11-25 11:10:00"), plazo)
            .is_empty());

        // Marcada, y el buzón no vuelve a cambiar: sigue faltando, y pasado el plazo se olvida
        // aunque no se relea.
        let sin_mirar = BTreeMap::new();
        assert!(registro
            .podar(&sin_mirar, &existentes, hora("2026-11-25 12:00:00"), plazo)
            .is_empty());
        assert_eq!(
            registro.podar(&sin_mirar, &existentes, hora("2026-11-25 12:10:00"), plazo),
            [id]
        );
    }

    #[test]
    fn lo_guardado_deja_de_contar_como_cambiado() {
        let mut registro = RegistroDeEncargos::default();
        registro.anotar(&otro("a"), "f", EstadoDelEncargo::Pendiente);
        let id = registro.cambiadas()[0].id.clone();
        registro.dar_por_guardada(&id);
        assert!(registro.cambiadas().is_empty());
        registro.terminar(&otro("a"), EstadoDelEncargo::Atendido, "g");
        assert_eq!(registro.cambiadas().len(), 1);
    }

    fn encargo() -> Encargo {
        Encargo {
            de: "claude.md".into(),
            texto: "Para Programator: haz esto".into(),
            forma: Forma::Linea,
            latido: Some("2026-11-25 10:00:00".into()),
        }
    }

    #[test]
    fn la_ficha_nueva_lleva_el_identificador_del_encargo_y_lo_devuelve_igual() {
        let ficha = FichaDeEncargo::nueva(&encargo(), "f", EstadoDelEncargo::Pendiente);
        assert_eq!(ficha.id, encargo().id());
        let devuelto = ficha.encargo();
        assert_eq!(devuelto.id(), encargo().id());
        assert_eq!(devuelto.forma, Forma::Linea);
    }

    #[test]
    fn solo_pendiente_y_en_curso_estan_abiertos() {
        use EstadoDelEncargo::*;
        for estado in [Pendiente, EnCurso] {
            assert!(estado.esta_abierto(), "{estado:?}");
        }
        for estado in [Atendido, Cancelado, Cerrado, Visto] {
            assert!(!estado.esta_abierto(), "{estado:?}");
        }
    }

    #[test]
    fn un_fallo_suma_un_intento_y_avisa_al_llegar_al_tope() {
        let mut ficha = FichaDeEncargo::nueva(&encargo(), "f", EstadoDelEncargo::EnCurso);
        assert!(!ficha.anotar_fallo("uno", 2));
        assert_eq!(ficha.estado, EstadoDelEncargo::Pendiente);
        assert!(ficha.anotar_fallo("dos", 2));
        assert_eq!(ficha.intentos, 2);
        assert_eq!(ficha.motivo, "dos");
    }

    /// Uso real del 26/11: cambiar una palabra del cuerpo de un encargo ya atendido lo vuelve a
    /// atender, y su respuesta tiene que decir que es la segunda vez. Lo es si ya se atendió uno
    /// del mismo buzón, la misma intervención y la misma primera línea; otro encargo de la misma
    /// intervención no lo es.
    #[test]
    fn un_encargo_retocado_tras_atenderlo_se_reconoce_como_reatencion() {
        let mut registro = RegistroDeEncargos::default();
        let original = otro("Para Programator: FECHAS-1 módulo\nescribe una línea por requisito");
        registro.anotar(&original, "f", EstadoDelEncargo::Pendiente);
        let retocado = otro("Para Programator: FECHAS-1 módulo\nescribe una frase por requisito");
        let vecino = otro("Para Programator: P2 función es_par");
        assert!(!registro.es_reatencion(&retocado), "aún sin atender");

        registro.terminar(&original, EstadoDelEncargo::Atendido, "g");
        registro.anotar(&retocado, "h", EstadoDelEncargo::Pendiente);
        registro.anotar(&vecino, "h", EstadoDelEncargo::Pendiente);

        assert!(registro.es_reatencion(&retocado));
        assert!(!registro.es_reatencion(&vecino));
        assert!(!registro.es_reatencion(&original));
        let de_otro_buzon = Encargo {
            de: "codex.md".into(),
            ..retocado.clone()
        };
        assert!(!registro.es_reatencion(&de_otro_buzon));
    }

    #[test]
    fn terminar_deja_el_estado_y_la_fecha() {
        let mut ficha = FichaDeEncargo::nueva(&encargo(), "f", EstadoDelEncargo::EnCurso);
        ficha.terminar(EstadoDelEncargo::Atendido, "2026-11-25 12:00:00");
        assert_eq!(ficha.estado, EstadoDelEncargo::Atendido);
        assert_eq!(ficha.atendido_el.as_deref(), Some("2026-11-25 12:00:00"));
    }
}
