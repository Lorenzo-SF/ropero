//! La salida de `ropero doctor`: qué falta y **qué hacer**.
//!
//! ## El orden
//!
//! El doc CLI v2 §14 manda un orden:
//!
//! 1. Las **dependencias del propio ropero** (que el binario esté en el
//!    `PATH`, `hf`, `git`, `python3`…).
//! 2. Por **cada motor** — disponible o no — **sus dependencias**.
//!
//! ## El principio
//!
//! Una dependencia que falta no se reporta como un error seco: se dice
//! **qué hacer** con ella (P-13). "falta `hf`" no sirve de nada; "falta
//! `hf`: instálalo con `pipx install huggingface_hub`" sí.
//!
//! ## La salida
//!
//! El doctor **no** decide el código de salida: dice si todo está bien
//! ([`DoctorReport::todo_ok`]) y la CLI decide el 0 o el 1.

use crate::EngineManifest;

/// Una dependencia: qué es y cómo se llama.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// El nombre del comando (o del binario).
    pub name: String,
    /// Qué hacer cuando falta.
    pub remedy: String,
}

/// Las dependencias del propio ropero.
///
/// Son las que el doc CLI v2 §14 nombra, más las que el propio proyecto
/// necesita para funcionar.
pub const DEPENDENCIAS_ROPERO: &[(&str, &str)] = &[
    ("ropero", "añade el directorio del binario al PATH"),
    (
        "hf",
        "instálalo con `pipx install huggingface_hub` (o `pip install`)",
    ),
    ("git", "instálalo con tu gestor de paquetes"),
    ("python3", "instálalo con tu gestor de paquetes"),
];

/// La dependencia de ropero con el nombre `name`.
pub fn dependencia_ropero(name: &str) -> Dependency {
    let (_, remedy) = DEPENDENCIAS_ROPERO
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or(&("?", "instálalo con tu gestor de paquetes"));
    Dependency {
        name: name.to_string(),
        remedy: (*remedy).to_string(),
    }
}

/// El estado de una dependencia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepStatus {
    /// Está satisfied.
    Ok,
    /// Falta.
    Missing,
}

/// El resultado de comprobar una dependencia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepCheck {
    /// La dependencia.
    pub dependency: Dependency,
    /// Si está satisfied.
    pub status: DepStatus,
}

impl DepCheck {
    /// La línea del doctor para esta dependencia.
    pub fn line(&self) -> String {
        match self.status {
            DepStatus::Ok => format!("  ok       {}", self.dependency.name),
            DepStatus::Missing => format!(
                "  falta    {}: {}",
                self.dependency.name, self.dependency.remedy
            ),
        }
    }
}

/// El informe del doctor: lo que se comprobó.
#[derive(Debug, Clone, PartialEq)]
pub struct DoctorReport {
    /// Las dependencias del propio ropero.
    pub ropero: Vec<DepCheck>,
    /// Las dependencias de cada motor, en el orden canónico.
    pub engines: Vec<EngineDeps>,
}

/// Las dependencias de un motor.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineDeps {
    /// El motor.
    pub manifest: EngineManifest,
    /// Si el motor está instalado.
    pub installed: bool,
    /// Sus dependencias y su estado.
    pub checks: Vec<DepCheck>,
}

impl EngineDeps {
    /// Si al motor le falta alguna dependencia.
    pub fn falta_algo(&self) -> bool {
        self.checks.iter().any(|c| c.status == DepStatus::Missing)
    }
}

impl DoctorReport {
    /// Si todo está en su sitio.
    ///
    /// Es lo que decide el código de salida: 0 si es `true`, 1 si no.
    pub fn todo_ok(&self) -> bool {
        self.ropero.iter().all(|c| c.status == DepStatus::Ok)
            && self.engines.iter().all(|e| !e.falta_algo())
    }

    /// El número de comprobaciones que fallan.
    pub fn fallan(&self) -> usize {
        let ropero = self
            .ropero
            .iter()
            .filter(|c| c.status == DepStatus::Missing)
            .count();
        let motores = self
            .engines
            .iter()
            .map(|e| {
                e.checks
                    .iter()
                    .filter(|c| c.status == DepStatus::Missing)
                    .count()
            })
            .sum::<usize>();
        ropero + motores
    }

    /// Las líneas del informe, en el orden del doc.
    pub fn lines(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        out.push("DEPENDENCIAS DE ROPERO".to_string());
        for check in &self.ropero {
            out.push(check.line());
        }
        out.push(String::new());
        out.push("DEPENDENCIAS DE LOS MOTORES".to_string());
        for motor in &self.engines {
            let estado = if motor.installed {
                "instalado"
            } else {
                "no instalado"
            };
            out.push(format!("  {} ({estado})", motor.manifest.id.as_str()));
            if motor.checks.is_empty() {
                out.push("    (sin dependencias)".to_string());
            }
            for check in &motor.checks {
                out.push(format!("    {}", check.line()));
            }
        }
        out
    }

    /// El resumen final: qué falta y qué hacer.
    pub fn summary(&self) -> String {
        if self.todo_ok() {
            "todo en su sitio".to_string()
        } else {
            format!("{} dependencia(s) ausente(s)", self.fallan())
        }
    }
}
