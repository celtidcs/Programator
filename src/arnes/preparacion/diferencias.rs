//! Las diferencias entre lo que había en el proyecto y la propuesta ya formateada, línea a línea,
//! para que el agente vea qué cambiaría sin abrir la copia.
//!
//! Se marca con `- ` lo que se quita, con `+ ` lo que se pone y con dos espacios lo que sigue igual,
//! y solo se enseñan las líneas iguales que rodean a un cambio. Los saltos de línea `\r\n` cuentan
//! como `\n`.

/// Cuántas líneas iguales se enseñan antes y después de cada cambio.
pub const LINEAS_DE_CONTEXTO: usize = 2;

/// Lo que se pone donde se saltan líneas iguales.
const SALTO: &str = "…";

/// En qué quedó la comparación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diferencias {
    Iguales,
    Cambios(String),
    /// Los dos textos no caben en la tabla de comparación: pasan de `tope_de_celdas`.
    DemasiadoGrandes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paso<'a> {
    Igual(&'a str),
    Quita(&'a str),
    Pone(&'a str),
}

/// Compara `antes` con `despues`. `tope_de_celdas` acota la tabla de la subsecuencia común más
/// larga, que crece con el producto de las líneas distintas de cada lado.
pub fn diferencias(antes: &str, despues: &str, tope_de_celdas: usize) -> Diferencias {
    let a: Vec<&str> = antes.lines().collect();
    let b: Vec<&str> = despues.lines().collect();
    let comun_delante = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let comun_detras = a[comun_delante..]
        .iter()
        .rev()
        .zip(b[comun_delante..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let medio_a = &a[comun_delante..a.len() - comun_detras];
    let medio_b = &b[comun_delante..b.len() - comun_detras];
    if medio_a.is_empty() && medio_b.is_empty() {
        return Diferencias::Iguales;
    }
    let celdas = (medio_a.len() + 1).saturating_mul(medio_b.len() + 1);
    if celdas > tope_de_celdas {
        return Diferencias::DemasiadoGrandes;
    }
    let pasos: Vec<Paso> = a[..comun_delante]
        .iter()
        .map(|l| Paso::Igual(l))
        .chain(pasos_del_medio(medio_a, medio_b))
        .chain(a[a.len() - comun_detras..].iter().map(|l| Paso::Igual(l)))
        .collect();
    Diferencias::Cambios(componer(&pasos))
}

/// Los pasos que llevan de `a` a `b`, por la subsecuencia común más larga.
fn pasos_del_medio<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<Paso<'a>> {
    let ancho = b.len() + 1;
    // largo[i * ancho + j]: la subsecuencia común más larga de a[i..] y b[j..].
    let mut largo = vec![0u32; (a.len() + 1) * ancho];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            largo[i * ancho + j] = if a[i] == b[j] {
                largo[(i + 1) * ancho + j + 1] + 1
            } else {
                largo[(i + 1) * ancho + j].max(largo[i * ancho + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut pasos = Vec::new();
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            pasos.push(Paso::Igual(a[i]));
            i += 1;
            j += 1;
        } else if i < a.len()
            && (j == b.len() || largo[(i + 1) * ancho + j] >= largo[i * ancho + j + 1])
        {
            // A igualdad, lo que se quita va antes de lo que se pone.
            pasos.push(Paso::Quita(a[i]));
            i += 1;
        } else {
            pasos.push(Paso::Pone(b[j]));
            j += 1;
        }
    }
    pasos
}

/// El texto de los pasos: los cambios, con sus líneas de contexto, y `…` donde se saltan otras.
fn componer(pasos: &[Paso]) -> String {
    let cambios: Vec<usize> = pasos
        .iter()
        .enumerate()
        .filter(|(_, paso)| !matches!(paso, Paso::Igual(_)))
        .map(|(i, _)| i)
        .collect();
    let cerca = |i: usize| {
        cambios
            .iter()
            .any(|&c| i + LINEAS_DE_CONTEXTO >= c && i <= c + LINEAS_DE_CONTEXTO)
    };
    let mut lineas = Vec::new();
    let mut saltado = false;
    for (i, paso) in pasos.iter().enumerate() {
        if !cerca(i) {
            saltado = true;
            continue;
        }
        if saltado && !lineas.is_empty() {
            lineas.push(SALTO.to_string());
        }
        saltado = false;
        lineas.push(match paso {
            Paso::Igual(l) => format!("  {l}"),
            Paso::Quita(l) => format!("- {l}"),
            Paso::Pone(l) => format!("+ {l}"),
        });
    }
    if saltado {
        lineas.push(SALTO.to_string());
    }
    if pasos.first().is_some_and(|_| !cerca(0)) {
        lineas.insert(0, SALTO.to_string());
    }
    lineas.join("\n")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const TOPE: usize = 1_000_000;

    fn cambios(antes: &str, despues: &str) -> String {
        match diferencias(antes, despues, TOPE) {
            Diferencias::Cambios(texto) => texto,
            otro => panic!("se esperaban cambios: {otro:?}"),
        }
    }

    #[test]
    fn dos_textos_iguales_no_tienen_cambios() {
        assert_eq!(
            diferencias("a\nb\n", "a\r\nb\r\n", TOPE),
            Diferencias::Iguales
        );
        assert_eq!(diferencias("", "", TOPE), Diferencias::Iguales);
    }

    #[test]
    fn se_marca_lo_que_se_quita_y_lo_que_se_pone() {
        assert_eq!(cambios("a\nb\nc\n", "a\nB\nc\n"), "  a\n- b\n+ B\n  c");
        assert_eq!(cambios("", "uno\n"), "+ uno");
        assert_eq!(cambios("uno\n", ""), "- uno");
    }

    /// Solo se enseñan dos líneas iguales a cada lado; las demás, con «…».
    #[test]
    fn lo_igual_lejos_de_un_cambio_se_salta() {
        let antes = "1\n2\n3\n4\n5\n6\n7\n8\n9\n";
        let despues = "1\n2\n3\n4\nCINCO\n6\n7\n8\n9\n";
        assert_eq!(
            cambios(antes, despues),
            "…\n  3\n  4\n- 5\n+ CINCO\n  6\n  7\n…"
        );
    }

    /// Dos cambios separados por más del contexto van en dos trozos.
    #[test]
    fn dos_cambios_lejanos_van_separados() {
        let antes = "a\n1\n2\n3\n4\n5\nb\n";
        let despues = "A\n1\n2\n3\n4\n5\nB\n";
        assert_eq!(
            cambios(antes, despues),
            "- a\n+ A\n  1\n  2\n…\n  4\n  5\n- b\n+ B"
        );
    }

    /// Lo que se intercala no cuenta como cambiar todo lo demás.
    #[test]
    fn una_linea_intercalada_es_un_solo_cambio() {
        assert_eq!(
            cambios("a\nb\nc\nd\n", "a\nb\nX\nc\nd\n"),
            "  a\n  b\n+ X\n  c\n  d"
        );
        assert_eq!(cambios("x\na\ny\n", "a\nx\ny\n"), "- x\n  a\n+ x\n  y");
    }

    #[test]
    fn por_encima_del_tope_no_se_compara() {
        // Lo común de los extremos no cuenta para el tope.
        let comun = "igual\n".repeat(1000);
        assert!(matches!(
            diferencias(&format!("{comun}a\n"), &format!("{comun}b\n"), 4),
            Diferencias::Cambios(_)
        ));
        assert!(matches!(
            diferencias(&format!("a\n{comun}"), &format!("b\n{comun}"), 4),
            Diferencias::Cambios(_)
        ));
        assert_eq!(
            diferencias("a\nb\n", "c\nd\n", 8),
            Diferencias::DemasiadoGrandes
        );
        assert!(matches!(
            diferencias("a\nb\n", "c\nd\n", 9),
            Diferencias::Cambios(_)
        ));
    }
}
