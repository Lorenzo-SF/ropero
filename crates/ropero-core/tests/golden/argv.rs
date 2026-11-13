//! Golden master del argv (T-08).
//!
//! Procedimiento §8.1 del doc B: antes de reescribir una función se congela
//! su comportamiento actual. Este fichero congela el argv que la **v1**
//! (`ropero-code/bin/python/modelconf.py args`) produce para los TOML
//! reales, y comprueba que la implementación en Rust produce exactamente lo
//! mismo.
//!
//! ## Procedencia de los valores congelados
//!
//! Los argv de abajo se capturaron **ejecutando la v1 en modo lectura**
//! (la v1 no se modificó) con un entorno fijo, para que la captura no
//! dependa del HOME ni del GGUF_DIR de quien la ejecuta:
//!
//! ```text
//! cd ropero-code
//! GGUF_DIR=/golden/gguf SAFETENSORS_DIR=/golden/safetensors HOME=/golden/home \
//!   python3 bin/python/modelconf.py args --root models --name <modelo>
//! ```
//!
//! La v1 imprime **un argumento por línea**; el test los junta en un
//! `Vec<String>` en ese mismo orden.
//!
//! | Test | Modelo | Comando v1 | Petición extra |
//! |---|---|---|---|
//! | `golden_argv_groot` | `groot` | `modelconf.py args --root models --name groot` | (nivel por defecto) |
//! | `golden_argv_nemotron` | `nemotron` | `modelconf.py args --root models --name nemotron` | (nivel por defecto) |
//! | `golden_argv_qwen38` | `qwen38` | `modelconf.py args --root models --name qwen38` | (nivel por defecto) |
//! | `golden_argv_gyro_m` | `gyro-m` | `modelconf.py args --root models --name gyro-m` | (nivel por defecto) |
//!
//! ## `replace` NO está ejercitado por el catálogo real
//!
//! Ningún TOML de `ropero-code/models/` declara `[args.option.<o>]`, y por
//! tanto **ninguno declara `replace`**: la v1 no tiene ningún caso real que
//! congelar para esta clave. Se documenta aquí (y se cubre con el test
//! sintético `replace_sustituye_la_pareja_del_base` de
//! `crates/ropero-core/tests/args.rs`) para que conste que la puerta de
//! T-08 se cierra por golden test **y** por test sintético.
//!
//! `gyro-m` sí declara `[options].list` (7 opciones) pero **no** declara los
//! bloques `[args.option.<o>]`: están comentados en el TOML. Por eso su argv
//! frozen es vacío, y eso también se congela.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ropero_core::{
    LaunchRequest, ModelSpec, ResolveContext, ResolvedFiles, build_argv, parse_model,
};

/// Base del catálogo real de la v1: `<repo-root>/ropero-code/models`.
fn base_real() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("ropero-code")
        .join("models")
}

/// Contexto de resolución que corresponde al entorno fijo de la captura.
///
/// La v1 resolvió `GGUF_DIR=/golden/gguf`, `SAFETENSORS_DIR=/golden/safetensors`
/// y `HOME=/golden/home`. El `ResolveContext` se construye a mano (no con
/// `resolve_context`, que lee el entorno real) para que el golden master sea
/// hermético.
fn ctx_golden(context: u32, files: ResolvedFiles) -> ResolveContext {
    ResolveContext {
        context,
        files,
        env: BTreeMap::new(),
    }
}

/// Ficheros resueltos de un modelo `gguf` sin `chat_template`.
fn files_golden(model: &str, gguf: &str) -> ResolvedFiles {
    ResolvedFiles {
        gguf: Some(format!("/golden/gguf/{model}/{gguf}")),
        mtp: None,
        mmproj: None,
        chat_template: None,
        gguf_dir: "/golden/gguf".to_string(),
        safetensors_dir: "/golden/safetensors".to_string(),
        model_path: String::new(),
    }
}

/// Parsea un TOML real del catálogo de la v1.
fn spec_real(model: &str) -> ModelSpec {
    let path = base_real().join(model);
    let text = std::fs::read_to_string(&path).expect("leer el TOML real del modelo");
    parse_model(&text, &path).expect("parsear el TOML real del modelo")
}

/// Petición vacía: el nivel por defecto del TOML.
fn request_default() -> LaunchRequest {
    LaunchRequest {
        level: None,
        flags: Vec::new(),
        options: Vec::new(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// groot — llama-server, nivel por defecto "think"
// ─────────────────────────────────────────────────────────────────────────────

/// argv congelado de la v1 para `groot`.
///
/// Captura: `GGUF_DIR=/golden/gguf SAFETENSORS_DIR=/golden/safetensors
/// HOME=/golden/home python3 bin/python/modelconf.py args --root models
/// --name groot`
const ARGV_GROOT: &[&str] = &[
    "--n-gpu-layers",
    "999",
    "--kv-unified",
    "--kv-offload",
    "--cache-reuse",
    "512",
    "--cache-type-k",
    "q8_0",
    "--cache-type-v",
    "q8_0",
    "--n-predict",
    "8192",
    "--keep",
    "16384",
    "--presence-penalty",
    "1.5",
    "--repeat-penalty",
    "1.0",
    "--spec-type",
    "draft-mtp",
    "--spec-draft-n-max",
    "3",
    "--spec-draft-n-min",
    "0",
    "--spec-draft-p-min",
    "0.8",
    "--spec-draft-type-k",
    "q8_0",
    "--spec-draft-type-v",
    "q8_0",
    "--temp",
    "1.0",
    "--top-p",
    "0.95",
    "--top-k",
    "20",
    "--min-p",
    "0.0",
    "--reasoning",
    "true",
    "--chat-template-kwargs",
    r#"{"enable_thinking":true}"#,
];

#[test]
fn golden_argv_groot() {
    let spec = spec_real("llama/groot.toml");
    let ctx = ctx_golden(
        65536,
        files_golden("groot", "Groot-19B-A3B-261023-NVFP4.gguf"),
    );
    let argv = build_argv(&spec, &request_default(), &ctx).expect("montar el argv");
    assert_eq!(argv, ARGV_GROOT);
}

// ─────────────────────────────────────────────────────────────────────────────
// nemotron — llama-server, nivel por defecto "instruct"
// ─────────────────────────────────────────────────────────────────────────────

/// argv congelado de la v1 para `nemotron`.
///
/// Captura: `GGUF_DIR=/golden/gguf SAFETENSORS_DIR=/golden/safetensors
/// HOME=/golden/home python3 bin/python/modelconf.py args --root models
/// --name nemotron`
const ARGV_NEMOTRON: &[&str] = &[
    "--n-gpu-layers",
    "54",
    "--n-cpu-moe",
    "12",
    "--kv-unified",
    "--kv-offload",
    "--cache-type-k",
    "q8_0",
    "--cache-type-v",
    "q8_0",
    "--repeat-penalty",
    "1.1",
    "--repeat-last-n",
    "64",
    "--n-predict",
    "8192",
    "--keep",
    "16384",
    "--chat-template-file",
    "/golden/gguf/nemotron/chat_template.jinja",
    "--spec-type",
    "ngram-map-k4v",
    "--spec-ngram-map-k4v-size-n",
    "24",
    "--spec-ngram-map-k4v-size-m",
    "48",
    "--spec-ngram-map-k4v-min-hits",
    "1",
    "--temp",
    "0.2",
    "--reasoning",
    "false",
    "--spec-draft-n-max",
    "3",
    "--chat-template-kwargs",
    r#"{"enable_thinking":false}"#,
];

#[test]
fn golden_argv_nemotron() {
    let spec = spec_real("llama/nemotron.toml");
    let mut files = files_golden(
        "nemotron",
        "NVIDIA-Nemotron-3.5-Lightning-30B-A3B-NVFP4-MTP.gguf",
    );
    files.chat_template = Some("/golden/gguf/nemotron/chat_template.jinja".to_string());
    let ctx = ctx_golden(65536, files);
    let argv = build_argv(&spec, &request_default(), &ctx).expect("montar el argv");
    assert_eq!(argv, ARGV_NEMOTRON);
}

// ─────────────────────────────────────────────────────────────────────────────
// qwen38 — llama-server, nivel por defecto "low"
// ─────────────────────────────────────────────────────────────────────────────

/// argv congelado de la v1 para `qwen38`.
///
/// Captura: `GGUF_DIR=/golden/gguf SAFETENSORS_DIR=/golden/safetensors
/// HOME=/golden/home python3 bin/python/modelconf.py args --root models
/// --name qwen38`
const ARGV_QWEN38: &[&str] = &[
    "--n-gpu-layers",
    "66",
    "--cache-type-k",
    "turbo3",
    "--cache-type-v",
    "turbo3",
    "--n-predict",
    "16384",
    "--reasoning-preserve",
    "--checkpoint-min-step",
    "1024",
    "--spec-type",
    "nextn",
    "--spec-draft-n-max",
    "4",
    "--spec-draft-n-min",
    "1",
    "--temp",
    "0.6",
    "--top-p",
    "0.95",
    "--top-k",
    "20",
    "--min-p",
    "0.05",
    "--presence-penalty",
    "0.6",
    "--repeat-penalty",
    "1.0",
    "--reasoning-effort",
    "low",
    "--reasoning-budget",
    "2048",
    "--chat-template-kwargs",
    r#"{"reasoning_effort":"low"}"#,
];

#[test]
fn golden_argv_qwen38() {
    let spec = spec_real("llama/qwen38.toml");
    let ctx = ctx_golden(
        65536,
        files_golden("qwen38", "Qwen3.8-27B-GSQ-RCO-IQ3_S-mtp.gguf"),
    );
    let argv = build_argv(&spec, &request_default(), &ctx).expect("montar el argv");
    assert_eq!(argv, ARGV_QWEN38);
}

// ─────────────────────────────────────────────────────────────────────────────
// gyro-m — strata, sin [args]: argv vacío
// ─────────────────────────────────────────────────────────────────────────────

/// argv congelado de la v1 para `gyro-m`: **vacío**.
///
/// El TOML declara `[options].list` (7 opciones) pero sus bloques
/// `[args.option.<o>]` están comentados, y no hay `[args].base`: el argv
/// montado es vacío.
///
/// Captura: `GGUF_DIR=/golden/gguf SAFETENSORS_DIR=/golden/safetensors
/// HOME=/golden/home python3 bin/python/modelconf.py args --root models
/// --name gyro-m`
const ARGV_GYRO_M: &[&str] = &[];

#[test]
fn golden_argv_gyro_m() {
    let spec = spec_real("strata/gyro-m.toml");
    let ctx = ctx_golden(
        65536,
        ResolvedFiles {
            gguf: None,
            mtp: None,
            mmproj: None,
            chat_template: None,
            gguf_dir: "/golden/gguf".to_string(),
            safetensors_dir: "/golden/safetensors".to_string(),
            model_path: "gyro/Gyro-M".to_string(),
        },
    );
    let argv = build_argv(&spec, &request_default(), &ctx).expect("montar el argv");
    assert_eq!(argv, ARGV_GYRO_M);
}
