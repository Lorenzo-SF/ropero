#!/usr/bin/env bash

# ropero_download_gguf "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF" \
#     "Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf" "Qwen3 Coder" "17.7 GB"

# ==============================================================================
# Qwen3-Coder-30B-A3B-Instruct — Config calibrada
# ==============================================================================
# Modelo: Qwen/Qwen3-Coder-30B-A3B-Instruct (UD-Q4_K_XL GGUF)
#   - MoE 30.5B / 3.3B activo (128 expertos, 8 activados)
#   - 48 layers, GQA 32Q / 4KV
#   - Hidden (no especificado en card, inferido de Qwen3)
#   - Vocab Qwen3 (~150K)
#   - Contexto 262,144 nativo
#   - **Solo non-thinking mode** (spec repo: NO genera <think> blocks)
#     → NO usar enable_thinking (modelo no lo soporta, lo ignora)
#   - Sampling canónico Qwen3: temp=0.7, top_p=0.8, top_k=20, repetition_penalty=1.05
#   - Output recomendado: 65,536 tokens
# ==============================================================================
# Peculiaridades críticas:
#   1. MoE 30B en 16GB VRAM: requiere --n-cpu-moe (no cabe entero)
#   2. Modelo denso de atención, NO hybrid Gated DeltaNet como Qwen3-Coder-Next
#   3. No usa MTP ni speculative decoding (GGUF básico)
# ==============================================================================

MODEL_ENGINE="llama-server"
MODEL_GGUF="Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf"
MODEL_GGUF="$MODEL_GGUF"
MODEL_CTX="${ROPERO_CODER_CTX:-131072}"

N_CPU_MOE="${ROPERO_CODER_N_CPU_MOE:-30}"             # 30 layers con expertos MoE en CPU (16GB VRAM estricto)
# Alias público (lo carga bin/ropero como nombre alternativo del modelo).
# Para cambiar el modelo físico asociado al alias `coder`, edita este script;
# `opencode.jsonc` no se entera.
MODEL_ALIAS="${MODEL_ALIAS:-coder}"

get_model_args_qwencoder() {
    local -a args=(
        # ---- GPU offload (MoE 30B/3B activo) ----
        --n-gpu-layers -1                              # todos los layers GPU-eligible
        --n-cpu-moe "$N_CPU_MOE"                       # expertos MoE en CPU
        --no-kv-offload                                # KV cache en GPU

        # ---- Atención ----
        # -fa on lo pone ropero entry globalmente

        # ---- KV cache ----
        --cache-type-k q8_0                            # q8_0 recomendado para tool use
        --cache-type-v q8_0
        --cache-prompt
        --context-shift
        --kv-unified                                   # single user, unified buffer

        # ---- Modelo / carga ----
        --jinja                                        # chat template Qwen3 coder
        --reasoning-format deepseek                    # limpia tags <think> (no genera, pero por si)
        --load-mode none

        # ---- Sampling (canónico Qwen3 coder) ----
        --temp 0.7
        --top-p 0.8
        --top-k 20
        --min-p 0.0
        --repeat-penalty 1.05
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
# 1. Si OOM en GPU:
#      → Subir ROPERO_CODER_N_CPU_MOE a 40
#      → O bajar ROPERO_CODER_CTX a 65536
#
# 2. Si quieres más velocidad:
#      → Bajar n-cpu-moe a 20 (más expertos en GPU)
#
# 3. **NO usar --chat-template-kwargs enable_thinking**: el modelo no soporta
#      thinking mode (spec repo). Generar reasoning sin querer sería bug upstream.
# ==============================================================================