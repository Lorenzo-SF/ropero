//! Interactive administration of the invoking user's engine credentials.
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use ropero_core::EngineId;

/// Drop all privileges before opening user-controlled paths or accessing the keyring.
///
/// # Safety
/// Call from the CLI at startup, with no other threads. This changes the process
/// credentials and restores the environment used by discovery and keyring helpers.
#[cfg(unix)]
pub unsafe fn assume_invoking_user() -> Result<(), String> {
    use std::ffi::{CStr, OsString};
    use std::os::unix::ffi::OsStringExt;
    // SAFETY: geteuid reads the process credentials.
    if unsafe { libc::geteuid() } != 0 {
        return Err("Use sudo ropero config keys --model <MODEL> [--set]".into());
    }
    let uid: libc::uid_t = std::env::var("SUDO_UID")
        .ok()
        .and_then(|id| id.parse().ok())
        .filter(|id| *id != 0)
        .ok_or("Run sudo from the account that owns the models")?;
    // SAFETY: single-threaded, and all passwd fields are copied before any further NSS call.
    let (name, home, group) = unsafe {
        let pw = libc::getpwuid(uid)
            .as_ref()
            .ok_or("Unknown invoking user")?;
        if pw.pw_dir.is_null() || pw.pw_name.is_null() {
            return Err("Incomplete account information for invoking user".into());
        }
        (
            CStr::from_ptr(pw.pw_name).to_owned(),
            OsString::from_vec(CStr::from_ptr(pw.pw_dir).to_bytes().to_vec()),
            pw.pw_gid,
        )
    };
    if !Path::new(&home).is_absolute() {
        return Err("Missing home directory for invoking user".into());
    }
    // SAFETY: name is a live NUL-terminated string, gid/uid were resolved
    // from the system account database. Lose supplementary and primary root
    // groups before dropping uid. Caller exits on any error.
    unsafe {
        if libc::initgroups(name.as_ptr(), group as _) != 0
            || libc::setgid(group) != 0
            || libc::setuid(uid) != 0
            || libc::getuid() != uid
            || libc::geteuid() != uid
        {
            return Err(format!(
                "Cannot assume invoking user: {}",
                std::io::Error::last_os_error()
            ));
        }
        // Process environment is changed only in the single-threaded startup path.
        std::env::set_var("HOME", home);
        let login = OsString::from_vec(name.to_bytes().to_vec());
        std::env::set_var("USER", &login);
        std::env::set_var("LOGNAME", login);
        #[cfg(target_os = "linux")]
        {
            let runtime = format!("/run/user/{uid}");
            if Path::new(&runtime).is_dir() {
                std::env::set_var("XDG_RUNTIME_DIR", &runtime);
                if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
                    std::env::set_var(
                        "DBUS_SESSION_BUS_ADDRESS",
                        format!("unix:path={runtime}/bus"),
                    );
                }
            }
        }
    }
    Ok(())
}

/// # Safety
/// Same single-threaded startup contract as on Unix.
#[cfg(not(unix))]
pub unsafe fn assume_invoking_user() -> Result<(), String> {
    Err("Key administration requires a platform with sudo".into())
}

pub fn manage(state_dir: &Path, engine: EngineId, set: bool, lang: &str) -> Result<(), String> {
    if engine == EngineId::Basalt {
        return Err("Basalt runs without an API key / Basalt arranca sin clave".into());
    }
    let en = lang == "EN";
    let mut terminal = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|_| "Interactive terminal required / Se necesita una terminal interactiva")?;
    let engine_name = engine.as_str();
    let question = match (en, set) {
        (true, true) => format!("Replace the key for all models using {engine_name}? [y/N] "),
        (true, false) => format!("Show the key for {engine_name} on this terminal? [y/N] "),
        (false, true) => format!("¿Cambiar la clave de todos los modelos de {engine_name}? [s/N] "),
        (false, false) => format!("¿Mostrar la clave de {engine_name} en esta terminal? [s/N] "),
    };
    write!(terminal, "{question}")
        .and_then(|_| terminal.flush())
        .map_err(|e| e.to_string())?;
    let mut answer = String::new();
    BufReader::new(&terminal)
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    if !matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes" | "s" | "sí"
    ) {
        return Err(if en { "Cancelled" } else { "Cancelado" }.into());
    }
    if set {
        let key = crate::key_store::prompt_secret(if en {
            "New engine key: "
        } else {
            "Nueva clave del engine: "
        })?;
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("Invalid API key / Clave de API inválida".into());
        }
        let confirm = crate::key_store::prompt_secret(if en {
            "Repeat the new key: "
        } else {
            "Repite la nueva clave: "
        })?;
        if key != confirm {
            return Err("Keys differ / Las claves son distintas".into());
        }
        crate::api_key::admin_write(state_dir, engine, &key)?;
        writeln!(
            terminal,
            "{}",
            if en { "Key saved" } else { "Clave guardada" }
        )
        .map_err(|e| e.to_string())?;
    } else {
        let key = crate::api_key::admin_read(state_dir, engine)?
            .ok_or("No stored key / Sin clave guardada")?;
        writeln!(terminal, "{key}").map_err(|e| e.to_string())?;
    }
    Ok(())
}
