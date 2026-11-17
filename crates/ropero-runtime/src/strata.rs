//! Resolución de perfiles y comandos de serving propios de Strata.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use ropero_core::{EngineManifest, Environment, ModelSpec};

/// Busca el run config instalado por Strata que corresponde al path del
/// modelo declarado en el catálogo de Ropero.
pub fn profile_for(
    engine: &EngineManifest,
    spec: &ModelSpec,
    env: &Environment,
) -> Result<PathBuf, String> {
    let expected = profile_id(&spec.model.path);
    let path = installed_profiles(engine, env)?
        .remove(&expected)
        .ok_or_else(|| {
            format!(
                "Strata no tiene un perfil instalado para '{}'; configura o instala ese modelo desde Strata primero",
                spec.model.path
            )
        })?;
    let path = validate_profile_path(&path)?;
    let Some(alias) = spec.model.alias.as_deref() else {
        return Ok(path);
    };
    profile_with_alias(&path, spec, env, alias)
}

fn profile_with_alias(
    source: &std::path::Path,
    spec: &ModelSpec,
    env: &Environment,
    alias: &str,
) -> Result<PathBuf, String> {
    let alias = alias.trim();
    if alias.is_empty() || alias.contains(',') || alias.chars().any(char::is_whitespace) {
        return Err("el alias de Strata debe ser un id no vacío, sin espacios ni comas".into());
    }
    let contents = std::fs::read(source).map_err(|error| {
        format!(
            "no se pudo leer el perfil Strata {}: {error}",
            source.display()
        )
    })?;
    let mut document: serde_json::Value = serde_json::from_slice(&contents)
        .map_err(|error| format!("perfil Strata inválido {}: {error}", source.display()))?;
    let object = document
        .as_object_mut()
        .ok_or_else(|| "el perfil Strata debe ser un objeto JSON".to_string())?;
    let aliases = match object.get("aliases") {
        None => String::new(),
        Some(serde_json::Value::String(aliases)) => aliases.clone(),
        Some(_) => return Err("`aliases` del perfil Strata no es texto".into()),
    };
    let mut names = aliases
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !names.iter().any(|name| name == alias) {
        names.push(alias.to_string());
    }
    object.insert("aliases".into(), serde_json::Value::String(names.join(",")));

    let state_dir = crate::state::state_dir(&env.home, env.get("ROPERO_STATE_DIR"));
    let directory = state_dir.join("strata-profiles");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("no se pudo crear el directorio de perfiles Ropero: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("no se pudieron proteger los perfiles Ropero: {error}"))?;
    }
    let path = directory.join(format!("{}.json", spec.model.name));
    let temporary = directory.join(format!(".{}.{}.tmp", spec.model.name, std::process::id()));
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("no se pudo serializar el perfil Strata: {error}"))?;
    write_private_profile(&temporary, &bytes)?;
    std::fs::rename(&temporary, &path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!("no se pudo guardar el perfil Ropero para Strata: {error}")
    })?;
    Ok(path)
}

fn write_private_profile(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("no se pudo crear el perfil temporal Strata: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("no se pudo proteger el perfil temporal Strata: {error}"))?;
    }
    file.write_all(bytes)
        .map_err(|error| format!("no se pudo escribir el perfil temporal Strata: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("no se pudo sincronizar el perfil temporal Strata: {error}"))
}

/// Comprueba si Strata tiene un perfil instalado con todos sus componentes.
/// La consulta del CLI se cachea por ruta de ejecutable durante el proceso:
/// `ropero models` evalúa muchos modelos en una sola invocación.
pub fn model_installed(spec: &ModelSpec, engine: &EngineManifest, env: &Environment) -> bool {
    installed_profiles(engine, env)
        .ok()
        .is_some_and(|mut profiles| profiles.remove(&profile_id(&spec.model.path)).is_some())
}

fn installed_profiles(
    engine: &EngineManifest,
    env: &Environment,
) -> Result<std::collections::BTreeMap<String, PathBuf>, String> {
    type Profiles = std::collections::BTreeMap<String, PathBuf>;
    static CACHE: OnceLock<Mutex<std::collections::HashMap<PathBuf, Result<Profiles, String>>>> =
        OnceLock::new();
    let binary = crate::sys_engine::binary_of(engine, env).ok_or_else(|| {
        "Strata no está instalado; instala el engine antes de probar el modelo".to_string()
    })?;
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Some(cached) = cache
        .lock()
        .map_err(|_| "falló la caché de perfiles de Strata".to_string())?
        .get(&binary)
        .cloned()
    {
        return cached;
    }
    let result = (|| {
        let output = Command::new(&binary)
            .args(["models", "list", "--installed", "--output", "json"])
            .output()
            .map_err(|e| {
                format!("no se pudieron consultar los modelos instalados de Strata: {e}")
            })?;
        if !output.status.success() {
            return Err(format!(
                "Strata no pudo enumerar sus perfiles instalados: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let document: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Strata devolvió JSON inválido al enumerar perfiles: {e}"))?;
        let profiles = document
            .get("models")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "Strata no devolvió la lista de perfiles instalados".to_string())?;
        Ok(profiles
            .iter()
            .filter(|entry| {
                entry
                    .get("component_status")
                    .and_then(serde_json::Value::as_str)
                    == Some("present")
            })
            .filter_map(|entry| {
                let id = entry.get("id")?.as_str()?;
                let path = entry.get("source_config")?.as_str()?;
                let path = PathBuf::from(path);
                (path.is_file()).then(|| (profile_id(id), path))
            })
            .collect())
    })();
    cache
        .lock()
        .map_err(|_| "falló la caché de perfiles de Strata".to_string())?
        .insert(binary, result.clone());
    result
}

fn validate_profile_path(path: &std::path::Path) -> Result<PathBuf, String> {
    let path = path.to_path_buf();
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
    use super::{profile_id, profile_with_alias};
    use ropero_core::{Environment, parse_model};
    use std::collections::BTreeMap;
    use std::path::Path;

    #[test]
    fn path_del_catalogo_corresponde_al_id_de_perfil_de_strata() {
        assert_eq!(profile_id("gyro/Gyro-M"), "gyro-gyro-m");
        assert_eq!(profile_id("gyro/Gyro-S"), "gyro-gyro-s");
    }

    #[test]
    fn el_alias_se_anade_a_una_copia_del_perfil_sin_alterar_el_original() {
        let root = std::env::temp_dir().join(format!("ropero-strata-alias-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.json");
        std::fs::write(
            &source,
            r#"{"model_name":"id-del-engine","aliases":"otro"}"#,
        )
        .unwrap();
        let spec = parse_model(
            include_str!("../../../models/strata/gyro-m.toml"),
            Path::new("models/strata/gyro-m.toml"),
        )
        .unwrap();
        let env = Environment::new(BTreeMap::new(), root.clone(), root.clone());

        let path = profile_with_alias(&source, &spec, &env, "qwen-ignis").unwrap();
        let original: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&source).unwrap()).unwrap();
        let copied: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(original["aliases"], "otro");
        assert_eq!(copied["model_name"], "id-del-engine");
        assert_eq!(copied["aliases"], "otro,qwen-ignis");
        let _ = std::fs::remove_dir_all(root);
    }
}
