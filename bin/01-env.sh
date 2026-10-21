#!/usr/bin/env bash
# =============================================================================
# 01-env.sh — entorno propio de ropero (spec migración)
# Setup se queda lo suyo (BIN_DIR, MODEL_DIR...); ropero se lleva lo suyo.
# Seguro de sourcear (solo defaults, cero ejecución).
# =============================================================================

: "${GGUF_DIR:=$HOME/models/gguf}"
: "${SAFETENSORS_DIR:=$HOME/models/safetensors}"
: "${LOG_DIR:=$HOME/.logs/ropero}"
: "${LLAMA_DIR:=$HOME/llama.cpp}"
: "${BIN_DIR:=$HOME/.local/bin}"

ropero_env_setup() {
    mkdir -p "$GGUF_DIR" "$SAFETENSORS_DIR" "$LOG_DIR" "$LLAMA_DIR" 2>/dev/null || true
}
