//! El predicado **único** sobre `[files]` (doc B §4.1.1 y §3.2.3).
//!
//! `[files]` declara cuatro claves: `gguf`, `mtp`, `mmproj` y
//! `chat_template`. Dos lugares del dominio las consultan: la validación y
//! el montaje del argv. Antes cada uno tenía su propio `match` y no se
//! parecían (la validación daba error ante una clave desconocida y el argv
//! la aceptaba en silencio).
//!
//! Aquí se comprueba que:
//! - el **vocabulario** de `[files]` es uno solo (`FILE_KEYS`/`is_file_key`);
//! - el **predicado** es uno solo (`has_file`), y da la misma respuesta sea
//!   la tabla de lo declarado (`Files`) o la de lo resuelto (`ResolvedFiles`);
//! - la validación y el argv preguntan al mismo predicado, y cada uno aplica
//!   su política sobre la clave desconocida sin duplicar el `match`.

use std::collections::BTreeMap;
use std::path::Path;

use ropero_core::{
    ArgvError, Files, LaunchRequest, ModelSpec, ResolveContext, ResolvedFiles, ValidationContext,
    ValidationRule, build_argv, has_file, is_file_key, parse_model, validate,
};

/// Un `[files]` con sólo `gguf` declarado.
fn files_con_gguf() -> Files {
    Files {
        gguf: Some(Path::new("modelo.gguf").to_path_buf()),
        mtp: None,
        mmproj: None,
        chat_template: None,
    }
}

/// Un contexto resuelto con sólo `gguf` disponible.
fn ctx_con_gguf() -> ResolveContext {
    ResolveContext {
        context: 4096,
        files: ResolvedFiles {
            gguf: Some("/g/modelo/modelo.gguf".to_string()),
            mtp: None,
            mmproj: None,
            chat_template: None,
            gguf_dir: "/g".to_string(),
            safetensors_dir: "/s".to_string(),
            model_path: String::new(),
        },
        env: BTreeMap::new(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// El vocabulario
// ─────────────────────────────────────────────────────────────────────────────

/// Las cuatro claves de `[files]`, y sólo esas.
#[test]
fn el_vocabulario_de_files_tiene_cuatro_claves() {
    assert_eq!(
        ropero_core::FILE_KEYS,
        &["gguf", "mtp", "mmproj", "chat_template"]
    );
    for clave in ropero_core::FILE_KEYS {
        assert!(is_file_key(clave), "la clave '{clave}' es de [files]");
    }
    assert!(!is_file_key("inventada"));
    assert!(!is_file_key(""));
}

/// El vocabulario no depende de ninguna tabla: es el mismo para la
/// validación y para el argv.
#[test]
fn el_vocabulario_es_el_mismo_para_las_dos_tablas() {
    assert!(is_file_key("gguf"));
    // La tabla no declara `gguf` en este caso, pero la clave sigue siendo
    // del vocabulario: eso lo decide `is_file_key`, no la tabla.
    let vacia = Files {
        gguf: None,
        mtp: None,
        mmproj: None,
        chat_template: None,
    };
    assert!(is_file_key("gguf"));
    assert!(!has_file(&vacia, "gguf"));
}

// ─────────────────────────────────────────────────────────────────────────────
// El predicado único
// ─────────────────────────────────────────────────────────────────────────────

/// El predicado responde por la clave que está en la tabla.
#[test]
fn el_predicado_encuentra_la_clave_declarada() {
    let files = files_con_gguf();
    assert!(has_file(&files, "gguf"));
    assert!(!has_file(&files, "mtp"));
    assert!(!has_file(&files, "mmproj"));
    assert!(!has_file(&files, "chat_template"));
}

/// El predicado da **la misma respuesta** sobre la tabla declarada y sobre
/// la tabla resuelta: es el mismo predicado, no dos.
#[test]
fn el_predicado_da_la_misma_respuesta_en_ambas_tablas() {
    let declarados = files_con_gguf();
    let resueltos = ctx_con_gguf();
    for clave in ropero_core::FILE_KEYS {
        assert_eq!(
            has_file(&declarados, clave),
            has_file(&resueltos.files, clave),
            "las dos tablas discrepan en '{clave}'"
        );
    }
}

/// Una clave que **no** es de `[files]` no está en ninguna tabla.
#[test]
fn una_clave_desconocida_no_esta_en_ninguna_tabla() {
    let declarados = files_con_gguf();
    let resueltos = ctx_con_gguf();
    assert!(!has_file(&declarados, "inventada"));
    assert!(!has_file(&resueltos.files, "inventada"));
}

// ─────────────────────────────────────────────────────────────────────────────
// La validación y el argv preguntan al mismo predicado
// ─────────────────────────────────────────────────────────────────────────────

/// Un TOML cuyo flag exige un fichero que no está en `[files]`.
const TOML_REQUIRES_AUSENTE: &str = r#"
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

/// Un TOML cuyo flag exige una clave que **no** existe en `[files]`.
const TOML_REQUIRES_DESCONOCIDA: &str = r#"
[model]
name = "modelo"
engine = "llama-server"
context = 4096

[files]
gguf = "modelo.gguf"

[flags]
list = ["raro"]

[args]
base = ["--ctx-size", "${ctx}"]

[args.flag.raro]
base = ["--raro"]
requires = ["clave-que-no-existe"]
"#;

fn spec(toml: &str) -> ModelSpec {
    parse_model(toml, Path::new("models/llama/modelo.toml")).expect("parsear el modelo")
}

/// La validación da error ante un `requires` que no nombra una clave
/// existente en `[files]` (doc B §4.1.1).
#[test]
fn la_validacion_rechaza_un_requires_desconocido() {
    let spec = spec(TOML_REQUIRES_DESCONOCIDA);
    let ctx = ValidationContext {
        source: "models/llama/modelo.toml".to_string(),
        source_text: Some(TOML_REQUIRES_DESCONOCIDA.to_string()),
        seen_names: Default::default(),
    };
    let errores = validate(&spec, &ctx);
    let reglas: Vec<ValidationRule> = errores.iter().map(|e| e.rule()).collect();
    assert!(
        reglas.contains(&ValidationRule::FlagRequiresMissingFile),
        "la validación no rechazó el requires desconocido: {reglas:?}"
    );
}

/// La validación da error ante un `requires` que nombra una clave de
/// `[files]` que no está declarada.
#[test]
fn la_validacion_rechaza_un_requires_no_declarado() {
    let spec = spec(TOML_REQUIRES_AUSENTE);
    let ctx = ValidationContext {
        source: "models/llama/modelo.toml".to_string(),
        source_text: Some(TOML_REQUIRES_AUSENTE.to_string()),
        seen_names: Default::default(),
    };
    let errores = validate(&spec, &ctx);
    let reglas: Vec<ValidationRule> = errores.iter().map(|e| e.rule()).collect();
    assert!(
        reglas.contains(&ValidationRule::FlagRequiresMissingFile),
        "la validación no rechazó el requires no declarado: {reglas:?}"
    );
}

/// El argv: un `requires` que nombra una clave de `[files]` y no está
/// disponible es error de uso (doc B §3.2.3).
#[test]
fn el_argv_rechaza_un_requires_no_disponible() {
    let spec = spec(TOML_REQUIRES_AUSENTE);
    let peticion = LaunchRequest {
        level: None,
        flags: vec!["sin-vision".to_string()],
        options: Vec::new(),
    };
    let err = build_argv(&spec, &peticion, &ctx_con_gguf())
        .expect_err("un flag sin su fichero requerido debe fallar");
    assert!(matches!(err, ArgvError::MissingFile { .. }), "{err:?}");
}

/// El argv **no cambia** con una clave que no es de `[files]`: se acepta,
/// como antes. El TOML inválido lo caza `validate`, que sí conoce el
/// vocabulario.
#[test]
fn el_argv_sigue_aceptando_un_requires_desconocido() {
    let spec = spec(TOML_REQUIRES_DESCONOCIDA);
    let peticion = LaunchRequest {
        level: None,
        flags: vec!["raro".to_string()],
        options: Vec::new(),
    };
    let argv = build_argv(&spec, &peticion, &ctx_con_gguf())
        .expect("una clave desconocida en requires no debe romper el argv");
    assert_eq!(argv, vec!["--ctx-size", "4096", "--raro"]);
}
