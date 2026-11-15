//! El JSON de la batería: escapar, construir y leer, sin dependencias.
//!
//! Tres cosas son JSON y no se puede añadir un crate: el cuerpo del `POST` al
//! motor, el resultado de cada test guardado en disco, y la salida de
//! `ropero test --json`. Así que el escape y el parser viven aquí.
//!
//! Las reglas de escape que se aplican (y se prueban en `tests/json.rs`):
//!
//! - la comilla y la barra invertida se escapan;
//! - `\n`, `\t`, `\r` se escriben como escape corto;
//! - cualquier carácter de control (< 0x20) se escribe `\u00xx`;
//! - los acentos, la `ñ`, la raya y la barra `/` **no** se escapan: son
//!   caracteres normales de un texto en español;
//! - al leer, `\u` se resuelve y la cadena se devuelve tal cual.

use std::fmt;

/// Un objeto JSON: campos en el orden en que se escribieron.
///
/// `Eq` no se deriva aquí: un valor puede ser un número, y los números
/// de coma flotante no tienen igualdad exacta. Es lo que impide derivarlo
/// también en [`Json`].
#[derive(Debug, Clone, PartialEq)]
pub struct Objeto {
    campos: Vec<(String, Json)>,
}

impl Objeto {
    /// El valor de una clave, o `None` si la clave no está: una clave que no
    /// existe **no** inventa un valor.
    pub fn get(&self, clave: &str) -> Option<&Json> {
        self.campos
            .iter()
            .find(|(escrita, _)| escrita == clave)
            .map(|(_, valor)| valor)
    }

    /// Los campos, en orden.
    pub fn campos(&self) -> &[(String, Json)] {
        &self.campos
    }
}

/// Un valor JSON.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Objeto),
}

impl Json {
    /// Un objeto a partir de sus campos, en el orden dado.
    pub fn objeto(campos: Vec<(String, Json)>) -> Json {
        Json::Obj(Objeto { campos })
    }

    /// La clave de un objeto; `None` si esto no es un objeto.
    pub fn get(&self, clave: &str) -> Option<&Json> {
        match self {
            Json::Obj(objeto) => objeto.get(clave),
            _ => None,
        }
    }

    /// Un array; `None` si esto no es un array.
    pub fn arr(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(texto) => Some(texto),
            _ => None,
        }
    }

    pub fn as_num(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// `Some(())` si el valor es `null`: así se comprueba que la clave
    /// **existe** y vale `null`, sin confundirlo con una clave ausente.
    pub fn as_null(&self) -> Option<()> {
        match self {
            Json::Null => Some(()),
            _ => None,
        }
    }

    /// El valor escrito como JSON.
    pub fn a_texto(&self) -> String {
        match self {
            Json::Null => "null".to_string(),
            Json::Bool(v) => {
                if *v {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Json::Num(n) => numero(*n),
            Json::Str(texto) => cadena(texto),
            Json::Arr(items) => {
                let mut out = String::from("[");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&item.a_texto());
                }
                out.push(']');
                out
            }
            Json::Obj(objeto) => {
                let mut out = String::from("{");
                for (i, (clave, valor)) in objeto.campos.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&cadena(clave));
                    out.push(':');
                    out.push_str(&valor.a_texto());
                }
                out.push('}');
                out
            }
        }
    }
}

/// El error del parser: dice **qué** falta, no dónde se calló.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub mensaje: String,
}

impl JsonError {
    fn nuevo(mensaje: String) -> JsonError {
        JsonError { mensaje }
    }
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.mensaje)
    }
}

impl std::error::Error for JsonError {}

/// Escapa un texto para meterlo dentro de una cadena JSON.
pub fn escape(texto: &str) -> String {
    let mut out = String::with_capacity(texto.len() + 2);
    for c in texto.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Un texto entre comillas, escapado.
pub fn cadena(texto: &str) -> String {
    format!("\"{}\"", escape(texto))
}

/// El cuerpo del `POST` de inferencia: un mensaje de usuario con el prompt.
///
/// El prompt llega tal cual: los acentos, las comillas y los saltos de línea
/// del archivo del test se escapan, no se pierden.
pub fn cuerpo_inferencia(modelo: &str, prompt: &str, stream: bool) -> String {
    Json::objeto(vec![
        ("model".to_string(), Json::Str(modelo.to_string())),
        (
            "messages".to_string(),
            Json::Arr(vec![Json::objeto(vec![
                ("role".to_string(), Json::Str("user".to_string())),
                ("content".to_string(), Json::Str(prompt.to_string())),
            ])]),
        ),
        ("stream".to_string(), Json::Bool(stream)),
    ])
    .a_texto()
}

/// Lee un valor JSON. Si algo no cuadra, se dice qué falta.
pub fn parse(texto: &str) -> Result<Json, JsonError> {
    if texto.trim().is_empty() {
        return Err(JsonError::nuevo(
            "el texto está vacío: no hay JSON que leer".to_string(),
        ));
    }
    let bytes = texto.as_bytes();
    let mut pos = 0usize;
    let valor = leer_valor(bytes, &mut pos)?;
    saltar(bytes, &mut pos);
    if pos < bytes.len() {
        return Err(JsonError::nuevo(format!(
            "sobra texto después del JSON: el resto (`{}`) no se lee",
            &texto[pos..]
        )));
    }
    Ok(valor)
}

/// El error de un JSON que se acabó antes de cerrarse.
fn sin_cierre() -> JsonError {
    JsonError::nuevo(
        "el JSON se acabó antes de cerrarse: falta la comilla de cierre o la llave `}`".to_string(),
    )
}

fn saltar(bytes: &[u8], pos: &mut usize) {
    while let Some(b) = bytes.get(*pos) {
        if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
            *pos += 1;
        } else {
            break;
        }
    }
}

fn leer_valor(bytes: &[u8], pos: &mut usize) -> Result<Json, JsonError> {
    saltar(bytes, pos);
    match bytes.get(*pos) {
        None => Err(sin_cierre()),
        Some(b'{') => leer_objeto(bytes, pos),
        Some(b'[') => leer_array(bytes, pos),
        Some(b'"') => Ok(Json::Str(leer_cadena(bytes, pos)?)),
        Some(b't') => leer_literal(bytes, pos, "true", Json::Bool(true)),
        Some(b'f') => leer_literal(bytes, pos, "false", Json::Bool(false)),
        Some(b'n') => leer_literal(bytes, pos, "null", Json::Null),
        Some(b'0'..=b'9') | Some(b'-') => leer_numero(bytes, pos),
        Some(b) => Err(JsonError::nuevo(format!(
            "esto no es JSON: empieza con `{}`. Se esperaba un valor JSON",
            *b as char
        ))),
    }
}

fn leer_literal(
    bytes: &[u8],
    pos: &mut usize,
    palabra: &str,
    valor: Json,
) -> Result<Json, JsonError> {
    if bytes.get(*pos..*pos + palabra.len()) == Some(palabra.as_bytes()) {
        *pos += palabra.len();
        return Ok(valor);
    }
    Err(sin_cierre())
}

fn leer_objeto(bytes: &[u8], pos: &mut usize) -> Result<Json, JsonError> {
    *pos += 1;
    let mut campos: Vec<(String, Json)> = Vec::new();
    loop {
        saltar(bytes, pos);
        match bytes.get(*pos) {
            None => return Err(sin_cierre()),
            Some(b'}') => {
                *pos += 1;
                return Ok(Json::Obj(Objeto { campos }));
            }
            Some(b'"') => {
                let clave = leer_cadena(bytes, pos)?;
                saltar(bytes, pos);
                if bytes.get(*pos) != Some(&b':') {
                    return Err(JsonError::nuevo(format!(
                        "el campo `{clave}` no trae los dos puntos que separan la clave del valor"
                    )));
                }
                *pos += 1;
                let valor = leer_valor(bytes, pos)?;
                campos.push((clave, valor));
                saltar(bytes, pos);
                match bytes.get(*pos) {
                    Some(b',') => {
                        *pos += 1;
                        saltar(bytes, pos);
                        if bytes.get(*pos) == Some(&b'}') {
                            return Err(sin_cierre());
                        }
                        if bytes.get(*pos) != Some(&b'"') {
                            return Err(sin_cierre());
                        }
                    }
                    Some(b'}') => {
                        *pos += 1;
                        return Ok(Json::Obj(Objeto { campos }));
                    }
                    _ => return Err(sin_cierre()),
                }
            }
            Some(_) => {
                return Err(JsonError::nuevo(
                    "un objeto JSON empieza su clave con una comilla".to_string(),
                ));
            }
        }
    }
}

fn leer_array(bytes: &[u8], pos: &mut usize) -> Result<Json, JsonError> {
    *pos += 1;
    let mut items: Vec<Json> = Vec::new();
    loop {
        saltar(bytes, pos);
        match bytes.get(*pos) {
            None => return Err(sin_cierre()),
            Some(b']') => {
                *pos += 1;
                return Ok(Json::Arr(items));
            }
            Some(_) => {
                let valor = leer_valor(bytes, pos)?;
                items.push(valor);
                saltar(bytes, pos);
                match bytes.get(*pos) {
                    Some(b',') => {
                        *pos += 1;
                        saltar(bytes, pos);
                        if bytes.get(*pos) == Some(&b']') {
                            return Err(sin_cierre());
                        }
                    }
                    Some(b']') => {
                        *pos += 1;
                        return Ok(Json::Arr(items));
                    }
                    _ => return Err(sin_cierre()),
                }
            }
        }
    }
}

fn leer_numero(bytes: &[u8], pos: &mut usize) -> Result<Json, JsonError> {
    let inicio = *pos;
    while let Some(b) = bytes.get(*pos) {
        if matches!(b, b'+' | b'-' | b'.' | b'e' | b'E' | b'0'..=b'9') {
            *pos += 1;
        } else {
            break;
        }
    }
    let Some(texto) = str::from_utf8(bytes.get(inicio..*pos).unwrap_or(&[])).ok() else {
        return Err(sin_cierre());
    };
    let valor = match texto.parse::<f64>() {
        Ok(v) => v,
        Err(_) => {
            return Err(JsonError::nuevo(format!("`{texto}` no es un número JSON")));
        }
    };
    Ok(Json::Num(valor))
}

fn leer_cadena(bytes: &[u8], pos: &mut usize) -> Result<String, JsonError> {
    *pos += 1;
    let mut out = String::new();
    loop {
        match bytes.get(*pos) {
            None => {
                return Err(JsonError::nuevo(
                    "hay una cadena sin cerrar: falta la comilla de cierre".to_string(),
                ));
            }
            Some(b'"') => {
                *pos += 1;
                return Ok(out);
            }
            Some(b'\\') => {
                *pos += 1;
                match bytes.get(*pos) {
                    None => {
                        return Err(JsonError::nuevo(
                            "hay una cadena sin cerrar: falta la comilla de cierre".to_string(),
                        ));
                    }
                    Some(b'n') => {
                        out.push('\n');
                        *pos += 1;
                    }
                    Some(b't') => {
                        out.push('\t');
                        *pos += 1;
                    }
                    Some(b'r') => {
                        out.push('\r');
                        *pos += 1;
                    }
                    Some(b'\\') => {
                        out.push('\\');
                        *pos += 1;
                    }
                    Some(b'/') => {
                        out.push('/');
                        *pos += 1;
                    }
                    Some(b'"') => {
                        out.push('"');
                        *pos += 1;
                    }
                    Some(b'b') => {
                        out.push('\u{8}');
                        *pos += 1;
                    }
                    Some(b'f') => {
                        out.push('\u{c}');
                        *pos += 1;
                    }
                    Some(b'u') => {
                        *pos += 1;
                        out.push(escape_unicode(bytes, pos)?);
                    }
                    Some(c) => {
                        return Err(JsonError::nuevo(format!(
                            "el escape `\\{}` no existe en JSON",
                            *c as char
                        )));
                    }
                }
            }
            Some(_) => {
                let mut fin = *pos;
                while !matches!(bytes.get(fin), Some(b'"') | Some(b'\\') | None) {
                    fin += 1;
                }
                let Some(segmento) = str::from_utf8(bytes.get(*pos..fin).unwrap_or(&[])).ok()
                else {
                    return Err(sin_cierre());
                };
                out.push_str(segmento);
                *pos = fin;
            }
        }
    }
}

/// Los cuatro dígitos hexadecimales de un escape `\u`.
fn escape_unicode(bytes: &[u8], pos: &mut usize) -> Result<char, JsonError> {
    let Some(cuatro) = bytes.get(*pos..*pos + 4) else {
        return Err(sin_cierre());
    };
    let mut codigo = 0u32;
    for b in cuatro {
        let Some(digito) = (*b as char).to_digit(16) else {
            return Err(JsonError::nuevo(format!(
                "el escape `\\u` lleva un carácter que no es hexadecimal: `{}`",
                *b as char
            )));
        };
        codigo = codigo * 16 + digito;
    }
    *pos += 4;
    match char::from_u32(codigo) {
        Some(c) => Ok(c),
        None => Err(JsonError::nuevo(format!(
            "el escape `\\u{codigo:04x}` no es un carácter"
        ))),
    }
}

/// Un número escrito como JSON. Un número sin parte decimal se escribe entero;
/// uno que no es finito se escribe `null` (no se inventa un valor).
fn numero(n: f64) -> String {
    if !n.is_finite() {
        return "null".to_string();
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}
