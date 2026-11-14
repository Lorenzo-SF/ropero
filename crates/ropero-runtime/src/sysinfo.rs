// ropero-runtime: procesos, puertos y estado en disco.

use ropero_core::SystemInfo;
/// La captura de la información de la máquina (la frontera de E/S).
///
/// `ropero-core` es dominio puro: no lee disco, no consulta el entorno, no
/// lanza procesos (doc B §3.2.3). La información de la máquina —el SO, la
/// GPU, la NPU, la RAM— llega al dominio como **dato**, y esta es la capa
/// que lo lee de verdad.
///
/// ## Degradación elegante
///
/// Ninguna de las consultas es obligatoria: si `/proc/meminfo` no existe, o
/// `nvidia-smi` no está, o `sysctl` falla, el campo se queda en su valor por
/// defecto y se sigue. Un motor se declara incompatible por lo que
/// **falta**, así que un dato que no se pudo leer se trata como ausente: es
/// el mismo criterio que usa la v1 (`ropero-code/bin/lib/engines/*.sh`).
///
/// ## Dónde vive cada mitad
///
/// La **decisión** (qué SO es, si esto es una NPU, cuántos GB son) es del
/// dominio: vive en [`ropero_core::sysinfo`]. Aquí sólo se **lee** el disco
/// y el entorno, y se le pasa el texto.
use ropero_core::sysinfo;

/// Captura la información de la máquina.
pub fn capture_system_info() -> SystemInfo {
    SystemInfo {
        os: sysinfo::detect_os(std::env::consts::OS),
        arch: std::env::consts::ARCH.to_string(),
        tiene_nvidia: which("nvidia-smi").is_some(),
        tiene_metal: sysinfo::es_metal(std::env::consts::OS, std::env::consts::ARCH),
        tiene_npu_amd: detect_npu_amd(),
        ram_total_gb: detect_ram_gb(),
    }
}

/// Si hay una NPU AMD XDNA.
///
/// Las señales son las de la v1 (`ropero-code/bin/lib/engines/fastflowlm.sh`):
/// el device DRM (`/dev/accel/accel0`), el XRT (`xrt-smi`) y el silicon que
/// anuncia `lscpu`.
fn detect_npu_amd() -> bool {
    if std::env::consts::OS != "linux" {
        return false;
    }
    if std::path::Path::new("/dev/accel/accel0").exists() {
        return true;
    }
    let xrt = run_capture("xrt-smi", &["examine"]).map(|s| sysinfo::hace_npu(&s));
    let lscpu = run_capture("lscpu", &[]).map(|s| sysinfo::hace_npu(&s));
    xrt.unwrap_or(false) || lscpu.unwrap_or(false)
}

/// La RAM total de la máquina, en GB.
///
/// Se lee de `/proc/meminfo` (Linux). En macOS, donde no existe, se usa
/// `sysctl -n hw.memsize`. Si ninguna de las dos está, se queda en 0: un
/// motor que pida RAM mínima se declara incompatible, que es el criterio
/// conservador y el mismo de "falta".
fn detect_ram_gb() -> f64 {
    if let Some(kb) = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|t| sysinfo::memtotal_kb(&t))
    {
        return sysinfo::ram_gb_desde_kb(kb as f64);
    }
    match run_capture("sysctl", &["-n", "hw.memsize"]) {
        Some(salida) => sysinfo::ram_gb_desde_bytes(salida.trim()),
        None => 0.0,
    }
}

/// Busca un ejecutable en el `PATH`.
fn which(binario: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binario))
        .find(|candidato| es_ejecutable(candidato))
}

/// Si una ruta es un fichero ejecutable.
fn es_ejecutable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Ejecuta un comando y captura su salida estándar.
///
/// Si el comando no existe o falla, devuelve `None`: la consulta es
/// opcional por definición.
fn run_capture(programa: &str, args: &[&str]) -> Option<String> {
    which(programa)?;
    let salida = std::process::Command::new(programa)
        .args(args)
        .output()
        .ok()?;
    if !salida.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&salida.stdout).into_owned())
}
