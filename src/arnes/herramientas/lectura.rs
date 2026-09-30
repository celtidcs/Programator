//! Lógica de recorte seguro UTF-8, acotación por líneas y preparación de texto para lectura.
//!
//! Garantiza que una lectura recortada nunca rompa un carácter multibyte, que los saltos
//! de línea (LF y CRLF) se preserven intactos para no inducir diferencias fantasma en Windows,
//! y que el modelo sea advertido explícitamente cuando visualiza un fragmento acotado o recortado.

/// Recorta `texto` a lo sumo a `tope` bytes sin partir un carácter multibyte por la mitad: baja
/// hasta el límite de carácter válido más cercano. Devuelve también si hubo recorte.
pub(super) fn recortar_por_caracter(texto: &str, tope: usize) -> (&str, bool) {
    if texto.len() <= tope {
        return (texto, false);
    }
    let mut corte = tope;
    while corte > 0 && !texto.is_char_boundary(corte) {
        corte -= 1;
    }
    (&texto[..corte], true)
}

/// Identifica los intervalos de bytes de cada línea (1-indexada) dentro de `contenido`.
///
/// Cada elemento es `(inicio_byte, fin_byte)`, incluyendo el salto de línea (`\n` o `\r\n`)
/// si la línea lo contenía. No normaliza ni modifica ningún byte del texto original.
pub(super) fn indices_de_lineas(contenido: &str) -> Vec<(usize, usize)> {
    if contenido.is_empty() {
        return Vec::new();
    }
    let mut lineas = Vec::new();
    let mut inicio = 0;
    for (i, b) in contenido.bytes().enumerate() {
        if b == b'\n' {
            lineas.push((inicio, i + 1));
            inicio = i + 1;
        }
    }
    if inicio < contenido.len() {
        lineas.push((inicio, contenido.len()));
    }
    lineas
}

/// Resultado de acotar el contenido de un fichero por líneas.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct AcotacionLineas<'a> {
    pub texto: &'a str,
    pub total_lineas: usize,
    pub linea_desde: usize,
    pub linea_hasta: usize,
    pub acotado: bool,
    pub fuera_de_rango: bool,
}

/// Acota el contenido por rango de líneas (base 1) preservando exactamente los bytes y finales de línea.
pub(super) fn acotar_por_lineas(
    contenido: &str,
    desde: Option<usize>,
    hasta: Option<usize>,
) -> AcotacionLineas<'_> {
    let lineas = indices_de_lineas(contenido);
    let total_lineas = lineas.len();

    if total_lineas == 0 {
        return AcotacionLineas {
            texto: "",
            total_lineas: 0,
            linea_desde: 1,
            linea_hasta: 0,
            acotado: false,
            fuera_de_rango: desde.is_some() || hasta.is_some(),
        };
    }

    let d = desde.unwrap_or(1).max(1);
    if d > total_lineas {
        return AcotacionLineas {
            texto: "",
            total_lineas,
            linea_desde: d,
            linea_hasta: d,
            acotado: true,
            fuera_de_rango: true,
        };
    }

    let h = match hasta {
        Some(fin) => fin.min(total_lineas).max(d),
        None => total_lineas,
    };

    let idx_desde = d - 1;
    let idx_hasta = h - 1;

    let byte_inicio = lineas[idx_desde].0;
    let byte_fin = lineas[idx_hasta].1;

    let es_completo = d == 1 && h == total_lineas && desde.is_none() && hasta.is_none();

    AcotacionLineas {
        texto: &contenido[byte_inicio..byte_fin],
        total_lineas,
        linea_desde: d,
        linea_hasta: h,
        acotado: !es_completo,
        fuera_de_rango: false,
    }
}

/// Deja un fichero en condiciones de entrar en la conversación:
/// - Si se indicaron líneas (`desde_linea`, `hasta_linea`), extrae el rango indicado e informa.
/// - Lo recorta al tope indicado si excede la capacidad configurada.
/// - Si lo recortó, le dice al modelo cuánto ocupa de verdad, para que sepa que está viendo un trozo.
///
/// **El contenido va tal cual, y eso es deliberado: no lo sanees aquí.** Hubo una versión que
/// le pasaba `neutralizar_marcas` por si el fichero llevaba líneas que fingieran ser marcas
/// del arnés. Era defensa en profundidad redundante y cobraba un precio real. Redundante
/// porque la defensa contra marcas falsas ya vive en `Canal::publicar`, que es el **único**
/// punto por el que el texto del modelo entra al canal, y ahí sí es efectiva: lo que el modelo
/// lea aquí no llega a un fichero compartido sin pasar por allí. Y el precio es que
/// `neutralizar_marcas` normaliza los finales de línea (`\r\n` a `\n`) y se come el salto
/// final, porque está hecha con `lines()` y `join("\n")`. Esta herramienta existe para que el
/// modelo lea código **y lo reescriba**, en un proyecto de Windows: corromper así el fichero
/// que tiene que manipular metería diferencias fantasma de CRLF en sus propuestas. Por la
/// misma razón tampoco se usa `citar`, que le pondría un `> ` a cada línea.
pub(super) fn preparar_lectura(
    ruta: &str,
    contenido: &str,
    tope: usize,
    desde_linea: Option<usize>,
    hasta_linea: Option<usize>,
) -> String {
    let acotacion = acotar_por_lineas(contenido, desde_linea, hasta_linea);

    if acotacion.fuera_de_rango {
        return format!(
            "[Aviso del arnés: «{ruta}» tiene {} líneas; el rango solicitado (desde {}) comienza después del final del fichero.]",
            acotacion.total_lineas, acotacion.linea_desde
        );
    }

    let (trozo, recortado) = recortar_por_caracter(acotacion.texto, tope);

    let mut salida = if acotacion.acotado {
        format!(
            "[Líneas {} a {} de «{ruta}» (total {} líneas)]:\n{trozo}",
            acotacion.linea_desde, acotacion.linea_hasta, acotacion.total_lineas
        )
    } else {
        trozo.to_string()
    };

    if recortado {
        let ocupacion = acotacion.texto.len();
        if acotacion.acotado {
            salida.push_str(&format!(
                "\n\n[Aviso del arnés: el fragmento solicitado de «{ruta}» ocupa {ocupacion} bytes y solo se te muestran los primeros {tope}. NO has visto el resto; acota un rango menor con «desde_linea» y «hasta_linea».]"
            ));
        } else {
            salida.push_str(&format!(
                "\n\n[Aviso del arnés: «{ruta}» ocupa {ocupacion} bytes y solo se te muestran los primeros {tope}. NO has visto el resto del fichero; no supongas lo que dice. Puedes acotar con «desde_linea» y «hasta_linea».]"
            ));
        }
    }

    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_recorte_no_parte_un_caracter_multibyte_por_la_mitad() {
        let tope = 16_384;
        let cabida = tope / 3;
        let contenido = "€".repeat(tope);

        let (trozo, recortado) = recortar_por_caracter(&contenido, tope);
        assert!(recortado);
        assert_eq!(trozo.chars().count(), cabida);
        assert!(trozo.len() <= tope);
        assert!(trozo.chars().all(|c| c == '€'));
    }

    #[test]
    fn texto_que_no_supera_tope_no_se_recorta() {
        let texto = "código corto";
        let (trozo, recortado) = recortar_por_caracter(texto, 100);
        assert!(!recortado);
        assert_eq!(trozo, texto);
    }

    #[test]
    fn preparar_lectura_incluye_aviso_solo_si_se_recorta() {
        let corto = preparar_lectura("src/main.rs", "fn main() {}", 100, None, None);
        assert_eq!(corto, "fn main() {}");

        let largo = preparar_lectura("src/largo.rs", "abcdefghij", 5, None, None);
        assert!(largo.starts_with("abcde"));
        assert!(largo.contains(
            "[Aviso del arnés: «src/largo.rs» ocupa 10 bytes y solo se te muestran los primeros 5."
        ));
    }

    #[test]
    fn acotar_por_lineas_extrae_rango_exacto_conservando_saltos() {
        let contenido = "linea 1\nlinea 2\nlinea 3\nlinea 4\n";
        let acotado = acotar_por_lineas(contenido, Some(2), Some(3));
        assert_eq!(acotado.texto, "linea 2\nlinea 3\n");
        assert_eq!(acotado.total_lineas, 4);
        assert_eq!(acotado.linea_desde, 2);
        assert_eq!(acotado.linea_hasta, 3);
        assert!(acotado.acotado);
        assert!(!acotado.fuera_de_rango);
    }

    #[test]
    fn acotar_por_lineas_preserva_crlf_en_windows() {
        let contenido = "primera\r\nsegunda\r\ntercera\r\n";
        let acotado = acotar_por_lineas(contenido, Some(2), Some(2));
        assert_eq!(acotado.texto, "segunda\r\n");
        assert_eq!(acotado.linea_desde, 2);
        assert_eq!(acotado.linea_hasta, 2);
    }

    #[test]
    fn acotar_por_lineas_avisa_si_desde_supera_total() {
        let contenido = "uno\ndos\n";
        let acotado = acotar_por_lineas(contenido, Some(10), None);
        assert!(acotado.fuera_de_rango);
        assert_eq!(acotado.total_lineas, 2);

        let preparado = preparar_lectura("test.txt", contenido, 100, Some(10), None);
        assert!(preparado.contains("«test.txt» tiene 2 líneas; el rango solicitado (desde 10)"));
    }

    #[test]
    fn acotar_por_lineas_acota_hasta_al_total_si_se_pasa() {
        let contenido = "l1\nl2\nl3\n";
        let acotado = acotar_por_lineas(contenido, Some(2), Some(50));
        assert_eq!(acotado.texto, "l2\nl3\n");
        assert_eq!(acotado.linea_desde, 2);
        assert_eq!(acotado.linea_hasta, 3);
    }

    #[test]
    fn preparar_lectura_con_lineas_incluye_encabezado_y_aviso_si_recorta() {
        let contenido = "linea 1\nlinea 2\nlinea 3\n";
        let res = preparar_lectura("codigo.rs", contenido, 1000, Some(1), Some(2));
        assert!(
            res.starts_with("[Líneas 1 a 2 de «codigo.rs» (total 3 líneas)]:\nlinea 1\nlinea 2\n")
        );

        // Si excede el tope de bytes, avisa indicando acotar rango menor
        let res_corta = preparar_lectura("codigo.rs", contenido, 5, Some(1), Some(2));
        assert!(
            res_corta.contains("[Aviso del arnés: el fragmento solicitado de «codigo.rs» ocupa")
        );
        assert!(res_corta.contains("acota un rango menor"));
    }
}
