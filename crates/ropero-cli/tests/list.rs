//! Tests de `ropero list`: los modelos disponibles, agrupados por motor,
//! con la leyenda de colores.
//!
//! Van en su propio fichero porque son los del bloque 2 del ciclo: el
//! listado con grupos y colores es lo que esa bloque trae.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Raíz del catálogo de pruebas: `<cargo-manifest-dir>/tests/fixtures`.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Corre `ropero` contra el catálogo de pruebas.
fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ropero"))
        .args(args)
        .env("ROPERO_MODELS_DIR", fixture_root())
        .env("GGUF_DIR", fixture_root().join("gguf"))
        .env("SAFETENSORS_DIR", fixture_root().join("safetensors"))
        .env("LOG_DIR", fixture_root().join("logs"))
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// está instalado, su grupo no se despliega.
#[test]
fn list_agrupa_por_motor() {
    let (code, stdout, stderr) = run(&["list"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    // Los motores del fixture salen, cada uno en su color de la leyenda.
    assert!(stdout.contains("turbo-llama"), "stdout={}", stdout);
    assert!(stdout.contains("llama-server"), "stdout={}", stdout);
    // Y un motor no instalado dice por qué.
    assert!(
        stdout.contains("no está instalado"),
        "el grupo plegado no dice la razón: stdout={}",
        stdout
    );
}

/// instalado: no están disponibles (doc CLI v2 §15).
#[test]
fn list_no_saca_los_no_disponibles() {
    let (code, stdout, stderr) = run(&["list"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(
        !stdout.contains("modelo-alpha"),
        "un modelo de un motor no instalado no está disponible: stdout={}",
        stdout
    );
    assert!(
        !stdout.contains("modelo-beta"),
        "un modelo de un motor no instalado no está disponible: stdout={}",
        stdout
    );
}

/// `ropero list --all` saca también los que no están disponibles, marcados.
#[test]
fn list_all_marca_los_no_disponibles() {
    let (code, stdout, stderr) = run(&["list", "--all"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(stdout.contains("modelo-alpha"), "stdout={}", stdout);
    assert!(
        stdout.contains("no disponible"),
        "lo no disponible va marcado: stdout={}",
        stdout
    );
}

/// leyenda.
#[test]
fn list_pinta_con_el_color_de_la_leyenda() {
    let (code, stdout, stderr) = run(&["list"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    // La leyenda del doc CLI v2 §4: turbo-llama es amarillo (220).
    assert!(
        stdout.contains("\x1b[38;5;220m"),
        "el motor no va en su color: stdout={:?}",
        stdout
    );
}
