//! La redacción del informe de una tanda en Markdown, para leer: los datos de la tanda y las
//! tablas por encargo, por familia y por clase de trabajo, y la velocidad del motor. Solo compone
//! texto a partir de los resultados; no lee ni escribe ningún fichero.

use super::estadisticas::{
    calcular_clases, nota_de_familia, resumir_encargos, velocidad_de_generacion,
    velocidad_de_lectura, ResumenDeEncargo, FAMILIAS,
};
use super::persistencia::NOMBRE_RESULTADOS;
use super::{DatosDeTanda, ResultadoDeSemilla, ResultadosDeTanda};
use crate::argumentos::OPCION_BANCO_INFORME;
use crate::banco::config::Aislamiento;
use crate::banco::ficha::{ClaseDeUso, Familia};
use crate::banco::mensajes;
use crate::banco::puntuacion::NOTA_MAXIMA;
use crate::concordancia::{cantidad, segun};
use crate::plataforma::NOMBRE_DEL_EJECUTABLE;

/// Cuántos caracteres de la huella SHA-256 se muestran en el informe; el TOML la guarda entera.
const CARACTERES_DE_HUELLA: usize = 12;

/// Un número para leer, con coma decimal.
pub(crate) fn decimal(valor: f64, decimales: usize) -> String {
    format!("{valor:.decimales$}").replace('.', ",")
}

pub(crate) fn nombre_de_familia(familia: Familia) -> &'static str {
    match familia {
        Familia::Oficio => "oficio",
        Familia::Criterio => "criterio",
        Familia::Uso => "uso",
    }
}

/// La cabecera de una columna de notas, con la escala: «2,8» no dice nada sin saber sobre cuánto.
fn cabecera_de_nota(nombre: &str) -> String {
    format!("{nombre} (sobre {NOTA_MAXIMA})")
}

fn texto_de_nota(nota: Option<f64>, pendiente: bool) -> String {
    match (pendiente, nota) {
        (true, _) => "pendiente".into(),
        (false, Some(n)) => decimal(n, 1),
        (false, None) => "no evaluado".into(),
    }
}

/// Cómo se aisló la comprobación local, para el informe. Sin dato, nada: la tanda es anterior.
fn linea_de_aislamiento(aislamiento: Option<&str>) -> String {
    let Some(nombre) = aislamiento else {
        return String::new();
    };
    match Aislamiento::desde_nombre(nombre) {
        Some(Aislamiento::IntegridadBaja) => {
            "- Aislada con integridad baja y en un Job de Windows: no podía \
                                    escribir fuera de su carpeta, pero sí usar la red\n"
                .into()
        }
        Some(Aislamiento::EspacioDeNombres) => {
            "- Aislada en un espacio de nombres sin red\n".into()
        }
        Some(Aislamiento::Ninguno) => {
            "- **Sin aislar**: el código del modelo corrió con todos los permisos \
                            del usuario\n"
                .into()
        }
        None => format!("- Aislamiento: {nombre}\n"),
    }
}

/// Dónde se ejecutaron las pruebas ocultas y, si fue en este equipo, lo que quedó fuera.
fn linea_de_pruebas_ocultas(datos: &DatosDeTanda) -> String {
    let Some(maquina) = &datos.maquina_de_comprobacion else {
        return "- Pruebas ocultas: no se ejecutó código del modelo; lo que se puntúa con ellas \
                sale «no evaluado»\n"
            .into();
    };
    // Solo la comprobación local anota el sistema.
    let Some(sistema) = &datos.sistema_de_comprobacion else {
        return format!("- Pruebas ocultas: en {maquina}\n");
    };
    let donde = format!(
        "- Pruebas ocultas: en el equipo que pasó la tanda ({sistema}), dentro de su carpeta\n{}",
        linea_de_aislamiento(datos.aislamiento.as_deref())
    );
    if datos.rastro_fuera.is_empty() {
        return format!("{donde}- Fuera de la carpeta de la tanda no quedó rastro\n");
    }
    let lineas: String = datos
        .rastro_fuera
        .iter()
        .map(|ruta| format!("  - {ruta}\n"))
        .collect();
    format!(
        "{donde}- **Fuera de la carpeta de la tanda apareció esto** (no se ha borrado nada):\n\
         {lineas}"
    )
}

/// El informe en Markdown.
pub fn componer_informe(resultados: &ResultadosDeTanda) -> String {
    let resumenes = resumir_encargos(&resultados.encargos);
    let mut texto = cabecera_del_informe(resultados);
    texto.push_str(&seccion_por_encargo(&resumenes));
    texto.push_str(&seccion_por_familia(&resumenes));
    texto.push_str(&seccion_por_clase(resultados));
    texto.push_str(&seccion_de_velocidad(resultados));
    texto
}

/// El título, el aviso de rúbricas pendientes y los datos de la tanda.
fn cabecera_del_informe(resultados: &ResultadosDeTanda) -> String {
    let datos = &resultados.tanda;
    let pendientes = resultados.pendientes();
    let mut texto = format!("# Banco de modelos: {}\n\n", datos.modelo);

    if pendientes > 0 {
        texto.push_str(&format!(
            "> **{} {} por puntuar.** Rellena su línea «Nota:» y rehaz el informe con \
             `{NOMBRE_DEL_EJECUTABLE} {OPCION_BANCO_INFORME} {NOMBRE_RESULTADOS}`. Hasta \
             entonces no se da la media de criterio.\n\n",
            segun(pendientes, "Queda", "Quedan"),
            cantidad(pendientes, "rúbrica", "rúbricas"),
        ));
    }

    let huella: String = datos
        .huella_sha256
        .chars()
        .take(CARACTERES_DE_HUELLA)
        .collect();
    texto.push_str(&format!(
        "- Fecha: {}\n- Modelo: {} (SHA-256 {huella}…)\n- Motor: {}\n- Programator: {}\n\
         - Semillas: {}, temperatura {}\n",
        datos.fecha,
        datos.modelo,
        datos.compilacion_motor,
        datos.version_programator,
        mensajes::semillas_legibles(&datos.semillas),
        decimal(datos.temperatura, 2),
    ));
    texto.push_str(&linea_de_pruebas_ocultas(datos));
    if let (Some(capas), Some(vram), Some(carga)) = (
        datos.capas_en_gpu,
        datos.vram_ocupada_mib,
        datos.segundos_de_carga,
    ) {
        texto.push_str(&format!(
            "- Carga: {capas} capas en la GPU, {vram} MiB de VRAM, {} s\n",
            decimal(carga, 1)
        ));
    }
    texto
}

/// La tabla con una fila por encargo.
fn seccion_por_encargo(resumenes: &[ResumenDeEncargo]) -> String {
    let mut texto = format!(
        "\n## Por encargo\n\n| Encargo | Familia | {} | Dispersión | Semillas | Detalle |\n\
         |---|---|---|---|---|---|\n",
        cabecera_de_nota("Nota")
    );
    for r in resumenes {
        let media = r.resumen.and_then(|s| s.media);
        let dispersion = r
            .resumen
            .and_then(|s| s.dispersion)
            .map(|d| decimal(d, 2))
            .unwrap_or_default();
        let semillas = r
            .resumen
            .map(|s| format!("{} de {}", s.evaluadas, s.evaluadas + s.no_evaluadas))
            .unwrap_or_else(|| format!("{} pendientes", r.pendientes));
        texto.push_str(&format!(
            "| {} {} | {} | {} | {dispersion} | {semillas} | {} |\n",
            r.id,
            r.titulo,
            nombre_de_familia(r.familia),
            texto_de_nota(media, r.pendientes > 0),
            detalle_del_encargo(r),
        ));
    }
    texto
}

/// La columna «Detalle» de un encargo: lo que se contó para ponerle la nota.
fn detalle_del_encargo(r: &ResumenDeEncargo) -> String {
    let mut detalle = Vec::new();
    if let Some((superadas, total)) = r.superadas {
        detalle.push(format!("{superadas} de {total} comprobaciones"));
    }
    if let Some((aciertos, sembrados)) = r.aciertos {
        detalle.push(format!("{aciertos} de {sembrados} defectos"));
    }
    if r.aciertos.is_some() || r.clase == Some(ClaseDeUso::FicheroLimpio) {
        detalle.push(format!("{} inventados", r.inventados));
    }
    if r.por_rubrica > 0 {
        detalle.push(mensajes::detalle_puntuada_con_rubrica(r.por_rubrica));
    }
    if !r.motivos_no_evaluado.is_empty() {
        detalle.push(format!("no evaluado: {}", r.motivos_no_evaluado.join("; ")));
    }
    if !r.intentos_de_red.is_empty() {
        detalle.push(mensajes::detalle_intentos_de_red(&r.intentos_de_red));
    }
    detalle.join(", ")
}

/// La nota media de cada familia que tiene encargos.
fn seccion_por_familia(resumenes: &[ResumenDeEncargo]) -> String {
    let mut texto = format!(
        "\n## Por familia\n\n| Familia | {} |\n|---|---|\n",
        cabecera_de_nota("Nota media")
    );
    for familia in FAMILIAS {
        let de_la_familia: Vec<&ResumenDeEncargo> =
            resumenes.iter().filter(|r| r.familia == familia).collect();
        if de_la_familia.is_empty() {
            continue;
        }
        let pendiente = de_la_familia.iter().any(|r| r.pendientes > 0);
        texto.push_str(&format!(
            "| {} | {} |\n",
            nombre_de_familia(familia),
            texto_de_nota(nota_de_familia(resumenes, familia), pendiente),
        ));
    }
    texto
}

/// Las filas que se anotan en el registro de modelos.
fn seccion_por_clase(resultados: &ResultadosDeTanda) -> String {
    let mut texto = format!(
        "\n## Por clase de trabajo\n\nLas filas que se anotan en el registro de modelos y que la \
         guía cuenta al agente de pago.\n\n\
         | Clase de encargo | {} | Inventados | Tokens/s generando |\n|---|---|---|---|\n",
        cabecera_de_nota("Nota")
    );
    for clase in calcular_clases(&resultados.encargos) {
        texto.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            clase.fila.titulo(),
            texto_de_nota(clase.nota_media, clase.pendientes > 0),
            clase.inventados,
            clase
                .tokens_por_segundo
                .map(|v| decimal(v, 1))
                .unwrap_or_default(),
        ));
    }
    texto
}

/// El tiempo medio y la velocidad del motor en la tanda.
fn seccion_de_velocidad(resultados: &ResultadosDeTanda) -> String {
    let semillas: Vec<&ResultadoDeSemilla> = resultados
        .encargos
        .iter()
        .flat_map(|e| e.semillas.iter())
        .collect();
    let mut texto = String::from("\n## Velocidad\n\n");
    if !semillas.is_empty() {
        let media = semillas.iter().map(|s| s.segundos).sum::<f64>() / semillas.len() as f64;
        texto.push_str(&format!(
            "- Tiempo medio por encargo: {} s\n",
            decimal(media, 0)
        ));
    }
    let sin_cifra = crate::unidades::SIN_MEDIR.to_string();
    texto.push_str(&format!(
        "- Leyendo: {} tokens/s\n- Generando: {} tokens/s\n",
        velocidad_de_lectura(semillas.iter().copied())
            .map(|v| decimal(v, 1))
            .unwrap_or_else(|| sin_cifra.clone()),
        velocidad_de_generacion(semillas.iter().copied())
            .map(|v| decimal(v, 1))
            .unwrap_or(sin_cifra),
    ));
    let sin_consumo: usize = semillas.iter().map(|s| s.respuestas_sin_consumo).sum();
    if sin_consumo > 0 {
        texto.push_str(&format!(
            "- {sin_consumo} respuestas del motor no trajeron su consumo: las cifras de tokens se \
             quedan cortas.\n"
        ));
    }
    texto
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::resultados::pruebas::{semilla, tanda_de_ejemplo};
    use crate::banco::resultados::{
        componer_toml, leer_toml, EstadoDeNota, PuntuadaCon, FILAS_DE_DESEMPENO,
    };
    use crate::banco::rubrica::nombre_del_fichero;

    #[test]
    fn el_informe_dice_donde_se_comprobo_y_lo_que_quedo_fuera() {
        let mut resultados = tanda_de_ejemplo();
        resultados.tanda.maquina_de_comprobacion = Some("maquina-linux".into());
        assert!(componer_informe(&resultados).contains("- Pruebas ocultas: en maquina-linux\n"));

        resultados.tanda.maquina_de_comprobacion = Some("local".into());
        resultados.tanda.sistema_de_comprobacion = Some("windows".into());
        let sin_rastro = componer_informe(&resultados);
        assert!(sin_rastro.contains("en el equipo que pasó la tanda (windows)"));
        assert!(sin_rastro.contains("no quedó rastro"));

        resultados.tanda.rastro_fuera = vec!["C:/Users/x/.nuget/packages/algo".into()];
        let con_rastro = componer_informe(&resultados);
        assert!(con_rastro.contains("apareció esto"), "{con_rastro}");
        assert!(con_rastro.contains("  - C:/Users/x/.nuget/packages/algo\n"));
    }

    #[test]
    fn el_informe_marca_el_encargo_y_la_semilla_que_intentaron_usar_la_red() {
        let mut resultados = tanda_de_ejemplo();
        assert!(!componer_informe(&resultados).contains("intentó conectarse"));

        resultados.encargos[0].semillas[0].intentos_de_red = vec!["pypi.org:443".into()];
        let informe = componer_informe(&resultados);

        let fila = informe
            .lines()
            .find(|linea| linea.starts_with("| E03"))
            .unwrap();
        assert!(
            fila.contains(
                "⚠️ intentó conectarse a: pypi.org:443 (semilla 42); cuenta como fallida"
            ),
            "{fila}"
        );
        // Se guarda y se vuelve a leer; una tanda sin intentos no escribe la clave.
        let texto = componer_toml(&resultados).unwrap();
        assert_eq!(leer_toml(&texto).unwrap(), resultados);
        assert_eq!(texto.matches("intentos_de_red").count(), 1);
    }

    #[test]
    fn el_informe_dice_que_semillas_de_codigo_se_puntuaron_con_la_rubrica() {
        // Las dos notas no son comparables: el informe lo dice.
        let mut resultados = tanda_de_ejemplo();
        resultados.encargos[0].semillas[0].puntuada_con = Some(PuntuadaCon::PruebasOcultas);
        assert!(!componer_informe(&resultados).contains("con la rúbrica, leyendo el código"));

        resultados.encargos[0].semillas[1] = ResultadoDeSemilla {
            puntuada_con: Some(PuntuadaCon::Rubrica),
            rubrica: Some(format!("rubricas/{}", nombre_del_fichero("E03", 43))),
            ..semilla(43, EstadoDeNota::Evaluada, Some(2.0))
        };
        let informe = componer_informe(&resultados);

        let fila = informe
            .lines()
            .find(|linea| linea.starts_with("| E03"))
            .unwrap();
        assert!(
            fila.contains("1 semilla puntuada con la rúbrica, leyendo el código sin ejecutarlo"),
            "{fila}"
        );
        assert!(fila.contains("no es comparable"), "{fila}");
    }

    #[test]
    fn el_informe_dice_como_se_aislo_y_avisa_si_no_se_aislo() {
        let mut resultados = tanda_de_ejemplo();
        resultados.tanda.maquina_de_comprobacion = Some("local".into());
        resultados.tanda.sistema_de_comprobacion = Some("windows".into());
        let con = |aislamiento: Option<&str>, resultados: &mut ResultadosDeTanda| {
            resultados.tanda.aislamiento = aislamiento.map(String::from);
            componer_informe(resultados)
        };

        assert!(con(Some("integridad_baja"), &mut resultados).contains("sí usar la red"));
        assert!(con(Some("espacio_de_nombres"), &mut resultados).contains("sin red"));
        assert!(con(Some("ninguno"), &mut resultados).contains("**Sin aislar**"));
        assert!(!con(None, &mut resultados).contains("islad"));
    }

    #[test]
    fn con_rubricas_pendientes_el_informe_lo_avisa_y_no_da_la_media_de_criterio() {
        let informe = componer_informe(&tanda_de_ejemplo());

        assert!(informe.contains("Queda 1 rúbrica por puntuar"));
        assert!(informe.contains("| criterio | pendiente |"));
        assert!(informe.contains("| oficio | 3,5 |"));
    }

    #[test]
    fn las_tres_tablas_de_notas_dicen_sobre_cuanto_se_puntua() {
        // No es lo mismo 2,8/10 que 2,8/5.
        let informe = componer_informe(&tanda_de_ejemplo());

        assert!(
            informe.contains("| Familia | Nota (sobre 5) |"),
            "{informe}"
        );
        assert!(informe.contains("| Familia | Nota media (sobre 5) |"));
        assert!(informe.contains("| Clase de encargo | Nota (sobre 5) |"));
    }

    #[test]
    fn el_informe_dice_por_que_no_se_evaluo_una_semilla() {
        // Tanda del 12/11/2026: todo el código salió «no evaluado» y el informe no decía por qué.
        let informe = componer_informe(&tanda_de_ejemplo());

        assert!(
            informe.contains("no evaluado: falta en este equipo: rust"),
            "{informe}"
        );
    }

    #[test]
    fn sin_pendientes_no_hay_aviso() {
        let mut resultados = tanda_de_ejemplo();
        resultados
            .encargos
            .retain(|e| e.familia != Familia::Criterio);

        assert!(!componer_informe(&resultados).contains("Quedan"));
    }

    #[test]
    fn la_tabla_de_clases_tiene_las_seis_filas_en_su_orden() {
        // Hasta el 12/11/2026 el orden lo fijaba plantillas/desempeno-medido.md, escrita a mano.
        // Se retiró con el registro de modelos: ahora lo fija FILAS_DE_DESEMPENO, que comparten
        // el informe, el registro y la guía.
        let informe = componer_informe(&tanda_de_ejemplo());
        let filas: Vec<usize> = FILAS_DE_DESEMPENO
            .iter()
            .map(|f| informe.find(&format!("| {} |", f.titulo())).unwrap())
            .collect();
        assert_eq!(filas.len(), 6);
        assert!(filas.windows(2).all(|par| par[0] < par[1]));
    }
}
