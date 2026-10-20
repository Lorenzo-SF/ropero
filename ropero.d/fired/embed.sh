#!/usr/bin/env bash
# =============================================================================
# Modelo: embed
#
# Puerto fijo (MODELO pineado a 9990).
# Con MODEL_PORT declarado, ropero usa ESE puerto siempre, haya --cpu o no:
# así el embed puede convivir con un modelo en la GPU (:9999) y otro en CPU
# (:9998) sin pelear por los slots. Si quitas la línea, vuelve al comportamiento
# de dos slots (--cpu → :9998, si no → :9999).
# Override puntual sin editar el .sh: ROPERO_EMBED_PORT=9991 ropero embed
# =============================================================================

MODEL_ENGINE="llama-server"

MODEL_GGUF="jina-code-embeddings-1.5b-Q8_0.gguf"
MODEL_CTX=8192
MODEL_PORT=9990

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