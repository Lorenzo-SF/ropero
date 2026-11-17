//! Tests CLI de configuración: `config`, `doctor` y política de bootstrap.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ropero"))
}

fn base_env(cmd: &mut Command, home: &PathBuf, cwd: &PathBuf, xdg: &PathBuf) {
    cmd.current_dir(cwd)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", xdg)
        .env_remove("ROPERO_MODELS_DIR")
        .env_remove("ROPERO_ROOT")
        .env_remove("GGUF_DIR")
        .env_remove("SAFETENSORS_DIR")
        .env_remove("LOG_DIR")
        .env_remove("ROPERO_BIN_DIR");
}

/// `config` y `doctor` muestran el estado del archivo y su origen.
#[test]
fn config_y_doctor_reportan_estado_y_origen() {
    let base = std::env::temp_dir().join("ropero-cli-config-doctor");
    let _ = std::fs::remove_dir_all(&base);
    let home = base.join("home");
    let cwd = base.join("cwd");
    let xdg = base.join("xdg");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&home).unwrap();

    let mut c = bin();
    base_env(&mut c, &home, &cwd, &xdg);
    c.arg("config");
    let out = c.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "stderr={stderr}");
    assert!(stdout.contains("CONFIGURATION"), "stdout={stdout}");
    assert!(stdout.contains("PATHS"), "stdout={stdout}");
    assert!(
        stdout.contains("config.toml file") || stdout.contains("default"),
        "stdout={stdout}"
    );

    let mut d = bin();
    base_env(&mut d, &home, &cwd, &xdg);
    d.arg("doctor");
    let out = d.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("CONFIGURACIÓN"),
        "stdout={stdout}\nstderr={stderr}"
    );
    // puede ser 0 o 1 según deps del sistema, pero no debe ocultar config
    assert!(
        stdout.contains("archivo de configuración") || stdout.contains("estado"),
        "stdout={stdout}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// Un override de dev no debe hacer que se auto-descubra el repo como catálogo.
/// Con cwd dentro de un repo con `models/`, sin overrides, no debe usarlo
/// implícitamente: usa el catálogo externo de config.
#[test]
fn sin_overrides_no_autodescubre_repo() {
    let base = std::env::temp_dir().join("ropero-cli-no-autodiscover");
    let _ = std::fs::remove_dir_all(&base);
    let home = base.join("home");
    let cwd_repo = base.join("repo-falso");
    let models_repo = cwd_repo.join("models");
    let xdg = base.join("xdg");
    std::fs::create_dir_all(models_repo.join("turbo-llama")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    // modelo en repo-falso
    std::fs::write(
        models_repo.join("turbo-llama").join("m.toml"),
        "[engine]\nid = \"turbo-llama\"\n[model]\nid = \"solo-repo\"\n[weights]\npath = \"x.gguf\"\nsource = \"local\"\n[backend]\nname = \"llama.cpp\"\nbinary = \"llama-server\"\nmode = \"inprocess\"\n[levels.mid]\nname = \"mid\"\ndefault = true\n",
    )
    .unwrap();

    let mut c = bin();
    base_env(&mut c, &home, &cwd_repo, &xdg);
    c.arg("config");
    let out = c.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "stderr={stderr}");
    let esperado = xdg.join("ropero").join("models");
    assert!(
        stdout.contains(esperado.to_str().unwrap()),
        "debe usar catálogo externo, no repo: stdout={stdout}"
    );
    assert!(
        !stdout.contains(models_repo.to_str().unwrap()),
        "no debe autodetectar models/ del repo: stdout={stdout}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// Configuración ilegible: `config` y `doctor` la muestran y fallan; no se oculta.
#[test]
fn config_ilegible_se_reporta_en_config_y_doctor() {
    let base = std::env::temp_dir().join("ropero-cli-config-ilegible");
    let _ = std::fs::remove_dir_all(&base);
    let home = base.join("home");
    let cwd = base.join("cwd");
    let xdg = base.join("xdg");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&home).unwrap();

    // bootstrap inicial
    let mut c = bin();
    base_env(&mut c, &home, &cwd, &xdg);
    c.arg("config");
    let _ = c.output().unwrap();
    let archivo = xdg.join("ropero").join("config.toml");
    std::fs::write(&archivo, "gguf_dir = [\n").unwrap();

    let mut c = bin();
    base_env(&mut c, &home, &cwd, &xdg);
    c.arg("config");
    let out = c.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(
        out.status.code(),
        Some(0),
        "debe fallar; stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("no vale") || stderr.contains("ilegible") || stdout.contains("config.toml"),
        "stdout={stdout} stderr={stderr}"
    );

    let mut d = bin();
    base_env(&mut d, &home, &cwd, &xdg);
    d.arg("doctor");
    let out = d.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_ne!(
        out.status.code(),
        Some(0),
        "doctor debe fallar con config ilegible: stdout={stdout}"
    );
    assert!(
        stdout.contains("no vale") || stdout.contains("qué hacer"),
        "stdout={stdout}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// Con override ROPERO_MODELS_DIR, el bootstrap sigue creando config en XDG (ubicación de usuario),
/// no en el repo/override, y el archivo no persiste el override.
#[test]
fn override_no_impide_bootstrap_xdg_y_no_persiste_override() {
    let base = std::env::temp_dir().join("ropero-cli-override-xdg");
    let _ = std::fs::remove_dir_all(&base);
    let home = base.join("home");
    let cwd = base.join("cwd");
    let xdg = base.join("xdg");
    let override_models = base.join("override-models");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(override_models.join("turbo-llama")).unwrap();

    let mut c = bin();
    base_env(&mut c, &home, &cwd, &xdg);
    c.env("ROPERO_MODELS_DIR", override_models.as_os_str());
    c.arg("config");
    let out = c.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={stderr} stdout={stdout}"
    );
    let archivo = xdg.join("ropero").join("config.toml");
    assert!(archivo.exists(), "debe crear config en XDG de usuario");
    let contenido = std::fs::read_to_string(&archivo).unwrap();
    assert!(
        !contenido.contains(override_models.to_str().unwrap()),
        "no debe persistir override: {contenido}"
    );
    assert!(
        stdout.contains(override_models.to_str().unwrap()),
        "efectivo debe usar override: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&base);
}
