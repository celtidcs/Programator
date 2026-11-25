//! De dónde salió cada modelo del registro, para poder reponerlo si falta.
//!
//! El origen se busca por la huella del fichero que se midió entre los ficheros conocidos de
//! Hugging Face: los que acaba de contestar la búsqueda en curso y los de las listas de búsquedas
//! anteriores. Completar es puro; leer las listas es la única parte que toca disco.

use std::path::{Path, PathBuf};

use super::catalogo::DetalleDeRepositorio;
use super::lista::{self, NOMBRE_DE_LA_LISTA};
use super::mes;
use crate::error::Resultado;
use crate::ficheros;
use crate::registro::anotar_tanda::{anotar_tanda, Desenlace};
use crate::registro::modelo::{self, Origen, Registro};

/// Un fichero de Hugging Face del que se sabe la huella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FicheroConocido {
    pub repositorio: String,
    /// El nombre del fichero en el repositorio.
    pub fichero: String,
    /// En bytes, si se sabe.
    pub tamano: Option<u64>,
    pub huella: String,
}

/// Lo que se sacó de las listas de búsquedas anteriores.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeLasListas {
    pub conocidos: Vec<FicheroConocido>,
    /// Las listas que no se pudieron leer o no se entienden: se saltan, y quien llama lo dice.
    pub ilegibles: Vec<ListaIlegible>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListaIlegible {
    pub ruta: PathBuf,
    pub causa: String,
}

/// Todos los ficheros con huella de los repositorios consultados, también los que la búsqueda
/// descarta.
pub fn de_la_busqueda(detalles: &[DetalleDeRepositorio]) -> Vec<FicheroConocido> {
    detalles
        .iter()
        .flat_map(|detalle| {
            detalle.ficheros.iter().filter_map(|fichero| {
                Some(FicheroConocido {
                    repositorio: detalle.id.clone(),
                    fichero: fichero.nombre.clone(),
                    tamano: fichero.tamano,
                    huella: fichero.huella_sha256.clone()?,
                })
            })
        })
        .collect()
}

/// Los candidatos y los ya descargados con huella de cada lista de cada mes, también las
/// apartadas con la hora, de la más reciente a la más antigua.
pub fn de_las_listas(carpeta_de_revisiones: &Path) -> Resultado<DeLasListas> {
    let mut leidas = DeLasListas::default();
    for ruta in mes::todos_los_de(carpeta_de_revisiones, NOMBRE_DE_LA_LISTA)? {
        match ficheros::leer(&ruta).and_then(|texto| lista::leer(&texto)) {
            Ok(lista) => leidas.conocidos.extend(
                lista
                    .candidatos
                    .iter()
                    .chain(&lista.ya_descargados)
                    .filter_map(|entrada| {
                        let fichero = &entrada.fichero;
                        Some(FicheroConocido {
                            repositorio: fichero.repositorio.clone(),
                            fichero: fichero.fichero.clone(),
                            tamano: fichero.tamano,
                            huella: fichero.huella_sha256.clone()?,
                        })
                    }),
            ),
            Err(causa) => leidas.ilegibles.push(ListaIlegible {
                ruta,
                causa: causa.to_string(),
            }),
        }
    }
    Ok(leidas)
}

/// Rellena el origen de las entradas que no lo tienen con el primero de `conocidos` cuya huella
/// es la suya, sin distinguir mayúsculas. Devuelve cuántas completó.
pub fn completar_origenes(registro: &mut Registro, conocidos: &[FicheroConocido]) -> usize {
    let mut completadas = 0;
    for entrada in registro.modelo.iter_mut().filter(|e| e.origen.is_none()) {
        let Some(conocido) = conocidos
            .iter()
            .find(|c| c.huella.eq_ignore_ascii_case(&entrada.huella_sha256))
        else {
            continue;
        };
        entrada.origen = Some(Origen {
            repositorio: conocido.repositorio.clone(),
            fichero: conocido.fichero.clone(),
            tamano: conocido.tamano,
        });
        completadas += 1;
    }
    completadas
}

/// El registro con los orígenes que le faltan completados con las listas de búsquedas
/// anteriores: lo que la reposición sabrá de él antes de buscar, sin red. Si la carpeta de
/// revisiones no se deja leer, el registro tal cual; las listas ilegibles se saltan.
pub fn con_los_de_las_listas(registro: &Registro, carpeta_de_revisiones: &Path) -> Registro {
    let mut completo = registro.clone();
    if let Ok(listas) = de_las_listas(carpeta_de_revisiones) {
        completar_origenes(&mut completo, &listas.conocidos);
    }
    completo
}

/// Los modelos del registro que faltan en la carpeta de modelos, con lo que se sabe de su origen
/// sin red: el registro completado con las listas de `carpeta_de_revisiones` y clasificado como
/// en [`super::ausentes::clasificar`]. Es lo que cuenta el arranque; sin las listas decía «no se
/// sabe de qué repositorio salió» de un modelo que una búsqueda anterior sí conocía.
pub fn ausentes_al_arrancar(
    registro: &Registro,
    carpeta_de_revisiones: &Path,
    tarjeta: &str,
    en_modelos: &[(String, u64)],
    modelo_en_uso: Option<&str>,
) -> super::ausentes::Ausentes {
    let completo = con_los_de_las_listas(registro, carpeta_de_revisiones);
    super::ausentes::clasificar(&completo, tarjeta, en_modelos, modelo_en_uso)
}

/// Anota una tanda en el registro y, si se anotó, completa los orígenes que falten con las listas
/// de búsquedas guardadas: el candidato que se probó sale de una, con su huella y su tamaño. El
/// registro solo se vuelve a escribir si se completó alguno. Las listas que no se pueden leer se
/// saltan sin decir nada: anotar no depende de ellas.
pub fn anotar_con_su_origen(
    tanda: &Path,
    ruta_del_registro: &Path,
    carpeta_de_revisiones: &Path,
) -> Resultado<Desenlace> {
    let desenlace = anotar_tanda(tanda, ruta_del_registro)?;
    if matches!(desenlace, Desenlace::Anotada(..)) {
        let listas = de_las_listas(carpeta_de_revisiones)?;
        let mut registro = modelo::leer(ruta_del_registro)?;
        if completar_origenes(&mut registro, &listas.conocidos) > 0 {
            modelo::escribir(ruta_del_registro, &registro)?;
        }
    }
    Ok(desenlace)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;
    use crate::registro::modelo::EntradaDeModelo;
    use crate::revision::candidatos::FicheroCandidato;
    use crate::revision::catalogo::FicheroDelRepositorio;
    use crate::revision::lista::{EntradaDeLaLista, ListaDeCandidatos};

    const SITIO: &str = "https://huggingface.co";

    fn conocido(repositorio: &str, huella: &str) -> FicheroConocido {
        FicheroConocido {
            repositorio: repositorio.into(),
            fichero: "Modelo-Q4_K_M.gguf".into(),
            tamano: Some(5_000),
            huella: huella.into(),
        }
    }

    fn entrada(huella: &str) -> EntradaDeModelo {
        let mut e = entrada_de_ejemplo();
        e.huella_sha256 = huella.into();
        e
    }

    fn candidato(repositorio: &str, huella: Option<&str>) -> EntradaDeLaLista {
        EntradaDeLaLista::nueva(
            FicheroCandidato {
                repositorio: repositorio.into(),
                fichero: "Modelo-Q4_K_M.gguf".into(),
                cuantizacion: "Q4_K_M".into(),
                tamano: Some(5_000),
                huella_sha256: huella.map(str::to_string),
                licencia: None,
                ultima_modificacion: None,
                descargas: None,
            },
            SITIO,
        )
    }

    fn escribir_lista(
        carpeta: &Path,
        nombre: &str,
        candidatos: Vec<EntradaDeLaLista>,
        ya_descargados: Vec<EntradaDeLaLista>,
    ) {
        ficheros::crear_carpeta(carpeta).unwrap();
        let lista = ListaDeCandidatos {
            fecha: "2026-11-13".into(),
            tarjeta: "NVIDIA GeForce RTX 4070 Ti SUPER".into(),
            vram_mib: 16_376,
            cabe_mib: 14_000,
            candidatos,
            ya_descargados,
        };
        ficheros::escribir(&carpeta.join(nombre), &lista::escribir(&lista).unwrap()).unwrap();
    }

    fn repositorios(conocidos: &[FicheroConocido]) -> Vec<&str> {
        conocidos.iter().map(|c| c.repositorio.as_str()).collect()
    }

    #[test]
    fn completa_por_huella_sin_distinguir_mayusculas() {
        let mut registro = Registro {
            modelo: vec![entrada("abcdef")],
        };
        let completadas =
            completar_origenes(&mut registro, &[conocido("alguien/M-GGUF", "ABCDEF")]);
        assert_eq!(completadas, 1);
        assert_eq!(
            registro.modelo[0].origen,
            Some(Origen {
                repositorio: "alguien/M-GGUF".into(),
                fichero: "Modelo-Q4_K_M.gguf".into(),
                tamano: Some(5_000),
            })
        );
    }

    #[test]
    fn no_toca_un_origen_que_ya_esta() {
        let mut ya = entrada("abcdef");
        let origen = Origen {
            repositorio: "primero/M-GGUF".into(),
            fichero: "M.gguf".into(),
            tamano: None,
        };
        ya.origen = Some(origen.clone());
        let mut registro = Registro { modelo: vec![ya] };
        assert_eq!(
            completar_origenes(&mut registro, &[conocido("otro/M-GGUF", "abcdef")]),
            0
        );
        assert_eq!(registro.modelo[0].origen, Some(origen));
    }

    #[test]
    fn una_huella_desconocida_deja_el_origen_vacio() {
        let mut registro = Registro {
            modelo: vec![entrada("abcdef")],
        };
        assert_eq!(
            completar_origenes(&mut registro, &[conocido("alguien/M-GGUF", "123456")]),
            0
        );
        assert_eq!(registro.modelo[0].origen, None);
    }

    #[test]
    fn vale_el_primero_la_busqueda_antes_que_las_listas() {
        let raiz = tempfile::tempdir().unwrap();
        escribir_lista(
            &raiz.path().join("2026-11"),
            NOMBRE_DE_LA_LISTA,
            vec![candidato("de-la-lista/M-GGUF", Some("abcdef"))],
            vec![],
        );
        let busqueda = de_la_busqueda(&[DetalleDeRepositorio {
            id: "de-la-busqueda/M-GGUF".into(),
            ultima_modificacion: None,
            descargas: None,
            licencia: None,
            ficheros: vec![FicheroDelRepositorio {
                nombre: "Modelo-Q4_K_M.gguf".into(),
                tamano: Some(5_000),
                huella_sha256: Some("abcdef".into()),
            }],
        }]);
        let mut conocidos = busqueda;
        conocidos.extend(de_las_listas(raiz.path()).unwrap().conocidos);
        assert_eq!(
            repositorios(&conocidos),
            ["de-la-busqueda/M-GGUF", "de-la-lista/M-GGUF"]
        );
        let mut registro = Registro {
            modelo: vec![entrada("abcdef")],
        };
        completar_origenes(&mut registro, &conocidos);
        assert_eq!(
            registro.modelo[0]
                .origen
                .as_ref()
                .map(|o| o.repositorio.as_str()),
            Some("de-la-busqueda/M-GGUF")
        );
    }

    #[test]
    fn de_la_busqueda_solo_toma_los_ficheros_con_huella() {
        let conocidos = de_la_busqueda(&[DetalleDeRepositorio {
            id: "alguien/M-GGUF".into(),
            ultima_modificacion: None,
            descargas: None,
            licencia: None,
            ficheros: vec![
                FicheroDelRepositorio {
                    nombre: "README.md".into(),
                    tamano: Some(10),
                    huella_sha256: None,
                },
                FicheroDelRepositorio {
                    nombre: "M-Q4_K_M.gguf".into(),
                    tamano: Some(5_000),
                    huella_sha256: Some("abcdef".into()),
                },
            ],
        }]);
        assert_eq!(
            conocidos,
            [FicheroConocido {
                repositorio: "alguien/M-GGUF".into(),
                fichero: "M-Q4_K_M.gguf".into(),
                tamano: Some(5_000),
                huella: "abcdef".into(),
            }]
        );
    }

    #[test]
    fn lee_las_listas_de_todos_los_meses_y_las_apartadas_con_la_hora() {
        let raiz = tempfile::tempdir().unwrap();
        let octubre = raiz.path().join("2026-10");
        let noviembre = raiz.path().join("2026-11");
        escribir_lista(
            &octubre,
            "candidatos-081938.toml",
            vec![candidato("octubre-apartada/M-GGUF", Some("a1"))],
            vec![],
        );
        escribir_lista(
            &noviembre,
            NOMBRE_DE_LA_LISTA,
            vec![
                candidato("noviembre/candidato", Some("b1")),
                candidato("noviembre/sin-huella", None),
            ],
            vec![candidato("noviembre/ya-descargado", Some("b2"))],
        );
        escribir_lista(
            &noviembre,
            "candidatos-120000.toml",
            vec![candidato("noviembre-apartada/M-GGUF", Some("b3"))],
            vec![],
        );
        let leidas = de_las_listas(raiz.path()).unwrap();
        assert!(leidas.ilegibles.is_empty());
        assert_eq!(
            repositorios(&leidas.conocidos),
            [
                "noviembre/candidato",
                "noviembre/ya-descargado",
                "noviembre-apartada/M-GGUF",
                "octubre-apartada/M-GGUF",
            ]
        );
    }

    #[test]
    fn una_lista_ilegible_se_salta_y_se_dice() {
        let raiz = tempfile::tempdir().unwrap();
        let noviembre = raiz.path().join("2026-11");
        escribir_lista(
            &noviembre,
            "candidatos-120000.toml",
            vec![candidato("buena/M-GGUF", Some("a1"))],
            vec![],
        );
        let rota = noviembre.join(NOMBRE_DE_LA_LISTA);
        ficheros::escribir(&rota, "esto no es [una lista").unwrap();
        let leidas = de_las_listas(raiz.path()).unwrap();
        assert_eq!(repositorios(&leidas.conocidos), ["buena/M-GGUF"]);
        assert_eq!(leidas.ilegibles.len(), 1);
        assert_eq!(leidas.ilegibles[0].ruta, rota);
        assert!(
            leidas.ilegibles[0].causa.contains(NOMBRE_DE_LA_LISTA),
            "{:?}",
            leidas.ilegibles[0]
        );
    }

    #[test]
    fn el_registro_sabe_el_origen_que_dice_una_lista_anterior() {
        let raiz = tempfile::tempdir().unwrap();
        escribir_lista(
            &raiz.path().join("2026-11"),
            NOMBRE_DE_LA_LISTA,
            vec![candidato("de-la-lista/M-GGUF", Some("abcdef"))],
            vec![],
        );
        let registro = Registro {
            modelo: vec![entrada("abcdef"), entrada("otra")],
        };
        let completo = con_los_de_las_listas(&registro, raiz.path());
        let origenes: Vec<Option<&str>> = completo
            .modelo
            .iter()
            .map(|e| e.origen.as_ref().map(|o| o.repositorio.as_str()))
            .collect();
        assert_eq!(origenes, [Some("de-la-lista/M-GGUF"), None]);
    }

    #[test]
    fn al_arrancar_el_modelo_que_falta_sabe_el_origen_que_dice_una_lista() {
        let raiz = tempfile::tempdir().unwrap();
        escribir_lista(
            &raiz.path().join("2026-11"),
            NOMBRE_DE_LA_LISTA,
            vec![candidato("de-la-lista/M-GGUF", Some("abcdef"))],
            vec![],
        );
        let falta = entrada("abcdef");
        let (nombre, tarjeta) = (falta.nombre.clone(), falta.tarjeta.clone());
        let registro = Registro {
            modelo: vec![falta],
        };

        let ausentes = ausentes_al_arrancar(&registro, raiz.path(), &tarjeta, &[], Some(&nombre));

        let origenes: Vec<Option<&str>> = ausentes
            .utiles
            .iter()
            .map(|u| u.origen.as_ref().map(|o| o.repositorio.as_str()))
            .collect();
        assert_eq!(origenes, [Some("de-la-lista/M-GGUF")]);
    }

    #[test]
    fn sin_listas_el_registro_queda_como_estaba() {
        let raiz = tempfile::tempdir().unwrap();
        let registro = Registro {
            modelo: vec![entrada("abcdef")],
        };
        assert_eq!(
            con_los_de_las_listas(&registro, &raiz.path().join("no-esta")),
            registro
        );
    }

    #[test]
    fn sin_carpeta_de_revisiones_no_hay_listas() {
        let raiz = tempfile::tempdir().unwrap();
        assert_eq!(
            de_las_listas(&raiz.path().join("no-esta")).unwrap(),
            DeLasListas::default()
        );
    }

    /// Una tanda de ejemplo, que se puede anotar, en `raiz/banco`.
    fn tanda_en(raiz: &Path) -> PathBuf {
        let tanda = raiz.join("banco").join("2026-11-24-1000-modelo");
        crate::registro::anotar_tanda::pruebas::guardar_tanda(&tanda, true);
        tanda
    }

    #[test]
    fn anotar_una_tanda_de_un_candidato_de_la_lista_le_pone_su_origen() {
        let raiz = tempfile::tempdir().unwrap();
        let tanda = tanda_en(raiz.path());
        let huella = crate::banco::resultados::pruebas::tanda_de_ejemplo()
            .tanda
            .huella_sha256;
        let revisiones = raiz.path().join("revisiones");
        escribir_lista(
            &revisiones.join("2026-11"),
            NOMBRE_DE_LA_LISTA,
            vec![candidato("alguien/M-GGUF", Some(&huella.to_uppercase()))],
            vec![],
        );
        let ruta_del_registro = raiz.path().join("registro-de-modelos.toml");

        let desenlace = anotar_con_su_origen(&tanda, &ruta_del_registro, &revisiones).unwrap();

        assert!(matches!(desenlace, Desenlace::Anotada(..)));
        let leido = modelo::leer(&ruta_del_registro).unwrap();
        assert_eq!(leido.modelo.len(), 1);
        assert_eq!(
            leido.modelo[0].origen,
            Some(Origen {
                repositorio: "alguien/M-GGUF".into(),
                fichero: "Modelo-Q4_K_M.gguf".into(),
                tamano: Some(5_000),
            })
        );
    }

    #[test]
    fn una_tanda_de_un_modelo_que_no_esta_en_ninguna_lista_se_anota_sin_origen() {
        let raiz = tempfile::tempdir().unwrap();
        let tanda = tanda_en(raiz.path());
        let ruta_del_registro = raiz.path().join("registro-de-modelos.toml");

        let desenlace =
            anotar_con_su_origen(&tanda, &ruta_del_registro, &raiz.path().join("revisiones"))
                .unwrap();

        assert!(matches!(desenlace, Desenlace::Anotada(..)));
        let leido = modelo::leer(&ruta_del_registro).unwrap();
        assert_eq!(leido.modelo.len(), 1);
        assert_eq!(leido.modelo[0].origen, None);
    }
}
