//! Comprobación del catálogo real de la v1 (`ropero-code/models`).
//!
//! Este test está marcado `#[ignore]` a propósito: el catálogo real sigue
//! viviendo en `ropero-code/` (la v1 lo resuelve con `ROPERO_ROOT` relativo
//! a `ropero-code/`) hasta la fase F9/T-05, cuando el catálogo pase a la raíz
//! del workspace. Mientras tanto, la batería por defecto (`cargo test`)
//! debe ser hermética y no depender de ese directorio.
//!
//! Cómo ejecutarlo:
//!
//! ```text
//! cargo test --locked --workspace -- --ignored
//! ```
//!
//! Si `ropero-code/models` no existe (por ejemplo, en una copia del repo sin
//! la v1), el test se omite con un mensaje claro en vez de fallar.

use std::path::{Path, PathBuf};

/// Nº de modelos activos esperados en el catálogo real.
const ACTIVOS_ESPERADOS: usize = 17;
/// Nº de modelos retirados (`fired/`) esperados en el catálogo real.
const FIRED_ESPERADOS: usize = 28;
/// Nº total de modelos del catálogo real (activos + retirados).
const TOTAL_ESPERADO: usize = ACTIVOS_ESPERADOS + FIRED_ESPERADOS;

/// Base del catálogo real: `<repo-root>/ropero-code/models`.
fn base_real() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("ropero-code")
        .join("models")
}

/// Recoge los TOML de un directorio, ordenados (orden explícito: el del
/// sistema de ficheros no se supone estable).
fn tomls(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("toml"))
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

/// Recoge los TOML de `<base>/<motor>` y de `<base>/<motor>/fired/`.
fn recoge(base: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut active = Vec::new();
    let mut fired = Vec::new();
    let mut dirs: Vec<PathBuf> = match std::fs::read_dir(base) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => Vec::new(),
    };
    dirs.sort();
    for dir in dirs {
        active.extend(tomls(&dir));
        fired.extend(tomls(&dir.join("fired")));
    }
    (active, fired)
}

#[test]
#[ignore = "el catálogo real vive en ropero-code/ hasta la fase F9/T-05; ejecutar con `cargo test --locked --workspace -- --ignored`"]
fn catalogo_real_tiene_45_modelos() {
    let base = base_real();
    if !base.is_dir() {
        eprintln!(
            "omitido: no existe el catálogo real en {} (la v1 no está presente)",
            base.display()
        );
        return;
    }
    let (active, fired) = recoge(&base);
    assert_eq!(
        active.len(),
        ACTIVOS_ESPERADOS,
        "modelos activos en {}",
        base.display()
    );
    assert_eq!(
        fired.len(),
        FIRED_ESPERADOS,
        "modelos retirados en {}",
        base.display()
    );
    assert_eq!(
        active.len() + fired.len(),
        TOTAL_ESPERADO,
        "total de modelos en {}",
        base.display()
    );
}

/// El `_template.toml` del catálogo real debe existir y declarar `[model]`.
#[test]
#[ignore = "el catálogo real vive en ropero-code/ hasta la fase F9/T-05; ejecutar con `cargo test --locked --workspace -- --ignored`"]
fn catalogo_real_tiene_plantilla() {
    let base = base_real();
    if !base.is_dir() {
        eprintln!(
            "omitido: no existe el catálogo real en {} (la v1 no está presente)",
            base.display()
        );
        return;
    }
    let template = base.join("_template.toml");
    assert!(template.is_file(), "falta {}", template.display());
    let text = std::fs::read_to_string(&template).expect("leer la plantilla");
    assert!(text.contains("[model]"), "la plantilla no declara [model]");
}
