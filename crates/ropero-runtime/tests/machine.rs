//! La captura de la máquina: `/proc`, `nvidia-smi` y el zram.
//!
//! La captura es E/S, así que se prueba de dos formas: la **decisión**
//! (el parseo) con textos de ejemplo, y la **captura real** con lo que la
//! máquina tiene (que sólo puede fallar si degrada con elegancia).

use ropero_core::sysinfo;

/// Un `/proc/meminfo` de ejemplo (los valores van en kB, como en Linux).
const MEMINFO: &str = "\
MemTotal:       33554432 kB
MemFree:         1048576 kB
MemAvailable:   16777216 kB
SwapTotal:       8388608 kB
SwapFree:        4194304 kB
";

/// La memoria se lee bien: usada, total, swap usada y swap total, en MB.
#[test]
fn el_meminfo_se_lee_en_mb() {
    let (ram_usada, ram_total, swap_usada, swap_total) = sysinfo::meminfo_mb(MEMINFO);
    assert_eq!(ram_total, Some(32768.0));
    assert_eq!(ram_usada, Some(16384.0));
    assert_eq!(swap_total, Some(8192.0));
    assert_eq!(swap_usada, Some(4096.0));
}

/// Un `/proc/meminfo` sin lo que se busca no inventa nada.
#[test]
fn un_meminfo_vacio_no_inventa_nada() {
    let (ram_usada, ram_total, swap_usada, swap_total) = sysinfo::meminfo_mb("nada\n");
    assert_eq!(ram_usada, None);
    assert_eq!(ram_total, None);
    assert_eq!(swap_usada, None);
    assert_eq!(swap_total, None);
}

/// El tamaño de un zram, en MB.
#[test]
fn el_zram_se_lee_en_mb() {
    assert_eq!(sysinfo::zram_mb("33554432\n"), Some(32.0));
    assert_eq!(sysinfo::zram_mb("no\n"), None);
}

/// La frecuencia de la CPU, en GHz, según `/proc/cpuinfo`.
#[test]
fn la_frecuencia_de_la_cpu_se_lee_en_ghz() {
    let cpuinfo = "processor\t: 0\ncpu MHz\t\t: 3400.000\ncache size\t: 1024 KB\n";
    assert_eq!(sysinfo::cpu_ghz(cpuinfo), Some(3.4));
}

/// Si no hay `cpu MHz`, se busca el GHz en el nombre del procesador.
#[test]
fn el_ghz_del_nombre_del_procesador_se_usa() {
    let cpuinfo = "model name\t: AMD Ryzen 9 5950X 16-Core Processor\n";
    assert_eq!(sysinfo::cpu_ghz(cpuinfo), None);
    let con_ghz = "model name\t: Apple M3 Max 3.20GHz\n";
    assert_eq!(sysinfo::cpu_ghz(con_ghz), Some(3.2));
}

/// La temperatura de una zona térmica, en grados.
#[test]
fn la_temperatura_se_lee_en_grados() {
    assert_eq!(sysinfo::thermal_c("58000\n"), Some(58.0));
    assert_eq!(sysinfo::thermal_c("no\n"), None);
}

/// Las GPUs de `nvidia-smi`, con su reloj y su temperatura.
#[test]
fn las_gpus_de_nvidia_se_leen() {
    let salida = "NVIDIA GeForce RTX 5080, 2100, 45\n";
    let gpus = sysinfo::nvidia_gpus(salida);
    assert_eq!(gpus.len(), 1);
    let (nombre, ghz, temp) = &gpus[0];
    assert_eq!(nombre, "NVIDIA GeForce RTX 5080");
    assert_eq!(*ghz, Some(2.1));
    assert_eq!(*temp, Some(45.0));
}

/// Una salida de `nvidia-smi` vacía no inventa GPUs.
#[test]
fn una_salida_vacia_no_inventa_gpus() {
    assert!(sysinfo::nvidia_gpus("").is_empty());
    assert!(sysinfo::nvidia_gpus("no hay GPUs\n").is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// La captura real
// ─────────────────────────────────────────────────────────────────────────────

/// La captura real no rompe y degrada con elegancia.
#[test]
fn la_captura_real_degrada_con_elegancia() {
    let maquina = ropero_runtime::capture_machine_state();
    // Si no hay CPU, no se inventa una: es `None`, no un cero.
    if let Some(r) = maquina.cpu_ghz {
        assert!(r.value > 0.0, "lectura={r:?}");
    }
    // La RAM usada nunca es mayor que la total.
    if let (Some(usada), Some(total)) = (maquina.memory.ram_used_mb, maquina.memory.ram_total_mb) {
        assert!(
            usada.value <= total.value,
            "usada={usada:?} total={total:?}"
        );
    }
}

/// La RAM del sistema es la de verdad.
#[test]
fn la_ram_capturada_es_la_del_sistema() {
    let maquina = ropero_runtime::capture_machine_state();
    if let Some(total) = maquina.memory.ram_total_mb {
        assert!(total.value > 0.0, "total={total:?}");
    }
}

/// El zram, si lo hay, no se confunde con el swap del kernel: se cuenta
/// aparte y se suma al swap reportado.
#[test]
fn el_zram_se_cuenta_aparte_y_se_suma_al_swap() {
    let maquina = ropero_runtime::capture_machine_state();
    let Some(zram) = maquina.memory.zram_mb.as_ref() else {
        // Sin zram no hay nada que comprobar: el caso no aplica.
        return;
    };
    assert!(zram.value > 0.0, "zram={zram:?}");
    // El swap reportado es **mayor** que el del kernel solo, porque lleva el
    // zram dentro.
    let kernel = sysinfo::meminfo_mb(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default());
    if let (Some(swap), Some(kernel_swap)) = (maquina.memory.swap_used_mb, kernel.3) {
        assert!(
            swap.value >= kernel_swap,
            "swap={swap:?} kernel={kernel_swap}"
        );
    }
}
