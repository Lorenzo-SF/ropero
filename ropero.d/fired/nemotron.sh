#!/usr/bin/env bash
# ==============================================================================
# 🔥 FIRED — modelo fuera del scope base (qwen / fable / gptoss_*).
# Movido a bin/ropero.d/fired/ tras decisión del usuario (2026-09-10).
# Recuperable: mv fired/nemotron.sh ./ && descargar GGUF (ver bloque abajo).
# ==============================================================================
# --- BLOQUE DE DESCARGA (antes en 21-download-models.sh, ahora comentado) ---
# ropero_download_gguf "unsloth/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-GGUF" \
#     "NVIDIA-Nemotron-3.5-Lightning-30B-A3B-MXFP4_MOE.gguf" "Nemotron 3" "23.3 GB"

# ==============================================================================
# NVIDIA-Nemotron-3.5-Lightning-30B-A3B — Config calibrada
# ==============================================================================
# Modelo: nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B (MXFP4_MOE GGUF)
#   - MoE 30B / 3B activo
#   - HYBRID: Mamba-2 SSM + MoE + Attention layers (no SWA, es SSM)
#   - Context nativo: 1M tokens (single H100: 256K). En 16GB VRAM: 131K max
#   - MTP heads integrados (draft-mtp)
#   - Reasoning ON por defecto (controlable via enable_thinking)
#   - Sampling canónico NVIDIA: temp=1.0, top_p=0.95
#   - vLLM usa --mamba-ssm-cache-dtype float16 (no aplica a llama.cpp directo)
# ==============================================================================
# Peculiaridades críticas:
#   1. Mamba SSM layers: NO son SWA, son state-space models
#      → en llama.cpp deben funcionar con -fa on (flash attn) en attention
#      → Mamba layers usan cache SSM separado (llama.cpp lo gestiona)
#   2. Reasoning ON por defecto: enable_thinking=true (modelo más verboso)
#   3. tool-call-parser: qwen3_coder (en llama.cpp puede que requiera config)
# ==============================================================================

MODEL_ENGINE="llama-server"
NEMOTRON_GGUF="NVIDIA-Nemotron-3.5-Lightning-30B-A3B-MXFP4_MOE.gguf"

MODEL_GGUF="$NEMOTRON_GGUF"
MODEL_CTX="${ROPERO_NEMOTRON_CTX:-131072}"

N_CPU_MOE="${ROPERO_NEMOTRON_N_CPU_MOE:-35}"     # 35 layers con expertos MoE en CPU (16GB VRAM estricto)
                                                   # bajar de 20 (default original) por OOM. Subir si sobra VRAM.

get_model_args_nemotron() {
    local -a args=(
        # ---- GPU offload (MoE 30B/3B activo, 16GB VRAM justos) ----
        --n-gpu-layers -1                              # todos los layers GPU-eligible
        --n-cpu-moe "$N_CPU_MOE"                       # expertos MoE en CPU
        --no-kv-offload                                # KV cache en GPU

        # ---- Atención ----
        # -fa on lo pone ropero entry globalmente
        # NO --swa-full (Nemotron usa Mamba SSM, no SWA)

        # ---- KV cache ----
        --cache-type-k q4_0                            # agresivo (mamba SSM no usa esto, attn sí)
        --cache-type-v q4_0
        --cache-prompt
        --context-shift
        --kv-unified                                   # single user, unified buffer

        # ---- Speculative decoding (MTP heads internos) ----
        --spec-type draft-mtp
        --spec-draft-n-max 3
        --spec-draft-n-min 0
        --spec-draft-backend-sampling

        # ---- Modelo / carga ----
        --jinja                                        # chat template NVIDIA Nemotron
        --reasoning-format deepseek                    # limpia tags <think> del output
        --chat-template-kwargs '{"enable_thinking": false}'  # OFF por defecto (ON devora tokens en prompts cortos)
        --load-mode none

        # ---- Sampling (canónico NVIDIA) ----
        --temp 1.0
        --top-p 0.95
        --top-k 0                                      # desactivado (solo top-p)
        --min-p 0.0
        --repeat-penalty 1.0                           # sin penalty
        --repeat-last-n 64

        # ---- Batching / threads ----
        --batch-size 2048                              # bajado de 4096 (libera compute buffers)
        --ubatch-size 512                              # bajado de 1024
        --parallel 1

        # ---- Generación ----
        --n-predict 8192                               # reasoning puede requerir más
        --keep 1024

        # ---- Misc ----
        # --log-verbosity lo pone ropero entry (default 4)
    )
    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# ==============================================================================
# 1. Si falla por "missing tensor blk.X.ssm_conv1d.weight":
#      → el modelo requiere llama.cpp reciente que soporte Mamba-2 layers
#      → nuestro build 10851 debería OK
#
# 2. Si quieres reasoning OFF (más rápido):
#      → --chat-template-kwargs '{"enable_thinking": false}'
#
# 3. Si OOM en GPU con --n-cpu-moe 20:
#      → Sube a 30. Con 16GB y 30B MoE es ajustadísimo.
# ==============================================================================