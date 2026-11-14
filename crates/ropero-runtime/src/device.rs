//! El device: los procesos de ropero que están vivos.
//!
//! ## Por qué no se busca por nombre
//!
//! Un `pkill -f llama-server` mataría **todos** los llama-server de la
//! máquina, incluidos los que arrancó otro y los de otro proyecto. El
//! `--force` del doc CLI v2 §8.2 dice «si **otros procesos lanzados por
//! ropero** ocupan el device, matarlos»: sólo los de ropero.
//!
//! ## Cómo se sabe cuáles son
//!
//! Por el **estado en disco**: los procesos que ropero lanzó son los que
//! tienen fichero en `$STATE_DIR/ports/`. Y cada uno se verifica con su
//! identidad completa (PID + `start_time` + digest del binario), para que
//! un PID reciclado no recibe un SIGTERM.

use crate::launch::Device;
use crate::process::ProcIdentity;
use crate::state::{managed_ports, read_state, state_dir};

/// El device real: el que mira el estado en disco.
pub struct StateDevice {
    /// El directorio de estado.
    pub state_dir: std::path::PathBuf,
}

impl StateDevice {
    /// Un device que mira el estado en el directorio dado.
    pub fn new(state_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
        }
    }

    /// Un device con el directorio de estado por defecto.
    ///
    /// El directorio sale del entorno del proceso (`ROPERO_STATE_DIR` o
    /// `$HOME/.local/state/ropero`).
    pub fn del_entorno() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let var = std::env::var("ROPERO_STATE_DIR").ok();
        Self::new(state_dir(std::path::Path::new(&home), var.as_deref()))
    }
}

impl Device for StateDevice {
    fn ropero_processes(&self) -> Vec<ProcIdentity> {
        let mut identidades: Vec<ProcIdentity> = Vec::new();
        for port in managed_ports(&self.state_dir) {
            let Some(estado) = read_state(&self.state_dir, port) else {
                continue;
            };
            let Some(pid) = estado.pid else {
                continue;
            };
            // La identidad **completa** del estado: si el PID fue
            // reciclado, el `start_time` no coincide y el proceso no
            // cuenta como nuestro.
            let esperada = ProcIdentity {
                pid,
                start_time: estado.start_time.unwrap_or(0),
                exe_digest: estado.exe_digest.as_deref().and_then(parse_digest),
            };
            let Some(actual) = ProcIdentity::of(pid) else {
                continue;
            };
            // Sin `start_time` en el estado no se puede probar nada: se
            // salta (el estado lo escribió otra versión).
            if estado.start_time.is_none() {
                continue;
            }
            if !actual.es_la_misma(&esperada) {
                continue;
            }
            identidades.push(actual);
        }
        identidades
    }
}

/// Un digest hexadecimal de vuelta a bytes.
fn parse_digest(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}
