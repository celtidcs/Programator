//! La ficha de cada verbo del repertorio: la única declaración de qué existe y qué forma tiene.
//!
//! **Por qué existe este módulo.** Hasta la 0.10.0 lo que el arnés podía hacer estaba escrito en
//! cuatro sitios y en ninguno era el código: dos constantes con los nombres pero sin argumentos
//! (`herramientas.rs`), una petición al motor sin `properties` (`llama.rs`), una tabla a mano en el
//! preámbulo del modelo y una guía para los demás agentes que afirmaba que el modelo no tenía
//! herramientas. Los cuatro divergieron, y dos de las divergencias costaron una jornada de trabajo
//! en NatureLand el 23/09/2026.
//!
//! Aquí se declara una vez y de aquí come todo. **Este módulo no ejecuta nada**: no toca disco, no
//! llama al motor y no decide si una solicitud se concede. Eso sigue siendo de `herramientas.rs`.

/// Un argumento de un verbo, con el nombre **exacto** que el modelo tiene que escribir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argumento {
    /// El nombre tal y como lo lee `Repertorio::argumento`. Si esto y el código discrepan, el
    /// modelo recibe una denegación por seguir sus propias instrucciones.
    pub nombre: &'static str,
    /// Una línea para el modelo y para quien dirige. No es documentación interna: se publica.
    pub para_que: &'static str,
}

/// En qué situación está un verbo del repertorio.
///
/// Las tres son distintas a propósito. Reconocer no es conceder: un verbo denegado se reconoce para
/// poder explicar por qué no se da, pero **no se le declara al modelo**, porque anunciarlo solo
/// sirve para que queme solicitudes contra el tope del encargo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Se le declara al modelo y se atiende.
    Concedible,
    /// Se reconoce y se deniega siempre. El motivo se le explica al modelo tal cual.
    DenegadaSiempre { motivo: &'static str },
    /// Se deniega hoy y **debe volver a `Concedible`** cuando se cumpla `condicion`. Existe para
    /// que esa condición sea un dato y no un párrafo que alguien tenga que acordarse de leer.
    Pendiente {
        motivo: &'static str,
        condicion: &'static str,
    },
}

/// Todo lo que se sabe de un verbo.
#[derive(Debug, Clone, Copy)]
pub struct Ficha {
    pub nombre: &'static str,
    pub para_que: &'static str,
    pub obligatorios: &'static [Argumento],
    pub opcionales: &'static [Argumento],
    pub estado: Estado,
}

impl Ficha {
    /// Si hoy se le declara al modelo y se atiende.
    pub fn es_concedible(&self) -> bool {
        matches!(self.estado, Estado::Concedible)
    }

    /// La firma en una línea: obligatorios en orden, opcionales entre corchetes.
    ///
    /// Es lo que se le devuelve al modelo cuando se le deniega por argumentos. Antes de la 0.10.0
    /// la denegación decía solo qué faltaba, nunca qué había: el modelo no tiene memoria entre
    /// encargos ni documentación de sus verbos, así que solo podía adivinar. Adivinó cinco veces
    /// seguidas el 23/09/2026 y costó un encargo entero.
    pub fn firma(&self) -> String {
        let mut partes: Vec<String> = self
            .obligatorios
            .iter()
            .map(|a| a.nombre.to_string())
            .collect();
        partes.extend(self.opcionales.iter().map(|a| format!("[{}]", a.nombre)));
        format!("{}({})", self.nombre, partes.join(", "))
    }
}

/// Los ocho verbos que el arnés reconoce, en el orden del repertorio del protocolo.
///
/// **Este orden no es pedagógico: es el que fija la prueba
/// `el_repertorio_es_exactamente_el_que_fija_la_especificacion` de `herramientas.rs`, de antes de
/// que este módulo existiera.** `Repertorio::nombres()` se deriva de aquí, así que si este orden y
/// el de `Repertorio::nombres()` se separan, la derivación cambia el contrato sin que nadie lo pida.
/// Por eso el orden se copia tal cual, no se reinventa.
pub const REPERTORIO: &[Ficha] = &[
    Ficha {
        nombre: "leer_fichero",
        para_que: "Leer un fichero de la carpeta de trabajo, para no tener que suponer qué dice",
        obligatorios: &[Argumento {
            nombre: "ruta",
            para_que: "Ruta del fichero, relativa a la carpeta de trabajo",
        }],
        opcionales: &[],
        estado: Estado::Concedible,
    },
    Ficha {
        nombre: "listar",
        para_que: "Ver qué hay en un directorio de la carpeta de trabajo",
        obligatorios: &[Argumento {
            nombre: "ruta",
            para_que: "Ruta del directorio, relativa a la carpeta de trabajo",
        }],
        opcionales: &[],
        estado: Estado::Concedible,
    },
    Ficha {
        nombre: "buscar",
        para_que: "Buscar texto en los ficheros de la carpeta de trabajo",
        // Sin argumentos declarados a propósito: llega en la v1.1 y su forma no está decidida.
        // Inventarla aquí sería escribir una firma que nadie ha diseñado.
        obligatorios: &[],
        opcionales: &[],
        estado: Estado::DenegadaSiempre {
            motivo: "«buscar» llega en la v1.1",
        },
    },
    Ficha {
        nombre: "verificar",
        para_que: "Pedir que se ejecute una verificación del proyecto y darte su resultado",
        obligatorios: &[Argumento {
            nombre: "cual",
            para_que: "Cuál de las verificaciones de la lista blanca quieres",
        }],
        opcionales: &[],
        estado: Estado::Pendiente {
            motivo: "todavía no la ejecuta nadie: el arnés aún no encadena el ciclo completo, así \
                     que no hay resultado que darte. Llegará cuando lo encadene. No publiques como \
                     verificado nada que no se haya ejecutado de verdad.",
            condicion: "cuando `ciclo.rs` ejecute la verificación de verdad, este estado pasa a \
                        `Concedible` y hay que ajustar la prueba \
                        `solo_son_concedibles_los_cinco_que_hoy_se_atienden`. Es la Tarea 16.",
        },
    },
    Ficha {
        nombre: "publicar",
        para_que: "Dejar tu respuesta en tu buzón del canal, para que la lea el equipo",
        obligatorios: &[Argumento {
            nombre: "texto",
            para_que: "El cuerpo entero de lo que quieres publicar",
        }],
        opcionales: &[],
        estado: Estado::Concedible,
    },
    Ficha {
        nombre: "escribir_propuesta",
        para_que: "Entregar código o un documento como candidato, para que alguien lo revise",
        obligatorios: &[
            Argumento {
                nombre: "nombre",
                para_que: "Nombre del fichero, a secas: sin rutas ni carpetas",
            },
            Argumento {
                nombre: "contenido",
                para_que: "El contenido completo del fichero, no un fragmento",
            },
        ],
        opcionales: &[],
        estado: Estado::Concedible,
    },
    Ficha {
        nombre: "proponer_poda",
        para_que: "Proponer qué bloques de un buzón ya están superados",
        obligatorios: &[],
        opcionales: &[],
        estado: Estado::DenegadaSiempre {
            motivo: "«proponer_poda» la invoca el arnés, no el modelo",
        },
    },
    Ficha {
        nombre: "reservar",
        para_que: "Avisar al equipo de que vas a trabajar sobre un fichero",
        obligatorios: &[
            Argumento {
                nombre: "ruta",
                para_que: "Ruta del fichero que reservas, relativa a la carpeta de trabajo",
            },
            Argumento {
                nombre: "motivo",
                para_que: "Para qué lo reservas, en una línea",
            },
        ],
        opcionales: &[],
        estado: Estado::Concedible,
    },
];

/// Todos los verbos que el arnés reconoce.
pub fn todas() -> &'static [Ficha] {
    REPERTORIO
}

/// La ficha de un verbo, si existe.
pub fn buscar(nombre: &str) -> Option<&'static Ficha> {
    REPERTORIO.iter().find(|f| f.nombre == nombre)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn publicar_declara_el_argumento_que_el_repertorio_pide_de_verdad() {
        // El preámbulo escrito a mano decía «cuerpo» y `Repertorio::publicar` pide «texto»: un
        // modelo que siguiera sus instrucciones se llevaba una denegación. Esta prueba existe
        // para que ese desajuste no pueda volver.
        let publicar = buscar("publicar").expect("publicar está en el repertorio");
        assert_eq!(publicar.obligatorios.len(), 1);
        assert_eq!(publicar.obligatorios[0].nombre, "texto");
    }

    #[test]
    fn la_firma_pone_los_obligatorios_en_orden_y_los_opcionales_entre_corchetes() {
        let propuesta = buscar("escribir_propuesta").expect("está en el repertorio");
        assert_eq!(propuesta.firma(), "escribir_propuesta(nombre, contenido)");
    }

    #[test]
    fn todo_verbo_declarado_dice_para_que_sirve() {
        for ficha in todas() {
            assert!(
                !ficha.para_que.trim().is_empty(),
                "«{}» no dice para qué sirve",
                ficha.nombre
            );
        }
    }

    #[test]
    fn todo_argumento_declarado_dice_para_que_sirve() {
        for ficha in todas() {
            for argumento in ficha.obligatorios.iter().chain(ficha.opcionales) {
                assert!(
                    !argumento.para_que.trim().is_empty(),
                    "«{}» no explica el argumento «{}»",
                    ficha.nombre,
                    argumento.nombre
                );
            }
        }
    }

    #[test]
    fn ningun_verbo_se_declara_dos_veces() {
        let mut vistos: Vec<&str> = todas().iter().map(|f| f.nombre).collect();
        let total = vistos.len();
        vistos.sort_unstable();
        vistos.dedup();
        assert_eq!(vistos.len(), total, "hay un verbo declarado dos veces");
    }

    #[test]
    fn solo_son_concedibles_los_cinco_que_hoy_se_atienden() {
        let concedibles: Vec<&str> = todas()
            .iter()
            .filter(|f| f.es_concedible())
            .map(|f| f.nombre)
            .collect();
        assert_eq!(
            concedibles,
            vec![
                "leer_fichero",
                "listar",
                "publicar",
                "escribir_propuesta",
                "reservar"
            ]
        );
    }

    #[test]
    fn buscar_un_verbo_que_no_existe_no_devuelve_nada() {
        assert!(buscar("firmar").is_none());
    }

    #[test]
    fn el_orden_de_la_ficha_es_el_del_repertorio_que_ya_fijaba_la_especificacion() {
        // La Tarea 2 deriva `Repertorio::nombres()` de aquí y hay una prueba que fija ese orden
        // desde antes de que la ficha existiera. Si los dos órdenes se separan, la derivación
        // cambia el contrato sin que nadie lo pida.
        let nombres: Vec<&str> = todas().iter().map(|f| f.nombre).collect();
        assert_eq!(
            nombres,
            vec![
                "leer_fichero",
                "listar",
                "buscar",
                "verificar",
                "publicar",
                "escribir_propuesta",
                "proponer_poda",
                "reservar"
            ]
        );
    }
}
