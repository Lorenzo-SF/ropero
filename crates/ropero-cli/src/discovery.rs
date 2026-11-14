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

use ropero_core::{Catalog, EngineManifest, Environment, ModelSpec, Snapshot};

/// El escaneo del catálogo, con el entorno ya capturado.
pub struct Scan {
    /// El catálogo de modelos.
    pub catalog: Catalog,
    /// Los problemas del escaneo (acumulados, no abortan).
    pub problems: Vec<String>,
    /// Cuántos ficheros TOML se leyeron.
    pub files: usize,
    /// El directorio de modelos.
    pub base: PathBuf,
    /// La raíz del catálogo.
    pub root: PathBuf,
    /// Los textos leídos, por ruta.
    pub texts: BTreeMap<PathBuf, String>,
    /// El entorno del proceso, ya capturado.
    pub env: Environment,
    /// Los manifiestos de motor leídos de `engines/`.
    pub engines: Vec<EngineManifest>,
}

/// Escanea el catálogo y devuelve la foto.
///
/// Los avisos del parseo se avisan por stderr: no son errores, así que no
/// cambian el código de salida.
pub fn scan() -> Result<Scan, i32> {
    let env = capture_environment();
    let root = ropero_runtime::find_root(&env, &ropero_runtime::exe_ancestors()).map_err(fail)?;
    let leido = ropero_runtime::scan(&root).map_err(fail)?;
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
    })
}

/// Captura el entorno del proceso.
pub fn capture_environment() -> Environment {
    Environment::new(
        std::env::vars().collect(),
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
    let detector = ropero_runtime::Detector { env: &scan.env };
    let modelos: Vec<ModelSpec> = scan
        .catalog
        .models
        .iter()
        .chain(scan.catalog.fired.iter())
        .cloned()
        .collect();
    ropero_core::snapshot::build_snapshot(&scan.engines, &modelos, &detector)
}
