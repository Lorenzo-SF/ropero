//! Newtypes del dominio.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Port(pub u16);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub struct ModelName(pub String);

impl Port {
    pub fn value(self) -> u16 {
        self.0
    }
    pub fn into_inner(self) -> u16 {
        self.0
    }
}

impl ModelName {
    pub fn value(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<u16> for Port {
    fn from(v: u16) -> Self {
        Self(v)
    }
}

impl From<String> for ModelName {
    fn from(v: String) -> Self {
        Self(v)
    }
}

impl From<&str> for ModelName {
    fn from(v: &str) -> Self {
        Self(v.to_string())
    }
}

impl std::fmt::Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for ModelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
