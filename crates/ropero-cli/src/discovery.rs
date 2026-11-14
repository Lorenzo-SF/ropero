//! El descubrimiento del catálogo: la **frontera de E/S**.
//!
//! Aquí se lee el disco y el entorno del proceso; el resto del programa
//! trabaja con lo que sale de aquí. El dominio puro (`ropero-core`) recibe
//! todo como dato: el catálogo parseado, los textos leídos y el entorno
//! capturado ([`ropero_core::Environment`]).
use ropero_core::{Catalog, Environment, ModelSpec};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Scan {
    pub catalog: Catalog,
    pub problems: Vec<String>,
    /// Avisos del parseo (p. ej. la canonicalización de un alias legacy de
    /// motor). No son errores: no cambian el código de salida.
    pub notices: Vec<String>,
    pub files: usize,
    pub base: PathBuf,
    pub root: PathBuf,
    pub texts: BTreeMap<PathBuf, String>,
    /// El entorno del proceso, ya capturado: lo que el dominio puro necesita
    /// en lugar de leer el entorno real.
    pub env: Environment,
}
fn fail(e: impl std::fmt::Display) -> i32 {
    ropero_core::err_line(&format!("error: {}", e));
    1
}
/// Captura el entorno del proceso: las variables, el `$HOME` y el directorio
/// de trabajo. Es la frontera: aquí se lee el entorno real y el resultado se
/// pasa al dominio puro como dato.
pub fn capture_environment() -> Environment {
    Environment::new(
        std::env::vars().collect(),
        std::env::var("HOME").unwrap_or_default(),
        std::env::current_dir().unwrap_or_default(),
    )
}
pub fn scan() -> Result<Scan, i32> {
    let env = capture_environment();
    let root = find_model_root(&env).map_err(fail)?;
    let scan = scan_root(&root, &env).map_err(fail)?;
    // Los avisos del parseo se enseñan siempre: un alias legacy se
    // canonicaliza con aviso, nunca en silencio (doc B §4.2).
    for notice in &scan.notices {
        ropero_core::err_line(notice);
    }
    Ok(scan)
}
fn scan_root(root: &Path, env: &Environment) -> Result<Scan, String> {
    // La decisión de qué directorio es la base es de E/S: la capa de
    // presentación pregunta al disco y el dominio puro recibe el nombre.
    let base = ropero_core::models_base(root, root.join("models").is_dir());
    let mut active: Vec<ModelSpec> = Vec::new();
    let mut fired: Vec<ModelSpec> = Vec::new();
    let mut problems: Vec<String> = Vec::new();
    let mut notices: Vec<String> = Vec::new();
    let mut texts = BTreeMap::new();
    let mut files = 0;
    let mut dirs = fs::read_dir(&base)
        .map_err(|e| format!("error de E/S en {}: {}", base.display(), e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect::<Vec<PathBuf>>();
    dirs.sort();
    for dir in dirs {
        let folder = dir.file_name().unwrap_or_default().to_string_lossy();
        // Las carpetas de `models/` se nombran como el motor, pero no siempre
        // igual (`llama/` → `llama-server`, `flm/` → `fastflowlm`).
        //
        // Una carpeta que no corresponde a ningún motor es un **problema**
        // del catálogo (R-DATA-12), no el fin del escaneo: se acumula y se
        // sigue, como hace la v1
        // (`ropero-code/bin/python/modelconf.py:1648`, que acumula el
        // problema y sigue). Antes esta carpeta abortaba el `scan()`
        // entero y **ningún** modelo se validaba.
        if ropero_core::EngineId::from_dir_name(&folder).is_none() {
            let msg = format!("motor desconocido '{folder}' en {}", dir.display());
            ropero_core::err_line(&format!("aviso: {msg}"));
            problems.push(msg);
            continue;
        }
        files += collect(&dir, &mut active, &mut problems, &mut notices, &mut texts)?;
        let retired = dir.join("fired");
        if retired.is_dir() {
            files += collect(
                &retired,
                &mut fired,
                &mut problems,
                &mut notices,
                &mut texts,
            )?;
        }
    }
    let catalog = ropero_core::build_catalog(active, fired).map_err(|e| e.to_string())?;
    Ok(Scan {
        catalog,
        problems,
        notices,
        texts,
        files,
        base,
        root: root.to_path_buf(),
        env: env.clone(),
    })
}
fn collect(
    dir: &Path,
    out: &mut Vec<ModelSpec>,
    problems: &mut Vec<String>,
    notices: &mut Vec<String>,
    texts: &mut BTreeMap<PathBuf, String>,
) -> Result<usize, String> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("error de E/S en {}: {}", dir.display(), e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("toml"))
        .collect();
    files.sort();
    let mut n = 0;
    for path in files {
        let text = fs::read_to_string(&path)
            .map_err(|e| format!("error de E/S en {}: {}", path.display(), e))?;
        n += 1;
        texts.insert(path.clone(), text.clone());
        match ropero_core::parse_model(&text, &path) {
            Ok(spec) => {
                notices.extend(spec.notices.iter().cloned());
                out.push(spec);
            }
            Err(e) => {
                let msg = format!("{}: {}", path.display(), e);
                ropero_core::err_line(&format!("aviso: no se pudo parsear {}", msg));
                problems.push(msg);
            }
        }
    }
    Ok(n)
}
/// Busca la raíz del catálogo.
///
/// Primero las variables que lo apuntan (`ROPERO_MODELS_DIR`,
/// `ROPERO_ROOT`), luego la cadena de directorios desde el directorio de
/// trabajo y desde el propio ejecutable. La **decisión** (qué se prueba, en
/// qué orden) es del dominio; aquí sólo se responde qué existe en disco.
pub fn find_model_root(env: &Environment) -> Result<PathBuf, String> {
    let mut chain: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        chain.extend(parent.ancestors().map(Path::to_path_buf));
    }
    ropero_core::find_root(env, &chain, accept, has_subdir)
        .ok_or_else(|| "no se encontró el directorio de modelos".to_string())
}
/// Dice si un directorio es la raíz del catálogo y, si lo es, cuál.
fn accept(p: &Path) -> Option<PathBuf> {
    let models = p.join("models");
    let known =
        models.is_dir() || models.join("_template.toml").is_file() || models.join("llama").is_dir();
    if known || is_models_base(p) {
        return Some(p.to_path_buf());
    }
    if p.join("ropero-code/models").is_dir() {
        return Some(p.join("ropero-code"));
    }
    None
}
fn is_models_base(p: &Path) -> bool {
    ropero_core::MODELS_BASE_NAMES
        .iter()
        .any(|n| p.join(n).exists())
}
fn has_subdir(p: &Path) -> bool {
    p.read_dir()
        .map(|mut d| d.any(|e| e.map(|x| x.path().is_dir()).unwrap_or(false)))
        .unwrap_or(false)
}
