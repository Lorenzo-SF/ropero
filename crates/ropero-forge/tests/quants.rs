//! La tabla de quants: nombre ↔ número de `llama_ftype`.
//!
//! La referencia es `ropero-code/bin/quant/tipos.txt` (R-GGUF-54: la lista
//! del motor manda, y esta es la de respaldo). Los números son los del
//! enum `llama_ftype` — los que lleva el GGUF en `general.file_type` —
//! **no** los del `ggml_type` (NVFP4 es 39 aquí y 40 allá).

use ropero_forge::quants;

/// Los números clave, con su nombre canónico.
#[test]
fn los_numeros_de_los_quants_son_los_del_llama_ftype() {
    assert_eq!(quants::numero_de("F16"), Some(1));
    assert_eq!(quants::numero_de("BF16"), Some(32));
    assert_eq!(quants::numero_de("IQ2_M"), Some(29));
    assert_eq!(quants::numero_de("Q4_K_M"), Some(15));
    assert_eq!(quants::numero_de("NVFP4"), Some(39));
    assert_eq!(quants::numero_de("MXFP4_MOE"), Some(38));
}

/// El nombre de un número es el **canónico**, no el alias: el 15 es
/// `Q4_K_M` (no `Q4_K`), el 12 es `Q3_K_M` (no `Q3_K`).
#[test]
fn el_nombre_de_un_numero_es_el_canonico() {
    assert_eq!(quants::nombre_de(15), Some("Q4_K_M"));
    assert_eq!(quants::nombre_de(12), Some("Q3_K_M"));
    assert_eq!(quants::nombre_de(17), Some("Q5_K_M"));
    assert_eq!(quants::nombre_de(39), Some("NVFP4"));
    assert_eq!(quants::nombre_de(9999), None);
}

/// Los alias **se aceptan** como nombre pedido (el motor los entiende),
/// aunque el nombre que se declara en la procedencia sea el canónico.
#[test]
fn los_alias_se_aceptan_como_nombre() {
    assert_eq!(quants::numero_de("Q4_K"), Some(15));
    assert_eq!(quants::numero_de("Q3_K"), Some(12));
}

/// Los pesos en F32 (0), F16 (1) y BF16 (32) **no** están cuantizados:
/// todo lo demás sí.
#[test]
fn f32_f16_y_bf16_no_son_quants() {
    assert!(!quants::es_quantizado(0));
    assert!(!quants::es_quantizado(1));
    assert!(!quants::es_quantizado(32));
    assert!(quants::es_quantizado(15));
    assert!(quants::es_quantizado(39));
}

/// `--list-quants` lista nombre y número, uno por línea, con la pareja
/// completa (que es lo que va en `general.file_type`).
#[test]
fn la_lista_de_quants_salen_todos() {
    let lista = quants::lista();
    assert!(
        lista
            .iter()
            .any(|l| l.contains("IQ2_M") && l.contains("29"))
    );
    assert!(
        lista
            .iter()
            .any(|l| l.contains("NVFP4") && l.contains("39"))
    );
    assert!(
        lista
            .iter()
            .any(|l| l.contains("Q4_K_M") && l.contains("15"))
    );
    // Un nombre desconocido no aparece.
    assert!(!lista.iter().any(|l| l.contains("Q9_K")));
}
