//! Lo que Programator deduce por su cuenta del estado de cada propuesta, sin que nadie se lo diga.
//! Puro: recibe lo encontrado en el proyecto y decide.
//!
//! - `aceptada`: hay en el proyecto un fichero con el nombre de la propuesta (sin el `-00N` que le
//!   puso la carpeta de candidatos) y la misma huella.
//! - `sustituida`: el mismo buzón dejó después otra propuesta para el mismo nombre pedido
//!   (`x.py` y luego `x-002.py`).
//!
//! Solo se deduce sobre lo que sigue en `propuesta`: lo que dijo el agente manda y no se vuelve a
//! deducir.

use super::{EntradaDeEntrega, EstadoDeLaPropuesta, RegistroDeEntregas};
use std::collections::BTreeSet;

/// Cuántas cifras lleva la variante que añade la carpeta de candidatos (`-002`), la de
/// `herramientas::candidatos::siguiente_libre`.
const CIFRAS_DE_LA_VARIANTE: usize = 3;

/// Un fichero del proyecto con el nombre de alguna propuesta: su ruta relativa y su huella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encontrado {
    pub ruta: String,
    pub huella: String,
}

/// El nombre pedido de una propuesta: sin la variante `-00N` que añadió la carpeta de candidatos
/// para no pisar otra. `sub/x-002.py` es `sub/x.py`.
pub fn nombre_pedido(propuesta: &str) -> String {
    let (carpeta, fichero) = match propuesta.rsplit_once('/') {
        Some((carpeta, fichero)) => (Some(carpeta), fichero),
        None => (None, propuesta),
    };
    let (tronco, extension) = match fichero.rsplit_once('.') {
        Some((tronco, extension)) if !tronco.is_empty() => (tronco, Some(extension)),
        _ => (fichero, None),
    };
    let tronco = sin_variante(tronco);
    let fichero = match extension {
        Some(extension) => format!("{tronco}.{extension}"),
        None => tronco.to_string(),
    };
    match carpeta {
        Some(carpeta) => format!("{carpeta}/{fichero}"),
        None => fichero,
    }
}

/// El tronco sin un `-NNN` final de tres cifras.
fn sin_variante(tronco: &str) -> &str {
    match tronco.rsplit_once('-') {
        Some((resto, cifras))
            if !resto.is_empty()
                && cifras.len() == CIFRAS_DE_LA_VARIANTE
                && cifras.bytes().all(|b| b.is_ascii_digit()) =>
        {
            resto
        }
        _ => tronco,
    }
}

/// El nombre de fichero, sin carpeta, que se busca en el proyecto para una propuesta.
pub fn fichero_buscado(propuesta: &str) -> String {
    let pedido = nombre_pedido(propuesta);
    pedido
        .rsplit_once('/')
        .map_or(pedido.clone(), |(_, fichero)| fichero.to_string())
}

/// Los nombres de fichero que hay que buscar en el proyecto: los de lo que sigue en `propuesta`.
pub fn ficheros_buscados(registro: &RegistroDeEntregas) -> BTreeSet<String> {
    registro
        .entregas
        .iter()
        .filter(|entrada| sigue_propuesta(entrada))
        .map(|entrada| fichero_buscado(&entrada.propuesta))
        .collect()
}

/// Deduce lo que se pueda. Con `encontrados`, primero las aceptadas; después, siempre, las
/// sustituidas entre lo que siga en `propuesta`. Devuelve si cambió algo.
pub fn deducir(registro: &mut RegistroDeEntregas, encontrados: Option<&[Encontrado]>) -> bool {
    let mut cambio = false;
    if let Some(encontrados) = encontrados {
        for entrada in registro.entregas.iter_mut().filter(|e| sigue_propuesta(e)) {
            let buscado = fichero_buscado(&entrada.propuesta);
            if let Some(destino) = encontrados.iter().find(|f| {
                f.huella == entrada.huella
                    && fichero_de(&f.ruta) == buscado
                    && !entrada.ya_en_el_proyecto.contains(&f.ruta)
            }) {
                entrada.estado = EstadoDeLaPropuesta::Aceptada;
                entrada.destino = Some(destino.ruta.clone());
                cambio = true;
            }
        }
    }
    let total = registro.entregas.len();
    for posicion in 0..total {
        let (antes, despues) = registro.entregas.split_at_mut(posicion + 1);
        let entrada = &mut antes[posicion];
        if !sigue_propuesta(entrada) {
            continue;
        }
        let pedido = nombre_pedido(&entrada.propuesta);
        if despues
            .iter()
            .any(|otra| otra.de == entrada.de && nombre_pedido(&otra.propuesta) == pedido)
        {
            entrada.estado = EstadoDeLaPropuesta::Sustituida;
            cambio = true;
        }
    }
    cambio
}

/// Si todavía está en `propuesta`: lo único sobre lo que se deduce.
fn sigue_propuesta(entrada: &EntradaDeEntrega) -> bool {
    entrada.estado == EstadoDeLaPropuesta::Propuesta
}

fn fichero_de(ruta: &str) -> &str {
    ruta.rsplit(['/', '\\']).next().unwrap_or(ruta)
}

#[cfg(test)]
mod pruebas {
    use super::super::{huella, VeredictoDeLaEntrega, LO_DIO_PROGRAMATOR};
    use super::*;

    fn entrada(propuesta: &str, de: &str, contenido: &str) -> EntradaDeEntrega {
        EntradaDeEntrega {
            propuesta: propuesta.to_string(),
            encargo: "id".to_string(),
            nombre_del_encargo: "n".to_string(),
            de: de.to_string(),
            modelo: None,
            fecha: "2026-11-25 16:00:00".to_string(),
            veredicto: VeredictoDeLaEntrega::Pasa,
            huella: huella(contenido),
            estado: EstadoDeLaPropuesta::Propuesta,
            estado_lo_dio: LO_DIO_PROGRAMATOR.to_string(),
            destino: None,
            motivo: None,
            archivada: None,
            ya_en_el_proyecto: Vec::new(),
        }
    }

    fn registro(entradas: Vec<EntradaDeEntrega>) -> RegistroDeEntregas {
        RegistroDeEntregas {
            entregas: entradas,
            ..RegistroDeEntregas::default()
        }
    }

    fn estados(registro: &RegistroDeEntregas) -> Vec<EstadoDeLaPropuesta> {
        registro.entregas.iter().map(|e| e.estado).collect()
    }

    #[test]
    fn el_nombre_pedido_quita_solo_la_variante_de_tres_cifras() {
        assert_eq!(nombre_pedido("x-002.py"), "x.py");
        assert_eq!(nombre_pedido("sub/x-099.py"), "sub/x.py");
        assert_eq!(nombre_pedido("x-002"), "x");
        assert_eq!(nombre_pedido("x.py"), "x.py");
        assert_eq!(nombre_pedido("x-2.py"), "x-2.py");
        assert_eq!(nombre_pedido("calc-2026.py"), "calc-2026.py");
        assert_eq!(nombre_pedido("-002.py"), "-002.py");
        assert_eq!(nombre_pedido(".bashrc"), ".bashrc");
        assert_eq!(fichero_buscado("sub/x-002.py"), "x.py");
    }

    /// E2: copiada tal cual al proyecto, en cualquier carpeta, está aceptada; con CRLF también.
    #[test]
    fn una_propuesta_copiada_al_proyecto_esta_aceptada() {
        let mut con_lf = registro(vec![entrada("x-002.py", "claude.md", "a\nb\n")]);
        let encontrados = [Encontrado {
            ruta: "src/x.py".to_string(),
            huella: huella("a\r\nb\r\n"),
        }];

        assert!(deducir(&mut con_lf, Some(&encontrados)));

        assert_eq!(estados(&con_lf), [EstadoDeLaPropuesta::Aceptada]);
        assert_eq!(con_lf.entregas[0].destino.as_deref(), Some("src/x.py"));
    }

    /// Lo que ya estaba igual al escribirla no cuenta; una copia nueva en otro sitio, sí.
    #[test]
    fn lo_que_ya_estaba_identico_no_la_acepta() {
        let mut igual = entrada("x.py", "claude.md", "a\n");
        igual.ya_en_el_proyecto = vec!["src/x.py".to_string()];
        let mut registro = registro(vec![igual]);
        let ya_estaba = [Encontrado {
            ruta: "src/x.py".to_string(),
            huella: huella("a\n"),
        }];

        assert!(!deducir(&mut registro, Some(&ya_estaba)));
        assert_eq!(estados(&registro), [EstadoDeLaPropuesta::Propuesta]);

        let con_copia_nueva = [
            ya_estaba[0].clone(),
            Encontrado {
                ruta: "lib/x.py".to_string(),
                huella: huella("a\n"),
            },
        ];
        assert!(deducir(&mut registro, Some(&con_copia_nueva)));
        assert_eq!(registro.entregas[0].destino.as_deref(), Some("lib/x.py"));
    }

    #[test]
    fn otro_contenido_u_otro_nombre_no_la_acepta() {
        let mut registro = registro(vec![entrada("x.py", "claude.md", "a\n")]);
        let encontrados = [
            Encontrado {
                ruta: "src/x.py".to_string(),
                huella: huella("b\n"),
            },
            Encontrado {
                ruta: "src/y.py".to_string(),
                huella: huella("a\n"),
            },
        ];

        assert!(!deducir(&mut registro, Some(&encontrados)));
        assert_eq!(estados(&registro), [EstadoDeLaPropuesta::Propuesta]);
    }

    /// E3.
    #[test]
    fn una_posterior_del_mismo_nombre_y_buzon_sustituye_a_la_anterior() {
        let mut registro = registro(vec![
            entrada("x.py", "claude.md", "1"),
            entrada("x.py", "codex.md", "1"),
            entrada("x-002.py", "claude.md", "2"),
            entrada("y.py", "claude.md", "3"),
        ]);

        assert!(deducir(&mut registro, None));

        assert_eq!(
            estados(&registro),
            [
                EstadoDeLaPropuesta::Sustituida,
                EstadoDeLaPropuesta::Propuesta,
                EstadoDeLaPropuesta::Propuesta,
                EstadoDeLaPropuesta::Propuesta,
            ]
        );
    }

    #[test]
    fn lo_que_dijo_el_agente_no_se_vuelve_a_deducir() {
        let mut rechazada = entrada("x.py", "claude.md", "a\n");
        rechazada.estado = EstadoDeLaPropuesta::Rechazada;
        let mut registro = registro(vec![rechazada, entrada("x-002.py", "claude.md", "b\n")]);
        let encontrados = [Encontrado {
            ruta: "x.py".to_string(),
            huella: huella("a\n"),
        }];

        assert!(!deducir(&mut registro, Some(&encontrados)));
        assert_eq!(
            estados(&registro),
            [
                EstadoDeLaPropuesta::Rechazada,
                EstadoDeLaPropuesta::Propuesta
            ]
        );
    }

    #[test]
    fn solo_se_buscan_los_ficheros_de_lo_que_sigue_propuesto() {
        let mut aceptada = entrada("a.py", "claude.md", "a");
        aceptada.estado = EstadoDeLaPropuesta::Aceptada;
        let registro = registro(vec![aceptada, entrada("sub/b-002.py", "claude.md", "b")]);

        assert_eq!(
            ficheros_buscados(&registro).into_iter().collect::<Vec<_>>(),
            ["b.py"]
        );
    }
}
