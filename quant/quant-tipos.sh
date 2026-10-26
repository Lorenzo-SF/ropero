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

# La lista de respaldo vive en tipos.txt, al lado de este script: la leen
# tambien el python (model-card.py), y duplicarla en dos sitios es la forma
# segura de que se queden desincronizadas.
QUANTS_RESPALDO="$(sed -nE 's/^([0-9]+)[[:space:]]+([A-Z0-9_]+)$/\1=\2/p' \
    "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tipos.txt" | tr '\n' ' ')"

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
