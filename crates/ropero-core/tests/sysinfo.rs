//! La decisión de qué es la máquina, sin leer nada.
//!
//! La E/S de la información del sistema vive en la CLI
//! (`ropero-cli`/`ropero-runtime`); aquí sólo está la **decisión**, que es
//! pura y por eso se puede probar sin disco y sin máquina.

use ropero_core::Os;
use ropero_core::sysinfo;

/// El SO se traduce al tipo del dominio.
#[test]
fn el_so_se_traduce_al_tipo_del_dominio() {
    assert_eq!(sysinfo::detect_os("linux"), Os::Linux);
    assert_eq!(sysinfo::detect_os("macos"), Os::Macos);
    assert_eq!(sysinfo::detect_os("windows"), Os::Windows);
}

/// Un SO desconocido se trata como Linux: es el único que la v2 soporta de
/// verdad, y tratarlo como otro escondería el problema.
#[test]
fn un_so_desconocido_se_trata_como_linux() {
    assert_eq!(sysinfo::detect_os("plan9"), Os::Linux);
    assert_eq!(sysinfo::detect_os(""), Os::Linux);
}

/// Metal es propio de macOS en ARM.
#[test]
fn el_metal_es_de_macos_en_arm() {
    assert!(sysinfo::es_metal("macos", "aarch64"));
    assert!(!sysinfo::es_metal("macos", "x86_64"));
    assert!(!sysinfo::es_metal("linux", "aarch64"));
    assert!(!sysinfo::es_metal("windows", "aarch64"));
}

/// Las señales de la NPU AMD son las de la v1: `npu`, `accel`, `xdna` y
/// `ryzen ai`.
#[test]
fn las_senales_de_la_npu_amd_son_las_de_la_v1() {
    assert!(sysinfo::hace_npu("NPU: 1"));
    assert!(sysinfo::hace_npu("accel0"));
    assert!(sysinfo::hace_npu("XDNA"));
    assert!(sysinfo::hace_npu("Ryzen AI Software"));
    assert!(!sysinfo::hace_npu("Intel"));
    assert!(!sysinfo::hace_npu(""));
}

/// El `MemTotal` de `/proc/meminfo` se lee en kB.
#[test]
fn el_memtotal_de_meminfo_se_lee_en_kb() {
    let meminfo = "MemTotal:       32768000 kB\nMemFree:         100 kB\n";
    assert_eq!(sysinfo::memtotal_kb(meminfo), Some(32768000));
    assert_eq!(sysinfo::memtotal_kb("nada"), None);
    assert_eq!(sysinfo::memtotal_kb(""), None);
}

/// La RAM se convierte a GB.
#[test]
fn la_ram_se_convierte_a_gb() {
    assert!((sysinfo::ram_gb_desde_kb(1024.0 * 1024.0) - 1.0).abs() < 0.001);
    assert!((sysinfo::ram_gb_desde_kb(2.0 * 1024.0 * 1024.0) - 2.0).abs() < 0.001);
    assert!((sysinfo::ram_gb_desde_bytes("1073741824") - 1.0).abs() < 0.001);
    assert_eq!(sysinfo::ram_gb_desde_bytes("no es un número"), 0.0);
}
