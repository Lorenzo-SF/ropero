#!/usr/bin/env bash
# =============================================================================
# Modelo: gptoss  (gpt-oss-20b) — UN modelo, el nivel de razonamiento va por
#             argumento.
#
#   ropero gptoss              → medium (el default)
#   ropero gptoss --high       → razonamiento alto
#   ropero gptoss --medium     → razonamiento medio
#   ropero gptoss --low        → razonamiento bajo
#
# Los alias públicos siguen funcionando, cada uno con su nivel:
#
#   ropero verifier            → gptoss --high    (era gptoss_high.sh)
#   ropero designer            → gptoss --medium  (era gptoss_medium.sh)
#
# POR QUÉ UN SOLO SCRIPT Y NO TRES
# ─────────────────────────────────
# Antes eran tres scripts (gptoss_high/medium/low) más un cuarto
# (gptossfallback) que NO era un cuarto nivel sino otro GGUF: el mismo modelo
# con distinta profundidad de razonamiento, repartido en tres ficheros que se
# podían desincronizar sin que nada lo dijera. El síntoma era siempre el
# mismo: se tocaba la tabla de n-gramas en un sitio y el otro modelo seguía
# con los valores viejos.
#
# La tabla de n-gramas por nivel es ahora UNA función, y el nivel es un
# argumento. Cuatro formas de pedir lo mismo, un solo sitio donde está la
# calibración.
#
# ── EL NIVEL ES DEL MODELO, NO DEL CLI ─────────────────────────────────────
# El CLI no sabe qué es "high". Sabe que hay una lista de niveles, y la lista
# la declara el .sh del modelo (MODEL_LEVELS). Por eso `ropero gptoss --high`
# funciona y `ropero embed --high` da error: embed no tiene niveles. Al revés
# —un CLI con un --high global que cada modelo traduce— obliga a inventar un
# vocabulario que luego cada modelo contradice.
#
# El nivel viaja al script por ROPERO_MODEL_LEVEL. Un alias puede traer su
# nivel por defecto (verifier → high), y el flag explícito gana sobre el del
# alias: `ropero verifier --low` es "el verifier pero con poco razonamiento".
#
# =============================================================================
# EL GGUF: UD-Q8_K_XL (y por qué este)
# ─────────────────────────────────────
#   MXFP4  12,1 GB  → lo usan antes gptoss_high/medium/low
#   Q8_K_XL 13,2 GB → ESTE
#
# Tres razones:
#
#   1. Es el que está en disco. El MXFP4 no lo has descargado, así que los
#      scripts antigos apuntaban a un fichero inexistente.
#   2. El fallback (gptossfallback.sh) ya usaba el Q8_K_XL. Con este GGUF
#      como modelo único, el "seguro frío" deja de ser un segundo modelo
#      distinto: es el mismo fichero, y tener dos scripts para el mismo
#      fichero era una duplicación que solo servía para tener dos contextos
#      que calibrar.
#   3. MXFP4 es un cuant de 4 bits con microsescalares; el Q8_K_XL es 8 bits
#      "de verdad". En un MoE de 3,6B activos (20B totales) la diferencia de
#      calidad se nota más que en un denso, y el +1,1 GB cabe en la tarjeta.
#
# ── Y EL CONTEXTO BAJA A 65536 POR ESO ─────────────────────────────────────
# gpt-oss-20b: 24 capas, 8 cabezas KV, head_dim 64.
#   KV por token = 24 × 8 × 64 × 2 (K y V) = 24.576 elementos
#   en q8_0 (1,0625 B/elem) = 26.112 B/token
#
#   pesos Q8_K_XL ....... 13,20 GB = 12,29 GiB
#   KV q8_0 @ 65536 ..... 1,71 GB  = 1,59 GiB
#   KV q8_0 @ 131072 .... 3,42 GB  = 3,19 GiB   ← con el MXFP4 (11,3 GiB) ya
#                                                   no cabía en 16 GB
#
# En la RTX 5080 Laptop (15,9 GiB útiles) el KV de 131072 con el Q8_K_XL
# dejaría el modelo sin sitio para los buffers de cómputo. Con 65536:
#   12,29 + 1,59 = 13,88 GiB de pesos + KV, y queda margen para el contexto
#   de cómputo y el contexto de CUDA.
#
# Si necesitas 128K: ROPERO_GPTOSS_CTX=131072 ROPERO_GPTOSS_KV=q4_0 (el KV en
# q4_0 son 1,7 GB y entra, pero es un trueque de calidad en la cache).
# =============================================================================

MODEL_ENGINE="llama-server"

MODEL_GGUF="${ROPERO_GPTOSS_GGUF:-gpt-oss-20b-UD-Q8_K_XL.gguf}"
MODEL_CTX="${ROPERO_GPTOSS_CTX:-65536}"

# Tipo de la KV cache. q8_0 es el default; q4_0 duplica el contexto posible
# a cambio de precisión en la cache.
GPTOSS_KV="${ROPERO_GPTOSS_KV:-q8_0}"

# Niveles que este modelo acepta por argumento. El CLI los valida.
MODEL_LEVELS="${MODEL_LEVELS:-low medium high}"
# Nivel por defecto si no se pide ninguno.
MODEL_LEVEL_DEFAULT="${MODEL_LEVEL_DEFAULT:-medium}"

# Aliases públicos, cada uno con su nivel por defecto. "verifier" y
# "designer" los pide el opencode.jsonc de gunter: no son nuestros, son
# un contrato con otro sitio.
MODEL_ALIASES="${MODEL_ALIASES:-verifier:high designer:medium}"

# -----------------------------------------------------------------------------
# COPIAS PROPIAS — por qué hacen falta
#
# MODEL_LEVELS, MODEL_LEVEL_DEFAULT, MODEL_FLAGS y MODEL_OPTIONS son ARRAYS
# asociativos del CLI (declare -A). El loader sourcea TODOS los scripts
# seguidos, así que dentro de get_model_args_* Those valores ya son los del
# ÚLTIMO script sourceado, no los de este. Es el mismo motivo por el que
# MODEL_GGUF va en array (ver la cabecera de 21-download-models.sh).
#
# Así que el script captura los suyos al sourcearse, y la función lee la
# COPIA. Si se leyera el array directamente, el nivel de gptoss sería el del
# último script cargado: o el de otro modelo, o vacío.
# -----------------------------------------------------------------------------
GPTOSS_LEVELS="$MODEL_LEVELS"
GPTOSS_LEVEL_DEFAULT="$MODEL_LEVEL_DEFAULT"

# -----------------------------------------------------------------------------
# gptoss_spec_args <low|medium|high>
#
# Imprime, UN ARGUMENTO POR LÍNEA, los flags de especulación por n-gramas de
# ese nivel de razonamiento. Un argumento por línea porque es el mismo
# contrato que get_model_args_*: el launcher lo lee con mapfile y lo ejecuta
# sin word-splitting.
#
# Los n-gramas no son "drafting" como el MTP: el modelo propone continuaciones
# de texto que ya ha visto y el objetivo las acepta o las rechaza. Cuanto más
# largo es el CoT, más oportunidades hay de que el texto se repita (fórmulas, imports,
# bloques repetidos) — y ahí es donde el n-gram pega. Por eso el nivel alto
# usa un n más corto y ventana más larga: en código denso se repite mucho,
# pero en trozos cortos.
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
            # Razonamiento alto: bloques extensos, código denso o estructuras
            # lógicas repetitivas.
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

get_model_args_gptoss() {
    # El nivel llega por ROPERO_MODEL_LEVEL (lo pone el CLI con --high/--low o
    # el default del alias). Si no viene, el default del script.
    local level="${ROPERO_MODEL_LEVEL:-${GPTOSS_LEVEL_DEFAULT:-medium}}"

    # Validación del nivel contra la lista que declara este modelo. Se repite
    # aquí (y no solo en el CLI) porque el nivel también puede venir del
    # entorno, y un ROPERO_MODEL_LEVEL=typo tiene que fallar con un mensaje
    # claro en vez de arrancar el modelo con la calibración de otro nivel.
    case " $GPTOSS_LEVELS " in
        *" $level "*) ;;
        *)
            echo "ERROR: nivel de razonamiento inválido: '$level' (válidos: $GPTOSS_LEVELS)" >&2
            return 1
            ;;
    esac

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
        --cache-type-k "$GPTOSS_KV"
        --cache-type-v "$GPTOSS_KV"
        --jinja
        --kv-unified
        --load-mode auto
    )

    # Especificación por N-Gramas según el reasoning_effort.
    #
    # El `|| return 1` va sobre una COMMAND SUBSTITUTION a propósito: ahí sí se
    # propaga el exit code. Con `mapfile -t < <(gptoss_spec_args ...)` NO se
    # propagaría, porque la sustitución de proceso corre en un subshell y su
    # estado se pierde — el nivel inválido pasaría y los args saldrían sin
    # especificación, sin decir nada.
    local spec_out
    spec_out="$(gptoss_spec_args "$level")" || return 1
    local -a spec_args
    mapfile -t spec_args <<<"$spec_out"
    args+=("${spec_args[@]}")

    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# 1. "ropero gptoss --high dice opción desconocida"
#    → El modelo no declara ese nivel. Los niveles de gptoss son low/medium/
#      high; el CLI solo acepta los que el .sh declara en MODEL_LEVELS.
# 2. "Arranca con el nivel de razonamiento por defecto en vez del mío"
#    → ROPERO_MODEL_LEVEL=typo: el nivel se valida contra MODEL_LEVELS y el
#      arranque falla con el ERROR en vez de caer al default en silencio.
# 3. "Se queda sin memoria al arrancar"
#    → Bajar ROPERO_GPTOSS_CTX a 32768, o ROPERO_GPTOSS_KV=q4_0 para
#      duplicar el contexto posible. Y ojo: en la tarjeta de 16 GB el
#      Q8_K_XL con ctx 131072 NO cabe (ver la aritmética de la cabecera).
# 4. "El modelo anterior era más rápido"
#    → El MXFP4 (12,1 GB) es más pequeño que el Q8_K_XL (13,2 GB). El
#      Q8_K_XL deja menos sitio para la KV, y a igualdad de ctx eso se nota.
#      Con ROPERO_GPTOSS_GGUF=openai_gpt-oss-20b-MXFP4.gguf se vuelve al
#      MXFP4 (pero hay que descargarlo).
# ==============================================================================
