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
    /// Error de configuración que sólo se detecta al aplicar overrides a los
    /// manifests (por ejemplo, rangos efectivos solapados).
    pub config_error: Option<ropero_core::config::ErrorConfig>,
}

/// Escanea la raíz del catálogo.
///
/// `env` es el entorno ya capturado: el runtime no consulta el entorno del
/// proceso (el core tampoco puede).
pub fn scan(root: &Path, env: &Environment) -> Result<Scan, String> {
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
    let mut candidates = BTreeMap::<EngineId, Vec<PathBuf>>::new();
    for dir in dirs {
        let folder = dir.file_name().unwrap_or_default().to_string_lossy();
        // Una carpeta que no corresponde a ningún motor es un **problema** del
        // catálogo (R-DATA-12), no el fin del escaneo: se acumula y se sigue.
        let Some(engine) = EngineId::from_dir_name(&folder) else {
            let msg = format!("motor desconocido '{folder}' en {}", dir.display());
            problems.push(msg);
            continue;
        };
        candidates.entry(engine).or_default().push(dir);
    }
    // Nombres antiguos de carpetas (`flm`, `mlx`, `tensorrt`) siguen siendo
    // válidos para instalaciones previas. Si conviven con el nombre canónico,
    // se escanea una sola carpeta: primero la canónica y, si está vacía, la
    // antigua. Así no aparecen modelos duplicados y una carpeta canónica
    // creada por el bootstrap no oculta un catálogo legado que aún tiene datos.
    let mut scanned = Vec::new();
    let mut scanned_paths = BTreeMap::<PathBuf, EngineId>::new();
    for (engine, mut engine_dirs) in candidates {
        engine_dirs.sort_by_key(|dir| {
            let canonical = dir.file_name().is_some_and(|name| name == engine.as_str());
            (!canonical, dir.clone())
        });
        let selected = engine_dirs
            .iter()
            .find(|dir| dir_has_model_toml(dir))
            .or_else(|| engine_dirs.first());
        if let Some(dir) = selected {
            let identity = dir.canonicalize().unwrap_or_else(|_| dir.clone());
            scanned_paths.insert(identity, engine);
            scanned.push((engine, dir.clone()));
        }
    }
    for (engine, dir) in scanned {
        files += collect_engine(
            engine,
            &dir,
            &mut active,
            &mut fired,
            &mut problems,
            &mut notices,
            &mut texts,
        )?;
    }
    let catalog = ropero_core::build_catalog(active, fired).map_err(|e| e.to_string())?;
    // Los manifiestos de motor viven en `engines/`, junto al catálogo. Un
    // manifiesto roto es un **problema** que se acumula (R-ENG-5).
    // Si el catálogo apunta al directorio `models/` (p. ej. override
    // ROPERO_MODELS_DIR=<raíz>/models), los motores pueden estar en la raíz
    // hermana `<raíz>/engines`.
    let engines_dir = if root.join(ropero_core::ENGINES_DIR_NAME).is_dir() {
        root.join(ropero_core::ENGINES_DIR_NAME)
    } else {
        ropero_core::config::raiz_del_catalogo(&base).join(ropero_core::ENGINES_DIR_NAME)
    };
    let mut engines = scan_engines(&engines_dir, &mut problems);
    for model in catalog.models.iter().chain(catalog.fired.iter()) {
        if let Some(engine) = engines
            .iter()
            .find(|engine| engine.id == model.model.engine)
            && !engine
                .formats
                .iter()
                .any(|format| format == model.model.format.as_str())
        {
            problems.push(format!(
                "el modelo '{}' usa formato '{}' que no declara el manifiesto de '{}' ({})",
                model.model.name,
                model.model.format.as_str(),
                engine.id.as_str(),
                model.source.display()
            ));
        }
    }
    let config = env.config_resuelta();
    let config_error = config.overlay_engines(&mut engines).err();
    Ok(Scan {
        catalog,
        problems,
        notices,
        texts,
        files,
        base,
        root: root.to_path_buf(),
        engines,
        config_error,
    })
}

fn dir_has_model_toml(dir: &Path) -> bool {
    fs::read_dir(dir).ok().is_some_and(|entries| {
        entries.flatten().any(|entry| {
            entry.path().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "toml")
        })
    })
}

fn collect_engine(
    engine: EngineId,
    dir: &Path,
    active: &mut Vec<ModelSpec>,
    fired: &mut Vec<ModelSpec>,
    problems: &mut Vec<String>,
    notices: &mut Vec<String>,
    texts: &mut BTreeMap<PathBuf, String>,
) -> Result<usize, String> {
    let mut found = Vec::new();
    let mut count = collect(dir, &mut found, problems, notices, texts)?;
    append_for_engine(engine, dir, found, active, problems);
    let retired = dir.join("fired");
    if retired.is_dir() {
        let mut found = Vec::new();
        count += collect(&retired, &mut found, problems, notices, texts)?;
        append_for_engine(engine, &retired, found, fired, problems);
    }
    Ok(count)
}

fn append_for_engine(
    engine: EngineId,
    dir: &Path,
    models: Vec<ModelSpec>,
    destination: &mut Vec<ModelSpec>,
    problems: &mut Vec<String>,
) {
    for model in models {
        if model.model.engine != engine {
            problems.push(format!(
                "el modelo '{}' declara engine '{}' pero está en el directorio de '{}' ({})",
                ropero_core::real_name(&model),
                model.model.engine.as_str(),
                engine.as_str(),
                dir.display()
            ));
        } else {
            destination.push(model);
        }
    }
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
    // Los manifiestos de los motores soportados son parte fija de Ropero y
    // deben estar disponibles aunque el usuario aún no tenga `engines/`.
    // El directorio del usuario puede sustituirlos por id, pero nunca es la
    // única fuente: de lo contrario un directorio vacío ocultaba todos los
    // motores instalables.
    const BUILTIN: &[(&str, &str)] = &[
        ("airllm", include_str!("../../../engines/airllm.toml")),
        ("ignis", include_str!("../../../engines/ignis.toml")),
        (
            "fastflowllm",
            include_str!("../../../engines/fastflowllm.toml"),
        ),
        (
            "llama-server",
            include_str!("../../../engines/llama-server.toml"),
        ),
        ("mlx-serve", include_str!("../../../engines/mlx-serve.toml")),
        ("strata", include_str!("../../../engines/strata.toml")),
        (
            "tensorrt-llm",
            include_str!("../../../engines/tensorrt-llm.toml"),
        ),
        (
            "turbo-llama",
            include_str!("../../../engines/turbo-llama.toml"),
        ),
        ("vllm", include_str!("../../../engines/vllm.toml")),
    ];
    let mut manifests = Vec::new();
    for (id, text) in BUILTIN {
        let path = PathBuf::from(format!("<ropero>/engines/{id}.toml"));
        match parse_engine_manifest(text, &path) {
            Ok(manifest) => manifests.push(manifest),
            Err(e) => problems.push(e.to_string()),
        }
    }
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
            Ok(manifiesto) => {
                if let Some(existing) = manifests.iter_mut().find(|m| m.id == manifiesto.id) {
                    *existing = manifiesto;
                } else {
                    manifests.push(manifiesto);
                }
            }
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

fn expand_home(ruta: &str) -> PathBuf {
    if let Some(rest) = ruta.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return Path::new(&home).join(rest);
    }
    PathBuf::from(ruta)
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
            .any(|ruta| expand_home(ruta).exists())
}

/// Detecta un engine usando el entorno capturado y el `bin_dir` configurado.
pub fn engine_installed_with_env(manifest: &EngineManifest, env: &Environment) -> bool {
    let on_path = |binary: &str| {
        env.get("PATH")
            .map(std::env::split_paths)
            .into_iter()
            .flatten()
            .map(|dir| dir.join(binary))
            .any(|candidate| es_ejecutable(&candidate))
    };
    let checkout_binary = manifest.install.as_ref().and_then(|install| {
        let binary = install.binary.as_deref()?;
        let source = install
            .source_dir
            .as_deref()
            .map(|raw| env.expand_path(raw))
            .unwrap_or_else(|| {
                env.home
                    .join(".local/share/ropero/engines")
                    .join(manifest.id.as_str())
            });
        let canonical = source.join(manifest.id.as_str());
        let legacy = source.join("fastflowlm");
        let engine_dir =
            if manifest.id == EngineId::FastFlowLm && !canonical.exists() && legacy.exists() {
                legacy
            } else {
                canonical
            };
        Some(engine_dir.join("build-ropero/install").join(binary))
    });
    manifest
        .detect
        .binaries
        .iter()
        .any(|binary| on_path(binary))
        || manifest.detect.binaries.iter().any(|binary| {
            let candidate = env.env_path("ROPERO_BIN_DIR", ".local/bin").join(binary);
            es_ejecutable(&candidate)
        })
        || manifest.detect.paths.iter().any(|raw| {
            let candidate = env.expand_path(raw);
            es_ejecutable(&candidate)
        })
        || checkout_binary.as_deref().is_some_and(es_ejecutable)
        || (manifest.id == EngineId::Ignis && ignis_installed_under_ropero(env))
}

fn ignis_installed_under_ropero(env: &Environment) -> bool {
    let root = env.home.join(".local/share/ropero/engines/ignis");
    std::fs::read_dir(root).ok().is_some_and(|entries| {
        entries.flatten().any(|entry| {
            let candidate = entry.path().join("ignis-server");
            es_ejecutable(&candidate)
        })
    })
}

/// Si el modelo está descargado: sus ficheros `[files]` existen.
///
/// Se resuelven con `resolve_paths` (el dominio): el core decide **qué**
/// ruta mirar, y aquí sólo se pregunta al disco.
pub fn model_downloaded(spec: &ModelSpec, env: &Environment) -> bool {
    if spec.model.format == ropero_core::ModelFormat::Ninfer && spec.model.engine == EngineId::Ignis
    {
        return crate::ignis::model_installed(spec, env);
    }
    // Modelos FLM (tag en almacén delegado): sin manifiesto no podemos consultar
    // `flm list`; tratamos como no confirmado.
    if ropero_core::flm::is_flm_model(spec) {
        return false;
    }
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

/// Como [`model_downloaded`], pero con contexto de motor para almacenes delegados.
pub fn model_downloaded_with_engine(
    spec: &ModelSpec,
    env: &Environment,
    engine: Option<&EngineManifest>,
) -> bool {
    if spec.model.engine == EngineId::Strata {
        return engine.is_some_and(|engine| crate::strata::model_installed(spec, engine, env));
    }
    if spec.model.format == ropero_core::ModelFormat::Ninfer && spec.model.engine == EngineId::Ignis
    {
        return crate::ignis::model_installed(spec, env);
    }
    if ropero_core::flm::is_flm_model(spec) {
        let Some(engine) = engine else {
            return false;
        };
        if crate::flm::delegated(engine) {
            return crate::flm::installed_status(engine, spec, env).unwrap_or(false);
        }
        return false;
    }
    model_downloaded(spec, env)
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
    /// Manifiestos de motor, para consultar almacenes delegados (FLM).
    pub engines: &'a [EngineManifest],
}

impl ropero_core::snapshot::Detect for Detector<'_> {
    fn engine_installed(&self, manifest: &EngineManifest) -> bool {
        engine_installed_with_env(manifest, self.env)
    }

    fn model_downloaded(&self, spec: &ModelSpec) -> bool {
        let engine = self.engines.iter().find(|e| e.id == spec.model.engine);
        model_downloaded_with_engine(spec, self.env, engine)
    }
}
