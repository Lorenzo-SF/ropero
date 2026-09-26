#!/usr/bin/env bash

set -euo pipefail

MODEL_ENGINE="llama-server"
DEVSTRAL_GGUF="Devstral-Small-2-24B-Instruct-2512-UD-Q4_K_XL.gguf"
MODEL_GGUF="$DEVSTRAL_GGUF"
MODEL_CTX="${ROPERO_DEVSTRAL_CTX:-131072}"
# Alias público (lo carga bin/ropero como nombre alternativo del modelo).
# Para cambiar el modelo físico asociado al alias `coder_lite`, edita este script;
# `opencode.jsonc` no se entera.
MODEL_ALIAS="${MODEL_ALIAS:-coder_lite}"

get_model_args_devstral(){
  local -a args=(
      --n-gpu-layers -1
      --no-kv-offload

      --cache-type-k q8_0
      --cache-type-v q8_0
      --cache-prompt
      --cache-reuse 512
      --kv-unified
      --context-shift

      --batch-size 4096
      --ubatch-size 1024
      --parallel 1
      --threads 12
      --threads-batch 16
      --poll 30

      --jinja
      --chat-template-kwargs '{"enable_thinking": false}'
      --reasoning-format deepseek

      --temp 0.6
      --top-k 20
      --top-p 0.95
      --min-p 0.05
      --repeat-penalty 1.0
      --repeat-last-n 64
      --seed -1

      --n-predict 4096
      --keep 1024

      --load-mode none
      --metrics
      --perf
    )
    printf '%s\n' "${args[@]}"
} 
 
