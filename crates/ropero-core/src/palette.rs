//! Los **colores reales** de la leyenda de motores (doc CLI v2 §4).
//!
//! La leyenda del doc dice "naranja", "verde pistacho", "verde pálido": son
//! nombres de color, no códigos ANSI. Este módulo los traduce a **ANSI
//! de 256 colores**, que es lo que un terminal moderno puede pintar de
//! verdad (y no un `\x1b[33m` genérico que no es ni naranja ni pistacho).
//!
//! ## Por qué 256 colores y no los 16 de siempre
//!
//! Los 16 básicos no tienen naranja ni verde pistacho: el "amarillo" de
//! ANSI es un `#ffff00` chillón y el "verde" es `#00aa00`. Con la paleta de
//! 256 colores se puede pedir un naranja de verdad (`208`), un pistacho
//! (`154`) o un verde pálido (`193`).
//!
//! ## El color se puede apagar
//!
//! Un listado con color dentro de un canal no es legible. Por eso hay un
//! interruptor ([`Palette::enabled`]): cuando está apagado, el texto sale
//! **sin** secuencias ANSI, y el listado sigue siendo el mismo.

use crate::EngineColor;

/// El prefijo de las secuencias ANSI de 256 colores.
const INICIO: &str = "\x1b[38;5;";
/// El sufijo que cierra una secuencia de color.
const FIN: &str = "m";
/// El cierre del color de primer plano: el 39 lo deja como estaba.
///
/// Antes se cerraba con el 49, que es un **fondo**, no un primer plano: el
/// texto que venía detrás de un motor salía en gris. Con el 39, lo que
/// sigue a un nombre de motor sale con el color de siempre.
const CIERRE: &str = "\x1b[39m";

/// La paleta: los colores del documento, o ninguno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Si se pinta con color.
    pub enabled: bool,
}

impl Palette {
    /// Una paleta con color.
    pub const fn color() -> Self {
        Palette { enabled: true }
    }

    /// Una paleta sin color (para canalizar, o para `--no-color`).
    pub const fn plain() -> Self {
        Palette { enabled: false }
    }

    /// Pinta un texto del color de un motor.
    ///
    /// Sin color, devuelve el texto tal cual: así el llamante no tiene que
    /// preguntar si hay que escapar.
    pub fn paint(&self, color: EngineColor, texto: &str) -> String {
        match self.ansi(color) {
            Some(codigo) => format!("{INICIO}{codigo}{FIN}{texto}{CIERRE}"),
            None => texto.to_string(),
        }
    }

    /// El código ANSI de 256 colores de un color de la leyenda.
    ///
    /// `None` para el color "sin-color" y para el que no tiene equivalente.
    pub fn ansi(&self, color: EngineColor) -> Option<u8> {
        if !self.enabled {
            return None;
        }
        engine_ansi(color)
    }
}

/// El código ANSI de 256 colores de cada color de la leyenda.
///
/// La paleta es **fija**: el doc CLI v2 §4 dice qué color es cada motor, y
/// el mismo motor es siempre el mismo color.
pub fn engine_ansi(color: EngineColor) -> Option<u8> {
    match color {
        // 208: naranja de verdad (#ff8700).
        EngineColor::Naranja => Some(208),
        // 220: amarillo cálido (#ffd700).
        EngineColor::Amarillo => Some(220),
        // 196: rojo intenso (#ff0000).
        EngineColor::Rojo => Some(196),
        // 245: gris medio (#8a8a8a).
        EngineColor::Gris => Some(245),
        // 255: blanco puro.
        EngineColor::Blanco => Some(255),
        // 154: verde pistacho (#00ff00 con un punto de amarillo).
        EngineColor::VerdePistacho => Some(154),
        // 99: morado (#875fff).
        EngineColor::Morado => Some(99),
        // 193: verde pálido (#d7ff00, el "verde suave" de la paleta).
        EngineColor::VerdePalido => Some(193),
        // Sin color: el motor no tiene color asignado.
        EngineColor::SinColor => None,
    }
}

/// El nombre del color de la leyenda, tal y como lo escribe el manifiesto.
pub fn color_name(color: EngineColor) -> &'static str {
    color.as_str()
}

/// Pinta un texto con el color de un motor, usando la paleta con color.
pub fn paint(color: EngineColor, texto: &str) -> String {
    Palette::color().paint(color, texto)
}
