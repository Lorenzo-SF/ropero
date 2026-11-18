//! El descubrimiento del catálogo: la frontera entre la CLI y el runtime.
//!
//! La CLI es un **dispatcher** (R-ARQ-1): aquí sólo se juntan las tres
//! cosas que el dominio necesita —el catálogo, el entorno y la foto de la
//! máquina— y se le pasa al core. La E/S de verdad (leer el disco, buscar
//! un binario en el `PATH`) vive en `ropero-runtime`, que es la capa de
//! E/S del workspace.
//!
//! ## La foto de la máquina
//!
//! Aquí es donde se responde a las dos preguntas del doc CLI v2 §6:
//!
//! - ¿Qué motor está **instalado**? Su `detect` lo encuentra.
//! - ¿Qué modelo está **descargado**? Sus ficheros `[files]` existen.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ropero_core::config::{Config, EstadoConfig};
use ropero_core::{Catalog, EngineManifest, Environment, Snapshot};

/// El escaneo del catálogo, con el entorno ya capturado.
pub struct Scan {
    pub catalog: Catalog,
    pub problems: Vec<String>,
    pub files: usize,
    pub base: PathBuf,
    pub root: PathBuf,
    pub texts: BTreeMap<PathBuf, String>,
    pub env: Environment,
    pub engines: Vec<EngineManifest>,
    pub estado_config: EstadoConfig,
    pub config: Config,
}

/// Escanea el catálogo y devuelve la foto.
///
/// La configuración ilegible (parseo/lectura) es un error explícito: nunca se
/// oculta con una configuración efectiva de reemplazo.
pub fn scan() -> Result<Scan, i32> {
    let env = capture_environment();
    let (estado_config, config, env) = ropero_runtime::prepara_config(&env).map_err(fail)?;
    let scan = scan_with_config(env, estado_config, config)?;
    if let Some(error) = &scan.estado_config.error {
        return Err(fail(format!("configuración inválida: {error}")));
    }
    Ok(scan)
}

/// Escanea tolerando configuración ilegible, para comandos que deben reportarla
/// (`config`, `doctor`).
pub fn scan_tolerant() -> Result<Scan, i32> {
    let env = capture_environment();
    let (estado_config, config, env) = ropero_runtime::carga_config(&env).map_err(fail)?;
    scan_with_config(env, estado_config, config)
}

fn scan_with_config(
    env: Environment,
    mut estado_config: EstadoConfig,
    config: Config,
) -> Result<Scan, i32> {
    let root = ropero_runtime::resolve_catalog_root(&env, &config).map_err(fail)?;
    let leido = ropero_runtime::scan(&root, &env).map_err(fail)?;
    if estado_config.error.is_none() {
        estado_config.error = leido.config_error.clone();
    }
    for aviso in &leido.notices {
        ropero_core::err_line(aviso);
    }
    for problema in &leido.problems {
        ropero_core::err_line(&format!("aviso: {problema}"));
    }
    Ok(Scan {
        catalog: leido.catalog,
        problems: leido.problems,
        files: leido.files,
        base: leido.base,
        root: leido.root,
        texts: leido.texts,
        env,
        engines: leido.engines,
        estado_config,
        config,
    })
}

/// Captura el entorno del proceso.
pub fn capture_environment() -> Environment {
    let mut vars: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    ropero_runtime::flm::augment_environment(&mut vars);
    Environment::new(
        vars,
        std::env::var("HOME").unwrap_or_default(),
        std::env::current_dir().unwrap_or_default(),
    )
}

/// Reporta un error de E/S y devuelve el código 1.
fn fail(e: impl std::fmt::Display) -> i32 {
    ropero_core::err_line(&format!("error: {}", e));
    1
}

/// La foto de lo que hay en la máquina.
///
/// - Un **motor** está instalado si su `detect` lo encuentra.
/// - Un **modelo** está descargado si sus ficheros `[files]` existen.
pub fn snapshot(scan: &Scan) -> Snapshot {
    use ropero_core::snapshot::{EnginePresence, EngineState, ModelPresence, ModelState};

    let engines = std::thread::scope(|scope| {
        let checks = scan
            .engines
            .iter()
            .map(|manifest| {
                (
                    manifest,
                    scope.spawn(move || {
                        ropero_runtime::engine_installed_with_env(manifest, &scan.env)
                    }),
                )
            })
            .collect::<Vec<_>>();
        checks
            .into_iter()
            .map(|(manifest, check)| EngineState {
                presence: if check.join().unwrap_or(false) {
                    EnginePresence::Installed
                } else {
                    EnginePresence::Missing
                },
                manifest: manifest.clone(),
            })
            .collect::<Vec<_>>()
    });
    // Model presence checks can query delegated stores such as Strata and
    // FLM. Run independent checks concurrently so summary and listing time
    // follows the slowest engine query instead of the sum of all queries.
    let models = std::thread::scope(|scope| {
        let checks = scan
            .catalog
            .models
            .iter()
            .map(|spec| {
                let engine = scan
                    .engines
                    .iter()
                    .find(|engine| engine.id == spec.model.engine);
                (
                    spec,
                    scope.spawn(move || {
                        ropero_runtime::model_downloaded_with_engine(spec, &scan.env, engine)
                    }),
                )
            })
            .collect::<Vec<_>>();
        checks
            .into_iter()
            .map(|(spec, check)| ModelState {
                presence: if check.join().unwrap_or(false) {
                    ModelPresence::Downloaded
                } else {
                    ModelPresence::Missing
                },
                spec: spec.clone(),
            })
            .collect::<Vec<_>>()
    });
    Snapshot::new(engines, models)
}
