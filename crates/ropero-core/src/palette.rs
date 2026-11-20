//! Los **colores reales** de la leyenda de motores (doc CLI v2 §4).
//!
//! Este es el único archivo con la asignación de colores del producto:
//! cada tono está declarado como un literal hexadecimal y se convierte a
//! RGB truecolor sólo al escribir ANSI en el terminal.
//!
//! ## El color se puede apagar
//!
//! Un listado con color dentro de un canal no es legible. Por eso hay un
//! interruptor ([`Palette::enabled`]): cuando está apagado, el texto sale
//! **sin** secuencias ANSI, y el listado sigue siendo el mismo.

use crate::{EngineColor, EngineId};

/// Cierra los atributos de fondo y primer plano aplicados por la paleta.
pub const CIERRE: &str = "\x1b[0m";

/// Un color RGB declarado como un único literal hexadecimal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexColor(pub u32);

impl HexColor {
    pub const fn rgb(self) -> (u8, u8, u8) {
        (
            ((self.0 >> 16) & 0xff) as u8,
            ((self.0 >> 8) & 0xff) as u8,
            (self.0 & 0xff) as u8,
        )
    }

    pub fn foreground(self) -> String {
        let (r, g, b) = self.rgb();
        format!("\x1b[38;2;{r};{g};{b}m")
    }
}

pub const HELP_HEADING: HexColor = HexColor(0x7dd3fc);
pub const RAM_LOW: HexColor = HexColor(0x86efac);
pub const RAM_MEDIUM: HexColor = HexColor(0xfde68a);
pub const RAM_HIGH: HexColor = HexColor(0xfca5a5);
pub const INTEL: HexColor = HexColor(0x0071c5);
pub const AMD: HexColor = HexColor(0xed1c24);
pub const APPLE_SILICON: HexColor = HexColor(0x1d1d1f);
pub const SNAPDRAGON: HexColor = HexColor(0xd22630);
pub const NVIDIA: HexColor = HexColor(0x76b900);

// Semantic log colors also live here so CLI components share one palette.
pub const LOG_ERROR: HexColor = HexColor(0xef4444);
pub const LOG_WARNING: HexColor = HexColor(0xf59e0b);
pub const LOG_INFO: HexColor = HexColor(0x22d3ee);
pub const LOG_PERFORMANCE: HexColor = HexColor(0xd946ef);
pub const LOG_MUTED: HexColor = HexColor(0x9ca3af);
pub const LOG_LIGHT: HexColor = HexColor(0xd1d5db);
pub const LOG_DEFAULT: HexColor = HexColor(0x22c55e);
pub const LOG_LLAMA_WARN_OFFICIAL: HexColor = HexColor(0xfdba74);
pub const LOG_LLAMA_WARN_TURBO: HexColor = HexColor(0xfde68a);

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
        engine_hex(color).map_or_else(|| texto.to_string(), |hex| self.paint_hex(hex, texto))
    }

    /// Pinta texto negro sobre el fondo hexadecimal indicado.
    pub fn paint_hex(&self, color: HexColor, texto: &str) -> String {
        if self.enabled {
            let (r, g, b) = color.rgb();
            format!("\x1b[48;2;{r};{g};{b}m\x1b[38;2;0;0;0m{texto}{CIERRE}")
        } else {
            texto.to_string()
        }
    }

    /// Pinta texto en primer plano para conservar el fondo de la terminal.
    pub fn paint_fg(&self, color: HexColor, texto: &str) -> String {
        if self.enabled {
            format!("{}{}{}", color.foreground(), texto, CIERRE)
        } else {
            texto.to_string()
        }
    }
}

/// Hexadecimal de los colores de motores y leyendas.
pub const fn engine_hex(color: EngineColor) -> Option<HexColor> {
    match color {
        EngineColor::Naranja => Some(HexColor(0xfdba74)),
        EngineColor::Amarillo => Some(HexColor(0xfde68a)),
        EngineColor::Rojo => Some(HexColor(0xfca5a5)),
        EngineColor::Gris => Some(HexColor(0xd1d5db)),
        EngineColor::Blanco => Some(HexColor(0xf3f4f6)),
        EngineColor::VerdePistacho => Some(HexColor(0xbef264)),
        EngineColor::Morado => Some(HexColor(0xc4b5fd)),
        EngineColor::VerdePalido => Some(HexColor(0xd9f99d)),
        EngineColor::Cian => Some(HexColor(0xa5f3fc)),
        EngineColor::Azul => Some(HexColor(0x93c5fd)),
        EngineColor::SinColor => None,
    }
}

pub const fn engine_color_for_id(id: EngineId) -> EngineColor {
    match id {
        EngineId::LlamaServer => EngineColor::Naranja,
        EngineId::TurboLlama => EngineColor::Amarillo,
        EngineId::MlxServe => EngineColor::Gris,
        EngineId::FastFlowLm => EngineColor::Rojo,
        EngineId::TensorRtLlm => EngineColor::VerdePistacho,
        EngineId::Vllm => EngineColor::Morado,
        EngineId::AirLlm => EngineColor::Blanco,
        EngineId::Strata => EngineColor::VerdePalido,
        EngineId::Ignis => EngineColor::Cian,
        EngineId::Basalt => EngineColor::Azul,
        EngineId::Colibri => EngineColor::VerdePistacho,
    }
}

pub fn engine_rgb(color: EngineColor) -> Option<(u8, u8, u8)> {
    engine_hex(color).map(HexColor::rgb)
}

/// El nombre del color de la leyenda, tal y como lo escribe el manifiesto.
pub fn color_name(color: EngineColor) -> &'static str {
    color.as_str()
}

/// Pinta un texto con el color de un motor, usando la paleta con color.
pub fn paint(color: EngineColor, texto: &str) -> String {
    Palette::color().paint(color, texto)
}
