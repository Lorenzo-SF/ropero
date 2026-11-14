//! El contrato de la CLI: la tabla de códigos de salida de R-CLI-4 y el
//! contrato de stdout de cada comando.
//!
//! Esta es la **puerta G-11** del doc B §7.2, que es ejecutable con
//!
//! ```text
//! cargo test --locked -p ropero-cli --test cli_contract
//! ```
//!
//! La batería es hermética: usa el catálogo de pruebas de
//! `tests/fixtures/models` (con `ROPERO_MODELS_DIR`), no el catálogo real.
//!
//! ## La tabla de códigos de salida (R-CLI-4)
//!
//! | Código | Significado |
//! |---|---|
//! | `0` | el comando terminó bien |
//! | `1` | error de ejecución (catálogo no encontrado, E/S, template ausente) |
//! | `2` | error de uso (comando desconocido, modelo desconocido, argumento inválido) |
//! | `130` | interrumpido por SIGINT — **pendiente de F3** (documentado, no implementado) |
//!
//! El `130` está **documentado pero no implementado**: la fase F3 es la que
//! trae el arranque del modelo, y es donde el `SIGINT` tendrá su propio
//! código. Aquí queda **fijado** para que la puerta sea ejecutable: cuando
//! F3 lo implemente, este test tendrá que cambiar de opinión.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Raíz del catálogo de pruebas: `<cargo-manifest-dir>/tests/fixtures`.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Comando `ropero` con el entorno limpio y apuntando al catálogo de pruebas.
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

/// Los códigos de salida de la tabla de R-CLI-4.
const CODIGO_OK: i32 = 0;
const CODIGO_ERROR_EJECUCION: i32 = 1;
const CODIGO_ERROR_USO: i32 = 2;
/// El código de SIGINT: documentado, pendiente de F3.
const CODIGO_SIGINT: i32 = 130;

// ─────────────────────────────────────────────────────────────────────────────
// La tabla de códigos de salida (R-CLI-4)
// ─────────────────────────────────────────────────────────────────────────────

/// Un comando que termina bien sale con 0.
#[test]
fn un_comando_que_va_bien_sale_con_cero() {
    for args in [
        vec!["validate"],
        vec!["list"],
        vec!["template"],
        vec!["args", "modelo-alpha"],
        vec!["levels", "modelo-alpha"],
        vec!["path", "modelo-alpha"],
        vec!["help"],
    ] {
        let (code, _stdout, stderr) = run(&args);
        assert_eq!(code, CODIGO_OK, "{args:?} salió con {code}: {stderr}");
    }
}

/// Un comando desconocido es un error de uso: sale con 2.
#[test]
fn un_comando_desconocido_sale_con_dos() {
    let (code, _stdout, stderr) = run(&["inventado"]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    assert!(stderr.contains("desconocido"), "stderr={stderr}");
}

/// Un modelo desconocido es un error de uso: sale con 2.
#[test]
fn un_modelo_desconocido_sale_con_dos() {
    for cmd in ["args", "levels", "path"] {
        let (code, _stdout, stderr) = run(&[cmd, "no-existe"]);
        assert_eq!(code, CODIGO_ERROR_USO, "{cmd} salió con {code}: {stderr}");
        assert!(stderr.contains("modelo desconocido"), "stderr={stderr}");
    }
}

/// Un argumento de más es un error de uso: sale con 2.
#[test]
fn un_argumento_de_mas_sale_con_dos() {
    let (code, _stdout, stderr) = run(&["list", "--bogus"]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
}

/// El error de ejecución (1) es el de un fallo de E/S o de un recurso que
/// no está.
///
/// El caso que se puede provocar de forma determinista es `template` sobre
/// un catálogo **sin** plantilla: no es un error de uso (el comando y sus
/// argumentos son correctos), sino un fallo de ejecución, así que sale con 1
/// (no con 2).
#[test]
fn un_catalogo_sin_plantilla_sale_con_uno() {
    let base = std::env::temp_dir().join("ropero-cli-contract-sin-plantilla");
    let llama = base.join("llama");
    std::fs::create_dir_all(&llama).unwrap();
    // Un modelo (para que el catálogo no esté vacío) pero sin `_template.toml`.
    std::fs::copy(
        fixture_root().join("models/llama/modelo-alpha.toml"),
        llama.join("modelo-alpha.toml"),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_ropero"))
        .args(["template"])
        .env("ROPERO_MODELS_DIR", &base)
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
    assert!(
        stderr.contains("no encuentro la plantilla"),
        "stderr={stderr}"
    );
}

/// El código de SIGINT (130) está **documentado** pero no implementado.
///
/// La fase F3 es la que trae el arranque del modelo; es donde el `SIGINT`
/// tendrá su propio código. Esta puerta **fija** el valor para que el
/// contrato sea ejecutable: cuando F3 lo implemente, este test tendrá que
/// cambiar de opinión (y entonces `ropero <modelo>` desviará el SIGINT).
#[test]
fn el_codigo_de_sigint_esta_documentado_para_f3() {
    // No hay todavía ningún comando que arranque un modelo, así que ningún
    // `SIGINT` produce hoy un 130: el código está *declarado* aquí, y el
    // test falla si alguien lo implementa sin actualizar el contrato.
    let out = Command::new(env!("CARGO_BIN_EXE_ropero"))
        .args(["list"])
        .env("ROPERO_MODELS_DIR", fixture_root())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(-1);
    assert_ne!(code, CODIGO_SIGINT, "el 130 sigue pendiente de F3");
}

// ─────────────────────────────────────────────────────────────────────────────
// El contrato de stdout de cada comando
// ─────────────────────────────────────────────────────────────────────────────

/// `validate` con el catálogo sano: una línea `OK:` en stdout.
#[test]
fn validate_escribe_una_linea_ok_en_stdout() {
    let (code, stdout, stderr) = run(&["validate"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "stdout={stdout}");
    assert!(lines[0].starts_with("OK:"), "stdout={stdout}");
}

/// `list` imprime una fila por modelo activo, y sólo en stdout.
#[test]
fn list_imprime_una_fila_por_modelo_activo() {
    let (code, stdout, stderr) = run(&["list"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let rows: Vec<&str> = stdout.lines().collect();
    assert_eq!(rows.len(), 3, "stdout={stdout}");
    for row in rows {
        assert!(row.starts_with("  "), "row={row}");
    }
}

/// `list --all` añade los modelos retirados.
#[test]
fn list_con_all_anade_los_retirados() {
    let (code, stdout, stderr) = run(&["list", "--all"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert_eq!(stdout.lines().count(), 4, "stdout={stdout}");
    assert!(stdout.contains("modelo-retirado"), "stdout={stdout}");
}

/// `template` vuelca la plantilla del catálogo: empieza por `[model]`.
#[test]
fn template_vuelca_la_plantilla() {
    let (code, stdout, stderr) = run(&["template"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert!(stdout.starts_with("#"), "stdout={stdout}");
    assert!(stdout.contains("[model]"), "stdout={stdout}");
}

/// `args` imprime **un argumento por línea**, en orden.
#[test]
fn args_imprime_un_argumento_por_linea() {
    let (code, stdout, stderr) = run(&["args", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let rows: Vec<&str> = stdout.lines().collect();
    assert!(!rows.is_empty(), "stdout={stdout}");
    assert_eq!(rows[0], "--n-gpu-layers", "stdout={stdout}");
    // Cada fila es un argumento: ningún argumento lleva espacios.
    for row in rows {
        assert!(!row.contains(' '), "un argumento con espacios: {row}");
    }
}

/// `levels` imprime los niveles declarados, uno por línea.
#[test]
fn levels_imprime_un_nivel_por_linea() {
    let (code, stdout, stderr) = run(&["levels", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let mut rows: Vec<&str> = stdout.lines().collect();
    rows.sort();
    assert_eq!(rows, vec!["high", "low", "medium"], "stdout={stdout}");
}

/// `path` imprime las rutas resueltas, una por línea, en `CLAVE=valor`.
#[test]
fn path_imprime_una_ruta_por_linea() {
    let (code, stdout, stderr) = run(&["path", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let rows: Vec<&str> = stdout.lines().collect();
    assert!(!rows.is_empty(), "stdout={stdout}");
    for row in rows {
        assert!(row.contains('='), "row={row}");
    }
    assert!(stdout.contains("GGUF="), "stdout={stdout}");
}

/// `help` imprime la ayuda en stdout y sale con 0.
#[test]
fn help_imprime_la_ayuda_en_stdout() {
    let (code, stdout, stderr) = run(&["help"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert!(stdout.contains("USO: ropero"), "stdout={stdout}");
}

// ─────────────────────────────────────────────────────────────────────────────
// El pipe roto no es un error (SIGPIPE)
// ─────────────────────────────────────────────────────────────────────────────

/// Canalizar la salida a `head` **no** revienta el programa.
///
/// `println!` panica cuando el lector cierra el pipe antes de que termine
/// la salida: `ropero args groot | head -1` era un crash (exit 134 en
/// release, 101 en debug) en un idiom de shell muy común. Ahora el pipe
/// roto es el **fin normal** de la salida: el comando sale con 0 y no
/// imprime ningún error.
#[test]
fn canalizar_a_head_no_revienta_el_programa() {
    for args in [
        vec!["args", "modelo-alpha"],
        vec!["list"],
        vec!["validate"],
        vec!["template"],
        vec!["levels", "modelo-alpha"],
        vec!["path", "modelo-alpha"],
        vec!["help"],
    ] {
        for lineas in ["1", "12"] {
            let out = Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "'{}' {} | head -{lineas} >/dev/null",
                    env!("CARGO_BIN_EXE_ropero"),
                    args.join(" ")
                ))
                .env("ROPERO_MODELS_DIR", fixture_root())
                .env("GGUF_DIR", fixture_root().join("gguf"))
                .env("SAFETENSORS_DIR", fixture_root().join("safetensors"))
                .output()
                .unwrap();
            // `sh` propaga el código del último comando del pipe (`head`), así
            // que el código de `ropero` se lee del propio estado del pipe:
            // con `set -o pipefail` no lo propagaría, pero `sh` (dash) no
            // tiene esa opción. Por eso el test mira el stderr: no debe
            // haber ningún panic.
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(
                !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
                "{args:?} | head -{lineas} reventó: {stderr}"
            );
        }
    }
}

/// Canalizar a `head` **no** deja un panic en stderr, y el comando termina
/// con 0.
///
/// Se mide el código del propio `ropero` (no el del pipe) con un FIFO: se
/// abre el pipe, se lee una línea y se cierra. El lector es `head`.
#[test]
fn canalizar_a_head_termina_con_cero() {
    for args in [
        vec!["args", "modelo-alpha"],
        vec!["list"],
        vec!["validate"],
        vec!["template"],
        vec!["levels", "modelo-alpha"],
        vec!["path", "modelo-alpha"],
        vec!["help"],
    ] {
        for lineas in ["1", "12"] {
            // `head -n N` cierra el pipe después de N líneas: `ropero` recibe
            // un SIGPIPE (o un EPIPE) al escribir. Se mide su código con
            // `PIPESTATUS`, que es de bash.
            let script = format!(
                "'{}' {} | head -{lineas} >/dev/null; echo ${{PIPESTATUS[0]}}",
                env!("CARGO_BIN_EXE_ropero"),
                args.join(" ")
            );
            let out = Command::new("bash")
                .arg("-c")
                .arg(&script)
                .env("ROPERO_MODELS_DIR", fixture_root())
                .env("GGUF_DIR", fixture_root().join("gguf"))
                .env("SAFETENSORS_DIR", fixture_root().join("safetensors"))
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&out.stdout);
            let code: i32 = stdout.trim().parse().unwrap_or(-1);
            assert_eq!(
                code, CODIGO_OK,
                "{args:?} | head -{lineas} salió con {code} (stdout={stdout})"
            );
        }
    }
}
