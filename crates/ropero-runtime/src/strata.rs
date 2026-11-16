//! Resolución de perfiles y comandos de serving propios de Strata.

use std::path::PathBuf;
use std::process::Command;

use ropero_core::{EngineManifest, Environment, ModelSpec};

/// Busca el run config instalado por Strata que corresponde al path del
/// modelo declarado en el catálogo de Ropero.
pub fn profile_for(
    engine: &EngineManifest,
    spec: &ModelSpec,
    env: &Environment,
) -> Result<PathBuf, String> {
    let binary = crate::sys_engine::binary_of(engine, env).ok_or_else(|| {
        "Strata no está instalado; instala el engine antes de probar el modelo".to_string()
    })?;
    let output = Command::new(&binary)
        .args(["models", "list", "--installed", "--output", "json"])
        .output()
        .map_err(|e| format!("no se pudieron consultar los modelos instalados de Strata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Strata no pudo enumerar sus perfiles instalados: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Strata devolvió JSON inválido al enumerar perfiles: {e}"))?;
    let expected = profile_id(&spec.model.path);
    let profiles = document["models"]
        .as_array()
        .ok_or_else(|| "Strata no devolvió la lista de perfiles instalados".to_string())?;
    let profile = profiles.iter().find(|entry| {
        entry["id"]
            .as_str()
            .is_some_and(|id| profile_id(id) == expected)
    });
    let path = profile
        .and_then(|entry| entry["source_config"].as_str())
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            format!(
                "Strata no tiene un perfil instalado para '{}'; configura o instala ese modelo desde Strata primero",
                spec.model.path
            )
        })?;
    if !path.is_file() {
        return Err(format!(
            "el perfil que anuncia Strata no existe: {}",
            path.display()
        ));
    }
    Ok(path)
}

/// Convierte la pareja familia/modelo de `[model].path` en el id que Strata
/// asigna a sus run configs (p. ej. `gyro/Gyro-M` → `gyro-gyro-m`).
fn profile_id(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c == '/' || c == '_' {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

/// Argumentos de serving del CLI de Strata. El lanzador de Ropero añade
/// `--port` al final, como en los demás engines.
pub fn serve_argv(profile: &std::path::Path) -> Vec<String> {
    vec![
        "serve".to_string(),
        "--profile".to_string(),
        profile.to_string_lossy().into_owned(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::profile_id;

    #[test]
    fn path_del_catalogo_corresponde_al_id_de_perfil_de_strata() {
        assert_eq!(profile_id("gyro/Gyro-M"), "gyro-gyro-m");
        assert_eq!(profile_id("gyro/Gyro-S"), "gyro-gyro-s");
    }
}
