//! La salida de los comandos, como **texto puro**.
//!
//! La CLI imprime; el core decide **qué** se imprime. Aquí vive el formato
//! de las filas del catálogo, de las rutas resueltas y de los niveles: son
//! funciones puras (entra un modelo, sale un `String`), y por eso pueden
//! vivir en el dominio sin tocar el disco.
//!
//! La razón es la regla de la CLI (doc B §3.3): *la lógica no vive aquí*.
//! Un `match` que hace trabajo de negocio en la CLI es un bug de
//! arquitectura; la CLI sólo despacha e imprime.

use std::path::Path;

use crate::ModelSpec;
use crate::catalog::Catalog;
use crate::paths::ResolvedPaths;

/// Una fila del catálogo: el modelo y si está retirado.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogRow {
    pub spec: ModelSpec,
    pub fired: bool,
}

/// Las filas del catálogo, con el formato de `ropero list`.
///
/// Un modelo por fichero de origen (un alias no es una fila nueva) y, si
/// `all`, también los modelos retirados.
pub fn list_rows(cat: &Catalog, all: bool) -> Vec<CatalogRow> {
    crate::catalog::catalog_rows(cat, all)
        .into_iter()
        .map(|(spec, fired)| CatalogRow {
            spec: spec.clone(),
            fired,
        })
        .collect()
}

/// La línea de una fila del catálogo, tal y como la imprime `ropero list`.
pub fn list_row_text(spec: &ModelSpec, fired: bool) -> String {
    let mut extra = String::new();
    if let Some(a) = &spec.model.alias {
        extra.push_str(&format!("alias={} ", a));
    }
    if fired {
        extra.push_str("retirado ");
    }
    format!(
        "  {:<16} engine={:<13} format={:<12} ctx={:<7} {}{}",
        spec.model.name,
        spec.model.engine.as_str(),
        spec.model.format.as_str(),
        spec.model.context,
        extra,
        crate::catalog::model_description(spec)
    )
}

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

/// El mensaje de error cuando un token de `ropero args` no tiene forma de
/// `--<nivel>`, `--<flag>` ni `--<opcion>=<valor>`.
///
/// El mensaje dice **qué hacer** (P-13).
pub fn invalid_token_message(token: &str) -> String {
    format!(
        "se esperaba --<nivel>|--<flag>|--<opt>=<v> (recibido: '{}')",
        token
    )
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

/// El aviso de nivel repetido: gana el último.
///
/// Cuando el usuario pide dos niveles distintos, el último manda; el aviso
/// dice cuál es el que manda.
pub fn repeated_level_notice(previous: &str, current: &str) -> String {
    format!("aviso: nivel repetido: '{previous}' → '{current}' (gana el último)")
}

/// El mensaje de error cuando el argv no se puede montar, con la pista de
/// dónde está la ayuda.
pub fn argv_error_message(error: &str) -> String {
    format!("error: {error}\n  (ropero args --help → uso de args)")
}

/// La línea de resumen de `ropero validate` cuando algo falla.
pub fn validate_failed_line(base: &Path) -> String {
    format!("validación fallida en {}", base.display())
}
