//! Lógica de recorte seguro UTF-8 y preparación de texto para lectura.
//!
//! Garantiza que una lectura recortada nunca rompa un carácter multibyte y que el modelo
//! sea advertido explícitamente de que está viendo un fragmento del fichero.

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

/// Deja un fichero en condiciones de entrar en la conversación: lo recorta al tope indicado
/// y, si lo recortó, le dice al modelo cuánto ocupa de verdad, para que sepa que está viendo
/// un trozo.
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
pub(super) fn preparar_lectura(ruta: &str, contenido: &str, tope: usize) -> String {
    let (trozo, recortado) = recortar_por_caracter(contenido, tope);
    let mut salida = trozo.to_string();
    if recortado {
        salida.push_str(&format!(
            "\n\n[Aviso del arnés: «{ruta}» ocupa {} bytes y solo se te muestran los primeros \
             {tope}. NO has visto el resto del fichero; no supongas lo que dice.]",
            contenido.len()
        ));
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
        let corto = preparar_lectura("src/main.rs", "fn main() {}", 100);
        assert_eq!(corto, "fn main() {}");

        let largo = preparar_lectura("src/largo.rs", "abcdefghij", 5);
        assert!(largo.starts_with("abcde"));
        assert!(largo.contains(
            "[Aviso del arnés: «src/largo.rs» ocupa 10 bytes y solo se te muestran los primeros 5."
        ));
    }
}
