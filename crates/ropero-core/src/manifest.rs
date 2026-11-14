//! El manifiesto de motor: `engines/<id>.toml` (doc B §4.3).
//!
//! Un motor es **un fichero de datos**. Este módulo es el tipo que lo
//! deserializa, lo valida y decide —sin tocar el disco— si un motor es
//! compatible con una máquina.
//!
//! ## Lo que el manifiesto declara
//!
//! | Campo | Qué es |
//! |---|---|
//! | `id` | El id canónico del motor (ADR-004). |
//! | `label` | Cómo se enseña el motor en los listados. |
//! | `color` | El color de la leyenda (doc CLI v2 §4). |
//! | `description` | Qué es el motor, en una línea. |
//! | `maturity` | Si el motor está probado o no (R-CTX-3). |
//! | `formats` | Los formatos de modelo que acepta (doc CLI v2 §4). |
//! | `port_min`/`port_max` | Su rango de puertos (doc CLI v2 §5.1). |
//! | `deps` | Los comandos externos de los que depende (`ropero doctor`). |
//! | `detect` | Cómo se sabe que está instalado. |
//! | `compat` | Las reglas de SO y hardware. |
//!
//! ## Lo que aquí NO hay
//!
//! `ropero-core` es dominio puro: no lee disco, no consulta el entorno, no
//! lanza procesos (doc B §3.2.3). Por eso el manifiesto se **parsea desde su
//! texto** ([`parse_engine_manifest`]) y la compatibilidad se **evalúa**
//! contra un [`SystemInfo`] que entra por parámetro —lo captura la CLI, que
//! es quien sabe leer `/proc/meminfo` o `sysctl`.

use serde::{Deserialize, Serialize};

use crate::engine::EngineId;

/// El color de un motor en los listados (doc CLI v2 §4).
///
/// Es un enum cerrado: la leyenda del doc fija ocho colores y un color
/// fuera de la leyenda es un error de datos, no un texto libre.
///
/// Los nombres son los **exactos** de la leyenda, tildes incluidas: el doc
/// escribe `verde-pálido`, no `verde-palido`. Por eso el `rename` es
/// explícito en vez de dejar que lo derive `kebab-case`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EngineColor {
    #[default]
    #[serde(rename = "sin-color")]
    SinColor,
    #[serde(rename = "naranja")]
    Naranja,
    #[serde(rename = "amarillo")]
    Amarillo,
    #[serde(rename = "rojo")]
    Rojo,
    #[serde(rename = "gris")]
    Gris,
    #[serde(rename = "blanco")]
    Blanco,
    #[serde(rename = "verde-pistacho")]
    VerdePistacho,
    #[serde(rename = "morado")]
    Morado,
    #[serde(rename = "verde-pálido")]
    VerdePalido,
}

impl EngineColor {
    /// El nombre del color, tal y como lo escribe el manifiesto.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SinColor => "sin-color",
            Self::Naranja => "naranja",
            Self::Amarillo => "amarillo",
            Self::Rojo => "rojo",
            Self::Gris => "gris",
            Self::Blanco => "blanco",
            Self::VerdePistacho => "verde-pistacho",
            Self::Morado => "morado",
            Self::VerdePalido => "verde-pálido",
        }
    }
}

impl std::fmt::Display for EngineColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// La madurez de un motor (R-CTX-3, doc B §3.3.1).
///
/// La distinción que importa es **probado / no probado**: `ropero install
/// engines` y `ropero doctor` la enseñan, para que la especificación no
/// finja que todos los motores están igual de probados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Maturity {
    /// Probado en la máquina del dueño.
    Proven,
    /// Integrado pero nunca arrancado.
    #[default]
    Integrated,
    /// A medio integrar.
    Partial,
}

impl Maturity {
    /// El nombre de la madurez, tal y como lo escribe el manifiesto.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Proven => "proven",
            Self::Integrated => "integrated",
            Self::Partial => "partial",
        }
    }

    /// Si el motor está **probado** (arrancado de verdad).
    ///
    /// Es la distinción que T-14 exige que se vea: `proven` es la única
    /// madurez probada.
    pub fn es_probado(&self) -> bool {
        matches!(self, Self::Proven)
    }
}

impl std::fmt::Display for Maturity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// El sistema operativo de la máquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Os {
    #[default]
    Linux,
    Macos,
    Windows,
}

impl Os {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
        }
    }

    /// El nombre del SO en español, para los mensajes.
    pub fn nombre(&self) -> &'static str {
        match self {
            Self::Linux => "Linux",
            Self::Macos => "macOS",
            Self::Windows => "Windows",
        }
    }
}

impl std::fmt::Display for Os {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.nombre())
    }
}

/// El rango de puertos de un motor (doc CLI v2 §5.1).
///
/// Cada motor tiene el suyo para que varios puedan servir a la vez sin
/// pisarse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PortRange {
    pub min: u16,
    pub max: u16,
}

impl PortRange {
    /// Construye un rango.
    ///
    /// Un rango **invertido** (`min > max`) se conserva tal cual: la
    /// validación del conjunto lo rechaza con un mensaje que lo dice. No se
    /// "arregla" exchanging los extremos, porque entonces el error se
    /// escondería en vez de reportarse.
    pub fn new(min: u16, max: u16) -> Self {
        Self { min, max }
    }

    /// Si el puerto cae dentro del rango.
    pub fn contains(&self, port: u16) -> bool {
        port >= self.min && port <= self.max
    }

    /// Si dos rangos se solapan.
    pub fn se_solapan(&self, otro: &Self) -> bool {
        self.min <= otro.max && otro.min <= self.max
    }
}

impl std::fmt::Display for PortRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.min, self.max)
    }
}

/// Cómo se sabe que un motor está instalado.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectSpec {
    /// Los binarios que, si están en el `PATH`, dicen que el motor está.
    #[serde(default)]
    pub binaries: Vec<String>,

    /// Las rutas donde puede estar instalado.
    #[serde(default)]
    pub paths: Vec<String>,
}

/// Una regla de compatibilidad (doc B §3.3.2).
///
/// Cada regla dice **una** cosa: el SO, la NVIDIA, el Metal, la NPU AMD o
/// la RAM. Varias reglas se acumulan (un `Vec`), y todas se evalúan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatRule {
    /// El SO (o los SOs) en el que el motor funciona.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<Vec<Os>>,

    /// Requiere GPU NVIDIA con `nvidia-smi` presente.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nvidia_smi: bool,

    /// Requiere Apple Silicon con Metal.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub metal: bool,

    /// Requiere NPU AMD XDNA.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub amd_xnpu: bool,

    /// Requiere un mínimo de RAM (GB).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_ram_gb: Option<f64>,
}

impl CompatRule {
    /// El SO (o los SOs) de la regla, si la regla lo dice.
    pub fn os(&self) -> Option<&[Os]> {
        self.os.as_deref()
    }

    /// Si la regla exige GPU NVIDIA.
    pub fn nvidia_smi(&self) -> bool {
        self.nvidia_smi
    }

    /// Si la regla exige Metal.
    pub fn metal(&self) -> bool {
        self.metal
    }

    /// Si la regla exige NPU AMD.
    pub fn amd_xnpu(&self) -> bool {
        self.amd_xnpu
    }
}

/// El manifiesto de un motor (doc B §4.3).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EngineManifest {
    /// El id canónico del motor (ADR-004).
    pub id: EngineId,

    /// Cómo se enseña el motor en los listados.
    pub label: String,

    /// El color de la leyenda (doc CLI v2 §4).
    #[serde(default)]
    pub color: EngineColor,

    /// Qué es el motor, en una línea.
    #[serde(default)]
    pub description: String,

    /// Si el motor está probado o no (R-CTX-3).
    #[serde(default)]
    pub maturity: Maturity,

    /// Los formatos de modelo que el motor acepta (doc CLI v2 §4).
    #[serde(default)]
    pub formats: Vec<String>,

    /// El rango de puertos del motor (doc CLI v2 §5.1).
    pub port_range: PortRange,

    /// Los comandos externos de los que depende (`ropero doctor`).
    #[serde(default)]
    pub deps: Vec<String>,

    /// Cómo se sabe que el motor está instalado.
    #[serde(default)]
    pub detect: DetectSpec,

    /// Las reglas de SO y hardware.
    #[serde(default)]
    pub compat: Vec<CompatRule>,
}

/// El manifiesto tal y como viene en el TOML: el rango de puertos son dos
/// campos sueltos (`port_min` y `port_max`), no una tabla.
///
/// Se deserialize en dos pasos: primero el TOML plano, luego se arma el
/// [`EngineManifest`] con el rango ya construido. Así el TOML se ve como lo
/// escribió el dueño (`port_min = 8100`, `port_max = 8199`) y el tipo del
/// dominio es el que conviene.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct EngineManifestFlat {
    id: EngineId,
    label: String,
    #[serde(default)]
    color: EngineColor,
    #[serde(default)]
    description: String,
    #[serde(default)]
    maturity: Maturity,
    #[serde(default)]
    formats: Vec<String>,
    port_min: u16,
    port_max: u16,
    #[serde(default)]
    deps: Vec<String>,
    #[serde(default)]
    detect: DetectSpec,
    #[serde(default)]
    compat: Vec<CompatRule>,
}

/// El error de carga de un manifiesto.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineManifestError {
    /// El TOML no se puede deserializar (sintaxis, tipo o campo desconocido).
    Parse {
        path: std::path::PathBuf,
        line: Option<usize>,
        message: String,
    },
    /// El `id` no es un motor conocido.
    UnknownEngine {
        path: std::path::PathBuf,
        name: String,
    },
}

impl std::fmt::Display for EngineManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse {
                path,
                line,
                message,
            } => {
                write!(f, "{}:{}: {}", path.display(), line.unwrap_or(0), message)
            }
            Self::UnknownEngine { path, name } => {
                write!(f, "{}: motor desconocido: '{}'", path.display(), name)
            }
        }
    }
}

impl std::error::Error for EngineManifestError {}

/// Parsea el TOML de un manifiesto de motor.
///
/// `source` es la ruta de origen: sólo se usa para los mensajes. No se lee
/// ningún fichero aquí: el texto llega como `&str` (el core no hace E/S).
///
/// El `id` se valida **antes** de deserializar, para que el mensaje sea el
/// de "motor desconocido" y no el genérico de `serde`.
pub fn parse_engine_manifest(
    toml_text: &str,
    source: &std::path::Path,
) -> Result<EngineManifest, EngineManifestError> {
    let crudo: toml::Value =
        toml::from_str(toml_text).map_err(|err: toml::de::Error| EngineManifestError::Parse {
            path: source.to_path_buf(),
            line: crate::parse_error_line(toml_text, &err),
            message: err.message().to_string(),
        })?;
    if let Some(id) = crudo.get("id").and_then(|v| v.as_str())
        && EngineId::from_str_legacy(id).is_err()
    {
        return Err(EngineManifestError::UnknownEngine {
            path: source.to_path_buf(),
            name: id.to_string(),
        });
    }
    let plano: EngineManifestFlat =
        toml::from_str(toml_text).map_err(|err: toml::de::Error| EngineManifestError::Parse {
            path: source.to_path_buf(),
            line: crate::parse_error_line(toml_text, &err),
            message: err.message().to_string(),
        })?;
    Ok(EngineManifest {
        id: plano.id,
        label: plano.label,
        color: plano.color,
        description: plano.description,
        maturity: plano.maturity,
        formats: plano.formats,
        port_range: PortRange::new(plano.port_min, plano.port_max),
        deps: plano.deps,
        detect: plano.detect,
        compat: plano.compat,
    })
}

/// El nombre de fichero de un manifiesto de motor: `<id>.toml`.
///
/// El nombre del fichero es el id del motor: un manifiesto en
/// `engines/otro.toml` que diga `id = "llama-server"` es un error de
/// datos (la validación del conjunto lo detecta).
pub fn manifest_file_name(id: &EngineId) -> String {
    format!("{}.toml", id.as_str())
}

/// El nombre del directorio donde viven los manifiestos: `engines`.
pub const ENGINES_DIR_NAME: &str = "engines";

/// Valida un conjunto de manifiestos.
///
/// Devuelve los errores **acumulados**: uno por problema, con el mensaje en
/// español. Un conjunto vacío de problemas significa que el conjunto es
/// válido.
pub fn validate_engine_set(manifiestos: &[EngineManifest]) -> Vec<String> {
    let mut errores = Vec::new();
    let mut ids_vistos: Vec<&str> = Vec::new();
    for m in manifiestos {
        if m.label.trim().is_empty() {
            errores.push(format!("motor '{}': la etiqueta está vacía", m.id.as_str()));
        }
        if m.color == EngineColor::SinColor {
            errores.push(format!("motor '{}': falta el color", m.id.as_str()));
        }
        if m.description.trim().is_empty() {
            errores.push(format!("motor '{}': falta la descripción", m.id.as_str()));
        }
        if m.formats.is_empty() {
            errores.push(format!("motor '{}': no declara formatos", m.id.as_str()));
        }
        if m.deps.is_empty() {
            errores.push(format!(
                "motor '{}': no declara dependencias (las necesita el doctor)",
                m.id.as_str()
            ));
        }
        if m.detect.binaries.is_empty() && m.detect.paths.is_empty() {
            errores.push(format!(
                "motor '{}': no dice cómo detectarlo (detect.binaries / detect.paths)",
                m.id.as_str()
            ));
        }
        if m.port_range.min > m.port_range.max {
            errores.push(format!(
                "motor '{}': el rango de puertos está invertido ({} > {})",
                m.id.as_str(),
                m.port_range.min,
                m.port_range.max
            ));
        }
        if ids_vistos.contains(&m.id.as_str()) {
            errores.push(format!(
                "motor duplicado: '{}' aparece más de una vez",
                m.id.as_str()
            ));
        }
        ids_vistos.push(m.id.as_str());
    }
    // Los rangos de puertos no se solapan: dos motores no pueden pelearse
    // por el mismo puerto.
    for (i, a) in manifiestos.iter().enumerate() {
        for b in manifiestos.iter().skip(i + 1) {
            if a.port_range.se_solapan(&b.port_range) {
                errores.push(format!(
                    "los motores '{}' y '{}' comparten el rango de puertos {} (empieza en {})",
                    a.id.as_str(),
                    b.id.as_str(),
                    a.port_range,
                    a.port_range.min
                ));
            }
        }
    }
    errores
}
