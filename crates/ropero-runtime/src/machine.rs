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
        cpu_brand: cpu_brand().unwrap_or_else(|| "desconocido".to_string()),
        dgpu: dgpu(),
        igpu: igpu(),
        memory: memory(),
    }
}

fn cpu_brand() -> Option<String> {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;
                matches!(key.trim(), "model name" | "Hardware" | "Processor")
                    .then(|| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            })
        })
        .or_else(|| run_capture("sysctl", &["-n", "machdep.cpu.brand_string"]))
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
        Os::Linux => nvidia_gpu().or_else(|| linux_gpu(false)),
        _ => None,
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
    linux_gpu(true)
}

/// Identifica una GPU PCI dedicada (boot_vga=0) o primaria/integrada
/// (boot_vga=1). Los valores dinámicos quedan ausentes si el driver no los
/// publica con una interfaz que podamos consultar de forma portable.
fn linux_gpu(primary: bool) -> Option<GpuState> {
    let salida = run_capture("lspci", &["-D", "-mm", "-nn"])?;
    for linea in salida.lines() {
        let Some((address, name)) = parse_lspci_gpu(linea) else {
            continue;
        };
        let Some(boot_vga) =
            std::fs::read_to_string(format!("/sys/bus/pci/devices/{address}/boot_vga"))
                .ok()
                .map(|value| value.trim() == "1")
        else {
            continue;
        };
        if boot_vga != primary {
            continue;
        }
        return Some(GpuState {
            name,
            ghz: None,
            temp_c: None,
            vram_used_mb: None,
            vram_total_mb: None,
        });
    }
    None
}

fn parse_lspci_gpu(line: &str) -> Option<(String, String)> {
    let mut quoted = line.split('"');
    let address = line.split_whitespace().next()?.to_string();
    let _class = quoted.next()?;
    let class = quoted.next()?;
    if !class.to_ascii_lowercase().contains("controller")
        || !(class.to_ascii_lowercase().contains("vga")
            || class.to_ascii_lowercase().contains("display"))
    {
        return None;
    }
    let _separator = quoted.next()?;
    let vendor = quoted.next()?.trim();
    let _separator = quoted.next()?;
    let device = quoted.next()?.trim();
    Some((address, format!("{vendor} {device}")))
}

/// La GPU de NVIDIA, según `nvidia-smi`.
fn nvidia_gpu() -> Option<GpuState> {
    let salida = run_capture(
        "nvidia-smi",
        &[
            "--query-gpu=name,clocks.gr,temperature.gpu,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ],
    )?;
    let row = salida.lines().next()?;
    let mut fields = row.split(',').map(str::trim);
    let nombre = fields.next()?.to_string();
    let ghz = fields
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v / 1000.0);
    let temp = fields.next().and_then(|v| v.parse::<f64>().ok());
    let used = fields.next().and_then(|v| v.parse::<f64>().ok());
    let total = fields.next().and_then(|v| v.parse::<f64>().ok());
    Some(GpuState {
        name: nombre,
        ghz: ghz.map(|value| Reading { value, unit: "GHz" }),
        temp_c: temp.map(|value| Reading { value, unit: "°C" }),
        vram_used_mb: used.map(|value| Reading { value, unit: "MB" }),
        vram_total_mb: total.map(|value| Reading { value, unit: "MB" }),
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
        vram_used_mb: None,
        vram_total_mb: None,
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
        if let Some(mb) = sysinfo::zram_mb(&disksize)
            && mb > 0.0
        {
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

#[cfg(test)]
mod gpu_tests {
    use super::parse_lspci_gpu;

    #[test]
    fn parsea_nombre_y_bus_pci_de_tarjeta_grafica() {
        assert_eq!(
            parse_lspci_gpu(
                "0000:03:00.0 \"VGA compatible controller\" \"Advanced Micro Devices, Inc.\" \"Navi 31 [Radeon RX 7900 XTX]\" \"[0300]\""
            ),
            Some((
                "0000:03:00.0".into(),
                "Advanced Micro Devices, Inc. Navi 31 [Radeon RX 7900 XTX]".into()
            ))
        );
        assert!(parse_lspci_gpu("0000:00:1f.3 \"Audio device\" \"Intel\" \"Audio\"").is_none());
    }
}
