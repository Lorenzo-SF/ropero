//! La captura de la información de la máquina (la frontera de E/S).
//!
//! La batería comprueba lo que se puede comprobar **en cualquier máquina**:
//! que la captura no se rompe, que degrada con elegancia y que los valores
//! que devuelve son coherentes con el sistema que la ejecuta.

use ropero_core::Os;

/// La captura no rompe y devuelve el SO real del sistema.
#[test]
fn la_captura_devuelve_el_so_real() {
    let sistema = ropero_runtime::capture_system_info();
    let esperado = match std::env::consts::OS {
        "macos" => Os::Macos,
        "windows" => Os::Windows,
        _ => Os::Linux,
    };
    assert_eq!(sistema.os, esperado);
}

/// La arquitectura que se devuelve es la del binario.
#[test]
fn la_captura_devuelve_la_arquitectura_real() {
    let sistema = ropero_runtime::capture_system_info();
    assert_eq!(sistema.arch, std::env::consts::ARCH);
}

/// La captura degrada con elegancia: si no se puede leer la RAM, se
/// devuelve 0, y el sistema sigue siendo utilizable.
///
/// Un motor que pida RAM mínima se declarará incompatible: es el criterio
/// conservador, y el mismo de "falta".
#[test]
fn la_captura_degrada_con_elegancia() {
    let sistema = ropero_runtime::capture_system_info();
    assert!(
        sistema.ram_total_gb >= 0.0,
        "la RAM no puede ser negativa: {}",
        sistema.ram_total_gb
    );
}

/// La coherencia de los datos: si hay Metal, el SO es macOS en ARM; si no
/// hay NPU AMD, no se afirma que la haya.
#[test]
fn los_datos_capturados_son_coherentes() {
    let sistema = ropero_runtime::capture_system_info();
    if sistema.tiene_metal {
        assert_eq!(sistema.os, Os::Macos);
        assert_eq!(sistema.arch, "aarch64");
    }
    if sistema.tiene_npu_amd {
        assert_eq!(sistema.os, Os::Linux);
    }
}
