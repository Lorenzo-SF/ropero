//! Tests de integración de la CLI contra un catálogo de pruebas.
//!
//! La batería es hermética: no depende del catálogo real (`ropero-code/models`,
//! que sigue siendo de la v1 hasta la fase F9/T-05). En su lugar se usa el
//! catálogo de pruebas de `tests/fixtures/models`, que se apunta con la
//! variable de entorno `ROPERO_MODELS_DIR` (honorada por
//! `ropero_cli::discovery::find_model_root`).
//!
//! El catálogo de pruebas contiene, exactamente:
//! - 2 modelos activos: `modelo-alpha` (motor `llama-server`) y `modelo-beta`
//!   (motor `vllm`).
//! - 1 modelo retirado en `fired/`: `modelo-retirado`.
//! - 1 `_template.toml` que el comando `template` vuelca.
//!
//! Todos los datos son ficticios: no hay credenciales, tokens ni rutas
//! absolutas en los TOML del fixture.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Raíz del catálogo de pruebas: `<cargo-manifest-dir>/tests/fixtures`.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Comando `ropero` con el entorno limpio y apuntando al catálogo de pruebas.
///
/// Se fijan `ROPERO_MODELS_DIR` y las variables de rutas (`GGUF_DIR`,
/// `SAFETENSORS_DIR`, `LOG_DIR`) para que la salida no dependa del entorno
/// de quien ejecuta los tests.
fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ropero"));
    cmd.env("ROPERO_MODELS_DIR", fixture_root());
    cmd.env("GGUF_DIR", fixture_root().join("gguf"));
    cmd.env("SAFETENSORS_DIR", fixture_root().join("safetensors"));
    cmd.env("LOG_DIR", fixture_root().join("logs"));
    cmd
}

/// Ejecuta el binario y devuelve (exit code, stdout, stderr).
fn run(args: &[&str]) -> (i32, String, String) {
    let out = bin().args(args).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Filas de `list`: el prefijo de dos espacios es el que usa la CLI.
fn list_rows(stdout: &str) -> Vec<&str> {
    stdout.lines().collect()
}

#[test]
fn validate_ok() {
    let (code, stdout, stderr) = run(&["validate"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(stdout.contains("OK:"), "stdout={}", stdout);
    // 4 ficheros TOML del fixture: 3 activos + 1 retirado.
    assert!(stdout.contains("4 modelo(s)"), "stdout={}", stdout);
}

#[test]
fn list_prefix() {
    let (code, stdout, stderr) = run(&["list"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let rows = list_rows(&stdout);
    assert_eq!(rows.len(), 3, "stdout={}", stdout);
    for line in rows {
        assert!(line.starts_with("  "), "line={}", line);
    }
    assert!(stdout.contains("modelo-alpha"), "stdout={}", stdout);
    assert!(stdout.contains("modelo-beta"), "stdout={}", stdout);
}

#[test]
fn list_all_count() {
    let (code, stdout, stderr) = run(&["list", "--all"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    // 3 activos + 1 retirado del fixture (no los 45 del catálogo real).
    assert_eq!(list_rows(&stdout).len(), 4, "stdout={}", stdout);
    assert!(stdout.contains("modelo-retirado"), "stdout={}", stdout);
}

#[test]
fn list_hides_fired_by_default() {
    let (code, stdout, stderr) = run(&["list"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(
        !stdout.contains("modelo-retirado"),
        "los retirados no salen sin --all: stdout={}",
        stdout
    );
}

#[test]
fn template_outputs() {
    let (code, stdout, stderr) = run(&["template"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(stdout.contains("[model]"), "stdout={}", stdout);
    assert!(stdout.contains("[args]"), "stdout={}", stdout);
}

#[test]
fn levels_lines() {
    let (code, stdout, stderr) = run(&["levels", "modelo-alpha"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let mut rows: Vec<&str> = stdout.lines().collect();
    rows.sort();
    assert_eq!(rows, vec!["high", "low", "medium"], "stdout={}", stdout);
    assert!(!stdout.contains("nivel"));
}

#[test]
fn levels_of_model_without_levels_is_empty() {
    let (code, stdout, stderr) = run(&["levels", "modelo-beta"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert_eq!(stdout.lines().count(), 0, "stdout={}", stdout);
}

#[test]
fn args_lines() {
    let (code, stdout, stderr) = run(&["args", "modelo-alpha"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let rows = list_rows(&stdout);
    assert!(!rows.is_empty(), "stdout={}", stdout);
    assert!(rows.contains(&"--n-gpu-layers"), "stdout={}", stdout);
    // El nivel por defecto del fixture (`low`) se monta en el argv.
    assert!(rows.contains(&"--reasoning-budget"), "stdout={}", stdout);
}

#[test]
fn path_lines() {
    let (code, stdout, stderr) = run(&["path", "modelo-alpha"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(
        stdout.lines().any(|l| l.starts_with("GGUF=")),
        "stdout={}",
        stdout
    );
    assert!(
        stdout.lines().any(|l| l.starts_with("LOG=")),
        "stdout={}",
        stdout
    );
    // La ruta resuelta cuelga del catálogo de pruebas, no del HOME real.
    assert!(
        stdout.contains("modelo-alpha-q4_k_m.gguf"),
        "stdout={}",
        stdout
    );
}

#[test]
fn unknown_model_is_usage_error() {
    let (code, _stdout, stderr) = run(&["args", "no-existe"]);
    assert_eq!(code, 2, "stderr={}", stderr);
    assert!(stderr.contains("modelo desconocido"), "stderr={}", stderr);
}

#[test]
fn usage_error_exit2() {
    let (code, _stdout, stderr) = run(&["list", "--bogus"]);
    assert_eq!(code, 2, "stderr={}", stderr);
}

// ─────────────────────────────────────────────────────────────────────────────
// Alias de nivel (`[aliases]`)
// ─────────────────────────────────────────────────────────────────────────────

/// Un alias de nivel se monta como su nivel canónico: mismo argv.
#[test]
fn alias_de_nivel_monta_el_nivel_canónico() {
    let (code_alias, stdout_alias, stderr) = run(&["args", "modelo-alpha", "--alto"]);
    assert_eq!(code_alias, 0, "stderr={}", stderr);
    let (code_nivel, stdout_nivel, stderr) = run(&["args", "modelo-alpha", "--high"]);
    assert_eq!(code_nivel, 0, "stderr={}", stderr);
    assert_eq!(stdout_alias, stdout_nivel);
    // El alias apunta a "high": el argv lleva su presupuesto de razonamiento.
    assert!(
        stdout_alias.contains("--reasoning-budget"),
        "stdout={}",
        stdout_alias
    );
    assert!(stdout_alias.contains("16384"), "stdout={}", stdout_alias);
}

/// Un alias que no existe es error de uso (exit 2).
#[test]
fn alias_de_nivel_inexistente_es_error_de_uso() {
    let (code, _stdout, stderr) = run(&["args", "modelo-alpha", "--no-existe"]);
    assert_eq!(code, 2, "stderr={}", stderr);
}

// ─────────────────────────────────────────────────────────────────────────────
// `replace` en el argv
// ─────────────────────────────────────────────────────────────────────────────

/// `replace` sustituye la pareja del base: "--temp 0.7" no aparece cuando
/// se pide `--temp=0.9`.
#[test]
fn replace_sustituye_la_pareja_del_base() {
    let (code, stdout, stderr) = run(&["args", "modelo-alpha", "--temp=0.9"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let rows = list_rows(&stdout);
    assert_eq!(
        rows.iter().filter(|r| **r == "--temp").count(),
        1,
        "stdout={}",
        stdout
    );
    assert!(rows.contains(&"0.9"), "stdout={}", stdout);
    assert!(!rows.contains(&"0.7"), "stdout={}", stdout);
}

/// Sin pedir la opción, el base no se toca: "--temp 0.7" sigue ahí.
#[test]
fn replace_no_toca_el_base_si_no_se_pide_la_opcion() {
    let (code, stdout, stderr) = run(&["args", "modelo-alpha"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let rows = list_rows(&stdout);
    assert!(rows.contains(&"0.7"), "stdout={}", stdout);
    assert_eq!(
        rows.iter().filter(|r| **r == "--temp").count(),
        1,
        "stdout={}",
        stdout
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Alias legacy de motor (T-11)
// ─────────────────────────────────────────────────────────────────────────────

/// Un motor escrito con un alias legacy se canonicaliza **con aviso**, con
/// el formato literal del doc B §4.2, y el catálogo guarda el id canónico.
#[test]
fn el_alias_legacy_de_motor_se_canonicaliza_con_aviso() {
    let (code, stdout, stderr) = run(&["list"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(
        stderr.contains("AVISO: el motor 'mlx_lm' es un alias antiguo; se usa 'mlx-serve'"),
        "stderr={}",
        stderr
    );
    // La fila del catálogo lleva ya el id canónico.
    assert!(stdout.contains("engine=mlx-serve"), "stdout={}", stdout);
}

/// El aviso de canonicalización no es un error: `validate` sigue en verde.
#[test]
fn el_aviso_de_canonicalizacion_no_es_error() {
    let (code, stdout, stderr) = run(&["validate"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(stdout.contains("OK:"), "stdout={}", stdout);
    assert!(
        stderr.contains("AVISO: el motor 'mlx_lm'"),
        "stderr={}",
        stderr
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// La ayuda
// ─────────────────────────────────────────────────────────────────────────────

/// La ayuda sólo anuncia lo que existe: cada comando que anuncia está
/// implementado, y no anuncia el arranque (que es de la fase F3).
#[test]
fn la_ayuda_solo_anuncia_lo_que_existe() {
    let (code, stdout, stderr) = run(&["help"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    for comando in ["validate", "list", "template", "args", "levels", "path"] {
        assert!(
            stdout.contains(comando),
            "la ayuda no anuncia '{comando}': stdout={}",
            stdout
        );
        // Y el comando está implementado: sin argumentos puede pedir el
        // modelo (error de uso), pero no puede ser "desconocido".
        let (_code, _out, stderr) = run(&[comando]);
        assert!(
            !stderr.contains("desconocido"),
            "'{comando}' está anunciado pero no existe: stderr={}",
            stderr
        );
    }
}

/// La ayuda no promete el arranque de un modelo: `ropero <modelo>` no
/// existe todavía (es de la fase F3).
#[test]
fn la_ayuda_no_anuncia_el_arranque_de_un_modelo() {
    let (_code, stdout, _stderr) = run(&["help"]);
    assert!(!stdout.contains("--port"), "stdout={}", stdout);
    assert!(!stdout.contains("--background"), "stdout={}", stdout);
    assert!(!stdout.contains("<modelo> [--"), "stdout={}", stdout);
}
