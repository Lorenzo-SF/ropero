//! La salida de los motores: la leyenda, como **texto puro**.
//!
//! La CLI imprime; el core decide **qué** se imprime (doc B §3.3). Aquí
//! viven las líneas de `ropero engines`: una por motor, con su color, su
//! madurez y si encaja con esta máquina.

use crate::compat::{CompatReport, SystemInfo, evaluate_manifest_set};
use crate::manifest::EngineManifest;

/// Una fila de la lista de motores.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineRow {
    /// El motor.
    pub manifest: EngineManifest,
    /// Si el motor es compatible con la máquina.
    pub compatible: bool,
    /// Las razones de la incompatibilidad (vacías si es compatible).
    pub reasons: Vec<String>,
}

/// Las filas de `ropero engines`: una por motor.
///
/// Los motores van en el orden de los ids canónicos (la leyenda del doc),
/// no en el orden del disco: el listado es estable.
///
/// `todos` sale si también se teach los que **no** son compatibles con la
/// máquina. Por defecto sólo salen los compatibles: es lo que R-CTX-1 pide
/// (`install engines` sólo ofrece los motores compatibles).
pub fn engine_rows(
    manifiestos: &[EngineManifest],
    sistema: &SystemInfo,
    todos: bool,
) -> Vec<EngineRow> {
    let informe = evaluate_manifest_set(manifiestos, sistema);
    let mut filas = Vec::new();
    for manifest in sorted_by_canonical_id(manifiestos) {
        let evaluado = informe
            .compatibles
            .iter()
            .chain(informe.incompatibles.iter())
            .find(|r| r.manifest.id == manifest.id);
        let Some(evaluado) = evaluado else {
            continue;
        };
        if !todos && !evaluado.es_compatible() {
            continue;
        }
        filas.push(EngineRow {
            manifest: manifest.clone(),
            compatible: evaluado.es_compatible(),
            reasons: evaluado.reasons_text(),
        });
    }
    filas
}

/// Las filas de motores, en el orden de los ids canónicos.
fn sorted_by_canonical_id(manifiestos: &[EngineManifest]) -> Vec<EngineManifest> {
    let mut ordenados: Vec<EngineManifest> = manifiestos.to_vec();
    ordenados.sort_by_key(|m| {
        crate::CANONICAL_IDS
            .iter()
            .position(|id| *id == m.id.as_str())
            .unwrap_or(usize::MAX)
    });
    ordenados
}

/// Las líneas de `ropero engines`.
///
/// El formato de una fila es:
///
/// ```text
///   llama-server    naranja         proven    8100-8199   GGUF
/// ```
///
/// y el de un motor incompatible añade las razones:
///
/// ```text
///   fastflowlm      rojo            proven    8400-8499   Q4NX   (no: sólo funciona en Linux; necesita una NPU AMD XDNA)
/// ```
pub fn engine_lines(
    manifiestos: &[EngineManifest],
    sistema: &SystemInfo,
    todos: bool,
) -> Vec<String> {
    engine_rows(manifiestos, sistema, todos)
        .iter()
        .map(engine_row_text)
        .collect()
}

/// La línea de una fila de motores.
pub fn engine_row_text(fila: &EngineRow) -> String {
    let m = &fila.manifest;
    let formatos = m.formats.join(", ");
    let mut linea = format!(
        "  {:<14} {:<15} {:<10} {:<10} {}",
        m.id.as_str(),
        m.color.as_str(),
        m.maturity.as_str(),
        m.port_range.to_string(),
        formatos
    );
    if !fila.compatible {
        let razones = fila.reasons.join("; ");
        linea.push_str(&format!("   (no: {razones})"));
    }
    linea
}

/// La línea de un motor incompatible, con **todas** sus razones.
///
/// Es el formato del `doctor`: una línea por motor, con todo lo que falta.
pub fn incompatible_lines(informe: &[CompatReport]) -> Vec<String> {
    informe
        .iter()
        .flat_map(|r| {
            r.reasons_text()
                .into_iter()
                .map(|razon| format!("  {}: {razon}", r.manifest.id.as_str()))
        })
        .collect()
}
