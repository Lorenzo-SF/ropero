//! Per-engine API key storage. Encryption keys live in the desktop Secret Service
//! when available, or are derived from a user supplied passphrase.
use argon2::Argon2;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};
use ropero_core::EngineId;
use std::{
    io::{self, Write},
    path::Path,
};
use zeroize::Zeroize;

const MAGIC: &[u8; 8] = b"ROPKEY01";

pub fn prompt_secret(prompt: &str) -> Result<String, String> {
    rpassword::prompt_password(prompt).map_err(|e| format!("no se pudo leer el secreto: {e}"))
}

fn path(dir: &Path, engine: EngineId) -> std::path::PathBuf {
    dir.join("api_keys")
        .join(format!("{}.key", engine.as_str()))
}

pub fn read(dir: &Path, engine: EngineId) -> Result<Option<String>, String> {
    let path = path(dir, engine);
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("no se pudo leer la clave del engine: {e}")),
    };
    if !bytes.starts_with(MAGIC) {
        return Ok(Some(
            String::from_utf8(bytes)
                .map_err(|_| "clave antigua no es UTF-8")?
                .trim()
                .to_owned(),
        ));
    }
    let key = match secret_tool("lookup", engine)? {
        Some(key) => key,
        None => {
            let pass = prompt_secret("Almacén cifrado. Frase de paso: ")?;
            pass
        }
    };
    decrypt(&bytes, &key).map(Some)
}

pub fn write(dir: &Path, engine: EngineId, key: &str) -> Result<(), String> {
    let directory = dir.join("api_keys");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    let existing = path(dir, engine);
    let encrypted = existing.exists()
        && std::fs::read(&existing)
            .map_err(|e| e.to_string())?
            .starts_with(MAGIC);
    let bytes = if encrypted {
        let master =
            secret_tool("lookup", engine)?.ok_or("No se encontró la clave maestra del llavero")?;
        encrypt(key, &master)?
    } else {
        key.as_bytes().to_vec()
    };
    let tmp = directory.join(format!(".{}.{}.tmp", engine.as_str(), std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(&bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    std::fs::rename(tmp, existing).map_err(|e| e.to_string())
}

fn secret_tool(op: &str, engine: EngineId) -> Result<Option<String>, String> {
    let output = std::process::Command::new("secret-tool")
        .args([op, "application", "ropero", "engine", engine.as_str()])
        .output();
    let Ok(output) = output else { return Ok(None) };
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    ))
}

fn encrypt(text: &str, password: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    getrandom::fill(&mut salt)
        .and_then(|_| getrandom::fill(&mut nonce))
        .map_err(|e| e.to_string())?;
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .map_err(|e| e.to_string())?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), text.as_bytes())
        .map_err(|_| "falló el cifrado")?;
    key.zeroize();
    let mut result = MAGIC.to_vec();
    result.extend(salt);
    result.extend(nonce);
    result.extend(ciphertext);
    Ok(result)
}

fn decrypt(bytes: &[u8], password: &str) -> Result<String, String> {
    if bytes.len() < 48 {
        return Err("fichero de clave cifrada truncado".into());
    }
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), &bytes[8..24], &mut key)
        .map_err(|e| e.to_string())?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let clear = cipher
        .decrypt(XNonce::from_slice(&bytes[24..48]), &bytes[48..])
        .map_err(|_| "frase de paso incorrecta o fichero dañado")?;
    key.zeroize();
    String::from_utf8(clear).map_err(|e| e.to_string())
}
