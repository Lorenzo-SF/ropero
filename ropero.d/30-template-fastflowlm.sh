#!/usr/bin/env bash
# =============================================================================
# PLANTILLA: engine fastflowlm — la NPU AMD (XDNA) de los Ryzen AI
# -----------------------------------------------------------------------------
# ⚠️ ESTE MOTOR SOLO EXISTE EN DOS SITUACIONES A LA VEZ:
#    (1) cachy/arch, y (2) una CPU AMD con NPU XDNA2 (Ryzen AI: Strix, Strix
#    Halo, Kraken, Gorgon Point). En macOS no hay NPU AMD, y en una CPU sin NPU
#    no hay nada que instalar. `ropero --install` lo detecta solo y se salta.
#
# CÓMO USAR ESTA PLANTILLA
#
#   1. cp ropero.d/30-template-fastflowlm.sh ropero.d/mi-modelo.sh
#   2. Edita las líneas del final (sobre todo MODEL_PATH, que aquí es un TAG de
#      FLM, no una ruta). El nombre del fichero ES el nombre del modelo. El
#      prefijo 30-template- se quita al copiar: el loader salta *-template-*.
#   3. Renombra `get_engine_args_fastflowlm` a `get_engine_args_mi_modelo`, o
#      bórrala si no la usas: es opcional.
#   4. chmod +x ropero.d/mi-modelo.sh
#   5. flm pull <tag>            # descarga los kernels optimizados para la NPU
#      ropero list
#      ropero mi-modelo --background
#      ropero stop mi-modelo
#
# -----------------------------------------------------------------------------
# POR QUÉ UNA NPU EN UNA MÁQUINA QUE YA TIENE RTX
# -----------------------------------------------------------------------------
# Porque la GPU y la NPU son RECURSOS DISTINTOS, y no compiten. El patrón que
# sale bien es: el modelo PEQUEÑO y el de más tráfico (embeddings, el clasificador,
# el reformateador, un 1B-4B para lo simple) en la NPU, y el grande en la GPU.
# Los dos a la vez, sin que ninguno espere al otro. Además la NPU consume mucho
# menos: FLM es ~10x más eficiente en energía que la GPU para lo que hace.
#
# -----------------------------------------------------------------------------
# ⚠️ POR QUÉ ESTE MODELO LLEVA PROXY DE AUTH
# `flm serve` no tiene auth. Su propia documentación lo dice: "No real API key is
# needed — just pass \"flm\" as a placeholder" y "the key is not checked". Con
# HOST=0.0.0.0 sería un endpoint abierto a la LAN, así que ropero le pone delante
# ropero.d/_authproxy.py, que sí exige el LLAMA_API_KEY, y `flm serve` real
# escucha en 127.0.0.1 (:$PORT+1000). Además ropero pasa `--cors 0`: el proxy es
# el que ve al exterior y un endpoint de inferencia no necesita cross-origin.
# =============================================================================

# -----------------------------------------------------------------------------
# QUÉ TIENE QUE TENER EL MODELO
# -----------------------------------------------------------------------------
#
# ⚠️ MODEL_PATH NO ES UNA RUTA. Es un TAG de FastFlowLM, con su formato de
# etiqueta de tipo Ollama: "llama3.2:1b", "qwen3:4b", "deepseek-r1:1.5b"…
#
# Los pesos NO se bajan de HuggingFace con `hf download`. `flm pull <tag>` se
# baja además los KERNELS optimizados para la NPU, y sin ellos no arranca:
#
#   flm pull llama3.2:1b
#   flm pull llama3.2:1b --force    # si la descarga quedó corrupta (pasa)
#   flm list                        # los que ya tienes
#
# Viven en ~/.config/flm/ (o $FLM_MODEL_PATH/models/). ropero comprueba que el
# tag esté ahí antes de arrancar y, si no, te dice exactamente el `flm pull` que
# falta.
#
# Tags con kernels listos para NPU (familia y tamaño; el catálogo completo y los
# ctx por defecto están en https://fastflowlm.com/docs/models/):
#   llama3.2:1b   qwen3:0.6b   qwen3:1.7b   qwen3:4b   qwen3:8b
#   gemma3:1b     gemma3:4b    deepseek-r1:1.5b   deepseek-r1:7b
#   lfm2:1.2b     phi4:14b      gpt-oss:20b    qwen3.5:*
#
# ARQUITECTURAS: FLM trae sus propios kernels por familia (no compila kernels
# genéricos). Si pides un tag que FLM no soporta, el error sale en `flm pull`, no
# al arrancar el server. Un tag que sí te interesa pero no está en su catálogo
# no se puede forzar.
#
# -----------------------------------------------------------------------------
# REQUISITOS DEL STACK (los deja `ropero --install`, pero conviene saberlos)
# -----------------------------------------------------------------------------
#   · driver amdxdna     — kernel 7.0+ lo trae, o amdxdna-dkms (AUR) para 6.19
#   · xrt + xrt-plugin-amdxdna
#   · firmware NPU >= 1.1.0.0 (1.1.2.64 en kernels 6.19)
#   · memlock ilimitado  — * soft/hard memlock unlimited en limits.conf
#
# LA TRAMPA QUE MÁS COSTA TIEMPO:
# `flm validate` usa el DRM device (/dev/accel/accel0) y `flm run` usa XRT
# (xrt::device(0)). Que validate PASE NO GARANTIZA que run funcione: el síntoma es
# "No such device with index '0'" con un validate en verde. Para descartar esa
# capa:
#
#   xrt-smi examine       # debe listar la NPU en la tabla de dispositivos
#
# `ropero --check` llama a `flm validate` y, si falla, dice cuál de las cuatro
# causas es la más probable.
# =============================================================================

MODEL_ENGINE="fastflowlm"
# Un TAG de FLM, NO una ruta. Ejemplos: llama3.2:1b · qwen3:4b · gpt-oss:20b
# Se descarga con: flm pull <tag>
MODEL_PATH="llama3.2:1b"

# Contexto. ropero lo pasa como `--ctx-len`, y FLM lo ajusta solo a potencia de
# dos (mínimo 512). El contexto por defecto de cada tag está en la doc de FLM:
# los tags pequeños suelen ir de 4k a 32k, y algunos (Qwen3-4B-Thinking-2507)
# llegan a 256k. Pon el que de verdad vas a usar: cada salto de potencia de dos
# reserva más KV en la NPU.
MODEL_CTX=8192

# Puerto propio. Sin MODEL_PORT usaría el slot :9999, que es donde probablemente
# esté tu llama-server en la RTX. Pinear el FLM aparte (9994) permite que la NPU
# y la GPU sirvan a la vez, que es justo el caso de uso de este engine.
MODEL_PORT=9994

# Sólo informativo; `ropero list` lo muestra entre corchetes.
MODEL_FORMAT="flm-tag"

# -----------------------------------------------------------------------------
# FLAGS EXTRA DEL ENGINE (opcional)
# -----------------------------------------------------------------------------
# Estos son los flags que `flm serve` acepta REALMENTE. ropero YA pone, y no
# debes repetirlos:
#     flm serve <tag> --port <p> --ctx-len <ctx> --cors 0
#
# Lo que queda:
#
#   -q N / --q-len N      cola de peticiones (default 10). Sin esto, con varios
#                         clientes, FLM rechaza o encola a su manera las
#                         peticiones que exceden la cola y el cliente se cuelga
#                         esperando. 20-50 si hay más de un consumidor.
#
#   -s N / --socket N     conexiones de socket simultáneas (default 10). La
#                         propia doc de FLM recomienda ponerlo IGUAL O MAYOR que
#                         la cola: si hay más conexiones que sockets, el cliente
#                         espera en el accept() en vez de en la cola, y no puedes
#                         verlo en ningún log.
#
#   --quiet               silencia el log de FLM. Útil si lo lanzas desde otro
#                         proceso (que es el caso: ropero) y el log se te
#                         mezcla con el del proxy.
#
#   -e 1                  carga además el modelo de EMBEDDINGS. La NPU puede
#                         tener un LLM y un de embeddings cargados A LA VEZ (son
#                         tipos distintos), pero solo UNO de cada tipo a la vez.
#                         SÍ te interesa si tienes graphify o RAG: los embeddings
#                         en la NPU no tocan la GPU.
#   -a 1                  carga además el modelo de ASR/audio (Whisper). Por lo
#                         mismo: convive con el LLM, pero solo uno de ASR.
#
# OJO CON LOS MODELOS "FLASH" (tags `*-flash:*`):
#   · son single-turn: el server es STATELESS, cada request empieza de cero. No
#     reenvíes el historial (se descarta y solo pagas el prefill de sobra).
#   · contexto FIJO de 1k tokens: ignoran `--ctx-len` por completo.
#   · NO soportan tool calling: las `tools` se descartan en silencio.
#   · el system prompt se prefill-ea una vez y se cachea. Mantenlo FIJO y varía
#     solo el mensaje de usuario: alternar entre dos system prompts tira la
#     caché y rehaces el prefill cada vez, que es justo lo que mata el diseño.
# =============================================================================

get_engine_args_fastflowlm() {
    printf '%s\n' \
        --q-len 20 \
        --socket 20 \
        --quiet
}

# ── LO QUE flm serve SÍ Y NO HACE ────────────────────────────────────────────
# · Endpoints que sirve: v1/models, v1/chat/completions, v1/embeddings,
#   v1/audio/transcriptions. Los que no implemente, el proxy los reenvía tal cual
#   y el engine contesta 404 con su propio formato — que es lo correcto.
# · Tool calling: NO. Los modelos flash además lo descartan en silencio.
# · El server es stateless entre requests: no hay KV cache entre turnos (la
#   CLI `flm run` sí la tiene). En multi-turno, reenvía siempre la conversación
#   completa y paga el prefill entero cada turno.
# · Un modelo por tipo: llm, embedding, asr.Uno de cada uno, todos a la vez.