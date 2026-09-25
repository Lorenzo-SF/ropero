#!/usr/bin/env bash
# ==============================================================================
# 🔥 FIRED — modelo fuera del scope base (airllm / mlx_lm).
# Movido de bin/ropero.d/fired/ a bin/ropero.d/ (2026-09-10).
# Repo original: openai/gpt-oss-120b (vía AirLLM)
# Path local: $SAFETENSORS_DIR/gpt-oss-120b
# ==============================================================================

# =============================================================================
# Modelo: airgptoss (AirLLM GPT-OSS 120B)
#
#   Este modelo está basado en el safetensors gpt-oss-120b y se ejecuta con AirLLM.
#   El script configura la ruta al modelo (relativa a SAFETENSORS_DIR) y el tamaño
#   de contexto (ajustable con ROPERO_AIRGPTOSS_CTX si lo deseas).
#
#   La variable MODEL_ENGINE debe ser "airllm" para que ropero use el flujo
#   específico de AirLLM en lugar del de llama-server.
#
#   ⚠️ RENDIMIENTO EN macOS:
#     - En Apple Silicon, AirLLM puede usar MPS (Metal Performance Shaders) automáticamente.
#     - Un modelo de 120B requiere mínimo 30-40 GB de RAM (cuantizado 4-bit) o más.
#     - Asegúrate de que $SAFETENSORS_DIR/gpt-oss-120b exista antes de arrancar.
#     - Recomendado: usar --ctx-size ajustado a tu memoria disponible.
# =============================================================================

MODEL_ENGINE="airllm"
MODEL_PATH="gpt-oss-120b"
MODEL_CTX=131072