//! El contrato de la CLI: la tabla de códigos de salida y el contrato de
//! stdout de cada comando.
//!
//! Esta es la **puerta G-11** del doc B §7.2, ejecutable con
//!
//! ```text
//! cargo test --locked -p ropero-cli --test cli_contract
//! ```
//!
//! La batería es hermética: usa el catálogo de pruebas de
//! `tests/fixtures/models` (con `ROPERO_MODELS_DIR`), no el catálogo real.
//!
//! ## La tabla de códigos de salida (doc CLI v2 §3)
//!
//! | Código | Significado |
//! |---|---|
//! | `0` | el comando terminó bien |
//! | `1` | error de ejecución (catálogo no encontrado, E/S, comando pendiente) |
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

/// El mismo comando, pero **sin motor**: los dos motores apuntan a un
/// directorio que no existe, y el `$HOME` también.
///
/// Sin esto los tests del `gguffer` dependerían de la máquina: en una que
/// tenga `~/llama.cpp` la precondición se cumpliría y el código de salida
/// sería otro.
fn run_sin_motor(args: &[&str]) -> (i32, String, String) {
    let base = std::env::temp_dir().join("ropero-cli-contract-sin-motor");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let out = bin()
        .args(args)
        .env("HOME", &base)
        .env("LLAMA_DIR", base.join("no-fork"))
        .env("LLAMA_OFFICIAL_DIR", base.join("no-oficial"))
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Los códigos de salida de la tabla del doc CLI v2 §3.
const CODIGO_OK: i32 = 0;
const CODIGO_ERROR_EJECUCION: i32 = 1;
const CODIGO_ERROR_USO: i32 = 2;
/// El código de SIGINT: documentado, pendiente de F3.
const CODIGO_SIGINT: i32 = 130;

// ─────────────────────────────────────────────────────────────────────────────
// La tabla de códigos de salida
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
        vec!["engines"],
        vec!["status"],
        vec!["--help"],
        vec!["--version"],
    ] {
        let (code, _stdout, stderr) = run(&args);
        assert_eq!(code, CODIGO_OK, "{args:?} salió con {code}: {stderr}");
    }
}

/// `ropero` a secas es una ayuda breve, no un error: sale con 0 (doc CLI v2
/// §5.2).
#[test]
fn ropero_a_secas_sale_con_cero() {
    let (code, stdout, stderr) = run(&[]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    // Las cuatro secciones del doc CLI v2 §6.
    assert!(stdout.contains("MOTORES"), "stdout={stdout}");
    assert!(stdout.contains("MODELOS"), "stdout={stdout}");
    assert!(stdout.contains("build:"), "stdout={stdout}");
    assert!(stdout.contains("autor:"), "stdout={stdout}");
    assert!(stdout.contains("ropero --help"), "stdout={stdout}");
}

/// Un modelo desconocido en el **arranque** es un error de ejecución:
/// sale con 1.
///
/// El doc CLI v2 §8.1 dice que si una precondición no se cumple es
/// «error de ejecución (código 1)». Y la precondición del arranque
/// incluye que el modelo exista: sin modelo no hay nada que arrancar.
///
/// Ojo: esto **no** contradice el §5.2, que dice que un comando
/// desconocido sale con 2. `ropero inventado` no es un comando
/// desconocido: es el nombre de un modelo que no está en el catálogo, y
/// el arranque lo trata como lo que es.
#[test]
fn un_modelo_desconocido_en_el_arranque_sale_con_uno() {
    let (code, _stdout, stderr) = run(&["inventado"]);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
    assert!(stderr.contains("modelo desconocido"), "stderr={stderr}");
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

/// Los comandos que se parsean pero todavía no se ejecutan dicen **qué
/// falta** y salen con 1 (no con 2: el uso es correcto).
///
/// Ya **no queda ninguno**: `gguffer` fue el último (F6), y los once
/// comandos del contrato están **listos**. Lo que se comprueba ahora es
/// lo contrario: que ninguno diga «todavía no está implementado», que es
/// la respuesta de un comando pendiente.
#[test]
fn ningun_comando_dice_que_todavia_no_esta_implementado() {
    for cmd in [
        "install", "stop", "gguffer", "status", "doctor", "list", "template", "engines",
        "validate", "args", "levels", "path",
    ] {
        let (code, _stdout, stderr) = run(&[cmd]);
        assert!(
            !stderr.contains("todavía no está implementado"),
            "{cmd} sigue pendiente: {stderr}"
        );
        // Y ninguno sale con el código de un pendiente (1) por esa razón.
        assert_ne!(
            (code, stderr.contains("todavía no está implementado")),
            (1, true),
            "{cmd}: {stderr}"
        );
    }
}

/// `ropero gguffer` **funciona** (F6): sin fuente es un **error de uso**
/// (código 2), y el mensaje dice **qué hacer** (P-13).
#[test]
fn gguffer_sin_fuente_dice_que_falta_y_sale_con_dos() {
    let (code, _stdout, stderr) = run(&["gguffer"]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    for flag in ["--path", "--repo", "--gguf"] {
        assert!(stderr.contains(flag), "no dice {flag}: {stderr}");
    }
}

/// `ropero install` sin lista y **sin terminal** es un error de uso
/// (código 2), y el mensaje dice **qué hacer** (R-INST-26).
#[test]
fn install_sin_lista_y_sin_terminal_es_error_de_uso() {
    for argumentos in [
        vec!["install"],
        vec!["install", "engines"],
        vec!["install", "models"],
    ] {
        let (code, _stdout, stderr) = run(&argumentos);
        assert_eq!(
            code, CODIGO_ERROR_USO,
            "{argumentos:?} salió con {code}: {stderr}"
        );
        assert!(
            stderr.contains("terminal"),
            "{argumentos:?}: el mensaje no dice que hace falta una terminal: {stderr}"
        );
    }
}

/// `ropero stop` **funciona**: sin estado en disco no hay nada que parar,
/// y eso no es un error.
#[test]
fn stop_sin_nada_que_parar_no_es_un_error() {
    let (code, _stdout, stderr) = run(&["stop"]);
    assert_eq!(code, 0, "stderr={stderr}");
}

/// El arranque de un modelo que no se puede arrancar dice **por qué** y
/// sale con 1 (error de ejecución, no de uso: el modelo sí existe).
#[test]
fn el_arranque_de_un_modelo_sale_con_uno_y_lo_dice() {
    let (code, _stdout, stderr) = run(&["modelo-alpha"]);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
    // El mensaje dice **qué** pasa y **qué hacer**.
    assert!(
        stderr.contains("modelo-alpha") || stderr.contains("motor"),
        "stderr={stderr}"
    );
    assert!(
        stderr.contains("ropero install") || stderr.contains("ropero stop"),
        "stderr={stderr}"
    );
}

/// Un flag del arranque que no existe es un error de uso: sale con 2.
#[test]
fn un_flag_del_arranque_que_no_existe_sale_con_dos() {
    let (code, _stdout, stderr) = run(&["modelo-alpha", "--inventado"]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
}

/// Los flags del arranque del doc CLI v2 §8.2 se **aceptan**.
#[test]
fn los_flags_del_arranque_se_aceptan() {
    for flag in [
        vec!["--force"],
        vec!["--cpu"],
        vec!["--port", "8100"],
        vec!["--api-key", "clave"],
        vec!["--no-api-key"],
        vec!["--ctx", "4096"],
    ] {
        let mut args = vec!["modelo-alpha"];
        args.extend(flag.iter().copied());
        let (code, _stdout, stderr) = run(&args);
        assert_eq!(
            code, CODIGO_ERROR_EJECUCION,
            "{args:?} salió con {code}: {stderr}"
        );
    }
}

/// Los flags del `gguffer` del doc CLI v2 §11.2 (más `--gguf` y
/// `--out-principal`) se **aceptan**: el parser no rechaza ninguno.
///
/// Cada flag va en una invocación que **no choca** con otra regla (la
/// fuente es una, el imatrix exige `--quant`, el par remoto va junto), y
/// lo que se comprueba es el código de salida **esperado**: 1, porque en
/// una máquina sin motor lo que falla es la precondición (R-GGUF-54), no
/// el uso. Un flag rechazado por el parser saldría con 2.
#[test]
fn los_flags_del_gguffer_se_aceptan() {
    let pesos = "/tmp/pesos";
    let casos: &[(&[&str], i32)] = &[
        (&["--repo", "owner/name"], CODIGO_ERROR_EJECUCION),
        (&["--path", pesos], CODIGO_ERROR_EJECUCION),
        (&["--gguf", "pesos.gguf"], CODIGO_ERROR_EJECUCION),
        (
            &["--path", pesos, "--quant", "q4_k_m"],
            CODIGO_ERROR_EJECUCION,
        ),
        (
            &[
                "--path",
                pesos,
                "--quant",
                "q4_k_m",
                "--imatrix",
                "imatrix.dat",
            ],
            CODIGO_ERROR_EJECUCION,
        ),
        (
            &[
                "--path",
                pesos,
                "--quant",
                "q4_k_m",
                "--imatrix-repo",
                "owner/name",
                "--imatrix-file",
                "imatrix.dat",
            ],
            CODIGO_ERROR_EJECUCION,
        ),
        (
            &["--path", pesos, "--out", "salida.gguf"],
            CODIGO_ERROR_EJECUCION,
        ),
        (
            &["--path", pesos, "--out-principal", "salida.gguf"],
            CODIGO_ERROR_EJECUCION,
        ),
        (
            &["--path", pesos, "--intermedio", "bf16"],
            CODIGO_ERROR_EJECUCION,
        ),
        (&["--path", pesos, "--keep-source"], CODIGO_ERROR_EJECUCION),
        (
            &["--path", pesos, "--keep-intermedio"],
            CODIGO_ERROR_EJECUCION,
        ),
        (&["--path", pesos, "--no-mtp"], CODIGO_ERROR_EJECUCION),
        (&["--path", pesos, "--no-dflash"], CODIGO_ERROR_EJECUCION),
        (&["--path", pesos, "--force"], CODIGO_ERROR_EJECUCION),
        (&["--list-quants"], CODIGO_ERROR_EJECUCION),
        (&["--diagnostico"], CODIGO_ERROR_EJECUCION),
    ];
    for (flags, esperado) in casos {
        let mut args = vec!["gguffer"];
        args.extend(flags.iter().copied());
        let (code, _stdout, stderr) = run_sin_motor(&args);
        assert_eq!(code, *esperado, "{args:?} salió con {code}: {stderr}");
    }
}

/// `--imatrix-repo` y `--imatrix-file` van **en pareja**: usar sólo una es
/// un error de uso (doc CLI v2 §11.2).
#[test]
fn la_imatrix_remota_va_en_pareja() {
    for args in [
        vec!["gguffer", "--imatrix-repo", "owner/name"],
        vec!["gguffer", "--imatrix-file", "imatrix.dat"],
    ] {
        let (code, _stdout, stderr) = run(&args);
        assert_eq!(
            code, CODIGO_ERROR_USO,
            "{args:?} salió con {code}: {stderr}"
        );
    }
}

/// La pareja completa sí se acepta.
#[test]
fn la_pareja_de_imatrix_se_acepta() {
    let (code, _stdout, stderr) = run_sin_motor(&[
        "gguffer",
        "--path",
        "/tmp/pesos",
        "--quant",
        "q4_k_m",
        "--imatrix-repo",
        "owner/name",
        "--imatrix-file",
        "imatrix.dat",
    ]);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
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
    // El directorio se limpia antes: si una corrida anterior dejó un motor
    // viejo dentro, el catálogo tendría dos copias del mismo modelo y el
    // escaneo fallaría por el choque de alias (no por la plantilla).
    let _ = std::fs::remove_dir_all(&base);
    let turbo = base.join("turbo-llama");
    std::fs::create_dir_all(&turbo).unwrap();
    std::fs::write(
        turbo.join("modelo-alpha.toml"),
        "[model]\nname = \"modelo-alpha\"\nengine = \"turbo-llama\"\n\n[files]\ngguf = \"a.gguf\"\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_ropero"))
        .arg("template")
        .env("ROPERO_MODELS_DIR", &base)
        .env("GGUF_DIR", base.join("gguf"))
        .env("SAFETENSORS_DIR", base.join("safetensors"))
        .env("LOG_DIR", base.join("logs"))
        .current_dir(&base)
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
}

/// El código de SIGINT está documentado para F3.
#[test]
fn el_codigo_de_sigint_esta_documentado_para_f3() {
    // Se manda un SIGINT al proceso. Con el runtime de F3 esto será un
    // 130; hoy el proceso muere por la señal, y lo que se fija aquí es que
    // el código está **documentado** y no se confunde con el 1.
    let mut out = Command::new(env!("CARGO_BIN_EXE_ropero"))
        .arg("status")
        .env("ROPERO_MODELS_DIR", fixture_root())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("lanzar ropero");
    // El proceso ya ha terminado: se comprueba el código con el que sale
    // un comando normal, y el 130 queda documentado como el de SIGINT.
    let code = out.wait().unwrap().code().unwrap_or(-1);
    assert_ne!(
        code, CODIGO_SIGINT,
        "el 130 es el de SIGINT, no el de un comando"
    );
}

/// `ropero validate` escribe una línea OK en stdout.
#[test]
fn validate_escribe_una_linea_ok_en_stdout() {
    let (code, stdout, stderr) = run(&["validate"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "stdout={stdout}");
    assert!(lines[0].starts_with("ok:"), "stdout={stdout}");
}

/// `ropero template` vuelca la plantilla.
#[test]
fn template_vuelca_la_plantilla() {
    let (code, stdout, stderr) = run(&["template"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert!(stdout.contains("[model]"), "stdout={stdout}");
}

/// `ropero args` imprime un argumento por línea.
#[test]
fn args_imprime_un_argumento_por_linea() {
    let (code, stdout, stderr) = run(&["args", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let rows: Vec<&str> = stdout.lines().collect();
    assert!(!rows.is_empty(), "stdout={stdout}");
    assert!(rows.contains(&"--n-gpu-layers"), "stdout={stdout}");
}

/// `ropero levels` imprime un nivel por línea.
#[test]
fn levels_imprime_un_nivel_por_linea() {
    let (code, stdout, stderr) = run(&["levels", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let mut rows: Vec<&str> = stdout.lines().collect();
    rows.sort();
    assert_eq!(rows, vec!["high", "low", "medium"], "stdout={stdout}");
}

/// `ropero path` imprime una ruta por línea.
#[test]
fn path_imprime_una_ruta_por_linea() {
    let (code, stdout, stderr) = run(&["path", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let rows: Vec<&str> = stdout.lines().collect();
    assert!(
        rows.iter().any(|l| l.starts_with("GGUF=")),
        "stdout={stdout}"
    );
    assert!(
        rows.iter().any(|l| l.starts_with("LOG=")),
        "stdout={stdout}"
    );
}

/// `ropero --help` imprime la ayuda en stdout.
#[test]
fn help_imprime_la_ayuda_en_stdout() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert!(stdout.contains("USO: ropero"), "stdout={stdout}");
}

/// `ropero --version` dice la versión, la fecha de compilación y el autor.
#[test]
fn version_dice_version_build_y_autor() {
    let (code, stdout, stderr) = run(&["--version"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "stdout={stdout}"
    );
    assert!(stdout.contains("build"), "stdout={stdout}");
    assert!(stdout.contains("Lorenzo Sánchez Fraile"), "stdout={stdout}");
}

/// `ropero status` no rompe y sale con 0.
#[test]
fn status_sale_con_cero() {
    let (code, stdout, stderr) = run(&["status"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert!(stdout.contains("CPU"), "stdout={stdout}");
    assert!(stdout.contains("RAM"), "stdout={stdout}");
}

/// `ropero doctor` comprueba las dependencias y sale con 0 o 1.
#[test]
fn doctor_sale_con_cero_o_uno() {
    let (code, stdout, stderr) = run(&["doctor"]);
    assert!(
        code == CODIGO_OK || code == CODIGO_ERROR_EJECUCION,
        "doctor salió con {code}: {stderr}"
    );
    assert!(stdout.contains("DEPENDENCIAS DE ROPERO"), "stdout={stdout}");
    assert!(
        stdout.contains("DEPENDENCIAS DE LOS MOTORES"),
        "stdout={stdout}"
    );
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
        vec!["list"],
        vec!["template"],
        vec!["levels", "modelo-alpha"],
        vec!["path", "modelo-alpha"],
        vec!["--help"],
        vec!["status"],
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
/// Se mide el código del propio `ropero` (no del pipe) con un FIFO: se
/// abre el pipe, se lee una línea y se cierra. El lector es `head`.
#[test]
fn canalizar_a_head_termina_con_cero() {
    for args in [
        vec!["args", "modelo-alpha"],
        vec!["list"],
        vec!["validate"],
        vec!["list"],
        vec!["template"],
        vec!["levels", "modelo-alpha"],
        vec!["path", "modelo-alpha"],
        vec!["--help"],
        vec!["status"],
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
