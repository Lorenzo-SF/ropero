#!/usr/bin/env bash
# ==============================================================================
# Gemma-4-26B-A4B-it-qat-UD-Q4_K_XL + MTP draft + mmproj (visión) — CONFIG OPTIMIZADA
# ==============================================================================
# Config basada en el script de Qwen3.8-27B, adaptada a Gemma 4 MoE:
#   - Modelo MoE (26B totales, ~4B activos) -> Q4_K_XL ocupa poco espacio
#   - MTP drafter externo con --spec-type draft-mtp
#   - Multimodal: --mmproj para entrada de imágenes (SOLO con --vision)
#   - Acceptance rate medido en esta máquina: 49-88% según el prompt
#
# Por que esta config es óptima para Gemma:
#   - Q4_K_XL (~15-16 GB) cabe en GPUs de 16-24 GB con KV cache q8_0
#   - MTP aporta aceleración neta con n-max 4 (equilibrio velocidad/overhead)
#   - Sampling canónico de Gemma 4: temp 1.0, top-p 0.95, top-k 64
#   - Contexto 131072 por defecto, ampliable a 262144 vía env var
#
# Peculiaridades de Gemma:
#   1. Spec-type draft-mtp: drafter nativo de Gemma (mtp-gemma-4-26B-A4B-it)
#   2. Model-draft: archivo mtp-gemma-4-26B-A4B-it.gguf (Q4_0 near-lossless)
#   3. mmproj: gemma-4-26B-A4B-it-mmproj-BF16.gguf (torre de visión)
#   4. MoE: si hay OOM, usar --n-cpu-moe para descargar expertos a RAM
#
# ── USO ─────────────────────────────────────────────────────────────────────
#   ropero gemma                      → texto puro, sin visión
#   ropero gemma --vision             → añade el mmproj (lee imágenes)
#   ropero gemma --n-cpu-moe 8        → 8 capas de expertos MoE en la CPU
#   ropero gemma --vision --n-cpu-moe 8
#   ropero gemma --high               → razonamiento + CoT conservado
#   ropero gemma --low                → sin razonamiento (respuesta directa)
#
# Override env vars (sin editar este archivo):
#   ROPERO_GEMMA_CTX=262144 ropero gemma    # 256k context
#   ROPERO_GEMMA_TEMP=0.7 ropero gemma      # más determinista (Gemma recomienda 1.0)
#   ROPERO_GEMMA_MTP_NMAX=2 ropero gemma    # menos especulación
#   ROPERO_GEMMA_N_CPU_MOE=8 ropero gemma   # experts MoE en CPU por defecto
# ==============================================================================

MODEL_ENGINE="llama-server"
MODEL_GGUF="gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf"

# Rutas de los ficheros del BUNDLE. Se usan $GGUF_DIR (no $HOME/models/gguf)
# para que un GGUF_DIR distinto se respete: 01-env.sh lo define y lo expande
# (un "~" dentro de comillas dobles no se expande, y por eso aquí no hay "~").
# Nombres propios (no MODEL_*): el loader sourcea todos los scripts seguidos
# y un MODEL_* compartido lo pisaría el siguiente script.
GEMMA_MTP="$GGUF_DIR/mtp-gemma-4-26B-A4B-it.gguf"
GEMMA_MMPROJ="$GGUF_DIR/gemma-4-26B-A4B-it-mmproj-BF16.gguf"

MODEL_CTX="${ROPERO_GEMMA_CTX:-131072}"

# Niveles de razonamiento. Gemma 4 NO tiene "reasoning_effort" como gpt-oss:
# su chat template solo conoce enable_thinking y preserve_thinking (el
# "Preserved Thinking" de Google). Los niveles se traducen a sus knobs reales:
#
#   low     → enable_thinking=false              → respuesta directa, sin CoT
#   medium  → enable_thinking=true               → razona y responde (default)
#   high    → enable_thinking + preserve_thinking → además conserva el CoT
#                                                anterior en el historial, que
#                                                es lo que necesita un flujo
#                                                agentic con tool calls
#
# MEDIDO en esta máquina (RTX 5080 Laptop, 16 GB):
#   enable_thinking=false → 0 caracteres de razonamiento en la respuesta
#   enable_thinking=true  → ~500 caracteres, y el CoT sale limpio en el campo
#                           `reasoning_content` de la API (llama-server sabe
#                           parsear el bloque <|channel>thought de Gemma 4).
MODEL_LEVELS="${MODEL_LEVELS:-low medium high}"
MODEL_LEVEL_DEFAULT="${MODEL_LEVEL_DEFAULT:-medium}"

# Flags y opciones que este modelo acepta por argumento. Los declara el .sh;
# el CLI los valida contra esta lista, así que `ropero embed --vision` da
# error en vez de ignorarse en silencio.
MODEL_FLAGS="${MODEL_FLAGS:-vision}"
MODEL_OPTIONS="${MODEL_OPTIONS:-n-cpu-moe}"

# -----------------------------------------------------------------------------
# COPIAS PROPIAS — por qué hacen falta
#
# MODEL_LEVELS, MODEL_LEVEL_DEFAULT, MODEL_FLAGS y MODEL_OPTIONS son ARRAYS
# asociativos del CLI (declare -A). El loader sourcea TODOS los scripts
# seguidos, así que dentro de get_model_args_* esos valores son los del
# ÚLTIMO script sourceado, no los de este. Es el mismo motivo por el que
# MODEL_GGUF va en array (ver la cabecera de 21-download-models.sh).
#
# El script captura los suyos al sourcearse y la función lee la COPIA.
# -----------------------------------------------------------------------------
GEMMA_LEVELS="$MODEL_LEVELS"
GEMMA_LEVEL_DEFAULT="$MODEL_LEVEL_DEFAULT"

# Alias público para el router de ropero
MODEL_ALIAS="${MODEL_ALIAS:-gemma}"

get_model_args_gemma() {
    # ---- Nivel de razonamiento → enable_thinking / preserve_thinking ----
    local level="${ROPERO_MODEL_LEVEL:-${GEMMA_LEVEL_DEFAULT:-medium}}"
    local thinking=true preserve=false
    case "$level" in
        low)    thinking=false; preserve=false ;;
        medium) thinking=true;  preserve=false ;;
        high)   thinking=true;  preserve=true  ;;
        *)
            echo "ERROR: nivel de razonamiento inválido: '$level' (válidos: $MODEL_LEVELS)" >&2
            return 1
            ;;
    esac

    # ---- MoE en CPU: default POR CASO, ajustable en los dos ----
    # Los expertos MoE son el 88% de los pesos (22,8B de 26B), así que cada
    # capa que se queda en la RAM libera ~0,46 GB de VRAM. Ese hueco es
    # exactamente el que necesita el mmproj cuando hay visión.
    #
    #   sin --vision → 0 capas en la CPU: no hay mmproj que hospedar, la
    #                   VRAM está libre y lo que se quiere es velocidad.
    #   con --vision → 5 capas en la CPU: el mmproj son 1139 MiB y sin
    #                   hacer hueco no entra.
    #
    # En los dos casos se puede forzar con --n-cpu-moe N (o con
    # ROPERO_GEMMA_N_CPU_MOE=N), que manda sobre el default.
    #
    # Coste de bajar capas: se calculan en la CPU, así que baja el tokens/s.
    # Es el trueque de "más contexto y visión" contra "más velocidad".
    local n_cpu_moe_default=0
    if [[ "${ROPERO_MODEL_FLAG_VISION:-0}" == "1" ]]; then
        n_cpu_moe_default=5
    fi
    local n_cpu_moe="${ROPERO_MODEL_OPT_N_CPU_MOE:-${ROPERO_GEMMA_N_CPU_MOE:-$n_cpu_moe_default}}"

    # ---- MTP: cuántos tokens se especulan de golpe ----
    # Unsloth: "--spec-draft-n-max 2 es el mejor punto de partida, pero no lo
    # des por óptimo: prueba del 1 al 6". Con 4 se midió 49-88% de aceptación
    # en esta máquina.
    local mtp_nmax="${ROPERO_GEMMA_MTP_NMAX:-4}"

    local -a args=(
        # ---- GPU offload ----
        # ngl 99: todos los layers en GPU (30 layers + MTP head)
        # Si hay OOM por el MoE, bajar a 60 o subir --n-cpu-moe
        --n-gpu-layers 99
        # La KV cache se QUEDA EN LA RAM, no en la VRAM. No es un descuido:
        # a 131072 en q8_0 la KV son 1139 MiB, y con ella dentro la tarjeta
        # (15,9 GiB útiles) no llega. En RAM sí cabe, y el penalty medido es
        # bajo: 104-140 tok/s con la GPU al 95% de ocupación.
        --no-kv-offload
        --n-cpu-moe "$n_cpu_moe"
        # ---- Atención ----
        --flash-attn on

        # ---- KV cache (q8_0 para precisión en contexto largo) ----
        --cache-type-k q8_0
        --cache-type-v q8_0
        --cache-prompt
        --context-shift
        --kv-unified                                   # single user, unified buffer

        # ---- Speculative decoding (MTP nativo) ----
        # Ruta ABSOLUTA en model-draft (~ no expande en comillas dobles)
        --spec-type draft-mtp
        --model-draft "$GEMMA_MTP"
        --n-gpu-layers-draft -1                        # auto-fit del draft
        --spec-draft-n-max "$mtp_nmax"                 # Gemma: 2-7 óptimo
        --spec-draft-n-min 1
        --spec-draft-p-min 0.0                         # sin umbral de probabilidad
        --spec-draft-backend-sampling

        # ---- Modelo / carga ----
        --jinja                                        # chat template de Gemma
        --load-mode none

        # ---- Sampling (canónico Gemma 4) ----
        --temp "${ROPERO_GEMMA_TEMP:-1.0}"
        --top-p 0.95
        --top-k 64
        --min-p 0.0
        --presence-penalty 1.5
        --repeat-penalty 1.0
        --repeat-last-n 64

        # ---- Batching / threads ----
        --batch-size 4096
        --ubatch-size 1024
        --parallel 1

        # ---- Generación ----
        --n-predict 16384
        --keep 1024
    )

    # ---- Multimodal (visión): SOLO si se pide ----
    # El mmproj son 1139 MiB de VRAM. El 95% de lo que se le pide a un modelo
    # de texto no lo necesita, así que por defecto el servidor arranca SIN
    # torre de visión: 14593 MiB en vez de 15732, y el hueco de VRAM se
    # dedicado al contexto o a más expertos MoE en GPU.
    #
    # Con --vision se añade el projector y el modelo lee imágenes.
    if [[ "${ROPERO_MODEL_FLAG_VISION:-0}" == "1" ]]; then
        if [[ ! -s "$GEMMA_MMPROJ" ]]; then
            echo "ERROR: --vision pide el proyector de visión y no está en disco:" >&2
            echo "       $GEMMA_MMPROJ" >&2
            echo "       Descárgalo con: ropero install models gemma4" >&2
            return 1
        fi
        args+=(--mmproj "$GEMMA_MMPROJ")
    fi

    # ---- Thinking (Gemma 4 es híbrido: razona y responde) ----
    args+=(--chat-template-kwargs "{\"enable_thinking\":$thinking,\"preserve_thinking\":$preserve}")

    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# ==============================================================================
# 1. OOM con contexto alto:
#      - Bajar KV a q4_0 (menos precisión, +5% velocidad)
#      - ROPERO_GEMMA_CTX=65536 ropero gemma
#      - Subir --n-cpu-moe (descarga más expertos MoE a RAM)
#
# 2. MTP acceptance rate bajo (<30%):
#      - Verificar log "draft acceptance rate"
#      - Probar --spec-draft-n-max 2 (menos especulación, mayor tasa)
#      - Probar --spec-draft-n-max 6 (más especulación si la tasa es alta)
#      - o ROPERO_GEMMA_MTP_NMAX=2 ropero gemma
#
# 3. Respuestas cortan antes de tiempo:
#      - Aumentar --n-predict (actual 16384)
#      - Ojo: con el razonamiento activo, el CoT se come el presupuesto. Una
#        respuesta corta con --n-predict corto puede devolver content vacío y
#        el CoT entero en reasoning_content.
#
# 4. Multimodal no funciona:
#      - Falta el flag: `ropero gemma` arranca SIN visión a propósito.
#        Para leer imágenes: `ropero gemma --vision`.
#      - Verificar que el mmproj existe (gemma-4-26B-A4B-it-mmproj-BF16.gguf)
#      - Comprobar log "[mtmd] vision model loaded"
#      - Con el mmproj cargado, el budget de tokens de imagen se recorta al
#        n_ubatch: con -ub 1024 da hasta 1024 tokens de imagen (el modelo
#        quiere 1120).
#
# 5. Si quieres 256k context:
#      - ROPERO_GEMMA_CTX=262144 ropero gemma
#      - Requiere GPU con más VRAM o --n-cpu-moe agresivo
#
# 6. La VRAM no cuadra:
#      - Medido con esta config (texto + visión): 15732 MiB de 16303.
#        Sin visión: ~14593 MiB. Sin --n-cpu-moe 5: revienta (no caben los
#        expertos + la KV + el mmproj).
# ==============================================================================
