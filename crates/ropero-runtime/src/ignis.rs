//! Operaciones de modelo delegadas al catálogo integrado de Ignis.

use std::process::{Command, Stdio};

use ropero_core::{EngineManifest, Environment, ModelFormat, ModelSpec};

/// Directorio de descarga configurado para Ignis (por defecto ~/models/ignis).
pub fn models_dir(env: &Environment) -> std::path::PathBuf {
    env.engine_artifact_path(
        ropero_core::EngineId::Ignis,
        "ARTIFACTS_DIR",
        "models/ignis",
    )
}

/// Comprueba los artefactos oficiales conocidos sin asumir que el id 27B
/// siempre se descarga con el mismo archivo: Ignis elige la variante por GPU.
pub fn model_installed(spec: &ModelSpec, env: &Environment) -> bool {
    let Some(id) = spec
        .download
        .as_ref()
        .map(|download| download.repo.as_str())
    else {
        return false;
    };
    let names: &[&str] = match id {
        "qwen3.8-27b" => &[
            "qwen3_8_27b_nvfp4full-v2.ninfer",
            "qwen3_8_27b_trellis_sc30-v2.ninfer",
        ],
        "qwen3.8-flash-next" => &["qwen3_8_flash_next_trellis_a25-v2.ninfer"],
        _ => return false,
    };
    names
        .iter()
        .any(|name| models_dir(env).join(name).is_file())
}

/// Descarga un modelo .ninfer y los ficheros companion con el descargador
/// oficial de Ignis. `download.repo` contiene el id de su catálogo.
pub fn pull_model(
    manifest: &EngineManifest,
    spec: &ModelSpec,
    env: &Environment,
) -> Result<i32, String> {
    if manifest.id != ropero_core::EngineId::Ignis || spec.model.format != ModelFormat::Ninfer {
        return Err("la descarga delegada de Ignis requiere un modelo ninfer".to_string());
    }
    let catalog_id = spec
        .download
        .as_ref()
        .map(|download| download.repo.trim())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            "el modelo Ignis debe declarar [download].repo como id del catálogo Ignis".to_string()
        })?;
    let catalog_id = id_de_descarga(catalog_id, env);
    let out_dir = models_dir(env);
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("no se pudo crear {}: {e}", out_dir.display()))?;
    let binary = crate::sys_engine::binary_of(manifest, env).ok_or_else(|| {
        "Ignis no está instalado; ejecuta 'ropero install engines ignis'".to_string()
    })?;
    let status = Command::new(&binary)
        .args(["model", "download", catalog_id, "--out"])
        .arg(out_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| {
            format!(
                "no se pudo ejecutar {} model download: {e}",
                binary.display()
            )
        })?;
    Ok(status.code().unwrap_or(1))
}

fn id_de_descarga<'a>(catalog_id: &'a str, env: &Environment) -> &'a str {
    if catalog_id != "qwen3.8-27b" {
        return catalog_id;
    }
    let Some(nvidia_smi) = crate::install::which_en(env, "nvidia-smi") else {
        return catalog_id;
    };
    let output = std::process::Command::new(nvidia_smi)
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output();
    let Ok(output) = output else {
        return catalog_id;
    };
    let mib = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<u64>().ok());
    if mib.is_some_and(|memory| memory <= 20_000) {
        "qwen3.8-27b-16g"
    } else {
        catalog_id
    }
}
