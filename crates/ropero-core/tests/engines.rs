//! Manifiestos de motor: carga, validación y unicidad (T-12).
//!
//! Doc B §4.3: un motor es **un fichero de datos** (`engines/<id>.toml`).
//! El manifiesto lleva el id canónico, la etiqueta, el color, los formatos
//! que acepta, la madurez, el rango de puertos, cómo se detecta que está
//! instalado y de qué dependencias depende.
//!
//! Puerta de T-12: todos los motores canónicos cargan y validan.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ropero_core::{
    EngineId, EngineManifest, Maturity, Os, PortRange, evaluate_manifest_set,
    parse_engine_manifest, validate_engine_set,
};

/// La raíz de los manifiestos: `<repo-root>/engines`.
fn base_engines() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("engines")
}

/// Los ids canónicos, en el orden de la leyenda de motores.
const LOS_MOTORES: &[(&str, EngineId)] = &[
    ("llama-server", EngineId::LlamaServer),
    ("turbo-llama", EngineId::TurboLlama),
    ("fastflowllm", EngineId::FastFlowLm),
    ("mlx-serve", EngineId::MlxServe),
    ("airllm", EngineId::AirLlm),
    ("tensorrt-llm", EngineId::TensorRtLlm),
    ("vllm", EngineId::Vllm),
    ("strata", EngineId::Strata),
    ("ignis", EngineId::Ignis),
    ("basalt", EngineId::Basalt),
];

/// Carga un manifiesto del repo real.
fn carga(id: &str) -> EngineManifest {
    let path = base_engines().join(format!("{id}.toml"));
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("leer {}: {e}", path.display()));
    parse_engine_manifest(&text, &path)
        .unwrap_or_else(|e| panic!("parsear {}: {e}", path.display()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Los manifiestos cargan y validan
// ─────────────────────────────────────────────────────────────────────────────

/// Los manifiestos existen en `engines/` y cargan sin errores.
#[test]
fn los_diez_manifiestos_cargan() {
    for (id, motor) in LOS_MOTORES {
        let manifiesto = carga(id);
        assert_eq!(manifiesto.id, *motor, "manifiesto={id}");
        assert_eq!(manifiesto.id.as_str(), *id, "manifiesto={id}");
    }
}

/// Cada manifiesto pasa la validación de su conjunto.
#[test]
fn los_diez_manifiestos_validan() {
    let manifiestos: Vec<EngineManifest> = LOS_MOTORES.iter().map(|(id, _)| carga(id)).collect();
    let errores = validate_engine_set(&manifiestos);
    assert!(
        errores.is_empty(),
        "los manifiestos tienen {} errores: {:?}",
        errores.len(),
        errores
    );
}

/// Cada manifiesto tiene **el nombre de fichero** de su id.
#[test]
fn cada_manifiesto_vive_en_su_fichero() {
    for (id, _) in LOS_MOTORES {
        let path = base_engines().join(format!("{id}.toml"));
        assert!(path.is_file(), "falta {}", path.display());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// El contenido de cada manifiesto
// ─────────────────────────────────────────────────────────────────────────────

/// Cada motor tiene color, etiqueta, descripción y formatos.
///
/// El color es el de la leyenda del doc CLI v2 §4: naranja, amarillo, rojo,
/// gris, blanco, verde pistacho, morado y verde pálido.
#[test]
fn cada_motor_tiene_color_etiqueta_descripcion_y_formatos() {
    for (id, _) in LOS_MOTORES {
        let m = carga(id);
        assert!(!m.label.is_empty(), "{id}: la etiqueta está vacía");
        assert!(!m.description.is_empty(), "{id}: la descripción está vacía");
        assert!(!m.formats.is_empty(), "{id}: no declara formatos");
        assert!(!m.color.as_str().is_empty(), "{id}: no declara color");
        assert!(
            !m.deps.is_empty(),
            "{id}: no declara dependencias (las necesita el doctor)"
        );
        assert!(
            !m.detect.binaries.is_empty() || !m.detect.paths.is_empty(),
            "{id}: no dice cómo detectarlo"
        );
    }
}

/// Los colores son los de la leyenda del doc CLI v2 §4.
#[test]
fn los_colores_son_los_de_la_leyenda() {
    let esperado = [
        ("llama-server", "naranja"),
        ("turbo-llama", "amarillo"),
        ("fastflowllm", "rojo"),
        ("mlx-serve", "gris"),
        ("airllm", "blanco"),
        ("tensorrt-llm", "verde-pistacho"),
        ("vllm", "morado"),
        ("strata", "verde-pálido"),
    ];
    for (id, color) in esperado {
        assert_eq!(carga(id).color.as_str(), color, "manifiesto={id}");
    }
}

/// Los rangos de puertos son los del doc CLI v2 §5.1.
#[test]
fn los_rangos_de_puertos_son_los_del_doc() {
    let esperado = [
        ("llama-server", 9980, 9989),
        ("turbo-llama", 9990, 9999),
        ("mlx-serve", 9960, 9969),
        ("fastflowllm", 9970, 9979),
        ("tensorrt-llm", 9950, 9959),
        ("vllm", 9940, 9949),
        ("airllm", 9930, 9939),
        ("strata", 9920, 9929),
    ];
    for (id, min, max) in esperado {
        let m = carga(id);
        assert_eq!(m.port_range, PortRange::new(min, max), "manifiesto={id}");
    }
}

/// Turbo-Llama conserva el puerto base 9999 del ropero viejo.
#[test]
fn el_rango_de_turbo_llama_incluye_el_9999() {
    let rango = carga("turbo-llama").port_range;
    assert!(rango.contains(9999), "rango={rango:?}");
}

/// La madurez distingue los motores probados de los que no (T-14).
#[test]
fn la_madurez_distingue_probados_de_no_probados() {
    let m = carga("llama-server");
    assert_eq!(m.maturity, Maturity::Proven);
    assert_eq!(m.maturity.as_str(), "proven");
    // Y hay motores que no están probados: el conjunto no es todo "proven".
    let manifiestos: Vec<EngineManifest> = LOS_MOTORES.iter().map(|(id, _)| carga(id)).collect();
    let probados = manifiestos
        .iter()
        .filter(|m| m.maturity == Maturity::Proven)
        .count();
    let no_probados = manifiestos.len() - probados;
    assert_eq!(probados, 3, "los 3 motores probados");
    assert_eq!(
        no_probados, 7,
        "los 7 motores parciales, no probados o integrados"
    );
}

/// Las restricciones de SO y hardware del doc CLI v2 §4.
///
/// Las reglas son **alternativas** y dentro de cada una las condiciones son
/// **acumulativas**: por eso la NPU de flm y la NVIDIA de tensorrt-llm van
/// en la misma regla que su SO, no en reglas sueltas.
#[test]
fn las_restricciones_de_hardware_son_las_del_doc() {
    // flm: sólo Linux, sólo CPU AMD compatible (NPU).
    let flm = carga("fastflowllm");
    let regla_flm = flm
        .compat
        .iter()
        .find(|r| r.os() == Some(&[Os::Linux]))
        .expect("flm debe tener la regla de Linux");
    assert!(
        regla_flm.amd_xnpu(),
        "flm debe exigir NPU AMD en su vía de Linux: {:?}",
        flm.compat
    );

    // mlx: sólo macOS ARM (Metal).
    let mlx = carga("mlx-serve");
    let regla_mlx = mlx
        .compat
        .iter()
        .find(|r| r.os() == Some(&[Os::Macos]))
        .expect("mlx debe tener la regla de macOS");
    assert!(
        regla_mlx.metal(),
        "mlx debe exigir Metal en su vía de macOS: {:?}",
        mlx.compat
    );

    // tensorrt-llm: sólo Linux + dGPU RTX (NVIDIA).
    let trt = carga("tensorrt-llm");
    let regla_trt = trt
        .compat
        .iter()
        .find(|r| r.os() == Some(&[Os::Linux]))
        .expect("tensorrt-llm debe tener la regla de Linux");
    assert!(
        regla_trt.nvidia_smi(),
        "tensorrt-llm debe exigir NVIDIA en su vía de Linux: {:?}",
        trt.compat
    );

    // vllm, airllm y strata: Linux y macOS.
    for id in ["vllm", "airllm", "strata"] {
        let m = carga(id);
        let sistemas: Vec<&[Os]> = m.compat.iter().filter_map(|r| r.os()).collect();
        assert!(
            sistemas.contains(&&[Os::Linux][..]) && sistemas.contains(&&[Os::Macos][..]),
            "{id} debe admitir Linux y macOS: {:?}",
            m.compat
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Las reglas de validación
// ─────────────────────────────────────────────────────────────────────────────

/// Un manifiesto con un campo desconocido se **rechaza**.
#[test]
fn un_manifiesto_con_un_campo_desconocido_se_rechaza() {
    let texto = r#"
id = "llama-server"
label = "x"
color = "naranja"
description = "x"
maturity = "proven"
formats = ["gguf"]
port_min = 8100
port_max = 8199
deps = ["cmake"]
clave_que_no_existe = "x"

[detect]
binaries = ["llama-server"]
paths = ["/usr/bin/llama-server"]
"#;
    let err = parse_engine_manifest(texto, Path::new("engines/llama-server.toml"))
        .expect_err("un campo desconocido debe rechazarse");
    let mensaje = err.to_string();
    assert!(
        mensaje.contains("clave_que_no_existe"),
        "el mensaje debe nombrar el campo culpable: {mensaje}"
    );
}

/// Un id que no es un motor conocido se rechaza.
#[test]
fn un_id_desconocido_se_rechaza() {
    let texto = r#"
id = "no-existe"
label = "x"
color = "naranja"
description = "x"
maturity = "proven"
formats = ["gguf"]
port_min = 8100
port_max = 8199
deps = ["cmake"]

[detect]
binaries = ["x"]
paths = ["/x"]
"#;
    let err = parse_engine_manifest(texto, Path::new("engines/no-existe.toml"))
        .expect_err("un id desconocido debe rechazarse");
    assert!(err.to_string().contains("motor desconocido"), "{err}");
}

/// Dos manifiestos con el mismo id es un error.
#[test]
fn dos_manifiestos_con_el_mismo_id_es_un_error() {
    let manifiestos = vec![carga("llama-server"), carga("llama-server")];
    let errores = validate_engine_set(&manifiestos);
    assert!(
        !errores.is_empty(),
        "dos manifiestos con el mismo id deben ser un error"
    );
    assert!(
        errores[0].contains("llama-server"),
        "el mensaje debe nombrar el id: {}",
        errores[0]
    );
}

/// Dos rangos de puertos que se solapan es un error.
#[test]
fn dos_rangos_de_puertos_que_se_solapan_es_un_error() {
    let mut primero = carga("llama-server");
    let mut segundo = carga("turbo-llama");
    segundo.port_range = PortRange::new(8100, 8250);
    primero.port_range = PortRange::new(8100, 8199);
    let errores = validate_engine_set(&[primero, segundo]);
    assert!(
        !errores.is_empty(),
        "los rangos solapados deben ser un error"
    );
    assert!(
        errores[0].contains("8100"),
        "el mensaje debe nombrar el puerto en conflicto: {}",
        errores[0]
    );
}

/// Un rango de puertos invertido (min > max) es un error.
#[test]
fn un_rango_de_puertos_invertido_es_un_error() {
    let errores = validate_engine_set(&[carga("llama-server")]);
    assert!(errores.is_empty(), "el manifiesto real es válido");

    let mut roto = carga("llama-server");
    roto.port_range = PortRange::new(8199, 8100);
    let errores = validate_engine_set(&[roto]);
    assert!(
        !errores.is_empty(),
        "un rango invertido (min > max) debe ser un error"
    );
}

/// Un manifiesto sin color, sin formatos o sin dependencias es un error:
/// son los datos que el CLI y el doctor necesitan.
#[test]
fn un_manifiesto_incompleto_es_un_error() {
    let mut sin_color = carga("llama-server");
    sin_color.color = ropero_core::EngineColor::default();
    assert!(
        !validate_engine_set(&[sin_color]).is_empty(),
        "un motor sin color debe ser un error"
    );

    let mut sin_formatos = carga("llama-server");
    sin_formatos.formats.clear();
    assert!(
        !validate_engine_set(&[sin_formatos]).is_empty(),
        "un motor sin formatos debe ser un error"
    );

    let mut sin_deps = carga("llama-server");
    sin_deps.deps.clear();
    assert!(
        !validate_engine_set(&[sin_deps]).is_empty(),
        "un motor sin dependencias debe ser un error"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// El conjunto completo
// ─────────────────────────────────────────────────────────────────────────────

/// El conjunto real de los diez manifiestos valida entero y sin ids repetidos.
#[test]
fn el_conjunto_real_de_manifiestos_es_valido() {
    let manifiestos: Vec<EngineManifest> = LOS_MOTORES.iter().map(|(id, _)| carga(id)).collect();
    assert_eq!(manifiestos.len(), 10);
    let errores = validate_engine_set(&manifiestos);
    assert!(
        errores.is_empty(),
        "el conjunto real tiene {} errores: {:?}",
        errores.len(),
        errores
    );
    let ids: BTreeSet<&str> = manifiestos.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids.len(), 10, "los diez ids son distintos");
}

/// El conjunto real cubre los ids canónicos, sin sobras ni faltas.
#[test]
fn el_conjunto_real_cubre_los_ids_canonicos() {
    let manifiestos: Vec<EngineManifest> = LOS_MOTORES.iter().map(|(id, _)| carga(id)).collect();
    let ids: BTreeSet<String> = manifiestos
        .iter()
        .map(|m| m.id.as_str().to_string())
        .collect();
    let canonicos: BTreeSet<String> = ropero_core::CANONICAL_IDS
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(ids, canonicos);
}

/// Los manifiestos se evalúan en conjunto: `evaluate_manifest_set` reparte
/// los motores en compatibles e incompatibles.
#[test]
fn el_conjunto_real_se_evalua() {
    let manifiestos: Vec<EngineManifest> = LOS_MOTORES.iter().map(|(id, _)| carga(id)).collect();
    let linux = ropero_core::SystemInfo {
        os: Os::Linux,
        arch: "x86_64".to_string(),
        tiene_nvidia: false,
        tiene_metal: false,
        tiene_npu_amd: false,
        ram_total_gb: 32.0,
    };
    let informe = evaluate_manifest_set(&manifiestos, &linux);
    // En Linux sin NVIDIA ni NPU AMD: incompatibles flm (NPU) y tensorrt-llm
    // (NVIDIA), y compatibles el resto que admite Linux.
    let incompatibles: BTreeSet<&str> = informe
        .incompatibles
        .iter()
        .map(|r| r.manifest.id.as_str())
        .collect();
    assert!(incompatibles.contains("fastflowllm"), "{incompatibles:?}");
    assert!(incompatibles.contains("tensorrt-llm"), "{incompatibles:?}");
    assert!(!incompatibles.contains("llama-server"), "{incompatibles:?}");
    assert!(!incompatibles.contains("vllm"), "{incompatibles:?}");
}
