//! La salida de `ropero list`: los modelos **disponibles**, agrupados por
//! motor, con la leyenda de colores.
//!
//! ## La forma
//!
//! ```text
//!   llama-server
//!     modelo-alpha          4.2 GB      llama-server
//!
//!   vllm
//!     modelo-beta           1.1 GB      vllm
//! ```
//!
//! Un motor que no está instalado no se despliega: su grupo sale **plegado**
//! con un conteo y la razón, para que el usuario sepa que existe pero que
//! aquí no se puede usar.
//!
//! ## La estética
//!
//! Es **nueva** (doc CLI v2 §2): nada de lo que se copió de la v1. La
//! regla que se sigue es la del doc: alineación limpia, color sólo donde
//! aporta (el nombre del motor), y nada de ruido.

use std::collections::BTreeMap;

use crate::EngineColor;
use crate::EngineId;
use crate::palette::Palette;
use crate::snapshot::{EngineGroup, ModelState, Snapshot, available_models, engine_groups};

/// Una línea del listado, ya compuesta.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// La línea, tal y como sale por stdout.
    pub text: String,
}

/// Las líneas de `ropero list`.
///
/// `todos` es el `--all`: añade los modelos que **no** están disponibles
/// (marcados) y despliega los grupos plegados.
pub fn list_lines(
    snapshot: &Snapshot,
    todos: bool,
    palette: &Palette,
    razones: &BTreeMap<EngineId, String>,
) -> Vec<String> {
    let grupos = engine_groups(snapshot, todos, razones);
    let mut out: Vec<String> = Vec::new();
    for grupo in &grupos {
        if grupo.models.is_empty() {
            continue;
        }
        out.push(group_header(grupo, palette));
        for modelo in &grupo.models {
            out.push(model_line(modelo, grupo));
        }
        out.push(String::new());
    }
    // Los grupos plegados van al final: no interrumpen la lista de lo que
    // sí se puede usar.
    for grupo in grupos.iter().filter(|g| g.models.is_empty()) {
        out.push(collapsed_line(grupo, palette));
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

/// La cabecera de un grupo: el nombre del motor, en su color.
pub fn group_header(grupo: &EngineGroup, palette: &Palette) -> String {
    let nombre = palette.paint(grupo.manifest.color, grupo.manifest.id.as_str());
    format!("  {nombre}")
}

/// La línea de un modelo dentro de su grupo.
pub fn model_line(modelo: &ModelState, grupo: &EngineGroup) -> String {
    let mut extra = String::new();
    if let Some(alias) = &modelo.spec.model.alias {
        extra.push_str(&format!(" ({alias})"));
    }
    let descripcion = crate::catalog::model_description(&modelo.spec);
    let descripcion = if descripcion.is_empty() {
        "-".to_string()
    } else {
        descripcion
    };
    let marca = if modelo.presence == crate::snapshot::ModelPresence::Downloaded {
        ""
    } else {
        "  [no disponible]"
    };
    let _ = grupo;
    format!(
        "    {:<28}{:<12}{}{}",
        modelo.spec.model.name, descripcion, extra, marca
    )
}

/// La línea de un grupo **plegado**: el motor no está instalado.
///
/// Dice **cuántos** modelos tiene y **por qué** no se pueden usar. La razón
/// la aporta el informe de compatibilidad (F2), si el motor es incompatible
/// con la máquina; si sí lo es, la razón es que no está instalado.
pub fn collapsed_line(grupo: &EngineGroup, palette: &Palette) -> String {
    let nombre = palette.paint(grupo.manifest.color, grupo.manifest.id.as_str());
    let motivo = match &grupo.reason {
        Some(r) => r.clone(),
        None => "no está instalado".to_string(),
    };
    format!("  {nombre}  ({motivo})")
}

/// El resumen de una línea de `ropero list` cuando no hay nada que listar.
pub fn empty_notice() -> String {
    "aviso: no hay modelos disponibles (instala motores y descarga modelos)".to_string()
}

/// El resumen de `ropero list` cuando hay algo.
pub fn list_summary(snapshot: &Snapshot) -> String {
    let n = available_models(snapshot).len();
    format!("{n} modelo(s) disponible(s)")
}

/// El color de la leyenda de un motor.
pub fn color_de(grupo: &EngineGroup) -> EngineColor {
    grupo.manifest.color
}
