//! Identificadores canónicos de motor de inferencia.
//!
//! `EngineId` es el tipo que viaja en `[model].engine` del TOML. Los alias
//! legacy (`llama`, `mlx_lm`, `flm`, `tensorrt`, `strata_engine`) se
//! canónizan en la fase de carga del catálogo mediante `from_str_legacy`,
//! emitiendo un aviso (`Some(&'static str)`) cuando se usó un alias.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEngine(pub String);

impl std::fmt::Display for UnknownEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "motor desconocido: {}", self.0)
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
    /// `Ok((engine, Some(aviso)))` para los alias legacy. `Err(UnknownEngine)`
    /// para cualquier otra cadena.
    pub fn from_str_legacy(s: &str) -> Result<(Self, Option<&'static str>), UnknownEngine> {
        match s {
            "llama-server" => Ok((Self::LlamaServer, None)),
            "llama" => Ok((
                Self::LlamaServer,
                Some("alias legacy 'llama': usar 'llama-server'"),
            )),
            "llama-server-official" => Ok((Self::LlamaServerOfficial, None)),
            "llama-official" => Ok((
                Self::LlamaServerOfficial,
                Some("carpeta legacy 'llama-official'"),
            )),
            "mlx-serve" => Ok((Self::MlxServe, None)),
            "mlx" => Ok((Self::MlxServe, Some("carpeta legacy 'mlx'"))),
            "mlx_lm" => Ok((
                Self::MlxServe,
                Some("alias legacy 'mlx_lm': usar 'mlx-serve'"),
            )),
            "fastflowlm" => Ok((Self::FastFlowLm, None)),
            "flm" => Ok((
                Self::FastFlowLm,
                Some("alias legacy 'flm': usar 'fastflowlm'"),
            )),
            "tensorrt-llm" => Ok((Self::TensorRtLlm, None)),
            "tensorrt" => Ok((
                Self::TensorRtLlm,
                Some("alias legacy 'tensorrt': usar 'tensorrt-llm'"),
            )),
            "vllm" => Ok((Self::Vllm, None)),
            "airllm" => Ok((Self::AirLlm, None)),
            "strata" => Ok((Self::Strata, None)),
            "strata_engine" => Ok((
                Self::Strata,
                Some("alias legacy 'strata_engine': usar 'strata'"),
            )),
            other => Err(UnknownEngine(other.to_string())),
        }
    }
}
