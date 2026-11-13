//! Tests del puerto pineado del modelo (`[model].port`).
//!
//! `Port` es el newtype del dominio: en el TOML es un entero a secas
//! (`port = 9997`), no una tabla.

use std::path::Path;

use ropero_core::{ModelSpec, Port, parse_model};

const TOML: &str = r#"
[model]
name = "modelo"
engine = "llama-server"
context = 4096
port = 9997

[files]
gguf = "modelo.gguf"
"#;

fn spec(toml: &str) -> ModelSpec {
    parse_model(toml, Path::new("models/llama/modelo.toml")).expect("parsear el modelo")
}

#[test]
fn el_puerto_se_lee_como_entero() {
    let spec = spec(TOML);
    assert_eq!(spec.model.port, Some(Port(9997)));
    assert_eq!(spec.model.port.map(|p| p.value()), Some(9997));
}

#[test]
fn el_puerto_es_opcional() {
    let toml = TOML.replace("port = 9997\n", "");
    let spec = spec(&toml);
    assert_eq!(spec.model.port, None);
}

#[test]
fn el_puerto_se_muestra_como_numero() {
    assert_eq!(Port(9997).to_string(), "9997");
}

#[test]
fn el_puerto_se_puede_construir_desde_un_entero() {
    assert_eq!(Port::from(8080), Port(8080));
    assert_eq!(Port(8080).into_inner(), 8080);
}
