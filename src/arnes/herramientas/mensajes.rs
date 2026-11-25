//! Lo que el repertorio le contesta al modelo al conceder o denegar una solicitud.
//!
//! Un solo sitio para estos textos: el repertorio decide y aquí solo se redacta. El motivo de un
//! verbo que se deniega siempre, o que está pendiente, no se repite aquí: lo da su ficha
//! (`arnes::ficha`). El de un fallo de ruta lo da `error::motivo_para_el_modelo`, que además se
//! asegura de no filtrar rutas absolutas.

use crate::arnes::ficha::Ficha;
use crate::concordancia::cantidad;

/// Lo que se contesta a una solicitud que el repertorio no reconoce.
pub(super) const SOLICITUD_NO_RECONOCIDA: &str = "solicitud no reconocida";

/// Lo que se contesta cuando `publicar` queda anotado para el cierre del ciclo.
pub(super) const PUBLICACION_ANOTADA: &str = "anotado; el arnés lo publicará al cerrar el ciclo";

/// Lo que se contesta cuando la reserva queda anotada.
pub(super) const RESERVA_ANOTADA: &str =
    "reserva anotada en estado.md; se quita sola al cerrar el encargo";

/// Un verbo que no está en el repertorio, con la lista de los que sí.
pub(super) fn fuera_del_repertorio(nombre: &str, reconocidos: &[&str]) -> String {
    format!(
        "«{nombre}» no está en el repertorio. Solo puedes usar: {}",
        reconocidos.join(", ")
    )
}

/// La firma del verbo y para qué sirve cada argumento, en una frase. Sin ficha, nada: una firma
/// falsa sería peor que ninguna.
pub(super) fn recordatorio_de_firma(ficha: Option<&Ficha>) -> String {
    let Some(ficha) = ficha else {
        return String::new();
    };
    let mut texto = format!("La firma es {}.", ficha.firma());
    for argumento in ficha.obligatorios.iter().chain(ficha.opcionales) {
        texto.push_str(&format!(" «{}»: {}.", argumento.nombre, argumento.para_que));
    }
    texto
}

/// Falta un argumento obligatorio.
pub(super) fn falta_el_argumento(clave: &str, firma: &str) -> String {
    format!("falta el argumento «{clave}». {firma}")
}

/// Un argumento que tenía que ser texto.
pub(super) fn no_es_cadena(clave: &str, firma: &str) -> String {
    format!("el argumento «{clave}» debe ser una cadena de texto. {firma}")
}

/// Un argumento que tenía que ser `true` o `false`.
pub(super) fn no_es_logico(clave: &str, firma: &str) -> String {
    format!("el argumento «{clave}» debe ser true o false. {firma}")
}

/// Un argumento de línea que tenía que ser un entero positivo.
pub(super) fn no_es_entero_positivo(clave: &str, firma: &str) -> String {
    format!("el argumento «{clave}» debe ser un entero positivo. {firma}")
}

/// Un rango de líneas al revés.
pub(super) fn rango_al_reves(desde: usize, hasta: usize, firma: &str) -> String {
    format!("«desde_linea» ({desde}) no puede ser mayor que «hasta_linea» ({hasta}). {firma}")
}

/// El fichero está en el ámbito pero no se dejó leer.
pub(super) fn lectura_fallida(ruta: &str, causa: &std::io::Error) -> String {
    format!("no se pudo leer «{ruta}»: {causa}")
}

/// La carpeta está en el ámbito pero no se dejó listar.
pub(super) fn listado_fallido(ruta: &str, causa: &std::io::Error) -> String {
    format!("no se pudo listar «{ruta}»: {causa}")
}

/// Una verificación que no está en la lista blanca.
pub(super) fn fuera_de_la_lista_blanca(cual: &str, lista: &[&str]) -> String {
    format!(
        "«{cual}» no está en la lista blanca de verificaciones: {}",
        lista.join(", ")
    )
}

/// Una verificación de la lista blanca que todavía no ejecuta nadie, con el motivo de su ficha.
pub(super) fn verificacion_pendiente(cual: &str, motivo: &str) -> String {
    format!("la verificación «{cual}» {motivo}")
}

/// La línea con que la respuesta cuenta una denegación: la herramienta y la primera línea del
/// motivo, recortada como las demás líneas del canal.
pub(super) fn denegada_para_el_canal(herramienta: &str, motivo: &str) -> String {
    let primera = motivo.lines().map(str::trim).find(|l| !l.is_empty());
    let primera = crate::arnes::verificacion::en_una_linea_del_canal(primera.unwrap_or_default());
    format!("`{herramienta}`: {primera}")
}

/// Una propuesta cuyo nombre la sacaría de la carpeta de las propuestas.
pub(super) fn saldria_de_la_carpeta(nombre: &str, carpeta: &str) -> String {
    format!("«{nombre}» saldría de {carpeta}/")
}

/// No se pudo crear la carpeta de la propuesta.
pub(super) fn carpeta_no_creada(causa: &std::io::Error) -> String {
    format!("no se pudo crear el directorio: {causa}")
}

/// No se pudo escribir la propuesta.
pub(super) fn propuesta_no_escrita(causa: &dyn std::fmt::Display) -> String {
    format!("no se pudo escribir: {causa}")
}

/// La propuesta quedó escrita, con lo que dijo su comprobación.
pub(super) fn propuesta_escrita(nombre: &str, veredicto: &str) -> String {
    format!("escrita la propuesta «{nombre}» — {veredicto}")
}

/// No se pudo preparar la carpeta donde se comprueba la propuesta.
pub(super) fn sin_carpeta_de_comprobacion(causa: &std::io::Error) -> String {
    format!("no se pudo preparar un directorio de trabajo: {causa}")
}

/// Un rango de lectura que empieza después del final del fichero.
pub(super) fn rango_tras_el_final(ruta: &str, total_lineas: usize, desde: usize) -> String {
    format!(
        "[Aviso del arnés: «{ruta}» tiene {}; el rango solicitado (desde {desde}) comienza después \
         del final del fichero.]",
        cantidad(total_lineas, "línea", "líneas")
    )
}

/// Un rango de lectura que no cabe en el tope: se le dice al modelo que no lo ha visto entero.
pub(super) fn fragmento_recortado(ruta: &str, ocupacion: usize, tope: usize) -> String {
    format!(
        "\n\n[Aviso del arnés: el fragmento solicitado de «{ruta}» ocupa {ocupacion} bytes y solo \
         se te muestran los primeros {tope}. NO has visto el resto; acota un rango menor con \
         «desde_linea» y «hasta_linea».]"
    )
}

/// Un fichero que no cabe en el tope: se le dice al modelo que no lo ha visto entero.
pub(super) fn fichero_recortado(ruta: &str, ocupacion: usize, tope: usize) -> String {
    format!(
        "\n\n[Aviso del arnés: «{ruta}» ocupa {ocupacion} bytes y solo se te muestran los primeros \
         {tope}. NO has visto el resto del fichero; no supongas lo que dice. Puedes acotar con \
         «desde_linea» y «hasta_linea».]"
    )
}

/// El campo de línea de un hallazgo de auditoría que el arnés corrigió contra el fichero real.
pub(super) fn linea_corregida(real: usize, citada: usize) -> String {
    format!("{real} (corregida; el modelo dijo {citada})")
}

/// El campo de línea de un hallazgo de auditoría que el arnés no pudo comprobar.
pub(super) fn linea_sin_verificar(citada: usize) -> String {
    format!("{citada} ⚠️ sin verificar automáticamente")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La línea de una denegación lleva solo la primera línea no vacía del motivo: el resto lo
    /// recibe el modelo, no el canal.
    #[test]
    fn la_denegacion_en_el_canal_solo_lleva_la_primera_linea_del_motivo() {
        let linea = denegada_para_el_canal("leer_fichero", "\n  fuera del ámbito\nsegunda línea\n");
        assert_eq!(linea, "`leer_fichero`: fuera del ámbito");
    }

    /// Un motivo largo se recorta: la línea del canal no crece con él.
    #[test]
    fn la_denegacion_en_el_canal_recorta_un_motivo_largo() {
        let maximo = crate::arnes::verificacion::MAXIMO_EN_UNA_LINEA_DEL_CANAL;
        let linea = denegada_para_el_canal("leer_fichero", &"x".repeat(maximo * 3));
        assert!(linea.contains("[…recortado]"), "{linea}");
        assert!(linea.chars().count() < maximo + 40, "{linea}");
    }

    #[test]
    fn sin_ficha_no_se_inventa_ninguna_firma() {
        assert_eq!(recordatorio_de_firma(None), "");
        let firma = recordatorio_de_firma(crate::arnes::ficha::buscar("leer_fichero"));
        assert!(firma.starts_with("La firma es "), "{firma}");
        assert!(firma.contains("«ruta»:"), "{firma}");
    }

    /// Con una sola línea, en singular: «tiene 1 línea», no «tiene 1 líneas».
    #[test]
    fn un_fichero_de_una_linea_va_en_singular() {
        assert!(rango_tras_el_final("a.rs", 1, 5).contains("«a.rs» tiene 1 línea;"));
        assert!(rango_tras_el_final("a.rs", 3, 5).contains("«a.rs» tiene 3 líneas;"));
    }
}
