//! El escaneo del catálogo: la **frontera de E/S** del disco.
//!
//! Aquí se lee el disco: el catálogo de `models/` y los manifiestos de
//! motor de `engines/`. Lo que sale es **dato** (el catálogo parseado, los
//! textos, los avisos), y el dominio puro lo usa sin saber de dónde vino.
//!
//! ## Por qué vive aquí y no en la CLI
//!
//! Porque la CLI es el **dispatcher** (R-ARQ-1): si la lectura del disco
//! viviera allí, el CLI dejaría de ser un dispatcher para ser una capa de
//! E/S, y su tamaño crecería sin límite. `ropero-runtime` es la capa de
//! E/S: ahí caben el escaneo, los procesos y la captura de la máquina.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ropero_core::{
    Catalog, EngineId, EngineManifest, Environment, ModelSpec, parse_engine_manifest,
};

/// El resultado de escanear el catálogo.
pub struct Scan {
    /// El catálogo de modelos.
    pub catalog: Catalog,
    /// Los problemas del escaneo (acumulados: no abortan).
    pub problems: Vec<String>,
    /// Los avisos del parseo (no son errores).
    pub notices: Vec<String>,
    /// Cuántos ficheros TOML se leyeron.
    pub files: usize,
    /// El directorio de modelos.
    pub base: PathBuf,
    /// La raíz del catálogo.
    pub root: PathBuf,
    /// Los textos leídos, por ruta.
    pub texts: BTreeMap<PathBuf, String>,
    /// Los manifiestos de motor leídos de `engines/`.
    pub engines: Vec<EngineManifest>,
}

/// Escanea la raíz del catálogo.
///
/// `env` es el entorno ya capturado: el runtime no consulta el entorno del
/// proceso (el core tampoco puede).
pub fn scan(root: &Path) -> Result<Scan, String> {
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
        if EngineId::from_dir_name(&folder).is_none() {
            let msg = format!("motor desconocido '{folder}' en {}", dir.display());
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

/// Lee los TOML de un directorio.
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

/// La raíz del catálogo: donde están `models/` y `engines/`.
///
/// `env` es el entorno ya capturado y `exe_ancestors` la cadena de
/// directorios del ejecutable: la búsqueda es la del dominio
/// ([`ropero_core::find_root`]), y sólo la *pregunta* al disco es de aquí.
pub fn find_root(env: &Environment, exe_ancestors: &[PathBuf]) -> Result<PathBuf, String> {
    ropero_core::find_root(env, exe_ancestors, accept, has_subdir)
        .ok_or_else(|| "no se encontró el directorio de modelos".to_string())
}

/// Un directorio es la raíz del catálogo si tiene modelos o bases.
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

/// Si un directorio tiene subdirectorios.
fn has_subdir(p: &Path) -> bool {
    p.read_dir()
        .map(|mut d| d.any(|e| e.map(|x| x.path().is_dir()).unwrap_or(false)))
        .unwrap_or(false)
}

/// La cadena de directorios del ejecutable actual.
pub fn exe_ancestors() -> Vec<PathBuf> {
    let mut chain: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        chain.extend(parent.ancestors().map(Path::to_path_buf));
    }
    chain
}

/// Busca un ejecutable en el `PATH`.
pub fn which(binario: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binario))
        .find(|candidato| es_ejecutable(candidato))
}

/// Si una ruta es un fichero ejecutable.
pub fn es_ejecutable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Si un motor está instalado: su `detect` lo encuentra.
///
/// El `detect` del manifiesto dice **cómo** se sabe: un binario en el
/// `PATH`, o una ruta donde puede estar instalado.
pub fn engine_installed(manifest: &EngineManifest) -> bool {
    manifest
        .detect
        .binaries
        .iter()
        .any(|binario| which(binario).is_some())
        || manifest
            .detect
            .paths
            .iter()
            .any(|ruta| Path::new(ruta).exists())
}

/// Si el modelo está descargado: sus ficheros `[files]` existen.
///
/// Se resuelven con `resolve_paths` (el dominio): el core decide **qué**
/// ruta mirar, y aquí sólo se pregunta al disco.
pub fn model_downloaded(spec: &ModelSpec, env: &Environment) -> bool {
    let paths = ropero_core::resolve_paths(spec, env);
    let existe = |ruta: Option<&str>| ruta.is_some_and(|r| Path::new(r).exists());
    let principal = existe(paths.gguf.as_deref()) || existe(paths.model_path.as_deref());
    let auxiliares = existe(paths.mtp.as_deref())
        || existe(paths.mmproj.as_deref())
        || existe(paths.chat_template.as_deref());
    // Un modelo sin artefactos (p. ej. sólo `[build]`) se considera
    // descargado si no tiene nada que descargar.
    principal || auxiliares || sin_artefactos(spec)
}

/// Si el modelo no declara ningún artefacto que pueda faltar.
fn sin_artefactos(spec: &ModelSpec) -> bool {
    spec.files.gguf.is_none()
        && spec.files.mtp.is_none()
        && spec.files.mmproj.is_none()
        && spec.files.chat_template.is_none()
        && spec.model.path.is_empty()
}

/// Si el binario `ropero` está en el `PATH`.
///
/// Se busca el nombre del ejecutable actual: si su nombre está en el
/// `PATH`, entonces se puede invocar como `ropero`.
pub fn ropero_en_path() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(candidato) = exe.file_name() else {
        return false;
    };
    which(candidato.to_string_lossy().as_ref()).is_some()
}

/// Si una ruta existe (para la plantilla del catálogo).
pub fn existe(path: &Path) -> bool {
    path.is_file()
}

/// El detector real: pregunta al `PATH` y al disco.
///
/// Implementa [`ropero_core::snapshot::Detect`], que es la frontera entre
/// el dominio (que decide qué significa) y el disco (que responde).
pub struct Detector<'a> {
    /// El entorno ya capturado (para resolver las rutas de los modelos).
    pub env: &'a Environment,
}

impl ropero_core::snapshot::Detect for Detector<'_> {
    fn engine_installed(&self, manifest: &EngineManifest) -> bool {
        engine_installed(manifest)
    }

    fn model_downloaded(&self, spec: &ModelSpec) -> bool {
        model_downloaded(spec, self.env)
    }
}
