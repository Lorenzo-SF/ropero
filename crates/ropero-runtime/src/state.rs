//! El estado en disco: la fuente de verdad de lo que ropero arrancó.
//!
//! ## El formato
//!
//! Líneas `CLAVE=valor`, tal y como fija R-ARQ-9 del doc A. El doc A es
//! normativo sobre el comportamiento observable, y el formato del fichero
//! de estado es observable: está en disco, lo puede leer un usuario, y hay
//! herramientas que lo leen. **No se cambia.**
//!
//! Las ocho claves de la v1 siguen ahí, con el mismo nombre y el mismo
//! significado, y un lector de v1 sigue entendiendo el fichero:
//!
//! ```ini
//! MODEL=groot
//! ENGINE=llama-server
//! PID=42137
//! PORT=9999
//! LEVEL=high
//! TS=1762362312
//! LOG=/home/usuario/.local/state/ropero/logs/9999.log
//! STAMP=a1b2c3d4
//! ```
//!
//! Las **tres claves nuevas** (`STARTTIME`, `EXEDIGEST`, `VERSION`) son
//! adiciones compatibles: sin ellas, un lector de v1 sigue entendiendo el
//! fichero; con ellas, un lector de v2 puede **probar** que el PID es
//! nuestro y no un PID reciclado (doc B §3.3.3).
//!
//! ## La escritura es atómica
//!
//! Se escribe en un fichero temporal y se renombra (`tmp` + `mv`): un
//! `status` concurrente no lee un fichero a medias.
//!
//! ## La escritura conserva las claves que no vienen
//!
//! Es lo que fija R-ARQ-9: escribir `PID` no borra `MODEL`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// La versión del formato del estado.
///
/// `2` es la de la v2: las ocho claves de la v1 más `STARTTIME`,
/// `EXEDIGEST` y `VERSION`.
pub const STATE_VERSION: &str = "2";

/// El estado de un puerto gestionado por ropero.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortState {
    /// `MODEL`: el modelo que se sirve.
    pub model: String,
    /// `ENGINE`: el motor que lo sirve.
    pub engine: String,
    /// `PID`: el proceso que lo sirve.
    pub pid: Option<u32>,
    /// `PORT`: el puerto en el que sirve.
    pub port: Option<u16>,
    /// `LEVEL`: el nivel de razonamiento con el que arrancó.
    pub level: String,
    /// `TS`: el instante del arranque (epoch, en segundos).
    pub ts: Option<u64>,
    /// `LOG`: el fichero de log del arranque.
    pub log: String,
    /// `STAMP`: el sello de la configuración de arranque.
    ///
    /// Es un hash del modelo, el nivel, el contexto y los ficheros: si el
    /// sello del estado **no** coincide con el que la configuración actual
    /// produce, el modelo se considera **obsoleto** y hay que reiniciarlo
    /// (doc B §3.3.4).
    pub stamp: String,
    /// `STARTTIME`: los segundos desde epoch del inicio del proceso.
    ///
    /// Es lo que distingue nuestro proceso de un PID reciclado.
    pub start_time: Option<u64>,
    /// `EXEDIGEST`: el SHA-256 del binario del motor.
    pub exe_digest: Option<String>,
}

impl PortState {
    /// El estado como las líneas `CLAVE=valor` del fichero.
    ///
    /// El orden es fijo y es el de la v1: las ocho claves de siempre
    /// primero, las nuevas después. Un fichero que se lee a ojo tiene que
    /// Parecerse al de la v1.
    pub fn to_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = Vec::with_capacity(11);
        lines.push(format!("MODEL={}", self.model));
        lines.push(format!("ENGINE={}", self.engine));
        lines.push(format!(
            "PID={}",
            self.pid.map(|p| p.to_string()).unwrap_or_default()
        ));
        lines.push(format!(
            "PORT={}",
            self.port.map(|p| p.to_string()).unwrap_or_default()
        ));
        lines.push(format!("LEVEL={}", self.level));
        lines.push(format!(
            "TS={}",
            self.ts.map(|t| t.to_string()).unwrap_or_default()
        ));
        lines.push(format!("LOG={}", self.log));
        lines.push(format!("STAMP={}", self.stamp));
        lines.push(format!(
            "STARTTIME={}",
            self.start_time.map(|t| t.to_string()).unwrap_or_default()
        ));
        lines.push(format!(
            "EXEDIGEST={}",
            self.exe_digest.clone().unwrap_or_default()
        ));
        lines.push(format!("VERSION={STATE_VERSION}"));
        lines
    }

    /// El estado leído de sus líneas `CLAVE=valor`.
    ///
    /// Las claves que no están se quedan vacías: un estado de la v1 (sin
    /// `STARTTIME` ni `EXEDIGEST`) se lee sin problema, sólo que sin las
    /// pruebas extra de identidad.
    pub fn from_lines(lines: &[String]) -> Self {
        let mut campos: BTreeMap<&str, &str> = BTreeMap::new();
        for linea in lines {
            let Some((clave, valor)) = linea.split_once('=') else {
                continue;
            };
            campos.insert(clave.trim(), valor.trim());
        }
        let numero = |clave: &str| campos.get(clave).and_then(|v| v.parse::<u64>().ok());
        Self {
            model: campos.get("MODEL").unwrap_or(&"").to_string(),
            engine: campos.get("ENGINE").unwrap_or(&"").to_string(),
            pid: numero("PID").and_then(|v| u32::try_from(v).ok()),
            port: numero("PORT").and_then(|v| u16::try_from(v).ok()),
            level: campos.get("LEVEL").unwrap_or(&"").to_string(),
            ts: numero("TS"),
            log: campos.get("LOG").unwrap_or(&"").to_string(),
            stamp: campos.get("STAMP").unwrap_or(&"").to_string(),
            start_time: numero("STARTTIME"),
            exe_digest: campos
                .get("EXEDIGEST")
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string()),
        }
    }
}

/// El directorio de estado: `$STATE_DIR`, por defecto `~/.local/state/ropero`.
///
/// El valor sale del entorno ya capturado (`ROPERO_STATE_DIR` si está, si
/// no `$HOME/.local/state/ropero`). El runtime no consulta el entorno del
/// proceso por su cuenta: quien lo llama se lo pasa.
pub fn state_dir(home: &Path, env_var: Option<&str>) -> PathBuf {
    match env_var {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home.join(".local").join("state").join("ropero"),
    }
}

/// El fichero de estado de un puerto: `$STATE_DIR/ports/<puerto>`.
///
/// El nombre del fichero es **el número del puerto**, sin extensión: es
/// el nombre que la v1 usa, y quien lea el directorio lo reconoce.
pub fn state_path(state_dir: &Path, port: u16) -> PathBuf {
    state_dir.join("ports").join(port.to_string())
}

/// Los puertos que tienen fichero de estado, ordenados.
pub fn managed_ports(state_dir: &Path) -> Vec<u16> {
    let dir = state_dir.join("ports");
    let Ok(entradas) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut puertos: Vec<u16> = entradas
        .flatten()
        .filter_map(|e| {
            let nombre = e.file_name().to_string_lossy().to_string();
            nombre.parse::<u16>().ok()
        })
        .collect();
    puertos.sort_unstable();
    puertos.dedup();
    puertos
}

/// Lee el estado de un puerto. `None` si no hay estado.
pub fn read_state(state_dir: &Path, port: u16) -> Option<PortState> {
    let texto = std::fs::read_to_string(state_path(state_dir, port)).ok()?;
    let lineas: Vec<String> = texto.lines().map(|l| l.to_string()).collect();
    Some(PortState::from_lines(&lineas))
}

/// Escribe el estado de un puerto, **conservando** las claves que no
/// vienen en el estado dado.
///
/// Las claves **vacías** del estado dado no se escriben: una clave vacía
/// significa «no lo sé», no «bórralo». Es lo que hace que se pueda
/// actualizar sólo el PID sin perder el modelo ni el motor.
///
/// La escritura es **atómica**: fichero temporal + `rename`. Un `status`
/// concurrente no lee un fichero a medias (R-ARQ-9).
pub fn write_state(state_dir: &Path, port: u16, estado: &PortState) -> Result<(), String> {
    let destino = state_path(state_dir, port);
    let dir = destino.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| format!("error de E/S en {}: {e}", dir.display()))?;

    // Sólo se escriben las claves con valor: las vacías son «no lo sé».
    let nuevas: Vec<String> = estado
        .to_lines()
        .into_iter()
        .filter(|linea| match linea.split_once('=') {
            Some((_, valor)) => !valor.is_empty(),
            None => false,
        })
        .collect();

    // Lo que ya estaba y no viene en la llamada se conserva.
    let mut lineas: Vec<String> = Vec::new();
    let claves_nuevas: Vec<&str> = nuevas
        .iter()
        .filter_map(|l| l.split_once('=').map(|(c, _)| c))
        .collect();
    if let Ok(texto) = std::fs::read_to_string(&destino) {
        for linea in texto.lines() {
            let Some((clave, _)) = linea.split_once('=') else {
                continue;
            };
            if claves_nuevas.contains(&clave) {
                continue;
            }
            lineas.push(linea.to_string());
        }
    }
    lineas.extend(nuevas);

    let temporal = destino.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&temporal, lineas.join("\n") + "\n")
        .map_err(|e| format!("error de E/S en {}: {e}", temporal.display()))?;
    std::fs::rename(&temporal, &destino).map_err(|e| {
        let _ = std::fs::remove_file(&temporal);
        format!("error de E/S en {}: {e}", destino.display())
    })?;
    Ok(())
}

/// Borra el estado de un puerto.
pub fn clear_state(state_dir: &Path, port: u16) {
    let _ = std::fs::remove_file(state_path(state_dir, port));
}

/// El sello de una configuración de arranque.
///
/// Es un hash del modelo, el nivel, el contexto y el argv: si el TOML
/// cambia mientras el modelo corre, el sello cambia y el modelo se
/// considera obsoleto (doc B §3.3.4, R-RUN-8).
pub fn stamp_de(parts: &[&str]) -> String {
    let mut hasher = crate::process::Sha256::new();
    for parte in parts {
        hasher.update(parte.as_bytes());
        hasher.update(b"\0");
    }
    let digest = hasher.finalize();
    digest[..12]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}
