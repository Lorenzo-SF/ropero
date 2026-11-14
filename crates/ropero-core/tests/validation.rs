use std::collections::{BTreeMap, BTreeSet};

use ropero_core::{ValidationContext, ValidationRule, parse_model, validate};

fn spec(source: &str) -> ropero_core::ModelSpec {
    parse_model(source, std::path::Path::new("models/llama/groot.toml"))
        .expect("el TOML de prueba debe parsear")
}

fn valid() -> ropero_core::ModelSpec {
    spec(
        r#"
[model]
name = "groot"
engine = "llama-server"
context = 32768

[files]
gguf = "groot.gguf"

[download]
repo = "owner/groot"
file = "groot.gguf"
"#,
    )
}

fn ctx() -> ValidationContext {
    ValidationContext {
        source: "models/llama/groot.toml".to_string(),
        source_text: None,
        seen_names: BTreeSet::new(),
    }
}

fn rule(errors: &[ropero_core::ValidationError]) -> ValidationRule {
    errors.first().expect("debe haber un error").rule()
}

#[test]
fn regla_01_rechaza_name_no_kebab_case() {
    let mut model = valid();
    model.model.name = "Groot.1".to_string();
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::NameKebabCase);
}

#[test]
fn regla_01_admite_guion_bajo_legacy() {
    let mut model = valid();
    model.model.name = "qwen_next".to_string();
    assert!(
        !validate(&model, &ctx())
            .iter()
            .any(|error| error.rule() == ValidationRule::NameKebabCase)
    );
}

#[test]
fn regla_02_rechaza_name_duplicado() {
    let mut model = valid();
    model.model.name = "otro".to_string();
    let mut context = ctx();
    context.seen_names.insert("otro".to_string());
    let errors = validate(&model, &context);
    assert_eq!(rule(&errors), ValidationRule::NameUnique);
}

#[test]
fn regla_03_rechaza_name_distinto_del_fichero() {
    let mut model = valid();
    model.model.name = "groot-trt".to_string();
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::NameMatchesFile);
}

#[test]
fn regla_04_rechaza_engine_desconocido() {
    let source = r#"
[model]
name = "groot"
engine = "desconocido"
context = 32768
"#;
    assert!(parse_model(source, std::path::Path::new("models/llama/groot.toml")).is_err());
}

#[test]
fn regla_05_requiere_path_para_safetensors() {
    let mut model = valid();
    model.model.format = ropero_core::ModelFormat::Safetensors;
    model.files.gguf = None;
    model.download = None;
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::SafetensorsRequiresPath);
}

#[test]
fn regla_06_prohibe_path_sin_safetensors() {
    let mut model = valid();
    model.model.path = "groot".to_string();
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::NonSafetensorsForbidsPath);
}

#[test]
fn regla_06_permite_path_en_q4nx_y_strata() {
    for format in [
        ropero_core::ModelFormat::Q4nx,
        ropero_core::ModelFormat::Strata,
    ] {
        let mut model = valid();
        model.model.format = format;
        model.model.path = "family/package".to_string();
        model.files.gguf = None;
        model.download = None;
        assert!(
            !validate(&model, &ctx())
                .iter()
                .any(|error| error.rule() == ValidationRule::NonSafetensorsForbidsPath)
        );
    }
}

#[test]
fn regla_07_requiere_files_gguf_para_gguf() {
    let mut model = valid();
    model.files.gguf = None;
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::GgufRequiresFile);
}

#[test]
fn regla_08_download_sin_build_requiere_repo_y_file() {
    let mut model = valid();
    model.download.as_mut().expect("download").repo.clear();
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::DownloadRequiresRepo);
}

#[test]
fn regla_09_download_y_build_requieren_eleccion_de_uso() {
    let mut model = valid();
    model.build = Some(ropero_core::Build {
        repo: "owner/groot".to_string(),
        ..Default::default()
    });
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::DownloadAndBuild);
    assert_eq!(errors[0].exit_code(), 2);
}

#[test]
fn regla_09_permite_download_auxiliar_con_build() {
    let mut model = valid();
    model.download.as_mut().expect("download").repo.clear();
    model.download.as_mut().expect("download").file.clear();
    model.download.as_mut().expect("download").name.clear();
    model.build = Some(ropero_core::Build {
        repo: "owner/groot".to_string(),
        ..Default::default()
    });
    assert!(
        !validate(&model, &ctx())
            .iter()
            .any(|error| error.rule() == ValidationRule::DownloadAndBuild)
    );
}

#[test]
fn regla_10_rechaza_placeholder_de_fichero_no_declarado() {
    let mut model = valid();
    model.args.base = vec!["--mmproj".to_string(), "${mmproj}".to_string()];
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::UndeclaredFilePlaceholder);
}

#[test]
fn regla_11_rechaza_default_de_level_fuera_de_list() {
    let mut model = valid();
    model.levels.list = vec!["low".to_string()];
    model.levels.default = Some("high".to_string());
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::LevelDefaultMissing);
}

#[test]
fn regla_12_rechaza_alias_a_level_inexistente() {
    let mut model = valid();
    model.levels.list = vec!["low".to_string()];
    model.aliases.insert("eco".to_string(), "high".to_string());
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::AliasLevelMissing);
}

#[test]
fn regla_13_rechaza_default_de_option_fuera_de_list() {
    let mut model = valid();
    model.options.list = vec!["temp".to_string()];
    model.options.default = Some("top-p".to_string());
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::OptionDefaultMissing);
}

#[test]
fn regla_14_rechaza_requires_de_file_inexistente() {
    let mut model = valid();
    model.args.flag.insert(
        "vision".to_string(),
        ropero_core::FlagArg {
            base: Vec::new(),
            requires: vec!["mmproj".to_string()],
        },
    );
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::FlagRequiresMissingFile);
}

#[test]
fn regla_15_rechaza_min_ram_gb_no_positivo() {
    let mut model = valid();
    model.limits.min_ram_gb = Some(-1.0);
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::MinRamMustBePositive);
}

/// `min_ram_gb = 0` también es un error: la regla es **estrictamente
/// mayor que 0** (doc B §4.1.1 / R-DATA-9), como en la v1
/// (`ropero-code/bin/python/modelconf.py:1563`, que rechaza `<= 0`).
#[test]
fn regla_15_rechaza_min_ram_gb_cero() {
    let mut model = valid();
    model.limits.min_ram_gb = Some(0.0);
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::MinRamMustBePositive);
    assert!(errors[0].message().contains("mayor que 0"));
}

/// Un `min_ram_gb` positivo **no** es un error.
#[test]
fn regla_15_admite_min_ram_gb_positivo() {
    let mut model = valid();
    model.limits.min_ram_gb = Some(4.0);
    assert!(
        !validate(&model, &ctx())
            .iter()
            .any(|error| error.rule() == ValidationRule::MinRamMustBePositive)
    );
}

/// `min_ram_gb` **no declarado** no es un error: el campo es opcional (como
/// en la v1, que sólo comprueba la clave cuando está presente:
/// `ropero-code/bin/python/modelconf.py:1560`).
#[test]
fn regla_15_admite_min_ram_gb_ausente() {
    let model = valid();
    assert!(model.limits.min_ram_gb.is_none());
    assert!(
        !validate(&model, &ctx())
            .iter()
            .any(|error| error.rule() == ValidationRule::MinRamMustBePositive)
    );
}

#[test]
fn regla_16_rechaza_context_invalido() {
    let mut model = valid();
    model.model.context = 33;
    let errors = validate(&model, &ctx());
    assert_eq!(rule(&errors), ValidationRule::ContextInvalid);
}

#[test]
fn regla_16_permite_context_omitido() {
    let mut model = valid();
    model.model.context = 0;
    assert!(
        !validate(&model, &ctx())
            .iter()
            .any(|error| error.rule() == ValidationRule::ContextInvalid)
    );
}

#[test]
fn acepta_un_toml_valido() {
    assert!(validate(&valid(), &ctx()).is_empty());
}

#[test]
fn valida_catalogo_completo_y_detecta_unicidad_global() {
    let mut first = valid();
    first.source = std::path::PathBuf::from("models/llama/groot.toml");
    let mut second = valid();
    second.source = std::path::PathBuf::from("models/llama-official/groot.toml");
    let catalog = ropero_core::Catalog {
        models: vec![first],
        fired: vec![second],
    };
    let errors = ropero_core::validate_catalog(&catalog, &BTreeMap::new());
    assert!(errors.iter().any(|error| {
        error.rule() == ValidationRule::NameUnique && error.path().ends_with("groot.toml")
    }));
}

#[test]
fn el_error_incluye_ruta_linea_y_mensaje() {
    let source = r#"[model]
name = "groot"
engine = "llama-server"
context = 33

[files]
gguf = "groot.gguf"
"#;
    let model = spec(source);
    let mut context = ctx();
    context.source_text = Some(source.to_string());
    let errors = validate(&model, &context);
    assert_eq!(errors.len(), 1);
    let error = &errors[0];
    assert_eq!(error.path(), "models/llama/groot.toml");
    assert_eq!(error.line(), Some(4));
    assert!(error.message().contains("context"));
}
