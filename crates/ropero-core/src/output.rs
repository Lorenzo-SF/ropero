//! La salida de los comandos, como **texto puro**.
//!
//! La CLI imprime; el core decide **qué** se imprime. Aquí viven el formato
//! de las rutas resueltas y de los niveles: son funciones puras (entra un
//! modelo, sale un `String`), y por eso pueden vivir en el dominio sin tocar
//! el disco.
//!
//! La razón es la regla de la CLI (doc B §3.3): *la lógica no vive aquí*.
//! Un `match` que hace trabajo de negocio en la CLI es un bug de
//! arquitectura; la CLI sólo despacha e imprime.
//!
//! El listado de modelos vive en [`crate::listing`] (es el nuevo formato
//! con grupos y colores) y los textos de la ayuda, en [`crate::surface`].

use std::path::Path;

use crate::ModelSpec;
use crate::paths::ResolvedPaths;

/// Las líneas de `ropero path`: las rutas resueltas, una por línea.
///
/// El formato es `CLAVE=valor`; una ruta que no existe no sale (quien pide
/// `path` pregunta por lo que hay).
///
/// Esta salida es una **exportación**, no una tabla: se pasa a un `eval` o
/// se lee con un `grep`, así que la clave va pegada al `=`. Es la única
/// excepción al sistema de formato (doc CLI v2 §2).
pub fn path_lines(paths: &ResolvedPaths) -> Vec<String> {
    let mut out = Vec::new();
    for (label, value) in [
        ("GGUF", &paths.gguf),
        ("MTP", &paths.mtp),
        ("MMPROJ", &paths.mmproj),
        ("CHAT_TEMPLATE", &paths.chat_template),
        ("MODEL_PATH", &paths.model_path),
    ] {
        if let Some(v) = value {
            out.push(format!("{}={}", label, v));
        }
    }
    out.push(format!("LOG={}", paths.log));
    out
}

/// Las líneas de `ropero levels`: los niveles declarados, uno por línea.
pub fn level_lines(spec: &ModelSpec) -> Vec<String> {
    spec.levels.list.clone()
}

/// El primer token de `ropero args` que no tiene forma de `--<nivel>`,
/// `--<flag>` ni `--<opcion>=<valor>`.
///
/// PURA: no lee disco, no consulta el entorno. La CLI decide qué hacer con
/// el token (que es un error de uso).
pub fn first_invalid_token(tokens: &[String]) -> Option<&str> {
    tokens
        .iter()
        .find(|tok| !tok.starts_with("--"))
        .map(|tok| tok.as_str())
}

/// La línea de resumen de `ropero validate` cuando todo está bien.
///
/// El prefijo es el del vocabulario de estados (`ok`), el mismo que usa el
/// doctor (doc CLI v2 §2).
pub fn validate_ok_line(files: usize, base: &Path) -> String {
    format!("ok: {} modelo(s) validados en {}", files, base.display())
}

/// La línea de resumen de `ropero validate` cuando algo falla.
pub fn validate_failed_line(base: &Path) -> String {
    format!("error: validación fallida en {}", base.display())
}

/// El tamaño declarado de un artefacto, con la **forma única** de la salida.
///
/// El tamaño de una descarga es siempre **aproximado** (por eso el `~`) y
/// siempre con **dos decimales**: así la columna del tamaño tiene una sola
/// forma y se alinea (doc CLI v2 §2).
///
/// `None` cuando el tamaño **no se sabe**: el catálogo no lo declara, o
/// declara un cero (un tamaño de cero no es un tamaño, es un "no lo sé").
pub fn tamano_declarado(size: &str) -> Option<String> {
    let limpio = size.trim();
    if limpio.is_empty() {
        return None;
    }
    if valor_tamano(limpio).is_some_and(|v| v == 0.0) {
        return None;
    }
    Some(forma_tamano(limpio))
}

/// La forma única de un tamaño: `~N.NN UNIDAD`.
///
/// Lo que no se puede leer como un tamaño sale tal cual: es mejor un texto
/// raro que un tamaño inventado.
pub fn forma_tamano(size: &str) -> String {
    let limpio = size.trim();
    let (numero, unidad) = match limpio.split_once(' ') {
        Some((n, u)) => (n.trim(), u.trim()),
        None => (limpio, ""),
    };
    let numero = numero.trim_start_matches('~').replace(',', ".");
    match numero.parse::<f64>() {
        Ok(valor) if !unidad.is_empty() => format!("~{valor:.2} {unidad}"),
        _ => limpio.to_string(),
    }
}

/// El número de un tamaño declarado: `"~3.5 GB"` → `3.5`.
fn valor_tamano(size: &str) -> Option<f64> {
    let numero = size
        .trim()
        .trim_start_matches('~')
        .split_whitespace()
        .next()?;
    numero.replace(',', ".").parse::<f64>().ok()
}
