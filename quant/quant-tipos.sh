#!/usr/bin/env bash
# =============================================================================
# quant-tipos.sh — la lista de tipos de quant: numero <-> nombre
# =============================================================================
# El GGUF lleva general.file_type, que es el NUMERO del tipo de ggml. Para
# poner el nombre en el nombre del repo y en el model card hace falta
# traducirlo.
#
# La lista sale del motor instalado (llama-quantize --help), que es la
# fuente de verdad. Pero si el motor no esta en el PATH, se usa la lista de
# respaldo de abajo: son los tipos del enum ggml_type de llama.cpp
# (ggml/include/ggml.h), con sus numeros exactos.
#
# La lista de respaldo se usa SOLO si no hay motor. Si el motor y la lista
# no coinciden, manda el motor.
# =============================================================================

# numero=nombre
QUANTS_RESPALDO="0=F32 1=F16 2=Q4_0 3=Q4_1 6=Q5_0 7=Q5_1 8=Q8_0 9=Q8_1 \
10=Q2_K 11=Q3_K 12=Q4_K 13=Q5_K 14=Q6_K 15=Q8_K 16=IQ2_XXS 17=IQ2_XS \
18=IQ3_XXS 19=IQ1_S 20=IQ4_NL 21=IQ3_S 22=IQ2_S 23=IQ4_XS 29=IQ1_M 30=BF16 \
34=TQ1_0 35=TQ2_0 39=MXFP4 40=NVFP4 41=Q1_0 42=Q2_0"

# ── la lista del motor, si esta ───────────────────────────────────────────
_quantize_bin() {
    local c
    for c in "${LLAMA_BIN:-}" "$HOME/llama.cpp/build/bin/llama-quantize" \
             "$(command -v llama-quantize 2>/dev/null || true)"; do
        [[ -n "$c" && -x "$c" ]] && { printf '%s' "$c"; return 0; }
    done
    return 1
}

quants_num() {
    local qbin lista
    if qbin="$(_quantize_bin)"; then
        lista="$("$qbin" --help 2>&1 | sed -nE 's/^[[:space:]]*([0-9]+)[[:space:]]+or[[:space:]]+([A-Z0-9_]+).*/\1=\2/p' | tr '\n' ' ')"
        if [[ -n "$lista" ]]; then
            printf '%s' "$lista"
            return 0
        fi
    fi
    printf '%s' "$QUANTS_RESPALDO"
}

# ── el nombre de un tipo por su numero ────────────────────────────────────
quant_nombre() {
    local num="$1" lista
    lista="$(quants_num)"
    printf '%s' "$lista" | tr ' ' '\n' | grep -E "^${num}=" | head -1 | cut -d= -f2
}

# ── el numero de un tipo por su nombre ────────────────────────────────────
quant_numero() {
    local nombre="$1" lista
    lista="$(quants_num)"
    printf '%s' "$lista" | tr ' ' '\n' | grep -E "=${nombre}$" | head -1 | cut -d= -f1
}

# ── es un nombre de quant? ────────────────────────────────────────────────
es_quant() {
    local nombre="$1" lista
    lista="$(quants_num)"
    printf '%s' "$lista" | tr ' ' '\n' | grep -qE "=${nombre}$"
}
