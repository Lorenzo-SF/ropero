//! La salida de los motores: la leyenda, como **texto puro**.
//!
//! La CLI imprime; el core decide **qué** se imprime (doc B §3.3). Aquí
//! viven las líneas de `ropero engines`: una por motor, con su color, su
//! madurez y si encaja con esta máquina.
//!
//! ## La forma
//!
//! ```text
//! MOTORES
//!   motor          color        madurez     puertos     formatos
//!   ────────────────────────────────────────────────────────────
//!   llama-server   naranja      proven      8100-8199   GGUF
//!   fastflowlm     rojo         proven      8400-8499   Q4NX
//!     no: sólo funciona en Linux; necesita una NPU AMD XDNA
//! ```
//!
//! El motor que no encaja **no** pierde su fila: la deja con una nota
//! debajo, que dice por qué. La nota es una fila que ocupa toda la tabla:
//! no descuadra la rejilla (doc CLI v2 §2).

use crate::compat::{CompatReport, SystemInfo, evaluate_manifest_set};
use crate::estilo::{ANCHO_MAXIMO, Celda, Columna, Seccion, Tabla};
use crate::manifest::EngineManifest;
use crate::palette::Palette;

/// El título de la sección de los motores.
const TITULO: &str = "MOTORES";

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

/// Los motores en el orden de los ids canónicos (la leyenda del doc).
///
/// Es el mismo orden en todas las salidas: el listado de motores y el
/// informe del doctor enseñan los motores en la misma fila.
pub fn sorted_by_canonical_id(manifiestos: &[EngineManifest]) -> Vec<EngineManifest> {
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
/// La tabla es la del sistema de formato (doc CLI v2 §2): los puertos se
/// alinean a la derecha (son números) y el nombre del motor va en el color
/// de la leyenda (§4).
pub fn engine_lines(
    manifiestos: &[EngineManifest],
    sistema: &SystemInfo,
    todos: bool,
    paleta: &Palette,
) -> Vec<String> {
    let filas = engine_rows(manifiestos, sistema, todos);
    let mut tabla = Tabla::nivel(1)
        .columna(Columna::texto("motor").maximo(ANCHO_MAXIMO))
        .columna(Columna::texto("color"))
        .columna(Columna::texto("madurez"))
        .columna(Columna::numero("puertos"))
        .columna(Columna::texto("formatos").maximo(ANCHO_MAXIMO));
    for fila in &filas {
        tabla.fila(vec![
            Celda::motor(fila.manifest.id.as_str(), fila.manifest.color),
            Celda::plana(fila.manifest.color.as_str()),
            Celda::plana(fila.manifest.maturity.as_str()),
            Celda::plana(fila.manifest.port_range.to_string()),
            Celda::plana(fila.manifest.formats.join(", ")),
        ]);
        if !fila.compatible {
            let razones = fila.reasons.join("; ");
            tabla.nota(Celda::plana(format!("no: {razones}")));
        }
    }
    let mut seccion = Seccion::nueva(TITULO);
    seccion.cuerpo(tabla.lineas(paleta));
    seccion.lineas()
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
