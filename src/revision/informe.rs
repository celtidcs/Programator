//! `candidatos.md`: la lista de una búsqueda para leerla, con los descartados agrupados por motivo
//! y con su tamaño, para que se vea qué se dejó fuera y por qué (especificación, §3.2, punto 6).

use super::candidatos::{Descartado, MotivoDeDescarte};
use super::lista::{enlace_del_repositorio, EntradaDeLaLista, ListaDeCandidatos};
use super::mensajes::motivo_de_descarte;
use super::{BYTES_POR_MIB, LARGO_DE_LA_FECHA};
use crate::informe::en_gib;

const CABECERA_DE_LA_TABLA: &str = "| Repositorio | Fichero | Cuantización | Tamaño | Licencia | \
                                    Modificado | Descargas |\n|---|---|---|---|---|---|---|\n";
const SIN_DATO: &str = "?";
const SIN_LICENCIA: &str = "sin licencia declarada";
const NINGUNO: &str = "Ninguno.\n";

/// El documento entero.
pub fn componer(lista: &ListaDeCandidatos, descartados: &[Descartado], sitio: &str) -> String {
    let mut texto = format!(
        "# Candidatos de la revisión del {}\n\n\
         Tarjeta: {}, {}. Cabe un fichero de hasta {}: la VRAM menos el margen de `[motor]` y la \
         reserva de `[revision]`.\n\n",
        lista.fecha,
        lista.tarjeta,
        en_gib(lista.vram_mib * BYTES_POR_MIB),
        en_gib(lista.cabe_mib * BYTES_POR_MIB),
    );
    texto += &seccion_de_ficheros("Candidatos", &lista.candidatos);
    texto += &seccion_de_ficheros("Ya descargados, sin medir", &lista.ya_descargados);
    texto += &seccion_de_descartados(descartados, sitio);
    texto
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
    Partido,
    SinCuantizacion,
}

const GRUPOS: [Grupo; 6] = [
    Grupo::NoCabe,
    Grupo::SinTamano,
    Grupo::FueraDePlazo,
    Grupo::YaRegistrado,
    Grupo::Partido,
    Grupo::SinCuantizacion,
];

fn grupo_de(motivo: &MotivoDeDescarte) -> Grupo {
    match motivo {
        MotivoDeDescarte::NoCabe { .. } => Grupo::NoCabe,
        MotivoDeDescarte::SinTamano => Grupo::SinTamano,
        MotivoDeDescarte::FueraDePlazo { .. } => Grupo::FueraDePlazo,
        MotivoDeDescarte::YaRegistrado => Grupo::YaRegistrado,
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
        let texto = componer(&lista(vec![entrada("a/b", Some("mit"))]), &[], SITIO);
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
        let texto = componer(&lista(vec![entrada("a/b", None)]), &[], SITIO);
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
        let texto = componer(&lista(vec![]), &descartados, SITIO);
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
}
