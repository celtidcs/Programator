//! Todo lo que la revisión de modelos dice por pantalla o deja escrito para el Director.

use std::path::Path;

use super::buscar::BusquedaHecha;
use super::candidatos::MotivoDeDescarte;
use super::descarga::{Aviso, Descargado};
use super::encargo::CandidatoPedido;
use super::proponer::PropuestaHecha;
use super::propuesta::{Categoria, Excluida, ParaRetirar};
use super::retirada::Retirada;
use super::DECIMALES_DE_LA_NOTA;
use crate::banco::resultados::{decimal, nombre_de_familia};
use crate::config::Config;
use crate::error::Error;
use crate::informe::en_gib;
use crate::registro::modelo::CodigoPuntuadoCon;

/// Sin la VRAM no se puede descartar nada con criterio (especificación, §3.2, punto 1).
pub const SIN_TARJETA: &str = "no se encontró la tarjeta gráfica, y sin saber su memoria no se \
                               puede decir qué modelos caben. Comprueba que el controlador está \
                               instalado con «programator --diagnostico»";

/// Las cifras del registro son de una tarjeta: sin saber cuál es esta, no se sabe cuáles valen.
pub const SIN_TARJETA_PARA_PROPONER: &str = "no se encontró la tarjeta gráfica, y las cifras del \
                                             registro valen solo para la tarjeta en que se \
                                             midieron. Comprueba que el controlador está \
                                             instalado con «programator --diagnostico»";

/// Lo que dejó `--revision-proponer`, y lo que toca después.
pub fn propuesta_hecha(hecha: &PropuestaHecha) -> String {
    let anterior = hecha.anterior.as_deref().map_or(String::new(), |nombre| {
        format!("\nLa propuesta anterior de este mes se ha guardado como «{nombre}».")
    });
    format!(
        "✅ Propuesta hecha: se quedan {}, se propone retirar {} y hay {} sin medir.\n\
         Está en «{}». La propuesta no retira nada: lo retira «--retirar-modelo» o, en la revisión \
         por el canal, «revisión: cerrar».{anterior}",
        hecha.se_quedan,
        hecha.para_retirar,
        hecha.sin_medir,
        hecha.ruta.display(),
    )
}

/// Lo que dejó `--revision-buscar`, y lo que toca después.
pub fn busqueda_hecha(hecha: &BusquedaHecha) -> String {
    let anterior = hecha.anterior.as_deref().map_or(String::new(), |nombre| {
        format!("\nLa búsqueda anterior de este mes se ha guardado como «{nombre}».")
    });
    format!(
        "✅ Búsqueda hecha: {} candidatos, {} ya descargados sin medir y {} descartados.\n\
         Está todo en «{}»: «candidatos.md» para leerlo, «candidatos.toml» para descargar y \
         «respuestas/» con lo que contestó Hugging Face.{anterior}",
        hecha.candidatos,
        hecha.ya_descargados,
        hecha.descartados,
        hecha.carpeta.display(),
    )
}

/// Por qué no se pudo hacer una orden de la revisión.
pub fn fallo(fallo: &Error) -> String {
    format!(
        "🔴 {fallo}.\nNo se ha tocado ningún modelo ni el registro. Si es la red, vuelve a \
         intentarlo; si es la configuración, revisa la sección [revision] de «programator.toml»."
    )
}

/// Una cifra de bytes para leerla, con los bytes exactos al lado.
fn bytes(cuantos: u64) -> String {
    format!("{} ({cuantos} bytes)", en_gib(cuantos))
}

/// Lo que va pasando en una descarga.
pub fn aviso_de_descarga(aviso: &Aviso) -> String {
    match aviso {
        Aviso::Reanuda { desde } => {
            format!(
                "↪️ Ya había {} bajados: se sigue desde ahí.",
                en_gib(*desde)
            )
        }
        Aviso::EmpiezaDeCero { tenia } => format!(
            "⚠️ Ya había {} bajados, pero el servidor no deja seguir desde ahí: se empieza de \
             cero.",
            en_gib(*tenia)
        ),
        Aviso::Avance { llevados, total } => {
            format!("⬇️ {} de {}", en_gib(*llevados), en_gib(*total))
        }
        Aviso::CompruebaLaHuella => "🔎 Bajado entero. Comprobando la huella SHA-256…".into(),
    }
}

/// Lo que dejó `--descargar-modelo`, y lo que toca después.
pub fn descarga_hecha(descargado: &Descargado) -> String {
    let ruta = descargado.ruta.display();
    format!(
        "✅ Descargado y comprobado: «{ruta}».\n\
         Lo siguiente es pasarle el banco: «programator --banco banco --modelo \"{ruta}\"»."
    )
}

/// Lo que dejó `--descargar-modelo` cuando el fichero ya estaba.
pub fn ya_estaba(ruta: &Path) -> String {
    format!(
        "✅ «{}» ya estaba, con la huella de la lista: no se ha bajado otra vez.",
        ruta.display()
    )
}

/// No hay ninguna búsqueda guardada de la que descargar.
pub fn sin_revision(carpeta: &Path) -> String {
    format!(
        "no hay ninguna búsqueda guardada en «{}»: primero hay que buscar con \
         «programator --revision-buscar»",
        carpeta.display()
    )
}

/// Solo se descarga lo que está en la lista de la búsqueda más reciente.
pub fn no_esta_en_la_lista(repositorio: &str, fichero: &str, lista: &Path) -> String {
    format!(
        "«{fichero}» de «{repositorio}» no está entre los candidatos de «{}». Solo se descarga \
         lo que encontró la búsqueda más reciente: mira los nombres exactos en «candidatos.md», \
         en la misma carpeta",
        lista.display()
    )
}

/// Un candidato sin huella o sin tamaño no se puede comprobar.
pub fn sin_huella_o_tamano(repositorio: &str, fichero: &str) -> String {
    format!(
        "Hugging Face no dio la huella o el tamaño de «{fichero}» de «{repositorio}», y sin ellos \
         no se puede comprobar lo que se baje. No se descarga"
    )
}

/// Ya hay un fichero con ese nombre y no es el de la lista.
pub fn ya_hay_otro(destino: &Path) -> String {
    format!(
        "ya hay un «{}» y no es el de la lista: su huella es otra. No se toca; si sobra, quítalo \
         a mano y vuelve a pedir la descarga",
        destino.display()
    )
}

/// La descarga se cortó; lo bajado se queda para seguir.
pub fn descarga_cortada(llevados: u64, total: u64, parcial: &Path) -> String {
    format!(
        "la descarga se cortó con {} de {} bajados. Lo bajado se queda en «{}»: vuelve a pedir la \
         descarga y seguirá desde ahí",
        en_gib(llevados),
        en_gib(total),
        parcial.display()
    )
}

/// El fichero bajado no es el que dijo la búsqueda; ya se ha borrado.
pub fn huella_distinta(esperada: &str, obtenida: &str) -> String {
    format!(
        "el fichero bajado no es el que encontró la búsqueda: su huella es {obtenida} y tenía que \
         ser {esperada}. Se ha borrado. Si vuelve a pasar, el repositorio ha cambiado: busca otra \
         vez con «--revision-buscar»"
    )
}

/// No hay sitio en el disco para lo que falta por bajar más el margen.
pub fn sin_espacio(necesita: u64, libre: u64, carpeta: &Path) -> String {
    format!(
        "no cabe en el disco de «{}»: hacen falta {}, contando «[revision] margen_disco_mib», y \
         hay {} libres. No se ha bajado nada",
        carpeta.display(),
        bytes(necesita),
        bytes(libre)
    )
}

/// El motivo de una retirada que la propuesta más reciente no proponía.
pub const RETIRADO_POR_ORDEN: &str = "retirado por orden del Director";

pub fn retirar_solo_un_nombre(fichero: &str) -> String {
    format!(
        "«{fichero}» no es un nombre de fichero: se retira por el nombre que tiene en la carpeta \
         de modelos, sin carpetas delante"
    )
}

pub fn retirar_solo_gguf(fichero: &str) -> String {
    format!("«{fichero}» no es un modelo: solo se retiran ficheros «.gguf»")
}

pub fn retirar_el_modelo_en_uso(fichero: &str) -> String {
    format!(
        "«{fichero}» es el modelo de «programator.toml» y no se retira. Si el Director quiere \
         retirarlo, primero hay que cambiar «[motor] modelo»"
    )
}

pub fn retirar_lo_que_no_esta(fichero: &str) -> String {
    format!("«{fichero}» no está en la carpeta de modelos: no hay nada que retirar")
}

/// El registro ya anota la retirada, pero el fichero no se dejó borrar.
pub fn retirada_sin_borrar(ruta: &Path, anotadas: usize, causa: &std::io::Error) -> String {
    format!(
        "la retirada ya consta en el registro ({anotadas} entradas), pero «{}» no se pudo borrar: \
         {causa}. Bórralo a mano cuando no lo use nada",
        ruta.display()
    )
}

/// Por qué no se pudo retirar. Sin la coletilla de [`fallo`]: si el borrado falla, el registro sí
/// se ha tocado, y el texto del error ya lo dice.
pub fn fallo_de_la_retirada(fallo: &Error) -> String {
    format!("🔴 {fallo}.")
}

/// Lo que dejó `--retirar-modelo`.
pub fn retirada_hecha(retirada: &Retirada) -> String {
    let registro = if retirada.anotadas == 0 {
        "No tenía cifras en el registro, así que no había nada que anotar.".to_string()
    } else {
        format!(
            "Anotado en el registro, en {} entradas, con sus cifras intactas: «{}».",
            retirada.anotadas, retirada.motivo
        )
    };
    format!(
        "🗑️ Retirado «{}»: se han liberado {}.\n{registro}",
        retirada.ruta.display(),
        en_gib(retirada.liberados)
    )
}

/// Cómo se llama una clase o una familia en la propuesta.
pub fn nombre_de_categoria(categoria: Categoria) -> String {
    match categoria {
        Categoria::Clase(fila) => fila.titulo().to_string(),
        Categoria::Familia(familia) => format!("Familia de {}", nombre_de_familia(familia)),
    }
}

/// Cómo se puntuó el código de una entrada del registro, para la propuesta.
pub fn forma_del_codigo(forma: Option<CodigoPuntuadoCon>) -> &'static str {
    match forma {
        Some(CodigoPuntuadoCon::PruebasOcultas) => "ejecutando las pruebas ocultas",
        Some(CodigoPuntuadoCon::Rubrica) => "leyendo el código con la rúbrica, sin ejecutarlo",
        Some(CodigoPuntuadoCon::Mixto) => "unas semillas de una forma y otras de la otra",
        Some(CodigoPuntuadoCon::SinEntregas) => {
            "sin entregar código, que vale 0 se puntúe como se puntúe"
        }
        None => "sin saberse cómo (la tanda es de antes del 13/11/2026)",
    }
}

/// Por qué no se compara una categoría que depende del código, dicho en una frase.
pub fn motivo_de_exclusion(excluida: &Excluida) -> String {
    let formas: Vec<String> = excluida
        .formas
        .iter()
        .map(|(nombre, forma)| format!("`{nombre}`, {}", forma_del_codigo(*forma)))
        .collect();
    format!(
        "«{}» se deja fuera: depende de encargos de código, y sus notas no se puntuaron igual \
         ({}). Una nota puntuada ejecutando pruebas y otra leyendo con rúbrica no son \
         comparables, ni lo es una de la que no se sabe cómo se puntuó, así que no entra en la \
         comparación ni en la decisión",
        nombre_de_categoria(excluida.categoria),
        formas.join("; ")
    )
}

/// Por qué una propuesta no propone nada, cuando lo que deja fuera la deja sin datos.
pub fn sin_datos_para_decidir() -> &'static str {
    "**No se propone ninguna sustitución ni retirada.** Con las cifras de código fuera, no queda \
     ninguna clase ni familia en la que comparar los modelos. Para poder decidir, pasa el banco a \
     los modelos de forma que su código se puntúe igual: con las pruebas ocultas encendidas y los \
     lenguajes instalados, o todos con la rúbrica."
}

/// Lo que se dice de un modelo que solo tiene notas en categorías que no se comparan.
pub fn sin_comparar(nombre: &str) -> String {
    format!(
        "- `{nombre}`: se queda sin decidir; todas sus notas son de categorías que no se \
         comparan.\n"
    )
}

/// Por qué se propone retirar un modelo, dicho en una frase. Es también el motivo que anota la
/// retirada en el registro.
pub fn motivo_de_retirada(retirar: &ParaRetirar) -> String {
    format!(
        "no gana ninguna clase ni familia: su mejor nota es {} en «{}», donde gana {} con {}",
        decimal(retirar.mejor_nota, DECIMALES_DE_LA_NOTA),
        nombre_de_categoria(retirar.en),
        retirar.le_gana.nombre,
        decimal(retirar.le_gana.nota, DECIMALES_DE_LA_NOTA)
    )
}

/// Por qué se dejó fuera un fichero, dicho en una frase.
pub fn motivo_de_descarte(motivo: &MotivoDeDescarte) -> String {
    match motivo {
        MotivoDeDescarte::NoCabe { tamano, cabe } => format!(
            "no cabe en la tarjeta: pesa {} y, con la reserva y el margen, caben {}",
            en_gib(*tamano),
            en_gib(*cabe)
        ),
        MotivoDeDescarte::SinTamano => {
            "Hugging Face no dio su tamaño, así que no se puede saber si cabe".into()
        }
        MotivoDeDescarte::FueraDePlazo {
            ultima_modificacion: Some(fecha),
        } => format!("fuera de plazo: el repositorio no se toca desde {fecha}"),
        MotivoDeDescarte::FueraDePlazo {
            ultima_modificacion: None,
        } => "fuera de plazo: Hugging Face no dio la fecha de la última modificación".into(),
        MotivoDeDescarte::YaRegistrado => {
            "ya está en el registro: su huella es la de un modelo medido".into()
        }
        MotivoDeDescarte::Partido { partes } => format!(
            "partido en {partes} ficheros: el motor y el lector GGUF trabajan hoy con uno solo"
        ),
        MotivoDeDescarte::SinCuantizacion => {
            "no tiene ningún fichero de las cuantizaciones de «[revision] cuantizaciones»".into()
        }
    }
}

/// Las tres formas de un pedido de revisión, para recordarlas cuando se escribe mal.
const FORMAS_DEL_PEDIDO: &str =
    "«revisión: buscar», «revisión: probar» con un candidato por línea \
                                 («<repositorio> <fichero>») o «revisión: cerrar»";

/// Un `revisión: probar` sin líneas debajo.
pub const PROBAR_SIN_CANDIDATOS: &str = "«revisión: probar» no trae ningún candidato: va uno por \
                                         línea debajo, «<repositorio> <fichero>», con los nombres \
                                         exactos de «candidatos.toml»";

/// Nadie puede pedir una revisión en este equipo.
pub const REVISION_DESHABILITADA: &str = "la revisión por encargo no está habilitada en este \
                                          equipo: «[revision] buzones_que_ordenan» está vacía en \
                                          «programator.toml»";

/// Lo que sigue a «revisión:» no es ninguno de los tres pedidos.
pub fn pedido_desconocido(palabra: &str) -> String {
    format!(
        "no sé qué es «{palabra}» en un pedido de revisión. Los pedidos son {FORMAS_DEL_PEDIDO}"
    )
}

/// Una línea de `revisión: probar` que no es `<repositorio> <fichero>`.
pub fn linea_de_candidato_mal_escrita(linea: &str) -> String {
    format!(
        "la línea «{linea}» no es un candidato: cada uno va como «<repositorio> <fichero>», \
         separados por un espacio y con los nombres exactos de «candidatos.toml»"
    )
}

/// El candidato que empieza a probarse.
pub fn probando(candidato: &CandidatoPedido, numero: usize, de: usize) -> String {
    format!(
        "🔬 Revisión: probando «{}» de «{}», {numero} de {de}.",
        candidato.fichero, candidato.repositorio
    )
}

/// La revisión cambió el modelo por defecto y el ciclo ya ha vuelto a leer la configuración.
pub fn configuracion_releida(config: &Config) -> String {
    format!(
        "🔁 La revisión cambió el modelo por defecto: ahora es «{}». El siguiente encargo arranca \
         el motor con él.",
        config.motor.modelo.as_deref().unwrap_or("(ninguno)")
    )
}

/// La revisión cambió el modelo, pero la configuración no se deja leer otra vez.
pub fn configuracion_sin_releer(fallo: &Error) -> String {
    format!(
        "🔴 La revisión cambió el modelo por defecto en «programator.toml», pero no se ha podido \
         volver a leer: {fallo}. Se sigue con la configuración de antes hasta que se reinicie \
         Programator."
    )
}

/// En el puerto del motor escucha un servidor que no arrancó Programator.
pub fn motor_ajeno(puerto: u16) -> String {
    format!(
        "en el puerto {puerto} hay un llama-server que no arrancó Programator. Para pasar el banco \
         hace falta la tarjeta libre, y no se para un proceso ajeno: ciérralo y vuelve a pedir la \
         prueba"
    )
}

/// Un buzón que no está entre los que pueden pedir una revisión.
pub fn buzon_sin_permiso(buzon: &str, permitidos: &[String]) -> String {
    format!(
        "«{buzon}» no puede pedir una revisión: solo pueden {}, según «[revision] \
         buzones_que_ordenan»",
        permitidos
            .iter()
            .map(|p| format!("«{p}»"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::PathBuf;

    fn hecha(anterior: Option<&str>) -> BusquedaHecha {
        BusquedaHecha {
            carpeta: PathBuf::from("revisiones/2026-11"),
            candidatos: 3,
            ya_descargados: 1,
            descartados: 40,
            anterior: anterior.map(str::to_string),
        }
    }

    #[test]
    fn la_busqueda_hecha_dice_las_cuentas_y_donde_esta() {
        let texto = busqueda_hecha(&hecha(None));
        assert!(
            texto.contains("3 candidatos, 1 ya descargados sin medir y 40 descartados"),
            "{texto}"
        );
        assert!(texto.contains("2026-11"), "{texto}");
        assert!(!texto.contains("anterior"), "{texto}");
    }

    #[test]
    fn si_habia_una_busqueda_del_mes_dice_como_se_guardo() {
        let texto = busqueda_hecha(&hecha(Some("candidatos-153012.toml")));
        assert!(texto.contains("«candidatos-153012.toml»"), "{texto}");
    }

    #[test]
    fn cada_aviso_de_la_descarga_dice_lo_suyo() {
        let gib = 1024 * 1024 * 1024;
        let casos = [
            (Aviso::Reanuda { desde: gib }, "se sigue desde ahí"),
            (Aviso::EmpiezaDeCero { tenia: gib }, "se empieza de cero"),
            (
                Aviso::Avance {
                    llevados: gib,
                    total: 2 * gib,
                },
                "1,0 GiB de 2,0 GiB",
            ),
            (Aviso::CompruebaLaHuella, "huella"),
        ];
        for (aviso, clave) in casos {
            let texto = aviso_de_descarga(&aviso);
            assert!(texto.contains(clave), "{aviso:?}: {texto}");
        }
    }

    #[test]
    fn la_descarga_hecha_dice_como_pasarle_el_banco() {
        let texto = descarga_hecha(&Descargado {
            ruta: PathBuf::from("modelos/m-Q4_K_M.gguf"),
            bajados: 10,
        });
        assert!(
            texto.contains("--modelo \"modelos/m-Q4_K_M.gguf\""),
            "{texto}"
        );
    }

    #[test]
    fn la_propuesta_hecha_dice_las_cuentas_y_que_nada_se_retira_sin_orden() {
        let texto = propuesta_hecha(&PropuestaHecha {
            ruta: PathBuf::from("revisiones/2026-11/propuesta.md"),
            se_quedan: 2,
            para_retirar: 1,
            sin_medir: 3,
            anterior: Some("propuesta-150000.md".into()),
            propuesta: crate::revision::propuesta::proponer(&crate::revision::propuesta::Entrada {
                registro: &crate::registro::modelo::Registro::default(),
                tarjeta: "NVIDIA de prueba",
                en_modelos: &[],
                modelo_en_uso: None,
            }),
        });
        assert!(
            texto.contains("se quedan 2, se propone retirar 1 y hay 3 sin medir"),
            "{texto}"
        );
        assert!(texto.contains("La propuesta no retira nada"));
        assert!(texto.contains("«propuesta-150000.md»"));
    }

    #[test]
    fn el_motivo_de_retirada_dice_su_mejor_nota_y_quien_le_gana() {
        use crate::banco::resultados::FilaDeDesempeno;
        use crate::revision::propuesta::Vencedor;
        let texto = motivo_de_retirada(&ParaRetirar {
            nombre: "flojo".into(),
            mejor_nota: 2.5,
            en: Categoria::Familia(crate::banco::ficha::Familia::Uso),
            le_gana: Vencedor {
                nombre: "fuerte".into(),
                nota: 3.0,
            },
        });
        assert_eq!(
            texto,
            "no gana ninguna clase ni familia: su mejor nota es 2,5 en «Familia de uso», donde \
             gana fuerte con 3,0"
        );
        assert_eq!(
            nombre_de_categoria(Categoria::Clase(FilaDeDesempeno::Formas)),
            "Auditar buscando formas"
        );
    }

    #[test]
    fn la_retirada_hecha_dice_lo_liberado_y_lo_anotado() {
        let gib = 1024 * 1024 * 1024;
        let con = retirada_hecha(&Retirada {
            ruta: PathBuf::from("modelos/m.gguf"),
            anotadas: 2,
            motivo: "no gana nada".into(),
            liberados: 14 * gib,
        });
        assert!(
            con.contains("14,0 GiB") && con.contains("en 2 entradas"),
            "{con}"
        );
        assert!(con.contains("«no gana nada»"), "{con}");
        let sin = retirada_hecha(&Retirada {
            ruta: PathBuf::from("modelos/m.gguf"),
            anotadas: 0,
            motivo: RETIRADO_POR_ORDEN.into(),
            liberados: gib,
        });
        assert!(sin.contains("No tenía cifras en el registro"), "{sin}");
    }

    #[test]
    fn el_fallo_dice_que_no_se_toco_nada() {
        let texto = fallo(&Error::Revision("sin red".into()));
        assert!(
            texto.contains("sin red") && texto.contains("No se ha tocado"),
            "{texto}"
        );
    }

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn el_que_no_cabe_dice_lo_que_pesa_y_lo_que_cabe() {
        let texto = motivo_de_descarte(&MotivoDeDescarte::NoCabe {
            tamano: 13 * GIB,
            cabe: 12 * GIB,
        });
        assert!(
            texto.contains("13,0 GiB") && texto.contains("12,0 GiB"),
            "{texto}"
        );
    }

    #[test]
    fn cada_motivo_tiene_su_frase() {
        let motivos = [
            MotivoDeDescarte::SinTamano,
            MotivoDeDescarte::FueraDePlazo {
                ultima_modificacion: Some("2026-01-01T00:00:00.000Z".into()),
            },
            MotivoDeDescarte::FueraDePlazo {
                ultima_modificacion: None,
            },
            MotivoDeDescarte::YaRegistrado,
            MotivoDeDescarte::Partido { partes: 3 },
            MotivoDeDescarte::SinCuantizacion,
        ];
        let claves = [
            "tamaño",
            "2026-01-01",
            "fecha",
            "registro",
            "3 ficheros",
            "cuantizaciones",
        ];
        for (motivo, clave) in motivos.iter().zip(claves) {
            let texto = motivo_de_descarte(motivo);
            assert!(texto.contains(clave), "{motivo:?}: {texto}");
        }
    }
}
