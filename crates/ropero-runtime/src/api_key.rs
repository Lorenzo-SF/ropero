//! Resolución y persistencia privada de la clave usada por los engines locales.

use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use ropero_core::EngineId;

use crate::key_store;

/// Serialize startup (until its port state is published), migration and rotation.
pub(crate) fn lock_engine(state_dir: &Path, engine: EngineId) -> Result<File, String> {
    std::fs::create_dir_all(state_dir).map_err(|e| e.to_string())?;
    let path = state_dir.join(format!(".{}.launch.lock", engine.as_str()));
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| format!("no se pudo abrir el bloqueo del engine: {e}"))?;
    file.lock()
        .map_err(|e| format!("no se pudo bloquear el engine: {e}"))?;
    Ok(file)
}

fn disabled_path(state_dir: &Path, engine: EngineId) -> PathBuf {
    state_dir
        .join("api_keys")
        .join(format!("{}.disabled", engine.as_str()))
}

fn active(state_dir: &Path, engine: EngineId) -> bool {
    crate::run::procesos_gestionados(state_dir)
        .iter()
        .any(|(_, s)| s.engine == engine.as_str())
}

fn clear_marker(state_dir: &Path, engine: EngineId) -> Result<(), String> {
    match std::fs::remove_file(disabled_path(state_dir, engine)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn admin_read(state_dir: &Path, engine: EngineId) -> Result<Option<String>, String> {
    key_store::read(state_dir, engine)
}

pub fn admin_write(state_dir: &Path, engine: EngineId, key: &str) -> Result<(), String> {
    let _lock = lock_engine(state_dir, engine)?;
    if engine == EngineId::Basalt {
        return Err("Basalt se sirve localmente sin clave".into());
    }
    if active(state_dir, engine) {
        return Err("detén los modelos activos de este engine antes de cambiar la clave".into());
    }
    validate_key(key)?;
    key_store::write(state_dir, engine, key)?;
    clear_marker(state_dir, engine)
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("la clave de API debe contener caracteres ASCII visibles sin espacios".into());
    }
    Ok(())
}

/// Called under the startup lock. A disabled launch retains the user's saved key.
pub(crate) fn launch_key(
    engine: EngineId,
    state_dir: &Path,
    explicit: Option<&str>,
    disabled: bool,
) -> Result<Option<String>, String> {
    if engine == EngineId::Basalt && explicit.is_some() {
        return Err("Basalt sólo admite un servidor local sin clave de API".into());
    }
    let disabled = disabled || engine == EngineId::Basalt;
    let marker = disabled_path(state_dir, engine);
    let running = active(state_dir, engine);
    if running && marker.try_exists().map_err(|e| e.to_string())? != disabled {
        return Err(
            "detén los modelos activos antes de cambiar la autenticación del engine".into(),
        );
    }
    if disabled {
        std::fs::create_dir_all(marker.parent().expect("marker parent"))
            .map_err(|e| e.to_string())?;
        std::fs::write(marker, []).map_err(|e| e.to_string())?;
        return Ok(None);
    }
    let saved = key_store::read(state_dir, engine)?;
    if let Some(key) = explicit {
        validate_key(key)?;
        if running && saved.as_deref() != Some(key) {
            return Err("detén los modelos activos antes de cambiar la clave del engine".into());
        }
    }
    let key = match (explicit, saved) {
        (Some(key), Some(saved)) if key == saved => saved,
        (None, Some(key)) => key,
        (explicit, _) => {
            let key = match explicit {
                Some(key) => key.to_owned(),
                None => match std::fs::read_to_string(state_dir.join("api_key")) {
                    Ok(legacy) => legacy.trim_end().to_owned(),
                    Err(e) if e.kind() == ErrorKind::NotFound => generate_key()?,
                    Err(e) => return Err(e.to_string()),
                },
            };
            validate_key(&key)?;
            key_store::write(state_dir, engine, &key)?;
            key
        }
    };
    validate_key(&key)?;
    clear_marker(state_dir, engine)?;
    Ok(Some(key))
}

/// Outer None means no stored credential. Some(None) means explicitly disabled.
pub fn connect_key(engine: EngineId, state_dir: &Path) -> Result<Option<Option<String>>, String> {
    if engine == EngineId::Basalt
        || disabled_path(state_dir, engine)
            .try_exists()
            .map_err(|e| e.to_string())?
    {
        return Ok(Some(None));
    }
    let key = key_store::read(state_dir, engine)?;
    if let Some(value) = &key {
        validate_key(value)?;
    }
    Ok(key.map(Some))
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
