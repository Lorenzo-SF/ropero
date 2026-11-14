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

// ─────────────────────────────────────────────────────────────────────────────
// Un motor desconocido no aborta el escaneo (R-DATA-12)
// ─────────────────────────────────────────────────────────────────────────────

/// Monta un catálogo en el disco con un directorio de motor **desconocido**
/// (`motor-novedoso/`) junto a los modelos válidos del fixture, y devuelve
/// su raíz.
///
/// El directorio desconocido se crea **vacío**: el caso que importa es que la
/// carpeta en sí no corresponde a ningún motor conocido, no lo que haya
/// dentro.
///
/// `nombre` da nombre al directorio: cada caso usa el suyo, porque los tests
/// corren en paralelo y dos tests con el mismo directorio se pisan.
fn catalogo_con_motor_desconocido(nombre: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("ropero-cli-motor-desconocido-{nombre}"));
    let models = base.join("models");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(models.join("llama")).unwrap();
    std::fs::create_dir_all(models.join("motor-novedoso")).unwrap();
    // Un modelo válido, para que el escaneo tenga algo que validar.
    std::fs::copy(
        fixture_root().join("models/llama/modelo-alpha.toml"),
        models.join("llama/modelo-alpha.toml"),
    )
    .unwrap();
    base
}

/// Corre `ropero` contra el catálogo del motor desconocido.
fn run_con_motor_desconocido(nombre: &str, args: &[&str]) -> (i32, String, String) {
    let base = catalogo_con_motor_desconocido(nombre);
    let out = Command::new(env!("CARGO_BIN_EXE_ropero"))
        .args(args)
        .env("ROPERO_MODELS_DIR", &base)
        .env("GGUF_DIR", base.join("gguf"))
        .env("SAFETENSORS_DIR", base.join("safetensors"))
        .env("LOG_DIR", base.join("logs"))
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Un directorio de motor desconocido **no aborta el escaneo**: los modelos
/// válidos se siguen listando.
///
/// Antes, el `return Err` del descubrimiento cancelaba el `scan()` entero:
/// con un `motor-novedoso/` en `models/`, la v2 no listaba **ningún** modelo.
/// La v1 acumula el problema y sigue (`ropero-code/bin/python/modelconf.py:1648`).
#[test]
fn un_motor_desconocido_no_aborta_el_escaneo() {
    let (code, stdout, stderr) = run_con_motor_desconocido("list", &["list"]);
    // `list` no falla por un problema del catálogo (igual que con un TOML
    // roto): lo avisa y sigue.
    assert_eq!(code, 0, "stdout={} stderr={}", stdout, stderr);
    // El modelo válido se lista: el escaneo continuó.
    assert!(
        stdout.contains("modelo-alpha"),
        "el modelo válido no se lista: stdout={} stderr={}",
        stdout,
        stderr
    );
    // Y el problema se reporta (el aviso va a stderr, como el de un TOML roto).
    assert!(
        stderr.contains("motor desconocido"),
        "el motor desconocido no se reporta: stderr={}",
        stderr
    );
}

/// El motor desconocido es un **problema** del catálogo: `validate` lo
/// reporta y sale con 1, pero sigue validando el resto de los modelos.
#[test]
fn un_motor_desconocido_es_un_problema_del_catalogo() {
    let (code, stdout, stderr) = run_con_motor_desconocido("validate", &["validate"]);
    assert_eq!(code, 1, "stdout={} stderr={}", stdout, stderr);
    assert!(
        stderr.contains("motor desconocido"),
        "el motor desconocido no se reporta: stderr={}",
        stderr
    );
    // El escaneo llegó hasta el final: el catálogo se construyó y se
    // validó (y por eso hay resumen, no un aborto del `scan()`).
    assert!(
        stderr.contains("validación fallida"),
        "el escaneo se abortó antes de validar: stderr={}",
        stderr
    );
    // El modelo válido no dio error: sólo el directorio desconocido es el
    // problema.
    assert!(
        !stderr.contains("modelo-alpha"),
        "el modelo válido se validó con error: stderr={}",
        stderr
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Los errores de parseo llevan ruta:línea (doc B §4.1.1)
// ─────────────────────────────────────────────────────────────────────────────

/// Monta un catálogo con un TOML **inválido** y devuelve su raíz.
///
/// `nombre` da nombre al directorio: cada caso usa el suyo, para que dos
/// tests no se pisen el fichero.
///
/// `contenido` es el texto del TOML; se escribe en
/// `models/llama/modelo-roto.toml`.
fn catalogo_con_toml_roto(nombre: &str, contenido: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("ropero-cli-toml-roto-{nombre}"));
    let models = base.join("models/llama");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(models.join("modelo-roto.toml"), contenido).unwrap();
    base
}

/// Corre `ropero validate` contra el catálogo con el TOML inválido.
fn run_con_toml_roto(nombre: &str, contenido: &str) -> (i32, String, String) {
    let base = catalogo_con_toml_roto(nombre, contenido);
    let out = Command::new(env!("CARGO_BIN_EXE_ropero"))
        .args(["validate"])
        .env("ROPERO_MODELS_DIR", &base)
        .env("GGUF_DIR", base.join("gguf"))
        .env("SAFETENSORS_DIR", base.join("safetensors"))
        .env("LOG_DIR", base.join("logs"))
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// La ruta del TOML roto del caso `nombre`.
fn ruta_toml_roto(nombre: &str) -> String {
    std::env::temp_dir()
        .join(format!("ropero-cli-toml-roto-{nombre}"))
        .join("models/llama/modelo-roto.toml")
        .display()
        .to_string()
}

/// Una clave desconocida se reporta como **`ruta:línea`**, y la línea es la
/// del campo culpable.
///
/// Antes el error salía como `ruta: error de parseo…`, sin línea: el
/// usuario no sabía dónde mirar (doc B §4.1.1).
#[test]
fn una_clave_desconocida_se_reporta_con_ruta_y_linea() {
    let toml = "[model]\nname = \"modelo-roto\"\nengine = \"llama-server\"\n\n[files]\ngguf = \"g.gguf\"\nclave_que_no_existe = \"x\"\n";
    let (code, stdout, stderr) = run_con_toml_roto("clave", toml);
    assert_eq!(code, 1, "stdout={} stderr={}", stdout, stderr);
    // La clave culpable está en la línea 7.
    let esperado = format!("{}:7", ruta_toml_roto("clave"));
    assert!(
        stderr.contains(&esperado),
        "el error no lleva ruta:línea ({esperado}): stderr={}",
        stderr
    );
    // Y se dice qué clave es la culpable.
    assert!(
        stderr.contains("clave_que_no_existe"),
        "el error no nombra la clave: stderr={}",
        stderr
    );
}

/// Un motor desconocido se reporta como **`ruta:línea`**, y la línea es la de
/// la clave `engine`.
#[test]
fn un_motor_desconocido_se_reporta_con_ruta_y_linea() {
    let toml = "[model]\nname = \"modelo-roto\"\nengine = \"motor-inventado\"\n\n[files]\ngguf = \"g.gguf\"\n";
    let (code, stdout, stderr) = run_con_toml_roto("motor", toml);
    assert_eq!(code, 1, "stdout={} stderr={}", stdout, stderr);
    // La clave `engine` está en la línea 3.
    let esperado = format!("{}:3", ruta_toml_roto("motor"));
    assert!(
        stderr.contains(&esperado),
        "el error no lleva ruta:línea ({esperado}): stderr={}",
        stderr
    );
    assert!(
        stderr.contains("motor desconocido"),
        "el error no dice qué pasa: stderr={}",
        stderr
    );
}
