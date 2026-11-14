//! El entorno del proceso, capturado **fuera** del dominio.
//!
//! `ropero-core` es dominio puro: no lee disco, no consulta el entorno, no
//! lanza procesos (doc B §3.2.3). Lo que el dominio necesita del exterior
//! —las variables de entorno, el `$HOME` y el directorio de trabajo— llega
//! aquí como **datos**, capturados por la capa de presentación
//! (`ropero-cli::discovery`).
//!
//! Este tipo es sólo un contenedor: no tiene ninguna operación de E/S. La
//! captura ocurre en el CLI, que es quien sabe cómo leer el entorno real.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// El entorno del proceso, ya capturado.
///
/// Es el parámetro que sustituye a la E/S en el dominio: lo que antes se
/// leía con `std::env::var` o `std::env::current_dir` llega aquí como dato.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Environment {
    /// Las variables de entorno, por nombre.
    pub vars: BTreeMap<String, String>,
    /// El `$HOME` del proceso (vacío si el proceso no lo tiene).
    pub home: PathBuf,
    /// El directorio de trabajo del proceso.
    pub cwd: PathBuf,
}

impl Environment {
    /// Construye un entorno con las variables, el `$HOME` y el directorio de
    /// trabajo dados.
    pub fn new(
        vars: BTreeMap<String, String>,
        home: impl Into<PathBuf>,
        cwd: impl Into<PathBuf>,
    ) -> Self {
        Environment {
            vars,
            home: home.into(),
            cwd: cwd.into(),
        }
    }

    /// El valor de una variable de entorno, si está definida.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(|v| v.as_str())
    }

    /// Expande `~` y convierte en absoluta una ruta relativa.
    ///
    /// La ruta absoluta es la que cuelga del directorio de trabajo: el mismo
    /// resultado que daba `std::env::current_dir()`, pero sin consultarlo.
    pub fn expand_path(&self, raw: &str) -> PathBuf {
        let expanded = if raw == "~" {
            self.home.to_string_lossy().to_string()
        } else if let Some(rest) = raw.strip_prefix("~/") {
            format!("{}/{}", self.home.to_string_lossy(), rest)
        } else {
            raw.to_string()
        };
        let p = PathBuf::from(expanded);
        if p.is_absolute() { p } else { self.cwd.join(p) }
    }

    /// Ruta de una variable de entorno, con valor por defecto relativo a
    /// `$HOME`.
    pub fn env_path(&self, var: &str, default_rel: &str) -> PathBuf {
        let raw = match self.vars.get(var) {
            Some(v) => v.clone(),
            None => format!(
                "{}/{}",
                self.home.to_string_lossy(),
                default_rel.trim_start_matches("~/")
            ),
        };
        self.expand_path(&raw)
    }
}

/// El directorio de modelos: `<root>/models` si es el que hay, si no
/// `<root>`.
///
/// La **decisión** es de la capa de presentación (sólo ella sabe qué hay en
/// disco); aquí sólo se decide el nombre a partir de lo que le dicen.
pub fn models_base(root: &Path, has_models_dir: bool) -> PathBuf {
    if has_models_dir {
        root.join("models")
    } else {
        root.to_path_buf()
    }
}

/// Los nombres que delatan un directorio que es la base de los modelos.
///
/// La lista es del dominio: qué nombres significan "aquí hay modelos" es una
/// regla del catálogo, no una pregunta al disco.
pub const MODELS_BASE_NAMES: &[&str] = &[
    "_template.toml",
    "turbo-llama",
    "llama-server",
    "airllm",
    "mlx",
    "strata",
];

/// Las variables de entorno que pueden apuntar a la raíz del catálogo.
pub const ROOT_VARS: &[&str] = &["ROPERO_MODELS_DIR", "ROPERO_ROOT"];

/// La raíz del catálogo, o `None` si no se encuentra.
///
/// `probe` responde si un directorio es la raíz del catálogo (y cuál es).
/// `has_subdir` responde si un directorio tiene subdirectorios.
///
/// PURA: la **decisión** (qué se prueba, en qué orden) es del dominio; lo
/// que existe en disco lo responde la capa de presentación.
///
/// El orden es el pactado: cada variable se prueba **entera** —primero como
/// raíz, luego como directorio con subdirectorios— antes de pasar a la
/// siguiente; y la cadena de directorios se prueba sólo como raíz.
pub fn find_root(
    env: &Environment,
    exe_ancestors: &[PathBuf],
    probe: impl Fn(&Path) -> Option<PathBuf>,
    has_subdir: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    for var in ROOT_VARS {
        let Some(val) = env.get(var) else { continue };
        let p = PathBuf::from(val);
        if let Some(root) = probe(&p) {
            return Some(root);
        }
        // Una variable de entorno puede apuntar al directorio de trabajo.
        if has_subdir(&p) {
            return Some(p);
        }
    }
    // La cadena de directorios se prueba sólo como raíz.
    let mut chain: Vec<PathBuf> = env.cwd.ancestors().map(Path::to_path_buf).collect();
    chain.extend(exe_ancestors.iter().cloned());
    for dir in chain {
        if let Some(root) = probe(&dir) {
            return Some(root);
        }
    }
    None
}
