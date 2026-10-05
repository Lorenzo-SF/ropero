#!/usr/bin/env bash
# =============================================================================
# metaloss — gpt-oss-20b en MLX (Metal), el slot de razonamiento rápido
# -----------------------------------------------------------------------------
# POR QUÉ ESTE MODELO EXISTE Y POR QUÉ SE LLAMA metaloss
#
# En ropero ya tienes gpt-oss-20b en tres sitios, los tres sobre GGUF con
# llama-server: gptoss_low / gptoss_medium / gptoss_high (alias `verifier` y
# `designer`). Este es el MISMO modelo por otro camino:
#
#   GGUF + llama.cpp  → Metal también, pero pasa por el backend de ggml, con su
#                       propia implementación de MoE, su propia cuantización y
#                       su propio sampler.
#   MLX + mlx_lm      → el backend nativo de Apple. Para un MoE como este, donde
#                       el 90% de los parámetros son expertos que se eligen 4 de
#                       32 en cada token, MLX gana: cuantización mxfp4 nativa,
#                       prompt cache de verdad y sin capas de traducción entre
#                       el modelo y la GPU.
#
# El nombre es para que no se confunda con los otros: `metaloss` = "gpt-oss en
# Metal". Si quisieras el mismo modelo en dos motores a la vez, este pinea su
# propio puerto y no pelea por :9999.
#
# -----------------------------------------------------------------------------
# ⚠️ ESTE MODELO LLEVA PROXY DE AUTH (ya lo pone ropero, no tienes que hacer nada)
# `mlx_lm.server` NO tiene `--api-key`: ni flag ni variable de entorno
# (verificado en su --help y en mlx_lm/server.py de 0.31.3). ropero le pone
# delante ropero.d/_authproxy.py, que sí exige tu LLAMA_API_KEY. El mlx_lm.server
# real escucha solo en 127.0.0.1, en :10992. No lo arranques a mano en 0.0.0.0.
#
# ⚠️ Y EL NOMBRE DEL MODELO LO REESCRIBE EL PROXY
# `mlx_lm.server` sólo acepta "default_model" como nombre de modelo: su
# ModelProvider registra `{"default_model": <lo que sea --model>}` y resuelve con
# `_model_map.get(nombre, nombre)`, así que lo que no está en el mapa se usa
# como si fuera una ruta o un REPO_ID de HuggingFace. Con "gptoss" en el body,
# huggingface_hub busca el repo `gptoss`, y lo que vuelve es:
#
#     404 + "Repository Not Found for url: .../api/models/gptoss/revision/main"
#
# que se lee como "le falta el repo" cuando el repo está descargado y el modelo
# cargado. Es el mismo bug con tres nombres distintos (gptoss, metaloss,
# default_model → sólo el último funciona).
#
# ropero lo resuelve en el proxy: /v1/models anuncia "metaloss" y además
# reescribe el campo `model` del body a "default_model" (--rewrite-model, lo
# decide ropero_engine_pinned_model). POR ESO en el opencode.jsonc puedes
# llamar a este modelo como quieras — "gptoss", "metaloss", lo que pongas en el
# provider — y funciona igual. No hace falta tocar nada aquí para renombrarlo.
# =============================================================================

# -----------------------------------------------------------------------------
# QUÉ MODELO ES ESTE Y POR QUÉ ESTE Y NO OTRO
# -----------------------------------------------------------------------------
#
# ⚠️ OJO: el repo que me pasaste, `unsloth/gpt-oss-20b`, NO es el correcto aquí.
# Es un checkpoint BF16 de 21B que Unsloth publica para vLLM/transformers:
#
#   · No está en formato MLX.
#   · Pesa ~40 GB en bf16. En una máquina con 36 GB de memoria unificada no cabe
#     ni de lejos: macOS empezaría a paginar y tendrías un modelo que tarda
#     minutos por token. Existe `mrtoots/unsloth-gpt-oss-20b-mlx-fp16` (alguien
#     lo convirtió a MLX), pero mantiene el fp16: 39.0 GB. Sigue sin caber.
#
# Para MLX el repo es de `mlx-community`. Y el criterio NO es "el más pequeño":
# con 36 GB tienes margen de sobra, así que la pregunta es cuál pierde menos
# calidad. Medido sobre los config.json reales de cada repo:
#
#   repo                                   pesos    reparto de bits
#   ─────────────────────────────────────────────────────────────────────────
#   mlx-community/gpt-oss-20b-MXFP4-Q4     10.4 GB  MoE mxfp4-4 · resto affine-4
#   mlx-community/gpt-oss-20b-MXFP4-Q8     11.2 GB  MoE mxfp4-4 · resto affine-8   ← este
#   lmstudio-community/gpt-oss-20b-MLX-8bit 20.7 GB  TODO affine-8, sin mxfp4
#   BMP / mrtoots (fp16)                    39.0 GB  no cabe
#
# La clave está en QUÉ es mxfp4. Los expertos MoE son ~19B de los 21B
# parámetros (los MoE/MLP experts solos son 19B), y en los tres repos de arriba
# esos 19B están a MXFP4 de 4 bits IGUAL. La diferencia entre "Q4" y "Q8" está
# solo en los otros 122 tensores: las proyecciones q/k/v/o, el router, embeddings y
# lm_head.
#
#   Q4 → esos 122 tensores a 4 bit   (ahorra 0.8 GB)
#   Q8 → esos 122 tensores a 8 bit   (+0.8 GB)
#
# Y aquí está el argumento: **ir a `lmstudio ... MLX-8bit` cuesta 9.5 GB más para
# subir a 8 bits unos 2B parámetros, dejando los 19B de expertos exactamente igual
# que en MXFP4-Q8.** Es un intercambio terrible. MXFP4-Q8 ya tiene la atención y
# el router en 8 bits, que es donde una cuantización agresiva se nota.
#
# El argumento definitivo a favor de MXFP4, y no de "una cuantización más": es el
# formato NATIVO del modelo. OpenAI post-entrenó gpt-oss con cuantización MXFP4 de
# los pesos MoE, y TODAS sus evaluaciones se hicieron con ella. MXFP4-Q8 no es un
# MXFP4 re-cuantizado a posteriori: es el mismo formato, con las capas no-MoE
# (no-MoE) a 8 bits para no perder lo que el modelo sí aprendió con precisión.
#
# Además: 260.575 descargas frente a 2.504 del Q4. La comunidad eligió.
#
# ── Alternativas si quisieras otra cosa ──────────────────────────────────────
#   Menos peso, aunque peor:  mlx-community/gpt-oss-20b-MXFP4-Q4   (10.4 GB)
#   Más bits, más sitio:      mlx-community/gpt-oss-20b-MXFP4-Q4   + ctx menor
#   Todo a 8 bits:            lmstudio-community/gpt-oss-20b-MLX-8bit (20.7 GB)
#                            Cabe en 36 GB, pero solo con ctx ≤ 32k y sin nada
#                            más abierto, y para el MoE no aporta (ver arriba).
#
# -----------------------------------------------------------------------------
# ARQUITECTURA Y FORMATO (lo que tiene que ser el directorio)
# -----------------------------------------------------------------------------
#
# FORMATO: pesos YA convertidos a MLX (safetensors). NO es GGUF (eso es
# llama.cpp) y NO es el checkpoint bf16 de HuggingFace tal cual.
#
# ARQUITECTURA: gpt_oss (GptOssForCausalLM). MoE: 32 expertos, 4 activos por
# token, 3.6B activos de 21B. 24 capas, hidden 2880, 64 cabezas de atención, 8
# cabezas KV, head_dim 64. Contexto máximo 131072.
#
# mlx-lm 0.31.3 trae `mlx_lm/models/gpt_oss.py` y carga este config MXFP4 sin
# que hagas nada: es el layout exacto que espera, con los overrides por tensor
# que el config declara (98 tensores affine-8 + 24 mxfp4-8, y mxfp4-4 para el
# resto). Verificado leyendo `mlx_lm/utils.py`.
#
# ── Descarga (12.1 GB) ──────────────────────────────────────────────────────
#   hf download mlx-community/gpt-oss-20b-MXFP4-Q8 \
#       --local-dir ~/models/safetensors/gpt-oss-20b-MXFP4-Q8
#
# O, desde ropero, que además es idempotente (pone un .download_complete):
#   ropero --install
# que llama a ropero_download_safetensors "mlx-community/gpt-oss-20b-MXFP4-Q8" \
#                                    "gpt-oss-20b-MXFP4-Q8" "12.1 GB"
#
# ⚠️ El directorio DEBE llamarse como MODEL_PATH de abajo. Es el preflight de
# ropero: si no existe, no arranca y te lo dice (con este mismo mensaje).
# =============================================================================

# Aviso que ropero imprime si el directorio no existe. Cada línea es una orden.
# `ropero metaloss` sin esto solo diría "Directorio de modelo no encontrado", y
# la respuesta (12 GB de HF) está tres scrolls más arriba en este fichero.
MODEL_ENGINE="mlx_lm"
MODEL_PATH="gpt-oss-20b-MXFP4-Q8"
MODEL_FORMAT="mlx-mxfp4"

# Contexto. 65536 porque es lo que de verdad se usa en sesiones agénticas y
# deja el KV cache en 3.0 GB (ver el cálculo en el guard de RAM de abajo). El
# máximo del modelo son 131072 y funciona: sube con
#
#     ROPERO_METALOSS_CTX=131072 ropero metaloss
#
# pero el prefill de 128k en un M4 Max de 10 núcleos se cuenta en minutos, así
# que para uso normal es tirar el presupuesto por el suelo para no notar nada.
# Unsloth pone 16384 como mínimo razonable; 65536 es el punto cómodo.
MODEL_CTX="${ROPERO_METALOSS_CTX:-65536}"

# Puerto propio. Sin MODEL_PORT usaría el slot :9999, que es donde suele estar
# tu llama-server. Pinear el 9992 permite tener el MoE en Metal y un GGUF en
# llama.cpp vivos a la vez, que es justo cuando tiene sentido tener los dos.
MODEL_PORT=9992

# SIN MODEL_ALIAS a propósito, y no es descuido: el nombre del modelo YA es el
# nombre del fichero, así que `ropero metaloss` funciona sin alias. Poner
# MODEL_ALIAS="metaloss" haría que el loader.registera la key y luego avisara de
# que colisiona con el script físico (que es justo lo que pasa con el resto de
# alias, que sirven para apuntar un nombre humano a un script con otro nombre).
# El alias tiene sentido cuando `ropero coder` debe ser devstral.sh. Aquí el
# nombre del script ya ES el nombre que quieres.

# -----------------------------------------------------------------------------
# PARÁMETROS DE INFERENCIA
#
# Los tres de sampling son los que recomienda OpenAI para gpt-oss y los repite
# Unsloth en su guía de ejecución. No son "los de ropero":
#
#   --temp 1.0   gpt-oss se post-entrenó con temperature 1.0. Bajarla a 0.6 o
#                0.7 (que es lo habitual en este repo para otros modelos) DEGRADA
#                el razonamiento: el modelo deja de explorar y las trazas CoT
#                salen mal. No pongas 0.0 por costumbre.
#   --top-p 1.0
#   --top-k 0    desactiva el corte por top-k. Con un MoE, descartar candidatos
#                por frecuencia tiende a descartar el experto que iba bien.
#
# OJO: opencode manda temperature y top_p en CADA request, así que estos son los
# defaults del servidor y solo se aplican si el cliente no los especifica. Para
# forzar los valores de OpenAI en todos los casos hay que ponerlos en el
# opencode.jsonc del provider.
# =============================================================================
get_engine_args_metaloss() {
    # ── Guard de memoria unificada ─────────────────────────────────────────
    # macOS no avisa de que le falta memoria. Carga lo que puede y empieza a
    # paginar, y un modelo de 12 GB paginando sobre 36 GB da ~0.2 tok/s con el
    # disco a tope: parece un modelo roto, no uno sin RAM. Peor aún, deja el
    # resto de la máquina inutilizable. Aquí se corta antes.
    #
    # ── POR QUÉ ARITMÉTICA ENTERA DE BASH Y NO AWK ──────────────────────────
    # Porque con LANG=es_ES.UTF-8 (el locale de esta máquina) la comparación
    # `awk -v n="$need" -v r="$ram" 'BEGIN{exit !(n > r*0.80)}'` devuelve
    # "no cabe" con 3 GB usados de 35 GB disponibles. awk formatea y lee
    # decimales según el locale, y al pasar el resultado de un awk a otro
    # `-v` los separadores decimales no coinciden con lo que espera. El efecto
    # es un guard que bloquea arranques perfectamente válidos.
    #
    # bash hace aritmética de enteros en MB y no le importa el locale. Todo lo
    # que sale por pantalla se formatea al final, y ahí da igual que salga "3,00"
    # o "3.00".
    if [[ "${ROPERO_METALOSS_SKIP_RAMCHECK:-0}" != "1" ]] && [[ "$(uname -s)" == "Darwin" ]]; then
        local dir ctx weights_mb ram_mb kv_mb need_mb limit_mb
        if [[ "$MODEL_PATH" = /* ]]; then
            dir="$MODEL_PATH"
        else
            dir="${SAFETENSORS_DIR:-$HOME/models/safetensors}/$MODEL_PATH"
        fi
        if [[ -d "$dir" ]]; then
            ram_mb=$(sysctl -n hw.memsize 2>/dev/null) || ram_mb=0
            ram_mb=$(( ram_mb / 1048576 ))          # bytes → MiB
            weights_mb=$(du -sm "$dir" 2>/dev/null | awk '{print $1; exit}')
            weights_mb="${weights_mb:-0}"
            ctx="${ROPERO_METALOSS_CTX:-$MODEL_CTX}"

            # KV cache = 48 KiB por token, según la config real del modelo
            # (24 capas × 8 cabezas KV × head_dim 64 × 2 bytes × K y V):
            #     2 × 8 × 64 × 2 × 24 = 49.152 bytes = 48 KiB
            #     ctx 16384 → 0.75 GB   65536 → 3.0 GB   131072 → 6.0 GB
            kv_mb=$(( 49152 * ctx / 1048576 ))
            need_mb=$(( weights_mb + kv_mb ))
            limit_mb=$(( ram_mb * 80 / 100 ))

            if (( ram_mb > 0 && need_mb > limit_mb )); then
                {
                    echo "ERROR: metaloss no cabe en la memoria unificada de esta máquina." >&2
                    printf '  pesos   : %d.%d GB  (%s)\n' \
                        $((weights_mb/1024)) $(( (weights_mb%1024)*10/1024 )) "$MODEL_PATH" >&2
                    printf '  KV cache: %d.%d GB  (ctx=%s, 48 KiB/token)\n' \
                        $((kv_mb/1024)) $(( (kv_mb%1024)*10/1024 )) "$ctx" >&2
                    printf '  TOTAL   : %d MiB  >  80%% de %d MiB\n\n' "$need_mb" "$limit_mb" >&2
                    echo "  macOS no avisa: paginaría y tendrías ~0.2 tok/s." >&2
                    echo "  Soluciones:" >&2
                    echo "    - bajar contexto:    ROPERO_METALOSS_CTX=16384 ropero metaloss" >&2
                    echo "    - modelo más pequeño: mlx-community/gpt-oss-20b-MXFP4-Q4 (10.4 GB)" >&2
                    echo "    - saltar el guard (sólo si sabes lo que haces):" >&2
                    echo "        ROPERO_METALOSS_SKIP_RAMCHECK=1 ropero metaloss" >&2
                }
                return 1
            fi
            printf '  metaloss: %d MiB de pesos + %d MiB de KV (ctx=%s) de %d MiB unificados\n' \
                "$weights_mb" "$kv_mb" "$ctx" "$ram_mb" >&2
        fi
    fi

    # ── Sampling: los de OpenAI para gpt-oss ──────────────────────────────
    printf '%s\n' \
        --temp 1.0 \
        --top-p 1.0 \
        --top-k 0

    # ── Reasoning effort ──────────────────────────────────────────────────
    # El knob más importante del modelo. gpt-oss usa el formato Harmony con
    # canales `analysis` y `final`, y su presupuesto de razonamiento lo fija
    # este valor (low / medium / high; por defecto medium).
    #
    # high es lo que quieres aquí: este es el slot de razonar. Cuesta latencia,
    # que es exactamente el trato que ofrece el modelo. Para respuestas rápidas
    # y baratas usa los gptoss_low/medium de GGUF, que ya tienes.
    printf '%s\n' --chat-template-args "{\"reasoning_effort\":\"${ROPERO_METALOSS_EFFORT:-high}\"}"

    # ── Tope de tokens generados ───────────────────────────────────────────
    # mlx_lm.server por defecto genera SÓLO 512 tokens si el request no trae
    # max_tokens. Para un modelo de razonamiento eso es un error garantizado:
    # la traza CoT se come el presupuesto y la respuesta llega cortada a mitad,
    # con el conclusion y sin el final. 4096 es un suelo razonable; opencode
    # manda su propio max_tokens por request, así que esto es el default.
    printf '%s\n' --max-tokens 4096

    # ── Prompt cache ──────────────────────────────────────────────────────
    # Cache de prompts YA tokenizados, por prefijo. El system prompt de un
    # agente (skills, reglas, tool schemas) es largo y no cambia entre turnos:
    # cada request repite esos miles de tokens, y el prefill es la parte cara.
    # Con esto, del segundo turno en adelante solo se prefill-ea lo nuevo.
    # Son 128 MB de RAM a cambio de nada, en una máquina con 36 GB.
    printf '%s\n' \
        --prompt-cache-size 32 \
        --prompt-cache-bytes 134217728

    # ── Prefill ───────────────────────────────────────────────────────────
    # 2048 es el default de mlx_lm. Con ctx de 65536 son 32 pasos por prompt
    # largo; a 4096 son 16, con menos overhead por paso. Sube RAM por request
    # en la misma proporción, y aquí hay de sobra.
    printf '%s\n' --prefill-step-size 4096

    # ── Pipelining ────────────────────────────────────────────────────────
    # En vez de tensor parallelism. En un Mac la GPU y la CPU comparten memoria
    # y hay UN solo device, así que el reparto por devices no tiene nada que
    # repartir; el pipelining es lo que corresponde.
    printf '%s\n' --pipeline

    printf '%s\n' --log-level INFO
}

# ── LO QUE NO HAY QUE PONER, Y POR QUÉ ───────────────────────────────────────
# · --draft-model (speculative decoding): tentador, pero aquí no compensa.
#   gpt-oss-20b es MoE con 4 de 32 expertos activos = 3.6B efectivos, y decodifica
#   rápido. Además el draft necesita EL MISMO vocabulario, y el único modelo
#   gpt-oss disponible es otro de 11 GB (no un "draft" pequeño de 1 GB). Te
#   costaría 11 GB de RAM para acelerar algo que ya va bien.
# · --trust-remote-code: los repos de mlx-community para gpt-oss no traen código
#   remoto; el tokenizer es un tokenizer.json estándar. No hace falta, y activar
#   ejecución remota de checkpoints descargados no es algo que quieras por
#   defecto.
# · --min-p: OpenAI especifica temp 1.0 / top_p 1.0 / top_k 0. Añadir min-p encima
#   es inventar una política de muestreo que el modelo no fue entrenado con.
#
# ── LO QUE mlx_lm.server NO HACE (y esto SÍ importa con gpt-oss) ─────────────
# · TOOL CALLING: el chat template de Harmony SÍ renderiza las tools (las
#   OpenAI las empaqueta en el system con <|constrain|>), pero mlx_lm.server NO
#   extrae la tool_call de la respuesta: te llega dentro de `content` con el
#   formato de Harmony. Un cliente que espere el campo `tool_calls` no lo verá.
#   Para agenteic con tools, usa llama-server con el GGUF de gpt-oss, que sí
#   parsea el reasoning y las tool calls.
# · REASONING: tampoco lo separa. El channel `analysis` de Harmony vuelve dentro
#   de `content`, mezclado con la respuesta final. Verás el razonamiento en la
#   respuesta (que es lo que OpenAI quiere para gpt-oss), pero no en un campo
#   `reasoning_content` separado.
# · REASONING_EFFORT por request: no existe. mlx_lm.server no tiene ese knob;
#   la única forma de cambiarlo es --chat-template-args en el arranque, que es
#   justo lo que hace este script. Por eso ROPERO_METALOSS_EFFORT hay que pasarlo
#   antes de arrancar, no por request.
#
# Nota sobre el template: Unsloth publica arreglos del chat template de gpt-oss
# porque los jinja estándar (que es el que usa mlx_lm) difieren de Harmony en
# detalles: los tool calls se renderizan con tojson (y se les escapan las comillas
# si llegan como string), hay saltos de línea de más en algunos bordes, y el razonamiento
# de una tool call debería ir en el canal `analysis` y no en `final`. Sus arreglos
# están en SUS subidas (GGUF/BnB/BF16), no en mlx-community. Para inferencia en
# texto plano da igual; para tool calling es justo el caso donde el otro engine gana.