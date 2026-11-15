//! La vista `ropero status`: el estado de la máquina, tipo `nvidia-smi`.
//!
//! ## Qué muestra
//!
//! | Recurso | Qué |
//! |---|---|
//! | CPU | GHz y temperatura |
//! | dGPU | GHz y temperatura (en macOS, la única GPU) |
//! | iGPU | GHz y temperatura (sólo Linux, si la hay) |
//! | RAM/SWAP | MB en uso, **contando zram** |
//!
//! ## Lo que no está, se dice
//!
//! Un recurso que no se puede leer **no se inventa**: sale `no disponible`.
//! Es la diferencia entre un `status` que miente y uno que informa.
//!
//! ## La forma
//!
//! ```text
//! ESTADO DE LA MÁQUINA
//!   recurso   reloj      temperatura
//!   ────────────────────────────────────────
//!   CPU       3.40 GHz   58.00 °C
//!   dGPU      2.10 GHz   45.00 °C
//!     NVIDIA GeForce RTX 5080
//!   iGPU      no disponible
//!
//!   recurso   uso         total
//!   ────────────────────────────────────────
//!   RAM       8192.00 MB  32768.00 MB
//!   SWAP      31698.00 MB 8192.00 MB
//!     (incluye 31186.00 MB de zram)
//! ```
//!
//! Son dos tablas (la de los procesadores y la de la memoria) con el
//! **mismo** sistema de formato del resto de la salida (doc CLI v2 §2):
//! los números a la derecha, el texto a la izquierda y el nombre de la
//! GPU y el zram como notas de su fila.
//!
//! ## La captura
//!
//! La lectura de `/proc/cpuinfo`, `/sys/class/thermal`, `nvidia-smi` y
//! `/proc/meminfo` es E/S, y vive en `ropero-runtime`. Aquí sólo vive la
//! **decisión** (qué se enseña y cómo se formatea), que es pura.

use std::fmt::Write as _;

use crate::estilo::{Celda, Columna, Seccion, Tabla};
use crate::palette::Palette;

/// Un recurso con su medida y su unidad.
///
/// Cuando no hay medida, el recurso **está ausente**: `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// El valor medido.
    pub value: f64,
    /// La unidad (`GHz`, `°C`, `MB`).
    pub unit: &'static str,
}

/// Una GPU, con su clocks y su temperatura.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuState {
    /// El nombre de la GPU (lo que dice el driver).
    pub name: String,
    /// El reloj de la GPU, en GHz.
    pub ghz: Option<Reading>,
    /// La temperatura de la GPU, en grados.
    pub temp_c: Option<Reading>,
}

/// La memoria de la máquina.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemoryState {
    /// La RAM en uso, en MB.
    pub ram_used_mb: Option<Reading>,
    /// La RAM total, en MB.
    pub ram_total_mb: Option<Reading>,
    /// El swap en uso, en MB (ya con zram dentro).
    pub swap_used_mb: Option<Reading>,
    /// El swap total, en MB (ya con zram dentro).
    pub swap_total_mb: Option<Reading>,
    /// Si hay zram: su tamaño en MB.
    pub zram_mb: Option<Reading>,
}

/// La foto de la máquina para el `status`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MachineState {
    /// El nombre del clock de la CPU, en GHz.
    pub cpu_ghz: Option<Reading>,
    /// La temperatura de la CPU, en grados.
    pub cpu_temp_c: Option<Reading>,
    /// La GPU **dedicada** (en macOS, la única GPU).
    pub dgpu: Option<GpuState>,
    /// La GPU **integrada** (sólo Linux).
    pub igpu: Option<GpuState>,
    /// La memoria.
    pub memory: MemoryState,
}

/// El texto de un recurso ausente.
pub const NO_DISPONIBLE: &str = "no disponible";

/// El título de la sección del estado.
const TITULO: &str = "ESTADO DE LA MÁQUINA";

/// Formatea una medida, o dice que no está.
pub fn format_reading(reading: &Option<Reading>) -> String {
    match reading {
        Some(r) => format!("{:.2} {}", r.value, r.unit),
        None => NO_DISPONIBLE.to_string(),
    }
}

/// El texto de `ropero status`.
///
/// Es una función **pura**: recibe la foto y devuelve el texto. Por eso se
/// puede probar sin máquina y sin disco.
pub fn status_text(maquina: &MachineState) -> String {
    let mut seccion = Seccion::nueva(TITULO);
    seccion.cuerpo(tabla_de_procesadores(maquina).lineas(&Palette::plain()));
    seccion.linea("");
    seccion.cuerpo(tabla_de_memoria(maquina).lineas(&Palette::plain()));
    let mut texto = seccion.lineas().join("\n");
    texto.push('\n');
    texto
}

/// La tabla de los procesadores: la CPU y las GPUs.
fn tabla_de_procesadores(maquina: &MachineState) -> Tabla {
    let mut tabla = Tabla::nivel(1)
        .columna(Columna::texto("recurso"))
        .columna(Columna::texto("reloj"))
        .columna(Columna::texto("temperatura"));
    tabla.fila(vec![
        Celda::plana("CPU"),
        Celda::plana(format_reading(&maquina.cpu_ghz)),
        Celda::plana(format_reading(&maquina.cpu_temp_c)),
    ]);
    for (nombre, gpu) in [("dGPU", &maquina.dgpu), ("iGPU", &maquina.igpu)] {
        let Some(gpu) = gpu else {
            tabla.fila(vec![
                Celda::plana(nombre),
                Celda::plana(NO_DISPONIBLE),
                Celda::plana(String::new()),
            ]);
            continue;
        };
        tabla.fila(vec![
            Celda::plana(nombre),
            Celda::plana(format_reading(&gpu.ghz)),
            Celda::plana(format_reading(&gpu.temp_c)),
        ]);
        // El nombre de la GPU es una nota de su fila: lo que dice el driver.
        tabla.nota(Celda::plana(gpu.name.clone()));
    }
    tabla
}

/// La tabla de la memoria: la RAM y el swap.
fn tabla_de_memoria(maquina: &MachineState) -> Tabla {
    let mut tabla = Tabla::nivel(1)
        .columna(Columna::texto("recurso"))
        .columna(Columna::numero("uso"))
        .columna(Columna::numero("total"));
    let ram = &maquina.memory;
    tabla.fila(vec![
        Celda::plana("RAM"),
        Celda::plana(format_reading(&ram.ram_used_mb)),
        Celda::plana(format_reading(&ram.ram_total_mb)),
    ]);
    tabla.fila(vec![
        Celda::plana("SWAP"),
        Celda::plana(format_reading(&ram.swap_used_mb)),
        Celda::plana(format_reading(&ram.swap_total_mb)),
    ]);
    // El zram va **aparte**: el swap lo incluye, pero no se confunde con él.
    if let Some(zram) = &ram.zram_mb {
        let nota = format!("(incluye {} de zram)", format_reading(&Some(zram.clone())));
        tabla.nota(Celda::plana(nota));
    }
    tabla
}

/// Las líneas del `status`, una por recurso.
///
/// La función devuelve líneas sueltas para que la CLI pueda imprimirla
/// como quiera; [`status_text`] las junta en un bloque.
pub fn status_lines(maquina: &MachineState) -> Vec<String> {
    status_text(maquina)
        .lines()
        .map(|l| l.to_string())
        .collect()
}

/// Convierte bytes en MB (mebibyte, que es lo que dice el `status`).
pub fn bytes_a_mb(bytes: f64) -> f64 {
    bytes / (1024.0 * 1024.0)
}

/// Convierte kB de `/proc/meminfo` en MB.
pub fn kb_a_mb(kb: f64) -> f64 {
    kb / 1024.0
}

/// Escribe un valor en un buffer, sin fallar nunca.
///
/// Es un `write!` que no puede fallar en un `String`: el `let _ =` deja
/// claro que el error es imposible, sin `unwrap()` (prohibido en el core).
pub fn escribe(destino: &mut String, valor: &str) {
    let _ = write!(destino, "{valor}");
}
