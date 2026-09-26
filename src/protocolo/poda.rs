//! Poda de buzones.
//!
//! Programator **nunca edita un buzón ajeno**: produce una propuesta que el dueño revisa y aplica.
//! La regla dura es *mover, nunca reescribir*: un bloque se archiva tal cual o se queda donde está.
//! `verificar_integridad` lo comprueba byte a byte antes de que nada llegue al disco.

use crate::error::{Error, Resultado};

/// Un fragmento indivisible del buzón, tal y como lo escribió su dueño.
#[derive(Debug, Clone)]
pub struct Bloque {
    pub texto: String,
}

/// Decisión del modelo sobre un bloque. Solo hay dos opciones a propósito: ante la duda, `Vivo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clasificacion {
    Vivo,
    Superado,
}

/// Resultado de una poda: qué se queda y qué se retira.
#[derive(Debug, Clone)]
pub struct Propuesta {
    pub podado: String,
    pub archivo: String,
}

/// El separador es una línea que dice exactamente `---`. Se compara la línea entera en vez de
/// buscar la cadena `"\n---\n"` dentro del texto: esa búsqueda literal no encuentra ni una sola
/// coincidencia en un fichero con finales de línea de Windows (`"\r\n---\r\n"`), y este es un
/// proyecto de Windows. Un buzón de tres secciones salía entonces como un bloque único y la poda
/// solo podía ser todo-o-nada.
const SEPARADOR_LINEA: &str = "---";

/// Una línea del buzón con lo que hace falta para cortar por ella sin perder un byte.
struct Linea<'a> {
    /// El texto de la línea, ya sin su terminador.
    texto: &'a str,
    /// Índice del byte siguiente a la línea **con** su terminador. Cortar aquí, y no en el final
    /// del texto, es lo que mantiene la reconstrucción byte a byte con `\n` y con `\r\n`.
    fin: usize,
    /// ¿La línea termina en un salto de línea propio? La última línea de un fichero que no acaba
    /// en salto no lo tiene, y sin él no puede cerrar un bloque: el separador de siempre exigía un
    /// salto por detrás.
    tiene_terminador: bool,
}

/// Parte el contenido en líneas conservando la posición de su terminador, sea `\n`, `\r\n` o
/// ninguno al final del fichero.
fn dividir_en_lineas(contenido: &str) -> Vec<Linea<'_>> {
    let mut lineas = Vec::new();
    let mut inicio = 0;

    while inicio < contenido.len() {
        let fin = match contenido[inicio..].find('\n') {
            Some(desplazamiento) => inicio + desplazamiento + 1,
            None => contenido.len(),
        };

        let mut fin_texto = fin;
        if contenido[inicio..fin_texto].ends_with('\n') {
            fin_texto -= 1;
            if contenido[inicio..fin_texto].ends_with('\r') {
                fin_texto -= 1;
            }
        }

        lineas.push(Linea {
            texto: &contenido[inicio..fin_texto],
            fin,
            tiene_terminador: fin > fin_texto,
        });
        inicio = fin;
    }

    lineas
}

/// Cuenta los backticks al inicio de una línea recortada, o devuelve 0 si no hay.
fn contar_backticks_apertura(linea: &str) -> usize {
    let trimmed = linea.trim();
    if !trimmed.starts_with('`') {
        return 0;
    }
    trimmed.chars().take_while(|c| *c == '`').count()
}

/// Devuelve el número de backticks si la línea es solo backticks (cierre), 0 si no.
fn contar_backticks_cierre(linea: &str) -> usize {
    let trimmed = linea.trim();
    if trimmed.is_empty() || !trimmed.starts_with('`') {
        return 0;
    }
    // Si toda la línea es solo backticks, es un cierre potencial
    if trimmed.chars().all(|c| c == '`') {
        trimmed.len()
    } else {
        0
    }
}

/// Parte el buzón por sus separadores, ignorando los que aparecen dentro de vallas de código.
///
/// La concatenación de todos los bloques devuelve el original byte a byte, con finales de línea
/// `\n` y con `\r\n`. Esa propiedad es la que hace comprobable la integridad de la poda, y nada
/// aquí puede romperla: el corte se hace siempre justo detrás del terminador de la línea
/// separadora, sea cual sea ese terminador.
///
/// Un `---` separa si y solo si: es la línea entera, no es la primera del fichero (el separador
/// de siempre exigía un salto de línea por delante), tiene terminador propio y **no cae dentro de
/// una valla de código abierta**. La línea que *cierra* la valla ya está fuera a estos efectos:
/// cerrar un bloque de código justo antes de un `---` es de lo más común en un buzón real, y antes
/// ese `---` no separaba.
pub fn trocear(contenido: &str) -> Vec<Bloque> {
    let lineas = dividir_en_lineas(contenido);

    let mut bloques = Vec::new();
    let mut inicio_bloque = 0usize;
    let mut en_valla = false;
    let mut backticks_apertura = 0usize;

    for (indice, linea) in lineas.iter().enumerate() {
        // Estado de valla **antes** de procesar esta línea: la que abre la valla queda fuera, y la
        // que la cierra queda dentro. El `---` que se juzga es el de esta misma línea, no el byte
        // del salto anterior, así que la valla se respeta línea a línea sin enmascarar nada.
        let dentro_de_valla = en_valla;

        if en_valla {
            // Cierre: línea de solos backticks, al menos tantos como los de la apertura.
            let backticks_cierre = contar_backticks_cierre(linea.texto);
            if backticks_cierre > 0 && backticks_cierre >= backticks_apertura {
                en_valla = false;
            }
        } else {
            // Apertura: línea que empieza por backticks (con o sin etiqueta de lenguaje).
            let backticks = contar_backticks_apertura(linea.texto);
            if backticks >= 3 {
                en_valla = true;
                backticks_apertura = backticks;
            }
        }

        let es_separador = !dentro_de_valla
            && indice > 0
            && linea.tiene_terminador
            && linea.texto == SEPARADOR_LINEA;

        if es_separador {
            bloques.push(Bloque {
                texto: contenido[inicio_bloque..linea.fin].to_string(),
            });
            inicio_bloque = linea.fin;
        }
    }

    if inicio_bloque < contenido.len() {
        bloques.push(Bloque {
            texto: contenido[inicio_bloque..].to_string(),
        });
    }

    bloques
}

/// Reparte los bloques entre la versión podada y el material archivado.
pub fn componer(bloques: &[Bloque], clases: &[Clasificacion]) -> Resultado<Propuesta> {
    if bloques.len() != clases.len() {
        return Err(Error::PodaInvalida(format!(
            "hay {} bloques y {} clasificaciones: el modelo no clasificó todo",
            bloques.len(),
            clases.len()
        )));
    }

    let mut podado = String::new();
    let mut archivo = String::new();

    for (bloque, clase) in bloques.iter().zip(clases) {
        match clase {
            Clasificacion::Vivo => podado.push_str(&bloque.texto),
            Clasificacion::Superado => archivo.push_str(&bloque.texto),
        }
    }

    Ok(Propuesta { podado, archivo })
}

/// Verifica que la propuesta es una partición válida del original.
///
/// Es la barrera contra la invención: el modelo no puede reescribir ni duplicar contenido, ni
/// reinventar texto. Comprueba:
/// 1. Recuento: el número de bloques poda + archivo == bloques original
/// 2. Contenido: multiconjunto de bloques idéntico (detecta pérdidas e invenciones)
/// 3. Orden: cada sección mantiene el orden original de sus bloques
///
/// Si el modelo reescribió una sola palabra, la propuesta se descarta sin llegar al disco.
pub fn verificar_integridad(original: &str, propuesta: &Propuesta) -> Resultado<()> {
    let bloques_original = trocear(original);
    let bloques_podado = trocear(&propuesta.podado);
    let bloques_archivo = trocear(&propuesta.archivo);

    // 1. Recuento: mismo número total de bloques
    if bloques_podado.len() + bloques_archivo.len() != bloques_original.len() {
        return Err(Error::PodaInvalida(format!(
            "recuento de bloques: {} original, {} podado + {} archivo = {} total",
            bloques_original.len(),
            bloques_podado.len(),
            bloques_archivo.len(),
            bloques_podado.len() + bloques_archivo.len()
        )));
    }

    // 2. Igualdad de multiconjuntos: mismo contenido en ambas partes
    let mut textos_original: Vec<String> =
        bloques_original.iter().map(|b| b.texto.clone()).collect();
    let mut textos_propuesta: Vec<String> = bloques_podado
        .iter()
        .chain(bloques_archivo.iter())
        .map(|b| b.texto.clone())
        .collect();

    textos_original.sort();
    textos_propuesta.sort();

    if textos_original != textos_propuesta {
        // Encontrar qué está diferente para mensajes informativos
        let perdidos: Vec<_> = textos_original
            .iter()
            .filter(|t| !textos_propuesta.contains(t))
            .collect();
        let inventados: Vec<_> = textos_propuesta
            .iter()
            .filter(|t| !textos_original.contains(t))
            .collect();

        let mut msg = String::new();
        if !perdidos.is_empty() {
            msg.push_str(&format!("perdidos: {}", perdidos.len()));
            if let Some(first) = perdidos.first() {
                let muestra: String = first.chars().take(60).collect();
                msg.push_str(&format!("; primer perdido: «{}»", muestra.trim()));
            }
        }
        if !inventados.is_empty() {
            if !msg.is_empty() {
                msg.push_str("; ");
            }
            msg.push_str(&format!("inventados: {}", inventados.len()));
            if let Some(first) = inventados.first() {
                let muestra: String = first.chars().take(60).collect();
                msg.push_str(&format!("; primer inventado: «{}»", muestra.trim()));
            }
        }

        return Err(Error::PodaInvalida(format!(
            "contenido modificado: {}",
            msg
        )));
    }

    // 3. Orden preservado: cada sección es una subsecuencia del original
    fn es_subsecuencia(subseq: &[Bloque], seq: &[Bloque]) -> bool {
        let mut sub_idx = 0;
        for bloque in seq {
            if sub_idx < subseq.len() && bloque.texto == subseq[sub_idx].texto {
                sub_idx += 1;
            }
        }
        sub_idx == subseq.len()
    }

    if !es_subsecuencia(&bloques_podado, &bloques_original) {
        return Err(Error::PodaInvalida(
            "bloques de la versión podada no mantienen su orden original".to_string(),
        ));
    }

    if !es_subsecuencia(&bloques_archivo, &bloques_original) {
        return Err(Error::PodaInvalida(
            "bloques del archivo no mantienen su orden original".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const BUZON: &str = "\
# Claude

**LATIDO:** 10:00 — antiguo

---

Acuerdo cerrado sobre el formato.

---

**LATIDO:** 14:00 — vigente

---

Tarea en curso sin terminar.
";

    #[test]
    fn trocea_el_buzon_por_separadores() {
        let bloques = trocear(BUZON);

        assert!(
            bloques.len() >= 4,
            "se esperaban al menos cuatro bloques y hubo {}",
            bloques.len()
        );
    }

    #[test]
    fn el_troceado_conserva_todo_el_contenido() {
        let bloques = trocear(BUZON);

        let recompuesto: String = bloques.iter().map(|b| b.texto.as_str()).collect();
        assert_eq!(
            recompuesto, BUZON,
            "trocear no puede perder ni añadir un byte"
        );
    }

    #[test]
    fn componer_reparte_cada_bloque_entre_podado_y_archivo() {
        let bloques = trocear(BUZON);
        let clases: Vec<Clasificacion> = bloques
            .iter()
            .map(|b| {
                if b.texto.contains("Acuerdo cerrado") {
                    Clasificacion::Superado
                } else {
                    Clasificacion::Vivo
                }
            })
            .collect();

        let propuesta = componer(&bloques, &clases).unwrap();

        assert!(propuesta.podado.contains("Tarea en curso"));
        assert!(!propuesta.podado.contains("Acuerdo cerrado"));
        assert!(propuesta.archivo.contains("Acuerdo cerrado"));
    }

    #[test]
    fn la_integridad_pasa_cuando_todo_se_movio_sin_reescribir() {
        let bloques = trocear(BUZON);
        let clases = vec![Clasificacion::Vivo; bloques.len()];
        let propuesta = componer(&bloques, &clases).unwrap();

        verificar_integridad(BUZON, &propuesta).unwrap();
    }

    #[test]
    fn la_integridad_descarta_una_propuesta_con_texto_reescrito() {
        let propuesta = Propuesta {
            podado: "Tarea en curso sin terminar, pero con mis palabras.\n".to_string(),
            archivo: String::new(),
        };

        let fallo = verificar_integridad(BUZON, &propuesta).unwrap_err();

        assert!(matches!(fallo, crate::error::Error::PodaInvalida(_)));
    }

    #[test]
    fn la_integridad_descarta_una_propuesta_que_pierde_un_bloque() {
        let bloques = trocear(BUZON);
        let clases = vec![Clasificacion::Vivo; bloques.len()];
        let mut propuesta = componer(&bloques, &clases).unwrap();
        propuesta.podado = propuesta
            .podado
            .replace("Acuerdo cerrado sobre el formato.", "");

        let fallo = verificar_integridad(BUZON, &propuesta).unwrap_err();

        assert!(matches!(fallo, crate::error::Error::PodaInvalida(_)));
    }

    #[test]
    fn componer_rechaza_un_numero_de_clasificaciones_que_no_cuadra() {
        let bloques = trocear(BUZON);

        let fallo = componer(&bloques, &[Clasificacion::Vivo]).unwrap_err();

        assert!(matches!(fallo, crate::error::Error::PodaInvalida(_)));
    }

    // ============== PRUEBAS NUEVAS OBLIGATORIAS (Ronda de arreglo 1) ==============

    #[test]
    fn ataque_1_bloque_duplicado_perdido() {
        // Original con dos bloques idénticos, propuesta que conserva solo uno
        let original = "\
Mensaje A

---

Mensaje B

---

Mensaje A
";

        let bloques = trocear(original);
        assert_eq!(bloques.len(), 3, "debe haber 3 bloques con dos copias de A");

        // Atacante: conserva solo una copia de "Mensaje A\n"
        let propuesta = Propuesta {
            podado: "\
Mensaje A

---

Mensaje B

---
"
            .to_string(),
            archivo: "Mensaje A\n".to_string(),
        };

        let fallo = verificar_integridad(original, &propuesta).unwrap_err();
        assert!(matches!(fallo, crate::error::Error::PodaInvalida(_)));
    }

    #[test]
    fn ataque_3_invension_financiada_por_duplicado_perdido() {
        // Pierde una copia de bloque duplicado + añade inventado de igual longitud
        let bloque_base = "Mensaje largo para financiar invención\n"; // 39 bytes
        let original = format!("{}---\n{}---\n{}", bloque_base, bloque_base, bloque_base);

        // Atacante: pierde un bloque, pero añade 39 bytes de inventado
        let inventado = "x".repeat(39);
        let propuesta = Propuesta {
            podado: format!("{}---\n{}---\n{}", bloque_base, bloque_base, inventado),
            archivo: String::new(),
        };

        let fallo = verificar_integridad(&original, &propuesta).unwrap_err();
        assert!(matches!(fallo, crate::error::Error::PodaInvalida(_)));
    }

    #[test]
    fn rechazo_bloques_reordenados() {
        // Mismo contenido, distinto orden dentro de podado
        let original = "A\n---\nB\n---\nC\n";

        let propuesta = Propuesta {
            podado: "B\n---\nA\n---\nC\n".to_string(),
            archivo: String::new(),
        };

        let fallo = verificar_integridad(original, &propuesta).unwrap_err();
        assert!(matches!(fallo, crate::error::Error::PodaInvalida(_)));
    }

    #[test]
    fn aceptacion_poda_legitima_con_duplicados() {
        // Poda legítima: bloques duplicados conservados, uno en cada sección
        let original = "A\n---\nB\n---\nA\n";

        let propuesta = Propuesta {
            podado: "A\n---\nB\n---\n".to_string(),
            archivo: "A\n".to_string(),
        };

        verificar_integridad(original, &propuesta).unwrap();
    }

    #[test]
    fn trocear_respeta_valla_de_codigo_con_separador() {
        // Original con valla que contiene `---`: trocear no debe partir dentro de la valla
        let original =
            "Bloque 1\n\n---\n\n```\nEsto es código con --- dentro\n```\n\n---\n\nBloque 2\n";

        let bloques = trocear(original);

        // Debe haber exactamente 3 bloques: antes valla, valla completa, después valla
        assert_eq!(
            bloques.len(),
            3,
            "trocear debe respetar vallas: esperados 3, obtenidos {}",
            bloques.len()
        );

        // Verificar que la concatenación reproduce el original byte a byte
        let recompuesto: String = bloques.iter().map(|b| b.texto.as_str()).collect();
        assert_eq!(
            recompuesto, original,
            "concatenación de bloques debe reproducir original exactamente"
        );

        // Verificar que la valla está intacta en el segundo bloque
        assert!(
            bloques[1].texto.contains("Esto es código con --- dentro"),
            "la valla debe contener el texto original"
        );
    }

    // ============== PRUEBAS NUEVAS OBLIGATORIAS (Ronda de arreglo 2) ==============

    #[test]
    fn valla_con_etiqueta_de_lenguaje_contiene_separador() {
        // Valla abierta con ```rust contiene --- dentro
        let original = "Inicio\n\n---\n\n```rust\nfn main() {\n    // --- esta es parte del código\n}\n```\n\n---\n\nFin\n";

        let bloques = trocear(original);

        // Debe haber 3 bloques, no más (no debe partir dentro de la valla)
        assert_eq!(
            bloques.len(),
            3,
            "valla con etiqueta debe ser respetada: esperados 3, obtenidos {}",
            bloques.len()
        );

        // Concatenación byte a byte
        let recompuesto: String = bloques.iter().map(|b| b.texto.as_str()).collect();
        assert_eq!(
            recompuesto, original,
            "concatenación debe reproducir original exactamente"
        );

        // La valla con etiqueta debe estar intacta
        assert!(
            bloques[1].texto.contains("```rust"),
            "valla debe contener apertura con etiqueta"
        );
        assert!(
            bloques[1].texto.contains("// --- esta es parte del código"),
            "valla debe contener el --- del código"
        );
    }

    #[test]
    fn cierre_con_mas_backticks_que_apertura() {
        // Valla abierta con ``` (3) y cerrada con ```` (4 backticks)
        // En markdown, el cierre con más backticks sigue siendo válido si son solo backticks
        let original = "Texto antes\n\n---\n\n```\nCódigo aquí\n````\n\n---\n\nTrabajo después\n";

        let bloques = trocear(original);

        // La valla abre con ``` y cierra con ```` (4 >= 3)
        // Así que hay 3 bloques: antes del ---,  la valla con código, después del ---
        assert_eq!(
            bloques.len(),
            3,
            "cierre con más backticks debe cerrarse: esperados 3, obtenidos {}",
            bloques.len()
        );

        // Concatenación byte a byte
        let recompuesto: String = bloques.iter().map(|b| b.texto.as_str()).collect();
        assert_eq!(
            recompuesto, original,
            "concatenación debe reproducir original exactamente"
        );
    }

    #[test]
    fn valla_abierta_con_cuatro_backticks_no_cierra_con_tres() {
        // Valla abierta con ```` (cuatro) y una línea de ``` (tres) dentro no cierra
        let original = "Bloque 1\n\n---\n\n````\nCódigo\n```\nSigue dentro de valla\n````\n\n---\n\nBloque 2\n";

        let bloques = trocear(original);

        // Debe haber 3 bloques: los --- deben ser respetados dentro de la valla de 4
        assert_eq!(
            bloques.len(),
            3,
            "cierre con menos backticks no cierra: esperados 3, obtenidos {}",
            bloques.len()
        );

        // Concatenación byte a byte
        let recompuesto: String = bloques.iter().map(|b| b.texto.as_str()).collect();
        assert_eq!(
            recompuesto, original,
            "concatenación debe reproducir original exactamente"
        );

        // La valla interior (3 backticks) no cierra la valla exterior (4 backticks)
        assert!(
            bloques[1].texto.contains("```\nSigue dentro de valla"),
            "los 3 backticks no cierran una valla de 4"
        );
    }

    #[test]
    fn valla_sin_cerrar_al_final() {
        // Valla sin cerrar al final del fichero
        let original = "Contenido normal\n\n---\n\n```python\nEste código nunca cierra";

        let bloques = trocear(original);

        // Debe haber 2 bloques: el primero hasta la valla, todo lo demás es segundo bloque
        assert_eq!(
            bloques.len(),
            2,
            "valla sin cerrar al final: esperados 2, obtenidos {}",
            bloques.len()
        );

        // Concatenación byte a byte
        let recompuesto: String = bloques.iter().map(|b| b.texto.as_str()).collect();
        assert_eq!(
            recompuesto, original,
            "concatenación debe reproducir original exactamente"
        );

        // El --- dentro de la valla no parte el bloque
        assert!(
            bloques[1].texto.contains("```python"),
            "valla sin cerrar debe estar en segundo bloque"
        );
    }

    // ============== PRUEBAS NUEVAS (tanda de arreglos finales) ==============

    #[test]
    fn un_separador_pegado_al_cierre_de_una_valla_si_parte() {
        // Cerrar un bloque de código justo antes del separador es de lo más común en un buzón
        // real, y antes no partía: la línea de cierre enmascaraba su propio salto de línea final,
        // que es el primer byte del separador.
        let original = "bloque a\n```\nlet x = 1;\n```\n---\nbloque b\n";

        let bloques = trocear(original);

        assert_eq!(
            bloques.len(),
            2,
            "el «---» pegado al cierre de la valla debe separar: esperados 2, obtenidos {}",
            bloques.len()
        );
        assert_eq!(bloques[0].texto, "bloque a\n```\nlet x = 1;\n```\n---\n");
        assert_eq!(bloques[1].texto, "bloque b\n");

        let recompuesto: String = bloques.iter().map(|b| b.texto.as_str()).collect();
        assert_eq!(recompuesto, original, "reconstrucción byte a byte");
    }

    #[test]
    fn un_separador_a_solas_dentro_de_una_valla_abierta_sigue_sin_partir() {
        // Lo contrario del caso anterior, y la garantía que no se puede romper al arreglarlo.
        let original = "antes\n\n---\n\n```\n---\ntodavía dentro de la valla\n---\n```\n\nfinal\n";

        let bloques = trocear(original);

        assert_eq!(
            bloques.len(),
            2,
            "los «---» de dentro de la valla no separan: esperados 2, obtenidos {}",
            bloques.len()
        );
        assert!(
            bloques[1].texto.contains("todavía dentro de la valla"),
            "la valla entera debe quedar en un solo bloque: {:?}",
            bloques[1].texto
        );

        let recompuesto: String = bloques.iter().map(|b| b.texto.as_str()).collect();
        assert_eq!(recompuesto, original, "reconstrucción byte a byte");
    }

    #[test]
    fn trocea_un_buzon_con_finales_de_linea_de_windows() {
        // Con «\r\n» no había ni una coincidencia de «\n---\n»: el buzón entero salía como un
        // bloque único y la poda solo podía ser todo-o-nada. Este es un proyecto de Windows.
        let original =
            "Sección uno\r\n\r\n---\r\n\r\nSección dos\r\n\r\n---\r\n\r\nSección tres\r\n";

        let bloques = trocear(original);

        assert_eq!(
            bloques.len(),
            3,
            "un buzón CRLF de tres secciones da tres bloques, no uno: obtenidos {}",
            bloques.len()
        );
        assert!(bloques[0].texto.ends_with("---\r\n"));
        assert!(bloques[1].texto.contains("Sección dos"));
        assert!(bloques[2].texto.contains("Sección tres"));
    }

    #[test]
    fn la_reconstruccion_es_exacta_con_los_dos_finales_de_linea() {
        // La garantía sagrada: `trocear` no puede perder ni añadir un byte, con «\n» y con
        // «\r\n». Si el cálculo de posiciones se descuadrara un byte por línea, esto lo caza.
        let con_lf = "uno\n\n---\n\ndos\n```\ncódigo\n```\n---\ntres";
        let con_crlf = "uno\r\n\r\n---\r\n\r\ndos\r\n```\r\ncódigo\r\n```\r\n---\r\ntres";

        for original in [con_lf, con_crlf] {
            let bloques = trocear(original);
            let recompuesto: String = bloques.iter().map(|b| b.texto.as_str()).collect();
            assert_eq!(
                recompuesto, original,
                "trocear no puede perder ni añadir un byte: {original:?}"
            );
            assert_eq!(
                recompuesto.len(),
                original.len(),
                "ni uno de más ni uno de menos"
            );
        }
    }

    #[test]
    fn la_integridad_se_comprueba_igual_con_finales_de_linea_de_windows() {
        let original = "Acuerdo viejo\r\n\r\n---\r\n\r\nTarea en curso\r\n";
        let bloques = trocear(original);
        let clases: Vec<Clasificacion> = bloques
            .iter()
            .map(|b| {
                if b.texto.contains("Acuerdo viejo") {
                    Clasificacion::Superado
                } else {
                    Clasificacion::Vivo
                }
            })
            .collect();

        let propuesta = componer(&bloques, &clases).unwrap();

        verificar_integridad(original, &propuesta).unwrap();
        assert_eq!(
            format!("{}{}", propuesta.archivo, propuesta.podado),
            original,
            "mover sin reescribir: los CRLF llegan intactos a las dos partes"
        );
    }

    #[test]
    fn un_separador_sin_salto_de_linea_final_no_cierra_bloque() {
        // El separador de siempre («\n---\n») exigía un salto por detrás: un «---» que sea la
        // última línea del fichero, sin terminador, no cierra nada.
        let original = "algo\n---";

        let bloques = trocear(original);

        assert_eq!(bloques.len(), 1, "sin salto final no hay separador");
        assert_eq!(bloques[0].texto, original);
    }
}
