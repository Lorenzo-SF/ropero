//! Tests de los identificadores de motor y de sus alias legacy (T-11).
//!
//! Doc B §4.2 y [ADR-004](../../../../docs/adr/ADR-004-nomenclatura-de-motores.md):
//! ocho motores con ids canónicos y alias legacy. Un alias legacy se
//! canonicaliza **con un aviso**, nunca en silencio:
//!
//! ```text
//! AVISO: el motor 'mlx_lm' es un alias antiguo; se usa 'mlx-serve'
//! ```
//!
//! **Cambio semántico de ADR-004**: `llama-server` pasa a significar el repo
//! **oficial** de llama.cpp (antes era el fork) y el fork privado pasa a
//! llamarse `turbo-llama` (antes `llama-server-official` era el oficial).
//!
//! Puerta de T-11: "Test de cada alias y de cada id canónico".

use std::path::Path;

use ropero_core::{EngineId, ModelSpec, parse_model};

/// Los ocho ids canónicos (ADR-004) y su identificador canónico.
const CANONICOS: &[(&str, EngineId)] = &[
    ("llama-server", EngineId::LlamaServer),
    ("turbo-llama", EngineId::TurboLlama),
    ("mlx-serve", EngineId::MlxServe),
    ("fastflowllm", EngineId::FastFlowLm),
    ("tensorrt-llm", EngineId::TensorRtLlm),
    ("vllm", EngineId::Vllm),
    ("airllm", EngineId::AirLlm),
    ("strata", EngineId::Strata),
    ("ignis", EngineId::Ignis),
    ("basalt", EngineId::Basalt),
    ("colibri", EngineId::Colibri),
];

/// Los alias legacy (ADR-004 §"Alias legacy") y su id canónico.
///
/// `llama-server-official` y `llama-official` apuntaban al oficial y ahora
/// apuntan a `llama-server`; `llama` también. `turbo` es el alias nuevo del
/// fork.
const ALIAS_LEGACY: &[(&str, EngineId)] = &[
    ("llama-server-official", EngineId::LlamaServer),
    ("llama-official", EngineId::LlamaServer),
    ("llama", EngineId::LlamaServer),
    ("turbo", EngineId::TurboLlama),
    ("mlx_lm", EngineId::MlxServe),
    ("fastflowlm", EngineId::FastFlowLm),
    ("flm", EngineId::FastFlowLm),
    ("tensorrt", EngineId::TensorRtLlm),
    ("strata_engine", EngineId::Strata),
];

/// Un TOML mínimo con el motor dado.
fn toml_con_motor(engine: &str) -> String {
    format!(
        r#"
[model]
name = "modelo"
engine = "{engine}"
context = 4096

[files]
gguf = "modelo.gguf"
"#
    )
}

/// Parsea el TOML con el motor dado.
fn parse(engine: &str) -> Result<ModelSpec, ropero_core::ModelError> {
    parse_model(
        &toml_con_motor(engine),
        Path::new("models/llama/modelo.toml"),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Los ocho ids canónicos
// ─────────────────────────────────────────────────────────────────────────────

/// Cada id canónico se acepta y se queda como es, sin aviso.
#[test]
fn los_nueve_ids_canonicos_se_aceptan_sin_aviso() {
    for (id, esperado) in CANONICOS {
        let spec =
            parse(id).unwrap_or_else(|e| panic!("el id canónico '{id}' debe aceptarse: {e}"));
        assert_eq!(spec.model.engine, *esperado, "id={id}");
        assert!(
            spec.notices.is_empty(),
            "el id canónico '{id}' no debe avisar: {:?}",
            spec.notices
        );
    }
}

/// El identificador canónico de cada motor es el que dice la tabla.
#[test]
fn cada_motor_expone_su_id_canonico() {
    for (id, motor) in CANONICOS {
        assert_eq!(motor.as_str(), *id);
    }
}

/// ADR-004: `llama-server` es el repo **oficial** de llama.cpp y el fork
/// privado se llama `turbo-llama`. El cambio semántico crítico.
#[test]
fn adr_004_llama_server_es_el_oficial_y_turbo_llama_el_fork() {
    assert_eq!(
        EngineId::from_str_legacy("llama-server"),
        Ok((EngineId::LlamaServer, None)),
        "'llama-server' debe ser el motor oficial"
    );
    assert_eq!(
        EngineId::from_str_legacy("turbo-llama"),
        Ok((EngineId::TurboLlama, None)),
        "'turbo-llama' debe ser el fork privado"
    );
    assert_ne!(
        EngineId::LlamaServer,
        EngineId::TurboLlama,
        "el oficial y el fork son motores distintos"
    );
}

/// La variante `LlamaServerOfficial` ya no existe: el oficial **es**
/// `LlamaServer` (ADR-004).
#[test]
fn la_variante_del_oficial_desaparece() {
    // Se comprueba por comportamiento: el id "llama-server-official" ya no
    // es un id canónico, es un alias.
    let (motor, aviso) = EngineId::from_str_legacy("llama-server-official")
        .expect("'llama-server-official' debe aceptarse como alias");
    assert_eq!(motor, EngineId::LlamaServer);
    assert!(
        aviso.is_some(),
        "'llama-server-official' ya no es canónico: debe avisar"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Los alias legacy
// ─────────────────────────────────────────────────────────────────────────────

/// Cada alias legacy se canonicaliza al id canónico, **con aviso**.
#[test]
fn los_alias_legacy_se_canonicalizan_con_aviso() {
    for (alias, esperado) in ALIAS_LEGACY {
        let spec =
            parse(alias).unwrap_or_else(|e| panic!("el alias '{alias}' debe aceptarse: {e}"));
        assert_eq!(spec.model.engine, *esperado, "alias={alias}");
        assert_eq!(
            spec.notices,
            vec![format!(
                "AVISO: el motor '{alias}' es un alias antiguo; se usa '{}'",
                esperado.as_str()
            )],
            "alias={alias}"
        );
    }
}

/// El aviso lleva **el formato literal del doc** (§4.2).
#[test]
fn el_aviso_usa_el_formato_literal_del_doc() {
    let spec = parse("mlx_lm").expect("el alias 'mlx_lm' debe aceptarse");
    assert_eq!(
        spec.notices,
        vec!["AVISO: el motor 'mlx_lm' es un alias antiguo; se usa 'mlx-serve'".to_string()]
    );
}

/// Un alias legacy nunca se canonicaliza **en silencio**: el aviso siempre
/// está.
#[test]
fn un_alias_legacy_nunca_se_canonicaliza_en_silencio() {
    for (alias, _) in ALIAS_LEGACY {
        let spec = parse(alias).expect("el alias debe aceptarse");
        assert!(
            !spec.notices.is_empty(),
            "el alias '{alias}' debe avisar: se canonicalizó en silencio"
        );
    }
}

/// `from_str_legacy` acepta los ocho canónicos y los alias, y devuelve el
/// aviso con el formato del doc.
#[test]
fn from_str_legacy_acepta_canonicos_y_alias() {
    for (id, esperado) in CANONICOS {
        let (motor, aviso) = EngineId::from_str_legacy(id).expect("id canónico");
        assert_eq!(motor, *esperado, "id={id}");
        assert!(aviso.is_none(), "el id canónico '{id}' no debe avisar");
    }
    for (alias, esperado) in ALIAS_LEGACY {
        let (motor, aviso) = EngineId::from_str_legacy(alias).expect("alias legacy");
        assert_eq!(motor, *esperado, "alias={alias}");
        assert_eq!(
            aviso.as_deref(),
            Some(
                format!(
                    "AVISO: el motor '{alias}' es un alias antiguo; se usa '{}'",
                    esperado.as_str()
                )
                .as_str()
            ),
            "alias={alias}"
        );
    }
}

#[test]
fn from_dir_name_acepta_ids_canonicos_y_alias_de_catalogo() {
    for (dir, expected) in [
        ("fastflowllm", EngineId::FastFlowLm),
        ("fastflowlm", EngineId::FastFlowLm),
        ("flm", EngineId::FastFlowLm),
        ("mlx-serve", EngineId::MlxServe),
        ("mlx", EngineId::MlxServe),
        ("tensorrt-llm", EngineId::TensorRtLlm),
        ("tensorrt", EngineId::TensorRtLlm),
    ] {
        assert_eq!(EngineId::from_dir_name(dir), Some(expected), "{dir}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// El id desconocido
// ─────────────────────────────────────────────────────────────────────────────

/// Un id que no es ni canónico ni alias legacy es **error de parseo**, con
/// un mensaje en español que dice qué hacer.
#[test]
fn un_motor_desconocido_es_error_de_parseo_en_espanol() {
    let err = parse("no-existe").expect_err("un motor desconocido debe fallar");
    let texto = err.to_string();
    assert!(texto.contains("motor desconocido"), "mensaje={texto}");
    assert!(texto.contains("no-existe"), "mensaje={texto}");
    // El mensaje dice qué hacer: los ids válidos.
    assert!(
        texto.contains("llama-server") && texto.contains("mlx-serve"),
        "mensaje={texto}"
    );
}

/// `from_str_legacy` rechaza un id desconocido.
#[test]
fn from_str_legacy_rechaza_un_id_desconocido() {
    let err = EngineId::from_str_legacy("no-existe").expect_err("id desconocido");
    assert_eq!(err.name(), "no-existe");
    assert!(err.to_string().contains("motor desconocido"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Los directorios de motor
// ─────────────────────────────────────────────────────────────────────────────

/// Los directorios de `models/` se mapean a su motor: `turbo-llama/` es
/// `TurboLlama`, `llama-server/` es el oficial, etc. (los directorios no son
/// ids de motor: son carpetas).
#[test]
fn los_directorios_de_motor_se_mapean_a_su_motor() {
    let casos = [
        ("turbo-llama", EngineId::TurboLlama),
        ("llama-server", EngineId::LlamaServer),
        ("mlx", EngineId::MlxServe),
        ("flm", EngineId::FastFlowLm),
        ("tensorrt", EngineId::TensorRtLlm),
        ("vllm", EngineId::Vllm),
        ("airllm", EngineId::AirLlm),
        ("strata", EngineId::Strata),
    ];
    for (dir, esperado) in casos {
        assert_eq!(EngineId::from_dir_name(dir), Some(esperado), "dir={dir}");
    }
    assert_eq!(EngineId::from_dir_name("no-existe"), None);
}

/// Los directorios **legacy** del catálogo de la v1 siguen mapeando a su
/// motor: un catálogo viejo se sigue leyendo, sólo se avisa del renombrado.
#[test]
fn los_directorios_legacy_siguen_mapeando_a_su_motor() {
    let casos = [
        ("llama", EngineId::TurboLlama),
        ("llama-official", EngineId::LlamaServer),
    ];
    for (dir, esperado) in casos {
        assert_eq!(EngineId::from_dir_name(dir), Some(esperado), "dir={dir}");
    }
}

/// Los ids canónicos son los del catálogo, en el orden de la leyenda.
#[test]
fn los_ids_canonicos_son_los_de_adr_004() {
    assert_eq!(
        ropero_core::CANONICAL_IDS,
        &[
            "llama-server",
            "turbo-llama",
            "mlx-serve",
            "fastflowllm",
            "tensorrt-llm",
            "vllm",
            "airllm",
            "strata",
            "ignis",
            "basalt",
            "colibri",
        ]
    );
}
