//! Rutas resueltas de un modelo: dónde vive cada artefacto.
//!
//! Regla R-MOD-6: el formato declara dónde vive el artefacto.
//! - `gguf`        → `$GGUF_DIR/<modelo>/<[files].gguf>`
//! - `safetensors` → `$SAFETENSORS_DIR/<[model].path>`
//! - `q4nx`        → `[model].path` es un TAG (almacén de FLM)
//! - `strata`      → `[model].path` es `<familia>/<paquete>`
//!
//! Aquí sólo se calculan las rutas: la presentación (los rótulos `GGUF=`,
//! `MTP=`, …) es de la CLI.

use std::path::PathBuf;

use crate::Environment;
use crate::ModelSpec;

/// Rutas resueltas de un modelo.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPaths {
    pub gguf: Option<String>,
    pub mtp: Option<String>,
    pub mmproj: Option<String>,
    pub chat_template: Option<String>,
    pub model_path: Option<String>,
    pub log: String,
}

/// Resuelve las rutas de un modelo a partir de su TOML y del entorno
/// capturado.
///
/// PURA: no lee disco, no consulta el entorno, no lanza procesos. El
/// entorno llega por parámetro (doc B §3.2.3).
pub fn resolve_paths(spec: &ModelSpec, env: &Environment) -> ResolvedPaths {
    let real = crate::catalog::real_name(spec);
    let gguf_dir = env.engine_artifact_path(spec.model.engine, "GGUF_DIR", "models/gguf");
    let artifacts_dir = env.engine_artifact_path(
        spec.model.engine,
        "ARTIFACTS_DIR",
        if spec.model.engine == crate::EngineId::Ignis {
            "models/ignis"
        } else if spec.model.engine == crate::EngineId::Basalt {
            "models/basalt"
        } else {
            "models"
        },
    );
    let safetensors_dir =
        env.engine_artifact_path(spec.model.engine, "SAFETENSORS_DIR", "models/safetensors");
    let log_dir = env.env_path("LOG_DIR", "logs");

    let in_gguf = |p: &PathBuf| gguf_dir.join(&real).join(p).to_string_lossy().to_string();

    let gguf = spec.files.gguf.as_ref().map(|g| {
        let name = env
            .get(&format!("ROPERO_{}_GGUF", crate::catalog::slug(&real)))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| g.to_string_lossy().to_string());
        gguf_dir
            .join(&real)
            .join(name)
            .to_string_lossy()
            .to_string()
    });
    let mtp = spec.files.mtp.as_ref().map(in_gguf);
    let mmproj = spec.files.mmproj.as_ref().map(in_gguf);
    let chat_template = spec.files.chat_template.as_ref().map(in_gguf).or_else(|| {
        let aux = spec.download.as_ref().and_then(|d| {
            d.aux.iter().find(|(_, a)| {
                a.file.contains("chat")
                    || a.name.contains("chat")
                    || a.file.ends_with("jinja")
                    || a.file.ends_with("jinja2")
            })
        })?;
        let name = if aux.1.name.is_empty() {
            &aux.1.file
        } else {
            &aux.1.name
        };
        if name.is_empty() {
            None
        } else {
            Some(
                gguf_dir
                    .join(&real)
                    .join(name)
                    .to_string_lossy()
                    .to_string(),
            )
        }
    });

    let model_path = if spec.model.path.is_empty() {
        None
    } else {
        Some(match (spec.model.engine, spec.model.format) {
            (crate::EngineId::FastFlowLm, _) | (crate::EngineId::Strata, _) => {
                spec.model.path.clone()
            }
            (crate::EngineId::Ignis, crate::ModelFormat::Ninfer) => artifacts_dir
                .join(&spec.model.path)
                .to_string_lossy()
                .to_string(),
            (crate::EngineId::Basalt, crate::ModelFormat::Basalt) => artifacts_dir
                .join(&spec.model.path)
                .to_string_lossy()
                .to_string(),
            (_, crate::ModelFormat::Safetensors) => {
                let p = PathBuf::from(&spec.model.path);
                if p.is_absolute() {
                    p.to_string_lossy().to_string()
                } else {
                    safetensors_dir
                        .join(&spec.model.path)
                        .to_string_lossy()
                        .to_string()
                }
            }
            _ => spec.model.path.clone(),
        })
    };

    ResolvedPaths {
        gguf,
        mtp,
        mmproj,
        chat_template,
        model_path,
        log: log_dir
            .join(format!("{}.log", real))
            .to_string_lossy()
            .to_string(),
    }
}
