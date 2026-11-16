//! Tests del montaje del argv: `replace`, alias de nivel y `requires`.
//!
//! La puerta de T-08 es "Golden tests contra el argv real de los TOML"; esos
//! viven en `tests/golden/argv.rs`. Aquí van los casos **sintéticos**, que
//! cubren las claves del TOML que el catálogo real de la v1 no ejercita
//! (`replace` no aparece en ningún TOML de `ropero-code/models/`).

use std::collections::BTreeMap;
use std::path::Path;

use ropero_core::{
    ArgvError, LaunchRequest, ModelSpec, ResolveContext, ResolvedFiles, build_argv, parse_model,
    parse_request,
};

/// Contexto de resolución mínimo y hermético.
fn ctx() -> ResolveContext {
    ResolveContext {
        context: 4096,
        files: ResolvedFiles {
            gguf: Some("/g/modelo/modelo.gguf".to_string()),
            mtp: None,
            mmproj: None,
            chat_template: None,
            gguf_dir: "/g".to_string(),
            artifacts_dir: "/a".to_string(),
            safetensors_dir: "/s".to_string(),
            model_path: String::new(),
        },
        env: BTreeMap::new(),
    }
}

/// Parsea un TOML de prueba.
fn spec(toml: &str) -> ModelSpec {
    parse_model(toml, Path::new("models/llama/modelo.toml")).expect("parsear el modelo")
}

/// Petición con un nivel, flags y opciones.
fn request(level: Option<&str>, flags: &[&str], options: &[(&str, &str)]) -> LaunchRequest {
    LaunchRequest {
        level: level.map(|l| l.to_string()),
        flags: flags.iter().map(|f| f.to_string()).collect(),
        options: options
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// `replace`
// ─────────────────────────────────────────────────────────────────────────────

/// TOML con una opción que declara `replace` sobre un flag del base.
const TOML_REPLACE: &str = r#"
[model]
name = "modelo"
engine = "llama-server"
context = 4096

[files]
gguf = "modelo.gguf"

[options]
list = ["temp"]

[args]
base = ["--ctx-size", "${ctx}", "--temp", "0.7", "--n-gpu-layers", "99"]

[args.option.temp]
base = ["--temp", "${value}"]
replace = "--temp"
"#;

/// `replace` sustituye la pareja del base: el argv NO lleva `--temp 0.7`
/// duplicado, sino una sola pareja `--temp 0.9`.
///
/// Doc B §3.2.3: *"[args.option.temp] base = ["--temp", "${value}"] con
/// `replace = "--temp"` elimina la pareja `--temp 0.7` del base y pone
/// `--temp <valor>` en su lugar."*
#[test]
fn replace_sustituye_la_pareja_del_base() {
    let spec = spec(TOML_REPLACE);
    let argv =
        build_argv(&spec, &request(None, &[], &[("temp", "0.9")]), &ctx()).expect("montar el argv");
    // La pareja "--temp 0.9" ocupa el sitio que tenía "--temp 0.7" en el
    // base (doc B §3.2.3: "pone `--temp <valor>` en su lugar").
    assert_eq!(
        argv,
        vec![
            "--ctx-size",
            "4096",
            "--temp",
            "0.9",
            "--n-gpu-layers",
            "99"
        ]
    );
    // La pareja antigua no puede quedar: el motor ganaría "por casualidad".
    assert_eq!(argv.iter().filter(|a| *a == "--temp").count(), 1);
    assert!(!argv.contains(&"0.7".to_string()), "argv={argv:?}");
}

/// Si la opción **no** se pide, el base **no se toca**: `--temp 0.7` sigue
/// en el argv y no aparece la pareja nueva.
#[test]
fn replace_no_toca_el_base_si_la_opcion_no_se_pide() {
    let spec = spec(TOML_REPLACE);
    let argv = build_argv(&spec, &request(None, &[], &[]), &ctx()).expect("montar el argv");
    assert_eq!(
        argv,
        vec![
            "--ctx-size",
            "4096",
            "--temp",
            "0.7",
            "--n-gpu-layers",
            "99",
        ]
    );
}

/// `replace` sobre un token que el base **no** declara: no es error; la
/// pareja nueva se añade al final y el base queda intacto.
///
/// Es el caso que la v1 documenta: *"Si el flag no está, no hace nada (no
/// es un error: el modelo puede no llevar ese flag en su base)"*
/// (`ropero-code/bin/python/modelconf.py:817-826`).
#[test]
fn replace_de_un_token_ausente_del_base_no_es_error() {
    let toml = TOML_REPLACE.replace(r#"replace = "--temp""#, r#"replace = "--inexistente""#);
    let spec = spec(&toml);
    let argv =
        build_argv(&spec, &request(None, &[], &[("temp", "0.9")]), &ctx()).expect("montar el argv");
    assert_eq!(
        argv,
        vec![
            "--ctx-size",
            "4096",
            "--temp",
            "0.7",
            "--n-gpu-layers",
            "99",
            "--temp",
            "0.9",
        ]
    );
}

/// `replace` sustituye la pareja **en el sitio que ocupaba** en el base: el
/// orden del argv es normativo (§3.2.3), así que el valor nuevo ocupa la
/// posición de la pareja vieja.
#[test]
fn replace_conserva_la_posicion_en_el_base() {
    let spec = spec(TOML_REPLACE);
    let argv =
        build_argv(&spec, &request(None, &[], &[("temp", "0.9")]), &ctx()).expect("montar el argv");
    // "--temp 0.9" va justo detrás de "--ctx-size 4096", donde estaba la
    // pareja "--temp 0.7" del base.
    let temp = argv
        .iter()
        .position(|a| a == "--temp")
        .expect("el flag --temp");
    assert_eq!(temp, 2, "argv={argv:?}");
    assert_eq!(argv[temp + 1], "0.9", "argv={argv:?}");
}

/// El base es una lista plana de tokens: la regla de la pareja es la de la
/// v1 (`ropero-code/bin/python/modelconf.py:817-826`), se quitan el token
/// **y el que le sigue**. Si el token es el último del base, sólo se quita
/// él.
#[test]
fn replace_sobre_el_ultimo_token_del_base_quita_solo_el_token() {
    let toml = TOML_REPLACE
        .replace(
            r#"base = ["--ctx-size", "${ctx}", "--temp", "0.7", "--n-gpu-layers", "99"]"#,
            r#"base = ["--ctx-size", "${ctx}", "--temp", "0.7", "--n-gpu-layers"]"#,
        )
        .replace(r#"replace = "--temp""#, r#"replace = "--n-gpu-layers""#);
    let spec = spec(&toml);
    let argv =
        build_argv(&spec, &request(None, &[], &[("temp", "0.9")]), &ctx()).expect("montar el argv");
    assert_eq!(
        argv,
        vec!["--ctx-size", "4096", "--temp", "0.7", "--temp", "0.9"]
    );
}

/// `replace` sobre un token del base que **no** es el último: se quitan el
/// token y el que le sigue (la pareja), no sólo el token.
#[test]
fn replace_quita_el_token_y_su_valor() {
    let toml = TOML_REPLACE.replace(r#"replace = "--temp""#, r#"replace = "--ctx-size""#);
    let spec = spec(&toml);
    let argv =
        build_argv(&spec, &request(None, &[], &[("temp", "0.9")]), &ctx()).expect("montar el argv");
    // Se fue "--ctx-size 4096" (la pareja sustituida) y la pareja nueva
    // "--temp 0.9" ocupa su sitio. El "--temp 0.7" del base sigue ahí: no
    // era el token sustituido.
    assert_eq!(
        argv,
        vec!["--temp", "0.9", "--temp", "0.7", "--n-gpu-layers", "99"]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Alias de nivel (`[aliases]`)
// ─────────────────────────────────────────────────────────────────────────────

/// TOML con un alias de nivel: `t` es exactamente `high`.
const TOML_ALIAS: &str = r#"
[model]
name = "modelo"
engine = "llama-server"
context = 4096

[files]
gguf = "modelo.gguf"

[levels]
list = ["low", "high"]
default = "low"

[aliases]
t = "high"

[args]
base = ["--ctx-size", "${ctx}"]

[args.level.low]
base = ["--reasoning-budget", "0"]

[args.level.high]
base = ["--reasoning-budget", "16384"]
"#;

/// Un alias de nivel se monta **exactamente** como el nivel al que apunta:
/// mismo argv y mismas consecuencias (doc B §3.2.2).
#[test]
fn alias_de_nivel_monta_el_nivel_canónico() {
    let spec = spec(TOML_ALIAS);
    let por_alias =
        build_argv(&spec, &request(Some("t"), &[], &[]), &ctx()).expect("argv del alias");
    let por_nivel =
        build_argv(&spec, &request(Some("high"), &[], &[]), &ctx()).expect("argv del nivel");
    assert_eq!(por_alias, por_nivel);
    assert_eq!(
        por_alias,
        vec!["--ctx-size", "4096", "--reasoning-budget", "16384"]
    );
}

/// El alias se reconoce como **nivel** al interpretar la línea de órdenes:
/// `--t` no es un flag (que sería un error de uso).
#[test]
fn alias_de_nivel_se_interpreta_como_nivel() {
    let spec = spec(TOML_ALIAS);
    let parsed = parse_request(&spec, &["--t".to_string()]);
    assert_eq!(parsed.level.as_deref(), Some("high"));
    assert!(parsed.flags.is_empty(), "flags={:?}", parsed.flags);
}

/// Un alias que apunta a un nivel inexistente es **error de uso**: el argv
/// no se monta.
#[test]
fn alias_de_nivel_a_un_nivel_inexistente_es_error_de_uso() {
    let toml = TOML_ALIAS.replace(r#"t = "high""#, r#"t = "inexistente""#);
    let spec = spec(&toml);
    let err = build_argv(&spec, &request(Some("t"), &[], &[]), &ctx())
        .expect_err("un alias a un nivel inexistente debe fallar");
    match err {
        ArgvError::UnknownLevel { level, .. } => assert_eq!(level, "inexistente"),
        other => panic!("se esperaba UnknownLevel, llegó {other:?}"),
    }
}

/// Un nivel que no existe **y** no es alias es error de uso.
#[test]
fn nivel_desconocido_es_error_de_uso() {
    let spec = spec(TOML_ALIAS);
    let err = build_argv(&spec, &request(Some("medio"), &[], &[]), &ctx())
        .expect_err("un nivel desconocido debe fallar");
    assert!(matches!(err, ArgvError::UnknownLevel { .. }), "{err:?}");
}

// ─────────────────────────────────────────────────────────────────────────────
// `requires`
// ─────────────────────────────────────────────────────────────────────────────

/// `requires` es una precondición de disco: si el fichero no está, el flag
/// es error de uso (doc B §3.2.3).
#[test]
fn requires_de_un_fichero_ausente_es_error_de_uso() {
    let toml = r#"
[model]
name = "modelo"
engine = "llama-server"
context = 4096

[files]
gguf = "modelo.gguf"

[flags]
list = ["sin-vision"]

[args]
base = ["--ctx-size", "${ctx}"]

[args.flag.sin-vision]
base = ["--no-mmproj"]
requires = ["mmproj"]
"#;
    let spec = spec(toml);
    let err = build_argv(&spec, &request(None, &["sin-vision"], &[]), &ctx())
        .expect_err("un flag sin su fichero requerido debe fallar");
    assert!(matches!(err, ArgvError::MissingFile { .. }), "{err:?}");
}

/// Con el fichero disponible, el flag se monta.
#[test]
fn requires_satisfecho_monta_el_flag() {
    let toml = r#"
[model]
name = "modelo"
engine = "llama-server"
context = 4096

[files]
gguf = "modelo.gguf"

[flags]
list = ["sin-vision"]

[args]
base = ["--ctx-size", "${ctx}"]

[args.flag.sin-vision]
base = ["--no-mmproj"]
requires = ["mmproj"]
"#;
    let spec = spec(toml);
    let mut ctx = ctx();
    ctx.files.mmproj = Some("/g/modelo/mmproj.gguf".to_string());
    let argv = build_argv(&spec, &request(None, &["sin-vision"], &[]), &ctx).expect("argv");
    assert_eq!(argv, vec!["--ctx-size", "4096", "--no-mmproj"]);
}
