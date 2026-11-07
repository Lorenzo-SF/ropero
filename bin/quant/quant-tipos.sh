#!/usr/bin/env bash
# =============================================================================
# quant-tipos.sh — la lista de tipos de quant: numero <-> nombre
# =============================================================================
# El GGUF lleva general.file_type, que es el NUMERO del llama_ftype (el enum
# de include/llama.h: "Q4_K_M" = 15, "NVFP4" = 39). Para poner el nombre en
# el nombre del repo y en el model card hace falta traducirlo.
#
# OJO: el llama_ftype NO es el ggml_type (el de los tensores sueltos: "Q4_K"
# = 12, "NVFP4" = 40). Son dos enums distintos con numeros parecidos: mezclarlos
# hace que un GGUF en NVFP4 se llame MXFP4.
#
# La lista sale del motor instalado (llama-quantize --help), que es la
# fuente de verdad. Pero si el motor no esta en el PATH, se usa la lista de
# respaldo de abajo: son los tipos del enum llama_ftype de llama.cpp
# (include/llama.h), con sus numeros exactos.
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

# ── la lista SIN los alias ─────────────────────────────────────────────────
# El motor imprime "15 or Q4_K : alias for Q4_K_M" y despues "15 or Q4_K_M":
# los dos son el 15, pero el nombre bueno es el segundo. Como el nombre se
# busca con el PRIMERO que aparece, hay que quitar los alias de la lista
# (se distinguen porque su descripcion es "alias for ...").
quants_canon() {
    local qbin lista
    if qbin="$(_quantize_bin)"; then
        lista="$("$qbin" --help 2>&1 | grep -v "alias for" | sed -nE 's/^[[:space:]]*([0-9]+)[[:space:]]+or[[:space:]]+([A-Z0-9_]+).*/\1=\2/p' | tr '\n' ' ')"
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
    lista="$(quants_canon)"
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
