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
    // Strata's installed run configs are top-level strata-<id>.json files.
    // Prefer the copy with present artifacts when several installs contain
    // the same profile id.
    if let Some(path) = direct_profile(engine, env, &expected) {
        let path = validate_profile_path(&path)?;
        return match spec.model.alias.as_deref() {
            Some(alias) => profile_with_alias(&path, spec, env, alias),
            None => Ok(path),
        };
    }
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

fn direct_profiles(engine: &EngineManifest, env: &Environment, expected: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(install) = env.get("STRATA_INSTALL_DIR") {
        dirs.push(PathBuf::from(install));
    }
    if let Some(data) = env.get("STRATA_DATA_DIR") {
        dirs.push(PathBuf::from(data).join("install"));
    }
    // Strata's installed CLI prefers profiles beside its current terminal
    // package and profiles remembered by its settings. Check the known local
    // locations without starting its comparatively slow inventory command.
    if let Some(binary) = crate::sys_engine::binary_of(engine, env)
        && let Some(parent) = binary.parent()
    {
        dirs.push(parent.to_path_buf());
    }
    dirs.push(env.home.join("strata"));
    let data = env
        .get("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| env.home.join(".local/share"));
    dirs.push(data.join("strata/install"));
    let mut profiles = Vec::new();
    for path in dirs
        .into_iter()
        .map(|dir| dir.join(format!("strata-{expected}.json")))
        .filter(|path| path.is_file())
    {
        if !profiles.contains(&path) {
            profiles.push(path);
        }
    }
    profiles
}

fn direct_profile(engine: &EngineManifest, env: &Environment, expected: &str) -> Option<PathBuf> {
    let profiles = direct_profiles(engine, env, expected);
    profiles
        .iter()
        .find(|path| profile_artifacts_present(path, env))
        .cloned()
        .or_else(|| profiles.into_iter().next())
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

/// Comprueba si Strata tiene un perfil instalado con sus artefactos requeridos.
pub fn model_installed(spec: &ModelSpec, engine: &EngineManifest, env: &Environment) -> bool {
    let expected = profile_id(&spec.model.path);
    let profiles = direct_profiles(engine, env, &expected);
    if !profiles.is_empty() {
        return profiles
            .iter()
            .any(|profile| profile_artifacts_present(profile, env));
    }
    installed_profiles(engine, env)
        .ok()
        .and_then(|mut profiles| profiles.remove(&expected))
        .is_some_and(|profile| profile_artifacts_present(&profile, env))
}

/// Comprueba los ficheros del perfil sin invocar `strata models inspect`.
/// El inventario de Strata además sondea hardware y valida kernels, lo que
/// puede tardar varios segundos; para saber si el modelo está descargado sólo
/// necesitamos comprobar las rutas requeridas que declara el run config.
fn profile_artifacts_present(path: &std::path::Path, env: &Environment) -> bool {
    const MAX_PROFILE_BYTES: u64 = 1024 * 1024;
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_PROFILE_BYTES
    {
        return false;
    }

    let Ok(raw) = std::fs::read(path) else {
        return false;
    };
    let Ok(config) = serde_json::from_slice::<serde_json::Value>(&raw) else {
        return false;
    };
    let Some(object) = config.as_object() else {
        return false;
    };
    let Some(exe) = object.get("exe").and_then(serde_json::Value::as_str) else {
        return false;
    };
    if !profile_engine_present(exe, path, env) {
        return false;
    }
    let Some(args) = object.get("args").and_then(serde_json::Value::as_array) else {
        return false;
    };
    let base = object
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            path.parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .into()
        });
    let resolve = |raw: &str| {
        let path = if let Some(rest) = raw.strip_prefix("~/") {
            env.home.join(rest)
        } else {
            std::path::PathBuf::from(raw)
        };
        if path.is_absolute() {
            path
        } else {
            base.join(path)
        }
    };
    let Some(tokenizer) = object.get("tokenizer").and_then(serde_json::Value::as_str) else {
        return false;
    };
    let tokenizer = resolve(tokenizer);
    for file in ["vocab.json", "merges.txt", "token_type.json"] {
        if !nonempty_file(&tokenizer.join(file)) {
            return false;
        }
    }

    let mut index = 0;
    while index < args.len() {
        let Some(flag) = args[index].as_str() else {
            return false;
        };
        let kind = match flag {
            "--pack" | "--mtp" => Some(false),
            "--native" | "--ple-gguf" | "--expert-profile" => Some(true),
            "--control-vector-scaled" => Some(true),
            _ => None,
        };
        if let Some(is_file) = kind {
            let Some(raw) = args.get(index + 1).and_then(serde_json::Value::as_str) else {
                return false;
            };
            // Numeric --mtp values select draft depth and name no file.
            if flag == "--mtp" && raw.parse::<u32>().is_ok() {
                index += 2;
                continue;
            }
            let raw = if flag == "--control-vector-scaled" {
                raw.rsplit_once(':')
                    .filter(|(_, scale)| scale.parse::<f32>().is_ok())
                    .map_or(raw, |(path, _)| path)
            } else {
                raw
            };
            let resolved = resolve(raw);
            let present = if is_file {
                nonempty_file(&resolved)
            } else {
                resolved.is_dir()
            };
            // MTP is optional when the profile explicitly disables it or
            // selects DFlash. In the other modes Strata requires the folder.
            let mtp_optional = object.get("mtp") == Some(&serde_json::Value::Bool(false))
                || object
                    .get("drafting")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|mode| {
                        matches!(mode.to_ascii_lowercase().as_str(), "off" | "dflash2")
                    });
            if !present && !(flag == "--mtp" && mtp_optional) {
                return false;
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    true
}

fn profile_engine_present(exe: &str, profile: &std::path::Path, env: &Environment) -> bool {
    let Some(component) = exe.strip_prefix("component:") else {
        let resolved = if let Some(rest) = exe.strip_prefix("~/") {
            env.home.join(rest)
        } else {
            std::path::PathBuf::from(exe)
        };
        let resolved = if resolved.is_absolute() {
            resolved
        } else {
            profile
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(resolved)
        };
        return executable_file(&resolved);
    };
    let component_id = component.trim();
    if component_id.is_empty() || component_id.contains(['/', '\\']) {
        return false;
    }
    let root = env
        .get("STRATA_COMPONENTS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            env.get("XDG_DATA_HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| env.home.join(".local/share"))
                .join("strata/components")
        });
    let package_root = if component_id == "strata-engine" {
        root.join("engine")
    } else if let Some(shape) = component_id.strip_prefix("strata-") {
        root.join("shapes")
            .join(shape.replace('-', "_"))
            .join("engine")
    } else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(package_root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            return false;
        }
        let package = entry.path();
        let manifest = package.join("engine-component.json");
        let Ok(raw) = std::fs::read(manifest) else {
            return false;
        };
        let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&raw) else {
            return false;
        };
        if manifest.get("id").and_then(serde_json::Value::as_str) != Some(component_id) {
            return false;
        }
        let Some(platform) = manifest.get("platform").and_then(serde_json::Value::as_str) else {
            return false;
        };
        let expected_platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        if !platform.starts_with(&expected_platform) {
            return false;
        }
        manifest
            .get("entrypoint")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|entrypoint| executable_file(&package.join(entrypoint)))
    })
}

fn executable_file(path: &std::path::Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() || metadata.len() == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn nonempty_file(path: &std::path::Path) -> bool {
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
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
    use super::{profile_artifacts_present, profile_id, profile_with_alias};
    use ropero_core::{Environment, parse_model};
    use std::collections::BTreeMap;
    use std::path::Path;

    #[test]
    fn path_del_catalogo_corresponde_al_id_de_perfil_de_strata() {
        assert_eq!(profile_id("gyro/Gyro-M"), "gyro-gyro-m");
        assert_eq!(profile_id("gyro/Gyro-S"), "gyro-gyro-s");
    }

    #[test]
    fn detecta_componentes_de_un_perfil_sin_invocar_strata() {
        let root = std::env::temp_dir().join(format!("ropero-strata-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let tokenizer = root.join("tokenizer");
        let pack = root.join("pack");
        let engine_dir = root.join("components/engine/test-engine");
        std::fs::create_dir_all(&tokenizer).unwrap();
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::create_dir_all(&engine_dir).unwrap();
        let engine = engine_dir.join("strata");
        std::fs::write(&engine, "engine").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(
            engine_dir.join("engine-component.json"),
            serde_json::json!({
                "id": "strata-engine",
                "platform": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
                "entrypoint": "strata"
            })
            .to_string(),
        )
        .unwrap();
        for file in ["vocab.json", "merges.txt", "token_type.json"] {
            std::fs::write(tokenizer.join(file), "x").unwrap();
        }
        let model = root.join("model.gguf");
        std::fs::write(&model, "x").unwrap();
        let profile = root.join("strata-test.json");
        std::fs::write(
            &profile,
            serde_json::json!({
                "exe": "component:strata-engine",
                "cwd": root,
                "tokenizer": "tokenizer",
                "args": ["--pack", "pack", "--native", "model.gguf"]
            })
            .to_string(),
        )
        .unwrap();
        let env = Environment::new(
            BTreeMap::from([(
                "STRATA_COMPONENTS_DIR".to_string(),
                root.join("components").to_string_lossy().into_owned(),
            )]),
            root.clone(),
            root.clone(),
        );

        assert!(profile_artifacts_present(&profile, &env));
        std::fs::remove_file(&model).unwrap();
        assert!(!profile_artifacts_present(&profile, &env));
        let _ = std::fs::remove_dir_all(root);
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
