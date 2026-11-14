//! El descubrimiento del catálogo: la **frontera de E/S**.
//!
//! Aquí se lee el disco y el entorno del proceso; el resto del programa
//! trabaja con lo que sale de aquí. El dominio puro (`ropero-core`) recibe
//! todo como dato: el catálogo parseado, los textos leídos y el entorno
//! capturado ([`ropero_core::Environment`]).
//!
//! Aquí se descubren **las dos** cosas que viven en la raíz: el catálogo de
//! `models/` y los manifiestos de motor de `engines/`.
use ropero_core::{Catalog, EngineManifest, Environment, ModelSpec, parse_engine_manifest};
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
    /// Los manifiestos de motor leídos de `engines/`.
    pub engines: Vec<EngineManifest>,
}
fn fail(e: impl std::fmt::Display) -> i32 {
    ropero_core::err_line(&format!("error: {}", e));
    1
}
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
    for notice in &scan.notices {
        ropero_core::err_line(notice);
    }
    Ok(scan)
}
fn scan_root(root: &Path, env: &Environment) -> Result<Scan, String> {
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
        // Una carpeta que no corresponde a ningún motor es un **problema** del
        // catálogo (R-DATA-12), no el fin del escaneo: se acumula y se sigue.
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
    // Los manifiestos de motor viven en `engines/`, junto al catálogo. Un
    // manifiesto roto es un **problema** que se acumula (R-ENG-5).
    let engines = scan_engines(&root.join(ropero_core::ENGINES_DIR_NAME), &mut problems);
    Ok(Scan {
        catalog,
        problems,
        notices,
        texts,
        files,
        base,
        root: root.to_path_buf(),
        env: env.clone(),
        engines,
    })
}

/// Los ficheros `.toml` de un directorio, ordenados.
///
/// El orden es explícito: el del sistema de ficheros no se supone estable.
fn tomls(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("error de E/S en {}: {}", dir.display(), e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|s| s == "toml"))
        .collect();
    files.sort();
    Ok(files)
}

/// Lee y parsea los manifiestos de motor de `dir`.
///
/// Un `.toml` que no se puede parsear es un problema que se acumula: un
/// motor roto no puede impedir que los otros se vean (R-ENG-5).
fn scan_engines(dir: &Path, problems: &mut Vec<String>) -> Vec<EngineManifest> {
    let mut manifests = Vec::new();
    let Ok(files) = tomls(dir) else {
        return manifests;
    };
    for path in files {
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                problems.push(format!("error de E/S en {}: {}", path.display(), e));
                continue;
            }
        };
        match parse_engine_manifest(&text, &path) {
            Ok(manifiesto) => manifests.push(manifiesto),
            Err(e) => problems.push(e.to_string()),
        }
    }
    manifests
}
fn collect(
    dir: &Path,
    out: &mut Vec<ModelSpec>,
    problems: &mut Vec<String>,
    notices: &mut Vec<String>,
    texts: &mut BTreeMap<PathBuf, String>,
) -> Result<usize, String> {
    let files = tomls(dir)?;
    let mut n = 0;
    for path in files {
        let text = fs::read_to_string(&path)
            .map_err(|e| format!("error de E/S en {}: {}", path.display(), e))?;
        n += 1;
        texts.insert(path.clone(), text.clone());
        ropero_core::parse_collect(&text, &path, out, problems, notices);
    }
    Ok(n)
}
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

fn accept(p: &Path) -> Option<PathBuf> {
    let models = p.join("models");
    let known = models.is_dir()
        || models.join("_template.toml").is_file()
        || models.join("turbo-llama").is_dir();
    let base = ropero_core::MODELS_BASE_NAMES
        .iter()
        .any(|n| p.join(n).exists());
    if known || base {
        return Some(p.to_path_buf());
    }
    None
}
fn has_subdir(p: &Path) -> bool {
    p.read_dir()
        .map(|mut d| d.any(|e| e.map(|x| x.path().is_dir()).unwrap_or(false)))
        .unwrap_or(false)
}
