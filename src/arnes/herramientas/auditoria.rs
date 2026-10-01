//! Corrige, en el texto que el modelo publica, la línea citada en un hallazgo de auditoría contra
//! la línea real del fichero (INC-N08 e INC-N10 de NatureLand).
//!
//! **Lo que arregla y lo que no.** El modelo entiende mal SOLID y cuenta mal las líneas; eso es
//! comprensión suya y aquí no se toca. Lo que sí se puede hacer, sin entender nada de lo que dice
//! el hallazgo, es una búsqueda de texto: si cita un fragmento literal (`literal`) de un fichero
//! real, se puede comprobar en qué línea está **de verdad** y corregir el número si hace falta. Es
//! el mismo principio que el veredicto de una propuesta compilada: el arnés no entiende Rust, pero
//! sabe ejecutar `rustc` y decir si pasa.
//!
//! **El límite, a propósito.** Si el literal citado aparece más de una vez en el fichero, no hay
//! forma honesta de saber a cuál se refería el modelo — adivinar y presentarlo como corregido sería
//! peor que dejar la cita sin verificar, porque invitaría a confiar sin comprobar. En ese caso, y
//! cuando no hay ninguna coincidencia, el hallazgo se marca «sin verificar automáticamente» en vez
//! de tocarse.
//!
//! **Solo actúa sobre líneas con esta forma exacta**, documentada en
//! `plantillas/formato-auditoria.md`:
//! `TIPO | fichero | miembro | línea | literal | consecuencia`, con `TIPO` uno de los reconocidos.
//! Cualquier otra línea —prosa, un título, una tabla de Markdown que no venga a cuento— se deja
//! exactamente como estaba. Quien no pegue esa plantilla en el encargo no nota ninguna diferencia.

use super::super::Ambito;

/// Los tipos de hallazgo que reconoce el formato formal de auditoría.
///
/// La misma lista que ya usan los encargos de auditoría en prosa (HARDCODEO, DUPLICADO, TAMAÑO,
/// SOLID); formalizarla aquí no inventa categorías nuevas, solo les da una forma verificable.
const TIPOS_RECONOCIDOS: &[&str] = &["HARDCODEO", "DUPLICADO", "TAMAÑO", "SOLID"];

/// Un hallazgo de auditoría ya separado en sus campos, antes de verificar nada.
struct Hallazgo<'a> {
    tipo: &'a str,
    fichero: &'a str,
    miembro: &'a str,
    linea_citada: usize,
    literal: &'a str,
    consecuencia: &'a str,
}

/// Qué salió de buscar el literal de un hallazgo dentro del fichero real.
enum Verificacion {
    /// Encontrado en una única línea, con el número real (base 1).
    UnaCoincidencia(usize),
    /// No aparece en el fichero.
    SinCoincidencias,
    /// Aparece más de una vez: no hay forma honesta de saber a cuál se refería.
    VariasCoincidencias,
    /// El fichero no se pudo resolver dentro del ámbito, o no se pudo leer.
    FicheroIlegible,
}

/// Separa una línea en los seis campos del formato formal, si tiene esa forma exacta.
///
/// `None` para cualquier cosa que no encaje: exactamente seis campos separados por `|`, con el
/// primero entre los tipos reconocidos, el cuarto un entero positivo y el quinto (`literal`) no
/// vacío. Una línea de prosa normal, o un hallazgo con un literal que contenga su propio `|` (un
/// fragmento de SQL, por ejemplo), no encaja y se deja tal cual: es una limitación conocida, no un
/// fallo silencioso, porque la línea nunca se toca.
fn separar_hallazgo(linea: &str) -> Option<Hallazgo<'_>> {
    let campos: Vec<&str> = linea.split('|').map(str::trim).collect();
    let [tipo, fichero, miembro, linea_citada, literal, consecuencia] = campos[..] else {
        return None;
    };
    if !TIPOS_RECONOCIDOS.contains(&tipo) || fichero.is_empty() || literal.is_empty() {
        return None;
    }
    let linea_citada = linea_citada.parse::<usize>().ok()?;
    Some(Hallazgo {
        tipo,
        fichero,
        miembro,
        linea_citada,
        literal,
        consecuencia,
    })
}

/// Busca `literal` como subcadena exacta en cada línea de `fichero`, resuelto dentro de `ambito`.
fn verificar_literal(ambito: &Ambito, fichero: &str, literal: &str) -> Verificacion {
    let Ok(resuelta) = ambito.resolver(fichero) else {
        return Verificacion::FicheroIlegible;
    };
    let Ok(contenido) = std::fs::read_to_string(&resuelta) else {
        return Verificacion::FicheroIlegible;
    };

    let coincidencias: Vec<usize> = contenido
        .lines()
        .enumerate()
        .filter(|(_, linea)| linea.contains(literal))
        .map(|(indice, _)| indice + 1)
        .collect();

    match coincidencias.len() {
        0 => Verificacion::SinCoincidencias,
        1 => Verificacion::UnaCoincidencia(coincidencias[0]),
        _ => Verificacion::VariasCoincidencias,
    }
}

/// Reconstruye la línea del hallazgo con el campo de línea corregido o anotado, según lo que diga
/// la verificación. Los demás campos se reescriben tal cual, salvo recortar los espacios que
/// `separar_hallazgo` ya quitó al leerlos.
fn reescribir_linea(hallazgo: &Hallazgo, verificacion: &Verificacion) -> String {
    let campo_linea = match verificacion {
        Verificacion::UnaCoincidencia(real) if *real == hallazgo.linea_citada => {
            hallazgo.linea_citada.to_string()
        }
        Verificacion::UnaCoincidencia(real) => {
            format!(
                "{real} (corregida; el modelo dijo {})",
                hallazgo.linea_citada
            )
        }
        Verificacion::SinCoincidencias | Verificacion::VariasCoincidencias => {
            format!("{} ⚠️ sin verificar automáticamente", hallazgo.linea_citada)
        }
        Verificacion::FicheroIlegible => hallazgo.linea_citada.to_string(),
    };
    format!(
        "{} | {} | {} | {campo_linea} | {} | {}",
        hallazgo.tipo, hallazgo.fichero, hallazgo.miembro, hallazgo.literal, hallazgo.consecuencia
    )
}

/// Recorre el texto que el modelo quiere publicar y corrige la línea citada de cada hallazgo de
/// auditoría que encuentre, dejando el resto exactamente igual.
///
/// Función de entrada de este módulo: es lo único que necesita conocer `Repertorio::publicar`.
pub fn corregir_lineas_de_auditoria(texto: &str, ambito: &Ambito) -> String {
    let terminaba_en_salto = texto.ends_with('\n');
    let reescrito = texto
        .lines()
        .map(|linea| match separar_hallazgo(linea) {
            Some(hallazgo) => {
                let verificacion = verificar_literal(ambito, hallazgo.fichero, hallazgo.literal);
                reescribir_linea(&hallazgo, &verificacion)
            }
            None => linea.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");

    if terminaba_en_salto {
        format!("{reescrito}\n")
    } else {
        reescrito
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::Path;

    fn ambito_con(ficheros: &[(&str, &str)]) -> (tempfile::TempDir, Ambito) {
        let dir = tempfile::tempdir().unwrap();
        for (ruta, contenido) in ficheros {
            let completa = dir.path().join(ruta);
            if let Some(padre) = completa.parent() {
                std::fs::create_dir_all(padre).unwrap();
            }
            std::fs::write(completa, contenido).unwrap();
        }
        let ambito = Ambito::nuevo(dir.path()).unwrap();
        (dir, ambito)
    }

    #[test]
    fn una_linea_ajena_al_formato_no_se_toca() {
        let (_dir, ambito) = ambito_con(&[]);
        let texto = "Esto es un párrafo normal sin ningún hallazgo.";

        assert_eq!(corregir_lineas_de_auditoria(texto, &ambito), texto);
    }

    #[test]
    fn una_linea_con_tipo_no_reconocido_no_se_toca() {
        let (_dir, ambito) = ambito_con(&[("f.cs", "línea uno\n")]);
        let texto = "COMENTARIO | f.cs | M | 1 | uno | nada";

        assert_eq!(corregir_lineas_de_auditoria(texto, &ambito), texto);
    }

    #[test]
    fn un_hallazgo_con_la_linea_correcta_no_cambia() {
        let (_dir, ambito) = ambito_con(&[("f.cs", "a\nb\nconst X = 380f;\n")]);
        let texto = "HARDCODEO | f.cs | Metodo | 3 | 380f | darle nombre";

        assert_eq!(corregir_lineas_de_auditoria(texto, &ambito), texto);
    }

    #[test]
    fn un_hallazgo_con_la_linea_equivocada_se_corrige_y_lo_dice() {
        let (_dir, ambito) = ambito_con(&[("f.cs", "a\nb\nconst X = 380f;\n")]);
        let texto = "HARDCODEO | f.cs | Metodo | 200 | 380f | darle nombre";

        let corregido = corregir_lineas_de_auditoria(texto, &ambito);

        assert!(corregido.contains("| 3 (corregida; el modelo dijo 200) |"));
        assert!(corregido.contains("380f"));
        assert!(corregido.contains("darle nombre"));
    }

    #[test]
    fn un_literal_que_aparece_dos_veces_no_se_corrige_y_se_marca() {
        let (_dir, ambito) = ambito_con(&[("f.cs", "x(0.6f);\ny(0.6f);\n")]);
        let texto = "HARDCODEO | f.cs | Metodo | 1 | 0.6f | darle nombre";

        let corregido = corregir_lineas_de_auditoria(texto, &ambito);

        assert!(corregido.contains("| 1 ⚠️ sin verificar automáticamente |"));
    }

    #[test]
    fn un_literal_que_no_aparece_no_se_corrige_y_se_marca() {
        let (_dir, ambito) = ambito_con(&[("f.cs", "a\nb\nc\n")]);
        let texto = "HARDCODEO | f.cs | Metodo | 1 | 999f | darle nombre";

        let corregido = corregir_lineas_de_auditoria(texto, &ambito);

        assert!(corregido.contains("⚠️ sin verificar automáticamente"));
    }

    #[test]
    fn un_fichero_que_no_existe_no_se_corrige_y_no_rompe_nada() {
        let (_dir, ambito) = ambito_con(&[]);
        let texto = "HARDCODEO | no-existe.cs | Metodo | 1 | 380f | darle nombre";

        let corregido = corregir_lineas_de_auditoria(texto, &ambito);

        assert!(corregido.contains("| 1 | 380f | darle nombre"));
    }

    #[test]
    fn un_fichero_fuera_del_ambito_no_se_corrige_y_no_rompe_nada() {
        let (_dir, ambito) = ambito_con(&[]);
        let texto = "HARDCODEO | ../fuera.cs | Metodo | 1 | 380f | darle nombre";

        let corregido = corregir_lineas_de_auditoria(texto, &ambito);

        assert!(corregido.contains("| 1 | 380f | darle nombre"));
    }

    #[test]
    fn varios_hallazgos_y_prosa_mezclados_cada_uno_se_trata_por_separado() {
        let (_dir, ambito) = ambito_con(&[("f.cs", "a\nconst X = 380f;\n")]);
        let texto = "Auditoría del fichero.\n\
                      HARDCODEO | f.cs | M1 | 5 | 380f | darle nombre\n\
                      SOLID | f.cs | Clase | 1 | N/A | separar responsabilidades\n\
                      Fin del informe.";

        let corregido = corregir_lineas_de_auditoria(texto, &ambito);
        let lineas: Vec<&str> = corregido.lines().collect();

        assert_eq!(lineas[0], "Auditoría del fichero.");
        assert!(lineas[1].contains("| 2 (corregida; el modelo dijo 5) |"));
        assert_eq!(lineas[3], "Fin del informe.");
    }

    #[test]
    fn preserva_si_el_texto_terminaba_o_no_en_salto_de_linea() {
        let (_dir, ambito) = ambito_con(&[]);

        assert!(corregir_lineas_de_auditoria("sin salto", &ambito) == "sin salto");
        assert!(corregir_lineas_de_auditoria("con salto\n", &ambito) == "con salto\n");
    }

    #[test]
    fn un_literal_con_su_propia_barra_no_encaja_y_la_linea_no_se_toca() {
        // «SELECT a | b» tiene siete campos al partir por «|», no seis: no es un fallo oculto,
        // es la limitación documentada en la cabecera del módulo.
        let (_dir, ambito) = ambito_con(&[("f.sql", "SELECT a | b FROM t;\n")]);
        let texto = "HARDCODEO | f.sql | Consulta | 1 | SELECT a | b | darle nombre";

        assert_eq!(corregir_lineas_de_auditoria(texto, &ambito), texto);
    }

    #[test]
    fn la_ruta_del_fichero_se_resuelve_relativa_a_la_carpeta_de_trabajo() {
        let (dir, ambito) = ambito_con(&[("sub/f.cs", "const Y = 42;\n")]);
        let texto = "HARDCODEO | sub/f.cs | M | 9 | 42 | darle nombre";

        let corregido = corregir_lineas_de_auditoria(texto, &ambito);

        assert!(corregido.contains("| 1 (corregida; el modelo dijo 9) |"));
        assert!(Path::new(dir.path()).join("sub/f.cs").exists());
    }
}
