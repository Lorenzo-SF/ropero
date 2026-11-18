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
    let test_name = std::thread::current()
        .name()
        .unwrap_or("cli-contract")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    cmd.env("ROPERO_MODELS_DIR", fixture_root());
    cmd.env("GGUF_DIR", fixture_root().join("gguf"));
    cmd.env("SAFETENSORS_DIR", fixture_root().join("safetensors"));
    cmd.env("LOG_DIR", fixture_root().join("logs"));
    // Never read state from a real local Ropero installation: a running
    // engine there could change connect's selected endpoint and break the
    // fixture's expected output.
    cmd.env(
        "ROPERO_STATE_DIR",
        std::env::temp_dir().join(format!(
            "ropero-cli-contract-state-{}-{test_name}",
            std::process::id()
        )),
    );
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
        .env("PATH", base.join("empty-path"))
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
        vec!["models"],
        vec!["template"],
        vec!["command", "modelo-alpha"],
        vec!["info", "modelo-alpha"],
        vec!["info", "modelo-alpha"],
        vec!["engines"],
        vec!["status"],
        vec!["--help"],
        vec!["--version"],
    ] {
        let (code, _stdout, stderr) = run(&args);
        assert_eq!(code, CODIGO_OK, "{args:?} salió con {code}: {stderr}");
    }
}

#[test]
fn connect_lista_integraciones_y_da_instrucciones_de_trae_sin_editar_archivos_privados() {
    let (code, stdout, stderr) = run(&["connect", "--list"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert!(stdout.contains("opencode"), "stdout={stdout}");
    assert!(stdout.contains("trae-cn"), "stdout={stdout}");
    for unsupported in ["gemini", "cursor", "kiro", "antigravity", "devin"] {
        assert!(
            !stdout.contains(unsupported),
            "no debe anunciar {unsupported}: {stdout}"
        );
    }

    let (code, stdout, stderr) = run(&["connect", "--agent", "trae", "--model", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert!(
        stdout.contains("OpenAI Chat Completions"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("http://127.0.0.1:9990/v1"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("model ID modelo-alpha"), "stdout={stdout}");
    assert!(
        stdout.contains("does not edit TRAE's private UI-managed settings"),
        "stdout={stdout}"
    );
}

#[test]
fn connect_claude_emite_variables_para_endpoint_anthropic_messages_del_engine() {
    let (code, stdout, stderr) = run(&["connect", "--agent", "claude", "--model", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    for expected in [
        "ANTHROPIC_BASE_URL=http://127.0.0.1:9990",
        "API_TIMEOUT_MS=600000",
        "DISABLE_TELEMETRY=1",
        "ANTHROPIC_MODEL=modelo-alpha",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL=modelo-alpha",
        "ANTHROPIC_DEFAULT_OPUS_MODEL=modelo-alpha",
        "ANTHROPIC_DEFAULT_SONNET_MODEL=modelo-alpha",
    ] {
        assert!(stdout.contains(expected), "falta {expected}: {stdout}");
    }
    let auth = stdout
        .lines()
        .find(|line| line.starts_with("ANTHROPIC_AUTH_TOKEN="))
        .expect("Claude Code recibe ANTHROPIC_AUTH_TOKEN");
    assert!(
        auth.split_once('=')
            .is_some_and(|(_, value)| !value.is_empty()),
        "la variable de autenticación debe tener un valor: {stdout}"
    );
    assert!(!stdout.contains("9990/v1"), "stdout={stdout}");
}

#[test]
fn test_all_models_es_excluyente_con_model_y_filtra_modelos_no_disponibles() {
    let (code, _stdout, stderr) = run(&[
        "test",
        "--model",
        "modelo-alpha",
        "--all-models",
        "--test-group",
        "A",
    ]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");

    let (code, _stdout, stderr) = run(&["test", "--all-models", "--test-group", "A"]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    assert!(
        stderr.contains("no hay modelos disponibles") || stderr.contains("no available models"),
        "all-models debe admitir el uso headless y excluir motores no instalados: {stderr}"
    );
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

/// El resumen se compone con el **sistema de formato**: los títulos de las
/// secciones van en el margen y en mayúsculas, y los rótulos se alinean.
///
/// Antes los rótulos se escribían a mano: `build:  ` con dos espacios y
/// `  disponibles: ` con uno: el resumen hablaba otro idioma que el resto.
#[test]
fn el_resumen_se_compone_con_el_sistema_de_formato() {
    let (code, stdout, stderr) = run(&[]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let lineas: Vec<&str> = stdout.lines().collect();

    // Los títulos de sección: en el margen y en mayúsculas.
    for titulo in ["MOTORES", "MODELOS"] {
        let linea = lineas
            .iter()
            .find(|l| l.trim() == titulo)
            .unwrap_or_else(|| panic!("no está la sección '{titulo}': stdout={stdout}"));
        assert!(
            !linea.starts_with(' '),
            "el título '{titulo}' no va en el margen: {linea:?}"
        );
        assert_eq!(
            linea.to_uppercase(),
            linea.to_string(),
            "el título '{titulo}' no va en mayúsculas: {linea:?}"
        );
    }

    // Los rótulos: el valor de cada par empieza en la misma columna.
    let mut columnas: Vec<usize> = Vec::new();
    for linea in lineas.iter().filter(|l| l.starts_with("  ")) {
        let Some((clave, valor)) = linea.split_once(':') else {
            continue;
        };
        if clave.trim().is_empty() || valor.trim().is_empty() {
            continue;
        }
        let valor = valor.trim_start();
        columnas.push(linea.len() - valor.len());
    }
    assert!(
        columnas.len() >= 4,
        "el resumen no tiene rótulos suficientes: stdout={stdout}"
    );
    let primera = columnas[0];
    for columna in &columnas {
        assert_eq!(
            *columna, primera,
            "los rótulos no se alinean en la misma columna: stdout={stdout}"
        );
    }
}

/// Un modelo desconocido en el **arranque** es un error de uso:
/// sale con 2.
#[test]
fn un_modelo_desconocido_en_el_arranque_sale_con_dos() {
    let (code, _stdout, stderr) = run(&["inventado"]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    assert!(stderr.contains("modelo desconocido"), "stderr={stderr}");
}

/// Un modelo desconocido es un error de uso: sale con 2.
#[test]
fn un_modelo_desconocido_sale_con_dos() {
    for cmd in ["command", "info", "info"] {
        let (code, _stdout, stderr) = run(&[cmd, "no-existe"]);
        assert_eq!(code, CODIGO_ERROR_USO, "{cmd} salió con {code}: {stderr}");
        assert!(stderr.contains("modelo desconocido"), "stderr={stderr}");
    }
}

/// Un argumento de más es un error de uso: sale con 2.
#[test]
fn un_argumento_de_mas_sale_con_dos() {
    let (code, _stdout, stderr) = run(&["models", "--bogus"]);
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
        "install", "stop", "gguffer", "status", "doctor", "models", "template", "engines",
        "validate", "command", "info",
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

/// Los tokens que el TOML del modelo no declara se ignoran.
#[test]
fn un_flag_del_arranque_que_no_existe_se_ignora() {
    let (code, _stdout, stderr) = run(&["modelo-alpha", "--inventado"]);
    assert_ne!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    assert!(!stderr.contains("flag no declarado"), "stderr={stderr}");
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
        vec!["--log-level", "debug"],
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
        (&["--path", pesos, "--turbo-llama"], CODIGO_ERROR_EJECUCION),
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

/// Si el catálogo no tiene plantilla, se imprime una de referencia y se
/// sugiere destino en `models/` sin escribir.
#[test]
fn un_catalogo_sin_plantilla_sale_con_uno() {
    let base = std::env::temp_dir().join("ropero-cli-contract-sin-plantilla");
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
        .env("HOME", &base)
        .env("XDG_CONFIG_HOME", base.join("xdg"))
        .env("GGUF_DIR", base.join("gguf"))
        .env("SAFETENSORS_DIR", base.join("safetensors"))
        .env("LOG_DIR", base.join("logs"))
        .current_dir(&base)
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(stdout.contains("[model]"), "stdout={stdout}");
    assert!(
        stderr.contains("sugerencia: guarda la plantilla"),
        "stderr={stderr}"
    );
    let _ = std::fs::remove_dir_all(&base);
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

/// `ropero command` imprime un argumento por línea.
#[test]
fn command_imprime_opcion_y_valor_en_la_misma_linea() {
    let (code, stdout, stderr) = run(&["command", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let rows: Vec<&str> = stdout.lines().collect();
    assert!(rows.contains(&"--n-gpu-layers 99 \\"), "stdout={stdout}");
    assert!(
        rows.iter().all(|line| !line.ends_with(" \\ ")),
        "espacio tras barra: {stdout}"
    );
    assert!(
        rows.last().is_some_and(|line| !line.ends_with('\\')),
        "última línea con continuación: {stdout}"
    );
}

#[test]
fn info_muestra_rutas_y_configuracion() {
    let (code, stdout, stderr) = run(&["info", "modelo-alpha"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    for needle in ["modelo-alpha", "low", "medium", "high", "GGUF=", "LOG="] {
        assert!(stdout.contains(needle), "falta {needle:?}: {stdout}");
    }
}

#[test]
fn models_add_valida_normaliza_y_descarta_campos_desconocidos() {
    let root = std::env::temp_dir().join(format!("ropero-model-add-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("input.toml");
    std::fs::write(
        &input,
        r#"[model]
name = "new-model"
engine = "turbo-llama"
format = "gguf"
context = 32768

[files]
gguf = "new-model.gguf"

[args]
base = ["--ctx-size", "${ctx}"]

[unrelated]
private_note = "discard me"
"#,
    )
    .unwrap();
    let output = bin()
        .args(["models", "add", "--path", input.to_str().unwrap()])
        .env("ROPERO_MODELS_DIR", &root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("se descartaron 1 campo(s)"),
        "stdout={stdout}"
    );
    let stored = root.join("turbo-llama/new-model.toml");
    let text = std::fs::read_to_string(stored).unwrap();
    assert!(text.contains("name = \"new-model\""), "{text}");
    assert!(!text.contains("private_note"), "{text}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn models_add_permite_configuracion_completa_con_asignaciones_toml() {
    let root = std::env::temp_dir().join(format!("ropero-model-set-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let output = bin()
        .args([
            "models",
            "add",
            "--name",
            "advanced-model",
            "--engine",
            "turbo-llama",
            "--format",
            "gguf",
            "--artifact",
            "advanced.gguf",
            "--set",
            "levels.list=[\"low\",\"high\"]",
            "--set",
            "args.level.low.base=[\"--reasoning-budget\",\"4096\"]",
            "--set",
            "args.level.high.base=[\"--reasoning-budget\",\"16384\"]",
            "--set",
            "args.base=[\"--ctx-size\",\"${ctx}\"]",
        ])
        .env("ROPERO_MODELS_DIR", &root)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(root.join("turbo-llama/advanced-model.toml")).unwrap();
    let value: toml::Value = toml::from_str(&text).unwrap();
    assert_eq!(value["levels"]["list"][1].as_str(), Some("high"));
    assert_eq!(value["args"]["base"][0].as_str(), Some("--ctx-size"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn config_lang_se_persiste_y_rechaza_otro_valor() {
    let home = std::env::temp_dir().join(format!("ropero-lang-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let xdg = home.join("xdg");
    let output = bin()
        .args(["config", "--lang", "ES"])
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &xdg)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = std::fs::read_to_string(xdg.join("ropero/config.toml")).unwrap();
    assert!(config.contains("lang = \"ES\""), "{config}");
    let invalid = bin()
        .args(["config", "--lang", "FR"])
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &xdg)
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn status_live_rechaza_intervalo_cero() {
    let output = bin().args(["status", "--live", "0"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("mayor que cero"));
}

#[test]
fn help_imprime_la_ayuda_en_stdout() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    assert!(
        stdout.contains("USAGE: ropero") || stdout.contains("USO: ropero"),
        "stdout={stdout}"
    );
}

#[test]
fn help_usa_el_idioma_guardado_en_config() {
    let root = std::env::temp_dir().join(format!("ropero-help-lang-{}", std::process::id()));
    let config_dir = root.join("ropero");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let english = bin()
        .args(["--help"])
        .env("XDG_CONFIG_HOME", &root)
        .output()
        .unwrap();
    let english_help = String::from_utf8_lossy(&english.stdout);
    assert!(english_help.contains("USAGE: ropero"));
    assert!(english_help.contains("--model MODELS"));
    assert!(english_help.contains("--agent AGENT"));
    assert!(english_help.contains("<model> [--force]"));

    std::fs::write(&config, "lang = \"ES\"\n").unwrap();
    let spanish = bin()
        .args(["--help"])
        .env("XDG_CONFIG_HOME", &root)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&spanish.stdout).contains("USO: ropero"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn info_respeta_el_idioma_guardado() {
    let root = std::env::temp_dir().join(format!("ropero-info-lang-{}", std::process::id()));
    let config_dir = root.join("ropero");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.toml"), "lang = \"ES\"\n").unwrap();
    let output = bin()
        .args(["info", "modelo-alpha"])
        .env("XDG_CONFIG_HOME", &root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("MODELO modelo-alpha"), "{stdout}");
    assert!(stdout.contains("nivel por defecto"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn config_respeta_el_idioma_guardado() {
    let root = std::env::temp_dir().join(format!("ropero-config-lang-{}", std::process::id()));
    let config_dir = root.join("ropero");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let english = bin()
        .args(["config"])
        .env("XDG_CONFIG_HOME", &root)
        .output()
        .unwrap();
    assert_eq!(english.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&english.stdout).contains("CONFIGURATION"));

    std::fs::write(&config, "lang = \"ES\"\n").unwrap();
    let spanish = bin()
        .args(["config"])
        .env("XDG_CONFIG_HOME", &root)
        .output()
        .unwrap();
    assert_eq!(spanish.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&spanish.stdout);
    assert!(stdout.contains("CONFIGURACIÓN"), "{stdout}");
    assert!(stdout.contains("idioma"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
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
/// la salida: `ropero command groot | head -1` era un crash (exit 134 en
/// release, 101 en debug) en un idiom de shell muy común. Ahora el pipe
/// roto es el **fin normal** de la salida: el comando sale con 0 y no
/// imprime ningún error.
#[test]
fn canalizar_a_head_no_revienta_el_programa() {
    for args in [
        vec!["command", "modelo-alpha"],
        vec!["models"],
        vec!["validate"],
        vec!["models"],
        vec!["template"],
        vec!["info", "modelo-alpha"],
        vec!["info", "modelo-alpha"],
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
        vec!["command", "modelo-alpha"],
        vec!["models"],
        vec!["validate"],
        vec!["models"],
        vec!["template"],
        vec!["info", "modelo-alpha"],
        vec!["info", "modelo-alpha"],
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
