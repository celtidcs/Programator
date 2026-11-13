//! Las notas del banco, de 0 a 5, calculadas a partir de hechos ya medidos.
//!
//! Solo funciones puras: aquí no se ejecuta nada, no se lee el disco ni se mira el reloj. Lo que
//! se mide (si compila, qué comprobaciones pasan, qué publicó el modelo) llega ya medido.
//!
//! - **Comprobaciones** (oficio y función acotada): 0 si no compila; si compila, cinco por la
//!   fracción de comprobaciones superadas, contando los requisitos de texto como comprobaciones.
//! - **Auditoría**: cinco por aciertos entre sembrados más inventados. Se pierde tanto por lo que
//!   se escapa como por lo que se inventa.
//! - **Fichero limpio**: 5 sin inventados, un punto menos por cada uno, sin bajar de 0.
//!
//! Todas se redondean a medio punto. Lo que no se pudo comprobar es [`Nota::NoEvaluado`], que no
//! cuenta como 0 en ninguna media.

use super::ficha::DefectoSembrado;
use crate::arnes::herramientas::auditoria::separar_campos;

/// La escala de la evaluación del 22/09/2026, que el banco conserva.
pub const NOTA_MAXIMA: f64 = 5.0;
/// Las notas se dan a medio punto, como el 22/09/2026.
const PASO_DE_REDONDEO: f64 = 0.5;
/// Lo que cuesta cada hallazgo inventado en un fichero limpio.
const PUNTOS_POR_INVENTADO: f64 = 1.0;
/// Una línea con al menos tantas barras parece un intento de hallazgo formal.
const BARRAS_DE_UN_INTENTO: usize = 3;
/// El encabezado de una tabla de hallazgos, que no es un hallazgo.
const ENCABEZADO_DE_TIPO: &str = "TIPO";

/// La nota de un encargo en una semilla.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Nota {
    Evaluada(f64),
    /// No se pudo comprobar, por ejemplo porque faltaba una herramienta. Nunca cuenta como 0.
    NoEvaluado,
}

fn redondear(valor: f64) -> f64 {
    (valor / PASO_DE_REDONDEO).round() * PASO_DE_REDONDEO
}

/// Lo que se midió de una entrega con pruebas ocultas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HechosDeComprobaciones {
    /// Faltaba una herramienta o el banco no ejecuta código.
    NoEvaluado,
    /// El modelo no entregó el fichero dentro del tiempo.
    NoEntregado,
    NoCompila,
    /// `total` incluye los requisitos de texto.
    Compila {
        superadas: usize,
        total: usize,
    },
}

/// Oficio y función acotada.
pub fn nota_de_comprobaciones(hechos: HechosDeComprobaciones) -> Nota {
    match hechos {
        HechosDeComprobaciones::NoEvaluado => Nota::NoEvaluado,
        HechosDeComprobaciones::NoEntregado | HechosDeComprobaciones::NoCompila => {
            Nota::Evaluada(0.0)
        }
        // Una ficha válida siempre tiene comprobaciones; si no, no hay nada que puntuar.
        HechosDeComprobaciones::Compila { total: 0, .. } => Nota::NoEvaluado,
        HechosDeComprobaciones::Compila { superadas, total } => {
            let fraccion = superadas.min(total) as f64 / total as f64;
            Nota::Evaluada(redondear(NOTA_MAXIMA * fraccion))
        }
    }
}

/// Un hallazgo leído de la respuesta del modelo, tal como lo cuenta el banco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HallazgoLeido {
    pub tipo: String,
    pub linea: usize,
}

/// Lo que se saca de una respuesta de auditoría.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LecturaDeHallazgos {
    pub hallazgos: Vec<HallazgoLeido>,
    /// Líneas que parecen un hallazgo pero no tienen el formato formal: no cuentan ni como
    /// acierto ni como inventado, y se informan aparte.
    pub sin_formato: usize,
}

/// Lee los hallazgos con el formato formal `TIPO | fichero | miembro | línea | literal |
/// consecuencia`.
///
/// Dos diferencias con el arnés: acepta cualquier tipo (U02 usa `VALIDACION` y `SIGNIFICADO`), y
/// lee el número inicial del campo de línea, porque lo publicado puede traer ya la corrección del
/// arnés («106 (corregida; el modelo dijo 200)»). El fichero citado no se mira: cada auditoría
/// revisa un solo fichero.
pub fn leer_hallazgos(respuesta: &str) -> LecturaDeHallazgos {
    let mut lectura = LecturaDeHallazgos::default();
    for linea in respuesta.lines() {
        match separar_campos(linea) {
            Some(campos) if campos.tipo == ENCABEZADO_DE_TIPO => {}
            Some(campos) => match numero_inicial(campos.linea) {
                Some(numero) if !campos.tipo.is_empty() => lectura.hallazgos.push(HallazgoLeido {
                    tipo: campos.tipo.to_string(),
                    linea: numero,
                }),
                _ => lectura.sin_formato += 1,
            },
            None if parece_un_intento(linea) => lectura.sin_formato += 1,
            None => {}
        }
    }
    lectura
}

fn numero_inicial(campo: &str) -> Option<usize> {
    let digitos: String = campo.chars().take_while(char::is_ascii_digit).collect();
    digitos.parse().ok()
}

/// Una línea con varias barras que no es el separador de una tabla de Markdown.
fn parece_un_intento(linea: &str) -> bool {
    let es_separador = linea
        .chars()
        .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'));
    !es_separador && linea.matches('|').count() >= BARRAS_DE_UN_INTENTO
}

/// El recuento de una auditoría, que el resultado guarda además de la nota.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecuentoDeAuditoria {
    pub sembrados: usize,
    pub aciertos: usize,
    /// Hallazgos que no caen cerca de ningún defecto.
    pub inventados: usize,
    /// Hallazgos que caen cerca de un defecto ya acertado por otro: el defecto existe, así que no
    /// son inventados, pero tampoco suman.
    pub repetidos: usize,
    pub sin_formato: usize,
}

/// Empareja hallazgos con defectos: cada defecto cuenta una sola vez, y un hallazgo acierta si
/// cae a `margen` líneas o menos de alguna de las líneas del defecto.
///
/// El emparejamiento es máximo, no voraz, para que el orden de los hallazgos no cambie el
/// recuento cuando varios defectos comparten líneas.
pub fn contar_auditoria(
    lectura: &LecturaDeHallazgos,
    defectos: &[DefectoSembrado],
    margen: usize,
) -> RecuentoDeAuditoria {
    let cerca = |hallazgo: &HallazgoLeido, defecto: &DefectoSembrado| {
        defecto
            .lineas
            .iter()
            .any(|&linea| linea.abs_diff(hallazgo.linea) <= margen)
    };
    let candidatos: Vec<Vec<usize>> = lectura
        .hallazgos
        .iter()
        .map(|h| {
            (0..defectos.len())
                .filter(|&d| cerca(h, &defectos[d]))
                .collect()
        })
        .collect();
    let aciertos = emparejamiento_maximo(&candidatos, defectos.len());
    let fuera = candidatos.iter().filter(|c| c.is_empty()).count();
    RecuentoDeAuditoria {
        sembrados: defectos.len(),
        aciertos,
        inventados: fuera,
        repetidos: lectura.hallazgos.len() - aciertos - fuera,
        sin_formato: lectura.sin_formato,
    }
}

/// Tamaño del emparejamiento máximo entre hallazgos y defectos (caminos de aumento).
fn emparejamiento_maximo(candidatos: &[Vec<usize>], defectos: usize) -> usize {
    let mut dueno: Vec<Option<usize>> = vec![None; defectos];
    let mut total = 0;
    for hallazgo in 0..candidatos.len() {
        let mut visto = vec![false; defectos];
        if aumentar(hallazgo, candidatos, &mut dueno, &mut visto) {
            total += 1;
        }
    }
    total
}

fn aumentar(
    hallazgo: usize,
    candidatos: &[Vec<usize>],
    dueno: &mut [Option<usize>],
    visto: &mut [bool],
) -> bool {
    for &defecto in &candidatos[hallazgo] {
        if visto[defecto] {
            continue;
        }
        visto[defecto] = true;
        let libre = match dueno[defecto] {
            None => true,
            Some(otro) => aumentar(otro, candidatos, dueno, visto),
        };
        if libre {
            dueno[defecto] = Some(hallazgo);
            return true;
        }
    }
    false
}

/// Auditoría con defectos sembrados.
pub fn nota_de_auditoria(recuento: &RecuentoDeAuditoria) -> Nota {
    let denominador = recuento.sembrados + recuento.inventados;
    if denominador == 0 {
        return Nota::NoEvaluado;
    }
    Nota::Evaluada(redondear(
        NOTA_MAXIMA * recuento.aciertos as f64 / denominador as f64,
    ))
}

/// Auditoría de un fichero limpio: todo hallazgo formal es inventado.
pub fn nota_de_fichero_limpio(inventados: usize) -> Nota {
    let nota = NOTA_MAXIMA - PUNTOS_POR_INVENTADO * inventados as f64;
    Nota::Evaluada(redondear(nota.max(0.0)))
}

/// La media y la dispersión de un encargo entre semillas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resumen {
    /// `None` si ninguna semilla se pudo evaluar.
    pub media: Option<f64>,
    /// Desviación típica de población: con una sola semilla evaluada es 0.
    pub dispersion: Option<f64>,
    pub evaluadas: usize,
    pub no_evaluadas: usize,
}

/// Resume las notas de un encargo sin contar los [`Nota::NoEvaluado`].
pub fn resumir(notas: &[Nota]) -> Resumen {
    let valores: Vec<f64> = notas
        .iter()
        .filter_map(|nota| match nota {
            Nota::Evaluada(valor) => Some(*valor),
            Nota::NoEvaluado => None,
        })
        .collect();
    let evaluadas = valores.len();
    let no_evaluadas = notas.len() - evaluadas;
    if evaluadas == 0 {
        return Resumen {
            media: None,
            dispersion: None,
            evaluadas,
            no_evaluadas,
        };
    }
    let media = valores.iter().sum::<f64>() / evaluadas as f64;
    let varianza = valores.iter().map(|v| (v - media).powi(2)).sum::<f64>() / evaluadas as f64;
    Resumen {
        media: Some(media),
        dispersion: Some(varianza.sqrt()),
        evaluadas,
        no_evaluadas,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn defecto(lineas: &[usize]) -> DefectoSembrado {
        DefectoSembrado {
            lineas: lineas.to_vec(),
            tipo: "HARDCODEO".into(),
            descripcion: "d".into(),
        }
    }

    fn hallazgo_en(linea: usize) -> String {
        format!("HARDCODEO | f.cs | M | {linea} | x | y")
    }

    fn evaluada(nota: Nota) -> f64 {
        match nota {
            Nota::Evaluada(valor) => valor,
            Nota::NoEvaluado => panic!("se esperaba una nota"),
        }
    }

    // --- Comprobaciones

    #[test]
    fn oficio_todo_bien_es_un_cinco() {
        let nota = nota_de_comprobaciones(HechosDeComprobaciones::Compila {
            superadas: 12,
            total: 12,
        });
        assert_eq!(nota, Nota::Evaluada(5.0));
    }

    #[test]
    fn oficio_que_compila_y_no_pasa_nada_es_un_cero() {
        let nota = nota_de_comprobaciones(HechosDeComprobaciones::Compila {
            superadas: 0,
            total: 9,
        });
        assert_eq!(nota, Nota::Evaluada(0.0));
    }

    #[test]
    fn oficio_que_no_compila_o_no_se_entrega_es_un_cero() {
        assert_eq!(
            nota_de_comprobaciones(HechosDeComprobaciones::NoCompila),
            Nota::Evaluada(0.0)
        );
        assert_eq!(
            nota_de_comprobaciones(HechosDeComprobaciones::NoEntregado),
            Nota::Evaluada(0.0)
        );
    }

    #[test]
    fn oficio_parcial_se_redondea_a_medio_punto() {
        // 8 de 12: 3,33 -> 3,5. 7 de 9: 3,89 -> 4.
        let ocho_de_doce = HechosDeComprobaciones::Compila {
            superadas: 8,
            total: 12,
        };
        let siete_de_nueve = HechosDeComprobaciones::Compila {
            superadas: 7,
            total: 9,
        };
        assert_eq!(nota_de_comprobaciones(ocho_de_doce), Nota::Evaluada(3.5));
        assert_eq!(nota_de_comprobaciones(siete_de_nueve), Nota::Evaluada(4.0));
    }

    #[test]
    fn oficio_sin_herramienta_no_se_evalua() {
        assert_eq!(
            nota_de_comprobaciones(HechosDeComprobaciones::NoEvaluado),
            Nota::NoEvaluado
        );
    }

    // --- Lectura de hallazgos

    #[test]
    fn se_lee_cualquier_tipo_y_la_linea_ya_corregida_por_el_arnes() {
        let respuesta = "Revisión:\n\
             VALIDACION | r.rs | reservar | 43 | x | y\n\
             HARDCODEO | r.rs | f | 106 (corregida; el modelo dijo 200) | x | y\n\
             SOLID | r.rs | f | 7 ⚠️ sin verificar automáticamente | x | y\n";

        let lectura = leer_hallazgos(respuesta);

        let lineas: Vec<usize> = lectura.hallazgos.iter().map(|h| h.linea).collect();
        assert_eq!(lineas, [43, 106, 7]);
        assert_eq!(lectura.hallazgos[0].tipo, "VALIDACION");
        assert_eq!(lectura.sin_formato, 0);
    }

    #[test]
    fn la_prosa_el_encabezado_y_el_separador_no_cuentan() {
        let respuesta = "No encuentro nada.\n\
             TIPO | fichero | miembro | línea | literal | consecuencia\n\
             |---|---|---|---|---|---|\n";

        assert_eq!(leer_hallazgos(respuesta), LecturaDeHallazgos::default());
    }

    #[test]
    fn un_hallazgo_sin_el_formato_formal_se_cuenta_aparte() {
        let respuesta = "| HARDCODEO | f.cs | M | 44 | 0.21m | constante |\n\
             HARDCODEO | f.cs | M | cerca del final | x | y\n";

        let lectura = leer_hallazgos(respuesta);

        assert!(lectura.hallazgos.is_empty());
        assert_eq!(lectura.sin_formato, 2);
    }

    // --- Auditoría

    fn lectura_de(lineas: &[usize]) -> LecturaDeHallazgos {
        let texto: Vec<String> = lineas.iter().map(|&l| hallazgo_en(l)).collect();
        leer_hallazgos(&texto.join("\n"))
    }

    #[test]
    fn auditoria_que_acierta_todo_y_no_inventa_es_un_cinco() {
        let defectos = [defecto(&[44]), defecto(&[41, 62])];

        let recuento = contar_auditoria(&lectura_de(&[45, 62]), &defectos, 2);

        assert_eq!(recuento.aciertos, 2);
        assert_eq!(recuento.inventados, 0);
        assert_eq!(nota_de_auditoria(&recuento), Nota::Evaluada(5.0));
    }

    #[test]
    fn auditoria_que_solo_inventa_es_un_cero() {
        let recuento = contar_auditoria(&lectura_de(&[1, 2, 3]), &[defecto(&[44])], 2);

        assert_eq!(recuento.aciertos, 0);
        assert_eq!(recuento.inventados, 3);
        assert_eq!(nota_de_auditoria(&recuento), Nota::Evaluada(0.0));
    }

    #[test]
    fn auditoria_sin_respuesta_es_un_cero() {
        let recuento = contar_auditoria(&LecturaDeHallazgos::default(), &[defecto(&[44])], 2);

        assert_eq!(nota_de_auditoria(&recuento), Nota::Evaluada(0.0));
    }

    #[test]
    fn se_pierde_por_lo_que_se_escapa_y_por_lo_que_se_inventa() {
        // 2 aciertos de 4 sembrados, 1 inventado: 5 * 2 / 5 = 2.
        let defectos = [
            defecto(&[10]),
            defecto(&[20]),
            defecto(&[30]),
            defecto(&[40]),
        ];

        let recuento = contar_auditoria(&lectura_de(&[10, 20, 90]), &defectos, 2);

        assert_eq!(evaluada(nota_de_auditoria(&recuento)), 2.0);
    }

    #[test]
    fn un_hallazgo_cerca_de_dos_defectos_solo_cuenta_una_vez() {
        let defectos = [defecto(&[10]), defecto(&[14])];

        let recuento = contar_auditoria(&lectura_de(&[12]), &defectos, 2);

        assert_eq!(recuento.aciertos, 1);
        assert_eq!(recuento.inventados, 0);
    }

    #[test]
    fn dos_defectos_en_la_misma_linea_se_aciertan_con_dos_hallazgos() {
        let defectos = [defecto(&[90]), defecto(&[90])];

        let recuento = contar_auditoria(&lectura_de(&[90, 90]), &defectos, 2);

        assert_eq!(recuento.aciertos, 2);
    }

    #[test]
    fn el_orden_de_los_hallazgos_no_cambia_el_recuento() {
        // El de la línea 11 puede ir con los dos; el de la 9 solo con el primero.
        let defectos = [defecto(&[10]), defecto(&[12])];

        let a = contar_auditoria(&lectura_de(&[11, 9]), &defectos, 1);
        let b = contar_auditoria(&lectura_de(&[9, 11]), &defectos, 1);

        assert_eq!(a.aciertos, 2);
        assert_eq!(a, b);
    }

    #[test]
    fn repetir_un_defecto_ya_acertado_no_es_inventar() {
        let recuento = contar_auditoria(&lectura_de(&[44, 45]), &[defecto(&[44])], 2);

        assert_eq!(recuento.aciertos, 1);
        assert_eq!(recuento.repetidos, 1);
        assert_eq!(recuento.inventados, 0);
        assert_eq!(nota_de_auditoria(&recuento), Nota::Evaluada(5.0));
    }

    #[test]
    fn el_margen_marca_la_frontera_del_acierto() {
        let defectos = [defecto(&[50])];

        assert_eq!(
            contar_auditoria(&lectura_de(&[52]), &defectos, 2).aciertos,
            1
        );
        assert_eq!(
            contar_auditoria(&lectura_de(&[53]), &defectos, 2).inventados,
            1
        );
    }

    #[test]
    fn los_hallazgos_sin_formato_no_cuentan_en_la_nota() {
        let mut lectura = lectura_de(&[44]);
        lectura.sin_formato = 5;

        let recuento = contar_auditoria(&lectura, &[defecto(&[44])], 2);

        assert_eq!(recuento.sin_formato, 5);
        assert_eq!(nota_de_auditoria(&recuento), Nota::Evaluada(5.0));
    }

    // --- Fichero limpio

    #[test]
    fn fichero_limpio_sin_inventados_es_un_cinco_y_baja_un_punto_por_cada_uno() {
        assert_eq!(nota_de_fichero_limpio(0), Nota::Evaluada(5.0));
        assert_eq!(nota_de_fichero_limpio(2), Nota::Evaluada(3.0));
    }

    #[test]
    fn fichero_limpio_no_baja_de_cero() {
        assert_eq!(nota_de_fichero_limpio(9), Nota::Evaluada(0.0));
    }

    // --- Resumen entre semillas

    #[test]
    fn la_media_no_cuenta_los_no_evaluados() {
        let resumen = resumir(&[Nota::Evaluada(4.0), Nota::NoEvaluado, Nota::Evaluada(2.0)]);

        assert_eq!(resumen.media, Some(3.0));
        assert_eq!(resumen.dispersion, Some(1.0));
        assert_eq!(resumen.evaluadas, 2);
        assert_eq!(resumen.no_evaluadas, 1);
    }

    #[test]
    fn sin_ninguna_nota_evaluada_no_hay_media() {
        let resumen = resumir(&[Nota::NoEvaluado, Nota::NoEvaluado]);

        assert_eq!(resumen.media, None);
        assert_eq!(resumen.dispersion, None);
    }

    #[test]
    fn una_sola_semilla_tiene_dispersion_cero() {
        assert_eq!(resumir(&[Nota::Evaluada(3.5)]).dispersion, Some(0.0));
    }
}
