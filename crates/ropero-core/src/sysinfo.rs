//! La decisión de qué es la máquina, sin leer nada.
//!
//! La E/S de la información del sistema vive en la CLI
//! (`ropero-cli::sysinfo`): aquí sólo está la **decisión**, que es pura y
//! por eso se puede probar sin disco y sin máquina.
//!
//! ## El criterio de "falta"
//!
//! Cuando un dato no se pudo leer, se trata como **ausente**: un motor que
//! pide Metal en una máquina donde no se pudo detectar Metal es
//! incompatible. Es el criterio conservador y el mismo que usa la v1: si
//! `nvidia-smi` no contesta, no hay NVIDIA.

use crate::manifest::Os;

/// Traduce el nombre del SO (`std::env::consts::OS`) al tipo del dominio.
///
/// Un SO desconocido se trata como Linux: es el único que la v2 soporta de
/// verdad, y tratarlo como otro no haría más que esconder el problema.
pub fn detect_os(nombre: &str) -> Os {
    match nombre {
        "macos" | "ios" => Os::Macos,
        "windows" => Os::Windows,
        _ => Os::Linux,
    }
}

/// Si la máquina es Apple Silicon con Metal.
///
/// El Metal es propio de macOS en ARM: en Linux o en Windows no lo hay, y
/// en macOS x86 tampoco.
pub fn es_metal(os: &str, arch: &str) -> bool {
    detect_os(os) == Os::Macos && arch == "aarch64"
}

/// Si el texto de `xrt-smi examine` o de `lscpu` anuncia una NPU AMD.
///
/// Las señales son las de la v1 (`ropero-code/bin/lib/engines/fastflowlm.sh`):
/// el XRT dice `npu` o `accel`, y `lscpu` dice `xdna` o `ryzen ai`.
pub fn hace_npu(texto: &str) -> bool {
    let texto = texto.to_lowercase();
    texto.contains("npu")
        || texto.contains("accel")
        || texto.contains("xdna")
        || texto.contains("ryzen ai")
}

/// El `MemTotal` de `/proc/meminfo`, en kB.
///
/// El fichero tiene líneas `clave: valor`; sólo interesa `MemTotal`, y su
/// valor viene en kB (`kB` o `KB` detrás del número).
pub fn memtotal_kb(texto: &str) -> Option<u64> {
    for linea in texto.lines() {
        let Some((clave, valor)) = linea.split_once(':') else {
            continue;
        };
        if clave.trim() != "MemTotal" {
            continue;
        }
        let numeros: String = valor.chars().filter(|c| c.is_ascii_digit()).collect();
        return numeros.parse::<u64>().ok();
    }
    None
}

/// Convierte kB a GB.
pub fn ram_gb_desde_kb(kb: f64) -> f64 {
    kb / (1024.0 * 1024.0)
}

/// Convierte bytes a GB.
///
/// `sysctl -n hw.memsize` devuelve los bytes en decimal.
pub fn ram_gb_desde_bytes(texto: &str) -> f64 {
    let Ok(bytes) = texto.trim().parse::<f64>() else {
        return 0.0;
    };
    bytes / (1024.0 * 1024.0 * 1024.0)
}
