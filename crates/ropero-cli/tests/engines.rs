//! Tests de integración del comando `ropero engines`.
//!
//! El comando lista los motores con su color, su madurez y su rango de
//! puertos, y dice cuáles encajan con esta máquina y por qué no los demás.
//!
//! La batería es hermética: el catálogo de pruebas es el de
//! `tests/fixtures`, y los manifiestos de motor son los **reales** del repo
//! (`<repo-root>/engines`), que son datos y no dependen de la máquina.

use std::path::Path;
use std::process::Command;

/// Raíz del catálogo de pruebas: `<cargo-manifest-dir>/tests/fixtures`.
fn fixture_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// La raíz del repo: `<cargo-manifest-dir>/../..`.
fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Corre `ropero` contra el catálogo de pruebas.
fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ropero"))
        .args(args)
        .env("ROPERO_MODELS_DIR", fixture_root())
        .env(
            "XDG_CONFIG_HOME",
            std::env::temp_dir().join("ropero-cli-engines-config"),
        )
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

/// `ropero engines` sale con 0 y lista los motores.
#[test]
fn engines_sale_con_cero_y_lista_los_motores() {
    let (code, stdout, stderr) = run(&["engines"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(stdout.contains("llama-server"), "stdout={stdout}");
    assert!(stdout.contains("turbo-llama"), "stdout={stdout}");
    // La leyenda de colores del doc CLI v2 §4.
    assert!(stdout.contains("naranja"), "stdout={stdout}");
    assert!(stdout.contains("amarillo"), "stdout={stdout}");
}

/// La lista muestra el color, la madurez y el rango de puertos.
#[test]
fn la_linea_del_motor_muestra_color_madurez_y_puertos() {
    let (code, stdout, stderr) = run(&["engines"]);
    assert_eq!(code, 0, "stderr={stderr}");
    let linea = stdout
        .lines()
        .find(|l| l.contains("llama-server"))
        .expect("debe salir la línea de llama-server");
    assert!(linea.contains("naranja"), "linea={linea}");
    assert!(linea.contains("proven"), "linea={linea}");
    assert!(linea.contains("9980-9989"), "linea={linea}");
}

/// Los motores que no encajan con la máquina salen con `--all` y **dicen
/// por qué**.
#[test]
fn los_motores_incompatibles_dicen_por_que() {
    let (code, stdout, stderr) = run(&["engines", "--all"]);
    assert_eq!(code, 0, "stderr={stderr}");
    // `mlx-serve` sólo encaja en macOS Apple Silicon con Metal. En macOS la
    // fila es compatible y no tiene nota; en el resto de plataformas explica
    // el motivo de incompatibilidad debajo de la fila.
    let lineas: Vec<&str> = stdout.lines().collect();
    let posicion = lineas
        .iter()
        .position(|l| l.contains("mlx-serve"))
        .expect("debe salir la línea de mlx-serve");
    if cfg!(target_os = "macos") {
        assert!(
            !lineas[posicion].contains("no:"),
            "fila={}",
            lineas[posicion]
        );
    } else {
        // El motivo va en la nota de debajo de su fila (doc CLI v2 §2).
        let nota = lineas
            .get(posicion + 1)
            .expect("debe salir la nota del motivo");
        assert!(nota.contains("no:"), "nota={nota}");
        assert!(
            nota.contains("macOS") || nota.contains("Metal"),
            "nota={nota}"
        );
    }
}

/// `ropero engines --help` sale con 0 y enseña el uso.
#[test]
fn engines_help_sale_con_cero() {
    let (code, stdout, stderr) = run(&["engines", "--help"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("USAGE:") || stdout.contains("USO:"),
        "stdout={stdout}"
    );
}

/// Un flag que el comando no conoce es un error de uso (exit 2).
#[test]
fn engines_con_un_flag_desconocido_es_error_de_uso() {
    let (code, _stdout, stderr) = run(&["engines", "--inventado"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("--inventado") || stderr.contains("unexpected argument"),
        "stderr={stderr}"
    );
}

/// El comando `engines` está registrado: no es un comando desconocido.
#[test]
fn engines_no_es_un_comando_desconocido() {
    let (code, _stdout, stderr) = run(&["engines"]);
    assert_ne!(code, 2, "stderr={stderr}");
    assert!(!stderr.contains("comando desconocido"), "stderr={stderr}");
}

/// Los manifiestos reales son los del repo: el comando los lee de ahí.
#[test]
fn los_manifiestos_reales_estan_en_la_raiz_del_repo() {
    let engines = repo_root().join("engines");
    assert!(engines.is_dir(), "falta {}", engines.display());
    for id in [
        "llama-server",
        "turbo-llama",
        "fastflowllm",
        "mlx-serve",
        "airllm",
        "tensorrt-llm",
        "vllm",
        "strata",
    ] {
        assert!(
            engines.join(format!("{id}.toml")).is_file(),
            "falta el manifiesto de {id}"
        );
    }
}
