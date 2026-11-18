//! Identificadores canónicos de motor de inferencia.
//!
//! `EngineId` es el tipo que viaja en `[model].engine` del TOML. Los alias
//! legacy (`llama-server-official`, `llama-official`, `llama`, `turbo`,
//! `mlx_lm`, `flm`, `tensorrt`, `strata_engine`) se canónizan en la fase de
//! carga del catálogo mediante [`EngineId::from_str_legacy`], que devuelve el
//! aviso con el formato del doc B §4.2:
//!
//! ```text
//! AVISO: el motor 'mlx_lm' es un alias antiguo; se usa 'mlx-serve'
//! ```
//!
//! Un alias legacy se canonicaliza **con un aviso**, nunca en silencio.
//!
//! ## La nomenclatura de [ADR-004]
//!
//! `llama-server` es el repo **oficial** de llama.cpp y el fork del dueño se
//! llama `turbo-llama`. Antes `llama-server` era el fork y el oficial se
//! llamaba `llama-server-official`; ese id es ahora un alias.

use serde::de::{Error as DeError, Unexpected};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum EngineId {
    /// El repo **oficial** de llama.cpp (ADR-004).
    #[serde(rename = "llama-server")]
    LlamaServer,
    /// El fork público del dueño (turboquant), antes `llama-server`.
    #[serde(rename = "turbo-llama")]
    TurboLlama,
    #[serde(rename = "mlx-serve")]
    MlxServe,
    #[serde(rename = "fastflowllm")]
    FastFlowLm,
    #[serde(rename = "tensorrt-llm")]
    TensorRtLlm,
    #[serde(rename = "vllm")]
    Vllm,
    #[serde(rename = "airllm")]
    AirLlm,
    #[serde(rename = "strata")]
    Strata,
    #[serde(rename = "ignis")]
    Ignis,
    #[serde(rename = "basalt")]
    Basalt,
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
            "motor desconocido: '{}'; usa uno de: llama-server, turbo-llama, \
             mlx-serve, fastflowllm, tensorrt-llm, vllm, airllm, strata, ignis, basalt",
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

/// Los ids canónicos, en el orden de la leyenda de motores.
pub const CANONICAL_IDS: &[&str] = &[
    "llama-server",
    "turbo-llama",
    "mlx-serve",
    "fastflowllm",
    "tensorrt-llm",
    "vllm",
    "airllm",
    "strata",
    "ignis",
    "basalt",
];

impl EngineId {
    /// Identificador canónico, tal y como se escribe en el TOML.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LlamaServer => "llama-server",
            Self::TurboLlama => "turbo-llama",
            Self::MlxServe => "mlx-serve",
            Self::FastFlowLm => "fastflowllm",
            Self::TensorRtLlm => "tensorrt-llm",
            Self::Vllm => "vllm",
            Self::AirLlm => "airllm",
            Self::Strata => "strata",
            Self::Ignis => "ignis",
            Self::Basalt => "basalt",
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
            "turbo-llama" => Self::TurboLlama,
            "mlx-serve" => Self::MlxServe,
            "fastflowllm" => Self::FastFlowLm,
            "tensorrt-llm" => Self::TensorRtLlm,
            "vllm" => Self::Vllm,
            "airllm" => Self::AirLlm,
            "strata" => Self::Strata,
            "ignis" => Self::Ignis,
            "basalt" => Self::Basalt,
            // Alias legacy (ADR-004): se canonicalizan CON aviso.
            //
            // `llama-server-official` y `llama-official` apuntaban al repo
            // oficial y ahora apuntan a `llama-server`; `llama` también
            // (en la v1 `llama` era el fork, que ahora es `turbo-llama`).
            "llama-server-official" => Self::LlamaServer,
            "llama-official" => Self::LlamaServer,
            "llama" => Self::LlamaServer,
            "turbo" => Self::TurboLlama,
            "mlx_lm" => Self::MlxServe,
            "fastflowlm" => Self::FastFlowLm,
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
    /// (`turbo-llama/` → `turbo-llama`, `mlx/` → `mlx-serve`, `flm/` →
    /// `fastflowllm`). `None` si el directorio no corresponde a ningún motor.
    ///
    /// Los directorios **legacy** del catálogo de la v1 (`llama/`,
    /// `llama-official/`) también se mapean, para que un catálogo viejo se
    /// siga leyendo tras el renombrado de ADR-004.
    pub fn from_dir_name(dir: &str) -> Option<Self> {
        match dir {
            "turbo-llama" => Some(Self::TurboLlama),
            "llama-server" => Some(Self::LlamaServer),
            "mlx" => Some(Self::MlxServe),
            "mlx-serve" => Some(Self::MlxServe),
            "flm" => Some(Self::FastFlowLm),
            "fastflowlm" => Some(Self::FastFlowLm),
            "fastflowllm" => Some(Self::FastFlowLm),
            "tensorrt" => Some(Self::TensorRtLlm),
            "tensorrt-llm" => Some(Self::TensorRtLlm),
            "vllm" => Some(Self::Vllm),
            "airllm" => Some(Self::AirLlm),
            "strata" => Some(Self::Strata),
            "ignis" => Some(Self::Ignis),
            "basalt" => Some(Self::Basalt),
            // Directorios legacy (ADR-004): el fork era `llama/` y el
            // oficial `llama-official/`.
            "llama" => Some(Self::TurboLlama),
            "llama-official" => Some(Self::LlamaServer),
            _ => None,
        }
    }
}
