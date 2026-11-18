//! La sonda de readiness real (doc B §3.3.6).
//!
//! ## La jerarquía de niveles
//!
//! ```text
//! 1. Tcp       — el puerto acepta conexiones.
//! 2. Http      — el endpoint de salud responde 200.
//! 3. Inference — el motor responde a una inferencia mínima.
//! ```text
//!
//! Se prueban en orden y **el primero que pasa es suficiente**: para un
//! modelo que carga rápido, esperar a la inferencia es tirar el tiempo
//! del usuario; para uno que carga despacio, el puerto abierto antes de
//! tiempo es mentir.
//!
//! ## La prueba de `curl` es la misma que la de la v1
//!
//! `GET /v1/models` con `Authorization: Bearer <clave>`: es lo que
//! responden los motores que hablan el protocolo OpenAI, y es lo que
//! permite además saber **qué** modelo está sirviendo.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::launch::Probe;

/// La sonda que habla con el puerto.
pub struct NetProbe {
    /// El tiempo que se espera en cada conexión.
    pub timeout: Duration,

    /// La clave de API, si la hay.
    pub api_key: Option<String>,
}

impl Default for NetProbe {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(3),
            api_key: None,
        }
    }
}

impl Probe for NetProbe {
    fn ready(&self, port: u16, model: &str) -> bool {
        // 1. TCP: si el puerto no acepta, no hay motor.
        if !tcp_ready(port, self.timeout) {
            return false;
        }
        // 2. HTTP: la salud del motor.
        if http_ready(port, "/health", self.timeout) {
            return true;
        }
        // 3. /v1/models: el protocolo OpenAI. Además dice qué modelo
        //    sirve, así que se comprueba que sea el nuestro.
        match http_models(port, self.timeout, self.api_key.as_deref()) {
            Some(ids) => ids.is_empty() || ids.iter().any(|id| id == model),
            // Si no responde a /v1/models pero el puerto está abierto, se
            // acepta: hay motores que no exponen ese endpoint.
            None => true,
        }
    }
}

/// La salud HTTP de Basalt sube antes que el modelo; `/health` informa del
/// progreso de carga y no basta como readiness. Esperamos al estado `loaded`
/// publicado por `/v1/models`.
pub struct BasaltProbe {
    pub timeout: Duration,
    pub model_id: String,
}

impl Probe for BasaltProbe {
    fn ready(&self, port: u16, _model: &str) -> bool {
        if !tcp_ready(port, self.timeout) {
            return false;
        }
        let Some(body) = http_get(port, "/v1/models", self.timeout, None) else {
            return false;
        };
        let Ok(payload) = serde_json::from_str::<serde_json::Value>(&body) else {
            return false;
        };
        basalt_model_loaded(&payload, &self.model_id)
    }
}

fn basalt_model_loaded(payload: &serde_json::Value, model_id: &str) -> bool {
    payload
        .get("data")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|models| {
            models.iter().any(|entry| {
                entry.get("id").and_then(serde_json::Value::as_str) == Some(model_id)
                    && entry.get("status").and_then(serde_json::Value::as_str) == Some("loaded")
            })
        })
}

/// Si el puerto acepta una conexión TCP.
pub fn tcp_ready(port: u16, timeout: Duration) -> bool {
    let direccion = format!("127.0.0.1:{port}");
    let Ok(mut direcciones) = direccion.to_socket_addrs() else {
        return false;
    };
    let Some(destino) = direcciones.next() else {
        return false;
    };
    TcpStream::connect_timeout(&destino, timeout).is_ok()
}

/// Si un endpoint HTTP responde con algo (2xx o 3xx).
fn http_ready(port: u16, path: &str, timeout: Duration) -> bool {
    http_get(port, path, timeout, None).is_some()
}

/// Los ids que publica `GET /v1/models`.
pub fn http_models(port: u16, timeout: Duration, api_key: Option<&str>) -> Option<Vec<String>> {
    let cuerpo = http_get(port, "/v1/models", timeout, api_key)?;
    Some(parse_model_ids(&cuerpo))
}

/// Los ids de un JSON de `/v1/models`.
///
/// El JSON se busca a pelo: el motor puede serializarlo con espacios o
/// sin ellos, y con las claves en distinto orden. Lo que se busca es el
/// campo `"id"`, que es el único que importa.
pub fn parse_model_ids(cuerpo: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut resto = cuerpo;
    while let Some(pos) = resto.find("\"id\"") {
        let despues = &resto[pos + 4..];
        let Some(colon) = despues.find(':') else {
            break;
        };
        let valor = despues[colon + 1..].trim_start();
        let Some(quote) = valor.find('"') else {
            break;
        };
        let valor = &valor[quote + 1..];
        let Some(cierre) = valor.find('"') else {
            break;
        };
        ids.push(valor[..cierre].to_string());
        resto = &valor[cierre + 1..];
    }
    ids
}

/// Un `GET` HTTP mínimo, sin dependencias.
///
/// Devuelve el cuerpo de la respuesta si el servidor respondió.
fn http_get(port: u16, path: &str, timeout: Duration, api_key: Option<&str>) -> Option<String> {
    let mut stream = abre_peticion(port, "GET", path, None, timeout, api_key)?;
    let respuesta = lee_todo(&mut stream);
    let texto = String::from_utf8_lossy(&respuesta).into_owned();
    // El cuerpo va después de la línea en blanco.
    let cuerpo = texto.split_once("\r\n\r\n")?;
    Some(cuerpo.1.to_string())
}

/// Abre la conexión con el motor y le escribe la petición.
///
/// Es lo que comparten el `GET` de la sonda y el `POST` de la inferencia:
/// la cabecera `Authorization: Bearer` va **aquí**, en la petición HTTP, y
/// nunca en el `argv` de otro proceso ni en la salida del programa.
pub fn abre_peticion(
    port: u16,
    metodo: &str,
    path: &str,
    cuerpo: Option<&str>,
    timeout: Duration,
    api_key: Option<&str>,
) -> Option<TcpStream> {
    let direccion = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect(&direccion).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;

    let mut peticion = format!("{metodo} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
    if let Some(clave) = api_key {
        peticion.push_str(&format!("Authorization: Bearer {clave}\r\n"));
    }
    if let Some(cuerpo) = cuerpo {
        peticion.push_str("Content-Type: application/json\r\n");
        peticion.push_str(&format!("Content-Length: {}\r\n", cuerpo.len()));
    }
    peticion.push_str("Connection: close\r\n\r\n");
    if let Some(cuerpo) = cuerpo {
        peticion.push_str(cuerpo);
    }
    stream.write_all(peticion.as_bytes()).ok()?;
    Some(stream)
}

/// Lee la respuesta entera, hasta que el servidor cierra.
pub fn lee_todo(stream: &mut TcpStream) -> Vec<u8> {
    let mut respuesta = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                respuesta.extend_from_slice(&buffer[..n]);
                if respuesta.len() > 8_000_000 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    respuesta
}

#[cfg(test)]
mod tests {
    use super::basalt_model_loaded;

    #[test]
    fn basalt_solo_esta_listo_cuando_el_alias_esta_loaded() {
        let payload = serde_json::json!({"data": [
            {"id": "public-model", "status": "loading"}
        ]});
        assert!(!basalt_model_loaded(&payload, "public-model"));
        let payload = serde_json::json!({"data": [
            {"id": "public-model", "status": "loaded"}
        ]});
        assert!(basalt_model_loaded(&payload, "public-model"));
        assert!(!basalt_model_loaded(&payload, "other-model"));
    }
}
