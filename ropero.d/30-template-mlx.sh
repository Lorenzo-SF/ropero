#!/usr/bin/env bash
# =============================================================================
# PLANTILLA: engine mlx_lm — modelos MLX en Apple Silicon
# -----------------------------------------------------------------------------
# CÓMO USAR ESTA PLANTILLA
#
#   1. cp ropero.d/30-template-mlx.sh ropero.d/mi-modelo.sh
#   2. Edita las líneas del final (MODEL_PATH, MODEL_CTX, MODEL_PORT,
#      MODEL_FORMAT). El nombre del fichero ES el nombre del modelo.
#      El prefijo 30-template- se quita al copiar: el loader de ropero salta
#      todo lo que tenga `-template-` en el nombre.
#   3. Renombra `get_engine_args_mlx` a `get_engine_args_mi_modelo` (guiones
#      bajos en vez de guiones), o bórrala si no la usas: es opcional.
#   4. chmod +x ropero.d/mi-modelo.sh
#   5. ropero list  ·  ropero mi-modelo --background  ·  ropero stop mi-modelo
#
# -----------------------------------------------------------------------------
# ⚠️ POR QUÉ ESTE MODELO LLEVA PROXY DE AUTH
# mlx_lm.server NO tiene `--api-key`: ni flag, ni variable de entorno, ni nada
# (verificado en su --help y en mlx_lm/server.py de 0.31.3). Con HOST=0.0.0.0
# eso sería un endpoint de inferencia abierto a la LAN. Así que ropero le pone
# delante ropero.d/_authproxy.py, que sí exige el LLAMA_API_KEY de ropero, y el
# mlx_lm.server real escucha solo en 127.0.0.1 (puerto :10992, fijo por engine
# para todos los modelos MLX). No lo saltes con ROPERO_NO_AUTH_PROXY=1 salvo que
# sea 127.0.0.1 y punto.
#
# ⚠️ Y NO PUEDES RENOMBRAR EL MODELO EN EL opencode.jsonc PARA CAMBIARLO
# `mlx_lm.server` sólo acepta "default_model" en el body: su ModelProvider
# resuelve con `_model_map.get(nombre, nombre)`, y lo único registrado es
# "default_model". Cualquier otro nombre se trata como un repo_id de
# HuggingFace → 404 con "Repository Not Found", que parece un problema de
# descarga y no lo es. El proxy ya lo arregla (--rewrite-model), así que da
# igual qué nombre le pongas al modelo en el opencode.jsonc. No lo declares
# MODEL_ALIAS "gptoss"-style esperando que el engine lo atienda: el nombre lo
# elige el cliente, y el proxy lo normaliza.
# =============================================================================

# -----------------------------------------------------------------------------
# QUÉ TIENE QUE TENER EL MODELO
# -----------------------------------------------------------------------------
#
# FORMATO: pesos YA CONVERTIDOS A MLX. Un directorio con config.json,
# safetensors (o .npz en versiones antiguas) y el tokenizer. NO es un GGUF, y
# NO es un safetensors "normal" de HuggingFace: hay que convertirlo primero,
# porque el conversor hace el layout de pesos que espera el backend de MLX.
#
#   hf download <repo> --local-dir ~/models/mlx-src/<modelo>
#   mlx_lm.convert --hf-path ~/models/mlx-src/<modelo> \
#                  --mlx-path $SAFETENSORS_DIR/<modelo>
#
# Modelos ya convertidos, listos para descargar (mlx-community en HuggingFace):
#   mlx-community/Qwen3-8B-4bit        mlx-community/Llama-3.3-70B-Instruct-4bit
#   mlx-community/Mistral-7B-Instruct-v0.3-4bit
#   mlx-community/gpt-oss-20b-MXFP4-Q8
#
# ARQUITECTURAS: las de MLX, que son las de HuggingFace sin nada especial —
# Llama, Qwen2/Qwen3, Mistral, Mixtral/MoE, gpt-oss, Gemma, Phi. Lo que manda no
# es la arquitectura sino la CONVERSIÓN: si el conversor no reconoce la config,
# falla ahí y no aquí. `mlx_lm.convert` avisa de los architectures no soportados.
#
# ⚠️ VISIÓN: un modelo de visión lo sirve `mlx_vlm.server`, NO `mlx_lm.server`.
# Si lo pones aquí con mlx_lm, el server arranca y responde mal, en vez de
# decirte que te has equivocado de server. Para VLM, engine="mlx_vlm" no existe
# todavía en ropero: usa `ropero` + llama-server con el GGUF de visión, o lanza
# mlx_vlm.server a mano.
#
# -----------------------------------------------------------------------------
# POR QUÉ MLX Y NO llama.cpp CON UN GGUF, EN UN MAC
# -----------------------------------------------------------------------------
# Los dos funcionan. La comparación honesta:
#
#   GGUF + llama.cpp   más universal (el mismo modelo corre en cualquier lado),
#                     cuantizaciones más finas, mejor en CPU y en modelos que no
#                     quepan.
#   MLX + mlx_lm      más rápido en GPU de Apple, sobre todo prefill en contexto
#                     largo y con prompt cache + speculative decoding, que es lo
#                     que exploitation justo abajo.
#
# En una M4 Max con un modelo 8B-30B cuantizado, MLX gana. Si el modelo es >30B,
# el prompt cache empieza a importar más que la diferencia de backend.
# =============================================================================

MODEL_ENGINE="mlx_lm"
# Relativo a $SAFETENSORS_DIR (= ~/models/safetensors), o ruta ABSOLUTA al
# directorio de pesos MLX CONVERTIDOS.
MODEL_PATH="PON-AQUI-EL-DIRECTORIO-MLX-CONVERTIDO"
MODEL_CTX=131072

# Puerto propio. Con MODEL_PORT este modelo SIEMPRE usa este puerto, tenga o no
# --cpu. Lo habitual es pinear los mlx aquí (9992/9993) y dejar :9999/:9998
# para los llama-server, así conviven en la misma máquina.
MODEL_PORT=9992

# Sólo informativo; `ropero list` lo muestra entre corchetes.
# Valores típicos: mlx, mlx-4bit, mlx-8bit.
MODEL_FORMAT="mlx-4bit"

# -----------------------------------------------------------------------------
# FLAGS EXTRA DEL ENGINE (opcional)
# -----------------------------------------------------------------------------
# Estos son los flags que `mlx_lm.server --help` (0.31.3) acepta REALMENTE.
# No hay más: un flag inventado hace que el servidor salga con "no such option".
#
#   --model, --adapter-path, --host, --port, --allowed-origins
#   --draft-model, --num-draft-tokens      speculative decoding
#   --trust-remote-code                    solo si el tokenizer trae código
#   --log-level {DEBUG,INFO,WARNING,ERROR,CRITICAL}
#   --chat-template, --use-default-chat-template, --chat-template-args
#   --temp, --top-p, --top-k, --min-p, --max-tokens
#   --decode-concurrency, --prompt-concurrency, --prefill-step-size
#   --prompt-cache-size, --prompt-cache-bytes
#   --pipeline
#
# ropero ya pone: --model, --host, --port y MODEL_CTX como --max-tokens NO
# (ojo: `--max-tokens` es el tope de tokens GENERADOS, no el tamaño de contexto.
# El contexto en mlx_lm lo decide el modelo, no un flag).
#
# Lo que de verdad hace exploitada la GPU de Apple, por impacto:
#
#   --prompt-cache-size N      y --prompt-cache-bytes N
#       Cache de prompts ya tokenizados. Con un system prompt largo y estable
#       (el caso de un agente: las herramientas y las instrucciones no cambian
#       entre turnos), la 2ª petición y siguientes se saltan el prefill de ese
#       prefijo. Es el MAYOR salto de latencia en contexto largo, y no cuesta
#       GPU: solo RAM. Defaults: 64 entradas y 256 MB.
#
#   --draft-model <dir> --num-draft-tokens N
#       Speculative decoding: un modelo PEQUEÑO adivina N tokens y el grande los
#       verifica de una. En GPU rápida con modelo lento (~7 tok/s) el tiempo de
#       verificación se solapa con la generación y se gana entre 1.5x y 2.5x.
#       Ej: mlx-community/Qwen3-0.6B-4bit como draft de un 8B.
#       En modelos ya muy rápidos el overhead del draft se come el beneficio.
#
#   --prefill-step-size N     (2048 por defecto) subirlo reduce el overhead en
#       prompts muy largos a cambio de más RAM por request.
#
#   --decode-concurrency N / --prompt-concurrency N
#       Cuántas requests se agrupan para decodificar / prefill en paralelo. Con
#       un solo cliente (opencode) déjalo en 1: no hay con qué rellenarlo y
#       agrupar mete latencia variable.
#
#   --temp / --top-p / --min-p    defaults del sampling cuando el request no los
#       manda. OJO: opencode los manda en cada request, así que solo afectan a
#       clientes que no los especifiquen.
#
#   --pipeline                pipelining en vez de tensor parallelism. En un Mac
#       (GPU unificada) es lo que toca; el tensor parallelism es para varios
#       GPUs.
# =============================================================================

get_engine_args_mlx() {
    printf '%s\n' \
        --prompt-cache-size 64 \
        --prompt-cache-bytes 268435456 \
        --log-level INFO
}

# ── LO QUE mlx_lm.server NO HACE ─────────────────────────────────────────────
# · NO valida la API key. De ahí el proxy (ver cabecera).
# · NO es multimodal: eso es mlx_vlm.server.
# · `--max-tokens` (def. 512) corta la respuesta, no la conversación. Si te
#   parece que "se corta a mitad", mira este valor, no MODEL_CTX.