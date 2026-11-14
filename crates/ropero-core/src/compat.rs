//! La evaluación de compatibilidad de un motor (doc B §3.3.2, T-13).
//!
//! ## Por qué acumula **todas** las razones
//!
//! Un motor incompatible explica **todas** las razones por las que no
//! funciona, no la primera. El motivo es el del doc: el usuario que está en
//! Linux y quiere instalar TensorRT-LLM necesita saber que le faltan *dos*
//! cosas (una NVIDIA y 32 GB de RAM), no una por intercourse. Cortar en la
//! primera razón obligaría al usuario a reinstalar y volver a preguntar para
//! descubrir la siguiente.
//!
//! ## El core no pregunta nada
//!
//! `ropero-core` es dominio puro: no lee disco, no consulta el entorno, no
//! lanza procesos (doc B §3.2.3). Por eso la información de la máquina
//! ([`SystemInfo`]) entra **por parámetro**: la captura en la CLI, que es
//! quien sabe leer `/proc/meminfo`, `nvidia-smi` o `sysctl`.

use crate::manifest::{CompatRule, EngineManifest, Os};

/// La información de la máquina, ya capturada.
///
/// La captura ocurre fuera del core (en la CLI): aquí sólo llega como dato.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemInfo {
    /// El sistema operativo.
    pub os: Os,
    /// La arquitectura (`x86_64`, `aarch64`…).
    pub arch: String,
    /// Si hay GPU NVIDIA (`nvidia-smi` contesta).
    pub tiene_nvidia: bool,
    /// Si hay Metal (macOS ARM).
    pub tiene_metal: bool,
    /// Si hay NPU AMD XDNA.
    pub tiene_npu_amd: bool,
    /// La RAM total, en GB.
    pub ram_total_gb: f64,
}

impl Default for SystemInfo {
    fn default() -> Self {
        SystemInfo {
            os: Os::Linux,
            arch: String::new(),
            tiene_nvidia: false,
            tiene_metal: false,
            tiene_npu_amd: false,
            ram_total_gb: 0.0,
        }
    }
}

/// El motivo por el que un motor no es compatible con una máquina.
///
/// Cada razón es un dato (no un texto): el mensaje lo compone el dominio,
/// para que sea el mismo en la CLI, en el `doctor` y en los tests.
#[derive(Debug, Clone, PartialEq)]
pub enum Incompatibility {
    /// El SO de la máquina no está entre los admitidos.
    OsRequerido {
        /// Los SOs que el motor admite.
        admitidos: Vec<Os>,
    },
    /// Falta GPU NVIDIA.
    NvidiaRequerida,
    /// Falta Metal.
    MetalRequerido,
    /// Falta NPU AMD XDNA.
    NpuAmdRequerida,
    /// Falta RAM.
    RamInsuficiente {
        /// La RAM que hace falta, en GB.
        necesaria_gb: f64,
    },
}

impl std::fmt::Display for Incompatibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OsRequerido { admitidos } => {
                let lista = admitidos
                    .iter()
                    .map(|os| os.nombre())
                    .collect::<Vec<_>>()
                    .join(" o ");
                write!(f, "sólo funciona en {lista}")
            }
            Self::NvidiaRequerida => write!(f, "necesita una GPU NVIDIA"),
            Self::MetalRequerido => write!(f, "necesita Apple Silicon con Metal"),
            Self::NpuAmdRequerida => write!(f, "necesita una NPU AMD XDNA"),
            Self::RamInsuficiente { necesaria_gb } => {
                write!(f, "necesita al menos {necesaria_gb} GB de RAM")
            }
        }
    }
}

/// El veredicto de compatibilidad de un motor con una máquina.
#[derive(Debug, Clone, PartialEq)]
pub struct CompatReport {
    /// El motor evaluado.
    pub manifest: EngineManifest,
    /// Las razones de la incompatibilidad, **todas** (puede estar vacío si
    /// el motor es compatible).
    pub reasons: Vec<Incompatibility>,
}

impl CompatReport {
    /// Si el motor es compatible con la máquina.
    pub fn es_compatible(&self) -> bool {
        self.reasons.is_empty()
    }

    /// Las razones, en texto, una por línea.
    pub fn reasons_text(&self) -> Vec<String> {
        self.reasons.iter().map(|r| r.to_string()).collect()
    }
}

/// Evalúa la compatibilidad de un motor con una máquina.
///
/// **Acumula todas las razones**: no se para en la primera.
///
/// ## La semántica de las reglas
///
/// Las reglas de un manifiesto son **alternativas**: la máquina vale si
/// cumple **al menos una**. Por eso un motor que funciona en Linux y en
/// macOS declara
///
/// ```toml
/// compat = [ { os = ["linux"] }, { os = ["macos"] } ]
/// ```
///
/// y no una sola regla con los dos SOs: así se puede añadir una tercera
/// vía (un Windows, una máquina con Metal…) sin reescribir las anteriores.
///
/// Dentro de **una** regla, las condiciones son **acumulativas**: una regla
/// que pide Linux *y* NVIDIA exige las dos.
///
/// ## Cómo se acumulan las razones
///
/// Se evalúa **cada** alternativa por separado. Las alternativas que
/// encajan no aportan razones; de las que no encajan, se acumulan **todos**
/// sus motivos.
///
/// La diferencia con "acumular todo y parar en la primera" se ve en un motor
/// con dos vías (Linux, o macOS con Metal) en una máquina Windows sin Metal:
/// la vía de Linux falla por el SO y la de macOS falla por el SO y por
/// Metal, así que el usuario ve las tres razones de una vez.
pub fn evaluate_compat(manifest: &EngineManifest, system: &SystemInfo) -> CompatReport {
    if manifest.compat.is_empty() {
        // Sin reglas, el motor es compatible con cualquier máquina.
        return CompatReport {
            manifest: manifest.clone(),
            reasons: Vec::new(),
        };
    }
    // Cada alternativa se evalúa por separado.
    let mut alternativas: Vec<Vec<Incompatibility>> = Vec::with_capacity(manifest.compat.len());
    for rule in &manifest.compat {
        let mut motivos = Vec::new();
        evalua_regla(rule, system, &mut motivos);
        alternativas.push(motivos);
    }
    // Si alguna alternativa encaja, el motor es compatible: no hay razones.
    if alternativas.iter().any(|motivos| motivos.is_empty()) {
        return CompatReport {
            manifest: manifest.clone(),
            reasons: Vec::new(),
        };
    }
    // Ninguna alternativa encaja: se acumulan los motivos de todas, sin
    // repetir los que son el mismo (el SO, por ejemplo, falla en las dos).
    let mut reasons: Vec<Incompatibility> = Vec::new();
    for motivos in alternativas {
        for motivo in motivos {
            if !reasons.contains(&motivo) {
                reasons.push(motivo);
            }
        }
    }
    CompatReport {
        manifest: manifest.clone(),
        reasons,
    }
}

/// Evalúa **una** regla y acumula sus razones.
fn evalua_regla(rule: &CompatRule, system: &SystemInfo, reasons: &mut Vec<Incompatibility>) {
    if let Some(admitidos) = &rule.os
        && !admitidos.contains(&system.os)
    {
        reasons.push(Incompatibility::OsRequerido {
            admitidos: admitidos.clone(),
        });
    }
    if rule.nvidia_smi && !system.tiene_nvidia {
        reasons.push(Incompatibility::NvidiaRequerida);
    }
    if rule.metal && !system.tiene_metal {
        reasons.push(Incompatibility::MetalRequerido);
    }
    if rule.amd_xnpu && !system.tiene_npu_amd {
        reasons.push(Incompatibility::NpuAmdRequerida);
    }
    if let Some(necesaria) = rule.min_ram_gb
        && system.ram_total_gb < necesaria
    {
        reasons.push(Incompatibility::RamInsuficiente {
            necesaria_gb: necesaria,
        });
    }
}

/// El informe de compatibilidad de un conjunto de motores.
#[derive(Debug, Clone, PartialEq)]
pub struct CompatSetReport {
    /// Los motores compatibles con la máquina.
    pub compatibles: Vec<CompatReport>,
    /// Los que no lo son, **con todas sus razones**.
    pub incompatibles: Vec<CompatReport>,
}

/// Evalúa un conjunto de motores contra una máquina.
///
/// Reparte los motores en compatibles e incompatibles. Cada incompatible
/// lleva **todas** sus razones (ver [`evaluate_compat`]).
pub fn evaluate_manifest_set(
    manifiestos: &[EngineManifest],
    system: &SystemInfo,
) -> CompatSetReport {
    let mut compatibles = Vec::new();
    let mut incompatibles = Vec::new();
    for manifest in manifiestos {
        let informe = evaluate_compat(manifest, system);
        if informe.es_compatible() {
            compatibles.push(informe);
        } else {
            incompatibles.push(informe);
        }
    }
    CompatSetReport {
        compatibles,
        incompatibles,
    }
}
