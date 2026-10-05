#!/usr/bin/env bash
# =============================================================================
# PLANTILLA: engine airllm — modelos que NO caben en la VRAM
# -----------------------------------------------------------------------------
# CÓMO USAR ESTA PLANTILLA
#
#   1. cp ropero.d/30-template-airllm.sh ropero.d/mi-modelo.sh
#   2. Edita SOLO las 4 líneas del final (MODEL_PATH, MODEL_CTX, MODEL_PORT,
#      MODEL_FORMAT). El nombre del fichero ES el nombre del modelo:
#      `ropero mi-modelo`. El prefijo 30-template- se quita al copiar, porque
#      el loader de ropero salta todo lo que tenga `-template-` en el nombre.
#   3. Si los `get_engine_args_*` de abajo no te sirven, bórralos: son
#      opcionales. Si te sirven, renombra la función a
#      get_engine_args_mi_modelo (el nombre del fichero, sin 30-, con guiones
#      bajos en vez de guiones).
#   4. chmod +x ropero.d/mi-modelo.sh
#   5. ropero list              # debe aparecer
#      ropero mi-modelo --background
#      ropero status            # fila con engine=airllm
#      ropero stop mi-modelo
#
# -----------------------------------------------------------------------------
# ⚠️ ANTES DE NADA: airllm NO trae servidor. Este es el único engine de ropero
# cuyo servidor es NUESTRO, porque airllm es una LIBRERÍA pura: no tiene
# `__main__.py`, ni subcomando `serve`, ni un solo entry point. El comando que
# ropero tenía antes, `python -m airllm serve`, no ha existido nunca.
#
# El server es ropero.d/_airllm_server.py, linkeado en ~/.local/bin/airllm-server
# como cualquier otro ejecutable. Ya trae --api-key y --model-id, así que airllm
# NO lleva proxy de auth: publica la API key de ropero directamente.
# =============================================================================

# -----------------------------------------------------------------------------
# QUÉ TIENE QUE TENER EL MODELO
# -----------------------------------------------------------------------------
#
# FORMATO: checkpoint de HuggingFace en safetensors o pytorch_model-*.bin.
# NO un .gguf (eso es llama.cpp), NO un directorio de pesos MLX (eso es mlx_lm),
# NO un .engine de TensorRT. Es un AutoModel.from_pretrained(), con su
# config.json y su tokenizer al lado.
#
# ARQUITECTURAS. Las lee airllm/auto_model.py 2.11.0, de `config.architectures[0]`:
#
#   Qwen2ForCausalLM   → AirLLMQWen2        MistralForCausalLM → AirLLMMistral
#   QWen*              → AirLLMQWen          MixtralForCausalLM → AirLLMMixtral
#   Baichuan*          → AirLLMBaichuan      Llama*            → AirLLMLlama2
#   ChatGLM*           → AirLLMChatGLM       InternLM*         → AirLLMInternLM
#
# Cualquier otra cae en el `else` de get_module_class y se sirve COMO LLAMA2.
# Para un Qwen3 o un gpt-oss eso significa iterar por `model.layers.N` con la
# numeración de Llama: se leerán capas que no existen y la salida será ruido.
# SIEMPRE comprueba la arquitectura antes de cablear nada:
#
#   python -c "from transformers import AutoConfig; \
#              print(AutoConfig.from_pretrained('<repo>').architectures)"
#
# ⚠️ EN macOS LA COSA ES DISTINTA Y NO ES UN DETALLE:
# `AutoModel.from_pretrained` devuelve SIEMPRE `AirLLMLlamaMlx` en Darwin, sea
# cual sea la arquitectura (`if is_on_mac_os:` sin mirar el config). O sea: en
# Apple Silicon airllm SOLO funciona con modelos estilo Llama. Un Qwen3 se
# cargará sin quejarse y generará basura. En macOS, comprueba que sea Llama.
#
# -----------------------------------------------------------------------------
# POR QUÉ EXISTE airllm Y CUÁNDO TIENE SENTIDO
# -----------------------------------------------------------------------------
# AirLLM trocea el modelo en capas y las va moviendo de disco a GPU capa a capa,
# liberando cada capa después de usarla. Eso permite correr un 120B en una GPU
# de 8 GB. El precio se paga en velocidad: el techo NO es cómputo, es I/O. Cada
# tokengenerated recorre el disco del modelo entero.
#
# Presupuesto real de memoria: RAM para TODAS las capas troceadas + UNA capa en
# la GPU. Si te falta RAM, el server muerde disco y cae a tok/s de un dígito.
#
# Cuándo SÍ: el modelo no cabe en la VRAM y aceptas 2-10 tok/s.
# Cuándo NO: si tienes GPU libre para el modelo, llama-server con un GGUF
# cuantizado es 10-50x más rápido. AirLLM es el último recurso, no el primero.
# =============================================================================

MODEL_ENGINE="airllm"
# Relativo a $SAFETENSORS_DIR (= ~/models/safetensors), o ruta ABSOLUTA.
# Debe ser un DIRECTORIO con el checkpoint completo (safetensors + config +
# tokenizer), no un .gguf suelto.
MODEL_PATH="PON-AQUI-EL-DIRECTORIO-DEL-CHECKPOINT"
MODEL_CTX=131072

# Puerto propio: con MODEL_PORT este modelo SIEMPRE usa este puerto, tenga o no
# --cpu. Es lo que permite tener un airllm de 120B conviviendo con un llama-server
# en la GPU sin pelear por :9999. Sin MODEL_PORT usa los slots (:9999 / :9998).
MODEL_PORT=9997

# Sólo informativo: `ropero list` lo muestra entre corchetes para saber de un
# vistazo qué formato espera. Valores típicos: safetensors, fp16, int4.
MODEL_FORMAT="safetensors"

# -----------------------------------------------------------------------------
# FLAGS EXTRA DEL ENGINE (opcional)
# -----------------------------------------------------------------------------
# lo que imprime esta función se AÑADE al final del comando del servidor. Si no
# la declaras, no pasa nada: es opcional.
#
# Los flags que existen, verificados en _airllm_server.py:
#   --max-seq-len N        MODEL_CTX ya lo pasa como esto. NO lo repitas.
#   --max-new-tokens N     tope de tokens generados por respuesta (def. 4096)
#   --temperature F        0.0 = determinista
#   --top-p F
#   --compression {none,4bit,8bit}
#   --layer-shards RUTA    dónde se guardan las capas troceadas
#   --no-prefetch          no prefetchear la capa siguiente
#   --device cuda:0|cpu|mps
#
# Los que de verdad mueven la aguja:
#
#   --compression 4bit   ~4x más rápido a cambio de algo de precisión. Es EL
#                        flag que más impacta. Exige `pip install bitsandbytes`.
#                        `8bit` es el punto dulce si 4bit se nota demasiado.
#
#   --layer-shards RUTA  AirLLM guarda las capas troceadas en el caché del
#                        modelo, que en macOS está en el volumen del sistema
#                        (SSD lento, a veces APFS con escritura coalescida). Si
#                        tienes un SSD externo o un volumen rápido, apúntalo
#                        ahí: cambia los tok/s más que cualquier otro flag, y
#                        de golpe. La primera vez DESCOMPRIME el modelo entero
#                        ahí (minutos, y el disco se lee entero).
#
#   --no-prefetch        Solo si el disco va justo de RAM. Por defecto
#                        prefetching=True: mientras infiere capa N, va leyendo
#                        la N+1. Desactivarlo reduce concurrencia de disco, que
#                        es justo el recurso escaso. No lo toques salvo que sepas
#                        que te falta RAM.
#
#   --device mps         En Apple Silicon, MPS en vez de CPU. Con
#                        AirLLMLlamaMlx es lo que usa por defecto; se pone
#                        explícito para dejarlo escrito.
# =============================================================================

get_engine_args_airllm() {
    printf '%s\n' \
        --compression 4bit \
        --max-new-tokens 4096 \
        --layer-shards "$HOME/models/airllm-shards"
}

# ── LO QUE ESTE SERVER NO HACE (para que no te pille de sorpresa) ───────────
# · tool calling: si el request trae `tools`, responde 400 con el motivo. Un
#   endpoint que aceptara las tools y contestara texto plano dejaría al
#   cliente colgado esperando un tool_call que no llega.
# · Una request a la vez (hay un lock global). Dos en paralelo no dan el doble
#   de velocidad: reparten el mismo I/O y duplican el swap.
# · embeddings: no es un endpoint de embeddings.