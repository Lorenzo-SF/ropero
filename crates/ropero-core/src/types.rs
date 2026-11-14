//! Newtypes del dominio.

use serde::{Deserialize, Serialize};

/// El puerto en el que se sirve un modelo.
///
/// Vive en `[model].port` (opcional): si no se declara, el runtime usa el
/// slot que toque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Port(pub u16);

impl Port {
    pub fn value(self) -> u16 {
        self.0
    }
    pub fn into_inner(self) -> u16 {
        self.0
    }
}

impl From<u16> for Port {
    fn from(v: u16) -> Self {
        Self(v)
    }
}

impl std::fmt::Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
