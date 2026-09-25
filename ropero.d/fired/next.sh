#!/usr/bin/env bash
# ==============================================================================
# 🔥 FIRED — modelo fuera del scope base (qwen / fable / gptoss_*).
# Movido a bin/ropero.d/fired/ tras decisión del usuario (2026-09-10).
# Recuperable: mv fired/next.sh ./ && descargar GGUF (ver bloque abajo).
# ==============================================================================
# --- BLOQUE DE DESCARGA (antes en 21-download-models.sh, ahora comentado) ---
# ropero_download_gguf "unsloth/Qwen3-Coder-Next-GGUF" \
#     "Qwen3-Coder-Next-UD-Q3_K_S.gguf" "Qwen Coder Next" "33.3 GB"

# ==============================================================================
# Qwen3-Coder-Next — Config calibrada (16GB VRAM MUY ajustado)
# ==============================================================================
# Modelo: Qwen/Qwen3-Coder-Next (cuantizado unsloth/Qwen3-Coder-Next-GGUF)
#   - 80B MoE / 3B activo (gigante: 512 expertos, 10 activos, 1 shared)
#   - 48 layers, hybrid Gated DeltaNet + Gated Attention + MoE
#   - Hidden 2048
#   - Context 256K nativo
#   - **NO thinking mode** (modelo no genera <think> blocks, especificación repo)
#   - Sampling canónico Qwen3: temp=1.0, top_p=0.95, top_k=40
#   - Hardware recomendado: >45GB unified memory (nosotros 16GB VRAM + 30GB RAM = 46GB JUSTO)
# ==============================================================================
# Peculiaridades críticas:
#   1. 80B MoE en 16GB VRAM: NECESARIO offload CPU masivo de expertos
#   2. Q3_K_S quant (33 GB en disco): ni entero cabe en RAM unificada
#   3. Context 256K nativo pero en 16GB VRAM solo llegamos a 65K con offload
#   4. NO usar enable_thinking (modelo no soporta; lo ignora)
# ==============================================================================

MODEL_ENGINE="llama-server"
MODEL_GGUF="Qwen3-Coder-Next-UD-Q3_K_S.gguf"
MODEL_GGUF="$MODEL_GGUF"
MODEL_CTX="${ROPERO_NEXT_CTX:-65536}"                 # 65k en 16GB VRAM (256K nativo necesita >45GB unified)

N_CPU_MOE="${ROPERO_NEXT_N_CPU_MOE:-44}"              # 44 de 48 layers con expertos en CPU (16GB estricto)
                                                       # 80B/3B activo = solo 3B activo pero 80B de params totales

get_model_args_next() {
    local -a args=(
        # ---- GPU offload (80B MoE, casi todo a CPU) ----
        --n-gpu-layers -1                              # todos los layers GPU-eligible
        --n-cpu-moe "$N_CPU_MOE"                       # expertos MoE en CPU
        --no-kv-offload                                # KV cache en GPU

        # ---- Atención ----
        # -fa on lo pone ropero entry globalmente
        # Gated DeltaNet layers: comportamiento SSM-like (llama.cpp lo gestiona)

        # ---- KV cache (mínimo para liberar VRAM) ----
        --cache-type-k q4_0
        --cache-type-v q4_0
        --cache-prompt
        --context-shift
        --kv-unified                                   # single user, unified buffer

        # ---- Modelo / carga ----
        --jinja                                        # chat template Qwen3 Next
        --load-mode mmap                               # mmap ON (33GB no cabe en RAM upfront, usa memoria virtual)

        # ---- Sampling (canónico Qwen3 instruct) ----
        --temp 1.0
        --top-p 0.95
        --top-k 40
        --min-p 0.0
        --repeat-penalty 1.0
        --repeat-last-n 64

        # ---- Batching / threads (reducido por VRAM) ----
        --batch-size 2048
        --ubatch-size 512
        --parallel 1

        # ---- Generación ----
        --n-predict 4096
        --keep 1024

        # ---- Misc ----
        # --log-verbosity lo pone ropero entry (default 4)
    )
    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# ==============================================================================
# 1. OOM en GPU con ctx 65536:
#      → Bajar ROPERO_NEXT_CTX a 32768
#      → Subir ROPERO_NEXT_N_CPU_MOE a 48 (todos los expertos)
#
# 2. Si quieres más velocidad a costa de calidad:
#      → No es realista con 80B MoE en 16GB. Mejor usar fable (35B MoE).
#
# 3. Si ctx largo es crítico:
#      → --kv-unified está activado, --cache-reuse 256 ayuda
#      → Pero la VRAM es el limitante real
#
# 4. **NO usar enable_thinking** (modelo no soporta, spec repo)
# ==============================================================================