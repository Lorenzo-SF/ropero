//! La E/S de la instalación: el terminal, el binario del motor y el enlace.
//!
//! ## Qué hay aquí y por qué
//!
//! Instalar un motor es **crear un enlace** (R-INST-1): ni copiar, ni
//! descargar, ni compilar. Y para crearlo hace falta saber dos cosas que
//! sólo existen fuera del proceso: si hay terminal (para el menú) y dónde
//! está el binario del motor.
//!
//! La **precedencia** del binario es la del doc A (R-INST-24):
//! **PATH → `$BIN_DIR` → rutas largas**. El motor que se usa es el que
//! está en el `PATH`, no el más nuevo ni el recién compilado.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use ropero_core::EngineManifest;
use ropero_core::Environment;

use crate::catalog_scan::es_ejecutable;

/// Si hay terminal: los menús y las preguntas sólo tienen sentido con TTY.
///
/// Sin TTY, `ropero install engines` es un **error de uso** (código 2):
/// un menú que se traga un EOF acabaría eligiendo por su cuenta
/// (R-INST-26).
pub fn hay_tty() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// Escribe una pregunta y lee la respuesta del usuario.
///
/// `None` si no hay nada que leer (EOF): el llamante decide qué hacer.
pub fn pregunta(texto: &str) -> Option<String> {
    print!("{texto}");
    let _ = std::io::stdout().flush();
    let mut linea = String::new();
    match std::io::stdin().lock().read_line(&mut linea) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(linea.trim().to_string()),
    }
}

/// El directorio de binarios: `$ROPERO_BIN_DIR` o `$HOME/.local/bin`.
pub fn bin_dir(env: &Environment) -> PathBuf {
    env.env_path("ROPERO_BIN_DIR", ".local/bin")
}

/// Busca un ejecutable en el `PATH` **del entorno capturado**.
///
/// Se usa el entorno capturado y no el del proceso para que el `PATH` que
/// cuenta sea el que ve el dominio (doc B §3.2.3).
pub fn which_en(env: &Environment, binario: &str) -> Option<PathBuf> {
    let path = env.get("PATH")?;
    std::env::split_paths(path)
        .map(|dir| dir.join(binario))
        .find(|candidato| es_ejecutable(candidato))
}

/// Resuelve el binario de un motor: **PATH → `$BIN_DIR` → rutas largas**
/// (R-INST-24).
pub fn resolve_binario(manifest: &EngineManifest, env: &Environment) -> Option<PathBuf> {
    let dir = bin_dir(env);
    for binario in &manifest.detect.binaries {
        if let Some(candidato) = which_en(env, binario) {
            return Some(candidato);
        }
    }
    for binario in &manifest.detect.binaries {
        let candidato = dir.join(binario);
        if candidato.is_file() {
            return Some(candidato);
        }
    }
    for ruta in &manifest.detect.paths {
        let candidato = PathBuf::from(ruta);
        if candidato.exists() {
            return Some(candidato);
        }
    }
    None
}

/// Lo que pasó al instalar un motor.
pub use ropero_core::install::EstadoMotor;

/// Instala un motor: **sólo crea el enlace** (R-INST-1).
///
/// El nombre del enlace es el `basename` del binario resuelto (R-INST-25).
pub fn instala_motor(manifest: &EngineManifest, env: &Environment) -> Result<EstadoMotor, String> {
    let dir = bin_dir(env);
    let Some(origen) = resolve_binario(manifest, env) else {
        return Ok(EstadoMotor::SinBinario);
    };
    let Some(nombre) = origen.file_name() else {
        return Err(format!(
            "el binario de '{}' no tiene nombre",
            manifest.id.as_str()
        ));
    };
    let destino = dir.join(nombre);
    match std::fs::symlink_metadata(&destino) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                Ok(EstadoMotor::YaEnlazado(destino))
            } else {
                Ok(EstadoMotor::Ocupado(destino))
            }
        }
        Err(_) => {
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("no puedo crear {}: {e}", dir.display()))?;
            std::os::unix::fs::symlink(&origen, &destino)
                .map_err(|e| format!("no puedo enlazar {}: {e}", destino.display()))?;
            Ok(EstadoMotor::Enlazado { origen, destino })
        }
    }
}

/// Si una ruta es un enlace simbólico.
pub fn es_enlace(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

/// Muestra un menú y lee la elección del usuario.
///
/// El menú lo compone el dominio ([`ropero_core::install`]); aquí sólo se
/// enseña y se lee. La respuesta se resuelve con la misma regla del
/// dominio: **números** del menú o **nombres**.
pub fn pregunta_menu(lineas: &[String], opciones: &[String]) -> Result<Vec<String>, String> {
    for linea in lineas {
        ropero_core::out_line(linea);
    }
    ropero_core::out_line("");
    ropero_core::out_line("elige qué instalar (números o ids, separados por espacios o comas):");
    let respuesta = pregunta("> ").ok_or_else(|| "no hay respuesta: se cancela".to_string())?;
    ropero_core::install::resuelve_seleccion(&respuesta, opciones).map_err(|e| e.to_string())
}

/// Pregunta qué vía se usa para un modelo.
pub fn pregunta_via(texto: &str) -> Result<ropero_core::install::Via, String> {
    let respuesta = pregunta(texto).ok_or_else(|| "no hay respuesta: se cancela".to_string())?;
    ropero_core::install::Via::de_respuesta(&respuesta)
        .ok_or_else(|| "respuesta no válida: escribe 'd' (download) o 'b' (build)".to_string())
}
