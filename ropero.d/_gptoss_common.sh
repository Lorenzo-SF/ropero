#!/usr/bin/env bash

MODEL_ENGINE="llama-server"

GPTOSS_COMMON_GGUF="openai_gpt-oss-20b-MXFP4.gguf"
GPTOSS_COMMON_GGUF="${ROPERO_GPTOSS_GGUF:-$GPTOSS_COMMON_GGUF}"

MODEL_GGUF="$GPTOSS_COMMON_GGUF"
MODEL_CTX="${ROPERO_GPTOSS_HIGH_CTX:-131072}"

get_model_args_gptoss_common() {
    local level="${1:-medium}"  # high | medium | low

    local spec="${ROPERO_GPTOSS_SPEC:-on}"

    local -a args=(
        --n-gpu-layers 99
        --temp 0.8
        --top-k 40
        --min-p 0.05
        --repeat-penalty 1.1
        --threads 12
        --threads-batch 24
        --reasoning-format auto
        --chat-template-kwargs "{\"reasoning_effort\":\"$level\"}"
        --batch-size 4096
        --ubatch-size 1024
        --n-predict 16384
        --slot-prompt-similarity 0.2
        --keep 8192
        --cache-type-k "q8_0"   
        --cache-type-v "q8_0"   
        --jinja
        --kv-unified
        --load-mode auto
    )

    # Condición para acoplar la especulación por N-Gramas según el reasoning_effort
    case "$level" in
        low)
            # Con razonamiento bajo (CoT corto), atajos rápidos en conectores lógicos
            args+=(
                --spec-type ngram-simple
                --spec-ngram-simple-size-n 6
                --spec-ngram-simple-size-m 16
                --spec-draft-n-min 1
                --spec-draft-n-max 2
            )
            ;;
        medium)
            # Balanceado para chat convencional y flujo asistencial estándar
            args+=(
                --spec-type ngram-simple
                --spec-ngram-simple-size-n 12
                --spec-ngram-simple-size-m 48
                --spec-draft-n-min 1
                --spec-draft-n-max 3
            )
            ;;
        high)
            # Razonamiento alto: Ideal para bloques extensos, código denso o estructuras lógicas repetitivas
            args+=(
                --spec-type ngram-simple
                --spec-ngram-simple-size-n 8
                --spec-ngram-simple-size-m 32
                --spec-draft-n-min 1
                --spec-draft-n-max 4
            )
            ;;
        *)
            echo "ERROR: El argumento de nivel en get_model_args_gptoss_common debe ser 'low' | 'medium' | 'high'" >&2
            return 1
            ;;
    esac
  

    printf '%s\n' "${args[@]}"
}
