//! La tabla de desempeño que la guía cuenta al agente de pago, compuesta desde el registro.
//!
//! Sustituye a la tabla escrita a mano que había en `plantillas/desempeno-medido.md`. **La regla
//! de entonces se mantiene: ninguna fila sale sin cifra y sin fecha.** Una clase que el banco no
//! pudo medir dice «sin medir», nunca una estimación.

use crate::banco::resultados::{FilaDeDesempeno, FILAS_DE_DESEMPENO};
use crate::config::{CapasGpu, MotorConfig};

use super::mensajes::{texto_de_cifra, SIN_MEDIR};
use super::modelo::{EntradaDeModelo, Registro};

/// El título de la sección, el mismo con cifras que sin ellas.
const TITULO: &str = "## En qué acierta y en qué no, medido";

/// La entrada de un modelo, por su nombre. Si se midió en varias tarjetas, la más reciente: la
/// fecha del banco («2026-11-12 12:19») se ordena bien como texto. Las retiradas no cuentan: la
/// guía deja de ofrecer un modelo retirado.
pub fn entrada_del_modelo<'a>(registro: &'a Registro, nombre: &str) -> Option<&'a EntradaDeModelo> {
    registro
        .modelo
        .iter()
        .filter(|entrada| entrada.nombre == nombre && entrada.retirado_el.is_none())
        .max_by(|a, b| a.medido_el.cmp(&b.medido_el))
}

/// La sección de la guía para el modelo configurado.
pub fn componer_desempeno(
    nombre_del_modelo: &str,
    entrada: Option<&EntradaDeModelo>,
    motor: &MotorConfig,
) -> String {
    match entrada {
        None => sin_medir(nombre_del_modelo),
        Some(entrada) => con_cifras(entrada, motor),
    }
}

fn sin_medir(nombre_del_modelo: &str) -> String {
    format!(
        "{TITULO}\n\n**Este modelo ({nombre_del_modelo}) no se ha medido con el banco de modelos**: no \
         hay cifras de lo que hace bien y lo que hace mal. Trátalo como desconocido: encárgale \
         tareas pequeñas y comprueba cada entrega.\n"
    )
}

fn con_cifras(entrada: &EntradaDeModelo, motor: &MotorConfig) -> String {
    let fecha = dia(&entrada.medido_el);
    let capas = entrada
        .capas_en_gpu
        .map_or_else(|| SIN_MEDIR.to_string(), |c| c.to_string());
    let mut texto = format!(
        "{TITULO}\n\nLo midió el banco de modelos de Programator el {fecha}, con {}, en {}, {capas} \
         capas en la GPU y {} tokens de contexto (tanda «{}»). Las notas van **de 0 a 5**. Una \
         fila que dice «{SIN_MEDIR}» no se pudo medir: no la des por buena.\n\n\
         | Clase de encargo | Nota (sobre 5) | Inventados | Fecha |\n|---|---|---|---|\n",
        entrada.nombre, entrada.tarjeta, entrada.contexto, entrada.tanda,
    );
    for clase in &entrada.clase {
        texto.push_str(&format!(
            "| {} | {} | {} | {fecha} |\n",
            clase.clase.titulo(),
            texto_de_cifra(clase.nota),
            clase.inventados,
        ));
    }
    texto.push_str(&format!(
        "\nVa a unos **{} tokens por segundo** generando y {} leyendo.\n",
        texto_de_cifra(entrada.generando_tokens_por_segundo),
        texto_de_cifra(entrada.leyendo_tokens_por_segundo),
    ));
    if let Some(aviso) = aviso_de_otra_configuracion(entrada, motor) {
        texto.push_str(&format!("\n{aviso}\n"));
    }
    texto
}

/// El título de la tabla de modelos por clase.
const TITULO_POR_CLASE: &str = "## Qué modelo atiende cada clase de encargo";
/// Lo que dice la tabla de una clase en la que ningún modelo aprobado tiene nota.
const SIN_NOTAS: &str = "ninguno tiene nota";
/// Quién atiende una clase sin ganador.
const EL_DE_POR_DEFECTO: &str = "el modelo por defecto";

/// La tabla de la guía con los modelos aprobados en cada clase y el que la atiende. `aprobado`
/// dice si una entrada cuenta (esta tarjeta, con su fichero); las retiradas no cuentan nunca.
/// `ganador_de` es la misma elección que hace el ciclo para un encargo de esa clase.
pub fn componer_modelos_por_clase(
    registro: &Registro,
    aprobado: impl Fn(&EntradaDeModelo) -> bool,
    ganador_de: impl Fn(FilaDeDesempeno) -> Option<String>,
) -> String {
    let aprobados = mas_recientes(
        registro
            .modelo
            .iter()
            .filter(|entrada| entrada.retirado_el.is_none() && aprobado(entrada)),
    );
    let mut texto = format!(
        "{TITULO_POR_CLASE}\n\nCon una línea `Clase:` en el encargo, Programator carga el modelo \
         que mejor hace esa clase según el banco. Sin ella, o si ningún modelo tiene nota en esa \
         clase, lo atiende {EL_DE_POR_DEFECTO}. Las notas van de 0 a 5.\n\n\
         | Se escribe | Clase de encargo | Modelos aprobados (nota) | Lo atiende |\n\
         |---|---|---|---|\n"
    );
    for fila in FILAS_DE_DESEMPENO {
        let notas: Vec<String> = aprobados
            .iter()
            .filter_map(|entrada| {
                let nota = entrada.clase.iter().find(|c| c.clase == fila)?.nota?;
                Some(format!(
                    "{} ({})",
                    entrada.nombre,
                    texto_de_cifra(Some(nota))
                ))
            })
            .collect();
        let lista = if notas.is_empty() {
            SIN_NOTAS.to_string()
        } else {
            notas.join(", ")
        };
        let atiende = ganador_de(fila).unwrap_or_else(|| EL_DE_POR_DEFECTO.to_string());
        texto.push_str(&format!(
            "| `{}` | {} | {lista} | **{atiende}** |\n",
            fila.nombre_corto(),
            fila.titulo()
        ));
    }
    texto
}

/// De cada modelo, su medida más reciente, en orden de nombre.
fn mas_recientes<'a>(
    entradas: impl Iterator<Item = &'a EntradaDeModelo>,
) -> Vec<&'a EntradaDeModelo> {
    let mut por_nombre: std::collections::BTreeMap<&str, &EntradaDeModelo> =
        std::collections::BTreeMap::new();
    for entrada in entradas {
        let sustituye = por_nombre
            .get(entrada.nombre.as_str())
            .is_none_or(|vieja| vieja.medido_el < entrada.medido_el);
        if sustituye {
            por_nombre.insert(&entrada.nombre, entrada);
        }
    }
    por_nombre.into_values().collect()
}

/// La fecha sin la hora: «2026-11-12 12:19» da «2026-11-12».
fn dia(medido_el: &str) -> &str {
    medido_el.split_whitespace().next().unwrap_or(medido_el)
}

/// Si Programator está configurado con otras capas u otro contexto que los medidos, la velocidad
/// será otra, y las respuestas también pueden cambiar. Con capas automáticas no se puede saber de
/// antemano cuántas habrá, así que solo se compara el contexto.
fn aviso_de_otra_configuracion(entrada: &EntradaDeModelo, motor: &MotorConfig) -> Option<String> {
    let otras_capas = match motor.capas_gpu {
        CapasGpu::Fijas(capas) => entrada.capas_en_gpu != Some(capas),
        CapasGpu::Auto => false,
    };
    let otro_contexto = motor.contexto != entrada.contexto;
    (otras_capas || otro_contexto).then(|| {
        format!(
            "**Ojo:** ahora Programator está configurado con {} y {} tokens de contexto, no como se \
             midió. La velocidad será otra, y las respuestas también pueden cambiar.",
            match motor.capas_gpu {
                CapasGpu::Fijas(capas) => format!("{capas} capas en la GPU"),
                CapasGpu::Auto => "las capas en automático".to_string(),
            },
            motor.contexto,
        )
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;

    fn motor(capas: CapasGpu, contexto: u32) -> MotorConfig {
        let mut config: crate::config::Config = toml::from_str(
            "[motor]\n\
             modelo = \"modelos/devstral.gguf\"\n",
        )
        .unwrap();
        config.motor.capas_gpu = capas;
        config.motor.contexto = contexto;
        config.motor
    }

    fn como_se_midio() -> MotorConfig {
        motor(CapasGpu::Fijas(34), 32768)
    }

    #[test]
    fn ninguna_fila_de_desempeno_puede_ir_sin_cifra_y_sin_fecha() {
        // La regla dura de la ficha del repertorio, que pasa de la plantilla escrita a mano a la
        // redacción desde el registro. Es lo que habría impedido que la fila de «diagnosticar»
        // —«lo mejor que hace»— sobreviviera a la medición que la desmentía: 0 de 12. Una clase
        // sin medir no puede salir en blanco ni con una estimación: dice «sin medir».
        let entrada = entrada_de_ejemplo();
        let texto = componer_desempeno(&entrada.nombre, Some(&entrada), &como_se_midio());

        let filas: Vec<&str> = texto
            .lines()
            .filter(|l| l.starts_with('|') && !l.contains("---") && !l.contains("Clase de encargo"))
            .collect();
        assert_eq!(filas.len(), entrada.clase.len(), "{texto}");

        for fila in filas {
            // «| clase | nota | inventados | fecha |»: seis trozos al partir por la barra.
            let celdas: Vec<&str> = fila.split('|').collect();
            assert_eq!(
                celdas.len(),
                6,
                "la fila no tiene las cuatro columnas: {fila}"
            );
            let nota = celdas[2].trim();
            assert!(
                nota.chars().any(|c| c.is_ascii_digit()) || nota == SIN_MEDIR,
                "la nota no trae cifra ni dice «{SIN_MEDIR}»: {fila}"
            );
            assert!(
                celdas[4].contains("2026") || celdas[4].contains("2027"),
                "la columna de fecha no trae ninguna fecha: {fila}"
            );
        }
    }

    #[test]
    fn con_entrada_la_tabla_dice_como_se_midio_y_la_escala() {
        let entrada = entrada_de_ejemplo();

        let texto = componer_desempeno(&entrada.nombre, Some(&entrada), &como_se_midio());

        assert!(texto.contains("2026-11-12"), "{texto}");
        assert!(
            texto.contains("NVIDIA GeForce RTX 4070 Ti SUPER"),
            "{texto}"
        );
        assert!(texto.contains("34 capas"), "{texto}");
        assert!(texto.contains("32768 tokens de contexto"), "{texto}");
        assert!(texto.contains("de 0 a 5"), "{texto}");
        assert!(
            texto.contains("| Función acotada | 5 | 0 | 2026-11-12 |"),
            "{texto}"
        );
        assert!(
            texto.contains("| Diagnosticar la causa de un fallo | sin medir |"),
            "{texto}"
        );
        assert!(texto.contains("10,5 tokens por segundo"), "{texto}");
        assert!(!texto.contains("Ojo"), "{texto}");
    }

    #[test]
    fn con_otras_capas_u_otro_contexto_se_avisa() {
        let entrada = entrada_de_ejemplo();

        let otras_capas = componer_desempeno(
            &entrada.nombre,
            Some(&entrada),
            &motor(CapasGpu::Fijas(20), 32768),
        );
        let otro_contexto = componer_desempeno(
            &entrada.nombre,
            Some(&entrada),
            &motor(CapasGpu::Auto, 16384),
        );
        let auto_mismo_contexto = componer_desempeno(
            &entrada.nombre,
            Some(&entrada),
            &motor(CapasGpu::Auto, 32768),
        );

        assert!(otras_capas.contains("Ojo"), "{otras_capas}");
        assert!(otras_capas.contains("20 capas"), "{otras_capas}");
        assert!(otro_contexto.contains("Ojo"), "{otro_contexto}");
        assert!(
            !auto_mismo_contexto.contains("Ojo"),
            "{auto_mismo_contexto}"
        );
    }

    #[test]
    fn sin_entrada_dice_que_no_esta_medido_y_no_da_cifras() {
        let texto = componer_desempeno("otro-modelo", None, &como_se_midio());

        assert!(texto.contains("otro-modelo"), "{texto}");
        assert!(texto.contains("no se ha medido"), "{texto}");
        assert!(!texto.contains('|'), "{texto}");
    }

    #[test]
    fn la_entrada_se_busca_por_nombre_y_gana_la_medida_mas_reciente() {
        let vieja = entrada_de_ejemplo();
        let mut nueva = vieja.clone();
        nueva.tarjeta = "otra tarjeta".into();
        nueva.medido_el = "2026-12-01 09:00".into();
        let mut otro = vieja.clone();
        otro.nombre = "otro".into();
        let registro = Registro {
            modelo: vec![vieja.clone(), nueva, otro],
        };

        let elegida = entrada_del_modelo(&registro, &vieja.nombre).unwrap();

        assert_eq!(elegida.tarjeta, "otra tarjeta");
        assert!(entrada_del_modelo(&registro, "no-esta").is_none());
    }

    #[test]
    fn una_entrada_retirada_no_sale_en_la_guia() {
        let mut retirada = entrada_de_ejemplo();
        retirada.retirado_el = Some("2026-12-01".into());
        let registro = Registro {
            modelo: vec![retirada.clone()],
        };

        assert!(entrada_del_modelo(&registro, &retirada.nombre).is_none());
    }

    /// La fila de la tabla por clase de `fila`, entera.
    fn fila_de(tabla: &str, fila: FilaDeDesempeno) -> String {
        let marca = format!("| `{}` |", fila.nombre_corto());
        tabla
            .lines()
            .find(|linea| linea.starts_with(&marca))
            .unwrap_or_else(|| {
                panic!(
                    "sin fila {marca}:
{tabla}"
                )
            })
            .to_string()
    }

    #[test]
    fn la_tabla_por_clase_lista_los_aprobados_con_su_nota_y_marca_quien_atiende() {
        let uno = entrada_de_ejemplo();
        let mut otro = entrada_de_ejemplo();
        otro.nombre = "qwen-coder".into();
        otro.clase[0].nota = Some(3.5);
        let registro = Registro {
            modelo: vec![uno.clone(), otro],
        };

        let tabla = componer_modelos_por_clase(
            &registro,
            |_| true,
            |fila| (fila == FilaDeDesempeno::FuncionAcotada).then(|| uno.nombre.clone()),
        );

        let acotada = fila_de(&tabla, FilaDeDesempeno::FuncionAcotada);
        assert!(
            acotada.contains(&format!("{} (5)", uno.nombre)),
            "{acotada}"
        );
        assert!(acotada.contains("qwen-coder (3,5)"), "{acotada}");
        assert!(
            acotada.ends_with(&format!("**{}** |", uno.nombre)),
            "{acotada}"
        );
        assert!(!tabla.contains("|||"));
    }

    #[test]
    fn una_clase_sin_notas_lo_dice_y_la_atiende_el_de_por_defecto() {
        let registro = Registro {
            modelo: vec![entrada_de_ejemplo()],
        };

        let tabla = componer_modelos_por_clase(&registro, |_| true, |_| None);

        let diagnostico = fila_de(&tabla, FilaDeDesempeno::Diagnostico);
        assert!(diagnostico.contains(SIN_NOTAS), "{diagnostico}");
        assert!(
            diagnostico.contains(&format!("**{EL_DE_POR_DEFECTO}**")),
            "{diagnostico}"
        );
    }

    #[test]
    fn ni_los_retirados_ni_los_no_aprobados_salen_en_la_tabla() {
        let mut retirado = entrada_de_ejemplo();
        retirado.retirado_el = Some("2026-11-20".into());
        let mut sin_fichero = entrada_de_ejemplo();
        sin_fichero.nombre = "sin-fichero".into();
        let registro = Registro {
            modelo: vec![retirado, sin_fichero],
        };

        let tabla = componer_modelos_por_clase(
            &registro,
            |entrada| entrada.nombre != "sin-fichero",
            |_| None,
        );

        let acotada = fila_de(&tabla, FilaDeDesempeno::FuncionAcotada);
        assert!(acotada.contains(SIN_NOTAS), "{acotada}");
    }

    #[test]
    fn de_cada_modelo_cuenta_su_medida_mas_reciente() {
        let mut vieja = entrada_de_ejemplo();
        vieja.clase[0].nota = Some(1.0);
        let mut nueva = entrada_de_ejemplo();
        nueva.medido_el = "2026-11-20 10:00".into();
        nueva.clase[0].nota = Some(4.0);
        let registro = Registro {
            modelo: vec![nueva, vieja],
        };

        let tabla = componer_modelos_por_clase(&registro, |_| true, |_| None);

        let acotada = fila_de(&tabla, FilaDeDesempeno::FuncionAcotada);
        assert!(
            acotada.contains("(4)") && !acotada.contains("(1)"),
            "{acotada}"
        );
    }

    #[test]
    fn la_tabla_tiene_una_fila_por_clase_con_su_nombre_corto() {
        let tabla = componer_modelos_por_clase(&Registro::default(), |_| true, |_| None);
        for fila in FILAS_DE_DESEMPENO {
            fila_de(&tabla, fila);
        }
    }
}
