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
pub fn validate_ok_line(files: usize, base: &Path) -> String {
    format!("OK: {} modelo(s) validados en {}", files, base.display())
}

/// La línea de resumen de `ropero validate` cuando algo falla.
pub fn validate_failed_line(base: &Path) -> String {
    format!("validación fallida en {}", base.display())
}
