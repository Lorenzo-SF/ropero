#!/usr/bin/env bash
# =============================================================================
# Modelo: qwen3vector
#
# Bi-encoder del vault. Etapa 1 del retrieval: vectoriza CADA nota por separado.
# NUNCA ve la consulta — por eso es rápido y por eso puede recorrer las 10.000.
#
# Puerto fijo (:9993). Con MODEL_PORT declarado ropero usa ESE puerto siempre,
# haya --cpu o no: así la lane del vault convive con el coder (:9999) y con el
# fallback (:9996) sin pelear por los slots.
#
# POR QUÉ ESTE Y NO EL `jina-code` DE embed.sh
# ───────────────────────────────────────────────
# `jina-code-embeddings-1.5b` (embed.sh, :9990) es un modelo **de código**, y
# sigue ahí porque para buscar en código es el correcto. Este vault guarda
# prosa en castellano, y ahí jina-code no es lo que quieres: no falla, devuelve
# vectores, y el RAG simplemente es malo. Es el modo de fallo silencioso más
# caro que hay.
#
# Medido contra f16 (coseno medio / mínimo):
#     Q4_K_M  0,975 / 0,943    Q5_K_M  0,991 / 0,978    Q8_0  0,999 / 0,999
# Con 639 MB no hay motivo para bajar de Q8_0.
#
# ⚠️ LO QUE HACE ESTE MODELO Y NINGÚN OTRO
# Usa **last-token pooling** y es **instruction-aware**. La doc oficial:
#   "Without last-token pooling and the instruction prefix, retrieval quality
#    collapses."
# Dos cosas, y las dos son invisibles si no las sabes:
#   1. `--pooling last`  → va en los flags de abajo. Si se cae, no peta.
#   2. `Instruct: {tarea}\nQuery:{consulta}` → lo pone el CLIENTE, en la
#      CONSULTA, nunca en los documentos. Si no lo pone, no peta tampoco.
# Si el vault devuelve vecindarios raros, mira esto antes que el modelo.
# =============================================================================

MODEL_ENGINE="llama-server"

MODEL_GGUF="Qwen3-Embedding-0.6B-Q8_0.gguf"
MODEL_CTX="${ROPERO_VECTOR_CTX:-32768}"
MODEL_PORT=9993

# Alias público. `ropero vector` en vez de `ropero qwen3vector`.
MODEL_ALIAS="${MODEL_ALIAS:-vector}"

get_model_args_qwen3vector() {
    local -a args=(
        # ---- FORZADO A CPU, no por defecto sino a propósito ----
        # `--cpu` de ropero equivale a esto, pero aquí se pone en el script para
        # que sea verdad aunque nadie se acuerde de la flag. El orden de los
        # args en llama.cpp manda: la última aparición de un flag gana, y
        # ropero añade su `--n-gpu-layers 0` AL FINAL cuando le pasas --cpu
        # (ropero:~1732), así que lo que hay aquí no se puede sobrescribir.
        #
        # Por qué CPU y no GPU: 639 MB entrarían de sobra en los ~3 GB que
        # dejan libre el coder, y por eso mismo es tentador. Pero si lo
        # arrancas en GPU, el día que el coder ocupe 16 GB el embedder falla
        # al cargar, y el retrieval — que es lo que tiene que estar SIEMPRE
        # vivo — se cae justo cuando más lo necesitas.
        --n-gpu-layers 0

        # ---- Lo que hace que sea un embedder ----
        --embedding
        --pooling last
        --embd-normalize 2                   # coseno: comparable con lo que
                                            # devuelve el reranker

        # ---- Batching: aquí es donde se gana velocidad ----
        # No genera tokens: solo prefill. Por eso un batch enorme no
        # embaralla la respuesta y multiplica el throughput. Es la diferencia
        # entre indexar 10.000 notas en minutos y en horas.
        --batch-size 4096
        --ubatch-size 4096
        --parallel 4                         # slots concurrentes

        --jinja
    )
    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# 1. "Las búsquedas devuelven vecindarios sin sentido"
#    → ¿Estás mandando `Instruct: {tarea}` delante de la CONSULTA?
#      Y solo de la consulta. El documento va limpio. Este es el fallo #1.
# 2. "Va muy lento"
#    → Sube --batch-size/--ubatch-size. En CPU el cuello es el prefill, y el
#      batch lo amortiza. Baja MODEL_CTX: 32768 es el techo, no el suelo.
# 3. "No arranca / OOM"
#    → No debería: son 639 MB en CPU. Si pasa, casi seguro se está corriendo
#      con GPU por otra vía (ROPERO_*_GPU o una versión vieja del script).
# ==============================================================================
