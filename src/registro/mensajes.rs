//! Lo que `--registro-anotar` cuenta por la terminal, compuesto en un solo sitio.

use std::path::Path;

use crate::banco::mensajes::{
    NOMBRE_CAPAS_EN_GPU, NOMBRE_COMPILACION_DEL_MOTOR, NOMBRE_VERSION_DE_PROGRAMATOR,
};
use crate::banco::resultados::{decimal, nombre_de_familia};
use crate::error::Error;

use super::anotar::{Anotacion, Cambio, MotivoParaNoAnotar, QueCambia, Valor};
use super::modelo::EntradaDeModelo;

/// Lo que se dice cuando no hay cifra.
pub const SIN_MEDIR: &str = "sin medir";

/// Una cifra para leer: entera si lo es, y si no con un decimal y coma.
pub fn texto_de_cifra(cifra: Option<f64>) -> String {
    match cifra {
        None => SIN_MEDIR.to_string(),
        Some(valor) if valor.fract() == 0.0 => format!("{valor:.0}"),
        Some(valor) => decimal(valor, 1),
    }
}

pub fn mensaje_anotada(
    anotacion: &Anotacion,
    entrada: &EntradaDeModelo,
    registro: &Path,
) -> String {
    let cabecera = format!(
        "✅ Anotado en «{}»: {} en {}, de la tanda {}.\n   Notas sobre 5: oficio {}; criterio {}; \
         uso {}.",
        registro.display(),
        entrada.nombre,
        entrada.tarjeta,
        entrada.tanda,
        texto_de_cifra(entrada.familias.oficio),
        texto_de_cifra(entrada.familias.criterio),
        texto_de_cifra(entrada.familias.uso),
    );
    match anotacion {
        Anotacion::Nueva => cabecera,
        Anotacion::Sustituye(cambios) if cambios.is_empty() => format!(
            "{cabecera}\n   Sustituye a la entrada anterior del mismo modelo y la misma tarjeta, y \
             ninguna cifra cambia."
        ),
        Anotacion::Sustituye(cambios) => format!(
            "{cabecera}\n   Sustituye a la entrada anterior del mismo modelo y la misma tarjeta. \
             Cambia:\n{}",
            cambios.iter().map(linea_de_cambio).collect::<String>()
        ),
    }
}

fn linea_de_cambio(cambio: &Cambio) -> String {
    format!(
        "   - {}: {} → {}\n",
        que_cambia(cambio.que),
        texto_de_valor(&cambio.antes),
        texto_de_valor(&cambio.despues)
    )
}

fn que_cambia(que: QueCambia) -> String {
    match que {
        QueCambia::Tanda => "la tanda".into(),
        QueCambia::CompilacionDelMotor => NOMBRE_COMPILACION_DEL_MOTOR.into(),
        QueCambia::VersionDeProgramator => NOMBRE_VERSION_DE_PROGRAMATOR.into(),
        QueCambia::CapasEnGpu => NOMBRE_CAPAS_EN_GPU.into(),
        QueCambia::Contexto => "el contexto".into(),
        QueCambia::Generando => "tokens por segundo generando".into(),
        QueCambia::Leyendo => "tokens por segundo leyendo".into(),
        QueCambia::Familia(familia) => format!("la nota de {}", nombre_de_familia(familia)),
        QueCambia::Clase(fila) => format!("la nota de «{}»", fila.titulo()),
    }
}

fn texto_de_valor(valor: &Valor) -> String {
    match valor {
        Valor::Texto(texto) => texto.clone(),
        Valor::Cifra(cifra) => texto_de_cifra(*cifra),
    }
}

pub fn mensaje_no_se_anota(motivo: &MotivoParaNoAnotar, tanda: &Path) -> String {
    let porque = match motivo {
        MotivoParaNoAnotar::QuedanRubricas(cuantas) => format!(
            "quedan {cuantas} rúbricas por puntuar, y sin ellas la nota de su familia saldría de \
             menos encargos. Puntúalas y vuelve a anotarla"
        ),
        MotivoParaNoAnotar::FaltaLaTarjeta => "no guarda en qué tarjeta gráfica se midió, y sin \
             eso las cifras no dicen de qué máquina son. Las tandas anteriores al 12/11/2026 no \
             la guardan: pasa una tanda nueva"
            .to_string(),
        MotivoParaNoAnotar::FaltanLosAjustesDelMotor => "no guarda el contexto ni la caché con \
             que se midió. Las tandas anteriores al 12/11/2026 no los guardan: pasa una tanda \
             nueva"
            .to_string(),
    };
    format!("🔴 La tanda «{}» no se anota: {porque}.", tanda.display())
}

pub fn mensaje_registro_ilegible(fallo: &Error) -> String {
    format!(
        "⚠️ No se pudo leer el registro de modelos ({fallo}). Programator arranca igual, pero la \
         guía dirá que el modelo no está medido. El registro lo escribe «--registro-anotar»: si \
         está roto, bórralo y vuelve a anotar las tandas."
    )
}

pub fn mensaje_fallo(fallo: &Error) -> String {
    format!("🔴 No se pudo anotar en el registro de modelos: {fallo}.")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::banco::ficha::Familia;
    use crate::registro::modelo::pruebas::entrada_de_ejemplo;

    #[test]
    fn una_entrada_nueva_dice_donde_que_modelo_y_sus_notas() {
        let texto = mensaje_anotada(
            &Anotacion::Nueva,
            &entrada_de_ejemplo(),
            Path::new("registro-de-modelos.toml"),
        );

        assert!(texto.contains("registro-de-modelos.toml"), "{texto}");
        assert!(texto.contains("devstral-small-2-24b-Q4_K_M"), "{texto}");
        assert!(
            texto.contains("NVIDIA GeForce RTX 4070 Ti SUPER"),
            "{texto}"
        );
        assert!(
            texto.contains("oficio 1,8; criterio 2,1; uso 2,8"),
            "{texto}"
        );
        assert!(!texto.contains("Sustituye"), "{texto}");
    }

    #[test]
    fn una_sustitucion_dice_cada_cifra_que_cambia() {
        let anotacion = Anotacion::Sustituye(vec![
            Cambio {
                que: QueCambia::Familia(Familia::Uso),
                antes: Valor::Cifra(Some(2.8)),
                despues: Valor::Cifra(Some(3.0)),
            },
            Cambio {
                que: QueCambia::Contexto,
                antes: Valor::Cifra(Some(16384.0)),
                despues: Valor::Cifra(None),
            },
        ]);

        let texto = mensaje_anotada(&anotacion, &entrada_de_ejemplo(), Path::new("r.toml"));

        assert!(texto.contains("la nota de uso: 2,8 → 3"), "{texto}");
        assert!(texto.contains("el contexto: 16384 → sin medir"), "{texto}");
    }

    #[test]
    fn una_sustitucion_sin_cambios_lo_dice() {
        let texto = mensaje_anotada(
            &Anotacion::Sustituye(Vec::new()),
            &entrada_de_ejemplo(),
            Path::new("r.toml"),
        );

        assert!(texto.contains("ninguna cifra cambia"), "{texto}");
    }

    #[test]
    fn cada_motivo_para_no_anotar_dice_que_hacer() {
        let tanda = Path::new("resultados/2026-11-11");
        for (motivo, remedio) in [
            (MotivoParaNoAnotar::QuedanRubricas(3), "Puntúalas"),
            (MotivoParaNoAnotar::FaltaLaTarjeta, "pasa una tanda nueva"),
            (
                MotivoParaNoAnotar::FaltanLosAjustesDelMotor,
                "pasa una tanda nueva",
            ),
        ] {
            let texto = mensaje_no_se_anota(&motivo, tanda);

            assert!(texto.contains("2026-11-11"), "{texto}");
            assert!(texto.contains(remedio), "{texto}");
        }
    }
}
