//! `candidatos.md`: la lista de una búsqueda para leerla, con los descartados agrupados por motivo
//! y con su tamaño, para que se vea qué se dejó fuera y por qué.

use super::candidatos::{Descartado, MotivoDeDescarte};
use super::lista::{enlace_del_repositorio, EntradaDeLaLista, ListaDeCandidatos};
use super::mensajes::{motivo_de_descarte, motivo_sin_reponer};
use super::reponer::Reposicion;
use super::{BYTES_POR_MIB, LARGO_DE_LA_FECHA};
use crate::informe::en_gib;
use crate::motor::fichero_de_modelo::con_extension;

const CABECERA_DE_LA_TABLA: &str = "| Repositorio | Fichero | Cuantización | Tamaño | Licencia | \
                                    Modificado | Descargas |\n|---|---|---|---|---|---|---|\n";
/// Lo que sale en una celda de la tabla cuando Hugging Face no dio el dato.
const SIN_DATO: &str = "?";
/// La licencia de un repositorio que no la declara en sus metadatos.
const SIN_LICENCIA: &str = "sin licencia declarada";
/// Lo que dice una sección vacía de los informes de la revisión.
pub(super) const NINGUNO: &str = "Ninguno.\n";

/// El documento entero.
pub fn componer(
    lista: &ListaDeCandidatos,
    descartados: &[Descartado],
    reposicion: &Reposicion,
    sitio: &str,
) -> String {
    let mut texto = format!(
        "# Candidatos de la revisión del {}\n\n\
         Tarjeta: {}, {}. Cabe un fichero de hasta {}: la VRAM menos el margen de `[motor]` y la \
         reserva de `[revision]`.\n\n",
        lista.fecha,
        lista.tarjeta,
        en_gib(lista.vram_mib.saturating_mul(BYTES_POR_MIB)),
        en_gib(lista.cabe_mib.saturating_mul(BYTES_POR_MIB)),
    );
    texto += &seccion_de_la_reposicion(reposicion);
    texto += &seccion_de_ficheros("Candidatos", &lista.candidatos);
    texto += &seccion_de_ficheros("Ya descargados, sin medir", &lista.ya_descargados);
    texto += &seccion_de_descartados(descartados, sitio);
    texto
}

/// Lo que se hizo, antes de buscar, con los modelos del registro cuyo fichero faltaba.
fn seccion_de_la_reposicion(reposicion: &Reposicion) -> String {
    let cuantos = reposicion.repuestos.len()
        + reposicion.sin_reponer.len()
        + reposicion.fuera_del_registro.len()
        + reposicion.sin_decidir.len();
    let mut texto = format!("## Modelos del registro que faltaban ({cuantos})\n\n");
    if !reposicion.hubo_ausentes() {
        texto += NINGUNO;
    }
    for repuesto in &reposicion.repuestos {
        texto += &format!(
            "- `{}`: repuesto desde su repositorio ({}), con la huella del registro \
             comprobada.\n",
            con_extension(&repuesto.nombre),
            en_gib(repuesto.bytes)
        );
    }
    for sin in &reposicion.sin_reponer {
        texto += &format!(
            "- `{}`: no se ha podido reponer: {}. Se vuelve a intentar en la próxima búsqueda.\n",
            con_extension(&sin.nombre),
            motivo_sin_reponer(&sin.motivo)
        );
    }
    for nombre in &reposicion.fuera_del_registro {
        texto += &format!(
            "- `{nombre}`: faltaba y no gana nada, así que sale del registro: queda retirado en \
             esta tarjeta.\n"
        );
    }
    for nombre in &reposicion.sin_decidir {
        texto += &format!(
            "- `{nombre}`: falta, y no se sabe si sería útil: no se repone ni se retira.\n"
        );
    }
    for ilegible in &reposicion.listas_ilegibles {
        texto += &format!(
            "\nNo se ha podido leer «{}», de una búsqueda anterior, y no se ha tenido en cuenta \
             para saber de dónde salió cada modelo: {}.\n",
            ilegible.ruta.display(),
            ilegible.causa
        );
    }
    texto + "\n"
}

fn seccion_de_ficheros(titulo: &str, entradas: &[EntradaDeLaLista]) -> String {
    let mut texto = format!("## {titulo} ({})\n\n", entradas.len());
    if entradas.is_empty() {
        return texto + NINGUNO + "\n";
    }
    texto += CABECERA_DE_LA_TABLA;
    for entrada in entradas {
        let f = &entrada.fichero;
        texto += &format!(
            "| [{}]({}) | {} | {} | {} | {} | {} | {} |\n",
            f.repositorio,
            entrada.enlace,
            f.fichero,
            f.cuantizacion,
            tamano(f.tamano),
            f.licencia.as_deref().unwrap_or(SIN_LICENCIA),
            dia(f.ultima_modificacion.as_deref()),
            f.descargas.map_or(SIN_DATO.to_string(), |d| d.to_string()),
        );
    }
    texto + "\n"
}

fn seccion_de_descartados(descartados: &[Descartado], sitio: &str) -> String {
    let mut texto = format!("## Descartados ({})\n\n", descartados.len());
    if descartados.is_empty() {
        return texto + NINGUNO;
    }
    for grupo in GRUPOS {
        let del_grupo: Vec<&Descartado> = descartados
            .iter()
            .filter(|d| grupo_de(&d.motivo) == grupo)
            .collect();
        if del_grupo.is_empty() {
            continue;
        }
        texto += &format!("### {} ({})\n\n", titulo_del_grupo(grupo), del_grupo.len());
        for descartado in del_grupo {
            texto += &format!(
                "- [{}]({}){} · {}: {}\n",
                descartado.repositorio,
                enlace_del_repositorio(sitio, &descartado.repositorio),
                descartado
                    .fichero
                    .as_deref()
                    .map_or(String::new(), |f| format!(" · {f}")),
                tamano(descartado.tamano),
                motivo_de_descarte(&descartado.motivo),
            );
        }
        texto += "\n";
    }
    texto
}

/// Los grupos, en el orden en que salen: primero lo que más interesa saber que se dejó fuera.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Grupo {
    NoCabe,
    SinTamano,
    FueraDePlazo,
    YaRegistrado,
    RegistradoSinFichero,
    Partido,
    SinCuantizacion,
}

const GRUPOS: [Grupo; 7] = [
    Grupo::NoCabe,
    Grupo::SinTamano,
    Grupo::FueraDePlazo,
    Grupo::YaRegistrado,
    Grupo::RegistradoSinFichero,
    Grupo::Partido,
    Grupo::SinCuantizacion,
];

fn grupo_de(motivo: &MotivoDeDescarte) -> Grupo {
    match motivo {
        MotivoDeDescarte::NoCabe { .. } => Grupo::NoCabe,
        MotivoDeDescarte::SinTamano => Grupo::SinTamano,
        MotivoDeDescarte::FueraDePlazo { .. } => Grupo::FueraDePlazo,
        MotivoDeDescarte::YaRegistrado => Grupo::YaRegistrado,
        MotivoDeDescarte::RegistradoSinFichero => Grupo::RegistradoSinFichero,
        MotivoDeDescarte::Partido { .. } => Grupo::Partido,
        MotivoDeDescarte::SinCuantizacion => Grupo::SinCuantizacion,
    }
}

fn titulo_del_grupo(grupo: Grupo) -> &'static str {
    match grupo {
        Grupo::NoCabe => "No caben en la tarjeta",
        Grupo::SinTamano => "Sin tamaño",
        Grupo::FueraDePlazo => "Fuera de plazo",
        Grupo::YaRegistrado => "Ya en el registro",
        Grupo::RegistradoSinFichero => "En el registro, sin fichero",
        Grupo::Partido => "Partidos en varios ficheros",
        Grupo::SinCuantizacion => "Sin ninguna de las cuantizaciones",
    }
}

fn tamano(bytes: Option<u64>) -> String {
    bytes.map_or(SIN_DATO.to_string(), en_gib)
}

fn dia(fecha: Option<&str>) -> &str {
    fecha
        .and_then(|f| f.get(..LARGO_DE_LA_FECHA))
        .unwrap_or(SIN_DATO)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::revision::candidatos::FicheroCandidato;

    const GIB: u64 = 1024 * BYTES_POR_MIB;
    const SITIO: &str = "https://huggingface.co";

    fn entrada(repositorio: &str, licencia: Option<&str>) -> EntradaDeLaLista {
        EntradaDeLaLista::nueva(
            FicheroCandidato {
                repositorio: repositorio.into(),
                fichero: "m-Q4_K_M.gguf".into(),
                cuantizacion: "Q4_K_M".into(),
                tamano: Some(4 * GIB),
                huella_sha256: None,
                licencia: licencia.map(str::to_string),
                ultima_modificacion: Some("2026-11-01T10:00:00.000Z".into()),
                descargas: Some(7),
            },
            SITIO,
        )
    }

    fn lista(candidatos: Vec<EntradaDeLaLista>) -> ListaDeCandidatos {
        ListaDeCandidatos {
            fecha: "2026-11-12".into(),
            tarjeta: "Tarjeta".into(),
            vram_mib: 16 * 1024,
            cabe_mib: 12 * 1024,
            candidatos,
            ya_descargados: vec![],
        }
    }

    #[test]
    fn cada_candidato_sale_con_enlace_tamano_licencia_y_fecha() {
        let texto = componer(
            &lista(vec![entrada("a/b", Some("mit"))]),
            &[],
            &Reposicion::default(),
            SITIO,
        );
        assert!(
            texto.contains("Cabe un fichero de hasta 12,0 GiB"),
            "{texto}"
        );
        assert!(texto.contains("## Candidatos (1)"), "{texto}");
        assert!(
            texto.contains(
                "| [a/b](https://huggingface.co/a/b) | m-Q4_K_M.gguf | Q4_K_M | 4,0 GiB | mit | \
                 2026-11-01 | 7 |"
            ),
            "{texto}"
        );
        assert!(
            texto.contains("## Ya descargados, sin medir (0)\n\nNinguno."),
            "{texto}"
        );
    }

    #[test]
    fn sin_licencia_se_dice_y_no_se_deja_en_blanco() {
        let texto = componer(
            &lista(vec![entrada("a/b", None)]),
            &[],
            &Reposicion::default(),
            SITIO,
        );
        assert!(texto.contains(SIN_LICENCIA), "{texto}");
    }

    #[test]
    fn los_descartados_salen_agrupados_por_motivo_y_con_su_tamano() {
        let descartados = [
            Descartado {
                repositorio: "a/viejo".into(),
                fichero: Some("v-Q4_K_M.gguf".into()),
                tamano: Some(GIB),
                motivo: MotivoDeDescarte::FueraDePlazo {
                    ultima_modificacion: Some("2026-01-01T00:00:00.000Z".into()),
                },
            },
            Descartado {
                repositorio: "a/grande".into(),
                fichero: Some("g-Q8_0.gguf".into()),
                tamano: Some(20 * GIB),
                motivo: MotivoDeDescarte::NoCabe {
                    tamano: 20 * GIB,
                    cabe: 12 * GIB,
                },
            },
            Descartado {
                repositorio: "a/vacio".into(),
                fichero: None,
                tamano: None,
                motivo: MotivoDeDescarte::SinCuantizacion,
            },
        ];
        let texto = componer(&lista(vec![]), &descartados, &Reposicion::default(), SITIO);
        let no_caben = texto.find("### No caben en la tarjeta (1)").unwrap();
        let plazo = texto.find("### Fuera de plazo (1)").unwrap();
        assert!(no_caben < plazo, "{texto}");
        assert!(
            texto.contains("· g-Q8_0.gguf · 20,0 GiB: no cabe"),
            "{texto}"
        );
        assert!(
            texto.contains("[a/vacio](https://huggingface.co/a/vacio) · ?: no tiene"),
            "{texto}"
        );
        assert!(!texto.contains("### Partidos"), "{texto}");
    }

    #[test]
    fn los_del_registro_sin_fichero_van_en_su_grupo_detras_de_los_registrados() {
        let descartado = |repositorio: &str, motivo| Descartado {
            repositorio: repositorio.into(),
            fichero: Some("m-Q4_K_M.gguf".into()),
            tamano: Some(GIB),
            motivo,
        };
        let descartados = [
            descartado("a/falta", MotivoDeDescarte::RegistradoSinFichero),
            descartado("a/esta", MotivoDeDescarte::YaRegistrado),
        ];
        let texto = componer(&lista(vec![]), &descartados, &Reposicion::default(), SITIO);
        let registrados = texto.find("### Ya en el registro (1)").unwrap();
        let sin_fichero = texto.find("### En el registro, sin fichero (1)").unwrap();
        assert!(registrados < sin_fichero, "{texto}");
        assert!(
            texto[sin_fichero..].contains("a/falta") && !texto[sin_fichero..].contains("a/esta"),
            "{texto}"
        );
    }
}
