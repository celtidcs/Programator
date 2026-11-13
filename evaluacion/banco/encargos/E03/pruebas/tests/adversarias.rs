//! Pruebas que el modelo no ve. Exigen solo lo que dice el enunciado: la firma de `expandir`, los
//! cuatro rechazos y un error que diga dónde falló. Los nombres de las variantes de `ErrorRango`
//! los elige el modelo, así que aquí solo se mira si el resultado es un error.

use banco_e03::rangos::{expandir, ErrorRango};

/// La firma exacta del enunciado. Si no coincide, esto no compila y la entrega saca un 0.
fn llamar(entrada: &str) -> Result<Vec<u32>, ErrorRango> {
    expandir(entrada)
}

#[test]
fn caso_basico_del_enunciado() {
    assert_eq!(llamar("1-3,7,10-12").ok(), Some(vec![1, 2, 3, 7, 10, 11, 12]));
}

#[test]
fn un_numero_suelto() {
    assert_eq!(llamar("7").ok(), Some(vec![7]));
}

#[test]
fn rango_de_un_solo_elemento() {
    assert_eq!(llamar("5-5").ok(), Some(vec![5]));
}

#[test]
fn rechaza_la_cadena_vacia() {
    assert!(llamar("").is_err());
}

#[test]
fn rechaza_un_rango_invertido() {
    assert!(llamar("5-2").is_err());
}

#[test]
fn rechaza_un_numero_no_valido() {
    assert!(llamar("1-3,abc").is_err());
    assert!(llamar("1-x").is_err());
}

#[test]
fn rechaza_separadores_sobrantes() {
    assert!(llamar("1,,2").is_err(), "coma doble en medio");
    assert!(llamar("1,2,").is_err(), "coma al final");
    assert!(llamar(",1").is_err(), "coma al principio");
}

#[test]
fn un_numero_que_no_cabe_en_u32_es_un_error_y_no_un_panico() {
    // 4294967296 es u32::MAX + 1. Si esto entra en pánico, la prueba falla igual que si devolviera
    // Ok: los dos son el mismo fallo de robustez.
    assert!(llamar("4294967296").is_err());
}

#[test]
fn el_error_dice_donde_fallo() {
    // El enunciado pide variantes que digan qué falló y dónde. Se acepta cualquier forma de decir
    // dónde: el fragmento que falló o una posición.
    let error = match llamar("1-3,abc,7") {
        Err(error) => error,
        Ok(valor) => panic!("aceptó una entrada con «abc»: {valor:?}"),
    };
    let texto = format!("{error:?}");
    assert!(
        texto.contains("abc") || texto.chars().any(|c| c.is_ascii_digit()),
        "el error no dice dónde falló: {texto}"
    );
}
