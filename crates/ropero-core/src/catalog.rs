//! Catálogo de modelos: dominio puro.
//!
//! Un modelo es un fichero TOML en `models/<engine>/*.toml` (activo) o
//! `models/<engine>/fired/*.toml` (retirado). Aquí sólo se *parsea* el TOML y
//! se *materializan* los alias como filas extra.
//!
//! El recorrido del disco (listar carpetas, leer ficheros) no vive aquí: es
//! E/S y pertenece a la capa de presentación (`ropero-cli::discovery`), que
//! llama a [`parse_model`] y a [`build_catalog`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::Environment;
use crate::ModelSpec;

/// Catálogo materializado: modelos activos y retirados.
#[derive(Debug, Clone, PartialEq)]
pub struct Catalog {
    /// Modelos activos (con sus aliases materializados como filas extra).
    pub models: Vec<ModelSpec>,
    /// Modelos retirados (`models/<engine>/fired/`).
    pub fired: Vec<ModelSpec>,
}

/// Error al parsear el TOML de un modelo.
#[derive(Debug)]
pub enum ModelError {
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    /// `[model].engine` no es ni un id canónico ni un alias legacy conocido.
    UnknownEngine {
        path: PathBuf,
        source: crate::UnknownEngine,
    },
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::Parse { path, source } => {
                write!(f, "error de parseo en {}: {}", path.display(), source)
            }
            ModelError::UnknownEngine { path, source } => {
                write!(f, "error de parseo en {}: {}", path.display(), source)
            }
        }
    }
}

impl std::error::Error for ModelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ModelError::Parse { source, .. } => Some(source),
            ModelError::UnknownEngine { source, .. } => Some(source),
        }
    }
}

/// Error al construir el catálogo (choque de nombres o de aliases).
#[derive(Debug)]
pub enum CatalogError {
    AliasConflict { name: String },
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatalogError::AliasConflict { name } => {
                write!(f, "choque de alias: '{}'", name)
            }
        }
    }
}

impl std::error::Error for CatalogError {}

/// Parsea el TOML de un modelo.
///
/// `source` es la ruta del fichero de origen: sólo se usa para los mensajes
/// de error y para [`ModelSpec::source`]. No se lee ningún fichero aquí: el
/// texto llega como `&str`.
///
/// Un `[model].engine` escrito con un alias legacy se canonicaliza al id
/// canónico y deja un aviso en [`ModelSpec::notices`] (doc B §4.2). Un id
/// que no es ni canónico ni alias conocido es un error de parseo.
pub fn parse_model(toml_text: &str, source: &Path) -> Result<ModelSpec, ModelError> {
    let (value, notices) = canonicalize_engine(toml_text, source)?;
    let mut spec: ModelSpec =
        value
            .try_into()
            .map_err(|err: toml::de::Error| ModelError::Parse {
                path: source.to_path_buf(),
                source: err,
            })?;
    spec.source = source.to_path_buf();
    spec.notices = notices;
    // El nombre real se fija aquí: después de materializar un alias la fila
    // pasa a llevar `model.name` = alias, pero el nombre real sigue siendo
    // el del TOML de origen.
    spec.real_name = Some(spec.model.name.clone());
    Ok(spec)
}

/// Canonicaliza un alias legacy de motor en el TOML, antes de deserializar.
///
/// Devuelve el TOML (como `toml::Value`) con `[model].engine` ya canónico y
/// los avisos del parseo. El TOML se deserializa en dos pasos porque el
/// aviso tiene que salir de la canonicalización: el enum `EngineId` sólo
/// acepta los ids canónicos, y el alias se acepta con un aviso, no en
/// silencio.
fn canonicalize_engine(
    toml_text: &str,
    source: &Path,
) -> Result<(toml::Value, Vec<String>), ModelError> {
    let mut value: toml::Value = toml::from_str(toml_text).map_err(|err| ModelError::Parse {
        path: source.to_path_buf(),
        source: err,
    })?;
    let mut notices = Vec::new();
    let Some(engine) = value
        .get("model")
        .and_then(|model| model.get("engine"))
        .and_then(|engine| engine.as_str())
    else {
        return Ok((value, notices));
    };
    let engine = engine.to_string();
    match crate::EngineId::from_str_legacy(&engine) {
        Ok((canonical, aviso)) => {
            if let Some(aviso) = aviso {
                notices.push(aviso);
            }
            if let Some(model) = value.get_mut("model").and_then(|m| m.as_table_mut())
                && let Some(slot) = model.get_mut("engine")
            {
                *slot = toml::Value::String(canonical.as_str().to_string());
            }
        }
        Err(unknown) => {
            return Err(ModelError::UnknownEngine {
                path: source.to_path_buf(),
                source: unknown,
            });
        }
    }
    Ok((value, notices))
}

/// Construye el catálogo a partir de los modelos ya parseados.
///
/// Materializa los aliases como filas extra (una por alias) y detecta
/// choques: dos modelos con el mismo nombre, o un alias que pise el nombre
/// de otro modelo. Activos y retirados se comprueban por separado.
pub fn build_catalog(
    active: Vec<ModelSpec>,
    fired: Vec<ModelSpec>,
) -> Result<Catalog, CatalogError> {
    Ok(Catalog {
        models: materialize_aliases(active)?,
        fired: materialize_aliases(fired)?,
    })
}

fn materialize_aliases(list: Vec<ModelSpec>) -> Result<Vec<ModelSpec>, CatalogError> {
    // Choque entre nombres reales.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for spec in &list {
        if !seen.insert(spec.model.name.clone()) {
            return Err(CatalogError::AliasConflict {
                name: spec.model.name.clone(),
            });
        }
    }

    let mut out: Vec<ModelSpec> = Vec::with_capacity(list.len() * 2);
    for spec in list {
        let alias = spec.model.alias.clone();
        out.push(spec.clone());
        let Some(alias) = alias else { continue };
        if alias == spec.model.name {
            // alias igual al propio nombre: se ignora
            continue;
        }
        if !seen.insert(alias.clone()) {
            return Err(CatalogError::AliasConflict { name: alias });
        }
        let mut alias_spec = spec.clone();
        alias_spec.model.name = alias;
        alias_spec.model.alias = None;
        out.push(alias_spec);
    }

    Ok(out)
}

/// Busca un modelo por nombre (activos primero, luego retirados).
pub fn find_spec<'a>(cat: &'a Catalog, name: &str) -> Option<&'a ModelSpec> {
    cat.models
        .iter()
        .find(|m| m.model.name == name)
        .or_else(|| cat.fired.iter().find(|m| m.model.name == name))
}

/// Rutas candidatas de la plantilla del catálogo (`models/_template.toml`).
///
/// `base` es el directorio de modelos ya resuelto por la capa de
/// presentación (`ropero_core::models_base`): aquí no se pregunta al disco
/// si existe.
pub fn template_candidates(root: &Path, base: &Path) -> Vec<PathBuf> {
    vec![
        base.join("_template.toml"),
        root.join("_template.toml"),
        root.join("ropero-code/models/_template.toml"),
    ]
}

/// Filas del catálogo: un modelo por fichero de origen (un alias no es una
/// fila nueva) y, si `all`, también los modelos retirados.
pub fn catalog_rows(cat: &Catalog, all: bool) -> Vec<(&ModelSpec, bool)> {
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut rows = Vec::new();
    for (list, fired) in [(&cat.models, false), (&cat.fired, true)] {
        if fired && !all {
            continue;
        }
        for spec in list {
            if seen.insert(spec.source.clone()) {
                rows.push((spec, fired));
            }
        }
    }
    rows
}

/// Descripción corta de un modelo: el tamaño o el nombre de su artefacto.
pub fn model_description(spec: &ModelSpec) -> String {
    let desc = spec
        .download
        .as_ref()
        .map(|d| {
            if d.size.is_empty() {
                d.file.clone()
            } else {
                d.size.clone()
            }
        })
        .unwrap_or_default();
    if !desc.is_empty() {
        return desc;
    }
    spec.files
        .gguf
        .as_ref()
        .map(|g| g.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            if spec.model.path.is_empty() {
                "-".to_string()
            } else {
                spec.model.path.clone()
            }
        })
}

/// Nombre real del modelo: el de su TOML de origen.
///
/// En la fila de un alias, `model.name` es el alias; el nombre real es el del
/// modelo original (es el directorio donde viven sus artefactos).
pub fn real_name(spec: &ModelSpec) -> String {
    spec.real_name
        .clone()
        .unwrap_or_else(|| spec.model.name.clone())
}

/// Convierte un nombre en el `SLUG` que usan las variables de entorno
/// (`ROPERO_<SLUG>_CTX`, `ROPERO_<SLUG>_GGUF`).
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut prev = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_uppercase());
            prev = false;
        } else if !prev && !out.is_empty() {
            out.push('_');
            prev = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// Contexto efectivo: `ROPERO_<SLUG>_CTX` si está definida, si no el del
/// TOML.
///
/// El override llega en el entorno ya capturado: el dominio no consulta el
/// entorno real.
pub fn effective_context(spec: &ModelSpec, env: &Environment) -> u32 {
    let var = format!("ROPERO_{}_CTX", slug(&real_name(spec)));
    if let Some(v) = env.get(&var)
        && let Ok(n) = v.parse::<u32>()
    {
        return n;
    }
    spec.model.context
}
