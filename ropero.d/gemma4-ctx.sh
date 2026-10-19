#!/usr/bin/env bash
# Fuente: https://unsloth.ai/docs/models/gemma-4/qat (requiere llama.cpp reciente).
# Datos Metal con prompt corto; no garantía de 16 GiB CUDA ni contexto lleno.

# Variables públicas leídas por el loader al sourcear.
# shellcheck disable=SC2034
MODEL_ENGINE="llama-server"
# shellcheck disable=SC2034
MODEL_GGUF="gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf"
# shellcheck disable=SC2034
MODEL_CTX=262144

get_model_args_gemma4_ctx() {
    local -a args=(
        --n-gpu-layers all --parallel 1 --fit off
        --cache-type-k q4_0 --cache-type-v q4_0
        --batch-size 512 --ubatch-size 512
        --n-cpu-moe 5
        --model-draft "$GGUF_DIR/mtp-gemma-4-26B-A4B-it.gguf"
        --spec-type draft-mtp --spec-draft-n-max 2 --spec-draft-ngl all
        --mmproj "$GGUF_DIR/gemma-4-26B-A4B-it-mmproj-BF16.gguf"
        --no-mmproj-offload --jinja
        --temp 1.0 --top-p 0.95 --top-k 64
    )
    printf '%s\n' "${args[@]}"
}
