//! La configuración de ropero: el directorio de configuración, el archivo de
//! configuración y las rutas que el usuario puede cambiar.
//!
//! ## Dónde vive
//!
//! | Qué | Dónde |
//! |---|---|
//! | el directorio de configuración | `$XDG_CONFIG_HOME/ropero`, o `~/.config/ropero` si esa variable no está |
//! | el archivo de configuración | `<directorio>/config.toml` |
//! | el catálogo oficial | `<directorio>/models/<motor>/<modelo>.toml` |
//!
//! ## La prioridad de cada ruta
//!
//! De más a menos:
//!
//! 1. la **variable de entorno** (`ROPERO_MODELS_DIR`, `GGUF_DIR`,
//!    `SAFETENSORS_DIR`, `ROPERO_BIN_DIR`, `LOG_DIR`);
//! 2. el **archivo de configuración** (el campo del mismo nombre);
//! 3. el **valor por defecto** (el que se deriva del `$HOME` y de la
//!    variable `XDG_CONFIG_HOME`).
//!
//! El archivo de configuración **no es obligatorio**: si no existe, se usan
//! los valores por defecto, y `ropero doctor` lo dice (y dice qué hacer).
//!
//! ## Sin E/S
//!
//! Este módulo es **dominio puro** (doc B §3.2.3): no lee disco ni consulta
//! el entorno. El entorno llega capturado ([`crate::Environment`]) y el texto
//! del archivo de configuración llega como **texto**: leerlo es de la capa de
//! E/S (`ropero-runtime`).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::estilo::{Celda, Columna, Seccion, Tabla, pares};
use crate::palette::Palette;
use crate::{EngineId, EngineManifest, Environment, PortRange};

/// El nombre del directorio de ropero dentro del de configuración.
pub const DIR_ROPERO: &str = "ropero";

/// El nombre del archivo de configuración.
pub const ARCHIVO_CONFIG: &str = "config.toml";

/// El nombre del directorio del catálogo dentro del de configuración.
pub const DIR_MODELOS: &str = "models";

/// El nombre del directorio de los manifiestos de motor dentro de la raíz
/// del catálogo.
pub const DIR_MOTORES: &str = "engines";

/// El nombre del directorio de logs dentro del de configuración.
pub const DIR_LOGS: &str = "logs";

/// Expande `~` y convierte en absoluta una ruta relativa.
///
/// La ruta absoluta cuelga del directorio de trabajo capturado.
pub fn expande_ruta(raw: &str, home: &Path, cwd: &Path) -> PathBuf {
    let expanded = if raw == "~" {
        home.to_string_lossy().to_string()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        format!("{}/{}", home.to_string_lossy(), rest)
    } else {
        raw.to_string()
    };
    let p = PathBuf::from(expanded);
    if p.is_absolute() { p } else { cwd.join(p) }
}

/// Ruta por defecto relativa a `$HOME`.
pub fn ruta_por_defecto(home: &Path, rel: &str) -> PathBuf {
    home.join(rel.trim_start_matches("~/"))
}

/// La variable de entorno que fija el directorio del catálogo.
pub const VAR_MODELOS: &str = "ROPERO_MODELS_DIR";

/// La variable de entorno que fija el directorio de los GGUFs.
pub const VAR_GGUF: &str = "GGUF_DIR";

/// La variable de entorno que fija el directorio de los checkpoints.
pub const VAR_SAFETENSORS: &str = "SAFETENSORS_DIR";

/// La variable de entorno que fija los artefactos de motores con almacén propio.
pub const VAR_ARTIFACTS: &str = "ARTIFACTS_DIR";

/// La variable de entorno que fija el directorio de los binarios.
pub const VAR_BIN: &str = "ROPERO_BIN_DIR";

/// La variable de entorno que fija el directorio de los logs.
pub const VAR_LOGS: &str = "LOG_DIR";

/// La variable de entorno que fija el directorio de configuración.
pub const VAR_XDG_CONFIG: &str = "XDG_CONFIG_HOME";

/// Los campos del archivo de configuración: su nombre y la variable de
/// entorno que manda sobre él.
///
/// Los dos nombres coinciden a propósito: el campo `gguf_dir` del archivo es
/// lo que dice la variable `GGUF_DIR`. Así no hay que aprender dos
/// vocabularios.
pub const CAMPOS: &[(&str, &str)] = &[
    ("models_dir", VAR_MODELOS),
    ("gguf_dir", VAR_GGUF),
    ("safetensors_dir", VAR_SAFETENSORS),
    ("artifacts_dir", VAR_ARTIFACTS),
    ("bin_dir", VAR_BIN),
    ("logs_dir", VAR_LOGS),
];

/// De dónde sale el valor de una ruta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origen {
    /// De una variable de entorno (la variable, con su nombre).
    Variable(String),
    /// Del archivo de configuración.
    Archivo,
    /// Del valor por defecto.
    Defecto,
}

impl Origen {
    /// El origen, como se cuenta en la salida: «variable GGUF_DIR»,
    /// «archivo config.toml» o «por defecto».
    pub fn texto(&self) -> String {
        match self {
            Origen::Variable(var) => format!("variable {var}"),
            Origen::Archivo => format!("archivo {ARCHIVO_CONFIG}"),
            Origen::Defecto => "por defecto".to_string(),
        }
    }

    /// El origen, como clave de máquina: «variable:GGUF_DIR», «archivo» o
    /// «defecto».
    ///
    /// Es lo que va en el JSON: una clave estable, que no cambia con el
    /// idioma del texto.
    pub fn clave(&self) -> String {
        match self {
            Origen::Variable(var) => format!("variable:{var}"),
            Origen::Archivo => "archivo".to_string(),
            Origen::Defecto => "defecto".to_string(),
        }
    }
}

/// Una ruta de la configuración y de dónde sale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ruta {
    /// El valor de la ruta.
    pub valor: PathBuf,
    /// De dónde sale.
    pub origen: Origen,
}

/// Overrides admitidos para un único engine.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct ConfigEngine {
    pub models_toml_dir: Option<Ruta>,
    pub artifacts_dir: Option<Ruta>,
    pub gguf_dir: Option<Ruta>,
    pub safetensors_dir: Option<Ruta>,
    pub port_min: Option<u16>,
    pub port_max: Option<u16>,
    /// Clave Bearer opcional que exigen los endpoints de este motor.
    /// Se redacta en todas las vistas de configuración.
    pub api_key: Option<String>,
}

impl std::fmt::Debug for ConfigEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigEngine")
            .field("models_toml_dir", &self.models_toml_dir)
            .field("artifacts_dir", &self.artifacts_dir)
            .field("gguf_dir", &self.gguf_dir)
            .field("safetensors_dir", &self.safetensors_dir)
            .field("port_min", &self.port_min)
            .field("port_max", &self.port_max)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl Ruta {
    /// Una ruta con su origen.
    pub fn new(valor: PathBuf, origen: Origen) -> Self {
        Ruta { valor, origen }
    }
}

/// La configuración de ropero: las rutas que el usuario puede cambiar.
///
/// Son las rutas globales que el usuario puede cambiar de verdad
/// (dónde está el catálogo, dónde los pesos, dónde los binarios y dónde los
/// logs). Un campo que no sirve para algo no se pone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Idioma preferido para la interfaz (es/en).
    pub lang: String,
    /// El directorio del catálogo: `<motor>/<modelo>.toml`.
    pub models_dir: Ruta,
    /// El directorio de los GGUFs: `<modelo>/<fichero>.gguf`.
    pub gguf_dir: Ruta,
    /// El directorio de los checkpoints: `<[model].path>`.
    pub safetensors_dir: Ruta,
    /// Artefactos descargados por motores con almacén propio (p. ej. Ignis).
    pub artifacts_dir: Ruta,
    /// El directorio donde se crean los enlaces de los motores.
    pub bin_dir: Ruta,
    /// El directorio de los logs: `<modelo>.log`.
    pub logs_dir: Ruta,
    /// Overrides por engine, sólo para ids reconocidos por Ropero.
    pub engines: BTreeMap<EngineId, ConfigEngine>,
}

impl Default for Config {
    /// La configuración de un entorno **vacío**: sin variables, sin `$HOME`
    /// y sin directorio de trabajo.
    ///
    /// Es lo que se usa cuando no se sabe nada del entorno (un `Default` de
    /// la estructura, no una configuración real).
    fn default() -> Self {
        config_defectos(&BTreeMap::new(), Path::new(""), Path::new(""))
    }
}

impl Config {
    /// La ruta que corresponde a una variable de entorno, si la configuración
    /// la tiene.
    ///
    /// Es lo que permite que [`crate::Environment::env_path`] sepa qué ruta
    /// decide la configuración: así lo que diga el archivo de configuración
    /// vale para **todos** los que piden una ruta por variable, y no sólo
    /// para los que saben que la configuración existe.
    pub fn ruta_de_var(&self, var: &str) -> Option<&Ruta> {
        match var {
            VAR_MODELOS => Some(&self.models_dir),
            VAR_GGUF => Some(&self.gguf_dir),
            VAR_SAFETENSORS => Some(&self.safetensors_dir),
            VAR_ARTIFACTS => Some(&self.artifacts_dir),
            VAR_BIN => Some(&self.bin_dir),
            VAR_LOGS => Some(&self.logs_dir),
            _ => None,
        }
    }

    /// Las rutas de la configuración, en el orden en que se enseñan.
    pub fn rutas(&self) -> [(&'static str, &Ruta); 6] {
        [
            ("models_dir", &self.models_dir),
            ("gguf_dir", &self.gguf_dir),
            ("safetensors_dir", &self.safetensors_dir),
            ("artifacts_dir", &self.artifacts_dir),
            ("bin_dir", &self.bin_dir),
            ("logs_dir", &self.logs_dir),
        ]
    }

    pub fn models_toml_dir(&self, engine: EngineId) -> Ruta {
        if matches!(self.models_dir.origen, Origen::Variable(_)) {
            return Ruta::new(
                self.models_dir.valor.join(engine.as_str()),
                self.models_dir.origen.clone(),
            );
        }
        if let Some(path) = self
            .engines
            .get(&engine)
            .and_then(|engine| engine.models_toml_dir.as_ref())
        {
            return path.clone();
        }
        Ruta::new(
            self.models_dir.valor.join(engine.as_str()),
            self.models_dir.origen.clone(),
        )
    }

    pub fn gguf_dir_de(&self, _engine: EngineId) -> Ruta {
        self.gguf_dir.clone()
    }

    pub fn artifacts_dir_de(&self, engine: EngineId) -> Ruta {
        if let Some(path) = self
            .engines
            .get(&engine)
            .and_then(|engine| engine.artifacts_dir.as_ref())
        {
            return path.clone();
        }
        if engine == EngineId::Basalt
            && (self.artifacts_dir.origen == Origen::Defecto
                || self
                    .artifacts_dir
                    .valor
                    .file_name()
                    .is_some_and(|name| name == "ignis"))
            && let Some(parent) = self.artifacts_dir.valor.parent()
        {
            return Ruta::new(parent.join("basalt"), self.artifacts_dir.origen.clone());
        }
        self.artifacts_dir.clone()
    }

    pub fn safetensors_dir_de(&self, _engine: EngineId) -> Ruta {
        self.safetensors_dir.clone()
    }

    /// Sobrescribe los puertos una vez cargados los manifests.
    pub fn overlay_engines(&self, manifests: &mut [EngineManifest]) -> Result<(), ErrorConfig> {
        let mut effective = manifests.to_vec();
        for manifest in &mut effective {
            let Some(overrides) = self.engines.get(&manifest.id) else {
                continue;
            };
            if overrides.port_min.is_some() || overrides.port_max.is_some() {
                let min = overrides.port_min.unwrap_or(manifest.port_range.min);
                let max = overrides.port_max.unwrap_or(manifest.port_range.max);
                if min == 0 || max == 0 || min > max {
                    return Err(ErrorConfig::de_campo(
                        &format!("engines.{}.port_min/port_max", manifest.id.as_str()),
                        &format!("{min}-{max}"),
                        "el rango efectivo debe usar puertos 1-65535 y port_min no puede superar port_max",
                    ));
                }
                manifest.port_range = PortRange::new(min, max);
            }
        }
        if let Some(error) = crate::validate_engine_set(&effective).first() {
            return Err(ErrorConfig::de_campo("engines", "", error));
        }
        manifests.clone_from_slice(&effective);
        Ok(())
    }
}

/// Un error del archivo de configuración.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorConfig {
    /// El campo que no vale (o el nombre del archivo, si el problema es del
    /// archivo entero).
    pub campo: String,
    /// El valor que se puso (vacío si el problema es del archivo entero).
    pub valor: String,
    /// Por qué no vale.
    pub motivo: String,
}

impl ErrorConfig {
    /// El error de un campo que no vale.
    pub fn de_campo(campo: &str, valor: &str, motivo: &str) -> Self {
        ErrorConfig {
            campo: campo.to_string(),
            valor: valor.to_string(),
            motivo: motivo.to_string(),
        }
    }

    /// El error del archivo entero: no se puede leer o no es TOML.
    pub fn de_archivo(motivo: &str) -> Self {
        ErrorConfig {
            campo: ARCHIVO_CONFIG.to_string(),
            valor: String::new(),
            motivo: motivo.to_string(),
        }
    }

    /// Qué hacer para arreglarlo (P-13).
    ///
    /// El mensaje dice siempre **qué hacer**: corregir el campo, o quitarlo
    /// para volver a los valores por defecto.
    pub fn que_hacer(&self) -> String {
        if self.valor.is_empty() {
            format!(
                "corrige {0} (o bórralo para usar los valores por defecto)",
                self.campo
            )
        } else {
            format!(
                "pon una ruta válida en {0} (o quítalo para usar el valor por defecto)",
                self.campo
            )
        }
    }
}

impl fmt::Display for ErrorConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.valor.is_empty() {
            write!(f, "{}: {}", self.campo, self.motivo)
        } else {
            write!(
                f,
                "{} no vale: {} (valor: '{}')",
                self.campo, self.motivo, self.valor
            )
        }
    }
}

impl std::error::Error for ErrorConfig {}

/// El estado del archivo de configuración en la máquina.
///
/// Lo que hay en disco **no** lo sabe el dominio: se lo dice la capa de E/S.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoConfig {
    /// El directorio de configuración.
    pub directorio: PathBuf,
    /// El archivo de configuración.
    pub archivo: PathBuf,
    /// Si el archivo existe.
    pub existe: bool,
    /// El error del archivo (si no se pudo leer o no valía).
    pub error: Option<ErrorConfig>,
}

/// El directorio de configuración: `$XDG_CONFIG_HOME/ropero` si la variable
/// está definida; si no, `~/.config/ropero`.
///
/// En macOS (sin `XDG_CONFIG_HOME`) es `~/.config/ropero` también: es el
/// mismo sitio, y el mismo camino para todo el mundo.
pub fn directorio_config(env: &Environment) -> PathBuf {
    directorio_config_de(&env.vars, &env.home, &env.cwd)
}

/// El directorio de configuración, con las variables y el `$HOME` dados.
pub fn directorio_config_de(vars: &BTreeMap<String, String>, home: &Path, cwd: &Path) -> PathBuf {
    match vars.get(VAR_XDG_CONFIG) {
        Some(valor) if !valor.trim().is_empty() => expande_ruta(valor, home, cwd).join(DIR_ROPERO),
        _ => home.join(".config").join(DIR_ROPERO),
    }
}

/// El archivo de configuración: `<directorio de configuración>/config.toml`.
pub fn archivo_config(env: &Environment) -> PathBuf {
    directorio_config(env).join(ARCHIVO_CONFIG)
}

/// La configuración por defecto: la derivan el `$HOME` y el directorio de
/// configuración.
///
/// **No** aplica las variables de entorno: eso es lo que hace
/// [`config_inicial`]. Aquí sólo están los valores por defecto, que son los
/// que gana el archivo de configuración.
pub fn config_defectos(vars: &BTreeMap<String, String>, home: &Path, cwd: &Path) -> Config {
    let dir_config = directorio_config_de(vars, home, cwd);
    let engines = BTreeMap::new();
    Config {
        lang: "EN".to_string(),
        models_dir: Ruta::new(dir_config.join(DIR_MODELOS), Origen::Defecto),
        gguf_dir: Ruta::new(ruta_por_defecto(home, "models/gguf"), Origen::Defecto),
        safetensors_dir: Ruta::new(
            ruta_por_defecto(home, "models/safetensors"),
            Origen::Defecto,
        ),
        artifacts_dir: Ruta::new(ruta_por_defecto(home, "models/ignis"), Origen::Defecto),
        bin_dir: Ruta::new(ruta_por_defecto(home, ".local/bin"), Origen::Defecto),
        logs_dir: Ruta::new(dir_config.join(DIR_LOGS), Origen::Defecto),
        engines,
    }
}

/// La configuración con las variables de entorno ya aplicadas.
///
/// Es la que lleva el entorno mientras no se lea el archivo de
/// configuración: las variables **mandan** sobre cualquier archivo.
pub fn config_inicial(vars: &BTreeMap<String, String>, home: &Path, cwd: &Path) -> Config {
    con_variables(config_defectos(vars, home, cwd), vars, home, cwd)
}

/// Aplica las variables de entorno encima de una configuración.
///
/// La variable **definida** gana, aunque esté vacía: es lo que dice el
/// usuario (y es lo que ya hacía [`crate::Environment::env_path`]).
fn con_variables(
    mut config: Config,
    vars: &BTreeMap<String, String>,
    home: &Path,
    cwd: &Path,
) -> Config {
    let aplica = |var: &str| -> Option<PathBuf> {
        vars.get(var).map(|valor| expande_ruta(valor, home, cwd))
    };
    if let Some(valor) = aplica(VAR_MODELOS) {
        config.models_dir = Ruta::new(valor, Origen::Variable(VAR_MODELOS.to_string()));
    }
    if let Some(valor) = aplica(VAR_GGUF) {
        config.gguf_dir = Ruta::new(valor, Origen::Variable(VAR_GGUF.to_string()));
    }
    if let Some(valor) = aplica(VAR_SAFETENSORS) {
        config.safetensors_dir = Ruta::new(valor, Origen::Variable(VAR_SAFETENSORS.to_string()));
    }
    if let Some(valor) = aplica(VAR_ARTIFACTS) {
        config.artifacts_dir = Ruta::new(valor, Origen::Variable(VAR_ARTIFACTS.to_string()));
    }
    if let Some(valor) = aplica(VAR_BIN) {
        config.bin_dir = Ruta::new(valor, Origen::Variable(VAR_BIN.to_string()));
    }
    if let Some(valor) = aplica(VAR_LOGS) {
        config.logs_dir = Ruta::new(valor, Origen::Variable(VAR_LOGS.to_string()));
    }
    config
}

/// La configuración que declara el archivo de configuración.
///
/// Los campos que el archivo no declara se quedan en su valor por defecto.
/// Un campo que no existe (o un valor que no es una ruta) es un **error**:
/// un archivo de configuración con un campo mal escrito callado es peor que
/// uno que dice qué está mal.
pub fn parse_config(texto: &str, env: &Environment) -> Result<Config, ErrorConfig> {
    let mut config = config_defectos(&env.vars, &env.home, &env.cwd);
    let tabla: toml::Table =
        toml::from_str(texto).map_err(|e| ErrorConfig::de_archivo(&e.to_string()))?;
    for (campo, valor) in tabla {
        if campo == "lang" {
            let Some(lang) = valor.as_str() else {
                return Err(ErrorConfig::de_campo("lang", "", "debe ser ES o EN"));
            };
            let normalized = lang.to_ascii_uppercase();
            if normalized != "ES" && normalized != "EN" {
                return Err(ErrorConfig::de_campo(
                    "lang",
                    lang,
                    "sólo se admite ES o EN",
                ));
            }
            config.lang = normalized;
            continue;
        }
        if campo == "engines" {
            let Some(engines) = valor.as_table() else {
                return Err(ErrorConfig::de_campo(
                    "engines",
                    "",
                    "debe ser una tabla con secciones [engines.<id>]",
                ));
            };
            for (raw_id, raw_config) in engines {
                let (id, _) = EngineId::from_str_legacy(raw_id).map_err(|_| {
                    ErrorConfig::de_campo(
                        &format!("engines.{raw_id}"),
                        raw_id,
                        "engine desconocido; usa un id canónico de Ropero",
                    )
                })?;
                let Some(fields) = raw_config.as_table() else {
                    return Err(ErrorConfig::de_campo(
                        &format!("engines.{raw_id}"),
                        "",
                        "debe ser una tabla",
                    ));
                };
                let parsed = parse_engine_config(raw_id, fields, env)?;
                let current_is_canonical = config
                    .engines
                    .get(&id)
                    .is_some_and(|_| raw_id != id.as_str());
                if !current_is_canonical || raw_id == id.as_str() {
                    config.engines.insert(id, parsed);
                }
            }
            continue;
        }
        if campo == "models" {
            let Some(models) = valor.as_table() else {
                return Err(ErrorConfig::de_campo(
                    "models",
                    "",
                    "debe ser una tabla con una sección [models.paths]",
                ));
            };
            for (section, section_value) in models {
                if section != "paths" {
                    return Err(ErrorConfig::de_campo(
                        &format!("models.{section}"),
                        "",
                        "sección desconocida (usa [models.paths])",
                    ));
                }
                let Some(paths) = section_value.as_table() else {
                    return Err(ErrorConfig::de_campo(
                        "models.paths",
                        "",
                        "debe ser una tabla de rutas",
                    ));
                };
                parse_model_paths(paths, &mut config, env)?;
            }
            continue;
        }
        let Some(crudo) = valor.as_str() else {
            return Err(ErrorConfig::de_campo(
                &campo,
                "",
                &format!("el valor no es una ruta (es {})", tipo_de(&valor)),
            ));
        };
        if let Some(motivo) = problema_de_ruta(crudo) {
            return Err(ErrorConfig::de_campo(&campo, crudo, motivo));
        }
        let ruta = env.expand_path(crudo);
        match campo.as_str() {
            "models_dir" => {
                config.models_dir = Ruta::new(ruta, Origen::Archivo);
            }
            "gguf_dir" => {
                config.gguf_dir = Ruta::new(ruta, Origen::Archivo);
            }
            "safetensors_dir" => {
                config.safetensors_dir = Ruta::new(ruta, Origen::Archivo);
            }
            "artifacts_dir" => {
                config.artifacts_dir = Ruta::new(ruta, Origen::Archivo);
            }
            "bin_dir" => {
                config.bin_dir = Ruta::new(ruta, Origen::Archivo);
            }
            "logs_dir" => {
                config.logs_dir = Ruta::new(ruta, Origen::Archivo);
            }
            otra => {
                return Err(ErrorConfig::de_campo(
                    otra,
                    crudo,
                    &format!(
                        "campo desconocido (los campos son: {})",
                        nombres_de_campos()
                    ),
                ));
            }
        }
    }
    Ok(config)
}

fn parse_model_paths(
    paths: &toml::Table,
    config: &mut Config,
    env: &Environment,
) -> Result<(), ErrorConfig> {
    for (field, value) in paths {
        let Some(raw) = value.as_str() else {
            return Err(ErrorConfig::de_campo(
                &format!("models.paths.{field}"),
                "",
                &format!("el valor no es una ruta (es {})", tipo_de(value)),
            ));
        };
        if let Some(reason) = problema_de_ruta(raw) {
            return Err(ErrorConfig::de_campo(
                &format!("models.paths.{field}"),
                raw,
                reason,
            ));
        }
        let route = Ruta::new(env.expand_path(raw), Origen::Archivo);
        match field.as_str() {
            "models_dir" => config.models_dir = route,
            "gguf_dir" => config.gguf_dir = route,
            "safetensors_dir" => config.safetensors_dir = route,
            "artifacts_dir" => config.artifacts_dir = route,
            "bin_dir" => config.bin_dir = route,
            "logs_dir" => config.logs_dir = route,
            _ => {
                return Err(ErrorConfig::de_campo(
                    &format!("models.paths.{field}"),
                    raw,
                    "campo desconocido (models_dir, gguf_dir, safetensors_dir, artifacts_dir, bin_dir, logs_dir)",
                ));
            }
        }
    }
    Ok(())
}

fn parse_engine_config(
    id: &str,
    fields: &toml::Table,
    env: &Environment,
) -> Result<ConfigEngine, ErrorConfig> {
    let mut result = ConfigEngine::default();
    for (field, value) in fields {
        let name = format!("engines.{id}.{field}");
        match field.as_str() {
            "models_path" | "models_toml_dir" | "artifacts_dir" | "gguf_dir"
            | "safetensors_dir" => {
                let Some(raw) = value.as_str() else {
                    return Err(ErrorConfig::de_campo(
                        &name,
                        "",
                        &format!("el valor no es una ruta (es {})", tipo_de(value)),
                    ));
                };
                if let Some(reason) = problema_de_ruta(raw) {
                    return Err(ErrorConfig::de_campo(&name, raw, reason));
                }
                let route = Ruta::new(env.expand_path(raw), Origen::Archivo);
                match field.as_str() {
                    "models_path" | "models_toml_dir" => result.models_toml_dir = Some(route),
                    "artifacts_dir" => result.artifacts_dir = Some(route),
                    "gguf_dir" => result.gguf_dir = Some(route),
                    _ => result.safetensors_dir = Some(route),
                }
            }
            "port_min" | "port_max" => {
                let Some(number) = value.as_integer() else {
                    return Err(ErrorConfig::de_campo(
                        &name,
                        "",
                        "debe ser un entero entre 1 y 65535",
                    ));
                };
                if !(1..=65535).contains(&number) {
                    return Err(ErrorConfig::de_campo(
                        &name,
                        &number.to_string(),
                        "debe ser un entero entre 1 y 65535",
                    ));
                }
                if field == "port_min" {
                    result.port_min = Some(number as u16);
                } else {
                    result.port_max = Some(number as u16);
                }
            }
            "color" => {
                // Legacy setting retained for config-file compatibility;
                // engine colors are fixed by the built-in hexadecimal theme.
                let _ = value;
            }
            "api_key" => {
                let Some(raw) = value.as_str() else {
                    return Err(ErrorConfig::de_campo(&name, "", "debe ser una cadena"));
                };
                if raw.is_empty() || raw.contains(['\n', '\r']) {
                    return Err(ErrorConfig::de_campo(
                        &name,
                        "[REDACTADO]",
                        "la clave no puede estar vacía ni contener saltos de línea",
                    ));
                }
                result.api_key = Some(raw.to_string());
            }
            _ => {
                return Err(ErrorConfig::de_campo(
                    &name,
                    value.to_string().as_str(),
                    "campo desconocido (models_path, artifacts_dir, gguf_dir, safetensors_dir, port_min, port_max, api_key)",
                ));
            }
        }
    }
    if let (Some(min), Some(max)) = (result.port_min, result.port_max)
        && min > max
    {
        return Err(ErrorConfig::de_campo(
            &format!("engines.{id}.port_min/port_max"),
            &format!("{min}-{max}"),
            "port_min no puede superar port_max",
        ));
    }
    Ok(result)
}

/// La configuración final: el archivo de configuración (si lo hay) sobre los
/// valores por defecto, y las variables de entorno encima.
///
/// El texto es el que se ha leído del disco: el dominio no lee disco.
pub fn resuelve_config(env: &Environment, texto: Option<&str>) -> Result<Config, ErrorConfig> {
    let base = match texto {
        Some(t) => parse_config(t, env)?,
        None => config_defectos(&env.vars, &env.home, &env.cwd),
    };
    Ok(con_variables(base, &env.vars, &env.home, &env.cwd))
}

/// Por qué no vale una ruta (si no vale).
fn problema_de_ruta(ruta: &str) -> Option<&'static str> {
    if ruta.trim().is_empty() {
        Some("la ruta está vacía")
    } else if ruta.contains('\0') {
        Some("la ruta tiene un carácter nulo")
    } else {
        None
    }
}

/// Cómo es un valor que no es una ruta: el mensaje dice **qué** hay.
fn tipo_de(valor: &toml::Value) -> &'static str {
    match valor {
        toml::Value::String(_) => "una cadena",
        toml::Value::Integer(_) | toml::Value::Float(_) => "un número",
        toml::Value::Boolean(_) => "un sí o un no",
        toml::Value::Datetime(_) => "una fecha",
        toml::Value::Array(_) => "una lista",
        toml::Value::Table(_) => "una tabla",
    }
}

/// Los nombres de los campos del archivo de configuración, en una lista.
fn nombres_de_campos() -> String {
    let mut fields: Vec<&str> = CAMPOS.iter().map(|(campo, _)| *campo).collect();
    fields.push("engines.<id>");
    fields.join(", ")
}

/// La raíz del catálogo: el directorio que contiene `models/` (y `engines/`).
///
/// El catálogo oficial vive en `<config>/models`, así que su raíz es
/// `<config>`. Si el usuario pone el catálogo en otro sitio, la raíz es ese
/// sitio: el runtime lee los manifiestos de motor de `engines/`, al lado del
/// catálogo.
pub fn raiz_del_catalogo(models_dir: &Path) -> PathBuf {
    let es_models = models_dir
        .file_name()
        .is_some_and(|nombre| nombre == DIR_MODELOS);
    if es_models && let Some(padre) = models_dir.parent() {
        return padre.to_path_buf();
    }
    models_dir.to_path_buf()
}

/// El ejemplo del archivo de configuración: valores globales y una tabla
/// completa por cada motor soportado.
pub fn ejemplo_config(config: &Config) -> String {
    let mut out = String::new();
    out.push_str("# La configuración de ropero.\n");
    out.push_str("#\n");
    out.push_str("# Cada línea dice dónde vive una cosa. Si quitas una línea, se\n");
    out.push_str("# usa el valor por defecto. Una variable de entorno manda sobre\n");
    out.push_str("# este archivo (ROPERO_MODELS_DIR, GGUF_DIR, SAFETENSORS_DIR,\n");
    out.push_str("# ROPERO_BIN_DIR, LOG_DIR).\n");
    out.push_str("#\n");
    out.push_str("# El catálogo de modelos del usuario vive en <config>/models/<motor>/\n");
    out.push_str("# y los manifiestos de motor en <config>/engines/.\n");
    out.push('\n');
    out.push_str(&format!("lang = \"{}\"\n\n", config.lang));
    out.push_str("[models.paths]\n");
    out.push_str(&format!(
        "# El catálogo: <motor>/<modelo>.toml\nmodels_dir = \"{}\"\n",
        escapa_toml(&config.models_dir.valor.to_string_lossy())
    ));
    out.push_str(&format!(
        "# Los GGUFs: <modelo>/<fichero>.gguf\ngguf_dir = \"{}\"\n",
        escapa_toml(&config.gguf_dir.valor.to_string_lossy())
    ));
    out.push_str(&format!(
        "# Los checkpoints: <[model].path>\nsafetensors_dir = \"{}\"\n",
        escapa_toml(&config.safetensors_dir.valor.to_string_lossy())
    ));
    out.push_str(&format!(
        "# Artefactos gestionados por motores con almacén propio (p. ej. Ignis)\nartifacts_dir = \"{}\"\n",
        escapa_toml(&config.artifacts_dir.valor.to_string_lossy())
    ));
    out.push_str(&format!(
        "# Los enlaces de los motores\nbin_dir = \"{}\"\n",
        escapa_toml(&config.bin_dir.valor.to_string_lossy())
    ));
    out.push_str(&format!(
        "# Los logs: <modelo>.log\nlogs_dir = \"{}\"\n",
        escapa_toml(&config.logs_dir.valor.to_string_lossy())
    ));
    out.push_str("\n# Configuración por motor. Las recetas y dependencias son fijas.\n");
    for id in [
        EngineId::LlamaServer,
        EngineId::TurboLlama,
        EngineId::FastFlowLm,
        EngineId::MlxServe,
        EngineId::TensorRtLlm,
        EngineId::Vllm,
        EngineId::AirLlm,
        EngineId::Strata,
        EngineId::Ignis,
        EngineId::Basalt,
    ] {
        let (port_min, port_max) = valores_por_defecto_engine(id);
        out.push_str(&format!(
            "\n[engines.{}]\nmodels_path = \"{}\"\nport_min = {}\nport_max = {}\n# api_key = \"\" # opcional; se envía como Bearer al endpoint local\n",
            id.as_str(),
            escapa_toml(&config.models_toml_dir(id).valor.to_string_lossy()),
            port_min,
            port_max,
        ));
        if id == EngineId::Basalt
            && let Some(parent) = config.artifacts_dir.valor.parent()
        {
            out.push_str(&format!(
                "artifacts_dir = \"{}\"\n",
                escapa_toml(&parent.join("basalt").to_string_lossy())
            ));
        }
    }
    out
}

/// Valores base de puertos, alineados con los manifiestos embebidos.
fn valores_por_defecto_engine(id: EngineId) -> (u16, u16) {
    match id {
        EngineId::LlamaServer => (9980, 9989),
        EngineId::TurboLlama => (9990, 9999),
        EngineId::FastFlowLm => (9970, 9979),
        EngineId::MlxServe => (9960, 9969),
        EngineId::TensorRtLlm => (9950, 9959),
        EngineId::Vllm => (9940, 9949),
        EngineId::AirLlm => (9930, 9939),
        EngineId::Strata => (9920, 9929),
        EngineId::Ignis => (9910, 9919),
        EngineId::Basalt => (9900, 9909),
    }
}

/// Escapa un texto para que valga dentro de una cadena básica TOML.
///
/// Produce el contenido que va entre comillas dobles `"..."`, escapando los
/// caracteres que TOML exige (comillas, barra inversa y controles).
pub fn escapa_toml(texto: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{0C}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Las líneas de `ropero config`: la configuración resuelta y de dónde sale
/// cada ruta.
pub fn lineas_config(
    estado: &EstadoConfig,
    config: &Config,
    paleta: &Palette,
    lang: &str,
) -> Vec<String> {
    let english = lang.eq_ignore_ascii_case("EN");
    let directorio = estado.directorio.to_string_lossy().to_string();
    let archivo = estado.archivo.to_string_lossy().to_string();
    let estado_del_archivo = if english {
        estado_texto_en(estado)
    } else {
        estado_texto(estado)
    };
    let mut out: Vec<String> = Vec::new();
    let mut seccion = Seccion::nueva(if english {
        "CONFIGURATION"
    } else {
        "CONFIGURACIÓN"
    });
    seccion.cuerpo(pares(
        1,
        &[
            (if english { "language" } else { "idioma" }, &config.lang),
            (
                if english {
                    "configuration directory"
                } else {
                    "directorio de configuración"
                },
                &directorio,
            ),
            (
                if english {
                    "configuration file"
                } else {
                    "archivo de configuración"
                },
                &archivo,
            ),
            (
                if english {
                    "file status"
                } else {
                    "estado del archivo"
                },
                &estado_del_archivo,
            ),
        ],
    ));
    out.extend(seccion.lineas());
    out.push(String::new());
    let mut tabla = Tabla::nivel(1)
        .columna(Columna::texto(if english { "path" } else { "ruta" }))
        .columna(Columna::texto(if english { "value" } else { "valor" }))
        .columna(Columna::texto(if english { "source" } else { "origen" }));
    for (nombre, ruta) in config.rutas() {
        tabla.fila(vec![
            Celda::plana(nombre),
            Celda::plana(ruta.valor.to_string_lossy().to_string()),
            Celda::plana(if english {
                origen_texto_en(&ruta.origen)
            } else {
                ruta.origen.texto()
            }),
        ]);
    }
    let mut seccion = Seccion::nueva(if english { "PATHS" } else { "RUTAS" });
    seccion.cuerpo(tabla.lineas(paleta));
    out.extend(seccion.lineas());
    if !config.engines.is_empty() {
        out.push(String::new());
        let mut tabla = Tabla::nivel(1)
            .columna(Columna::texto("engine"))
            .columna(Columna::texto(if english { "field" } else { "campo" }))
            .columna(Columna::texto(if english { "value" } else { "valor" }))
            .columna(Columna::texto(if english { "source" } else { "origen" }));
        for (engine, overrides) in &config.engines {
            let mut values = vec![(
                if english { "ports" } else { "puertos" },
                match (overrides.port_min, overrides.port_max) {
                    (None, None) => "manifest".to_string(),
                    (Some(min), Some(max)) => format!("{min}-{max}"),
                    (Some(min), None) => format!("{min}-manifest"),
                    (None, Some(max)) => format!("manifest-{max}"),
                },
                if overrides.port_min.is_some() || overrides.port_max.is_some() {
                    if english {
                        "config.toml file"
                    } else {
                        "archivo config.toml"
                    }
                } else {
                    "manifest"
                }
                .to_string(),
            )];
            if overrides.models_toml_dir.is_some()
                || overrides.artifacts_dir.is_some()
                || overrides.gguf_dir.is_some()
                || overrides.safetensors_dir.is_some()
            {
                values.push((
                    if english {
                        "legacy paths"
                    } else {
                        "rutas legacy"
                    },
                    if english {
                        "ignored; use [models.paths]"
                    } else {
                        "ignoradas; usa [models.paths]"
                    }
                    .to_string(),
                    if english {
                        "migration required"
                    } else {
                        "migración requerida"
                    }
                    .to_string(),
                ));
            }
            for (field, value, origin) in values {
                tabla.fila(vec![
                    Celda::plana(engine.as_str()),
                    Celda::plana(field),
                    Celda::plana(value),
                    Celda::plana(origin),
                ]);
            }
        }
        let mut seccion = Seccion::nueva(if english {
            "ENGINE OVERRIDES"
        } else {
            "OVERRIDES POR ENGINE"
        });
        seccion.cuerpo(tabla.lineas(paleta));
        out.extend(seccion.lineas());
    }
    if !estado.existe && estado.error.is_none() {
        out.push(String::new());
        let mut seccion = Seccion::nueva(if english {
            "TO CHANGE PATHS"
        } else {
            "PARA CAMBIAR LAS RUTAS"
        });
        seccion.cuerpo(pares(
            1,
            &[(if english { "file" } else { "archivo" }, &archivo)],
        ));
        out.extend(seccion.lineas());
        for linea in ejemplo_config(config).lines() {
            out.push(format!("    {linea}"));
        }
    }
    out
}

/// Las líneas del doctor sobre la configuración: si falta el archivo, **qué
/// hacer**; si está, dónde está.
pub fn lineas_doctor_config(estado: &EstadoConfig, config: &Config) -> Vec<String> {
    let archivo = estado.archivo.to_string_lossy().to_string();
    let estado_del_archivo = estado_texto(estado);
    let pista = que_hacer_config(estado);
    let mut out: Vec<String> = Vec::new();
    let mut seccion = Seccion::nueva("CONFIGURACIÓN");
    let mut campos: Vec<(&str, &str)> = vec![("archivo de configuración", &archivo)];
    campos.push(("estado", &estado_del_archivo));
    if let Some(pista) = pista.as_deref() {
        campos.push(("qué hacer", pista));
    }
    seccion.cuerpo(pares(1, &campos));
    out.extend(seccion.lineas());
    for (engine, overrides) in &config.engines {
        if overrides.models_toml_dir.is_some()
            || overrides.artifacts_dir.is_some()
            || overrides.gguf_dir.is_some()
            || overrides.safetensors_dir.is_some()
        {
            out.push(format!(
                "  {}: hay rutas por-engine antiguas; se ignoran. Muévelas a [models.paths] en {}",
                engine.as_str(),
                estado.archivo.display()
            ));
        }
        if overrides.port_min.is_some() || overrides.port_max.is_some() {
            out.push(format!(
                "    puertos: {}-{}",
                overrides
                    .port_min
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "manifest".to_string()),
                overrides
                    .port_max
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "manifest".to_string()),
            ));
        }
    }
    out
}

/// El estado del archivo de configuración, en una línea.
fn estado_texto(estado: &EstadoConfig) -> String {
    match &estado.error {
        Some(error) => format!("no vale: {}", error),
        None if estado.existe => "ok".to_string(),
        None => "no existe: se creará en el primer uso".to_string(),
    }
}

fn estado_texto_en(estado: &EstadoConfig) -> String {
    match &estado.error {
        Some(error) => format!("invalid: {error}"),
        None if estado.existe => "ok".to_string(),
        None => "not found: will be created on first use".to_string(),
    }
}

fn origen_texto_en(origen: &Origen) -> String {
    match origen {
        Origen::Variable(var) => format!("environment variable {var}"),
        Origen::Archivo => format!("{} file", ARCHIVO_CONFIG),
        Origen::Defecto => "default".to_string(),
    }
}

/// Qué hacer con el archivo de configuración, si hay algo que hacer.
fn que_hacer_config(estado: &EstadoConfig) -> Option<String> {
    estado.error.as_ref().map(|error| error.que_hacer())
}

/// La raíz de manifiestos de motor junto al catálogo.
///
/// Si `models_dir` termina en `models/`, los motores viven en
/// `<raíz>/engines/`.
pub fn directorio_engines(models_dir: &Path) -> PathBuf {
    raiz_del_catalogo(models_dir).join(DIR_MOTORES)
}

/// La configuración en JSON: la misma información que las líneas, para los
/// scripts.
///
/// El JSON se construye **a mano**: el core no puede añadir dependencias, y
/// un `serde_json` por un objeto de cadenas no compensa.
pub fn config_a_json(estado: &EstadoConfig, config: &Config) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!(
        "  \"directorio_config\": \"{}\",\n",
        escapa_json(&estado.directorio.to_string_lossy())
    ));
    out.push_str(&format!(
        "  \"archivo_config\": \"{}\",\n",
        escapa_json(&estado.archivo.to_string_lossy())
    ));
    out.push_str(&format!("  \"archivo_existe\": {},\n", estado.existe));
    out.push_str("  \"rutas\": {\n");
    let rutas = config.rutas();
    for (indice, (nombre, ruta)) in rutas.iter().enumerate() {
        let coma = if indice + 1 == rutas.len() { "" } else { "," };
        out.push_str(&format!(
            "    \"{nombre}\": {{\"valor\": \"{}\", \"origen\": \"{}\"}}{coma}\n",
            escapa_json(&ruta.valor.to_string_lossy()),
            escapa_json(&ruta.origen.clave())
        ));
    }
    out.push_str("  },\n  \"engines\": {");
    if !config.engines.is_empty() {
        out.push('\n');
        for (index, (engine, overrides)) in config.engines.iter().enumerate() {
            let tomls = config.models_toml_dir(*engine);
            let artifacts = config.artifacts_dir_de(*engine);
            let gguf = config.gguf_dir_de(*engine);
            let safetensors = config.safetensors_dir_de(*engine);
            let comma = if index + 1 == config.engines.len() {
                ""
            } else {
                ","
            };
            out.push_str(&format!(
                "    \"{}\": {{\n      \"models_path\": {{\"valor\": \"{}\", \"origen\": \"{}\"}},\n      \"artifacts_dir\": {{\"valor\": \"{}\", \"origen\": \"{}\"}},\n      \"gguf_dir\": {{\"valor\": \"{}\", \"origen\": \"{}\"}},\n      \"safetensors_dir\": {{\"valor\": \"{}\", \"origen\": \"{}\"}},\n      \"port_min\": {},\n      \"port_max\": {}\n    }}{comma}\n",
                engine.as_str(),
                escapa_json(&tomls.valor.to_string_lossy()),
                escapa_json(&tomls.origen.clave()),
                escapa_json(&artifacts.valor.to_string_lossy()),
                escapa_json(&artifacts.origen.clave()),
                escapa_json(&gguf.valor.to_string_lossy()),
                escapa_json(&gguf.origen.clave()),
                escapa_json(&safetensors.valor.to_string_lossy()),
                escapa_json(&safetensors.origen.clave()),
                overrides.port_min.map(|p| p.to_string()).unwrap_or_else(|| "null".to_string()),
                overrides.port_max.map(|p| p.to_string()).unwrap_or_else(|| "null".to_string()),
            ));
        }
        out.push_str("  ");
    }
    out.push_str("}\n}");
    out
}

/// Plantilla de modelo embebida como referencia.
///
/// Se usa como último recurso si el catálogo no tiene `_template.toml`.
pub const PLANTILLA_MODELO: &str = include_str!("../../../models/_template.toml");

/// Escapa un texto para que valga dentro de un JSON.
///
/// Los acentos y la `ñ` **no** se escapan: son caracteres normales de un
/// texto en español (y el JSON los admite tal cual).
pub fn escapa_json(texto: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
