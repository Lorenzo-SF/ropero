#!/usr/bin/env python3
# =============================================================================
# ropero.d/_airllm_server.py — el `airllm serve` que airllm NO trae
# =============================================================================
# POR QUÉ EXISTE
#
# ropero --install instalaba airllm y luego `start_model` arrancaba
# `python -m airllm serve`. airllm 2.11.0 no tiene `__main__.py` ni ningún
# subcomando `serve`; el paquete es una LIBRERÍA pura (verificado: ni un solo
# entry point en su dist-info, y ningún módulo con FastAPI/uvicorn dentro). Ese
# comando no ha existido nunca.
#
# AirLLM trae su propio ejemplo de FastAPI en el README, pero no como paquete
# instalable. Así que este fichero es ese ejemplo, hecho servidor de verdad y
# con el contrato que ropero necesita:
#
#   · `--api-key`     → valida el mismo LLAMA_API_KEY que llama-server. Con esto
#                       airllm NO necesita el proxy de _authproxy.py: es el
#                       único engine no-llama con auth propia.
#   · `--model-id`    → lo que devuelve /v1/models, para que `ropero stop <modelo>`
#                       y la columna MODELO del status funcionen igual que con
#                       llama-server, sin depender de cómo se llame el modelo en
#                       HuggingFace.
#   · puerto y host   → los mismos de ropero.
#
# IMPLEMENTACIÓN: FastAPI + uvicorn (un solo worker). Antes era http.server
# puro de la stdlib (BaseHTTPRequestHandler/ThreadingHTTPServer); se migró
# para tener async real, streaming SSE sobre ASGI y el log de acceso estándar
# de uvicorn (ver sección LOGGING más abajo). Mantiene la compatibilidad de los
# endpoints de API soportados; no pretende preservar el comportamiento
# implícito de métodos HTTP no implementados.
#
# QUÉ TIENE QUE TENER EL MODELO (esto es lo que decide si esto sirve)
#
#   · Formato:  checkpoint de HuggingFace en safetensors o pytorch_model*.bin
#               (NO gguf, NO mlx, NO un engine de tensorrt). Es un
#               AutoModel.from_pretrained(), no un loader de formato.
#   · Arquitecturas soportadas — leídas de airllm/auto_model.py 2.11.0:
#       Qwen2ForCausalLM  → AirLLMQWen2      MistralForCausalLM → AirLLMMistral
#       QWen*             → AirLLMQWen        MixtralForCausalLM → AirLLMMixtral
#       Baichuan*         → AirLLMBaichuan    Llama*            → AirLLMLlama2
#       ChatGLM*          → AirLLMChatGLM     InternLM*         → AirLLMInternLM
#     Cualquier otra cae en el `else` de get_module_class y se sirve como
#     Llama2 — que para un Qwen3 o un gpt-oss significa.peek layers que no
#     existen. Comprueba `config.architectures[0]` antes de cablearlo.
#   · ⚠️ EN macOS la cosa cambia: `AutoModel.from_pretrained` devuelve SIEMPRE
#     AirLLMLlamaMlx, sea cual sea la arquitectura (if is_on_mac_os, sin mirar
#     el config). Es decir: en Apple Silicon airllm SOLO funciona con modelos
#     estilo Llama. Un Qwen3 en macOS se cargará y dará basura.
#   · Tamaño: airllm no cuantiza para meter el modelo entero en la VRAM; lo
#     parte en capas y las va moviendo de disco a GPU. Por eso puede correr un
#     120B en una GPU de 8 GB — y por eso es LENTO (I/O, no cómputo).
#     El presupuesto real es RAM (todas las capas en disco) + una capa en GPU.
#
# CÓMO APROVECHAR LOS RECURSOS (los flags que de verdad mueven la aguja)
#
#   --compression 4bit   4x más rápido a cambio de algo de precisión. Requiere
#                        bitsandbytes. `8bit` es el punto dulce si molesta.
#   --max-seq-len N      OBLIGATORIO ponerlo. El default de airllm es 512
#                        (verificado en la firma de __init__), así que sin esto
#                        se te corta el contexto a 512 tokens por mucho que
#                        ropero pida 128k.
#   --layer-shards PATH  Dónde se guardan las capas troceadas. Por defecto, el
#                        caché del modelo — que en macOS está en el volumen del
#                        sistema, con el disco lento. Apuntar a un SSD externo o
#                        al volumen rápido cambia los tok/s más que cualquier
#                        otro flag.
#   --no-prefetch        Deja de ADVANTAR la carga de la siguiente capa. Útil
#                        solo si el disco va justo de RAM; por defecto
#                        prefetching=True porque es lo que gana.
#
# DEPENDENCIAS NUEVAS (no las instala este fichero: las instala quien gestione
# el entorno de airllm, vía `uv pip install fastapi uvicorn` o `pip install
# fastapi uvicorn`). Si no están, `main()` sale con mensaje claro a stderr y
# rc=2 ANTES de tocar el modelo — ver sección "SI FALTAN FASTAPI/UVICORN".
#
# LOGGING — formato de las líneas que emite el server, para quien le haga un
# colorize (patrones a buscar; todo va a stderr salvo que se indique otra
# cosa):
#
#   · Arranque de uvicorn (logger "uvicorn.error", formato por defecto
#     "INFO:     <mensaje>"):
#       INFO:     Started server process [<pid>]
#       INFO:     Waiting for application startup.
#       INFO:     Application startup complete.
#       INFO:     Uvicorn running on http://<host>:<port> (Press CTRL+C to quit)
#   · Apagado (SIGTERM — ropero manda SIGTERM y espera 10s, ver `ropero.d`):
#       INFO:     Shutting down
#       INFO:     Waiting for application shutdown.
#       INFO:     Application shutdown complete.
#       INFO:     Finished server process [<pid>]
#   · Acceso (logger "uvicorn.access", uno por request, access_log=True):
#       INFO:     127.0.0.1:PUERTO - "GET /v1/models HTTP/1.1" 200 OK
#       INFO:     127.0.0.1:PUERTO - "POST /v1/chat/completions HTTP/1.1" 200 OK
#   · Carga del modelo y errores — las emite ESTE módulo con `log()`, formato
#     "[airllm-server HH:MM:SS] <mensaje>":
#       [airllm-server 12:34:56] cargando /ruta (compression=none max_seq_len=32768 prefetch=True)
#       [airllm-server 12:34:56] modelo listo en 12.3s (contrato HF=True)
#       [airllm-server 12:34:56] architectures: Qwen2ForCausalLM
#       [airllm-server 12:34:56] AVISO: macOS → airllm usa SIEMPRE AirLLMLlamaMlx...
#       [airllm-server 12:34:56] ERROR: el modelo no existe: /ruta
#       [airllm-server 12:34:56] ERROR al cargar el modelo: ModuleNotFoundError: ...
#       [airllm-server 12:34:56] error generando: ValueError(...)
#       [airllm-server 12:34:56] error en stream: ValueError(...)
#       [airllm-server 12:34:56] OpenAI-compatible en http://host:port/v1 (model=..., auth=on)
#
# USO
#   airllm-server --model ~/models/safetensors/gpt-oss-120b \
#                  --model-id airgptoss --host 0.0.0.0 --port 9999 \
#                  --max-seq-len 131072 --compression 4bit --api-key sk-...
# =============================================================================

import argparse
import asyncio
import json
import os
import queue
import sys
import threading
import time
import uuid

# -----------------------------------------------------------------------------
# SI FALTAN FASTAPI/UVICORN
# -----------------------------------------------------------------------------
# airllm es una librería; FastAPI y uvicorn son dependencias NUEVAS de este
# server, no de ropero ni de airllm. Si no están instaladas en el intérprete
# que ropero usa para airllm (ver ropero_python_bin en _engines.sh), hay que
# fallar con un mensaje claro ANTES de intentar cargar el modelo — un
# ImportError sin más, 2 minutos después de haber cargado 60GB de pesos en
# RAM, es la peor forma de enterarse de que falta un `pip install`.
# -----------------------------------------------------------------------------
try:
    import uvicorn
    from fastapi import FastAPI, Request
    from fastapi.responses import JSONResponse, StreamingResponse

    _IMPORT_ERROR: Exception | None = None
except ImportError as _e:  # noqa: N818 — se usa como dato, no se relanza aquí
    uvicorn = None  # type: ignore[assignment]
    FastAPI = None  # type: ignore[assignment,misc]
    Request = None  # type: ignore[assignment,misc]
    JSONResponse = None  # type: ignore[assignment,misc]
    StreamingResponse = None  # type: ignore[assignment,misc]
    _IMPORT_ERROR = _e


# Un lock global y no uno por request: airllm mueve el modelo entero capa a
# capa por la GPU/RAM. Dos generaciones a la vez no es "más rápido", es
# "el mismo throughput repartido y el doble de swap". Es un threading.Lock
# (no un asyncio.Lock) a propósito: la generación real corre en un hilo aparte
# (ver _locked_generate / _stream_worker), nunca en el loop de asyncio.
_LOCK = threading.Lock()

_MODEL = None
_TOKENIZER = None
_HF_STYLE = True     # True → GenerationMixin (HF contract). False → AirLLMLlamaMlx
_ARGS = None


def log(msg: str) -> None:
    sys.stderr.write("[airllm-server %s] %s\n" % (time.strftime("%H:%M:%S"), msg))
    sys.stderr.flush()


# -----------------------------------------------------------------------------
# Carga
# -----------------------------------------------------------------------------
def load_backend() -> None:
    """Carga el modelo una vez al arrancar.

    Se hace en el arranque y no en el primer request a propósito: si fallase,
    es en el arranque donde quieres enterarte, con el stack completo, y no
    tres minutos después con un opencode esperando el primer token. Por eso
    `main()` llama a esto ANTES de arrancar uvicorn: el puerto no se abre
    hasta que el modelo está listo, exactamente igual que con el server
    anterior basado en http.server.
    """
    global _MODEL, _TOKENIZER, _HF_STYLE

    a = _ARGS
    kwargs = {
        "max_seq_len": a.max_seq_len,
        "prefetching": not a.no_prefetch,
    }
    if a.compression != "none":
        kwargs["compression"] = a.compression
    if a.layer_shards:
        kwargs["layer_shards_saving_path"] = a.layer_shards
    if a.device:
        kwargs["device"] = a.device
    if os.environ.get("HF_TOKEN"):
        kwargs["hf_token"] = os.environ["HF_TOKEN"]

    log("cargando %s (compression=%s max_seq_len=%d prefetch=%s)"
        % (a.model, a.compression, a.max_seq_len, kwargs["prefetching"]))
    t0 = time.time()

    from airllm import AutoModel

    model = AutoModel.from_pretrained(a.model, **kwargs)
    _MODEL = model

    # El discriminador entre los dos contratos de generate():
    #   · AirLLMLlamaMlx (macOS) tiene generate(texto) -> texto. NO es un
    #     GenerationMixin y su entrada es un string, no input_ids.
    #   · AirLLMBaseModel (CUDA) sí es GenerationMixin y habla el contrato de
    #     HuggingFace.
    try:
        from transformers import GenerationMixin
        _HF_STYLE = isinstance(model, GenerationMixin)
    except Exception:
        _HF_STYLE = hasattr(model, "input_embeddings")

    tok = getattr(model, "tokenizer", None)
    if tok is None:
        from transformers import AutoTokenizer
        tok = AutoTokenizer.from_pretrained(a.model, trust_remote_code=True)
    _TOKENIZER = tok

    log("modelo listo en %.1fs (contrato HF=%s)" % (time.time() - t0, _HF_STYLE))

    # La arquitectura real, para el log de arranque: es el dato que hace falta
    # cuando airllm cae al `else` de get_module_class y sirve basura.
    try:
        cfg = model.config
        archs = getattr(cfg, "architectures", None) or ["?"]
        log("architectures: %s" % ", ".join(archs))
        if not _HF_STYLE:
            log("AVISO: macOS → airllm usa SIEMPRE AirLLMLlamaMlx. Este modelo "
                "solo funciona si su arquitectura es estilo Llama.")
    except Exception:
        pass


# -----------------------------------------------------------------------------
# Inferencia
# -----------------------------------------------------------------------------
def _sampling(a):
    return {
        "max_new_tokens": a.max_new_tokens,
        "temperature": a.temperature,
        "top_p": a.top_p,
    }


def _render_prompt(messages):
    """Chat → texto, por el chat template del tokenizer.

    Es lo que hace que el modelo entienda de qué turno va la conversación. Sin
    template, a un modelo afinado para chat le llega la lista de mensajes cruda
    y contesta como si fuera un documento, no como un asistente.
    """
    try:
        return _TOKENIZER.apply_chat_template(
            messages, add_generation_prompt=True, tokenize=False
        )
    except Exception as e:
        log("el chat template falló (%s); uso concatenación plana" % e)
        out = []
        for m in messages:
            out.append("%s: %s" % (m.get("role", "user"), m.get("content", "")))
        out.append("assistant:")
        return "\n".join(out)


def generate_text(messages, a):
    """Generación sin streaming. Devuelve (texto_completo, n_tokens_aprox)."""
    prompt = _render_prompt(messages)

    if not _HF_STYLE:
        # Contrato de AirLLMLlamaMlx: texto entra, texto sale.
        return _MODEL.generate(prompt, **_sampling(a)), 0

    ids = _TOKENIZER.apply_chat_template(
        messages, add_generation_prompt=True, return_tensors="pt", return_dict=True
    )
    dev = getattr(_MODEL, "device", None)
    if dev is not None:
        ids = {k: v.to(dev) for k, v in ids.items()}
    n_in = ids["input_ids"].shape[-1]

    out = _MODEL.generate(**ids, do_sample=a.temperature > 0, **_sampling(a))
    # out[0] es la secuencia COMPLETA (prompt + generado): si se decodifica
    # entera, el cliente recibe su propia pregunta como respuesta.
    gen = out[0][n_in:]
    text = _TOKENIZER.decode(gen, skip_special_tokens=True)
    return text, int(gen.shape[-1])


def _locked_generate(messages, a):
    """generate_text bajo _LOCK, para llamar desde asyncio.to_thread."""
    with _LOCK:
        return generate_text(messages, a)


def stream_text(messages, a):
    """Generador de tokens, para `stream: true`.

    Los dos caminos tienen equivalente de streaming, pero por mecanismos
    distintos, y no son intercambiables:
      · HF:  TextIteratorStreamer + hilo aparte. Necesario porque generate()
        es bloqueante y bloquearía el server entero.
      · MLX: model_generate() ya es un generador; se consume directamente.
    """
    if not _HF_STYLE:
        prompt = _render_prompt(messages)
        n = 0
        for tok in _MODEL.model_generate(prompt, temperature=a.temperature):
            # model_generate puede dar un tensor de MLX o un int ya resuelto.
            # AirLLMLlamaMlx hace `t.item()` antes de decodificar, así que
            # llega como int; el isinstance cubre el otro caso sin romperse.
            if hasattr(tok, "item"):
                tok = tok.item()
            piece = _TOKENIZER.decode([int(tok)], skip_special_tokens=True)
            if piece:
                yield piece
            n += 1
            if n >= a.max_new_tokens:
                break
        return

    from transformers import TextIteratorStreamer

    ids = _TOKENIZER.apply_chat_template(
        messages, add_generation_prompt=True, return_tensors="pt", return_dict=True
    )
    dev = getattr(_MODEL, "device", None)
    if dev is not None:
        ids = {k: v.to(dev) for k, v in ids.items()}

    streamer = TextIteratorStreamer(
        _TOKENIZER, skip_prompt=True, skip_special_tokens=True
    )
    kwargs = _sampling(a)
    kwargs["streamer"] = streamer
    kwargs["do_sample"] = a.temperature > 0

    t = threading.Thread(target=_MODEL.generate, kwargs=dict(ids, **kwargs),
                         daemon=True)
    t.start()
    for piece in streamer:
        yield piece
    t.join()


# -----------------------------------------------------------------------------
# Puente generador-síncrono → asíncrono, para el streaming SSE
# -----------------------------------------------------------------------------
# stream_text() es un generador BLOQUEANTE (HF y MLX). FastAPI/uvicorn corren
# sobre asyncio: iterarlo directamente dentro de una ruta `async def` bloquea
# el loop entero y mata la concurrencia (incluido /health mientras se genera).
# La alternativa de "ejecutarlo entero en un hilo y devolver la lista" no vale:
# eso convierte el streaming en "espera todo y vuelca todo", que es justo lo
# que `stream: true` promete NO hacer.
#
# Por eso stream_text() se consume en un threading.Thread propio (no en el
# executor de asyncio.to_thread, para no competir por ese pool) y las piezas
# se entregan a través de una queue.Queue clásica; el lado async solo hace
# `await asyncio.to_thread(q.get)` por cada pieza, que ocupa un hilo del
# executor brevemente y punto. El orden de eventos replica EXACTAMENTE el del
# servidor anterior: piezas → chunk de cierre (solo si no hubo error) →
# objeto de error (solo si lo hubo) → siempre un {"done": true} final.
# -----------------------------------------------------------------------------
def _stream_worker(messages, a) -> "queue.Queue":
    q: "queue.Queue" = queue.Queue()

    def run():
        try:
            with _LOCK:
                for piece in stream_text(messages, a):
                    q.put(("piece", piece))
            q.put(("finished", None))
        except Exception as e:  # noqa: BLE001 — se reporta al cliente
            q.put(("error", e))

    threading.Thread(target=run, daemon=True).start()
    return q


# -----------------------------------------------------------------------------
# HTTP
# -----------------------------------------------------------------------------
def _msg_text(content):
    """Aplanar el content de un mensaje, que en VLM puede ser una lista de
    partes {type: text} / {type: image_url}. Un endpoint de texto que recibe eso
    y hace str() produce '{"type": "text"...}' dentro del prompt."""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        out = []
        for part in content:
            if isinstance(part, dict):
                if part.get("type") == "text":
                    out.append(part.get("text", ""))
                elif part.get("type") == "image_url":
                    out.append("[imagen]")
            else:
                out.append(str(part))
        return "\n".join(x for x in out if x)
    return str(content)


def _err(code: int, message: str, type_: str | None = None) -> "JSONResponse":
    body = {"error": {"message": message}}
    if type_:
        body["error"]["type"] = type_
    return JSONResponse(body, status_code=code)


def _authed(request: "Request") -> bool:
    want = _ARGS.api_key
    if not want:
        return True
    h = request.headers.get("Authorization", "")
    return h.startswith("Bearer ") and h[7:].strip() == want


async def _read_body(request: "Request"):
    """Devuelve (payload, error).

    Las rutas de OpenAI aceptan únicamente un objeto JSON como body. Separar
    aquí JSON inválido de un objeto vacío evita que una forma de request normal
    llegue a ``.get()`` y termine como un 500.
    """
    raw = await request.body()
    if not raw:
        return {}, None
    try:
        payload = json.loads(raw)
    except ValueError:
        return None, "JSON inválido"
    if not isinstance(payload, dict):
        return None, "el body JSON debe ser un objeto"
    return payload, None


def _normalise_messages(value):
    """Convierte mensajes OpenAI estructurados al formato interno o devuelve None."""
    if not isinstance(value, list):
        return None
    messages = []
    for message in value:
        if not isinstance(message, dict):
            return None
        role = message.get("role")
        content = message.get("content")
        if not isinstance(role, str) or not role or not isinstance(content, (str, list)):
            return None
        messages.append({"role": role, "content": _msg_text(content)})
    return messages


def _normalise_responses_input(value):
    """Normaliza el subconjunto compatible de ``/v1/responses``.

    Este servidor histórico trata responses como un alias de chat, no como la
    Response API completa. Se admite el input string habitual y una lista de
    mensajes ``{role, content}``; el resultado sigue siendo chat.completion.
    """
    if isinstance(value, str):
        return [{"role": "user", "content": value}]
    return _normalise_messages(value)


async def _chat(messages, payload):
    """El chat de verdad: valida, decide streaming o no, y responde.

    Función compartida por /v1/completions, /v1/chat/completions y
    /v1/responses — igual que en el servidor anterior.
    """
    if not messages:
        return _err(400, "messages vacío")
    if payload.get("tools"):
        # airllm no tiene template de herramientas: un endpoint de texto que
        # finge soportarlas hace que el cliente se cuelgue esperando un
        # tool_call que no va a llegar. Se dice explícitamente.
        return _err(400,
            "airllm no implementa tool calling (no es un motor "
            "de inferencia, es un offloader de capas). Las tools "
            "del request se ignorarían y el cliente colgaría.")

    cid = "chatcmpl-" + uuid.uuid4().hex[:24]
    created = int(time.time())
    model_id = _ARGS.model_id
    want_stream = bool(payload.get("stream"))

    if want_stream:
        return StreamingResponse(
            _sse(cid, created, model_id, messages),
            media_type="text/event-stream",
            headers={"Cache-Control": "no-cache"},
        )

    try:
        text, ntok = await asyncio.to_thread(_locked_generate, messages, _ARGS)
    except Exception as e:  # noqa: BLE001 — al cliente le sirve el motivo
        log("error generando: %r" % (e,))
        return _err(500, "%s: %s" % (type(e).__name__, e), type_="server_error")

    return {
        "id": cid,
        "object": "chat.completion",
        "created": created,
        "model": model_id,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": text},
            "finish_reason": "stop",
        }],
        "usage": {
            "prompt_tokens": 0,
            "completion_tokens": ntok,
            "total_tokens": ntok,
        },
    }


async def _sse(cid, created, model_id, messages):
    def event(obj) -> bytes:
        return ("data: %s\n\n" % json.dumps(obj)).encode()

    q = _stream_worker(messages, _ARGS)
    try:
        while True:
            kind, val = await asyncio.to_thread(q.get)
            if kind == "piece":
                yield event({
                    "id": cid, "object": "chat.completion.chunk",
                    "created": created, "model": model_id,
                    "choices": [{"index": 0,
                                 "delta": {"content": val},
                                 "finish_reason": None}],
                })
            elif kind == "finished":
                yield event({
                    "id": cid, "object": "chat.completion.chunk",
                    "created": created, "model": model_id,
                    "choices": [{"index": 0, "delta": {},
                                 "finish_reason": "stop"}],
                })
                break
            elif kind == "error":
                log("error en stream: %r" % (val,))
                yield event({"error": {"message": "%s: %s" % (type(val).__name__, val),
                                        "type": "server_error"}})
                break
    finally:
        yield event({"done": True})


def create_app() -> "FastAPI":
    """Construye la app ASGI. Solo se llama si fastapi/uvicorn importan.

    Starlette resuelve las rutas en el ORDEN en que se registran (no por
    especificidad), así que las rutas literales van primero y los catch-all
    (`/{full_path:path}`) se registran al final: si fueran antes, se
    comerían toda petición, incluidas /v1/models o /ropero/engine, y las
    rutas literales nunca se alcanzarían.
    """
    app = FastAPI(title="airllm-server", docs_url=None, redoc_url=None)

    @app.get("/health")
    async def health():
        return {"status": "ok", "engine": "airllm"}

    @app.get("/v1/models")
    async def list_models(request: Request):
        if not _authed(request):
            return _err(401, "invalid api key")
        return {
            "object": "list",
            "data": [{
                "id": _ARGS.model_id,
                "object": "model",
                "created": int(os.path.getmtime(_ARGS.model)
                               if os.path.exists(_ARGS.model) else time.time()),
                "owned_by": "ropero/airllm",
            }],
        }

    @app.get("/ropero/engine")
    async def ropero_engine(request: Request):
        # Ruta de diagnóstico para `_collect_slots`: sin ella, la columna
        # ENGINE del status tendría que deducir el engine del nombre del
        # proceso, que aquí es un `python3` y no dice nada. Los engines sin
        # auth propia (mlx_lm, trtllm-serve, flm) la tienen a través del
        # proxy; airllm sirve en el puerto de ropero directamente, así que
        # la publica él mismo. Sin auth a propósito: es local y de
        # diagnóstico, igual que antes.
        host, port = request.scope.get("server") or (_ARGS.host, _ARGS.port)
        return {
            "engine": "airllm",
            "model": _ARGS.model_id,
            "target_host": host,
            "target_port": port,
            "child_pid": 0,
            "child_alive": True,
        }

    @app.post("/v1/completions")
    async def completions(request: Request):
        if not _authed(request):
            return _err(401, "invalid api key")
        payload, error = await _read_body(request)
        if error:
            return _err(400, error, type_="invalid_request_error")
        prompt = payload.get("prompt", "")
        if isinstance(prompt, list):
            prompt = prompt[0] if prompt else ""
        messages = [{"role": "user", "content": _msg_text(prompt)}]
        return await _chat(messages, payload)

    async def _chat_like(request: Request):
        if not _authed(request):
            return _err(401, "invalid api key")
        payload, error = await _read_body(request)
        if error:
            return _err(400, error, type_="invalid_request_error")
        if request.url.path == "/v1/responses":
            # Alias de chat limitado compatible con el contrato histórico:
            # responses usa `input`, pero no pretende implementar objetos,
            # tools ni eventos propios de la Response API.
            msgs = _normalise_responses_input(payload.get("input", []))
            if msgs is None:
                return _err(400, "input debe ser un string o una lista de mensajes {role, content}",
                            type_="invalid_request_error")
        else:
            msgs = _normalise_messages(payload.get("messages", []))
            if msgs is None:
                return _err(400, "messages debe ser una lista de mensajes {role, content}",
                            type_="invalid_request_error")
        return await _chat(msgs, payload)

    @app.post("/v1/chat/completions")
    async def chat_completions(request: Request):
        return await _chat_like(request)

    @app.post("/v1/responses")
    async def responses(request: Request):
        return await _chat_like(request)

    @app.post("/{full_path:path}")
    async def handle_post(full_path: str, request: Request):
        # Mismo orden que el servidor anterior: la API key se exige ANTES de
        # mirar si la ruta existe, para cualquier POST que no case con las
        # rutas explícitas de arriba.
        path = "/" + full_path
        if not _authed(request):
            return _err(401, "invalid api key")
        return _err(404, "no such route: %s" % path)

    @app.get("/{full_path:path}")
    async def handle_get(full_path: str):
        # Catch-all para GET: el servidor anterior respondía 404 con este
        # mismo cuerpo JSON para cualquier ruta que no fuera /health,
        # /v1/models o /ropero/engine (ninguna otra GET exigía auth, así que
        # aquí tampoco se comprueba).
        path = "/" + full_path
        return _err(404, "no such route: %s" % path)

    return app


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Servidor OpenAI-compatible sobre airllm (FastAPI + uvicorn, "
                    "lo que airllm no trae)")
    ap.add_argument("--model", required=True,
                    help="ruta al checkpoint de HF (safetensors/bin), no gguf")
    ap.add_argument("--model-id", required=True,
                    help="nombre que se publica en /v1/models (el de ropero)")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=9999)
    ap.add_argument("--api-key", default="")
    ap.add_argument("--max-seq-len", type=int, default=32768,
                    help="contexto. El default de airllm es 512: ponlo siempre")
    ap.add_argument("--max-new-tokens", type=int, default=4096)
    ap.add_argument("--temperature", type=float, default=0.0)
    ap.add_argument("--top-p", type=float, default=1.0)
    ap.add_argument("--compression", default="none", choices=["none", "4bit", "8bit"],
                    help="4bit: ~4x más rápido, algo de precisión. Exige bitsandbytes")
    ap.add_argument("--layer-shards", default="",
                    help="dónde se guardan las capas troceadas (ponlo en SSD rápido)")
    ap.add_argument("--no-prefetch", action="store_true",
                    help="no prefetchear la capa siguiente. Solo si el disco va justo")
    ap.add_argument("--device", default="", help="cuda:0 | mps | cpu")
    global _ARGS
    args = ap.parse_args()
    _ARGS = args

    # Antes de tocar el modelo: sin fastapi/uvicorn no hay nada que servir.
    if _IMPORT_ERROR is not None:
        sys.stderr.write(
            "ERROR: faltan dependencias para airllm-server: %s\n" % _IMPORT_ERROR
        )
        sys.stderr.write("pip install fastapi uvicorn\n")
        sys.stderr.flush()
        return 2

    if not os.path.exists(args.model):
        log("ERROR: el modelo no existe: %s" % args.model)
        return 2
    if args.compression != "none":
        try:
            import bitsandbytes  # noqa: F401
        except ImportError:
            log("ERROR: --compression %s necesita bitsandbytes: pip install bitsandbytes"
                % args.compression)
            return 2

    try:
        load_backend()
    except Exception as e:  # noqa: BLE001
        log("ERROR al cargar el modelo: %s: %s" % (type(e).__name__, e))
        log("Si es un ModuleNotFoundError de transformers, airllm 2.11.0 pide "
            "transformers<5 (importa LlamaForCausalLM del nivel superior).")
        return 1

    app = create_app()
    log("OpenAI-compatible en http://%s:%d/v1 (model=%s, auth=%s)"
        % (args.host, args.port, args.model_id, "on" if args.api_key else "off"))
    try:
        # Un solo worker (sin multiprocessing: el modelo ya está cargado en
        # ESTE proceso y airllm/transformers no se comparten entre procesos
        # sin recargar todo). uvicorn instala sus propios handlers de
        # SIGINT/SIGTERM al correr en el hilo principal. El límite de apagado
        # graceful es deliberadamente de 5 s, inferior a los 10 s que ropero
        # reserva antes de su SIGKILL de respaldo: las requests activas no
        # pueden hacer que uvicorn espere indefinidamente.
        uvicorn.run(
            app,
            host=args.host,
            port=args.port,
            workers=1,
            log_level="info",
            access_log=True,
            timeout_graceful_shutdown=5,
        )
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
