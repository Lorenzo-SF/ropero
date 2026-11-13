use ropero_core::{Catalog, ModelSpec};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
pub struct Scan {
    pub catalog: Catalog,
    pub problems: Vec<String>,
    pub files: usize,
    pub base: PathBuf,
    pub root: PathBuf,
    pub texts: BTreeMap<PathBuf, String>,
}
fn fail(e: impl std::fmt::Display) -> i32 {
    eprintln!("error: {}", e);
    1
}
pub fn scan() -> Result<Scan, i32> {
    let root = find_model_root().map_err(fail)?;
    scan_root(&root).map_err(fail)
}
fn scan_root(root: &Path) -> Result<Scan, String> {
    let base = ropero_core::models_base(root);
    let mut active: Vec<ModelSpec> = Vec::new();
    let mut fired: Vec<ModelSpec> = Vec::new();
    let mut problems: Vec<String> = Vec::new();
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
        if ropero_core::EngineId::from_str_legacy(&folder).is_err() {
            return Err(format!("motor desconocido '{folder}' en {}", dir.display()));
        }
        files += collect(&dir, &mut active, &mut problems, &mut texts)?;
        let retired = dir.join("fired");
        if retired.is_dir() {
            files += collect(&retired, &mut fired, &mut problems, &mut texts)?;
        }
    }
    let catalog = ropero_core::build_catalog(active, fired).map_err(|e| e.to_string())?;
    Ok(Scan {
        catalog,
        problems,
        texts,
        files,
        base,
        root: root.to_path_buf(),
    })
}
fn collect(
    dir: &Path,
    out: &mut Vec<ModelSpec>,
    problems: &mut Vec<String>,
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
            Ok(spec) => out.push(spec),
            Err(e) => {
                let msg = format!("{}: {}", path.display(), e);
                eprintln!("aviso: no se pudo parsear {}", msg);
                problems.push(msg);
            }
        }
    }
    Ok(n)
}
pub fn find_model_root() -> Result<PathBuf, String> {
    for var in ["ROPERO_MODELS_DIR", "ROPERO_ROOT"] {
        if let Ok(val) = std::env::var(var) {
            let p = PathBuf::from(val);
            if let Some(root) = accept(&p) {
                return Ok(root);
            }
            // Una variable de entorno puede apuntar al directorio de trabajo.
            if p.is_dir() && has_subdir(&p) {
                return Ok(p);
            }
        }
    }
    let mut chain: Vec<PathBuf> = Vec::new();
    if let Ok(cur) = std::env::current_dir() {
        chain.extend(cur.ancestors().map(Path::to_path_buf));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        chain.extend(parent.ancestors().map(Path::to_path_buf));
    }
    for dir in chain {
        if let Some(root) = accept(&dir) {
            return Ok(root);
        }
    }
    Err("no se encontró el directorio de modelos".to_string())
}
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
    ["_template.toml", "llama", "airllm", "mlx", "strata"]
        .iter()
        .any(|n| p.join(n).exists())
}
fn has_subdir(p: &Path) -> bool {
    p.read_dir()
        .map(|mut d| d.any(|e| e.map(|x| x.path().is_dir()).unwrap_or(false)))
        .unwrap_or(false)
}
