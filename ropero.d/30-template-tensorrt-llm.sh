#!/usr/bin/env bash
# =============================================================================
# PLANTILLA: engine tensorrt-llm — NVIDIA RTX en cachy/arch
# -----------------------------------------------------------------------------
# CÓMO USAR ESTA PLANTILLA
#
#   1. cp ropero.d/30-template-tensorrt-llm.sh ropero.d/mi-modelo.sh
#   2. Edita las líneas del final. El nombre del fichero ES el nombre del modelo.
#      El prefijo 30-template- se quita al copiar: el loader de ropero salta
#      todo lo que tenga `-template-` en el nombre.
#   3. Renombra `get_engine_args_tensorrt_llm` a `get_engine_args_mi_modelo`
#      (guiones bajos), o bórrala si no la usas: es opcional.
#   4. chmod +x ropero.d/mi-modelo.sh
#   5. ropero list  ·  ropero mi-modelo --background  ·  ropero stop mi-modelo
#
# -----------------------------------------------------------------------------
# ⚠️ POR QUÉ ESTE MODELO LLEVA PROXY DE AUTH
# trtllm-serve NO tiene `--api_key` (verificado en tensorrt_llm/commands/serve.py
# y en la doc de latest y de 1.3). El `api_key="tensorrt_llm"` que pone la
# documentación oficial en el cliente OpenAI es el campo que EXIGE el SDK, no una
# comprobación del servidor: el servidor no lo lee. El código anterior de ropero
# le pasaba `--api-key` y trtllm-serve habría abortado con "no such option".
#
# Con HOST=0.0.0.0 y sin auth sería un endpoint abierto a la LAN, así que ropero
# le pone delante ropero.d/_authproxy.py, que sí exige el LLAMA_API_KEY, y el
# trtllm-serve real escucha en 127.0.0.1 (:$PORT+1000).
# =============================================================================

# -----------------------------------------------------------------------------
# QUÉ TIENE QUE TENER EL MODELO
# -----------------------------------------------------------------------------
#
# FORMATO (una de estas dos, y son muy distintas en coste):
#
#   a) CHECKPOINT DE HUGGINGFACE: un directorio con safetensors + config +
#      tokenizer. trtllm-serve lo compila al arrancar, y la PRIMERA vez tarda
#      MINUTOS (a veces 10-20 con un 70B). Las siguientes, si cachea el engine,
#      son segundos. Se compila en RAM/VRAM: con 2 checkpoints giant, la
#      compilación puede dejar la VRAM sin sitio para el KV cache.
#
#   b) TENSORRT ENGINE COMPILADO: un fichero .engine (un "plan" de TensorRT) ya
#      horneado con TU GPU concreta. Arranca en segundos. El peaje: un .engine
#      NO es portable entre GPUs ni entre versiones del driver, así que hay que
#      compilarlo en la máquina que lo va a servir (o con un contenedor idéntico).
#      Un .engine hecho para sm_120 (RTX 5090/5080) no corre en sm_86.
#
# NO es un .gguf. llama.cpp no lo puede cargar y trtllm-serve no lo entiende:
# son dos mundos separados. Un error típico es dejar un GGUF de QuantFactory en
# la ruta y obtener un "no se encontró ningún archivo de pesos" que no dice que
# el problema es el formato.
#
# ARQUITECTURAS: las que tenga soporte TensorRT-LLM. En la práctica todo lo
# grande y medio: Llama 3.x, Qwen2/Qwen3 (incl. MoE 30B-A3B), gpt-oss, Mistral,
# Mixtral, DeepSeek, Gemma, Phi, Falcon. Se comprueba en
# https://nvidia.github.io/TensorRT-LLM/latest/llm-support/supported-models.html
# y mirando el log de compilación: si una arquitectura no está soportada, el
# error sale al compilar el engine, no al arrancar.
#
# -----------------------------------------------------------------------------
# NOTA SOBRE LA GPU
# TensorRT-LLM es el engine que DE VERDAD explota una NVIDIA moderna: kernels
# escritos para la arquitectura, fp8/nvfp4, in-flight batching y paged KV cache.
# En una RTX 5080/5090 deja muy por detrás a llama.cpp cuando va de throughput
# con varios clientes. Si tienes GPU NVIDIA y el modelo cabe en VRAM, este es el
# engine — no llama.cpp.
# =============================================================================

MODEL_ENGINE="tensorrt-llm"
# Relativo a $SAFETENSORS_DIR (= ~/models/safetensors), o ruta ABSOLUTA.
# Un checkpoint de HF (directorio) o un .engine compilado.
MODEL_PATH="PON-AQUI-EL-CHECKPOINT-O-EL-ENGINE"
MODEL_CTX=131072

# Puerto propio. Sin MODEL_PORT usa el slot :9999, que es donde suele estar tu
# llama-server. Pinear el TRT aparte permite tener los dos vivos a la vez.
MODEL_PORT=9993

# Sólo informativo; `ropero list` lo muestra entre corchetes.
MODEL_FORMAT="hf-safetensors"

# -----------------------------------------------------------------------------
# FLAGS EXTRA DEL ENGINE (opcional)
# -----------------------------------------------------------------------------
# ropero YA pone, y no debes repetirlos aquí (duplicarlos rompe el arranque):
#     trtllm-serve serve --model <ruta> --host <h> --port <p>
#                         --served_model_name <nombre-de-ropero>
#
# Ojo con `serve`: el comando real es `trtllm-serve serve MODEL`. La forma sin
# `serve` existe pero está deprecated. ropero ya lo pone, así que tú no.
#
# `--served_model_name` es lo que hace que /v1/models devuelva el nombre del
# modelo de ropero en vez del nombre del checkpoint. Sin eso, `ropero stop
# <modelo>` no lo reconocería y la columna MODELO del status mostraría otra cosa.
#
# Flags reales y útiles (todos de trtllm-serve):
#
#   --max_seq_len N              OJO: no es el contexto del request, es el
#                                máximo absoluto que se reservará en el KV
#                                cache. Debe CASAR con MODEL_CTX. Si te
#                                pasas, reservas VRAM de más y pierdes
#                                concurrencia por nada; si te quedas corto,
#                                requests largos se truncan o fallan.
#
#   --kv_cache_dtype fp8         el MAYOR ahorro de VRAM de la lista, y por
#                                tanto el que más concurrencia te da en la misma
#                                tarjeta. fp8_e4m3 si quieres el mínimo. Con
#                                modelos ya cuantizados a 4 bits, el KV cache en
#                                fp16 se come una parte absurda de la VRAM.
#
#   --max_batch_size N           requests simultáneos. Por defecto suele ser
#                                modesto; subirlo es lo que convierte "rápido
#                                para uno" en "rápido para varios".
#   --max_num_tokens N           tokens por iteración de decodificación; más alto
#                                = más tokens por paso, menos pasos.
#
#   --free_gpu_memory_fraction F reserva un margen de VRAM libre (0.85-0.90).
#                                Sin margen, la fragmentación hace que el KV
#                                cache falle al crecer en caliente y el server
#                                se muera a mitad de una carga.
#
#   --enable_chunked_prefill     parte los prompts largos en trozos y los
#                                intercala con la decodificación. Sube bastante
#                                el throughput cuando todo el mundo manda
#                                contextos largos (agentes, RAG). En un servidor
#                                de un solo cliente el efecto es menor.
#
#   --num_postprocess_workers N  paraleliza el detokenizado. Con MoE (gpt-oss,
#                                Qwen3-A3B) el detokenizado es un cuello de
#                                botella Surprise: se nota en CPU al 100% con
#                                la GPU ociosa.
#
#   --reasoning_parser deepseek_r1   extrae el razonamiento a content con el
#                                campo correcto. Sin esto, un modelo de
#                                razonamiento devuelve el <think> dentro del
#                                texto y el cliente lo muestra entero.
#   --tool_parser <x>            extrae tool_calls. Desactivado por defecto.
#
#   --tensor_parallel_size N     reparte el modelo entre N GPUs (una RTX = 1).
#   --context_parallel_size N    para contexto largo en 1 GPU: reparte la
#                                atención, no el modelo. Útil con --max_seq_len
#                                de 128k en una sola tarjeta.
#   --backend {pytorch,tensorrt} pytorch compila al vuelo; tensorrt usa kernels
#                                horneados. tensorrt es más rápido pero obliga
#                                a compilar antes.
#
#   --trust_remote_code          solo si el modelo trae modelling.py propio.
#   --log_level INFO             'error' para silenciar el log de compilación,
#                                que es muy verboso y tapa los errores utiles.
#   --extra_llm_api_options RUTA fichero YAML con opciones del backend.
# =============================================================================

get_engine_args_tensorrt_llm() {
    printf '%s\n' \
        --max_seq_len 131072 \
        --kv_cache_dtype fp8 \
        --free_gpu_memory_fraction 0.88 \
        --enable_chunked_prefill \
        --num_postprocess_workers 4 \
        --log_level INFO
}

# ── LO QUE trtllm-serve SÍ HACE BIEN (aprovechalo) ──────────────────────────
# · `trtllm-bench` para medir antes de decidir nada. Está linkeado en
#   ~/.local/bin junto al resto (trtllm-*), es lo que te dice si un modelo/quant
#   le gana o pierde a llama.cpp en TU tarjeta. Sin medir, es fe ciega.
# · CUDA graphs y paged KV: no hace falta tocar nada, viene por defecto.