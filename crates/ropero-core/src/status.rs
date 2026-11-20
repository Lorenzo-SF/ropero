//! La vista `ropero status`: el estado de la máquina, tipo `nvidia-smi`.
//!
//! ## Qué muestra
//!
//! | Recurso | Qué |
//! |---|---|
//! | CPU | marca, GHz y temperatura |
//! | dGPU/iGPU | marca, GHz, temperatura y VRAM si el driver la publica |
//! | RAM/SWAP | GB usados/total y porcentaje de swap |
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
//! CPU Intel 3,40GHz 58,0°C │ RAM 8,0/32,0G 25% │ SWP 12% · zram 4,0G
//! dGPU NVIDIA GeForce RTX 5080 2,10GHz 45,0°C │ VRAM 8,0/16,0G 50%
//! iGPU Intel UHD 770 no disponible no disponible │ VRAM no disponible
//! LLM Sin modelos arrancados
//! ```
//!
//! La salida es compacta para consultarla mientras un modelo está corriendo.
//!
//! ## La captura
//!
//! La lectura de `/proc/cpuinfo`, `/sys/class/thermal`, `nvidia-smi` y
//! `/proc/meminfo` es E/S, y vive en `ropero-runtime`. Aquí sólo vive la
//! **decisión** (qué se enseña y cómo se formatea), que es pura.

use std::fmt::Write as _;

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
    /// Memoria de vídeo usada y total, en MB.
    pub vram_used_mb: Option<Reading>,
    pub vram_total_mb: Option<Reading>,
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
    /// Marca comercial identificada a partir del nombre de la CPU.
    pub cpu_brand: String,
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
    status_text_color(maquina, &Palette::plain())
}

/// Salida compacta y coloreada: temperatura y saturación de memoria por umbral.
pub fn status_text_color(maquina: &MachineState, paleta: &Palette) -> String {
    let brand = machine_brand(&maquina.cpu_brand);
    let cpu_label = paleta.paint_fg(brand_color(&brand), &format!("CPU {brand}"));
    let cpu = format!(
        "{cpu_label} {} {}",
        medida(&maquina.cpu_ghz),
        medida(&maquina.cpu_temp_c)
    );
    let mem = &maquina.memory;
    let ram = memoria(&mem.ram_used_mb, &mem.ram_total_mb, paleta);
    let swap_pct = porcentaje(&mem.swap_used_mb, &mem.swap_total_mb);
    let zram = mem
        .zram_mb
        .as_ref()
        .map_or_else(String::new, |z| format!(" · zram {:.1}G", z.value / 1024.0));
    let mut lines = vec![format!(
        "{} │ RAM {} │ SWP {}{}",
        decimal(&cpu),
        decimal(&ram),
        swap_pct,
        decimal(&zram)
    )];
    for (kind, gpu) in [
        ("dGPU", maquina.dgpu.as_ref()),
        ("iGPU", maquina.igpu.as_ref()),
    ] {
        let (name, ghz, temp, vram) = match gpu {
            Some(gpu) => (
                gpu.name.as_str(),
                medida(&gpu.ghz),
                medida(&gpu.temp_c),
                memoria(&gpu.vram_used_mb, &gpu.vram_total_mb, paleta),
            ),
            None => (
                NO_DISPONIBLE,
                NO_DISPONIBLE.to_string(),
                NO_DISPONIBLE.to_string(),
                NO_DISPONIBLE.to_string(),
            ),
        };
        let name = paleta.paint_fg(brand_color(name), name);
        lines.push(decimal(&format!(
            "{kind} {name} {ghz} {temp} │ VRAM {vram}"
        )));
    }
    lines.join("\n") + "\n"
}

fn machine_brand(name: &str) -> String {
    if name.trim().is_empty() || name == "desconocido" {
        return "desconocido".into();
    }
    let lower = name.to_ascii_lowercase();
    if lower.contains("apple")
        || lower.contains("m1")
        || lower.contains("m2")
        || lower.contains("m3")
        || lower.contains("m4")
        || lower.contains("m5")
    {
        "AppleSilicon".into()
    } else if lower.contains("snapdragon") || lower.contains("qualcomm") {
        "Snapdragon".into()
    } else if lower.contains("amd") || lower.contains("ryzen") {
        "AMD".into()
    } else if lower.contains("intel") || lower.contains("xeon") || lower.contains("core") {
        "Intel".into()
    } else {
        name.to_string()
    }
}

fn brand_color(name: &str) -> crate::palette::HexColor {
    let lower = name.to_ascii_lowercase();
    if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("rtx") {
        crate::palette::NVIDIA
    } else if lower.contains("amd") || lower.contains("radeon") {
        crate::palette::AMD
    } else if lower.contains("intel") || lower.contains("arc") {
        crate::palette::INTEL
    } else if lower.contains("snapdragon") || lower.contains("qualcomm") {
        crate::palette::SNAPDRAGON
    } else if lower.contains("apple") || lower.starts_with('m') {
        crate::palette::APPLE_SILICON
    } else {
        crate::palette::LOG_LIGHT
    }
}

fn decimal(value: &str) -> String {
    value.replace('.', ",")
}

fn medida(value: &Option<Reading>) -> String {
    value.as_ref().map_or_else(
        || NO_DISPONIBLE.into(),
        |r| match r.unit {
            "°C" => format!("{:.1}{}", r.value, r.unit),
            _ => format!("{:.2}{}", r.value, r.unit),
        },
    )
}

fn memoria(used: &Option<Reading>, total: &Option<Reading>, palette: &Palette) -> String {
    match (used, total) {
        (Some(u), Some(t)) if t.value > 0.0 => {
            let pct = u.value * 100.0 / t.value;
            let value = format!("{:.1}/{:.1}G {pct:.0}%", u.value / 1024.0, t.value / 1024.0);
            palette.paint_hex(
                if pct < 70.0 {
                    crate::palette::RAM_LOW
                } else if pct < 88.0 {
                    crate::palette::RAM_MEDIUM
                } else {
                    crate::palette::RAM_HIGH
                },
                &value,
            )
        }
        _ => format!("{} / {}", format_reading(used), format_reading(total)),
    }
}

pub fn vram_color(name: &str) -> crate::palette::HexColor {
    brand_color(name)
}

fn porcentaje(used: &Option<Reading>, total: &Option<Reading>) -> String {
    match (used, total) {
        (Some(u), Some(t)) if t.value > 0.0 => format!("{:.0}%", u.value * 100.0 / t.value),
        _ => NO_DISPONIBLE.into(),
    }
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
