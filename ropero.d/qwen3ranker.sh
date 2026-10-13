#!/usr/bin/env bash
# =============================================================================
# Modelo: qwen3ranker
#
# Cross-encoder del vault. Etapa 2 del retrieval: lee (consulta, nota) JUNTAS
# y devuelve un score. Lento a propósito, y por eso NUNCA recorre las 10.000
# notas — solo las ~50 que el embedder devolvió.
#
# 10.000 notas --[vector, bi-encoder]--> 50 --[ranker, cross-encoder]--> 8 --[LLM]--> respuesta
#
# POR QUÉ NO ES OPCIONAL
# ──────────────────────
# El embeder y el ranker hacen cosas DISTINTAS, no el mismo trabajo con distinto
# flag:
#
#   embeder  bi-encoder      ve cada texto SOLO          → vector   (barato)
#   ranker   cross-encoder   ve consulta Y documento     → score    (caro)
#
# La doc de BGE lo dice literalmente: "As cross-encoder models, re-ranker
# demonstrates higher accuracy than bi-encoder embedding model." Sin esta etapa
# el vault te devuelve ruido con toda la confianza del mundo.
#
# MTEB-R, mismo protocolo:
#     Qwen3-Reranker-0.6B   65,80      ← este
#     BGE-reranker-v2-m3    57,03
#
# No se puede sustituir por un LLM generativo: la cabecera de cross-encoder se
# entrena sobre pares, no sobre títulos. Un embedder NO puede hacer este papel.
#
# Puerto fijo (:9994).
# =============================================================================

MODEL_ENGINE="llama-server"

MODEL_GGUF="qwen3-reranker-0.6b-q8_0.gguf"
MODEL_CTX="${ROPERO_RANKER_CTX:-8192}"
MODEL_PORT=9994

# Alias público. `ropero ranker` en vez de `ropero qwen3ranker`.
MODEL_ALIAS="${MODEL_ALIAS:-ranker}"

get_model_args_qwen3ranker() {
    local -a args=(
        # ---- FORZADO A CPU ----
        # Mismo motivo que el embedder: son 639 MB y podrían ir a la GPU, pero
        # si el coder satura la VRAM el retrieval se cae entero. El retrieval
        # tiene que estar siempre vivo; el coder no.
        --n-gpu-layers 0

        # ---- Lo que hace que sea un reranker y no un embedder ----
        --reranking

        # ---- Batching ----
        # Aquí SÍ genera (un score por par), pero son scores de 1 token. Batch
        # alto para no pagar el coste fijo por cada uno de los 50 pares.
        --batch-size 2048
        --ubatch-size 2048
        --parallel 4

        --jinja
    )
    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# 1. "Los scores salen todos iguales / casi todos a cero"
#    → No arrancó con --reranking. Sin ese flag, llama-server lo trata como un
#      embedder más y te devuelve vectores donde esperabas scores. No peta.
# 2. "Se come notas largas"
#    → MODEL_CTX es el techo de (consulta + nota). 8192 va bien para notas de
#      vault. Si tienes notas de 20k, súbelo a 16382 — y recuerda que el coste
#      es lineal en el par, no en el corpus.
# 3. "Va lento"
#    → Es lo esperado, y es la razón de que solo vea 50 pares. Si va lento
#      CON 50 pares, el problema es que le estás pasando las 10.000.
# ==============================================================================
