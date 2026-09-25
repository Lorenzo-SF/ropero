#!/usr/bin/env bash
# =============================================================================
# Modelo: embed
# =============================================================================

MODEL_ENGINE="llama-server"

MODEL_GGUF="jina-code-embeddings-1.5b-Q8_0.gguf"
MODEL_CTX=8192

get_model_args_embed() {
    printf '%s\n' \
        --embedding \
        --pooling last \
        --embd-normalize 2 \
        --batch-size 1024 \
        --ubatch-size 1024 \
        --parallel 4 \
        --jinja  
}