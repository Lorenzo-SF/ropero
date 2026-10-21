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
# Optimizaciones aplicadas (respecto a la versión previa):
#   - spec-draft-n-max 4 → 8: con aceptación alta, más profundidad de draft
#     amortiza mejor cada forward pass del modelo principal (cuello de botella
#     real cuando la KV vive en RAM).
#   - spec-draft-p-min 0.0 → 0.6: el drafter se corta solo cuando su
#     confianza cae, evitando drafts que acabarían rechazados.
#   - NOTA: --n-cpu-moe 5 y --no-kv-offload se MANTIENEN porque sin ellos
#     hay OOM en esta máquina (16 GB VRAM). El precio es velocidad de
#     generación, pero es el mínimo viable sin cambiar hardware.
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
#   ROPERO_GEMMA_CTX=262144 ropero gemma     # 256k context
#   ROPERO_GEMMA_TEMP=0.7 ropero gemma       # más determinista
#   ROPERO_GEMMA_MTP_NMAX=6 ropero gemma     # menos especulación
#   ROPERO_GEMMA_MTP_PMIN=0.0 ropero gemma   # drafter sin umbral (comport. antiguo)
#   ROPERO_GEMMA_N_CPU_MOE=8 ropero gemma    # experts MoE en CPU por defecto
# ==============================================================================

MODEL_ENGINE="llama-server"
MODEL_GGUF="gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf"

# Rutas de los ficheros del BUNDLE. Se usan $GGUF_DIR (no $HOME/models/gguf)
# para que un GGUF_DIR distinto se respete: 01-env.sh lo define y lo expande
# (un "~" dentro de comillas dobles no se expande, y por eso aquí no hay "~").
GEMMA_MTP="$GGUF_DIR/mtp-gemma-4-26B-A4B-it.gguf"
GEMMA_MMPROJ="$GGUF_DIR/gemma-4-26B-A4B-it-mmproj-BF16.gguf"

MODEL_CTX="${ROPERO_GEMMA_CTX:-131072}"

# Niveles de razonamiento. Gemma 4 NO tiene "reasoning_effort" como gpt-oss:
# su chat template solo conoce enable_thinking y preserve_thinking.
#
#   low     → enable_thinking=false              → respuesta directa, sin CoT
#   medium  → enable_thinking=true               → razona y responde (default)
#   high    → enable_thinking + preserve_thinking → además conserva el CoT
#                                                anterior en el historial
#
# MEDIDO en esta máquina (RTX 5080 Laptop, 16 GB):
#   enable_thinking=false → 0 caracteres de razonamiento
#   enable_thinking=true  → ~500 caracteres en reasoning_content
MODEL_LEVELS="${MODEL_LEVELS:-low medium high}"
MODEL_LEVEL_DEFAULT="${MODEL_LEVEL_DEFAULT:-medium}"

# Flags y opciones que este modelo acepta por argumento.
MODEL_FLAGS="${MODEL_FLAGS:-vision}"
MODEL_OPTIONS="${MODEL_OPTIONS:-n-cpu-moe}"

# -----------------------------------------------------------------------------
# COPIAS PROPIAS — por qué hacen falta
#
# MODEL_LEVELS, MODEL_LEVEL_DEFAULT, MODEL_FLAGS y MODEL_OPTIONS son ARRAYS
# asociativos del CLI (declare -A). El loader sourcea TODOS los scripts
# seguidos, así que dentro de get_model_args_* esos valores son los del
# ÚLTIMO script sourceado, no los de este.
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

    # ---- MoE en CPU: 5 por defecto (obligatorio en esta máquina) ----
    # Los expertos MoE son el 88% de los pesos (22,8B de 26B), así que cada
    # capa que se queda en la RAM libera ~0,46 GB de VRAM.
    #
    # En esta máquina (RTX 5080 Laptop, 16 GB) el modelo completo + KV +
    # overhead no cabe en VRAM, así que 5 capas de expertos se quedan en CPU
    # tanto con visión como sin ella. Sin ese hueco hay OOM.
    #
    # Se puede forzar otro valor con --n-cpu-moe N (o ROPERO_GEMMA_N_CPU_MOE=N),
    # que manda sobre el default.
    #
    # Coste de bajar capas: se calculan en la CPU → baja tokens/s.
    # Subir capas libera VRAM pero penaliza velocidad; bajar capas acelera
    # pero puede provocar OOM.
    local n_cpu_moe_default=5
    local n_cpu_moe="${ROPERO_MODEL_OPT_N_CPU_MOE:-${ROPERO_GEMMA_N_CPU_MOE:-$n_cpu_moe_default}}"

    # ---- MTP: profundidad y umbral del drafter ----
    # n-max: cuántos tokens puede especular el drafter de golpe.
    #   Con aceptación alta (56.9% en la línea base), subir a 8 amortiza
    #   mejor cada forward pass del modelo principal (el cuello de botella
    #   real cuando la KV vive en RAM). Con n-max=4, output esperado por
    #   forward ≈ 3.36 tokens; con n-max=8 y misma aceptación ≈ 4.3.
    # p-min: umbral de confianza bajo el cual el drafter deja de especular.
    #   0.0 = nunca se corta (siempre llega a n-max). 0.6 = se corta cuando
    #   su confianza cae, ahorrando drafts que acabarían rechazados.
    # n-min: mínimo de tokens que siempre intenta especular.
    local mtp_nmax="${ROPERO_GEMMA_MTP_NMAX:-8}"
    local mtp_nmin="${ROPERO_GEMMA_MTP_NMIN:-1}"
    local mtp_pmin="${ROPERO_GEMMA_MTP_PMIN:-0.6}"

    local -a args=(
        # ---- GPU offload ----
        # ngl 99: todos los layers en GPU (30 layers + MTP head)
        --n-gpu-layers 99
        # La KV cache se QUEDA EN LA RAM, no en la VRAM. No es un descuido:
        # a 131072 en q8_0 la KV son 1139 MiB, y con ella dentro la tarjeta
        # (15,9 GiB útiles) no llega. En RAM sí cabe, y el penalty medido es
        # bajo: 104-140 tok/s con la GPU al 95% de ocupación.
        --kv-unified
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
        --spec-draft-n-max "$mtp_nmax"                 # 8: más profundidad
        --spec-draft-n-min "$mtp_nmin"
        --spec-draft-p-min "$mtp_pmin"                 # 0.6: corta cuando baja confianza
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
    # torre de visión: 14593 MiB en vez de 15732.
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
#      - Probar ROPERO_GEMMA_MTP_NMAX=4 (menos especulación)
#      - o ROPERO_GEMMA_MTP_PMIN=0.0 (sin umbral, comportamiento antiguo)
#      - Con n-max=8 y p-min=0.6 deberías ver acceptance por token similar
#        al baseline (aunque la media por draft baje); lo que importa es
#        tokens generados / tokens totales producidos por el modelo principal.
#
# 3. Respuestas cortan antes de tiempo:
#      - Aumentar --n-predict (actual 16384)
#      - Ojo: con el razonamiento activo, el CoT se come el presupuesto.
#
# 4. Multimodal no funciona:
#      - Falta el flag: `ropero gemma` arranca SIN visión a propósito.
#        Para leer imágenes: `ropero gemma --vision`.
#      - Verificar que el mmproj existe (gemma-4-26B-A4B-it-mmproj-BF16.gguf)
#      - Comprobar log "[mtmd] vision model loaded"
#
# 5. Si quieres 256k context:
#      - ROPERO_GEMMA_CTX=262144 ropero gemma
#      - Requiere GPU con más VRAM o --n-cpu-moe agresivo
#
# 6. La VRAM no cuadra:
#      - Medido con esta config (texto + visión): 15732 MiB de 16303.
#        Sin visión: ~14593 MiB. Sin --n-cpu-moe 5: revienta.
#
# 7. Barrido de MTP (si quieres exprimir más):
#      for nmax in 4 6 8 10; do
#        for pmin in 0.0 0.4 0.6 0.8; do
#          ROPERO_GEMMA_MTP_NMAX=$nmax ROPERO_GEMMA_MTP_PMIN=$pmin \
#            ropero gemma 2>&1 | grep "draft acceptance"
#        done
#      done
#      Quédate con el par (nmax, pmin) que dé mayor ratio
#      tokens_generados / tiempo_total en tu prompt típico.
# ==============================================================================