//! `ropero test` desde fuera: los códigos de salida y los mensajes.
//!
//! La batería vive en `docs/tests/`, así que el catálogo que se apunta es
//! el del repo (que tiene las dos cosas): los modelos de verdad y la
//! batería.

use std::path::{Path, PathBuf};
use std::process::Command;

/// La raíz del repo: el catálogo y la batería están los dos ahí.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// El comando `ropero`, con el estado en un directorio de usar y tirar.
fn bin() -> Command {
    let estado = std::env::temp_dir().join("ropero-cli-bateria-estado");
    let _ = std::fs::remove_dir_all(&estado);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ropero"));
    cmd.env("ROPERO_MODELS_DIR", repo_root());
    cmd.env("ROPERO_STATE_DIR", &estado);
    cmd
}

/// Ejecuta el binario y devuelve (exit code, stdout, stderr).
fn run(args: &[&str]) -> (i32, String, String) {
    let out = bin().args(args).output().expect("el binario se ejecuta");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Un grupo que no existe es un **error de uso** (código 2) que nombra lo
/// que se recibió.
#[test]
fn un_grupo_que_no_existe_sale_con_dos_y_nombra_el_valor() {
    let (code, _stdout, stderr) = run(&["test", "--model", "no-existe", "--test-group", "Z"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(stderr.contains('Z'), "nombra el valor: {stderr}");
    assert!(stderr.contains('F'), "dice qué grupos hay: {stderr}");
}

/// Dos selecciones a la vez son un error de uso.
#[test]
fn dos_selecciones_a_la_vez_salen_con_dos() {
    let (code, _stdout, stderr) =
        run(&["test", "--model", "m", "--test-group", "D", "--test", "F2"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(stderr.contains("D"), "nombra el grupo: {stderr}");
    assert!(stderr.contains("F2"), "nombra el test: {stderr}");
}

/// Sin terminal no se puede elegir el modelo: es un error de uso que dice
/// qué hacer.
#[test]
fn sin_argumentos_y_sin_terminal_no_se_abre_asistente() {
    let (code, _stdout, stderr) = run(&["test"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(stderr.contains("terminal"), "dice qué hacer: {stderr}");
}

/// Un modelo que no está en el catálogo es un error de uso.
#[test]
fn un_modelo_desconocido_sale_con_dos() {
    let (code, _stdout, stderr) = run(&["test", "--model", "no-existe", "--test-group", "F"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(stderr.contains("no-existe"), "nombra el modelo: {stderr}");
}

/// La ayuda del comando enseña sus flags.
#[test]
fn la_ayuda_enseña_los_flags_de_la_bateria() {
    let (code, stdout, stderr) = run(&["test", "--help"]);
    assert_eq!(code, 0, "stderr={stderr}");
    for flag in ["--model", "--test", "--test-group", "--path"] {
        assert!(stdout.contains(flag), "la ayuda no enseña {flag}: {stdout}");
    }
}
