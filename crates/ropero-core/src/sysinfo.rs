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

// ─────────────────────────────────────────────────────────────────────────────
// La captura del `status`
// ─────────────────────────────────────────────────────────────────────────────

/// La memoria de `/proc/meminfo`, en MB.
///
/// Devuelve (usada, total) en MB. El fichero trae líneas `clave: valor` en
/// kB: `MemTotal`, `MemAvailable`, `SwapTotal` y `SwapFree`.
pub fn meminfo_mb(texto: &str) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    let mut total = None;
    let mut disponible = None;
    let mut swap_total = None;
    let mut swap_libre = None;
    for linea in texto.lines() {
        let Some((clave, valor)) = linea.split_once(':') else {
            continue;
        };
        let clave = clave.trim();
        let kb = valor_kb(valor);
        match clave {
            "MemTotal" => total = kb,
            "MemAvailable" => disponible = kb,
            "SwapTotal" => swap_total = kb,
            "SwapFree" => swap_libre = kb,
            _ => {}
        }
    }
    let ram_usada = match (total, disponible) {
        (Some(t), Some(d)) => Some(t - d),
        _ => None,
    };
    let swap_usada = match (swap_total, swap_libre) {
        (Some(t), Some(f)) => Some(t - f),
        _ => None,
    };
    (
        ram_usada.map(|v| v / 1024.0),
        total.map(|v| v / 1024.0),
        swap_usada.map(|v| v / 1024.0),
        swap_total.map(|v| v / 1024.0),
    )
}

/// El valor de una línea de `/proc/meminfo`, en kB.
///
/// La línea es `clave:  12345 kB`; sólo se queda con el número.
pub fn valor_kb(valor: &str) -> Option<f64> {
    let numeros: String = valor
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    numeros.parse::<f64>().ok()
}

/// El tamaño de un dispositivo zram, en MB, según su `disksize`.
///
/// El fichero `disksize` de `/sys/block/zramN` está en bytes.
pub fn zram_mb(disksize: &str) -> Option<f64> {
    let bytes: f64 = disksize.trim().parse().ok()?;
    Some(bytes / (1024.0 * 1024.0))
}

/// La frecuencia de la CPU, en GHz, según `/proc/cpuinfo`.
///
/// Busca la línea `cpu MHz` y la divide entre 1000. Si no está, busca
/// `model name` y extrae el GHz del nombre (los Apple Silicon y los
/// Scaleway loannounced así).
pub fn cpu_ghz(cpuinfo: &str) -> Option<f64> {
    for linea in cpuinfo.lines() {
        let Some((clave, valor)) = linea.split_once(':') else {
            continue;
        };
        if clave.trim() == "cpu MHz" {
            let mhz: f64 = valor.trim().parse().ok()?;
            return Some(mhz / 1000.0);
        }
    }
    ghz_de_nombre(cpuinfo)
}

/// El GHz que anuncia el nombre del procesador.
///
/// Busca el número que va **seguido** de "GHz": en "Apple M3 Max 3.20GHz"
/// el número es `3.20`, y en "AMD Ryzen 9 5950X" no hay ninguno (ni el 9
/// ni el 5950 son GHz).
fn ghz_de_nombre(cpuinfo: &str) -> Option<f64> {
    for linea in cpuinfo.lines() {
        let Some((clave, valor)) = linea.split_once(':') else {
            continue;
        };
        if !clave.trim().starts_with("model name") {
            continue;
        }
        let texto = valor.to_lowercase();
        let mut pos = 0;
        while let Some(inicio) = texto[pos..].find(|c: char| c.is_ascii_digit()) {
            let desde = pos + inicio;
            let numero: String = texto[desde..]
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let fin = desde + numero.len();
            if texto[fin..].starts_with("ghz")
                && let Ok(valor) = numero.parse::<f64>()
            {
                return Some(valor);
            }
            pos = desde + numero.len().max(1);
        }
    }
    None
}

/// La temperatura de un sensor térmico, en grados Celsius.
///
/// `thermal_zoneN/temp` está en mili-grados: `42000` son 42 °C.
pub fn thermal_c(milis: &str) -> Option<f64> {
    let valor: f64 = milis.trim().parse().ok()?;
    Some(valor / 1000.0)
}

/// Una GPU, según la salida de `nvidia-smi --query-gpu=...`.
///
/// La salida esperada tiene una línea por GPU con los campos separados por
/// comas: `nombre, clocks.gr, temperature.gpu`.
pub fn nvidia_gpus(salida: &str) -> Vec<(String, Option<f64>, Option<f64>)> {
    let mut gpus = Vec::new();
    for linea in salida.lines() {
        let linea = linea.trim();
        if linea.is_empty() {
            continue;
        }
        let campos: Vec<&str> = linea.split(',').map(|c| c.trim()).collect();
        if campos.len() < 3 {
            continue;
        }
        let nombre = campos[0].to_string();
        let ghz = campos[1].parse::<f64>().ok().map(|mhz| mhz / 1000.0);
        let temp = campos[2].parse::<f64>().ok();
        gpus.push((nombre, ghz, temp));
    }
    gpus
}
