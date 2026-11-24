//! Las cuentas que se sacan de los encargos de una tanda: el resumen de cada encargo juntando sus
//! variantes y semillas, la nota de cada familia, las filas de desempeño y las velocidades del
//! motor. Todo son funciones puras: el informe y el registro de modelos parten de las mismas cifras.

use super::filas::FILAS_DE_DESEMPENO;
use super::{EstadoDeNota, PuntuadaCon, ResultadoDeEncargo, ResultadoDeSemilla, ResumenDeClase};
use crate::banco::ficha::{ClaseDeUso, Familia};
use crate::banco::puntuacion::{resumir, Nota, Resumen};
use crate::unidades::MS_POR_SEGUNDO;

/// Cómo quedó un encargo, juntando sus variantes y sus semillas.
#[derive(Debug, Clone, PartialEq)]
pub struct ResumenDeEncargo<'a> {
    pub id: &'a str,
    pub titulo: &'a str,
    pub familia: Familia,
    pub clase: Option<ClaseDeUso>,
    /// `None` mientras quede alguna semilla pendiente.
    pub resumen: Option<Resumen>,
    pub pendientes: usize,
    pub superadas: Option<(usize, usize)>,
    pub aciertos: Option<(usize, usize)>,
    pub inventados: usize,
    /// Por qué no se evaluaron las semillas que no se evaluaron, sin repetir. Sin esto, un
    /// «no evaluado» no dice si la entrega faltó, no compiló o faltaba algo en el equipo.
    pub motivos_no_evaluado: Vec<&'a str>,
    /// Cada semilla cuyo código intentó usar la red, con los destinos.
    pub intentos_de_red: Vec<(i64, Vec<String>)>,
    /// Las semillas de código puntuadas con la rúbrica, leyendo el código, y no con las pruebas.
    pub por_rubrica: usize,
}

fn sumar_opciones(valores: impl Iterator<Item = Option<usize>>) -> Option<usize> {
    valores.fold(None, |total, valor| match (total, valor) {
        (None, None) => None,
        (total, valor) => Some(total.unwrap_or(0) + valor.unwrap_or(0)),
    })
}

/// Los encargos agrupados por el que cuentan, en el orden en que aparecen.
pub fn resumir_encargos(encargos: &[ResultadoDeEncargo]) -> Vec<ResumenDeEncargo<'_>> {
    // Cada grupo guarda aparte el primero, que da el título, la familia y la clase: así no hace
    // falta indexar un grupo que, por cómo se forma, nunca está vacío.
    let mut grupos: Vec<(&ResultadoDeEncargo, Vec<&ResultadoDeEncargo>)> = Vec::new();
    for encargo in encargos {
        match grupos
            .iter_mut()
            .find(|(primero, _)| primero.cuenta_como == encargo.cuenta_como)
        {
            Some((_, grupo)) => grupo.push(encargo),
            None => grupos.push((encargo, vec![encargo])),
        }
    }
    grupos
        .into_iter()
        .map(|(primero, grupo)| {
            let id = primero.cuenta_como.as_str();
            let semillas: Vec<&ResultadoDeSemilla> =
                grupo.iter().flat_map(|e| e.semillas.iter()).collect();
            let notas: Vec<Option<Nota>> = semillas.iter().map(|s| s.nota_para_resumir()).collect();
            let pendientes = notas.iter().filter(|n| n.is_none()).count();
            let resumen = (pendientes == 0)
                .then(|| resumir(&notas.iter().flatten().copied().collect::<Vec<_>>()));
            let par = |a: Option<usize>, b: Option<usize>| a.zip(b);
            ResumenDeEncargo {
                id,
                titulo: &primero.titulo,
                familia: primero.familia,
                clase: primero.clase,
                resumen,
                pendientes,
                superadas: par(
                    sumar_opciones(semillas.iter().map(|s| s.comprobaciones_superadas)),
                    sumar_opciones(semillas.iter().map(|s| s.comprobaciones_total)),
                ),
                aciertos: par(
                    sumar_opciones(semillas.iter().map(|s| s.aciertos)),
                    sumar_opciones(semillas.iter().map(|s| s.sembrados)),
                ),
                inventados: semillas.iter().filter_map(|s| s.inventados).sum(),
                motivos_no_evaluado: motivos_no_evaluado(&semillas),
                intentos_de_red: semillas
                    .iter()
                    .filter(|s| !s.intentos_de_red.is_empty())
                    .map(|s| (s.semilla, s.intentos_de_red.clone()))
                    .collect(),
                por_rubrica: semillas
                    .iter()
                    .filter(|s| s.puntuada_con == Some(PuntuadaCon::Rubrica))
                    .count(),
            }
        })
        .collect()
}

fn motivos_no_evaluado<'a>(semillas: &[&'a ResultadoDeSemilla]) -> Vec<&'a str> {
    let mut motivos: Vec<&str> = Vec::new();
    for semilla in semillas {
        if semilla.estado != EstadoDeNota::NoEvaluado {
            continue;
        }
        if let Some(motivo) = semilla.detalle.as_deref() {
            if !motivos.contains(&motivo) {
                motivos.push(motivo);
            }
        }
    }
    motivos
}

/// La media de las medias de unos encargos; `None` si no hay ninguna o queda alguno pendiente.
fn media_de_encargos<'a>(resumenes: impl Iterator<Item = &'a ResumenDeEncargo<'a>>) -> Option<f64> {
    let mut medias = Vec::new();
    for resumen in resumenes {
        match resumen.resumen {
            None => return None,
            Some(Resumen { media: Some(m), .. }) => medias.push(m),
            Some(_) => {}
        }
    }
    (!medias.is_empty()).then(|| medias.iter().sum::<f64>() / medias.len() as f64)
}

/// Las familias del banco, en el orden del informe.
pub const FAMILIAS: [Familia; 3] = [Familia::Oficio, Familia::Criterio, Familia::Uso];

/// La nota media de una familia: la media de las de sus encargos. `None` si no tiene ninguno
/// evaluado o le queda alguno pendiente.
pub fn nota_de_familia(resumenes: &[ResumenDeEncargo], familia: Familia) -> Option<f64> {
    media_de_encargos(resumenes.iter().filter(|r| r.familia == familia))
}

/// Tokens generados por segundo de generación, sumando todas las semillas que lo midieron.
pub(crate) fn velocidad_de_generacion<'a>(
    semillas: impl Iterator<Item = &'a ResultadoDeSemilla>,
) -> Option<f64> {
    velocidad(semillas.filter_map(|s| Some((s.tokens_generados?, s.ms_generacion?))))
}

/// Tokens de entrada procesados (sin los que estaban en caché) por segundo.
pub(crate) fn velocidad_de_lectura<'a>(
    semillas: impl Iterator<Item = &'a ResultadoDeSemilla>,
) -> Option<f64> {
    velocidad(semillas.filter_map(|s| {
        let procesados = s
            .tokens_entrada?
            .saturating_sub(s.tokens_en_cache.unwrap_or(0));
        Some((procesados, s.ms_entrada?))
    }))
}

fn velocidad(medidas: impl Iterator<Item = (u64, f64)>) -> Option<f64> {
    let (tokens, ms) = medidas.fold((0u64, 0.0f64), |(t, m), (tokens, ms)| (t + tokens, m + ms));
    (ms > 0.0).then(|| tokens as f64 * MS_POR_SEGUNDO / ms)
}

/// Las filas de desempeño, calculadas a partir de los encargos.
pub fn calcular_clases(encargos: &[ResultadoDeEncargo]) -> Vec<ResumenDeClase> {
    let resumenes = resumir_encargos(encargos);
    FILAS_DE_DESEMPENO
        .iter()
        .map(|&fila| {
            let en_fila =
                |clase: Option<ClaseDeUso>| clase.is_some_and(|c| fila.clases().contains(&c));
            let de_la_fila: Vec<&ResumenDeEncargo> =
                resumenes.iter().filter(|r| en_fila(r.clase)).collect();
            ResumenDeClase {
                fila,
                nota_media: media_de_encargos(de_la_fila.iter().copied()),
                tokens_por_segundo: velocidad_de_generacion(
                    encargos
                        .iter()
                        .filter(|e| en_fila(e.clase))
                        .flat_map(|e| e.semillas.iter()),
                ),
                inventados: de_la_fila.iter().map(|r| r.inventados).sum(),
                pendientes: de_la_fila.iter().map(|r| r.pendientes).sum(),
            }
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::pruebas::{encargo, semilla, tanda_de_ejemplo};
    use crate::banco::resultados::FilaDeDesempeno;

    #[test]
    fn las_variantes_cuentan_como_un_solo_encargo() {
        let mut a = encargo("E10a", Familia::Oficio, None);
        a.cuenta_como = "E10".into();
        a.semillas = vec![semilla(42, EstadoDeNota::Evaluada, Some(2.0))];
        let mut b = a.clone();
        b.id = "E10b".into();
        b.semillas = vec![semilla(42, EstadoDeNota::Evaluada, Some(4.0))];

        let encargos = [a, b];
        let resumenes = resumir_encargos(&encargos);

        assert_eq!(resumenes.len(), 1);
        assert_eq!(resumenes[0].id, "E10");
        assert_eq!(resumenes[0].resumen.unwrap().media, Some(3.0));
    }

    #[test]
    fn la_clase_guarda_nota_velocidad_e_inventados() {
        let clases = tanda_de_ejemplo().clases;
        let formas = clases
            .iter()
            .find(|c| c.fila == FilaDeDesempeno::Formas)
            .unwrap();

        assert_eq!(formas.nota_media, Some(4.5));
        assert_eq!(formas.inventados, 1);
        assert_eq!(formas.tokens_por_segundo, Some(20.0));
        let diagnostico = clases
            .iter()
            .find(|c| c.fila == FilaDeDesempeno::Diagnostico)
            .unwrap();
        assert_eq!(diagnostico.nota_media, None);
    }
}
