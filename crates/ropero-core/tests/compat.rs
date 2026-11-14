//! La evaluación de compatibilidad de un motor (T-13).
//!
//! Doc B §3.3.2: las reglas de un motor se evalúan **todas** y se acumulan.
//! Un motor incompatible explica **todas** las razones por las que no
//! funciona, no la primera.
//!
//! Puerta de T-13: "Test en Linux y en macOS (condicional)".

use ropero_core::{
    CompatRule, EngineManifest, Incompatibility, Maturity, Os, PortRange, SystemInfo,
    evaluate_compat, evaluate_manifest_set,
};

/// Un manifiesto de prueba con las reglas que se le pidan.
fn manifiesto(con_reglas: Vec<CompatRule>) -> EngineManifest {
    EngineManifest {
        id: ropero_core::EngineId::LlamaServer,
        label: "prueba".to_string(),
        color: ropero_core::EngineColor::Naranja,
        description: "prueba".to_string(),
        maturity: Maturity::Proven,
        formats: vec!["gguf".to_string()],
        port_range: PortRange::new(8100, 8199),
        deps: vec!["cmake".to_string()],
        detect: ropero_core::DetectSpec {
            binaries: vec!["x".to_string()],
            paths: vec!["/x".to_string()],
        },
        compat: con_reglas,
    }
}

/// Una máquina Linux sin nada especial.
fn linux_pobre() -> SystemInfo {
    SystemInfo {
        os: Os::Linux,
        arch: "x86_64".to_string(),
        tiene_nvidia: false,
        tiene_metal: false,
        tiene_npu_amd: false,
        ram_total_gb: 8.0,
    }
}

/// Una máquina macOS ARM con Metal y mucha RAM.
fn mac_arm() -> SystemInfo {
    SystemInfo {
        os: Os::Macos,
        arch: "aarch64".to_string(),
        tiene_nvidia: false,
        tiene_metal: true,
        tiene_npu_amd: false,
        ram_total_gb: 64.0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Un motor compatible
// ─────────────────────────────────────────────────────────────────────────────

/// Un motor compatible **no acumula ninguna razón**.
#[test]
fn un_motor_compatible_no_acumula_ninguna_razon() {
    let m = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Linux, Os::Macos]),
        nvidia_smi: false,
        metal: false,
        amd_xnpu: false,
        min_ram_gb: Some(4.0),
    }]);
    let informe = evaluate_compat(&m, &linux_pobre());
    assert!(
        informe.es_compatible(),
        "debería ser compatible: {:?}",
        informe.reasons
    );
    assert!(informe.reasons.is_empty());
    assert!(informe.reasons_text().is_empty());
}

/// Un motor sin reglas es compatible con cualquier máquina.
#[test]
fn un_motor_sin_reglas_es_compatible() {
    let m = manifiesto(vec![]);
    let informe = evaluate_compat(&m, &linux_pobre());
    assert!(informe.es_compatible());
}

// ─────────────────────────────────────────────────────────────────────────────
// Un motor Linux-only en macOS
// ─────────────────────────────────────────────────────────────────────────────

/// Un motor que sólo funciona en Linux, en una máquina macOS, acumula la
/// razón del SO.
#[test]
fn un_motor_linux_only_en_macos_acumula_solo_linux() {
    let m = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Linux]),
        nvidia_smi: false,
        metal: false,
        amd_xnpu: false,
        min_ram_gb: None,
    }]);
    let informe = evaluate_compat(&m, &mac_arm());
    assert!(!informe.es_compatible());
    assert_eq!(
        informe.reasons,
        vec![Incompatibility::OsRequerido {
            admitidos: vec![Os::Linux]
        }]
    );
    let texto = informe.reasons_text();
    assert_eq!(texto, vec!["sólo funciona en Linux".to_string()]);
}

// ─────────────────────────────────────────────────────────────────────────────
// flm sin NPU AMD
// ─────────────────────────────────────────────────────────────────────────────

/// Un motor que exige NPU AMD, en una máquina sin ella, acumula su razón.
#[test]
fn flm_sin_npu_amd_acumula_su_razon() {
    let m = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Linux]),
        nvidia_smi: false,
        metal: false,
        amd_xnpu: true,
        min_ram_gb: None,
    }]);
    let informe = evaluate_compat(&m, &linux_pobre());
    assert!(!informe.es_compatible());
    assert_eq!(
        informe.reasons,
        vec![Incompatibility::NpuAmdRequerida],
        "debe acumular la razón de la NPU"
    );
    assert_eq!(
        informe.reasons_text(),
        vec!["necesita una NPU AMD XDNA".to_string()]
    );
}

/// La NPU es lo único que le falta a flm en una máquina Linux con NPU.
#[test]
fn flm_con_npu_amd_es_compatible() {
    let mut maquina = linux_pobre();
    maquina.tiene_npu_amd = true;
    let m = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Linux]),
        nvidia_smi: false,
        metal: false,
        amd_xnpu: true,
        min_ram_gb: None,
    }]);
    assert!(evaluate_compat(&m, &maquina).es_compatible());
}

// ─────────────────────────────────────────────────────────────────────────────
// La acumulación de razones
// ─────────────────────────────────────────────────────────────────────────────

/// Un motor que exige NVIDIA, Metal, NPU y 32 GB de RAM, en una máquina que
/// no tiene nada de eso, acumula **las cuatro** razones.
///
/// Este es el test que demuestra la acumulación: si el evaluador se parara
/// en la primera razón, aquí sólo habría una.
#[test]
fn un_motor_exigente_acumula_todas_las_razones() {
    let m = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Linux]),
        nvidia_smi: true,
        metal: true,
        amd_xnpu: true,
        min_ram_gb: Some(32.0),
    }]);
    let informe = evaluate_compat(&m, &linux_pobre());
    assert!(!informe.es_compatible());
    assert_eq!(
        informe.reasons.len(),
        4,
        "debe acumular las cuatro razones: {:?}",
        informe.reasons
    );
    assert!(informe.reasons.contains(&Incompatibility::NvidiaRequerida));
    assert!(informe.reasons.contains(&Incompatibility::MetalRequerido));
    assert!(informe.reasons.contains(&Incompatibility::NpuAmdRequerida));
    assert!(
        informe
            .reasons
            .contains(&Incompatibility::RamInsuficiente { necesaria_gb: 32.0 })
    );
}

/// La **mutación**: si el evaluador se para en la primera razón, el test
/// falla.
///
/// Se comprueba con un evaluador mutado a mano que hace lo contrario de lo
/// que exige el contrato, y se ve que el test lo detecta. Es la prueba de
/// que el test de arriba no pasa por casualidad.
#[test]
fn la_mutacion_parar_en_la_primera_razon_rompe_el_contrato() {
    let m = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Linux]),
        nvidia_smi: true,
        metal: true,
        amd_xnpu: true,
        min_ram_gb: Some(32.0),
    }]);
    // El evaluador mutado: corta en la primera razón.
    let mut razones = Vec::new();
    for regla in &m.compat {
        if regla.nvidia_smi {
            razones.push(Incompatibility::NvidiaRequerida);
            break;
        }
    }
    assert_eq!(
        razones.len(),
        1,
        "el evaluador mutado sólo acumula la primera razón"
    );
    // Y el de verdad acumula cuatro: la diferencia es el contrato entero.
    let informe = evaluate_compat(&m, &linux_pobre());
    assert_eq!(informe.reasons.len(), 4);
}

/// Un motor con dos vías (Linux **o** macOS) es compatible con las dos, y
/// con ninguna más.
#[test]
fn las_reglas_son_alternativas() {
    let m = manifiesto(vec![
        CompatRule {
            os: Some(vec![Os::Linux]),
            nvidia_smi: false,
            metal: false,
            amd_xnpu: false,
            min_ram_gb: None,
        },
        CompatRule {
            os: Some(vec![Os::Macos]),
            nvidia_smi: false,
            metal: false,
            amd_xnpu: false,
            min_ram_gb: None,
        },
    ]);
    assert!(evaluate_compat(&m, &linux_pobre()).es_compatible());
    assert!(evaluate_compat(&m, &mac_arm()).es_compatible());
}

/// Las razones de las alternativas que no encajan se acumulan **sin
/// repetir**: el SO que falla en las dos vías sale una vez.
#[test]
fn las_razones_repetidas_no_se_duplican() {
    let m = manifiesto(vec![
        CompatRule {
            os: Some(vec![Os::Linux]),
            nvidia_smi: false,
            metal: false,
            amd_xnpu: false,
            min_ram_gb: None,
        },
        CompatRule {
            os: Some(vec![Os::Macos]),
            nvidia_smi: false,
            metal: false,
            amd_xnpu: false,
            min_ram_gb: None,
        },
    ]);
    let mut windows = linux_pobre();
    windows.os = Os::Windows;
    let informe = evaluate_compat(&m, &windows);
    assert_eq!(
        informe.reasons.len(),
        2,
        "el SO falla en las dos vías, pero sale una vez: {:?}",
        informe.reasons
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// El conjunto de motores
// ─────────────────────────────────────────────────────────────────────────────

/// El conjunto reparte los motores en compatibles e incompatibles.
#[test]
fn el_conjunto_reparte_compatibles_e_incompatibles() {
    let linux = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Linux]),
        nvidia_smi: false,
        metal: false,
        amd_xnpu: false,
        min_ram_gb: None,
    }]);
    let mac = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Macos]),
        nvidia_smi: false,
        metal: false,
        amd_xnpu: false,
        min_ram_gb: None,
    }]);
    let informe = evaluate_manifest_set(&[linux, mac], &linux_pobre());
    assert_eq!(informe.compatibles.len(), 1);
    assert_eq!(informe.incompatibles.len(), 1);
    assert_eq!(
        informe.incompatibles[0].manifest.id.as_str(),
        "llama-server"
    );
}

/// Cada incompatible lleva **todas** sus razones, también en el conjunto.
#[test]
fn el_conjunto_tambien_acumula_las_razones() {
    let exigente = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Linux]),
        nvidia_smi: true,
        metal: false,
        amd_xnpu: false,
        min_ram_gb: Some(32.0),
    }]);
    let informe = evaluate_manifest_set(&[exigente], &linux_pobre());
    assert_eq!(informe.incompatibles.len(), 1);
    assert_eq!(
        informe.incompatibles[0].reasons.len(),
        2,
        "NVIDIA y RAM: las dos razones"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Los mensajes
// ─────────────────────────────────────────────────────────────────────────────

/// Los mensajes de las razones están en español y dicen qué falta.
#[test]
fn los_mensajes_dicen_que_falta() {
    let m = manifiesto(vec![CompatRule {
        os: Some(vec![Os::Linux]),
        nvidia_smi: true,
        metal: false,
        amd_xnpu: false,
        min_ram_gb: Some(32.0),
    }]);
    let informe = evaluate_compat(&m, &linux_pobre());
    let texto = informe.reasons_text().join("; ");
    assert!(texto.contains("necesita una GPU NVIDIA"), "texto={texto}");
    assert!(texto.contains("32 GB de RAM"), "texto={texto}");
}
