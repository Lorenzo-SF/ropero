#!/usr/bin/env bash
# ==============================================================================
# 🔥 FIRED — modelo fuera del scope base (qwen / fable / gptoss_*).
# Movido a bin/ropero.d/fired/ tras decisión del usuario (2026-09-10).
# Recuperable: mv fired/qwenvision.sh ./ + descargar GGUF + mmproj (ver abajo).
# ==============================================================================
# --- BLOQUES DE DESCARGA (antes en 21-download-models.sh, ahora comentados) ---
# ropero_download_gguf "unsloth/Qwen2.5-VL-7B-Instruct-GGUF" \
#     "Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf" "Qwen2.5-VL 7B UD-Q4" "4.8 GB"
# ropero_download_gguf "ggml-org/Qwen2.5-VL-7B-Instruct-GGUF" \
#     "mmproj-Qwen2.5-VL-7B-Instruct-Q8_0.gguf" "Qwen2.5-VL 7B mmproj Q8" "0.8 GB"

# =============================================================================
# Modelo: qwenvision — Qwen2.5-VL-7B-Instruct (UD-Q4_K_XL, ~4.5 GB)
# =============================================================================
# Visión para ttstt (analizar espectrogramas) y análisis general de imagen.
# Soporte en este build: handlers QWEN25VL en tools/mtmd/models/ ✓ (verificado
# en producción). Pesos: unsloth/Qwen2.5-VL-7B-Instruct-GGUF.
#
# MMPROJ: `mmproj-Qwen2.5-VL-7B-Instruct-Q8_0.gguf` de ggml-org
# (verificado en producción con este texto; el F16 de unsloth choca de
# nombre con los de gemma/nemo, se evita).
#
# NOTA histórica: Llama 3.2 Vision murió aquí ("unknown model architecture:
# 'mllama'"); Qwen2.5-VL es su sustituto verificado.
# =============================================================================
MODEL_ENGINE="llama-server"
QWENVISION_GGUF="Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf"
QWENVISION_MMPROJ="mmproj-Qwen2.5-VL-7B-Instruct-Q8_0.gguf"

MODEL_GGUF="$QWENVISION_GGUF"
MODEL_CTX="${ROPERO_QWENVISION_CTX:-32768}"

get_model_args_qwenvision() {
    local ngl="${ROPERO_QWENVISION_NGL:-999}"
    local temp="${ROPERO_QWENVISION_TEMP:-0.2}"  # Baja temperatura para análisis preciso
    local top_p="${ROPERO_QWENVISION_TOP_P:-0.9}"
    local top_k="${ROPERO_QWENVISION_TOP_K:-20}"
    local repeat="${ROPERO_QWENVISION_REPEAT:-1.05}"

    local -a args=(
        --n-gpu-layers "$ngl"
        --threads 12
        --threads-batch 24
        --temp "$temp"
        --top-p "$top_p"
        --top-k "$top_k"
        --repeat-penalty "$repeat"
        --cache-type-k q8_0
        --cache-type-v q8_0
        --mmproj "$GGUF_DIR/$QWENVISION_MMPROJ"
        --n-predict 4096
        --slot-prompt-similarity 0.8
        --keep 8192
        --jinja
        --load-mode none
    )

    printf '%s\n' "${args[@]}"
}
