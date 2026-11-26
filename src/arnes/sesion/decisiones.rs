//! Lo que la sesión decide al publicar un encargo, sin redactar nada: qué entrega pedía, si toca
//! avisar de que no la hubo y si un fallo al publicar el cierre obliga a reatenderlo. Los textos
//! están en `publicacion`.

/// El verbo con el que el modelo deja un fichero. Un encargo que no lo nombra no pide ningún
/// fichero.
pub(super) const VERBO_DE_PROPUESTA: &str = crate::arnes::ficha::verbo::ESCRIBIR_PROPUESTA;

/// Si el encargo pide dejar un fichero: nombra el verbo de las propuestas.
pub(super) fn entrega_pedida(texto_del_encargo: &str) -> EntregaPedida {
    if texto_del_encargo.contains(VERBO_DE_PROPUESTA) {
        EntregaPedida::Fichero
    } else {
        EntregaPedida::Libre
    }
}

/// Qué entrega pide un encargo. Es un tipo propio y no un `bool`, para que cada caso tenga nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntregaPedida {
    /// Nombra `escribir_propuesta`: una respuesta en texto no basta.
    Fichero,
    /// No pide nada concreto: si el modelo responde en texto, esa es la entrega.
    Libre,
}

/// Si un encargo que terminó sin herramientas de entrega merece el aviso: cuando pedía un fichero
/// o cuando el modelo no dijo nada. Si no pedía fichero y respondió en texto, esa es su entrega.
pub(super) fn avisa_sin_entrega(pedida: EntregaPedida, texto: &str) -> bool {
    pedida == EntregaPedida::Fichero || texto.trim().is_empty()
}

/// ¿Debe el fallo al publicar el **cierre** de un encargo impedir que el registro avance?
///
/// Si el encargo entregó al menos una propuesta y todos sus veredictos ya habían salido al canal
/// por la vía inmediata (`Canal::publicar_entrega` al escribir la propuesta, visto en uso
/// real), el fallo se queda en el resumen de cierre: la entrega ya quedó anotada. Reatender
/// el encargo solo produciría una segunda respuesta que contradice a la primera —exactamente lo que
/// se vio en uso real, dos pasadas sobre el mismo encargo 027-bis, la segunda sin
/// entrega y peor que la primera. Si no hubo ninguna propuesta (p. ej. `SinEntrega` o `Abortado`), o
/// si queda algún veredicto sin publicar, el fallo sí pierde información real y hay que reintentar.
pub(super) fn fallo_de_cierre_pierde_el_encargo(
    propuestas_de_este_encargo: usize,
    veredictos_pendientes: usize,
) -> bool {
    propuestas_de_este_encargo == 0 || veredictos_pendientes > 0
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_encargo_pide_fichero_si_nombra_escribir_propuesta() {
        assert_eq!(
            entrega_pedida("Para Programator: usa escribir_propuesta para a.rs"),
            EntregaPedida::Fichero
        );
        assert_eq!(
            entrega_pedida("Para Programator: ¿cuántas líneas tiene a.rs?"),
            EntregaPedida::Libre
        );
        assert!(avisa_sin_entrega(EntregaPedida::Fichero, "texto"));
        assert!(avisa_sin_entrega(EntregaPedida::Libre, " "));
        assert!(!avisa_sin_entrega(EntregaPedida::Libre, "texto"));
    }

    #[test]
    fn un_fallo_de_cierre_sin_propuestas_de_este_encargo_pierde_el_encargo() {
        assert!(fallo_de_cierre_pierde_el_encargo(0, 0));
    }

    #[test]
    fn un_fallo_de_cierre_con_veredictos_sin_publicar_pierde_el_encargo() {
        assert!(fallo_de_cierre_pierde_el_encargo(1, 1));
    }

    #[test]
    fn un_fallo_de_cierre_con_la_propuesta_ya_anotada_no_pierde_el_encargo() {
        assert!(!fallo_de_cierre_pierde_el_encargo(1, 0));
    }
}
