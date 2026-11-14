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
    let error = toolchain::ensure_toolchain(&Busqueda::default()).expect_err("no hay motor");
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
    let toolchain = toolchain::ensure_toolchain(&busqueda).expect("el fork está");
    assert_eq!(toolchain.motor.dir(), std::path::Path::new("/llama.cpp"));
    assert_eq!(
        toolchain.convert,
        PathBuf::from("/llama.cpp/convert_hf_to_gguf.py")
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
    let toolchain = toolchain::ensure_toolchain(&busqueda).expect("el oficial está");
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
    let error = toolchain::ensure_toolchain(&busqueda).expect_err("falta el cuantizador");
    match &error {
        Error::MotorIncompleto { falta, donde } => {
            assert!(falta.contains("llama-quantize"), "dice {falta}");
            assert!(donde.contains("PATH"), "dice {donde}");
            assert!(donde.contains("build/bin"), "dice {donde}");
        }
        otro => panic!("esperaba un MotorIncompleto, y salió: {otro:?}"),
    }
}
