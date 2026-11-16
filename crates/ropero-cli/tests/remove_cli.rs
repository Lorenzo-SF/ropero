use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ropero"))
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ropero-cli-remove-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn base_scan_env(cmd: &mut Command, xdg: &PathBuf) {
    cmd.env("XDG_CONFIG_HOME", xdg)
        .env("HOME", xdg)
        .env_remove("ROPERO_MODELS_DIR")
        .env_remove("ROPERO_ROOT");
}

#[test]
fn remove_sin_tty_no_borra_y_falla() {
    let base = tmp("notty");
    let xdg = base.join("xdg");
    let models = xdg.join("ropero").join("models").join("llama-server");
    let engines = xdg.join("ropero").join("engines");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::create_dir_all(&engines).unwrap();
    std::fs::write(
        models.join("demo.toml"),
        r#"
[model]
name = "demo"
engine = "llama-server"
context = 1

[files]
gguf = "demo.gguf"
"#,
    )
    .unwrap();
    let gguf_root = base.join("gguf").join("demo");
    std::fs::create_dir_all(&gguf_root).unwrap();
    let gguf = gguf_root.join("demo.gguf");
    std::fs::write(&gguf, b"x").unwrap();

    let mut c = bin();
    base_scan_env(&mut c, &xdg);
    c.args(["demo", "remove"])
        .env("GGUF_DIR", base.join("gguf"));
    let out = c.output().unwrap();
    assert!(
        !out.status.success(),
        "stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("TTY") || stderr.contains("confirmación"),
        "stderr={stderr}"
    );
    assert!(gguf.exists(), "no debe borrar sin confirmación/TTY");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn remove_ayuda() {
    let mut c = bin();
    c.args(["demo", "remove", "--help"]);
    let out = c.output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("USO:  ropero <alias> remove"), "{stdout}");
    assert!(stdout.contains("confirmación interactiva"), "{stdout}");
}

#[test]
fn remove_modelo_desconocido() {
    let base = tmp("unknown");
    let xdg = base.join("xdg");
    std::fs::create_dir_all(xdg.join("ropero").join("models")).unwrap();
    std::fs::create_dir_all(xdg.join("ropero").join("engines")).unwrap();
    let mut c = bin();
    base_scan_env(&mut c, &xdg);
    c.args(["noexiste", "remove"]);
    let out = c.output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("modelo desconocido"), "{stderr}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn remove_safetensors_sin_tty_muestra_plan_y_no_borra() {
    let base = tmp("st");
    let xdg = base.join("xdg");
    let models = xdg.join("ropero").join("models").join("vllm");
    let engines = xdg.join("ropero").join("engines");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::create_dir_all(&engines).unwrap();
    std::fs::write(
        models.join("ckpt.toml"),
        r#"
[model]
name = "ckpt"
engine = "vllm"
context = 1
format = "safetensors"
path = "familia/modelo"

[files]
"#,
    )
    .unwrap();
    let checkpoint = base.join("st/familia/modelo");
    std::fs::create_dir_all(&checkpoint).unwrap();
    std::fs::write(checkpoint.join("weights.safetensors"), b"weights").unwrap();
    let mut c = bin();
    base_scan_env(&mut c, &xdg);
    c.args(["ckpt", "remove"])
        .env("SAFETENSORS_DIR", base.join("st"));
    let out = c.output().unwrap();
    assert!(!out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("familia/modelo"), "{stdout}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no se ha eliminado nada"), "{stderr}");
    assert!(checkpoint.join("weights.safetensors").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn remove_avisa_y_conserva_checkpoint_compartido() {
    let base = tmp("shared");
    let xdg = base.join("xdg");
    let models = xdg.join("ropero").join("models").join("vllm");
    let engines = xdg.join("ropero").join("engines");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::create_dir_all(&engines).unwrap();
    for (name, file) in [("ckpt-a", "ckpt-a.toml"), ("ckpt-b", "ckpt-b.toml")] {
        std::fs::write(
            models.join(file),
            format!(
                "[model]\nname = \"{name}\"\nengine = \"vllm\"\ncontext = 1\nformat = \"safetensors\"\npath = \"familia/modelo\"\n\n[files]\n"
            ),
        )
        .unwrap();
    }
    let checkpoint = base.join("st/familia/modelo");
    std::fs::create_dir_all(&checkpoint).unwrap();
    std::fs::write(checkpoint.join("weights.safetensors"), b"weights").unwrap();

    let mut c = bin();
    base_scan_env(&mut c, &xdg);
    c.args(["ckpt-a", "remove"])
        .env("SAFETENSORS_DIR", base.join("st"));
    let out = c.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("se conserva"), "stdout={stdout}");
    assert!(stdout.contains("ckpt-b"), "stdout={stdout}");
    assert!(stdout.contains("se conservan"), "stdout={stdout}");
    assert!(checkpoint.join("weights.safetensors").exists());
    let _ = std::fs::remove_dir_all(&base);
}
