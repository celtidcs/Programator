//! Todo lo que la revisión de modelos dice por pantalla o deja escrito para el usuario.

use std::path::Path;

use super::ausentes::Util;
use super::buscar::{BusquedaHecha, CARPETA_DE_RESPUESTAS, NOMBRE_DEL_INFORME};
use super::candidatos::MotivoDeDescarte;
use super::descarga::{Aviso, Descargado, SUFIJO_PARCIAL};
use super::donde::Donde;
use super::encargo::CandidatoPedido;
use super::lista::NOMBRE_DE_LA_LISTA;
use super::probados::NOMBRE_DE_LO_PROBADO;
use super::proponer::PropuestaHecha;
use super::propuesta::{Categoria, Excluida, ParaRetirar};
use super::redaccion::NOMBRE_DE_LAS_RETIRADAS;
use super::reponer::{AvisoDeReposicion, MotivoSinReponer, Reposicion};
use super::retirada::Retirada;
use super::DECIMALES_DE_LA_NOTA;
use crate::argumentos::{
    OPCION_BANCO, OPCION_DIAGNOSTICO, OPCION_MODELO, OPCION_RETIRAR_MODELO, OPCION_REVISION_BUSCAR,
};
use crate::arranque::disponibles::PEDIDO_DE_BUSQUEDA;
use crate::banco::resultados::{decimal, nombre_de_familia};
use crate::concordancia::{cantidad, juntar_con_y, segun};
use crate::config::{Config, NOMBRE_DEL_FICHERO_DE_CONFIGURACION};
use crate::error::Error;
use crate::informe::en_gib;
use crate::motor::fichero_de_modelo::con_extension;
use crate::plataforma::NOMBRE_DEL_EJECUTABLE;
use crate::registro::modelo::CodigoPuntuadoCon;

/// Sin la VRAM no se puede descartar nada con criterio.
pub fn sin_tarjeta() -> String {
    format!(
        "no se encontró la tarjeta gráfica, y sin saber su memoria no se puede decir qué modelos \
         caben. Comprueba que el controlador está instalado con «{NOMBRE_DEL_EJECUTABLE} \
         {OPCION_DIAGNOSTICO}»"
    )
}

/// Las cifras del registro son de una tarjeta: sin saber cuál es esta, no se sabe cuáles valen.
pub fn sin_tarjeta_para_proponer() -> String {
    format!(
        "no se encontró la tarjeta gráfica, y las cifras del registro valen solo para la tarjeta \
         en que se midieron. Comprueba que el controlador está instalado con \
         «{NOMBRE_DEL_EJECUTABLE} {OPCION_DIAGNOSTICO}»"
    )
}

/// Lo que dejó `--revision-proponer`, y lo que toca después.
pub fn propuesta_hecha(hecha: &PropuestaHecha) -> String {
    let anterior = hecha.anterior.as_deref().map_or(String::new(), |nombre| {
        format!("\nLa propuesta anterior de este mes se ha guardado como «{nombre}».")
    });
    format!(
        "✅ Propuesta hecha: se quedan {}, se propone retirar {} y hay {} sin medir.\n\
         Está en «{}». La propuesta no retira nada: lo retira «{OPCION_RETIRAR_MODELO}» o, en la \
         revisión \
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
        "{}✅ Búsqueda hecha: {} candidatos, {} ya descargados sin medir y {} descartados.\n\
         Está todo en «{}»: «{NOMBRE_DEL_INFORME}» para leerlo, «{NOMBRE_DE_LA_LISTA}» para \
         descargar y «{CARPETA_DE_RESPUESTAS}/» con lo que contestó Hugging Face.{anterior}",
        lo_que_faltaba(&hecha.reposicion),
        hecha.candidatos,
        hecha.ya_descargados,
        hecha.descartados,
        hecha.carpeta.display(),
    )
}

/// Una frase con las cuentas de la reposición, para la terminal. Sin ausentes, nada.
fn lo_que_faltaba(reposicion: &Reposicion) -> String {
    if !reposicion.hubo_ausentes() {
        return String::new();
    }
    format!(
        "📦 De los modelos del registro que faltaban en «modelos»: {} repuestos, {} sin poder \
         reponer, {} fuera del registro y {} sin decidir. «{NOMBRE_DEL_INFORME}» dice cuáles y \
         por qué.\n",
        reposicion.repuestos.len(),
        reposicion.sin_reponer.len(),
        reposicion.fuera_del_registro.len(),
        reposicion.sin_decidir.len()
    )
}

/// Por qué no se pudo hacer una orden de la revisión. No dice que no se tocó nada: la búsqueda
/// repone y retira antes de buscar, el cierre retira y anota, y una descarga deja su `.parcial`,
/// así que un fallo puede llegar con parte del trabajo hecho. Lo que es cierto siempre es que no
/// se deshace.
pub fn fallo(fallo: &Error) -> String {
    format!(
        "🔴 {fallo}.\nLo hecho antes del fallo no se deshace: los modelos que ya se habían bajado \
         o repuesto siguen en la carpeta de modelos, lo anotado en el registro sigue anotado y \
         una descarga a medias queda como «{SUFIJO_PARCIAL}» para reanudarla. Si es la red, vuelve \
         a intentarlo; si es la configuración, revisa la sección [revision] de \
         «{NOMBRE_DEL_FICHERO_DE_CONFIGURACION}»."
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
         Lo siguiente es pasarle el banco: «{NOMBRE_DEL_EJECUTABLE} {OPCION_BANCO} banco \
         {OPCION_MODELO} \"{ruta}\"»."
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
         «{NOMBRE_DEL_EJECUTABLE} {OPCION_REVISION_BUSCAR}»",
        carpeta.display()
    )
}

/// Solo se descarga lo que está en la lista de la búsqueda más reciente.
pub fn no_esta_en_la_lista(repositorio: &str, fichero: &str, lista: &Path) -> String {
    format!(
        "«{fichero}» de «{repositorio}» no está entre los candidatos de «{}». Solo se descarga \
         lo que encontró la búsqueda más reciente: mira los nombres exactos en «{NOMBRE_DEL_INFORME}», \
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
         vez con «{OPCION_REVISION_BUSCAR}»"
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

/// El motivo de una retirada que la propuesta más reciente no proponía: la pidió quien usa
/// Programator con `--retirar-modelo`. Se guarda en el registro; las retiradas de antes de la
/// 1.1.0 conservan el texto que tenían.
pub const RETIRADO_POR_ORDEN: &str = "retirado a mano: la propuesta no lo pedía";

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
        "«{fichero}» es el modelo de «programator.toml» y no se retira. Para retirarlo, \
         primero hay que cambiar «[motor] modelo»"
    )
}

pub fn retirar_lo_que_no_esta(fichero: &str) -> String {
    format!("«{fichero}» no está en la carpeta de modelos: no hay nada que retirar")
}

/// El registro ya anota la retirada, pero el fichero no se dejó borrar.
pub fn retirada_sin_borrar(ruta: &Path, anotadas: usize, causa: &std::io::Error) -> String {
    format!(
        "la retirada ya consta en el registro ({}), pero «{}» no se pudo borrar: {causa}. Bórralo \
         a mano cuando no lo use nada",
        cantidad(anotadas, "entrada", "entradas"),
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
            "Anotado en el registro, en {}, con sus cifras intactas: «{}».",
            cantidad(retirada.anotadas, "entrada", "entradas"),
            retirada.motivo
        )
    };
    if retirada.sin_fichero {
        return format!(
            "🗑️ Retirado «{}» del registro: su fichero ya no estaba en la carpeta de modelos, así \
             que no se ha borrado nada.\n{registro}",
            retirada.ruta.display()
        );
    }
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
        MotivoDeDescarte::RegistradoSinFichero => "está en el registro pero falta su fichero en \
                                                   «modelos»: mira el apartado de los modelos del \
                                                   registro que faltaban"
            .into(),
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
pub fn probar_sin_candidatos() -> String {
    format!(
        "«revisión: probar» no trae ningún candidato: va uno por línea debajo, «<repositorio> \
         <fichero>», con los nombres exactos de «{NOMBRE_DE_LA_LISTA}»"
    )
}

/// Nadie puede pedir una revisión en este equipo.
pub fn revision_deshabilitada() -> String {
    format!(
        "la revisión por encargo no está habilitada en este equipo: «[revision] \
         identidades_que_ordenan» está vacía en «{NOMBRE_DEL_FICHERO_DE_CONFIGURACION}»"
    )
}

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
         separados por un espacio y con los nombres exactos de «{NOMBRE_DE_LA_LISTA}»"
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

/// Un pedido de revisión desde una carpeta que no es la de instalación.
pub fn revision_fuera_de_la_instalacion(instalacion: &Path) -> String {
    format!(
        "la revisión de modelos se hace en la carpeta donde está instalado Programator, «{}», y \
         esta no es: cierra Programator, ábrelo junto con el agente que la vaya a llevar en esa \
         carpeta y pídela allí. Allí están los modelos, el banco y las revisiones que tendrás que \
         leer",
        instalacion.display()
    )
}

/// Qué hacer cuando no hay modelos o ninguno cabe, según dónde se esté y si alguien tiene permiso
/// para pedir la revisión.
pub fn que_hacer_sin_modelos(
    donde: &Donde,
    configuracion: &Path,
    carpeta_de_modelos: &str,
    hay_identidades: bool,
) -> String {
    let como = como_pedir_la_revision(donde, configuracion, hay_identidades);
    let o_a_mano =
        format!("También puedes dejar tú un modelo que quepa en «{carpeta_de_modelos}».");
    match donde {
        Donde::Fuera { .. } => format!(
            "Programator no descarga nada por su cuenta. La revisión de modelos, que busca, \
             descarga y prueba los que caben, se hace en la carpeta donde está instalado \
             Programator: {como}.

{o_a_mano}"
        ),
        Donde::EnLaInstalacion if !hay_identidades => format!(
            "Programator no descarga nada por su cuenta. Para buscar, descargar y probar modelos \
             que quepan, {como}.

{o_a_mano}"
        ),
        Donde::EnLaInstalacion => format!(
            "Programator no descarga nada por su cuenta: {como}, que busca, descarga y prueba los \
             modelos que caben.

{o_a_mano}"
        ),
    }
}

/// Dónde y a quién se pide la revisión, en minúscula y sin punto final: fuera de la instalación,
/// que se abra allí; sin nadie con permiso, cómo darlo; con permiso, solo el pedido.
pub fn como_pedir_la_revision(
    donde: &Donde,
    configuracion: &Path,
    hay_identidades: bool,
) -> String {
    let permiso = format!(
        "dale permiso al agente que la lleve con «identidades_que_ordenan = [\"<su identidad>\"]» \
         en la sección [revision] de «{}»",
        configuracion.display()
    );
    match donde {
        Donde::Fuera { instalacion } => format!(
            "cierra este Programator, ábrelo junto con el agente que quieras que la lleve en \
             «{}», {permiso} y pídele allí «{PEDIDO_DE_BUSQUEDA}»",
            instalacion.display()
        ),
        Donde::EnLaInstalacion if !hay_identidades => {
            format!("{permiso} y pídele «{PEDIDO_DE_BUSQUEDA}»")
        }
        Donde::EnLaInstalacion => format!("pide «{PEDIDO_DE_BUSQUEDA}»"),
    }
}

/// El aviso del arranque cuando faltan en la carpeta de modelos modelos del registro que siguen
/// haciendo falta. `None` si no falta ninguno.
/// `como_pedir` es [`como_pedir_la_revision`]. Párrafos separados por una línea en blanco.
pub fn aviso_de_los_que_faltan(
    utiles: &[Util],
    como_pedir: &str,
    carpeta_de_modelos: &str,
) -> Option<String> {
    if utiles.is_empty() {
        return None;
    }
    let n = utiles.len();
    let todos_ganan = utiles.iter().all(|u| !u.gana.is_empty());
    let por_que = match (n, todos_ganan) {
        (1, true) => "es el mejor en algo",
        (_, true) => "son los mejores en algo",
        (1, false) => "sigue haciendo falta",
        (_, false) => "siguen haciendo falta",
    };
    let cuales = juntar_con_y(utiles.iter().map(describir_el_que_falta).collect());
    let mut primero = if n == 1 {
        format!(
            "⚠️ Falta en «{carpeta_de_modelos}» un modelo que, según lo medido en esta tarjeta, \
             {por_que}: {cuales}."
        )
    } else {
        format!(
            "⚠️ Faltan en «{carpeta_de_modelos}» {n} modelos que, según lo medido en esta \
             tarjeta, {por_que}: {cuales}."
        )
    };
    if utiles.iter().any(|u| !u.gana.is_empty()) {
        primero += &format!(
            " Mientras {}, esos encargos los atiende el mejor de los modelos que sí están.",
            segun(n, "falte", "falten")
        );
    }
    let segundo = format!(
        "Programator no {lo} descarga por su cuenta. {Lo} repone la revisión de modelos, que {lo} \
         baja de nuevo de Hugging Face y comprueba que es el mismo fichero que se midió: \
         {como_pedir}.",
        lo = segun(n, "lo", "los"),
        Lo = segun(n, "Lo", "Los"),
    );
    let mut parrafos = vec![primero, segundo];
    parrafos.extend(
        utiles
            .iter()
            .filter(|u| u.origen.is_none())
            .map(|u| sin_origen_al_arrancar(&u.nombre, carpeta_de_modelos)),
    );
    Some(parrafos.join("\n\n"))
}

/// ««A», el mejor en «Diagnosticar la causa de un fallo», que es el modelo por defecto de
/// programator.toml».
fn describir_el_que_falta(util: &Util) -> String {
    let mut texto = format!("«{}»", util.nombre);
    if !util.gana.is_empty() {
        let categorias = util
            .gana
            .iter()
            .map(|c| format!("«{}»", nombre_de_categoria(*c)))
            .collect();
        texto += &format!(", el mejor en {}", juntar_con_y(categorias));
    }
    if util.en_uso {
        texto +=
            &format!(", que es el modelo por defecto de {NOMBRE_DEL_FICHERO_DE_CONFIGURACION}");
    }
    texto
}

/// Un útil que falta y del que no se sabe de dónde salió: qué puede hacer la persona.
fn sin_origen_al_arrancar(nombre: &str, carpeta_de_modelos: &str) -> String {
    let fichero = con_extension(nombre);
    format!(
        "De «{nombre}» no se sabe de qué repositorio salió, así que la revisión no puede bajarlo: \
         si lo tienes, déjalo en «{carpeta_de_modelos}» con el nombre «{fichero}», o retíralo del \
         registro con «{NOMBRE_DEL_EJECUTABLE} {OPCION_RETIRAR_MODELO} {fichero}»."
    )
}

/// Quien pide la revisión no está en la lista.
pub fn identidad_sin_permiso(quien: &str, permitidas: &[String]) -> String {
    format!(
        "«{quien}» no puede pedir una revisión: solo pueden {}, según «[revision] \
         identidades_que_ordenan»",
        lista_entre_comillas(permitidas)
    )
}

/// El buzón de quien pide la revisión no dice quién es.
pub fn revision_sin_identidad(permitidas: &[String]) -> String {
    format!(
        "no sé quién pide esta revisión: la primera línea de tu buzón tiene que ser «# Bandeja de \
         <tu identidad>». Solo pueden pedirla {}",
        lista_entre_comillas(permitidas)
    )
}

/// El motivo que se anota en el registro a un modelo que faltaba y no gana nada.
pub fn motivo_de_retirada_por_ausencia(retirar: &ParaRetirar) -> String {
    format!(
        "Faltaba su fichero en «modelos». {}.",
        con_mayuscula(&motivo_de_retirada(retirar))
    )
}

/// Por qué no se pudo reponer un modelo útil que falta, dicho en una frase.
pub fn motivo_sin_reponer(motivo: &MotivoSinReponer) -> String {
    match motivo {
        MotivoSinReponer::SinOrigen => "no se sabe de dónde salió: no está en ninguna búsqueda \
                                        guardada ni en la de ahora"
            .into(),
        MotivoSinReponer::FalloAlConsultar(causa) => {
            format!("Hugging Face no contestó por su repositorio: {causa}")
        }
        MotivoSinReponer::YaNoEstaEnElSitio => "su fichero ya no está en su repositorio".into(),
        MotivoSinReponer::HuellaCambiada => {
            "el fichero de su repositorio ya no es el que se midió: su huella es otra".into()
        }
        MotivoSinReponer::SinTamano => {
            "Hugging Face no da el tamaño de su fichero, y sin él no se puede descargar".into()
        }
        MotivoSinReponer::HayOtroFichero => "en «modelos» ya hay un fichero con su nombre y otra \
                                             huella, y no se pisa"
            .into(),
        MotivoSinReponer::FalloAlDescargar(causa) => format!("falló la descarga: {causa}"),
    }
}

/// Lo que va pasando al reponer.
pub fn aviso_de_reposicion(aviso: &AvisoDeReposicion) -> String {
    match aviso {
        AvisoDeReposicion::Empieza {
            nombre,
            repositorio,
            bytes,
        } => format!(
            "📥 Reponiendo «{}» desde «{repositorio}» ({}).",
            con_extension(nombre),
            en_gib(*bytes)
        ),
        AvisoDeReposicion::Descarga(aviso) => aviso_de_descarga(aviso),
    }
}

/// La primera letra en mayúscula, para empezar una frase con un texto que va en minúscula.
fn con_mayuscula(texto: &str) -> String {
    let mut letras = texto.chars();
    match letras.next() {
        Some(primera) => primera.to_uppercase().chain(letras).collect(),
        None => String::new(),
    }
}

fn lista_entre_comillas(elementos: &[String]) -> String {
    elementos
        .iter()
        .map(|e| format!("«{e}»"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Una consulta a Hugging Face falló y la revisión no dejó nada escrito.
pub(crate) fn consulta_fallida_sin_escribir(cual: &str, causa: &str) -> String {
    format!("{cual} falló, y no se ha escrito nada: {causa}")
}

/// Hugging Face no contestó a una petición.
pub(crate) fn hugging_face_no_contesto(url: &str, causa: &dyn std::fmt::Display) -> String {
    format!("Hugging Face no contestó a {url}: {causa}")
}

/// La respuesta de Hugging Face llegó pero no se pudo leer entera.
pub(crate) fn respuesta_sin_leer(url: &str, causa: &dyn std::fmt::Display) -> String {
    format!("la respuesta de {url} no se pudo leer: {causa}")
}

/// La respuesta de una búsqueda en Hugging Face no se deja interpretar.
pub(crate) fn busqueda_que_no_se_entiende(causa: &dyn std::fmt::Display) -> String {
    format!("la búsqueda no se entiende: {causa}")
}

/// El detalle de un repositorio de Hugging Face no se deja interpretar.
pub(crate) fn detalle_que_no_se_entiende(causa: &dyn std::fmt::Display) -> String {
    format!("el detalle no se entiende: {causa}")
}

/// Una descarga se cortó por un fallo al escribir o leer; añade la causa.
pub(crate) fn descarga_cortada_por(
    llevados: u64,
    total: u64,
    parcial: &Path,
    causa: &dyn std::fmt::Display,
) -> String {
    format!("{}: {causa}", descarga_cortada(llevados, total, parcial))
}

/// La petición de descarga de un fichero falló.
pub(crate) fn descarga_sin_empezar(url: &str, causa: &dyn std::fmt::Display) -> String {
    format!("no se pudo descargar «{url}»: {causa}")
}

/// La lista de candidatos no se pudo pasar a TOML.
pub(crate) fn lista_sin_componer(causa: &dyn std::fmt::Display) -> String {
    format!("la lista de candidatos no se compone: {causa}")
}

/// El fichero de la lista de candidatos no se deja interpretar.
pub(crate) fn lista_que_no_se_entiende(causa: &dyn std::fmt::Display) -> String {
    format!("«{NOMBRE_DE_LA_LISTA}» no se entiende: {causa}")
}

/// Lo probado en un mes no se pudo pasar a TOML.
pub(crate) fn probados_sin_componer(causa: &dyn std::fmt::Display) -> String {
    format!("lo probado no se compone: {causa}")
}

/// El fichero de lo probado en un mes no se deja interpretar.
pub(crate) fn probados_que_no_se_entienden(causa: &dyn std::fmt::Display) -> String {
    format!("«{NOMBRE_DE_LO_PROBADO}» no se entiende: {causa}")
}

/// Las retiradas propuestas no se pudieron pasar a TOML.
pub(crate) fn retiradas_sin_componer(causa: &dyn std::fmt::Display) -> String {
    format!("las retiradas propuestas no se componen: {causa}")
}

/// El fichero de las retiradas propuestas no se deja interpretar.
pub(crate) fn retiradas_que_no_se_entienden(causa: &dyn std::fmt::Display) -> String {
    format!("«{NOMBRE_DE_LAS_RETIRADAS}» no se entiende: {causa}")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La alternativa de dejar un modelo a mano va en su propio párrafo, detrás de una línea en
    /// blanco.
    #[test]
    fn dejar_un_modelo_a_mano_va_en_su_propio_parrafo() {
        let texto = que_hacer_sin_modelos(
            &Donde::EnLaInstalacion,
            Path::new("programator.toml"),
            "modelos",
            false,
        );

        assert!(
            texto.contains(
                ".

También puedes dejar tú un modelo"
            ),
            "{texto}"
        );
    }

    fn util(nombre: &str, gana: Vec<Categoria>, en_uso: bool, con_origen: bool) -> Util {
        Util {
            nombre: nombre.into(),
            gana,
            en_uso,
            huella: "h".into(),
            origen: con_origen.then(|| crate::registro::modelo::Origen {
                repositorio: "r/m".into(),
                fichero: "m.gguf".into(),
                tamano: None,
            }),
        }
    }

    fn diagnostico() -> Categoria {
        Categoria::Clase(crate::banco::resultados::FilaDeDesempeno::Diagnostico)
    }

    fn fichero_limpio() -> Categoria {
        Categoria::Clase(crate::banco::resultados::FilaDeDesempeno::FicheroLimpio)
    }

    const PIDE: &str = "pide «revisión: buscar»";

    #[test]
    fn el_aviso_con_un_modelo_dice_cual_y_en_que_es_el_mejor() {
        let texto = aviso_de_los_que_faltan(
            &[util("JackOD", vec![diagnostico()], false, true)],
            PIDE,
            "modelos",
        )
        .unwrap();
        assert_eq!(
            texto,
            "⚠️ Falta en «modelos» un modelo que, según lo medido en esta tarjeta, es el mejor en \
             algo: «JackOD», el mejor en «Diagnosticar la causa de un fallo». Mientras falte, esos \
             encargos los atiende el mejor de los modelos que sí están.\n\nProgramator no lo \
             descarga por su cuenta. Lo repone la revisión de modelos, que lo baja de nuevo de \
             Hugging Face y comprueba que es el mismo fichero que se midió: pide «revisión: \
             buscar»."
        );
    }

    #[test]
    fn el_aviso_con_varios_los_enumera() {
        let texto = aviso_de_los_que_faltan(
            &[
                util("A", vec![diagnostico()], false, true),
                util("B", vec![fichero_limpio(), diagnostico()], false, true),
                util("C", vec![fichero_limpio()], false, true),
            ],
            PIDE,
            "modelos",
        )
        .unwrap();
        assert!(
            texto.starts_with(
                "⚠️ Faltan en «modelos» 3 modelos que, según lo medido en esta tarjeta, son los \
                 mejores en algo: «A», el mejor en «Diagnosticar la causa de un fallo», «B», el \
                 mejor en «Auditar un fichero que crees limpio, para confirmarlo» y \
                 «Diagnosticar la causa de un fallo» y «C», el mejor en"
            ),
            "{texto}"
        );
        assert!(texto.contains("Mientras falten,"), "{texto}");
        assert!(texto.contains("Programator no los descarga"), "{texto}");
        assert!(texto.contains("Los repone"), "{texto}");
    }

    #[test]
    fn el_aviso_dice_si_es_el_modelo_por_defecto() {
        let solo =
            aviso_de_los_que_faltan(&[util("devstral", vec![], true, true)], PIDE, "modelos")
                .unwrap();
        assert!(
            solo.starts_with(
                "⚠️ Falta en «modelos» un modelo que, según lo medido en esta tarjeta, sigue \
                 haciendo falta: «devstral», que es el modelo por defecto de programator.toml.\n\n"
            ),
            "{solo}"
        );
        assert!(!solo.contains("Mientras"), "{solo}");
        let ganando =
            aviso_de_los_que_faltan(&[util("d", vec![diagnostico()], true, true)], PIDE, "m")
                .unwrap();
        assert!(
            ganando.contains(
                "«d», el mejor en «Diagnosticar la causa de un fallo», que es el modelo por \
                 defecto de programator.toml. Mientras falte"
            ),
            "{ganando}"
        );
    }

    #[test]
    fn el_aviso_explica_que_hacer_con_uno_sin_origen() {
        let texto = aviso_de_los_que_faltan(
            &[
                util("con", vec![diagnostico()], false, true),
                util("sin", vec![diagnostico()], false, false),
            ],
            PIDE,
            "modelos",
        )
        .unwrap();
        let parrafos: Vec<&str> = texto.split("\n\n").collect();
        assert_eq!(parrafos.len(), 3, "{texto}");
        assert_eq!(
            parrafos[2],
            format!(
                "De «sin» no se sabe de qué repositorio salió, así que la revisión no puede \
                 bajarlo: si lo tienes, déjalo en «modelos» con el nombre «sin.gguf», o retíralo \
                 del registro con «{NOMBRE_DEL_EJECUTABLE} --retirar-modelo sin.gguf»."
            )
        );
    }

    #[test]
    fn fuera_de_la_instalacion_dice_donde_pedir_la_revision() {
        let fuera = Donde::Fuera {
            instalacion: PathBuf::from("C:/programator"),
        };
        let como =
            como_pedir_la_revision(&fuera, Path::new("C:/programator/programator.toml"), true);
        let texto =
            aviso_de_los_que_faltan(&[util("a", vec![diagnostico()], false, true)], &como, "m")
                .unwrap();
        assert!(
            texto.contains(
                "que se midió: cierra este Programator, ábrelo junto con el agente que quieras que \
                 la lleve en «C:/programator»"
            ),
            "{texto}"
        );
        assert!(
            texto.ends_with("y pídele allí «revisión: buscar»."),
            "{texto}"
        );
    }

    #[test]
    fn sin_utiles_que_falten_no_hay_aviso() {
        assert_eq!(aviso_de_los_que_faltan(&[], PIDE, "modelos"), None);
    }

    /// Qué se le dice a quien no tiene modelos, según dónde esté.
    #[test]
    fn sin_modelos_se_dice_adonde_ir_y_como_dar_permiso() {
        let configuracion = Path::new("inst/programator.toml");
        let fuera = Donde::Fuera {
            instalacion: "inst".into(),
        };

        let desde_fuera = que_hacer_sin_modelos(&fuera, configuracion, "inst/modelos", true);
        assert!(
            desde_fuera.contains("ábrelo junto con el agente"),
            "{desde_fuera}"
        );
        assert!(desde_fuera.contains("«inst»"), "{desde_fuera}");
        assert!(
            desde_fuera.contains("identidades_que_ordenan"),
            "{desde_fuera}"
        );

        let sin_permiso = que_hacer_sin_modelos(
            &Donde::EnLaInstalacion,
            configuracion,
            "inst/modelos",
            false,
        );
        assert!(!sin_permiso.contains("ábrelo"), "{sin_permiso}");
        assert!(
            sin_permiso.contains("identidades_que_ordenan"),
            "{sin_permiso}"
        );

        let con_permiso =
            que_hacer_sin_modelos(&Donde::EnLaInstalacion, configuracion, "inst/modelos", true);
        assert!(
            !con_permiso.contains("identidades_que_ordenan"),
            "{con_permiso}"
        );
        assert!(con_permiso.contains("«revisión: buscar»"), "{con_permiso}");
    }
    use std::path::PathBuf;

    fn hecha(anterior: Option<&str>) -> BusquedaHecha {
        BusquedaHecha {
            carpeta: PathBuf::from("revisiones/2026-11"),
            candidatos: 3,
            ya_descargados: 1,
            descartados: 40,
            anterior: anterior.map(str::to_string),
            reposicion: Reposicion::default(),
        }
    }

    #[test]
    fn la_busqueda_hecha_cuenta_lo_que_faltaba_solo_si_faltaba_algo() {
        assert!(!busqueda_hecha(&hecha(None)).contains("faltaban"));
        let mut con_ausentes = hecha(None);
        con_ausentes.reposicion.fuera_del_registro = vec!["viejo".into()];
        con_ausentes.reposicion.sin_decidir = vec!["dudoso".into()];
        let texto = busqueda_hecha(&con_ausentes);
        assert!(
            texto.starts_with(
                "📦 De los modelos del registro que faltaban en «modelos»: 0 repuestos, 0 sin \
                 poder reponer, 1 fuera del registro y 1 sin decidir."
            ),
            "{texto}"
        );
        assert!(texto.contains("✅ Búsqueda hecha"), "{texto}");
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
    fn cada_motivo_sin_reponer_dice_el_suyo() {
        let casos = [
            (MotivoSinReponer::SinOrigen, "de dónde salió"),
            (
                MotivoSinReponer::FalloAlConsultar("sin red".into()),
                "no contestó por su repositorio: sin red",
            ),
            (MotivoSinReponer::YaNoEstaEnElSitio, "ya no está"),
            (MotivoSinReponer::HuellaCambiada, "su huella es otra"),
            (MotivoSinReponer::SinTamano, "tamaño"),
            (MotivoSinReponer::HayOtroFichero, "no se pisa"),
            (
                MotivoSinReponer::FalloAlDescargar("se cortó".into()),
                "falló la descarga: se cortó",
            ),
        ];
        for (motivo, clave) in casos {
            let texto = motivo_sin_reponer(&motivo);
            assert!(texto.contains(clave), "{motivo:?}: {texto}");
        }
    }

    #[test]
    fn la_reposicion_dice_que_fichero_baja_de_donde_y_cuanto_pesa() {
        let texto = aviso_de_reposicion(&AvisoDeReposicion::Empieza {
            nombre: "m-Q4_K_M".into(),
            repositorio: "alguien/M-GGUF".into(),
            bytes: 2 * 1024 * 1024 * 1024,
        });
        assert!(
            texto.contains("«m-Q4_K_M.gguf» desde «alguien/M-GGUF» (2,0 GiB)"),
            "{texto}"
        );
        let avance = AvisoDeReposicion::Descarga(Aviso::CompruebaLaHuella);
        assert_eq!(
            aviso_de_reposicion(&avance),
            aviso_de_descarga(&Aviso::CompruebaLaHuella)
        );
    }

    #[test]
    fn la_retirada_por_ausencia_empieza_por_lo_que_faltaba_y_sigue_en_mayuscula() {
        let retirar = ParaRetirar {
            nombre: "b".into(),
            mejor_nota: 2.0,
            en: Categoria::Familia(crate::banco::ficha::Familia::Uso),
            le_gana: crate::revision::propuesta::Vencedor {
                nombre: "a".into(),
                nota: 4.0,
            },
        };
        assert_eq!(
            motivo_de_retirada_por_ausencia(&retirar),
            format!(
                "Faltaba su fichero en «modelos». N{}.",
                &motivo_de_retirada(&retirar)[1..]
            )
        );
        assert_eq!(con_mayuscula("ñandú"), "Ñandú");
        assert_eq!(con_mayuscula(""), "");
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
        // La orden lleva el nombre del ejecutable de este sistema, no
        // «programator» escrito a mano (en Windows es «programator.exe»).
        assert!(
            texto.contains(&format!("«{NOMBRE_DEL_EJECUTABLE} --banco banco --modelo")),
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
            sin_fichero: false,
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
            sin_fichero: false,
        });
        assert!(sin.contains("No tenía cifras en el registro"), "{sin}");
        assert!(!RETIRADO_POR_ORDEN.contains("Director"));
        assert!(!retirar_el_modelo_en_uso("m.gguf").contains("Director"));
        // Con una sola, en singular.
        let una = retirada_hecha(&Retirada {
            ruta: PathBuf::from("modelos/m.gguf"),
            anotadas: 1,
            motivo: "no gana nada".into(),
            liberados: gib,
            sin_fichero: false,
        });
        assert!(una.contains("en 1 entrada,"), "{una}");
        let sin_fichero = retirada_hecha(&Retirada {
            ruta: PathBuf::from("modelos/m.gguf"),
            anotadas: 1,
            motivo: "no gana nada".into(),
            liberados: 0,
            sin_fichero: true,
        });
        assert!(
            sin_fichero.starts_with(
                "🗑️ Retirado «modelos/m.gguf» del registro: su fichero ya no estaba en la carpeta \
                 de modelos, así que no se ha borrado nada.\nAnotado en el registro, en 1 entrada"
            ),
            "{sin_fichero}"
        );
        let sin_borrar = retirada_sin_borrar(
            Path::new("modelos/m.gguf"),
            1,
            &std::io::Error::other("en uso"),
        );
        assert!(sin_borrar.contains("(1 entrada)"), "{sin_borrar}");
    }

    #[test]
    fn el_fallo_dice_que_lo_hecho_no_se_deshace_y_no_que_no_se_toco_nada() {
        let texto = fallo(&Error::Revision("sin red".into()));
        assert!(
            texto.starts_with(&format!("🔴 {}.\n", Error::Revision("sin red".into()))),
            "{texto}"
        );
        assert!(
            texto.contains(
                "Lo hecho antes del fallo no se deshace: los modelos que ya se habían bajado o \
                 repuesto siguen en la carpeta de modelos, lo anotado en el registro sigue anotado \
                 y una descarga a medias queda como «.parcial» para reanudarla."
            ),
            "{texto}"
        );
        // Tras reponer o retirar, decir que no se tocó el registro sería falso.
        assert!(!texto.contains("No se ha tocado"), "{texto}");
        assert!(texto.ends_with("de «programator.toml»."), "{texto}");
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
            MotivoDeDescarte::RegistradoSinFichero,
            MotivoDeDescarte::Partido { partes: 3 },
            MotivoDeDescarte::SinCuantizacion,
        ];
        let claves = [
            "tamaño",
            "2026-01-01",
            "fecha",
            "registro",
            "falta su fichero",
            "3 ficheros",
            "cuantizaciones",
        ];
        for (motivo, clave) in motivos.iter().zip(claves) {
            let texto = motivo_de_descarte(motivo);
            assert!(texto.contains(clave), "{motivo:?}: {texto}");
        }
    }
}
