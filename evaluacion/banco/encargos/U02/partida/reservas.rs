//! Reservas de habitaciones de un hotel pequeño.
//!
//! Las fechas son días numerados desde el 1 de enero, para no depender de una biblioteca de fechas.
//! Una reserva ocupa las noches desde `entrada` hasta la víspera de `salida`.

/// Personas que caben como máximo en una habitación.
pub const MAXIMO_PERSONAS: u32 = 4;

/// Días de antelación con los que una cancelación se reembolsa entera.
pub const DIAS_PARA_REEMBOLSO_COMPLETO: u32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    Confirmada,
    Cancelada,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reserva {
    pub habitacion: u32,
    pub entrada: u32,
    pub salida: u32,
    pub personas: u32,
    pub precio_noche: u32,
    pub estado: Estado,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorReserva {
    DemasiadasPersonas { personas: u32 },
    HabitacionOcupada { habitacion: u32 },
}

pub struct Hotel {
    reservas: Vec<Reserva>,
}

impl Hotel {
    pub fn nuevo() -> Self {
        Self { reservas: Vec::new() }
    }

    pub fn reservar(
        &mut self,
        habitacion: u32,
        entrada: u32,
        salida: u32,
        personas: u32,
        precio_noche: u32,
    ) -> Result<usize, ErrorReserva> {
        if personas > MAXIMO_PERSONAS {
            return Err(ErrorReserva::DemasiadasPersonas { personas });
        }
        if self.ocupada(habitacion, entrada, salida) {
            return Err(ErrorReserva::HabitacionOcupada { habitacion });
        }
        self.reservas.push(Reserva {
            habitacion,
            entrada,
            salida,
            personas,
            precio_noche,
            estado: Estado::Confirmada,
        });
        Ok(self.reservas.len() - 1)
    }

    /// ¿Hay alguna reserva confirmada de esa habitación que se solape con las noches pedidas?
    fn ocupada(&self, habitacion: u32, entrada: u32, salida: u32) -> bool {
        self.reservas.iter().any(|r| {
            r.habitacion == habitacion && r.entrada < salida && entrada < r.salida
        })
    }

    /// Número de noches de una reserva.
    pub fn noches(reserva: &Reserva) -> u32 {
        reserva.salida - reserva.entrada + 1
    }

    pub fn importe(reserva: &Reserva) -> u64 {
        u64::from(Self::noches(reserva)) * u64::from(reserva.precio_noche)
    }

    /// Cancela la reserva y devuelve lo que se reembolsa: todo si se cancela con la antelación de
    /// `DIAS_PARA_REEMBOLSO_COMPLETO` o más, y la mitad si no.
    pub fn cancelar(&mut self, indice: usize, hoy: u32) -> Option<u64> {
        let reserva = self.reservas.get_mut(indice)?;
        reserva.estado = Estado::Cancelada;
        let importe = Self::importe(reserva);
        if reserva.entrada - hoy > DIAS_PARA_REEMBOLSO_COMPLETO {
            Some(importe)
        } else {
            Some(importe / 2)
        }
    }
}

impl Default for Hotel {
    fn default() -> Self {
        Self::nuevo()
    }
}
