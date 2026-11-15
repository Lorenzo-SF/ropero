use std::path::Path;

use ropero_core::{CatalogError, ModelError, ModelSpec, build_catalog, parse_model, real_name};

const TOML: &str = r#"
[model]
name = "groot"
engine = "llama-server"
context = 1024

[files]
gguf = "g.gguf"

[args]
base = ["--ctx", "${ctx}"]
"#;

fn spec(toml: &str) -> ModelSpec {
    parse_model(toml, Path::new("models/llama/groot.toml")).expect("parse model")
}

#[test]
fn active_and_fired_are_separate_lists() {
    let cat = build_catalog(vec![spec(TOML)], vec![spec(&TOML.replace("groot", "old"))]).unwrap();
    assert_eq!(cat.models.len(), 1);
    assert_eq!(cat.fired.len(), 1);
    assert_eq!(cat.models[0].model.name, "groot");
    assert_eq!(cat.fired[0].model.name, "old");
}

#[test]
fn alias_materialized() {
    let toml = TOML.replace("name = \"groot\"", "name = \"groot\"\nalias = \"g\"");
    let cat = build_catalog(vec![spec(&toml)], vec![]).unwrap();
    assert_eq!(cat.models.len(), 2);
    assert!(cat.models.iter().any(|m| m.model.name == "g"));
    // la fila del alias conserva el nombre real del modelo
    let alias = cat.models.iter().find(|m| m.model.name == "g").unwrap();
    assert_eq!(real_name(alias), "groot");
    assert_eq!(real_name(&cat.models[0]), "groot");
}

#[test]
fn alias_same_name_ignored() {
    let toml = TOML.replace("name = \"groot\"", "name = \"groot\"\nalias = \"groot\"");
    let cat = build_catalog(vec![spec(&toml)], vec![]).unwrap();
    assert_eq!(cat.models.len(), 1);
}

/// La fila de un alias se reconoce; la de su TOML de origen, no.
///
/// `es_alias` es la **única** definición de "fila de alias": el resto del
/// core decide con ella.
#[test]
fn la_fila_de_alias_se_reconoce() {
    let toml = TOML.replace("name = \"groot\"", "name = \"groot\"\nalias = \"g\"");
    let cat = build_catalog(vec![spec(&toml)], vec![]).unwrap();
    let original = cat
        .models
        .iter()
        .find(|m| m.model.name == "groot")
        .expect("debe estar el modelo");
    let alias = cat
        .models
        .iter()
        .find(|m| m.model.name == "g")
        .expect("debe estar el alias");
    assert!(!ropero_core::es_alias(original));
    assert!(ropero_core::es_alias(alias));
}

/// Un alias no es un modelo más: la cuenta es por nombre real.
#[test]
fn la_cuenta_de_modelos_no_duplica_el_alias() {
    let toml = TOML.replace("name = \"groot\"", "name = \"groot\"\nalias = \"g\"");
    let cat = build_catalog(vec![spec(&toml)], vec![]).unwrap();
    assert_eq!(ropero_core::cuenta_unicos(cat.models.iter()), 1);
}

#[test]
fn alias_clash_errors() {
    let a = spec(&TOML.replace("groot", "a"));
    let b = spec(&TOML.replace("name = \"groot\"", "name = \"b\"\nalias = \"a\""));
    let res = build_catalog(vec![a, b], vec![]);
    assert!(matches!(res, Err(CatalogError::AliasConflict { .. })));
}

#[test]
fn duplicate_name_errors() {
    let a = spec(&TOML.replace("groot", "a"));
    let b = spec(&TOML.replace("groot", "a"));
    let res = build_catalog(vec![a, b], vec![]);
    assert!(matches!(res, Err(CatalogError::AliasConflict { .. })));
}

#[test]
fn broken_toml_is_an_error() {
    let res = parse_model("not toml [[[", Path::new("models/llama/bad.toml"));
    assert!(matches!(res, Err(ModelError::Parse { .. })));
}

#[test]
fn source_is_kept() {
    let s = spec(TOML);
    assert_eq!(s.source, Path::new("models/llama/groot.toml"));
    assert_eq!(s.real_name.as_deref(), Some("groot"));
}

#[test]
fn models_base_prefers_models_dir() {
    let root = Path::new("/catalogo");
    // Con `models/` presente, la base es `models/`; sin él, la raíz es la
    // base. La existencia la decide la capa de presentación: aquí sólo se
    // decide el nombre.
    assert_eq!(ropero_core::models_base(root, true), root.join("models"));
    assert_eq!(ropero_core::models_base(root, false), root);
}

#[test]
fn legacy_engine_folder_mapped() {
    let (engine, aviso) = ropero_core::EngineId::from_str_legacy("llama").unwrap();
    assert_eq!(engine, ropero_core::EngineId::LlamaServer);
    assert!(aviso.is_some());
    assert!(ropero_core::EngineId::from_str_legacy("no-existe").is_err());
}
