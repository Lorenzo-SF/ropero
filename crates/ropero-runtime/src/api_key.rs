//! Resolución y persistencia privada de la clave usada por los engines locales.

use std::path::Path;

use ropero_core::config::Config;
use ropero_core::{EngineId, Environment};

/// Devuelve la clave de un engine sin generar una nueva.
pub fn configured_key(engine: EngineId, config: &Config, env: &Environment) -> Option<String> {
    config
        .engines
        .get(&engine)
        .and_then(|entry| entry.api_key.clone())
        .or_else(|| engine_env_key(engine).and_then(|name| env.get(name).map(str::to_owned)))
        .or_else(|| env.get("ROPERO_API_KEY").map(str::to_owned))
}

/// Resuelve la clave del servidor. La clave se guarda en el directorio de
/// estado para que `connect` pueda configurar el mismo bearer token.
pub fn launch_key(
    engine: EngineId,
    config: &Config,
    env: &Environment,
    state_dir: &Path,
    explicit: Option<&str>,
    disabled: bool,
) -> Result<Option<String>, String> {
    if engine == EngineId::Basalt {
        if explicit.is_some_and(|value| !value.is_empty()) {
            return Err("Basalt sólo acepta --api-key por argv y Ropero no expone credenciales en la lista de procesos; el servidor queda ligado a 127.0.0.1".to_string());
        }
        clear_engine_key(state_dir, engine);
        mark_engine_key_disabled(state_dir, engine)?;
        return Ok(None);
    }
    if disabled {
        clear_engine_key(state_dir, engine);
        mark_engine_key_disabled(state_dir, engine)?;
        return Ok(None);
    }

    let key = explicit
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| configured_key(engine, config, env))
        .or_else(|| read_engine_key(state_dir, engine))
        .or_else(|| read_key(state_dir))
        .map_or_else(generate_key, Ok)?;
    save_engine_key(state_dir, engine, &key)?;
    Ok(Some(key))
}

/// Resuelve la clave que debe entregar `connect`, respetando la configuración
/// del engine y la clave persistida al arrancarlo.
pub fn connect_key(
    engine: EngineId,
    config: &Config,
    env: &Environment,
    state_dir: &Path,
) -> Option<Option<String>> {
    if engine == EngineId::Basalt {
        return Some(None);
    }
    if disabled_marker_path(state_dir, engine).exists() {
        return Some(None);
    }
    read_engine_key(state_dir, engine)
        .or_else(|| read_key(state_dir))
        .or_else(|| configured_key(engine, config, env))
        .map(Some)
}

fn engine_env_key(engine: EngineId) -> Option<&'static str> {
    match engine {
        EngineId::LlamaServer | EngineId::TurboLlama => Some("LLAMA_API_KEY"),
        EngineId::Ignis => Some("IGNIS_SERVER_API_KEY"),
        EngineId::MlxServe => Some("MLX_API_KEY"),
        _ => None,
    }
}

fn read_key(state_dir: &Path) -> Option<String> {
    let key = std::fs::read_to_string(state_dir.join("api_key")).ok()?;
    let key = key.lines().next()?.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

fn engine_key_path(state_dir: &Path, engine: EngineId) -> std::path::PathBuf {
    state_dir
        .join("api_keys")
        .join(format!("{}.key", engine.as_str()))
}

fn read_engine_key(state_dir: &Path, engine: EngineId) -> Option<String> {
    let key = std::fs::read_to_string(engine_key_path(state_dir, engine)).ok()?;
    let key = key.lines().next()?.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

fn save_engine_key(state_dir: &Path, engine: EngineId, key: &str) -> Result<(), String> {
    let path = engine_key_path(state_dir, engine);
    let directory = path
        .parent()
        .ok_or_else(|| "ruta de clave de engine inválida".to_string())?;
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("no se pudo preparar el directorio de estado: {error}"))?;
    let temporary = directory.join(format!(".{}.{}.tmp", engine.as_str(), std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options
        .open(&temporary)
        .map_err(|error| format!("no se pudo guardar la clave de API: {error}"))?;
    file.write_all(key.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("no se pudo guardar la clave de API: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600)).map_err(
            |error| format!("no se pudieron proteger los permisos de la clave: {error}"),
        )?;
    }
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("no se pudo activar la clave de API: {error}"))?;
    let _ = std::fs::remove_file(disabled_marker_path(state_dir, engine));
    Ok(())
}

fn clear_engine_key(state_dir: &Path, engine: EngineId) {
    let _ = std::fs::remove_file(engine_key_path(state_dir, engine));
}

fn disabled_marker_path(state_dir: &Path, engine: EngineId) -> std::path::PathBuf {
    state_dir
        .join("api_keys")
        .join(format!("{}.disabled", engine.as_str()))
}

fn mark_engine_key_disabled(state_dir: &Path, engine: EngineId) -> Result<(), String> {
    let path = disabled_marker_path(state_dir, engine);
    let directory = path
        .parent()
        .ok_or_else(|| "ruta de clave de engine inválida".to_string())?;
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("no se pudo preparar el directorio de estado: {error}"))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|error| format!("no se pudo registrar que el engine va sin clave: {error}"))?;
    Ok(())
}

fn generate_key() -> Result<String, String> {
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut bytes = [0_u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut bytes))
            .map_err(|error| format!("no se pudo generar una clave de API segura: {error}"))?;
        Ok(format!("sk-ropero-{}", hex(&bytes)))
    }
    #[cfg(not(unix))]
    {
        Err("la generación segura de claves no está disponible en esta plataforma".into())
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    result
}
