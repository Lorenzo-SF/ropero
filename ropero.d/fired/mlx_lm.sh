#!/usr/bin/env bash
# ==============================================================================
# 🔥 FIRED — modelo fuera del scope base (qwen / fable / gptoss_*).
# Movido a bin/ropero.d/fired/ tras decisión del usuario (2026-09-10).
# Recuperable: mv fired/mlx_lm.sh ./ y descargar safetensors.
# (NO estaba en 21-download-models.sh — es legacy MLX).
# Path local: $SAFETENSORS_DIR/mlx-llama-7b
# ==============================================================================

# =============================================================================
# Modelo: mlx-lm (MLX LLM)
#
# Configura el entorno para MLX LLM. El modelo se encuentra en SAFETENSORS_DIR
# con la ruta relativa especificada en MODEL_PATH.
# =============================================================================
MODEL_ENGINE="mlx_lm"
MODEL_PATH="mlx-llama-7b"  # Ajusta según tu estructura
MODEL_CTX=131072
MODEL_GGUF="peo"


get_model_args_mlx_lm(){
    echo ""
}