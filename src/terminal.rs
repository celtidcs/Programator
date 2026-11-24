//! La terminal por la que el dominio cuenta lo que pasa, como un rasgo que se inyecta.
//!
//! **Por qué existe.** Si la sesión, el banco, el motor y el protocolo avisaran con `println!` y
//! `eprintln!` en mitad de su lógica, el dominio escribiría directamente en la terminal y lo que
//! avisa no lo podría ver ninguna prueba. Por eso reciben un [`Terminal`]:
//! `main`, que es la capa de presentación, construye [`TerminalReal`] y lo pasa hacia abajo, y las
//! pruebas pasan una terminal de mentira que guarda lo recibido para comprobarlo.
//!
//! Los textos no se componen aquí: siguen en los módulos de mensajes de cada capa. Este módulo
//! solo decide por dónde salen.

/// Por dónde sale un texto que el dominio quiere contar a la persona.
pub trait Terminal {
    /// La salida normal: lo que antes iba por `println!`.
    fn decir(&self, texto: &str);
    /// La salida de errores: lo que antes iba por `eprintln!`.
    fn avisar(&self, texto: &str);
}

/// La terminal de verdad: la salida estándar y la de errores del proceso. Es el único sitio de la
/// biblioteca, fuera de las pruebas, donde quedan `println!` y `eprintln!`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalReal;

impl Terminal for TerminalReal {
    fn decir(&self, texto: &str) {
        println!("{texto}");
    }

    fn avisar(&self, texto: &str) {
        eprintln!("{texto}");
    }
}

/// A cuál de las dos salidas fue un texto.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flujo {
    /// La salida normal ([`Terminal::decir`]).
    Normal,
    /// La salida de errores ([`Terminal::avisar`]).
    Errores,
}

/// Una terminal para las pruebas: no escribe nada y guarda, en orden, lo que recibe y por dónde.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct TerminalDeMentira {
    recibido: std::cell::RefCell<Vec<(Flujo, String)>>,
}

#[cfg(test)]
impl TerminalDeMentira {
    /// Lo recibido hasta ahora, en orden, con la salida por la que fue cada texto.
    pub(crate) fn capturado(&self) -> Vec<(Flujo, String)> {
        self.recibido.borrow().clone()
    }

    /// Solo los textos que fueron por la salida de errores, en orden.
    pub(crate) fn avisos(&self) -> Vec<String> {
        self.capturado()
            .into_iter()
            .filter(|(flujo, _)| *flujo == Flujo::Errores)
            .map(|(_, texto)| texto)
            .collect()
    }
}

#[cfg(test)]
impl Terminal for TerminalDeMentira {
    fn decir(&self, texto: &str) {
        self.recibido
            .borrow_mut()
            .push((Flujo::Normal, texto.to_string()));
    }

    fn avisar(&self, texto: &str) {
        self.recibido
            .borrow_mut()
            .push((Flujo::Errores, texto.to_string()));
    }
}
