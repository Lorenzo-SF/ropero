//! Formatos de artefacto de un modelo y la regla de dónde vive cada uno.
//!
//! `ModelFormat` es el tipo que viaja en `[model].format` (default `gguf`).
//! La regla R-MOD-6: el formato declara dónde vive el artefacto:
//! - `gguf`         → `$GGUF_DIR/<modelo>/<[files].gguf>`
//! - `safetensors`  → `$SAFETENSORS_DIR/<[model].path>`
//! - `q4nx`         → `[model].path` es un TAG (almacén de FLM)
//! - `strata`       → `[model].path` es `<familia>/<paquete>`

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactLocation {
    GgufDir,
    SafetensorsDir,
    FlmTag,
    StrataCatalog,
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
}

impl ModelFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gguf => "gguf",
            Self::Safetensors => "safetensors",
            Self::Q4nx => "q4nx",
            Self::Strata => "strata",
        }
    }

    /// Regla R-MOD-6: prefijo/categoría de ubicación esperada.
    pub fn artifact_location_enum(&self) -> ArtifactLocation {
        match self {
            Self::Gguf => ArtifactLocation::GgufDir,
            Self::Safetensors => ArtifactLocation::SafetensorsDir,
            Self::Q4nx => ArtifactLocation::FlmTag,
            Self::Strata => ArtifactLocation::StrataCatalog,
        }
    }

    pub fn location_kind(&self) -> &'static str {
        match self {
            Self::Gguf => "gguf_dir",
            Self::Safetensors => "safetensors_dir",
            Self::Q4nx => "flm_tag",
            Self::Strata => "strata_catalog",
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
        }
    }

    pub fn artifact_location(&self, model: &str, file: Option<&str>, path: &str) -> String {
        self.artifact_path(model, file, path)
    }
    pub fn expected_location(&self, model: &str, file: Option<&str>, path: &str) -> String {
        self.artifact_path(model, file, path)
    }
    pub fn location(&self, model: &str, file: Option<&str>, path: &str) -> String {
        self.artifact_path(model, file, path)
    }
    pub fn where_lives(&self, model: &str, file: Option<&str>, path: &str) -> String {
        self.artifact_path(model, file, path)
    }
    pub fn path_for(&self, model: &str, file: Option<&str>, path: &str) -> String {
        self.artifact_path(model, file, path)
    }
}

impl std::fmt::Display for ModelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// Free functions (alias) para mayor compatibilidad con tests que usen estilo
// funcional.
pub fn artifact_location(
    format: ModelFormat,
    model: &str,
    file: Option<&str>,
    path: &str,
) -> String {
    format.artifact_path(model, file, path)
}
pub fn expected_location(
    format: ModelFormat,
    model: &str,
    file: Option<&str>,
    path: &str,
) -> String {
    format.artifact_path(model, file, path)
}
pub fn location(format: ModelFormat, model: &str, file: Option<&str>, path: &str) -> String {
    format.artifact_path(model, file, path)
}
pub fn where_lives(format: ModelFormat, model: &str, file: Option<&str>, path: &str) -> String {
    format.artifact_path(model, file, path)
}
pub fn artifact_path(format: ModelFormat, model: &str, file: Option<&str>, path: &str) -> String {
    format.artifact_path(model, file, path)
}
