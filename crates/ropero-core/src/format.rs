//! Formatos de artefacto de un modelo y la regla de dónde vive cada uno.
//!
//! `ModelFormat` es el tipo que viaja en `[model].format` (default `gguf`).
//! La regla R-MOD-6: el formato declara dónde vive el artefacto:
//! - `gguf`         → `$GGUF_DIR/<modelo>/<[files].gguf>`
//! - `safetensors`  → `$SAFETENSORS_DIR/<[model].path>`
//! - `q4nx`         → `[model].path` es un TAG (almacén de FLM)
//! - `strata`       → `[model].path` es `<familia>/<paquete>`
//!
//! Aquí vive **una** forma de preguntar dónde está el artefacto
//! ([`ModelFormat::artifact_path`]) y **una** de preguntar en qué clase de
//! ubicación vive ([`ModelFormat::artifact_location`]). Los alias de una
//! misma función son ruido: inflan la API sin añadir nada.

use serde::{Deserialize, Serialize};

/// La clase de ubicación donde vive el artefacto de un formato.
///
/// Es la forma tipada de la regla R-MOD-6: no una cadena suelta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactLocation {
    GgufDir,
    SafetensorsDir,
    FlmTag,
    StrataCatalog,
    NinferDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
pub enum ModelFormat {
    #[serde(rename = "gguf")]
    #[default]
    Gguf,
    #[serde(rename = "safetensors")]
    Safetensors,
    #[serde(rename = "q4nx")]
    Q4nx,
    #[serde(rename = "strata")]
    Strata,
    #[serde(rename = "ninfer")]
    Ninfer,
}

impl ModelFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gguf => "gguf",
            Self::Safetensors => "safetensors",
            Self::Q4nx => "q4nx",
            Self::Strata => "strata",
            Self::Ninfer => "ninfer",
        }
    }

    /// Regla R-MOD-6: en qué clase de ubicación vive el artefacto principal.
    pub fn artifact_location(&self) -> ArtifactLocation {
        match self {
            Self::Gguf => ArtifactLocation::GgufDir,
            Self::Safetensors => ArtifactLocation::SafetensorsDir,
            Self::Q4nx => ArtifactLocation::FlmTag,
            Self::Strata => ArtifactLocation::StrataCatalog,
            Self::Ninfer => ArtifactLocation::NinferDir,
        }
    }

    /// Ubicación esperada del artefacto principal como cadena.
    ///
    /// - `gguf`: `$GGUF_DIR/<modelo>/<fichero>` (si `file` es `None`, sólo
    ///   el directorio del modelo).
    /// - `safetensors`: `$SAFETENSORS_DIR/<path>`.
    /// - `q4nx`: el TAG (`path`) tal cual.
    /// - `strata`: `<familia>/<paquete>` (`path`) tal cual.
    pub fn artifact_path(&self, model: &str, file: Option<&str>, path: &str) -> String {
        match self {
            Self::Gguf => match file {
                Some(f) => format!("$GGUF_DIR/{model}/{f}"),
                None => format!("$GGUF_DIR/{model}"),
            },
            Self::Safetensors => format!("$SAFETENSORS_DIR/{path}"),
            Self::Q4nx => path.to_string(),
            Self::Strata => path.to_string(),
            Self::Ninfer => {
                let artifact = if path.is_empty() {
                    file.unwrap_or("")
                } else {
                    path
                };
                if artifact.is_empty() {
                    "$ARTIFACTS_DIR".to_string()
                } else {
                    format!("$ARTIFACTS_DIR/{artifact}")
                }
            }
        }
    }
}

impl std::fmt::Display for ModelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
