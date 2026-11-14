//! Identificadores canónicos de motor de inferencia.
//!
//! `EngineId` es el tipo que viaja en `[model].engine` del TOML. Los alias
//! legacy (`llama`, `mlx_lm`, `flm`, `tensorrt`, `strata_engine`) se
//! canónizan en la fase de carga del catálogo mediante [`EngineId::from_str_legacy`],
//! que devuelve el aviso con el formato del doc B §4.2:
//!
//! ```text
//! AVISO: el motor 'mlx_lm' es un alias antiguo; se usa 'mlx-serve'
//! ```
//!
//! Un alias legacy se canonicaliza **con un aviso**, nunca en silencio.

use serde::de::{Error as DeError, Unexpected};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum EngineId {
    #[serde(rename = "llama-server")]
    LlamaServer,
    #[serde(rename = "llama-server-official")]
    LlamaServerOfficial,
    #[serde(rename = "mlx-serve")]
    MlxServe,
    #[serde(rename = "fastflowlm")]
    FastFlowLm,
    #[serde(rename = "tensorrt-llm")]
    TensorRtLlm,
    #[serde(rename = "vllm")]
    Vllm,
    #[serde(rename = "airllm")]
    AirLlm,
    #[serde(rename = "strata")]
    Strata,
}

/// Deserializa el motor acepta **los ids canónicos y los alias legacy**.
///
/// El alias se canonicaliza aquí, en la deserialización, y no antes: así el
/// TOML se deserializa **desde su texto original**, y el error que salga
/// lleva el `span` sobre ese texto (la línea real del campo culpable).
///
/// El aviso de canonicalización (doc B §4.2) no sale de aquí —el
/// deserializador no puede avisar—: lo compone `parse_model`, que ya leyó
/// el valor crudo del motor.
impl<'de> Deserialize<'de> for EngineId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let crudo = String::deserialize(deserializer)?;
        EngineId::from_str_legacy(&crudo)
            .map(|(motor, _)| motor)
            .map_err(|_| D::Error::invalid_value(Unexpected::Str(&crudo), &"un motor conocido"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEngine(pub String);

impl std::fmt::Display for UnknownEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "motor desconocido: '{}'; usa uno de: llama-server, llama-server-official, \
             mlx-serve, fastflowlm, tensorrt-llm, vllm, airllm, strata",
            self.0
        )
    }
}

impl std::error::Error for UnknownEngine {}

impl UnknownEngine {
    pub fn name(&self) -> &str {
        &self.0
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Los ocho ids canónicos, en el orden de la tabla de §4.2.
pub const CANONICAL_IDS: &[&str] = &[
    "llama-server",
    "llama-server-official",
    "mlx-serve",
    "fastflowlm",
    "tensorrt-llm",
    "vllm",
    "airllm",
    "strata",
];

impl EngineId {
    /// Identificador canónico, tal y como se escribe en el TOML.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LlamaServer => "llama-server",
            Self::LlamaServerOfficial => "llama-server-official",
            Self::MlxServe => "mlx-serve",
            Self::FastFlowLm => "fastflowlm",
            Self::TensorRtLlm => "tensorrt-llm",
            Self::Vllm => "vllm",
            Self::AirLlm => "airllm",
            Self::Strata => "strata",
        }
    }

    /// Canóniza un identificador de motor, aceptando alias legacy con aviso.
    ///
    /// Devuelve `Ok((engine, None))` para los identificadores canónicos y
    /// `Ok((engine, Some(aviso)))` para los alias legacy. El aviso lleva el
    /// formato literal del doc B §4.2. `Err(UnknownEngine)` para cualquier
    /// otra cadena.
    pub fn from_str_legacy(s: &str) -> Result<(Self, Option<String>), UnknownEngine> {
        let canonical = match s {
            "llama-server" => Self::LlamaServer,
            "llama-server-official" => Self::LlamaServerOfficial,
            "mlx-serve" => Self::MlxServe,
            "fastflowlm" => Self::FastFlowLm,
            "tensorrt-llm" => Self::TensorRtLlm,
            "vllm" => Self::Vllm,
            "airllm" => Self::AirLlm,
            "strata" => Self::Strata,
            // Alias legacy (tabla de §4.2): se canonicalizan CON aviso.
            "llama" => Self::LlamaServer,
            "mlx_lm" => Self::MlxServe,
            "flm" => Self::FastFlowLm,
            "tensorrt" => Self::TensorRtLlm,
            "strata_engine" => Self::Strata,
            other => return Err(UnknownEngine(other.to_string())),
        };
        let aviso = if canonical.as_str() == s {
            None
        } else {
            Some(format!(
                "AVISO: el motor '{s}' es un alias antiguo; se usa '{}'",
                canonical.as_str()
            ))
        };
        Ok((canonical, aviso))
    }

    /// El motor de una **carpeta** de `models/`.
    ///
    /// Los directorios no son ids de motor: son las carpetas donde viven los
    /// modelos de cada motor, y su nombre no siempre coincide con él
    /// (`llama/` → `llama-server`, `mlx/` → `mlx-serve`, `flm/` →
    /// `fastflowlm`). `None` si el directorio no corresponde a ningún motor.
    pub fn from_dir_name(dir: &str) -> Option<Self> {
        match dir {
            "llama" => Some(Self::LlamaServer),
            "llama-official" => Some(Self::LlamaServerOfficial),
            "mlx" => Some(Self::MlxServe),
            "flm" => Some(Self::FastFlowLm),
            "tensorrt" => Some(Self::TensorRtLlm),
            "vllm" => Some(Self::Vllm),
            "airllm" => Some(Self::AirLlm),
            "strata" => Some(Self::Strata),
            _ => None,
        }
    }
}
