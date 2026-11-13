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
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::Parse { path, source } => {
                write!(f, "error de parseo en {}: {}", path.display(), source)
            }
        }
    }
}

impl std::error::Error for ModelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ModelError::Parse { source, .. } => Some(source),
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
pub fn parse_model(toml_text: &str, source: &Path) -> Result<ModelSpec, ModelError> {
    let mut spec: ModelSpec = toml::from_str(toml_text).map_err(|err| ModelError::Parse {
        path: source.to_path_buf(),
        source: err,
    })?;
    spec.source = source.to_path_buf();
    // El nombre real se fija aquí: después de materializar un alias la fila
    // pasa a llevar `model.name` = alias, pero el nombre real sigue siendo
    // el del TOML de origen.
    spec.real_name = Some(spec.model.name.clone());
    Ok(spec)
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
pub fn template_candidates(root: &Path) -> Vec<PathBuf> {
    let base = models_base(root);
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
pub fn effective_context(spec: &ModelSpec) -> u32 {
    let var = format!("ROPERO_{}_CTX", slug(&real_name(spec)));
    if let Ok(v) = std::env::var(&var)
        && let Ok(n) = v.parse::<u32>()
    {
        return n;
    }
    spec.model.context
}

/// Directorio de modelos: `<root>/models` si existe, si no `<root>`.
pub fn models_base(root: &Path) -> PathBuf {
    if root.join("models").is_dir() {
        root.join("models")
    } else {
        root.to_path_buf()
    }
}

/// Expande `~` y convierte en absoluta una ruta relativa.
pub fn expand_path(raw: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    let expanded = if raw == "~" {
        home.clone()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        format!("{}/{}", home, rest)
    } else {
        raw.to_string()
    };
    let p = PathBuf::from(expanded);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    }
}

/// Ruta de una variable de entorno, con valor por defecto relativo a `$HOME`.
pub fn env_path(var: &str, default_rel: &str) -> PathBuf {
    let raw = std::env::var(var).unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/{}", home, default_rel.trim_start_matches("~/"))
    });
    expand_path(&raw)
}
