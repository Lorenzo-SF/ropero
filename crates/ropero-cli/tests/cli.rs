//! Tests de integración de la CLI contra un catálogo de pruebas.
//!
//! La batería es hermética: no depende del catálogo real, sino del de
//! `tests/fixtures/models`, que se apunta con la variable de entorno
//! `ROPERO_MODELS_DIR` (honorada por `ropero_cli::discovery`).
//!
//! El catálogo de pruebas contiene, exactamente:
//! - 2 modelos activos: `modelo-alpha` (motor `turbo-llama`) y `modelo-beta`
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

/// `ropero validate` valida el catálogo y sale con 0.
#[test]
fn validate_ok() {
    let (code, stdout, stderr) = run(&["validate"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(stdout.contains("ok:"), "stdout={}", stdout);
    // 4 ficheros TOML del fixture: 3 activos + 1 retirado.
    assert!(stdout.contains("4 modelo(s)"), "stdout={}", stdout);
}

/// `ropero list` agrupa por motor: sale un grupo por motor del catálogo.
///
/// En la máquina de pruebas los motores del fixture **no** están
/// instalados (no hay sus binarios en el `PATH`), así que sus grupos salen
/// **plegados**, con el motivo. Eso es lo que el doc pide: si el motor no
/// está instalado, su grupo no se despliega.
///
/// `ropero template` vuelca la plantilla.
#[test]
fn template_outputs() {
    let (code, stdout, stderr) = run(&["template"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(stdout.contains("[model]"), "stdout={}", stdout);
    assert!(stdout.contains("[args]"), "stdout={}", stdout);
}

/// `ropero levels <modelo>`: un nivel por línea.
#[test]
fn levels_lines() {
    let (code, stdout, stderr) = run(&["levels", "modelo-alpha"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let mut rows: Vec<&str> = stdout.lines().collect();
    rows.sort();
    assert_eq!(rows, vec!["high", "low", "medium"], "stdout={}", stdout);
    assert!(!stdout.contains("nivel"));
}

/// Un modelo sin niveles sale con la salida vacía y código 0.
#[test]
fn levels_of_model_without_levels_is_empty() {
    let (code, stdout, stderr) = run(&["levels", "modelo-beta"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert_eq!(stdout.lines().count(), 0, "stdout={}", stdout);
}

/// `ropero path <modelo>`: una ruta por línea.
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

/// Un modelo desconocido es un error de uso (código 2).
#[test]
fn unknown_model_is_usage_error() {
    let (code, _stdout, stderr) = run(&["args", "no-existe"]);
    assert_eq!(code, 2, "stderr={}", stderr);
    assert!(stderr.contains("modelo desconocido"), "stderr={}", stderr);
}

/// Un flag que el comando no conoce es un error de uso (código 2).
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

/// `replace` en el argv.
#[test]
fn replace_sustituye_la_pareja_del_base() {
    let (code, stdout, stderr) = run(&["args", "modelo-alpha", "--temp=0.9"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let rows: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        rows.iter().filter(|r| **r == "--temp").count(),
        1,
        "stdout={}",
        stdout
    );
    assert!(rows.contains(&"0.9"), "stdout={}", stdout);
    assert!(!rows.contains(&"0.7"), "stdout={}", stdout);
}

/// Sin pedir la opción, el base no se toca.
#[test]
fn replace_no_toca_el_base_si_no_se_pide_la_opcion() {
    let (code, stdout, stderr) = run(&["args", "modelo-alpha"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let rows: Vec<&str> = stdout.lines().collect();
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

/// El aviso de canonicalización no es un error: `validate` sigue en verde.
#[test]
fn el_aviso_de_canonicalizacion_no_es_error() {
    let (code, stdout, stderr) = run(&["validate"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(stdout.contains("ok:"), "stdout={}", stdout);
    assert!(
        stderr.contains("AVISO: el motor 'mlx_lm'"),
        "stderr={}",
        stderr
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// La ayuda
// ─────────────────────────────────────────────────────────────────────────────

/// La ayuda anuncia los once comandos del contrato, uno por línea.
#[test]
fn la_ayuda_anuncia_los_once_comandos() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    for comando in [
        "install", "stop", "gguffer", "status", "doctor", "list", "template", "engines",
        "validate", "args", "levels", "path",
    ] {
        assert!(
            stdout.contains(comando),
            "la ayuda no anuncia '{comando}': stdout={}",
            stdout
        );
    }
}

/// La ayuda está en español y sugiere el uso de cada comando.
#[test]
fn la_ayuda_esta_en_espanol_y_sugiere_el_uso() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(stdout.contains("USO:"), "stdout={}", stdout);
    assert!(stdout.contains("COMANDOS:"), "stdout={}", stdout);
    assert!(
        stdout.contains("ropero <comando> --help"),
        "la ayuda no sugiere el uso de cada comando: stdout={}",
        stdout
    );
    // Nada de cabeceras en inglés.
    assert!(!stdout.contains("Usage:"), "stdout={}", stdout);
    assert!(!stdout.contains("Commands:"), "stdout={}", stdout);
    assert!(!stdout.contains("Options:"), "stdout={}", stdout);
}

/// Las descripciones de la ayuda van todas en la **misma columna**.
///
/// Antes, los comandos se describían en una columna y las opciones en
/// otra: la ayuda se veía rota.
#[test]
fn la_ayuda_alinea_las_descripciones_en_una_columna() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let mut columnas: Vec<usize> = Vec::new();
    for linea in stdout.lines() {
        let Some((clave, valor)) = linea.split_once(": ") else {
            continue;
        };
        if clave.is_empty() || valor.is_empty() {
            continue;
        }
        // Las entradas de la ayuda van un nivel dentro.
        if !linea.starts_with("  ") || linea.starts_with("   ") {
            continue;
        }
        columnas.push(linea.len() - valor.len());
    }
    assert!(
        columnas.len() >= 10,
        "la ayuda no tiene entradas suficientes: stdout={}",
        stdout
    );
    let primera = columnas[0];
    for columna in &columnas {
        assert_eq!(
            *columna, primera,
            "las descripciones no se alinean en la misma columna: stdout={}",
            stdout
        );
    }
}

/// La pista de la ayuda va **al final** y un nivel dentro, no en el margen.
#[test]
fn la_ayuda_deja_la_pista_al_final_y_no_en_el_margen() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    let lineas: Vec<&str> = stdout.lines().collect();
    let (indice, pista) = lineas
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("ropero <comando> --help"))
        .unwrap_or_else(|| panic!("la ayuda no sugiere el uso: stdout={stdout}"));
    assert!(
        pista.starts_with("  "),
        "la pista no va un nivel dentro: {pista:?}"
    );
    assert_eq!(
        indice,
        lineas.len() - 1,
        "la pista no está al final: stdout={stdout}"
    );
}

/// El error de uso de `clap` sale **en español** y dice qué hacer.
///
/// `clap` habla inglés: su «Usage:» y su «For more information» dejaban la
/// salida del CLI en dos idiomas.
#[test]
fn los_errores_de_uso_de_clap_estan_en_espanol() {
    for argumentos in [vec!["list", "--bogus"], vec!["install", "--bogus"]] {
        let (code, _stdout, stderr) = run(&argumentos);
        assert_eq!(code, 2, "{argumentos:?} salió con {code}: {stderr}");
        assert!(
            stderr.starts_with("error: "),
            "{argumentos:?}: el error no lleva el prefijo: {stderr}"
        );
        assert!(
            stderr.contains("--bogus"),
            "{argumentos:?}: el error no dice qué falló: {stderr}"
        );
        assert!(
            stderr.contains("--help"),
            "{argumentos:?}: el error no dice qué hacer: {stderr}"
        );
        // Nada de inglés.
        for ingles in [
            "Usage:",
            "unexpected argument",
            "For more information",
            "tip:",
        ] {
            assert!(
                !stderr.contains(ingles),
                "{argumentos:?}: el error habla inglés ('{ingles}'): {stderr}"
            );
        }
    }
}

/// El modelo desconocido dice **lo mismo** venga de donde venga.
///
/// El texto es el del dominio: el runtime y la CLI lo toman de aquí.
#[test]
fn el_modelo_desconocido_dice_el_texto_del_dominio() {
    let (code, _stdout, stderr) = run(&["args", "no-existe"]);
    assert_eq!(code, 2, "stderr={}", stderr);
    let esperado = ropero_core::surface::unknown_model_message("no-existe");
    assert!(
        stderr.contains(&esperado),
        "el mensaje no es el del dominio:\n  esperado: {esperado}\n  stderr: {stderr}"
    );
}

/// Cada comando tiene su propio `--help`, en español.
#[test]
fn cada_comando_tiene_su_ayuda() {
    for comando in [
        "install", "stop", "gguffer", "status", "doctor", "list", "template", "engines",
        "validate", "args", "levels", "path",
    ] {
        let (code, stdout, stderr) = run(&[comando, "--help"]);
        assert_eq!(code, 0, "{comando} --help salió con {code}: {stderr}");
        assert!(
            stdout.contains("USO:"),
            "la ayuda de '{comando}' no está en español: stdout={}",
            stdout
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Un motor desconocido no aborta el escaneo (R-DATA-12)
// ─────────────────────────────────────────────────────────────────────────────

/// Monta un catálogo en el disco con un directorio de motor **desconocido**
/// (`motor-novedoso/`) junto a los modelos válidos del fixture, y devuelve
/// su raíz.
fn catalogo_con_motor_desconocido(nombre: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("ropero-cli-motor-desconocido-{nombre}"));
    let models = base.join("models");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(models.join("turbo-llama")).unwrap();
    std::fs::create_dir_all(models.join("motor-novedoso")).unwrap();
    std::fs::copy(
        fixture_root().join("models/turbo-llama/modelo-alpha.toml"),
        models.join("turbo-llama/modelo-alpha.toml"),
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

/// Un directorio de motor desconocido **no aborta el escaneo**.
#[test]
fn un_motor_desconocido_no_aborta_el_escaneo() {
    let (code, stdout, stderr) = run_con_motor_desconocido("list", &["validate"]);
    assert_eq!(code, 1, "stdout={} stderr={}", stdout, stderr);
    assert!(
        stderr.contains("motor desconocido"),
        "el motor desconocido no se reporta: stderr={}",
        stderr
    );
    // Y el escaneo llegó al final: el catálogo se construyó y se validó.
    assert!(
        stderr.contains("validación fallida"),
        "el escaneo se abortó antes de validar: stderr={}",
        stderr
    );
}

/// El motor desconocido es un **problema** del catálogo: `validate` lo
/// reporta y sale con 1.
#[test]
fn un_motor_desconocido_es_un_problema_del_catalogo() {
    let (code, stdout, stderr) = run_con_motor_desconocido("validate", &["validate"]);
    assert_eq!(code, 1, "stdout={} stderr={}", stdout, stderr);
    assert!(
        stderr.contains("motor desconocido"),
        "el motor desconocido no se reporta: stderr={}",
        stderr
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Los errores de parseo llevan ruta:línea (doc B §4.1.1)
// ─────────────────────────────────────────────────────────────────────────────

/// Monta un catálogo con un TOML **inválido** y devuelve su raíz.
fn catalogo_con_toml_roto(nombre: &str, contenido: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("ropero-cli-toml-roto-{nombre}"));
    let models = base.join("models/turbo-llama");
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
        .join("models/turbo-llama/modelo-roto.toml")
        .display()
        .to_string()
}

/// Una clave desconocida se reporta como **`ruta:línea`**.
#[test]
fn una_clave_desconocida_se_reporta_con_ruta_y_linea() {
    let toml = "[model]\nname = \"modelo-roto\"\nengine = \"llama-server\"\n\n[files]\ngguf = \"g.gguf\"\nclave_que_no_existe = \"x\"\n";
    let (code, stdout, stderr) = run_con_toml_roto("clave", toml);
    assert_eq!(code, 1, "stdout={} stderr={}", stdout, stderr);
    let esperado = format!("{}:7", ruta_toml_roto("clave"));
    assert!(
        stderr.contains(&esperado),
        "el error no lleva ruta:línea ({esperado}): stderr={}",
        stderr
    );
    assert!(
        stderr.contains("clave_que_no_existe"),
        "el error no nombra la clave: stderr={}",
        stderr
    );
}

/// Un motor desconocido se reporta como **`ruta:línea`**.
#[test]
fn un_motor_desconocido_se_reporta_con_ruta_y_linea() {
    let toml = "[model]\nname = \"modelo-roto\"\nengine = \"motor-inventado\"\n\n[files]\ngguf = \"g.gguf\"\n";
    let (code, stdout, stderr) = run_con_toml_roto("motor", toml);
    assert_eq!(code, 1, "stdout={} stderr={}", stdout, stderr);
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

/// Un motor escrito con un alias legacy se canonicaliza **con aviso**.
#[test]
fn el_alias_legacy_de_motor_se_canonicaliza_con_aviso() {
    let (code, _stdout, stderr) = run(&["validate"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(
        stderr.contains("AVISO: el motor 'mlx_lm' es un alias antiguo; se usa 'mlx-serve'"),
        "stderr={}",
        stderr
    );
}
