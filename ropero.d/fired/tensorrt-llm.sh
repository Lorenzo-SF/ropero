#!/usr/bin/env bash
# ==============================================================================
# 🔥 FIRED — modelo fuera del scope base (qwen / fable / gptoss_*).
# Movido a bin/ropero.d/fired/ tras decisión del usuario (2026-09-10).
# Recuperable: mv fired/tensorrt-llm.sh ./ y descargar modelo TensorRT-LLM.
# (NO estaba en 21-download-models.sh — es legacy).
# Path local: $SAFETENSORS_DIR/tensorrt-llm-model
# ==============================================================================

# =============================================================================
# Modelo: tensorrt-llm (TensorRT-LLM)
#
# Configura el entorno para TensorRT-LLM. El modelo se encuentra en SAFETENSORS_DIR
# con la ruta relativa especificada en MODEL_PATH.
# =============================================================================
MODEL_ENGINE="tensorrt-llm"
MODEL_PATH="tensorrt-llm-model"  # Ajusta según tu estructura
MODEL_CTX=131072
