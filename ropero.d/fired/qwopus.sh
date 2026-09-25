#!/usr/bin/env bash
# ==============================================================================
# 🔥 FIRED — modelo fuera del scope base (qwen / fable / gptoss_*).
# Movido a bin/ropero.d/fired/ tras decisión del usuario (2026-09-10).
# Recuperable: mv fired/qwopus.sh ./ && descargar GGUF (ver bloque abajo).
# ==============================================================================
# --- BLOQUE DE DESCARGA (antes en 21-download-models.sh, ahora comentado) ---
# ropero_download_gguf "Jackrong/Qwopus3.8-27B-Flash-GGUF" \
#     "Qwopus3.8-27B-Flash-MTP-Q4_K_M.gguf" "Qwopus3.8 MTP Q4" "16.8 GB"

# ==============================================================================
# Qwopus3.8-27B-Flash — Config calibrada (RTX 5080 16GB VRAM, 30GB RAM)
# ==============================================================================
# Modelo: Jackrong/Qwopus3.8-27B-Flash (fine-tune de Qwen/Qwen3.8-27B)
#   - DENSE 27B (no MoE)
#   - Fine-tune optimizado para agentes (Flash): menos reasoning, más throughput
#   - Thinking debe estar DESACTIVADO (es la especialidad Flash)
#   - Speculative decoding: MTP heads del propio modelo (--spec-type draft-mtp)
#   - q8_0 KV cache (recomendado repo)
#   - Benchmark interno repo: 13/14 agentic tasks, MTP acceptance 80.7%
#   - Known issue: indentación Python ocasional (fix en progreso)
# ==============================================================================
# Peculiaridades críticas:
#   1. Thinking ON por defecto → SI NO se desactiva, el modelo responde con
#      razonamiento verbose estilo Qwen3.8. Flash requiere thinking OFF.
#   2. MTP interno: el modelo trae heads MTP integrados; usar draft-mtp.
#   3. KV cache q8_0 (no q4_0): repo validado para tool use.
# ==============================================================================

MODEL_ENGINE="llama-server"
QWOPUS_GGUF="Qwopus3.8-27B-Flash-MTP-Q4_K_M.gguf"

MODEL_GGUF="$QWOPUS_GGUF"
MODEL_CTX="${ROPERO_QWOPUS_CTX:-131072}"

get_model_args_qwopus() {
    local -a args=(
        # ---- GPU offload (modelo denso 27B, cabe entero en 16GB) ----
        --n-gpu-layers -1                              # todos los layers a GPU
        --no-kv-offload                                # KV cache en GPU

        # ---- Atención ----
        # -fa on lo pone ropero entry globalmente

        # ---- KV cache (q8_0 recomendado repo) ----
        --cache-type-k q8_0
        --cache-type-v q8_0
        --cache-prompt
        --context-shift
        --kv-unified                                   # single user, unified buffer

        # ---- Speculative decoding (MTP heads internos del modelo) ----
        --spec-type draft-mtp
        --spec-draft-n-max 5                          # 5 drafts por ronda (max throughput)
        --spec-draft-n-min 1                          # no malgastar en rounds de 0 drafts
        --spec-draft-backend-sampling

        # ---- Modelo / carga ----
        --jinja                                        # chat template Qwen3.8 (jinja)
        --reasoning-format deepseek                    # limpia tags <think> del output
        --chat-template-kwargs '{"enable_thinking": false}'  # THINKING OFF (Flash requirement)
        --load-mode none

        # ---- Sampling (canónico Qwen3.8 instruct, repo Meta) ----
        --temp 0.6                                     # temp baja para respuestas estables
        --top-p 0.95
        --top-k 20
        --min-p 0.0
        --repeat-penalty 1.0                           # sin penalty (canónico)
        --repeat-last-n 64

        # ---- Batching / threads ----
        --batch-size 4096
        --ubatch-size 1024
        --parallel 1

        # ---- Generación ----
        --n-predict 8192                               # agentic puede requerir outputs largos
        --keep 1024

        # ---- Misc ----
        # --log-verbosity lo pone ropero entry (default 4)
    )
    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# ==============================================================================
# 1. Si thinking sigue apareciendo verbose:
#      → Verifica que chat_template_kwargs tiene enable_thinking=false EXACTO
#      → Algunos forks de Qwen usan enable_reasoning o reasoning_enabled
#
# 2. Si MTP no carga (error al startup):
#      → Quita --spec-type draft-mtp. Sin spec sigue funcionando, más lento.
#
# 3. Si quieres más velocidad:
#      → --spec-draft-n-max 7 (más drafts por ronda)
#      → --cache-type-v q4_0 (ojo: tool use puede degradarse)
#
# 4. Si problemas de indentación Python:
#      → Es un known issue del repo, fix en progreso. No es bug de tu config.
#      → Workaround: usar temperatura más baja (0.3) para reducir variabilidad
# ==============================================================================