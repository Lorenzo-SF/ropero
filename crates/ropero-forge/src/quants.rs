//! La tabla de quants: número de `llama_ftype` ↔ nombre canónico.
//!
//! ## Por qué esta tabla y no la del motor
//!
//! El GGUF lleva el quant en `general.file_type`, y ese número es del
//! enum `llama_ftype` (`include/llama.h` de llama.cpp): `"Q4_K_M"` = 15,
//! `"NVFP4"` = 39. Es **justo** el espacio de números que imprime
//! `llama-quantize --help`, y con él se puede saber si un GGUF ya está en
//! el quant pedido sin tocar el disco (R-GGUF-7, R-GGUF-9).
//!
//! OJO: hay **otro** espacio de números, el `ggml_type` (el del tensor
//! predominante): NVFP4 es 40 allí y MXFP4_MOE no existe (39 allí es
//! MXFP4). Confundirlos hace que un GGUF en NVFP4 (39) se llame MXFP4.
//! La referencia de lectura es `ropero-code/bin/quant/tipos.txt`.
//!
//! ## Los alias
//!
//! El motor imprime `"15 or Q4_K : alias for Q4_K_M"` **y después**
//! `"15 or Q4_K_M"`: los dos son el 15, pero el nombre bueno es el
//! segundo. Por eso la tabla lleva el **canónico primero** dentro de cada
//! número: al buscar el nombre de un número se coge el **primero** que
//! aparece ([`nombre_de`]), y al buscar el número de un nombre se acepta
//! el alias también ([`numero_de`]).

/// La tabla, en el orden del enum (la misma de `tipos.txt`).
///
/// Dentro de cada número, el **canónico va primero** (ver el encabezado
/// del módulo). Es la lista de **respaldo**: la fuente de verdad es el
/// motor instalado (`llama-quantize --help`), que el gguffer consulta
/// cuando lo tiene delante.
pub const TABLA: &[(u32, &str)] = &[
    (0, "F32"),
    (1, "F16"),
    (2, "Q4_0"),
    (3, "Q4_1"),
    (7, "Q8_0"),
    (8, "Q5_0"),
    (9, "Q5_1"),
    (10, "Q2_K"),
    (11, "Q3_K_S"),
    (12, "Q3_K_M"),
    (12, "Q3_K"),
    (13, "Q3_K_L"),
    (14, "Q4_K_S"),
    (15, "Q4_K_M"),
    (15, "Q4_K"),
    (16, "Q5_K_S"),
    (17, "Q5_K_M"),
    (17, "Q5_K"),
    (18, "Q6_K"),
    (19, "IQ2_XXS"),
    (20, "IQ2_XS"),
    (21, "Q2_K_S"),
    (22, "IQ3_XS"),
    (23, "IQ3_XXS"),
    (24, "IQ1_S"),
    (25, "IQ4_NL"),
    (26, "IQ3_S"),
    (27, "IQ3_M"),
    (28, "IQ2_S"),
    (29, "IQ2_M"),
    (30, "IQ4_XS"),
    (31, "IQ1_M"),
    (32, "BF16"),
    (36, "TQ1_0"),
    (37, "TQ2_0"),
    (38, "MXFP4_MOE"),
    (39, "NVFP4"),
    (40, "Q1_0"),
    (41, "Q2_0"),
    (42, "Q8_CR"),
    (43, "TQ3_1S"),
    (44, "TQ4_1S"),
    (45, "Q5_CR"),
    (46, "Q6_CR"),
];

/// El número de `llama_ftype` de un quant, por nombre.
///
/// Acepta los alias (`Q4_K` es el 15, igual que `Q4_K_M`). Los nombres no
/// distinguen mayúsculas: `q4_k_m` es el mismo quant.
pub fn numero_de(nombre: &str) -> Option<u32> {
    let limpio = nombre.trim().to_ascii_uppercase();
    TABLA
        .iter()
        .find(|(_, n)| *n == limpio)
        .map(|(num, _)| *num)
}

/// El nombre canónico de un número de `llama_ftype`.
///
/// El canónico es el **primero** que aparece en la tabla para ese número
/// (ver el encabezado del módulo).
pub fn nombre_de(numero: u32) -> Option<&'static str> {
    TABLA
        .iter()
        .find(|(num, _)| *num == numero)
        .map(|(_, n)| *n)
}

/// Si un `llama_ftype` son pesos **ya cuantizados**.
///
/// F32 (0), F16 (1) y BF16 (32) son pesos sin quant: sobre ellos sí se
/// puede cuantizar. Todo lo demás ya es un quant, y **re-cuantizar** no
/// se puede (el motor lo rechaza: *requantizing from type X is disabled*,
/// R-GGUF-8).
pub fn es_quantizado(numero: u32) -> bool {
    !matches!(numero, 0 | 1 | 32)
}

/// Las líneas de `--list-quants`: una por quant, con nombre y número.
pub fn lista() -> Vec<String> {
    let mut vistas: Vec<u32> = Vec::new();
    let mut out = Vec::new();
    for (num, nombre) in TABLA {
        if vistas.contains(num) {
            continue;
        }
        vistas.push(*num);
        out.push(format!("{num:>3}  {nombre}"));
    }
    out
}
