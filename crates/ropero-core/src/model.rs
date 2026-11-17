//! Schema de catalogación de modelos: la TOML de catalogación (`ropero-code/models/…toml`)
//! deserealizan in `ModelSpec` y validan in the rules documented in docs §3.2.1 → §3.2.3
//! y §4.1.1.
//!
//! `parse()` inphase the phase of parseado deserealizan via `toml::from_str`.
//! `validate()` inphase the phase of validan puro (no I/O, no process): return
//! `Vec<ModelError>` with the rules in §4.1.1.
//!
//! ## Correspondent `modelconf.py` (ropero-code/bin/python/modelconf.py)
//!
//! `ModelSpec` correspond `spec` dict in `check_model()`; `Files` correspond
//! `FILE_KEYS` (`modelconf.py:84`) and `Levels`/`Flags`/`Options`/`Limits`/`Args`
//! correspond `[levels]`/`[flags]`/`[options]`/`[limits]`/`[args]` in TOML
//! and in `validate_model()` in Python.
//!
//! ## Schema contract
//!
//! Every `struct` has `#[serde(deny_unknown_fields)]`: key not defined in the
//! schema (typo or key not used) → error of parseado of TOML (`toml::TomlError`)
//! and never deserealizan silently. The tokens `engine` and `format` are enums
//! (`EngineId`, `ModelFormat`) then the schema of `deny_unknown_fields` chequedan
//! `engine` in the phase of parseado, and `format` (optional) que chequedan
//! `format` in `validate()` (R-DATA-16, exit 5).
//!
//! ### Alias legacy de `engine`
//!
//! `EngineId` sólo acepta los identificadores canónicos (`llama-server`,
//! `turbo-llama`, `mlx-serve`, `fastflowllm`, `tensorrt-llm`, `vllm`,
//! `airllm`, `strata`). Los alias legacy (`llama-server-official`,
//! `llama-official`, `llama`, `turbo`, `mlx_lm`, `flm`, `tensorrt`,
//! `strata_engine`) los canonicaliza `parse_model` antes de deserializar, y
//! dejan un aviso en [`ModelSpec::notices`] (doc B §4.2):
//! un alias legacy se canonicaliza **con un aviso**, nunca en silencio.
//!
//! La nomenclatura canónica es la de [ADR-004]: `llama-server` es el repo
//! oficial de llama.cpp y `turbo-llama` el fork privado.
//!
//! ### Types contract
//!
//! Every `struct` has the field `#[serde(default)]` que is not obligatoria in TOML
//! (example: `format` default `gguf`, `limits` default empty) y que is not
//! obligatoria in the contract de §3.2.1.
//!
//! ## Notación
//!
//! - `limits.min_ram_gb` is `Option<f64>` (opcional: si no se declara, no se
//!   comprueba) y `limits.min_ram_note` is `String`.
//! - `download.aux` is `Vec<Aux>`.
//! - `chat_template_kwargs` is `BTreeMap<String, toml::Value>` (value can be
//!   `bool` in `enable_thinking` o `string` in `reasoning_effort`), not a scalar.
//! - `Level.level` is `String` (`low`, `think`, `xhigh`) not `u32`.
//! - `Aliases` in TOML has key `aliases` and `Limits` in TOML has key `limits`.
//! - `ModelSpec.source` (the file of `path` where `ModelSpec` came from) has
//!   `#[serde(default)]`: in the TOML has not a key `source` obligatoria, y que
//!   `parse()` setea.
//! - The table `[args]` is `ModelSpec.args` (in `struct Args`) y `extra_args`
//!   has no TOML key: `args` in TOML only is the table `[args]`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use toml::Value;

use crate::engine::EngineId;
use crate::format::ModelFormat;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Files {
    #[serde(default)]
    pub gguf: Option<PathBuf>,

    #[serde(default)]
    pub mtp: Option<PathBuf>,

    #[serde(default)]
    pub mmproj: Option<PathBuf>,

    #[serde(default)]
    pub chat_template: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aux {
    #[serde(default)]
    pub file: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub repo: String,

    #[serde(default)]
    pub size: String,

    #[serde(rename = "use")]
    #[serde(default)]
    pub use_: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Download {
    #[serde(default)]
    pub repo: String,

    #[serde(default)]
    pub gguf: bool,

    #[serde(default)]
    pub file: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub size: String,

    #[serde(default)]
    pub manual: bool,

    #[serde(default)]
    pub excludes: Vec<String>,

    #[serde(default)]
    pub aux: BTreeMap<String, Aux>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    #[serde(default)]
    pub repo: String,

    #[serde(default)]
    pub gguf: bool,

    #[serde(default)]
    pub gguf_file: String,

    #[serde(default)]
    pub quant: String,

    #[serde(default)]
    pub imatrix: String,

    #[serde(default)]
    pub imatrix_repo: String,

    #[serde(default)]
    pub imatrix_file: String,

    #[serde(default)]
    pub out: String,

    #[serde(default)]
    pub out_principal: String,

    #[serde(default)]
    pub from_subdir: bool,

    #[serde(default)]
    pub include: Vec<String>,

    #[serde(default)]
    pub intermedio: String,

    #[serde(default)]
    pub keep_source: bool,

    #[serde(default)]
    pub keep_intermedio: bool,

    #[serde(default)]
    pub no_mtp: bool,

    #[serde(default)]
    pub force: bool,

    #[serde(default)]
    pub size: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    #[serde(default)]
    pub base: Vec<String>,

    #[serde(default)]
    #[serde(rename = "chat_template_kwargs")]
    pub chat_template_kwargs: BTreeMap<String, Value>,

    #[serde(default)]
    pub reasoning_effort: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlagsSpec {
    #[serde(default)]
    pub list: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionsSpec {
    #[serde(default)]
    pub list: Vec<String>,

    #[serde(default)]
    /// Valor por defecto para cada opción declarada en `list`.
    pub default: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelSpec {
    #[serde(default)]
    pub list: Vec<String>,

    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// La RAM mínima del modelo, en GiB. Es **opcional**: si no se declara,
    /// no se comprueba nada (como en la v1, que sólo mira la clave cuando
    /// está presente: `ropero-code/bin/python/modelconf.py:1560`).
    ///
    /// Cuando **sí** se declara tiene que ser `> 0` (R-DATA-9): por eso es
    /// `Option<f64>` y no `f64` — con un `f64` a cero no se distinguiría
    /// "no declarado" de "declarado con 0".
    #[serde(default)]
    pub min_ram_gb: Option<f64>,

    #[serde(default)]
    pub min_ram_note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlagArg {
    #[serde(default)]
    pub base: Vec<String>,

    #[serde(default)]
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionArg {
    #[serde(default)]
    pub base: Vec<String>,

    #[serde(default)]
    pub values: BTreeMap<String, Vec<String>>,

    /// Token del `[args].base` que esta opción **sustituye** en vez de
    /// añadirse detrás (doc B §3.2.3, R-DATA-16).
    ///
    /// *"[args.option.temp] base = ["--temp", "${value}"] con
    /// `replace = "--temp"` elimina la pareja `--temp 0.7` del base y pone
    /// `--temp <valor>` en su lugar."*
    ///
    /// Sólo lo declara la v1 para `[args.option.<o>]`
    /// (`ropero-code/models/_template.toml:124-129`); los flags no lo
    /// admiten.
    #[serde(default)]
    pub replace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Args {
    #[serde(default)]
    pub base: Vec<String>,

    #[serde(default)]
    pub level: BTreeMap<String, Level>,

    #[serde(default)]
    pub flag: BTreeMap<String, FlagArg>,

    #[serde(default)]
    pub option: BTreeMap<String, OptionArg>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelMeta {
    pub name: String,

    pub engine: EngineId,

    #[serde(default)]
    pub context: u32,

    #[serde(default)]
    pub alias: Option<String>,

    /// Puerto pineado del modelo (`[model].port`). Si no se declara, el
    /// runtime usa el slot que toque.
    #[serde(default)]
    pub port: Option<crate::Port>,

    #[serde(default)]
    pub format: ModelFormat,

    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSpec {
    #[serde(rename = "model")]
    pub model: ModelMeta,

    #[serde(rename = "files")]
    pub files: Files,

    #[serde(default)]
    #[serde(rename = "download")]
    pub download: Option<Download>,

    #[serde(default)]
    #[serde(rename = "build")]
    pub build: Option<Build>,

    #[serde(default)]
    #[serde(rename = "levels")]
    pub levels: LevelSpec,

    #[serde(default)]
    #[serde(rename = "flags")]
    pub flags: FlagsSpec,

    #[serde(default)]
    #[serde(rename = "options")]
    pub options: OptionsSpec,

    #[serde(default)]
    #[serde(rename = "aliases")]
    pub aliases: BTreeMap<String, String>,

    #[serde(default)]
    #[serde(rename = "limits")]
    pub limits: Limits,

    #[serde(default)]
    #[serde(rename = "args")]
    pub args: Args,

    #[serde(default)]
    pub source: PathBuf,

    /// Avisos que emitió el parseo del TOML.
    ///
    /// Hoy, uno solo: la canonicalización de un alias legacy de motor (doc B
    /// §4.2). Core no imprime: los avisos viajan con el modelo y la capa de
    /// presentación los enseña al usuario.
    #[serde(skip)]
    pub notices: Vec<String>,

    /// Nombre real del modelo: el de `[model].name` en su TOML de origen.
    ///
    /// Lo fija `parse_model`. Sirve para distinguir la fila original de la
    /// fila de un alias (que lleva `model.name` = alias) sin volver a leer
    /// el fichero.
    #[serde(skip)]
    pub real_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetaKey {
    #[serde(default)]
    pub field: String,

    #[serde(default)]
    pub model: PathBuf,
}

pub const KEBAW_CASE_MAX: usize = 40;

pub const MAX_NAME_LEN: usize = 40;
