#!/usr/bin/env bash
# =============================================================================
# Modelo: ministral
#
# El cerebro del vault. Cubre DOS roles con un solo modelo:
#
#   · CLASIFICADOR  — dónde va esta nota (¿inbox o archive? ¿GTD o PARA?)
#   · ENRIQUECEDOR  — lee la nota + las 8 que devolvió el ranker y ESCRIBE
#                     prosa sobre ellas: un comentario, una descripción, un
#                     "esto ya se mencionó en…", "esto se parece a…"
#
# POR QUÉ UN SOLO MODELO PARA LOS DOS
# ───────────────────────────────────
# Los dos hacen EXACTAMENTE la misma llamada: reciben texto (la nota y sus
# vecinas) y devuelven JSON estructurado. La única diferencia es qué se le
# pregunta. Es el mismo modelo con tres prompts, no dos tareas distintas.
#
# Y no se fusionó por ahorrar RAM: se fusionó porque MEJORA. La alternativa
# era FunctionGemma 270M, y su model card dice que "is not intended for use as
# a direct dialogue model... after further fine-tuning", con un BFCL
# Parallel Multiple de 29,5 a 0-shot. Clasificar una nota en 5 categorías y 5
# destinos ES un caso Parallel Multiple. Es decir: la casilla donde el 270M
# está peor medido. Un 14B con function calling nativo le gana.
#
# ── Lo que SÍ es, medido (unsloth.ai/docs, docs.mistral.ai) ──
#   · denso 14B (13,5B LM + 0,4B encoder de visión) — NO es MoE
#   · contexto nativo 256K (393K extrapolando) — en CPU eso no se ve, ver abajo
#   · function calling NATIVO
#   · salida JSON NATIVA
#   · rendimiento comparable a Mistral Small 3.2 24B, con 24 GB sin cuantizar
#   · GGUF oficial: mistralai/Ministral-3-14B-Instruct-2512-GGUF
#
# ── Y lo que NO es, para no esperarse otra cosa ──
#   · NO es rápido. Es denso: 14B activos en cada token. Es un modelo de LOTE,
#     pensado para que el cron lo ejecute, no para conversar con él.
#   · Los 256K son de GPU. En CPU el techo lo pone el ancho de banda del KV
#     cache, muy por debajo. 32768 es el número honesto para esta máquina.
#
# Puerto fijo (:9995).
# =============================================================================

MODEL_ENGINE="llama-server"

MODEL_GGUF="Ministral-3-14B-Instruct-2512-UD-Q5_K_XL.gguf"
MODEL_CTX="${ROPERO_VAULT_CTX:-32768}"
MODEL_PORT=9995

# Alias público. `ropero vault` en vez de `ropero ministral`.
MODEL_ALIAS="${MODEL_ALIAS:-vault}"

get_model_args_ministral() {
    local -a args=(
        # ---- FORZADO A CPU ----
        # 9,6 GB de pesos en la VRAM mientras el coder la necesita. Esta es
        # exactamente la razón por la que la lane del vault va a CPU: el
        # procesamiento del vault es un cron horario, y si compartiera GPU
        # desalojaría el modelo con el que estás trabajando para hacer un
        # trabajo que nadie está esperando.
        --n-gpu-layers 0

        # ---- KV cache ----
        --cache-type-k q8_0
        --cache-type-v q8_0
        --cache-prompt
        --context-shift
        --kv-unified

        # ---- Plantilla / salida estructurada ----
        --jinja                                        # chat template Mistral 3
        --reasoning-format none                        # Instruct: no emite
                                                      # <think>, y decirlo
                                                      # explícito evita que el
                                                      # CoT se cuele en la nota

        # ---- Sampling ----
        # Un valor para tres roles distintos, y por eso NO está al extremo:
        #   clasificador  → quiere determinismo (aquí gana el --temp bajo)
        #   enriquecedor   → quiere prosa que no suene a plantilla
        #   ask           → quiere variedad
        # El ejemplo oficial de Unsloth para Instruct es temp 0.15; eso es
        # correcto para clasificar y seco para escribir. 0,3 es el punto medio.
        #
        # Y si un rol necesita otro valor, se lo pasa POR PETICIÓN: llama-server
        # acepta temp/seed en el body. Este es el default, no una obligación.
        --temp 0.3
        --top-p 0.95
        --repeat-penalty 1.0
        --repeat-last-n 64

        # ---- Batching / threads ----
        --batch-size 4096
        --ubatch-size 1024
        --parallel 1
        --threads 12
        --threads-batch 16

        # ---- Generación ----
        --n-predict 4096                              # suficiente para una
                                                      # nota enriquecida; la
                                                      # doc pide 16384 para
                                                      # Instruct, pero eso es
                                                      # para salidas masivas
        --keep 1024

        --load-mode none
    )
    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# VISIÓN (opcional, no incluida a propósito)
# ─────────────────────────────────────
# El modelo trae un encoder de visión de 0,4B, pero va SEPARADO, en un mmproj.
# Sin él el servidor arranca igual y solo lee texto — que es el 95% de un vault
# de Obsidian, así que la arranque sin visión es la opción por defecto.
#
# Para activarla:
#   1. descarga el projector al repo de unsloth (mmproj-F16.gguf)
#   2. añádelo al catálogo con _cat_gguf
#   3. y en este fichero, al final de args:  --mmproj /ruta/al/mmproj.gguf
#
# Con eso, el enriquecedor lee un diagrama o un pantallazo sin cambiar de modelo.
# ==============================================================================
# ==============================================================================
# TROUBLESHOOTING
# 1. "Va lentísimo"
#    → Es denso y está en CPU: eso es lo esperado. Se usa por lotes. Si
#      necesitas interacción, ese trabajo no es suyo.
# 2. "El cron no llega a completar el vault"
#    → Baja MODEL_CTX (32768 → 16384) o sube --threads-batch. Y solo si eso no
#      basta, considera el fine-tune del clasificador que se explica en el Apéndice:
#      un modelo pequeño y afinado es la respuesta a la latencia, no un ctx más
#      corto.
# 3. "El JSON sale con markdown alrededor (```json … ```)"
#    → --jinja está bien pero el prompt pide JSON "crudo". Se limpia en el
#      cliente; no lo metas en un parser que no lo tolere sin strip.
# 4. "El enriquecimiento inventa conexiones que no existen"
#    → ESO NO ES UN BUG DE ESTE SCRIPT. Es el diseño: el reranker decide qué
#      es relevante y pone un score; si el score no llega al umbral, no se
#      escribe nada. Si el modelo se está inventando conexiones, se le está
#      dando contexto sin scores, o no se está aplicando el umbral. La defensa
#      está aguas arriba, aquí no.
# ==============================================================================
