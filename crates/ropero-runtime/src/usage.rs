//! Log de uso local, append-only.
//!
//! Registra el uso de arranques sin secretos: nunca guarda API keys ni flags
//! o argumentos sensibles. El destino es el directorio de logs resuelto por
//! configuración.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nombre del fichero de uso dentro del directorio de logs.
pub const USAGE_FILE: &str = "usage.log";

/// Un registro de uso de arranque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRecord {
    /// Instante del registro (epoch segundos).
    pub timestamp: u64,
    /// Modelo solicitado.
    pub model: String,
    /// Motor del modelo.
    pub engine: String,
    /// Puerto, si se llegó a asignar/conocer.
    pub port: Option<u16>,
    /// Resultado: `ok`, `error`, `interrupted`, `uso`.
    pub result: String,
    /// Modo: `foreground` o `background`.
    pub mode: String,
}

impl UsageRecord {
    /// Serializa el registro como una línea `clave=valor` separada por tabuladores.
    ///
    /// Los valores se sanean para no introducir saltos de línea ni tabuladores.
    pub fn to_line(&self) -> String {
        let port = self
            .port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        format!(
            "ts={}\tmodel={}\tengine={}\tport={}\tresult={}\tmode={}\n",
            self.timestamp,
            sanitize(&self.model),
            sanitize(&self.engine),
            port,
            sanitize(&self.result),
            sanitize(&self.mode),
        )
    }
}

/// Sanea un valor para el log: sin saltos de línea, tabuladores ni secretos
/// obvios (no se guardan aquí, pero se evita romper el formato).
fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            _ => c,
        })
        .collect()
}

/// Ruta del log de uso dentro de `logs_dir`.
pub fn usage_path(logs_dir: &Path) -> PathBuf {
    logs_dir.join(USAGE_FILE)
}

/// Añade un registro al log de uso.
///
/// No guarda secretos: el llamante no debe pasar API keys ni flags/args
/// sensibles. Los errores de escritura se devuelven para no tragarlos.
pub fn append_usage(logs_dir: &Path, record: &UsageRecord) -> std::io::Result<()> {
    fs::create_dir_all(logs_dir)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(usage_path(logs_dir))?;
    file.write_all(record.to_line().as_bytes())?;
    file.flush()
}

/// Instante actual en segundos desde epoch.
pub fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Resuelve el directorio de logs desde el entorno/configuración capturada.
///
/// Usa `LOG_DIR` resuelto por la configuración (vía `env_path`).
pub fn logs_dir(env: &ropero_core::Environment) -> PathBuf {
    env.env_path("LOG_DIR", "logs")
}
