//! La captura de la máquina para `ropero status`.
//!
//! Aquí es donde se **lee**: `/proc/cpuinfo`, `/sys/class/thermal`,
//! `nvidia-smi`, `/proc/meminfo` y `/sys/block/zram*`. La **decisión** (qué
//! es el GHz, qué es la temperatura, cómo se compone el swap con el zram)
//! vive en el dominio, en [`ropero_core::sysinfo`].
//!
//! ## La regla de la degradación
//!
//! Un dato que no se puede leer **no se inventa**: se queda `None`, y el
//! `status` lo enseña como `no disponible`. Es preferible un hueco honesto
//! a un número falso.

use std::path::Path;

use ropero_core::status::{GpuState, MachineState, MemoryState, Reading};
use ropero_core::{Os, sysinfo};

/// Captura el estado de la máquina.
pub fn capture_machine_state() -> MachineState {
    MachineState {
        cpu_ghz: cpu_ghz(),
        cpu_temp_c: cpu_temp_c(),
        dgpu: dgpu(),
        igpu: igpu(),
        memory: memory(),
    }
}

/// La frecuencia de la CPU, en GHz.
fn cpu_ghz() -> Option<Reading> {
    let texto = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    sysinfo::cpu_ghz(&texto).map(|value| Reading { value, unit: "GHz" })
}

/// La temperatura de la CPU, en grados.
///
/// Se busca la zona térmica que hable de la CPU. En Linux, `/sys/class/
/// thermal/thermal_zone*/type` dice si una zona es la CPU (`x86_pkg_temp`,
/// `cpu-thermal`, `soc_thermal`…).
fn cpu_temp_c() -> Option<Reading> {
    let dir = Path::new("/sys/class/thermal");
    let entradas = std::fs::read_dir(dir).ok()?;
    let mut candidatas: Vec<std::path::PathBuf> = entradas
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    candidatas.sort();
    for zona in candidatas {
        let tipo = std::fs::read_to_string(zona.join("type")).unwrap_or_default();
        let tipo = tipo.trim().to_lowercase();
        let es_cpu = tipo.contains("cpu")
            || tipo.contains("pkg")
            || tipo.contains("soc")
            || tipo.contains("core");
        if !es_cpu {
            continue;
        }
        let Ok(milis) = std::fs::read_to_string(zona.join("temp")) else {
            continue;
        };
        if let Some(temp) = sysinfo::thermal_c(&milis) {
            return Some(Reading {
                value: temp,
                unit: "°C",
            });
        }
    }
    None
}

/// La GPU **dedicada**.
///
/// En Linux, la dGPU es la de NVIDIA (la consulta a `nvidia-smi`). En macOS
/// es la única GPU (la de Apple), que se lee con `system_profiler`.
fn dgpu() -> Option<GpuState> {
    match sysinfo::detect_os(std::env::consts::OS) {
        Os::Macos => macos_gpu(),
        _ => nvidia_gpu(),
    }
}

/// La GPU **integrada**: sólo Linux, y sólo si la hay.
///
/// En Linux la iGPU es la de Intel/AMD; se detecta por el PCI. No se
/// implementa la lectura directa: la dGPU de NVIDIA es la que el `status`
/// muestra como tal, y la iGPU aparece sólo si el driver la anuncia.
fn igpu() -> Option<GpuState> {
    if sysinfo::detect_os(std::env::consts::OS) != Os::Linux {
        return None;
    }
    let salida = run_capture("lspci", &["--mm", "--names"])?;
    for linea in salida.lines() {
        let minuscula = linea.to_lowercase();
        if !minuscula.contains("vga compatible controller")
            && !minuscula.contains("display controller")
        {
            continue;
        }
        // Una GPU NVIDIA es la dGPU, no la iGPU.
        if minuscula.contains("nvidia") {
            return None;
        }
        let nombre = linea
            .split_once(':')
            .map(|(_, r)| r.trim().to_string())
            .unwrap_or_else(|| linea.trim().to_string());
        return Some(GpuState {
            name: nombre,
            ghz: None,
            temp_c: None,
        });
    }
    None
}

/// La GPU de NVIDIA, según `nvidia-smi`.
fn nvidia_gpu() -> Option<GpuState> {
    let salida = run_capture(
        "nvidia-smi",
        &[
            "--query-gpu=name,clocks.gr,temperature.gpu",
            "--format=csv,noheader,nounits",
        ],
    )?;
    let gpus = sysinfo::nvidia_gpus(&salida);
    let (nombre, ghz, temp) = gpus.into_iter().next()?;
    Some(GpuState {
        name: nombre,
        ghz: ghz.map(|value| Reading { value, unit: "GHz" }),
        temp_c: temp.map(|value| Reading { value, unit: "°C" }),
    })
}

/// La GPU de macOS, según `system_profiler`.
fn macos_gpu() -> Option<GpuState> {
    let salida = run_capture("system_profiler", &["SPDisplaysDataType"])?;
    let mut nombre = None;
    for linea in salida.lines() {
        let minuscula = linea.to_lowercase();
        if !minuscula.contains("chipset model") {
            continue;
        }
        if let Some((_, valor)) = linea.split_once(':') {
            nombre = Some(valor.trim().to_string());
            break;
        }
    }
    Some(GpuState {
        name: nombre?,
        ghz: None,
        temp_c: None,
    })
}

/// La memoria: RAM y swap, **con el zram dentro**.
///
/// El zram de CachyOS es memoria comprimida: cuenta como swap usado. Por eso
/// el swap que se reporta es el del kernel **más** el zram, y no se
/// confunde uno con otro: el zram se enseña aparte.
fn memory() -> MemoryState {
    let (ram_usada, ram_total, swap_usada, swap_total) = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .map(|t| sysinfo::meminfo_mb(&t))
        .unwrap_or((None, None, None, None));
    let zram = zram_mb();
    let (swap_usada, swap_total) = match zram {
        Some(z) => (swap_usada.map(|s| s + z), swap_total.map(|t| t + z)),
        None => (swap_usada, swap_total),
    };
    MemoryState {
        ram_used_mb: ram_usada.map(|value| Reading { value, unit: "MB" }),
        ram_total_mb: ram_total.map(|value| Reading { value, unit: "MB" }),
        swap_used_mb: swap_usada.map(|value| Reading { value, unit: "MB" }),
        swap_total_mb: swap_total.map(|value| Reading { value, unit: "MB" }),
        zram_mb: zram.map(|value| Reading { value, unit: "MB" }),
    }
}

/// El tamaño total de los dispositivos zram, en MB.
///
/// Se suman todos los `zramN`: puede haber más de uno.
fn zram_mb() -> Option<f64> {
    let dir = Path::new("/sys/block");
    let entradas = std::fs::read_dir(dir).ok()?;
    let mut total = 0.0;
    let mut hay = false;
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name().to_string_lossy().to_string();
        if !nombre.starts_with("zram") {
            continue;
        }
        let Ok(disksize) = std::fs::read_to_string(entrada.path().join("disksize")) else {
            continue;
        };
        if let Some(mb) = sysinfo::zram_mb(&disksize) {
            total += mb;
            hay = true;
        }
    }
    if hay { Some(total) } else { None }
}

/// Ejecuta un comando y captura su salida estándar.
///
/// Si el comando no existe o falla, devuelve `None`: la consulta es
/// opcional por definición.
fn run_capture(programa: &str, args: &[&str]) -> Option<String> {
    let salida = std::process::Command::new(programa)
        .args(args)
        .output()
        .ok()?;
    if !salida.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&salida.stdout).into_owned())
}
