//! Tests del preflight y de la elección de puerto (doc B §3.3, CLI v2 §8.1).
//!
//! Estas dos cosas son **puras**: no leen disco, no consultan el entorno y
//! no lanzan procesos. Por eso viven en `ropero-core` y se prueban sin
//! máquina.
//!
//! ## El preflight acumula razones
//!
//! Un motor incompatible explica **todas** las razones, no la primera
//! (doc B §3.3.2). Lo mismo vale aquí: si faltan el engine, el modelo y la
//! RAM, el mensaje dice las tres. Un usuario que arregla una cosa y
//! vuelve a fallar por la siguiente pierde el viaje.

use ropero_core::{PortRange, Preflight, PreflightError, pick_port};

/// Un rango de puertos de motor (doc CLI v2 §5.1).
fn rango() -> PortRange {
    PortRange::new(8100, 8199)
}

#[test]
fn el_puerto_eligido_es_el_primero_del_rango() {
    let p = pick_port(rango(), &[]).expect("hay puertos libres");
    assert_eq!(p, 8100);
}

#[test]
fn el_puerto_eligido_esta_dentro_del_rango() {
    let ocupados: Vec<u16> = (8100..8150).collect();
    let p = pick_port(rango(), &ocupados).expect("hay puertos libres");
    assert_eq!(p, 8150);
    assert!(rango().contains(p));
}

#[test]
fn un_puerto_ocupado_fuera_del_rango_no_impide_elegir() {
    // 8099 y 8200 están fuera del rango: que estén ocupados no quita el
    // primer slot libre de llama-server.
    assert_eq!(pick_port(rango(), &[8099]), Some(8100));
    assert_eq!(pick_port(rango(), &[8200]), Some(8100));
    // Y un puerto libre fuera del rango no se elige nunca.
    assert_eq!(pick_port(rango(), &[8099, 8200]), Some(8100));
}

#[test]
fn un_rango_invertido_no_elige_puerto() {
    let invertido = PortRange::new(8199, 8100);
    assert_eq!(pick_port(invertido, &[]), None);
}

#[test]
fn un_rango_vacio_no_elige_puerto() {
    let vacio = PortRange::new(8100, 8099);
    assert_eq!(pick_port(vacio, &[]), None);
}

#[test]
fn si_todo_el_rango_esta_ocupado_no_hay_puerto() {
    let ocupados: Vec<u16> = (8100..8200).collect();
    assert_eq!(pick_port(rango(), &ocupados), None);
}

#[test]
fn los_puertos_ocupados_que_no_estan_en_el_rango_no_importan() {
    // 9999 (strata) ocupado no quita un slot de llama-server.
    let p = pick_port(rango(), &[9999]).expect("hay puertos libres");
    assert_eq!(p, 8100);
}

// ─────────────────────────────────────────────────────────────────────────────
// El preflight
// ─────────────────────────────────────────────────────────────────────────────

/// Un preflight que se cumple todo.
fn preflight_ok() -> Preflight {
    Preflight {
        engine: "llama-server".to_string(),
        engine_installed: true,
        model_installed: true,
        vram_available_bytes: Some(24 * 1024 * 1024 * 1024),
        ram_available_bytes: Some(64 * 1024 * 1024 * 1024),
        context_requested: 32768,
        vram_required_bytes: Some(8 * 1024 * 1024 * 1024),
        ram_required_bytes: Some(16 * 1024 * 1024 * 1024),
        cpu: false,
    }
}

#[test]
fn un_preflight_que_se_cumple_no_falla() {
    assert!(preflight_ok().check().is_ok());
}

#[test]
fn el_preflight_acumula_todas_las_razones() {
    let p = Preflight {
        engine: "llama-server".to_string(),
        engine_installed: false,
        model_installed: false,
        vram_available_bytes: Some(2 * 1024 * 1024 * 1024),
        ram_available_bytes: None,
        context_requested: 32768,
        vram_required_bytes: Some(8 * 1024 * 1024 * 1024),
        ram_required_bytes: Some(16 * 1024 * 1024 * 1024),
        cpu: false,
    };
    let Err(error) = p.check() else {
        panic!("el preflight tenía que fallar");
    };
    let PreflightError::Reasons(reasons) = error;
    // El engine no instalado, el modelo no instalado y la VRAM que no
    // llega: tres razones, no una.
    assert_eq!(reasons.len(), 3, "razones={reasons:?}");
    assert!(
        reasons.iter().any(|r| r.contains("motor")),
        "razones={reasons:?}"
    );
    assert!(
        reasons.iter().any(|r| r.contains("modelo")),
        "razones={reasons:?}"
    );
    assert!(
        reasons.iter().any(|r| r.contains("VRAM")),
        "razones={reasons:?}"
    );
}

#[test]
fn el_preflight_dice_que_hay_que_instalar_el_engine() {
    let mut p = preflight_ok();
    p.engine_installed = false;
    let Err(error) = p.check() else {
        panic!("el preflight tenía que fallar");
    };
    let PreflightError::Reasons(reasons) = error;
    assert_eq!(reasons.len(), 1);
    // P-13: el mensaje dice QUÉ hacer, no sólo QUÉ pasa.
    assert!(
        reasons[0].contains("ropero install engines"),
        "razón={}",
        reasons[0]
    );
}

#[test]
fn el_preflight_dice_que_hay_que_instalar_el_modelo() {
    let mut p = preflight_ok();
    p.model_installed = false;
    let Err(error) = p.check() else {
        panic!("el preflight tenía que fallar");
    };
    let PreflightError::Reasons(reasons) = error;
    assert_eq!(reasons.len(), 1);
    assert!(
        reasons[0].contains("ropero install models"),
        "razón={}",
        reasons[0]
    );
}

#[test]
fn con_cpu_se_exige_ram_y_no_vram() {
    let mut p = preflight_ok();
    p.cpu = true;
    p.vram_available_bytes = None;
    p.vram_required_bytes = Some(8 * 1024 * 1024 * 1024);
    // Con --cpu la VRAM no se mira: el modelo se sirve desde la RAM.
    assert!(p.check().is_ok());
}

#[test]
fn con_cpu_la_ram_insuficiente_falla() {
    let mut p = preflight_ok();
    p.cpu = true;
    p.ram_available_bytes = Some(1024 * 1024 * 1024);
    let Err(error) = p.check() else {
        panic!("el preflight tenía que fallar");
    };
    let PreflightError::Reasons(reasons) = error;
    assert_eq!(reasons.len(), 1, "razones={reasons:?}");
    assert!(reasons[0].contains("RAM"), "razón={}", reasons[0]);
}

#[test]
fn sin_cpu_la_vram_insuficiente_falla() {
    let mut p = preflight_ok();
    p.vram_available_bytes = Some(1024 * 1024 * 1024);
    let Err(error) = p.check() else {
        panic!("el preflight tenía que fallar");
    };
    let PreflightError::Reasons(reasons) = error;
    assert_eq!(reasons.len(), 1, "razones={reasons:?}");
    assert!(reasons[0].contains("VRAM"), "razón={}", reasons[0]);
}

#[test]
fn un_recurso_que_no_se_pudo_leer_no_bloquea() {
    // El dato ausente no es un dato malo: si no se pudo leer la VRAM, no se
    // inventa que falta (doc B §3.2.3: lo que no se puede leer no se inventa).
    let mut p = preflight_ok();
    p.vram_available_bytes = None;
    p.ram_available_bytes = None;
    assert!(p.check().is_ok());
}

#[test]
fn el_mensaje_del_preflight_dice_todas_las_razones() {
    let mut p = preflight_ok();
    p.engine_installed = false;
    p.model_installed = false;
    let Err(error) = p.check() else {
        panic!("el preflight tenía que fallar");
    };
    let texto = error.to_string();
    assert!(texto.contains("llama-server"), "mensaje={texto}");
    assert!(texto.contains("modelo"), "mensaje={texto}");
    // Y el mensaje dice qué hacer.
    assert!(texto.contains("ropero install"), "mensaje={texto}");
}
