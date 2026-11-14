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
//! ## La captura
//!
//! La lectura de `/proc/cpuinfo`, `/sys/class/thermal`, `nvidia-smi` y
//! `/proc/meminfo` es E/S, y vive en `ropero-runtime`. Aquí sólo vive la
//! **decisión** (qué se enseña y cómo se formatea), que es pura.

use std::fmt::Write as _;

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
    let mut out = String::new();
    out.push_str("ESTADO DE LA MÁQUINA\n\n");
    out.push_str(&format!(
        "  CPU      {:<18}{}\n",
        format_reading(&maquina.cpu_ghz),
        format_reading(&maquina.cpu_temp_c)
    ));
    out.push_str("            frecuencia       temperatura\n");

    let dgpu = maquina
        .dgpu
        .as_ref()
        .map(|g| {
            format!(
                "  dGPU     {:<18}{}   {}\n",
                format_reading(&g.ghz),
                format_reading(&g.temp_c),
                g.name
            )
        })
        .unwrap_or_else(|| format!("  dGPU     {NO_DISPONIBLE}\n"));
    out.push_str(&dgpu);

    let igpu = maquina
        .igpu
        .as_ref()
        .map(|g| {
            format!(
                "  iGPU     {:<18}{}   {}\n",
                format_reading(&g.ghz),
                format_reading(&g.temp_c),
                g.name
            )
        })
        .unwrap_or_else(|| format!("  iGPU     {NO_DISPONIBLE}\n"));
    out.push_str(&igpu);

    let ram = format_reading(&maquina.memory.ram_used_mb);
    let ram_total = format_reading(&maquina.memory.ram_total_mb);
    out.push_str(&format!("  RAM      {ram} de {ram_total}\n"));

    let swap = format_reading(&maquina.memory.swap_used_mb);
    let swap_total = format_reading(&maquina.memory.swap_total_mb);
    let zram = match &maquina.memory.zram_mb {
        Some(z) => format!("  (incluye {} de zram)", format_reading(&Some(z.clone()))),
        None => String::new(),
    };
    out.push_str(&format!("  SWAP     {swap} de {swap_total}{zram}\n"));
    out
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
