//! Los errores de parseo llevan **ruta y línea** (doc B §4.1.1).
//!
//! Una clave que el esquema no conoce (`deny_unknown_fields`) o un motor que
//! no existe matan el TOML al deserializar: el modelo nunca llega a
//! `validate()`. Por eso sus reglas (`UnknownKey` y `EngineInvalid`) se
//! construyen al clasificar el error de parseo, y el error que ve el usuario
//! lleva `ruta:línea`.

use std::path::Path;

use ropero_core::{ModelError, ValidationRule, parse_error, parse_model};

/// Un TOML con una clave que el esquema no conoce.
///
/// La clave culpable está en la **línea 8** (`gguf_desconocido`).
const TOML_CLAVE_DESCONOCIDA: &str = r#"[model]
name = "groot"
engine = "llama-server"
context = 1024

[files]
gguf = "g.gguf"
gguf_desconocido = "otro.gguf"
"#;

/// Un TOML con un motor que no existe.
///
/// La clave culpable (`engine`) está en la **línea 3**.
const TOML_MOTOR_DESCONOCIDO: &str = r#"[model]
name = "groot"
engine = "motor-que-no-existe"

[files]
gguf = "g.gguf"
"#;

fn parse(toml: &str) -> ModelError {
    parse_model(toml, Path::new("models/llama/groot.toml")).expect_err("el TOML es inválido")
}

#[test]
fn la_clave_desconocida_falla_con_la_regla_unknown_key() {
    let error = parse(TOML_CLAVE_DESCONOCIDA);
    let violation = parse_error(&error);
    assert_eq!(violation.rule(), ValidationRule::UnknownKey);
    assert_eq!(violation.key(), "gguf_desconocido");
}

#[test]
fn el_motor_desconocido_falla_con_la_regla_engine_invalid() {
    let error = parse(TOML_MOTOR_DESCONOCIDO);
    let violation = parse_error(&error);
    assert_eq!(violation.rule(), ValidationRule::EngineInvalid);
    assert_eq!(violation.key(), "model.engine");
}

#[test]
fn el_error_de_clave_desconocida_lleva_ruta_y_linea() {
    let error = parse(TOML_CLAVE_DESCONOCIDA);
    let violation = parse_error(&error);
    assert_eq!(violation.path(), "models/llama/groot.toml");
    // La línea apunta al campo real: `gguf_desconocido` está en la 8.
    assert_eq!(violation.line(), Some(8));
}

#[test]
fn el_error_de_motor_desconocido_lleva_ruta_y_linea() {
    let error = parse(TOML_MOTOR_DESCONOCIDO);
    let violation = parse_error(&error);
    assert_eq!(violation.path(), "models/llama/groot.toml");
    // La línea apunta al campo real: `engine` está en la 3.
    assert_eq!(violation.line(), Some(3));
}

#[test]
fn el_mensaje_del_error_de_parseo_incluye_la_linea() {
    let error = parse(TOML_CLAVE_DESCONOCIDA);
    let texto = error.to_string();
    assert!(
        texto.contains("models/llama/groot.toml:8"),
        "el error no lleva ruta:línea: {texto}"
    );
    // Y el texto que ve el usuario va con la línea.
    let violation = parse_error(&error);
    let linea = format!("{}:{}", violation.path(), violation.line().unwrap_or(0));
    assert_eq!(linea, "models/llama/groot.toml:8");
}

#[test]
fn el_error_de_motor_desconocido_tambien_incluye_la_linea() {
    let error = parse(TOML_MOTOR_DESCONOCIDO);
    let texto = error.to_string();
    assert!(
        texto.contains("models/llama/groot.toml:3"),
        "el error no lleva ruta:línea: {texto}"
    );
}

/// La línea se calcula **sobre el texto fuente**, no sobre el índice del
/// error: dos TOML con la clave culpable en sitios distintos dan líneas
/// distintas.
#[test]
fn la_linea_se_calcula_sobre_el_texto_fuente() {
    let con_espacio = format!("\n\n\n{TOML_CLAVE_DESCONOCIDA}");
    let error = parse(&con_espacio);
    let violation = parse_error(&error);
    // Tres líneas en blanco por delante: la clave culpable pasa de 8 a 11.
    assert_eq!(violation.line(), Some(11));
}

/// Un TOML con la clave culpable en la **última** línea: la cuenta no se
/// descuadra por un salto final.
#[test]
fn la_linea_es_la_ultima_cuando_el_campo_está_al_final() {
    let toml = "[model]\nname = \"groot\"\nengine = \"llama-server\"\n[files]\ngguf = \"g.gguf\"\ngguf_desconocido = \"x\"\n";
    let error = parse(toml);
    let violation = parse_error(&error);
    assert_eq!(violation.line(), Some(6));
}
