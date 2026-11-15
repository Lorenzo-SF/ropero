//! La **precondición del motor** (R-GGUF-54…57, T-29).
//!
//! La puerta de T-29: *sin motor: error correcto y cero bytes
//! descargados*. La parte pura es el mensaje: nombra el motor que falta
//! y la variable que lo apunta. El orden (precondición **antes** de la
//! fase 1) lo prueba la tirada con dobles (`tests/ejecutar.rs`).

use std::path::PathBuf;

use ropero_forge::Error;
use ropero_forge::toolchain::{self, Busqueda};

/// Sin conversor en ningún motor: el error nombra el conversor y las dos
/// variables (R-GGUF-55: el conversor no admite sustituto).
#[test]
fn sin_conversor_el_error_nombra_el_motor_y_la_variable() {
    let error = toolchain::ensure_toolchain(&Busqueda::default(), true).expect_err("no hay motor");
    match &error {
        Error::SinMotor { falta, donde } => {
            assert!(falta.contains("convert_hf_to_gguf.py"), "dice {falta}");
            assert!(donde.contains("LLAMA_DIR"), "dice {donde}");
            assert!(donde.contains("LLAMA_OFFICIAL_DIR"), "dice {donde}");
        }
        otro => panic!("esperaba un SinMotor, y salió: {otro:?}"),
    }
    assert_eq!(error.codigo(), 1);
}

/// Con el conversor en el **fork** se usa el fork.
#[test]
fn el_fork_manda_sobre_el_oficial() {
    let busqueda = Busqueda {
        dir_fork: Some(PathBuf::from("/llama.cpp")),
        convert_fork: Some(PathBuf::from("/llama.cpp/convert_hf_to_gguf.py")),
        quantize_fork: Some(PathBuf::from("/llama.cpp/build/bin/llama-quantize")),
        ..Busqueda::default()
    };
    let toolchain = toolchain::ensure_toolchain(&busqueda, true).expect("el fork está");
    assert_eq!(
        toolchain.motor.dir(),
        Some(std::path::Path::new("/llama.cpp"))
    );
    assert_eq!(
        toolchain.convert,
        Some(PathBuf::from("/llama.cpp/convert_hf_to_gguf.py"))
    );
    assert_eq!(
        toolchain.quantize,
        PathBuf::from("/llama.cpp/build/bin/llama-quantize")
    );
}

/// Con sólo el **oficial** (R-GGUF-57: la v2 mira los dos motores), se
/// usa el oficial.
#[test]
fn el_oficial_sirve_cuando_el_fork_no_esta() {
    let busqueda = Busqueda {
        dir_oficial: Some(PathBuf::from("/llama.cpp-official")),
        convert_oficial: Some(PathBuf::from("/llama.cpp-official/convert_hf_to_gguf.py")),
        quantize_oficial: Some(PathBuf::from(
            "/llama.cpp-official/build/bin/llama-quantize",
        )),
        ..Busqueda::default()
    };
    let toolchain = toolchain::ensure_toolchain(&busqueda, true).expect("el oficial está");
    assert!(matches!(
        toolchain.motor,
        ropero_forge::Motor::Oficial { .. }
    ));
}

/// Con conversor pero sin cuantizador: el error dice qué herramienta
/// falta y dónde se buscó.
#[test]
fn sin_cuantizador_el_error_dice_donde_se_busco() {
    let busqueda = Busqueda {
        dir_fork: Some(PathBuf::from("/llama.cpp")),
        convert_fork: Some(PathBuf::from("/llama.cpp/convert_hf_to_gguf.py")),
        ..Busqueda::default()
    };
    let error = toolchain::ensure_toolchain(&busqueda, true).expect_err("falta el cuantizador");
    match &error {
        Error::MotorIncompleto { falta, donde } => {
            assert!(falta.contains("llama-quantize"), "dice {falta}");
            assert!(donde.contains("PATH"), "dice {donde}");
            assert!(donde.contains("build/bin"), "dice {donde}");
        }
        otro => panic!("esperaba un MotorIncompleto, y salió: {otro:?}"),
    }
}

/// **Hallazgo 9 (R-GGUF-55)**: el conversor es imprescindible **sólo si hay que
/// convertir**. Con una fuente que ya es un GGUF, quien sólo tiene el
/// cuantizador puede fabricar: la precondición no puede exigir un conversor
/// que no se va a usar.
#[test]
fn sin_conversor_se_puede_cuantizar_si_no_hay_que_convertir() {
    let busqueda = Busqueda {
        dir_fork: Some(PathBuf::from("/llama.cpp")),
        quantize_fork: Some(PathBuf::from("/llama.cpp/build/bin/llama-quantize")),
        ..Busqueda::default()
    };
    let toolchain = toolchain::ensure_toolchain(&busqueda, false).expect("basta el cuantizador");
    assert_eq!(toolchain.convert, None, "inventa un conversor que no está");
    assert!(
        matches!(toolchain.motor, ropero_forge::Motor::Fork { .. }),
        "dice {:?}",
        toolchain.motor
    );
    assert_eq!(
        toolchain.quantize,
        PathBuf::from("/llama.cpp/build/bin/llama-quantize")
    );

    // Y con la misma búsqueda, cuando sí hay que convertir sigue siendo
    // imprescindible (R-GGUF-55: el conversor no admite sustituto).
    let error = toolchain::ensure_toolchain(&busqueda, true).expect_err("hay que convertir");
    assert!(matches!(error, Error::SinMotor { .. }), "salió: {error:?}");
    assert!(
        matches!(&error, Error::SinMotor { falta, .. } if falta.contains("convert_hf_to_gguf.py")),
        "dice {error}"
    );
}

/// **Hallazgo 9**: sin motor elegido (sólo el cuantizador en el `PATH`), el motor
/// es `Ninguno` y no se puede afirmar un directorio que no hay.
#[test]
fn sin_motor_elegido_no_se_inventa_su_directorio() {
    let busqueda = Busqueda {
        quantize_en_path: Some(PathBuf::from("/usr/bin/llama-quantize")),
        ..Busqueda::default()
    };
    let toolchain =
        toolchain::ensure_toolchain(&busqueda, false).expect("el cuantizador en el PATH");
    assert!(
        matches!(toolchain.motor, ropero_forge::Motor::Ninguno),
        "dice {:?}",
        toolchain.motor
    );
    assert_eq!(toolchain.motor.dir(), None, "inventa un directorio");
    let nombre = toolchain.motor.nombre();
    assert!(nombre.contains("ningún"), "dice {nombre}");
}

// ─────────────────────────────────────────────────────────────────────────────
// El diagnóstico y la lista de quants del motor, con un motor **falso**.
// ─────────────────────────────────────────────────────────────────────────────

/// El cuantizador falso: con `--help` publica su lista de quants, como el
/// motor de verdad.
const QUANTIZE_FALSO: &str = r#"#!/bin/sh
if [ "$1" = "--help" ]; then
  echo "  0  or F32"
  echo "  1  or F16"
  echo " 15  or Q4_K : alias for Q4_K_M"
  echo " 15  or Q4_K_M"
  echo " 39  or NVFP4"
  exit 0
fi
exit 0
"#;

/// Un motor falso en un directorio temporal, con su entorno.
fn motor_falso() -> (PathBuf, ropero_core::Environment) {
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use std::sync::atomic::{AtomicU32, Ordering};
    static CONTADOR: AtomicU32 = AtomicU32::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let base =
        std::env::temp_dir().join(format!("ropero-forge-toolchain-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let motor = base.join("motor");
    fs::create_dir_all(motor.join("build/bin")).expect("crear el motor");
    fs::write(motor.join("convert_hf_to_gguf.py"), "# conversor\n").expect("el conversor");
    let quantize = motor.join("build/bin/llama-quantize");
    fs::write(&quantize, QUANTIZE_FALSO).expect("el cuantizador");
    fs::set_permissions(&quantize, fs::Permissions::from_mode(0o755)).expect("permisos");
    let mut vars = BTreeMap::new();
    vars.insert(
        "LLAMA_DIR".to_string(),
        motor.to_string_lossy().into_owned(),
    );
    vars.insert(
        "LLAMA_OFFICIAL_DIR".to_string(),
        base.join("no-oficial").to_string_lossy().into_owned(),
    );
    vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
    vars.insert("HOME".to_string(), base.to_string_lossy().into_owned());
    let env = ropero_core::Environment::new(vars, base.clone(), base.clone());
    (base, env)
}

/// Los quants que **publica el motor**: con su número de `llama_ftype`, y
/// **sin los alias** (el nombre bueno es el canónico).
#[test]
fn los_quants_del_motor_se_leen_de_su_ayuda() {
    let (base, env) = motor_falso();
    let (ruta, quants) = toolchain::quants_del_motor(&env).expect("el motor publica quants");
    assert!(ruta.ends_with("llama-quantize"), "dice {ruta:?}");
    let nombres: Vec<&str> = quants.iter().map(|(_, n)| n.as_str()).collect();
    assert!(nombres.contains(&"F16"), "dice {nombres:?}");
    assert!(nombres.contains(&"Q4_K_M"), "dice {nombres:?}");
    // El alias ("Q4_K") no sale: el 15 es el 15, y el nombre bueno es el
    // canónico (ver el módulo `quants`).
    assert!(!nombres.contains(&"Q4_K"), "dice {nombres:?}");
    // Y el número es el `llama_ftype` del GGUF.
    let q4 = quants
        .iter()
        .find(|(_, n)| n == "Q4_K_M")
        .expect("el Q4_K_M");
    assert_eq!(q4.0, 15);
    let _ = std::fs::remove_dir_all(&base);
}

/// El diagnóstico **sin motor**: el informe enseña dónde se buscó, y el
/// reporte lleva el error (que es lo que hace que el comando salga con 1).
#[test]
fn el_diagnostico_sin_motor_lleva_el_error() {
    let mut vars = std::collections::BTreeMap::new();
    vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
    vars.insert("LLAMA_DIR".to_string(), "/no/existe/fork".to_string());
    vars.insert(
        "LLAMA_OFFICIAL_DIR".to_string(),
        "/no/existe/oficial".to_string(),
    );
    let env = ropero_core::Environment::new(vars, "/no/existe/home", "/tmp");
    let reporte = toolchain::diagnostico(&env);
    let informe = reporte.lineas.join("\n");
    assert!(informe.contains("LLAMA_DIR"), "dice {informe}");
    assert!(informe.contains("LLAMA_OFFICIAL_DIR"), "dice {informe}");
    assert!(informe.contains("falta motor"), "dice {informe}");
    // Y el error: el motor que falta y la variable que lo apunta.
    let error = reporte.error.expect("el diagnóstico lleva el error");
    match &error {
        Error::SinMotor { falta, donde } => {
            assert!(falta.contains("convert_hf_to_gguf.py"), "dice {falta}");
            assert!(donde.contains("LLAMA_DIR"), "dice {donde}");
        }
        otro => panic!("esperaba un SinMotor, y salió: {otro:?}"),
    }
    assert_eq!(error.codigo(), 1);
}
