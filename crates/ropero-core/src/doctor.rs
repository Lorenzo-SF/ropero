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
//! ## La forma
//!
//! ```text
//! DEPENDENCIAS DE ROPERO
//!   dependencia   estado   qué hacer
//!   ───────────────────────────────────────────────
//!   ropero        ok
//!   git           falta    instálalo con tu gestor de paquetes
//!
//! DEPENDENCIAS DE LOS MOTORES
//!   dependencia   estado   qué hacer
//!   ───────────────────────────────────────────────
//!   llama-server (instalado)
//!   cmake         ok
//!   ───────────────────────────────────────────────
//!   vllm (no instalado)
//!   uv            falta    instálalo con tu gestor de paquetes
//! ```
//!
//! La tabla de los motores es **una**: el motor es una **fila de grupo** y
//! una regla lo separa del siguiente. La cabecera sale **una sola vez**:
//! repetirla por motor(recargaba la salida y la partía en varias tablas).
//!
//! ## La salida
//!
//! El doctor **no** decide el código de salida: dice si todo está bien
//! ([`DoctorReport::todo_ok`]) y la CLI decide el 0 o el 1.

use crate::EngineManifest;
use crate::estilo::{Anchos, Celda, Columna, Seccion, Tabla};
use crate::palette::Palette;

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
    /// El estado de la dependencia: `ok` o `falta`.
    ///
    /// Es el **vocabulario** del doctor: el mismo en todas las salidas.
    pub fn estado(&self) -> &'static str {
        match self.status {
            DepStatus::Ok => "ok",
            DepStatus::Missing => "falta",
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
    ///
    /// El nombre del motor va en el color de la leyenda (doc CLI v2 §4) y
    /// las dependencias van en la tabla del sistema de formato (§2). Las
    /// tablas **comparten** los anchos: todas tienen las mismas columnas.
    ///
    /// La tabla de los motores es **una**: el motor es una fila de grupo y
    /// una regla lo separa del siguiente. La cabecera sale **una sola vez**
    /// (por sección): repetirla por motor(recargaba la salida y la partía
    /// en varias tablas).
    pub fn lines(&self, paleta: &Palette) -> Vec<String> {
        // Las dos tablas: la de ropero y la de los motores.
        let tabla_ropero = tabla_de_dependencias(&self.ropero, 1);
        let tabla_motores = tabla_de_motores(&self.engines, 1);
        let anchos = Anchos::combinado(&[&tabla_ropero, &tabla_motores]);

        let mut out: Vec<String> = Vec::new();
        // Las dependencias de ropero.
        let mut seccion = Seccion::nueva("DEPENDENCIAS DE ROPERO");
        seccion.cuerpo(tabla_ropero.lineas_con(&anchos, paleta));
        out.extend(seccion.lineas());
        out.push(String::new());
        // Las de cada motor, en el orden canónico.
        let mut seccion = Seccion::nueva("DEPENDENCIAS DE LOS MOTORES");
        seccion.cuerpo(tabla_motores.lineas_con(&anchos, paleta));
        out.extend(seccion.lineas());
        while out.last().is_some_and(|l| l.is_empty()) {
            out.pop();
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

/// La tabla de las dependencias: el nombre, el estado y qué hacer.
///
/// La columna de **qué hacer** sólo se llena cuando falta algo: es la
/// pista (P-13), y no tiene sentido repetirla cuando la dependencia está.
fn tabla_de_dependencias(comprobaciones: &[DepCheck], nivel: usize) -> Tabla {
    let mut tabla = Tabla::nivel(nivel)
        .columna(Columna::texto("dependencia"))
        .columna(Columna::texto("estado"))
        .columna(Columna::texto("qué hacer"));
    for check in comprobaciones {
        tabla.fila(celdas_de_dependencia(check));
    }
    tabla
}

/// La tabla de los motores: **una** tabla con el motor como fila de grupo.
///
/// El nombre del motor va en el color de la leyenda (§4) y su estado
/// ("instalado") detrás, fuera del color. Sus dependencias van debajo, en la
/// misma rejilla, y una regla separa un motor del siguiente.
fn tabla_de_motores(motores: &[EngineDeps], nivel: usize) -> Tabla {
    let mut tabla = Tabla::nivel(nivel)
        .columna(Columna::texto("dependencia"))
        .columna(Columna::texto("estado"))
        .columna(Columna::texto("qué hacer"));
    for motor in motores {
        let estado = if motor.installed {
            "instalado"
        } else {
            "no instalado"
        };
        tabla.grupo(Celda::motor_con_sufijo(
            motor.manifest.id.as_str(),
            format!(" ({estado})"),
            motor.manifest.color,
        ));
        if motor.checks.is_empty() {
            let nota = if motor.manifest.deps.is_empty() {
                "(sin dependencias)"
            } else {
                "(dependencias no aplicables en este equipo)"
            };
            tabla.nota(Celda::plana(nota));
            continue;
        }
        for check in &motor.checks {
            tabla.fila(celdas_de_dependencia(check));
        }
    }
    tabla
}

/// Las celdas de una dependencia: el nombre, el estado y qué hacer.
fn celdas_de_dependencia(check: &DepCheck) -> Vec<Celda> {
    let pista = match check.status {
        DepStatus::Ok => String::new(),
        DepStatus::Missing => check.dependency.remedy.clone(),
    };
    vec![
        Celda::plana(check.dependency.name.clone()),
        Celda::plana(check.estado()),
        Celda::plana(pista),
    ]
}
