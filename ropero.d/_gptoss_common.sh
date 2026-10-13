#!/usr/bin/env bash

MODEL_ENGINE="llama-server"

GPTOSS_COMMON_GGUF="openai_gpt-oss-20b-MXFP4.gguf"
GPTOSS_COMMON_GGUF="${ROPERO_GPTOSS_GGUF:-$GPTOSS_COMMON_GGUF}"

MODEL_GGUF="$GPTOSS_COMMON_GGUF"
MODEL_CTX="${ROPERO_GPTOSS_HIGH_CTX:-131072}"

# -----------------------------------------------------------------------------
# gptoss_spec_args <low|medium|high>
#
# Imprime, UN ARGUMENTO POR LÍNEA, los flags de especulación por n-gramas de
# ese nivel de razonamiento. Un argumento por línea porque es el mismo contrato
# que get_model_args_*: el launcher lo lee con mapfile y lo ejecuta sin
# word-splitting.
#
# ── POR QUÉ ESTÁ SUELTA Y NO DENTRO DE get_model_args_gptoss_common ───────────
# Porque hay CUATRO modelos gpt-oss, no tres: los tres de abajo (high/medium/low,
# todos con el mismo MXFP4) y gptossfallback.sh (que usa otro GGUF, UD-Q8_K_XL,
# y es un seguro frío). Los cuatro tienen que llevar la MISMA tabla de
# n-gramas por nivel, porque son el mismo modelo con distinta profundidad de
# razonamiento.
#
# Con la tabla dentro de get_model_args_gptoss_common, el fallback tenía que
# copiársela. Y una tabla copiada no se actualiza: se tocaba `high` aquí, el
# fallback seguía con los valores viejos, y los dos se comportaban distinto
# sin que nada lo dijera. Misma familia de modelo, distinta calibración, sin
# aviso. Es la razón de que esto sea una función aparte y no un bloque.
# -----------------------------------------------------------------------------
gptoss_spec_args() {
    local level="${1:-medium}"
    case "$level" in
        low)
            # Con razonamiento bajo (CoT corto), atajos rápidos en conectores lógicos
            printf '%s\n' \
                --spec-type ngram-simple \
                --spec-ngram-simple-size-n 6 \
                --spec-ngram-simple-size-m 16 \
                --spec-draft-n-min 1 \
                --spec-draft-n-max 2
            ;;
        medium)
            # Balanceado para chat convencional y flujo asistencial estándar
            printf '%s\n' \
                --spec-type ngram-simple \
                --spec-ngram-simple-size-n 12 \
                --spec-ngram-simple-size-m 48 \
                --spec-draft-n-min 1 \
                --spec-draft-n-max 3
            ;;
        high)
            # Razonamiento alto: Ideal para bloques extensos, código denso o estructuras lógicas repetitivas
            printf '%s\n' \
                --spec-type ngram-simple \
                --spec-ngram-simple-size-n 8 \
                --spec-ngram-simple-size-m 32 \
                --spec-draft-n-min 1 \
                --spec-draft-n-max 4
            ;;
        *)
            echo "ERROR: el nivel de razonamiento debe ser 'low' | 'medium' | 'high'; recibido '$level'" >&2
            return 1
            ;;
    esac
}

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

    # Especificación por N-Gramas según el reasoning_effort. La tabla vive en
    # gptoss_spec_args para que la compartan los tres de aquí y el fallback.
    #
    # El `|| return 1` va sobre una COMMAND SUBSTITUTION a propósito: ahí sí se
    # propaga el exit code. Con `mapfile -t < <(gptoss_spec_args ...)` NO se
    # propagaría, porque la sustitución de proceso corre en un subshell y su
    # estado se pierde — el nivel inválido pasaría y el args saldría sin
    # especificación, sin decir nada.
    local spec_out
    spec_out="$(gptoss_spec_args "$level")" || return 1
    local -a spec_args
    mapfile -t spec_args <<<"$spec_out"
    args+=("${spec_args[@]}")

    printf '%s\n' "${args[@]}"
}
