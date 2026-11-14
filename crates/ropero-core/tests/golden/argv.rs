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
//! ## Los TOML de origen están congelados en el repo (`tests/golden/models/`)
//!
//! La batería por defecto (`cargo test`) tiene que ser **hermética**: no
//! puede depender de `ropero-code/`, que es la v1 y puede no estar (o estar
//! desactualizada). Por eso los cuatro TOML reales que sirvieron de origen
//! están **congelados como fixtures** en `tests/golden/models/`, **copia
//! literal** de `ropero-code/models/` en el momento de la captura.
//!
//! El argv congelado, en cambio, **sigue siendo el auténtico de la v1**: no
//! se ha re-derivado de la v2. La prueba de que eso es cierto es
//! `golden_deriva_contra_la_v1`, que corre **sólo** cuando el catálogo real
//! está disponible (`cargo test --locked --workspace -- --ignored`): compara
//! byte a byte el argv congelado con el que produce la v1 hoy. Si alguien
//! toca el argv congelado sin re-congelarlo contra la v1, ese test falla.
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

/// Base del catálogo **congelado**: `<este target>/models`.
///
/// Los TOML son copia literal de `ropero-code/models/` en el momento de la
/// captura. La batería por defecto no lee `ropero-code/`: es hermética.
fn base_frozen() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("models")
}

/// Base del catálogo real de la v1: `<repo-root>/ropero-code/models`.
///
/// Sólo la usa el test de deriva, que está marcado `#[ignore]`.
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

/// Parsea un TOML **congelado** del fixture del golden.
fn spec_frozen(model: &str) -> ModelSpec {
    let path = base_frozen().join(model);
    let text = std::fs::read_to_string(&path).expect("leer el TOML congelado del modelo");
    parse_model(&text, &path).expect("parsear el TOML congelado del modelo")
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
    let spec = spec_frozen("llama/groot.toml");
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
    let spec = spec_frozen("llama/nemotron.toml");
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
    let spec = spec_frozen("llama/qwen38.toml");
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
    let spec = spec_frozen("strata/gyro-m.toml");
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

// ─────────────────────────────────────────────────────────────────────────────
// La deriva contra la v1 real
// ─────────────────────────────────────────────────────────────────────────────

/// El argv congelado **sigue siendo** el que produce la v1.
///
/// Este test compara, byte a byte, el argv congelado con el que monta la
/// v1 (`ropero-code/bin/python/modelconf.py`) hoy, sobre el catálogo real.
/// Está marcado `#[ignore]` porque la v1 no siempre está presente (y porque
/// la batería por defecto tiene que ser hermética): se ejecuta con
///
/// ```text
/// cargo test --locked --workspace -- --ignored
/// ```
///
/// Si el catálogo real no está, se omite con un mensaje claro (el mismo
/// patrón que `crates/ropero-core/tests/catalogo_real.rs`).
///
/// **Para qué sirve**: los fixtures de `tests/golden/models/` son una
/// *copia* del catálogo de la v1 en el momento de la captura. Si la v1
/// cambiara (o alguien re-congelara el argv a mano sin la v1 delante), este
/// test avisa: hay que re-congelar los fixtures y el argv.
#[test]
#[ignore = "compara el golden con la v1 real; ejecutar con `cargo test --locked --workspace -- --ignored`"]
fn golden_deriva_contra_la_v1() {
    let base = base_real();
    if !base.is_dir() {
        eprintln!(
            "omitido: no existe el catálogo real en {} (la v1 no está presente)",
            base.display()
        );
        return;
    }
    // La v1 imprime un argumento por línea.
    let v1 = |model: &str| -> Option<Vec<String>> {
        let output = std::process::Command::new("python3")
            .arg("bin/python/modelconf.py")
            .args(["args", "--root", "models", "--name", model])
            .current_dir(base.parent().expect("el directorio de la v1"))
            .env("GGUF_DIR", "/golden/gguf")
            .env("SAFETENSORS_DIR", "/golden/safetensors")
            .env("HOME", "/golden/home")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_string)
                .collect(),
        )
    };
    let expected = |argv: &[&str]| -> Vec<String> { argv.iter().map(|s| s.to_string()).collect() };

    for (model, frozen) in [
        ("groot", expected(ARGV_GROOT)),
        ("nemotron", expected(ARGV_NEMOTRON)),
        ("qwen38", expected(ARGV_QWEN38)),
        ("gyro-m", expected(ARGV_GYRO_M)),
    ] {
        let Some(produced) = v1(model) else {
            eprintln!("omitido: la v1 no pudo producir el argv de '{model}'");
            continue;
        };
        assert_eq!(
            produced, frozen,
            "el argv congelado de '{model}' ya no es el de la v1: re-congela los fixtures de \
             tests/golden/models/ y el argv congelado"
        );
    }
}

/// Los fixtures congelados son copia literal del catálogo de la v1.
///
/// Si el catálogo real cambia, hay que re-congelar los fixtures: este test
/// lo dice (no falla), porque la deriva del *contenido* del TOML no es
/// necesariamente un error (la v1 puede evolucionar sin que el argv cambie).
#[test]
#[ignore = "compara los fixtures congelados con el catálogo real; ejecutar con `cargo test --locked --workspace -- --ignored`"]
fn los_fixtures_congelados_coinciden_con_el_catalogo_real() {
    let base = base_real();
    if !base.is_dir() {
        eprintln!(
            "omitido: no existe el catálogo real en {} (la v1 no está presente)",
            base.display()
        );
        return;
    }
    for model in [
        "llama/groot.toml",
        "llama/nemotron.toml",
        "llama/qwen38.toml",
        "strata/gyro-m.toml",
    ] {
        let real = base.join(model);
        let frozen = base_frozen().join(model);
        let real_text = std::fs::read_to_string(&real).expect("leer el TOML real");
        let frozen_text = std::fs::read_to_string(&frozen).expect("leer el TOML congelado");
        if real_text != frozen_text {
            eprintln!(
                "aviso: el fixture congelado {} ha divergido del catálogo real {}: \
                 re-congélalo si el cambio es intencionado",
                frozen.display(),
                real.display()
            );
        }
    }
}
