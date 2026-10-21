#!/usr/bin/env python3
# =============================================================================
# endpoints/server.py — endpoint OpenAI-compatible de ropero para airllm,
#                        tensorrt-llm y fastflowlm
# =============================================================================
#
# POR QUÉ EXISTE
#
# ropero promete que TODOS sus engines hablan la misma API (mismo puerto, misma
# `Authorization: Bearer $LLAMA_API_KEY`, mismo `id` de modelo). Eso sale gratis
# con llama-server, que trae `--api-key`. Con estos tres no:
#
#   airllm        → es una LIBRERÍA pura: no tiene `serve` ni `__main__.py`.
#                   El server es NUESTRO (bin/_airllm_server.py).
#   trtllm-serve  → NO tiene `--api_key` (el api_key del SDK de OpenAI es un
#                   campo del CLIENTE, no una comprobación del servidor).
#   flm serve     → NO tiene auth: su doc dice "the key is not checked".
#
# Con HOST=0.0.0.0, esos tres quedarían SIN AUTH expuestos a la LAN. Antes lo
# tapaba bin/_authproxy.py, un proxy aparte. Este fichero hace lo mismo
# pero para los TRES a la vez, en un único proceso, y además habla el mismo
# contrato OpenAI que el resto de engines (no sólo "reenvía").
#
# QUÉ APORTA FRENTE AL PROXY DE AUTH
#
#   · Una sola API pública para los tres: /v1/models, /v1/chat/completions,
#     /v1/completions y /health, con el MISMO esquema de respuesta. El proxy
#     de auth sólo reenvía: si el engine no implementa una ruta, no existe.
#   · airllm deja de necesitar un fichero de server propio: se sirve en el
#     mismo proceso que el resto.
#   · El `model` del body se reescribe al nombre que el BACKEND conoce, así
#     que el cliente puede mandar cualquier nombre (y /v1/models anuncia el de
#     ropero, que es lo que hace funcionar `ropero stop <modelo>` y la columna
#     MODELO del status).
#   · Ciclo de vida en un sitio: el sub-proceso backend es HIJO de este
#     proceso, así que al morir el padre muere el hijo y se liberan los dos
#     puertos.
#
# ARQUITECTURA
#
#   cliente ──► [ endpoints/server.py ] ──► 127.0.0.1:<puerto interno>
#                (el ÚNICO que se expone)      (trtllm-serve | flm serve)
#
#   El sub-proceso SIEMPRE escucha en 127.0.0.1. El puerto interno es
#   `--backend-port` (por defecto, puerto externo + 1000).
#
# DEPENDENCIAS
#
#   fastapi + uvicorn (obligatorias: sin ellas no hay servidor).
#   httpx (opcional: sólo para el proxy). Si no está, el proxy usa
#   urllib.request de la stdlib, que es perfectamente viable y evita una
#   dependencia. Se puede forzar con ROPERO_ENDPOINT_PROXY=urllib.
#
#   airllm: sólo para el backend `airllm` (una biblioteca, no un binario).
#
# USO
#
#   python3 endpoints/server.py --backend tensorrt \
#       --model-path /ruta/al/checkpoint --model-id metaloss \
#       --host 0.0.0.0 --port 9998 --api-key sk-... --backend-port 12999
#
#   python3 endpoints/server.py --backend flm \
#       --model-path llama3.2:1b --model-id gemma4 --host 0.0.0.0 --port 9998 \
#       --api-key sk-... --backend-port 13999
#
#   python3 endpoints/server.py --backend airllm \
#       --model-path /ruta/al/safetensors --model-id airgptoss \
#       --host 0.0.0.0 --port 9999 --api-key sk-... --max-seq-len 131072
# =============================================================================

import argparse
import glob
import asyncio
import contextlib
import hmac
import json
import os
import queue
import shutil
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
import uuid

try:
    import uvicorn
    from fastapi import FastAPI, Request
    from fastapi.responses import JSONResponse, Response, StreamingResponse

    _IMPORT_ERROR = None
except ImportError as exc:  # noqa: N818 — se usa como dato, no se relanza aquí
    uvicorn = None  # type: ignore[assignment]
    FastAPI = None  # type: ignore[assignment,misc]
    Request = None  # type: ignore[assignment,misc]
    JSONResponse = None  # type: ignore[assignment,misc]
    Response = None  # type: ignore[assignment,misc]
    StreamingResponse = None  # type: ignore[assignment,misc]
    _IMPORT_ERROR = exc

try:
    import httpx
except ImportError:  # pragma: no cover — depende de la máquina
    httpx = None  # type: ignore[assignment]


# -----------------------------------------------------------------------------
# Constantes
# -----------------------------------------------------------------------------
LOG_PREFIX = "ropero-endpoint"

# Puerto interno por defecto respecto al externo: el interno nunca es el
# público. Es el mismo "salto de 1000" que usa el resto del repo.
INTERNAL_PORT_OFFSET = 1000

# Contexto por defecto del backend airllm. NO es el de airllm: el suyo es 512
# (verificado en la firma de __init__), y con 512 tokens el contexto se corta
# antes de empezar. Para los otros dos, si no se pide contexto, cada engine usa
# el suyo.
DEFAULT_CONTEXT = 32768

# Cabeceras que no se reenvían al backend: son salto-a-salto (RFC 7230 §6.1) o
# las fija el propio transporte. `authorization` se quita a propósito: el
# backend NO la valida (por eso existe este proxy), así que reenviar una
# credencial a un componente que no la necesita sólo amplía la superficie.
HOP_BY_HOP = {
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
    "authorization",
}


def log(msg: str) -> None:
    """Log a stderr con timestamp. Nunca a stdout: stdout lo consume ropero.

    Nunca se loguea la API key ni ninguna cabecera de auth. Este fichero sólo
    loguea nombres de modelo, puertos y errores.
    """
    sys.stderr.write("[%s %s] %s\n" % (LOG_PREFIX, time.strftime("%H:%M:%S"), msg))
    sys.stderr.flush()


# -----------------------------------------------------------------------------
# Utilidades de request
# -----------------------------------------------------------------------------
def msg_text(content):
    """Aplana el content de un mensaje, que en VLM puede ser una lista de
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


def normalise_messages(value):
    """Convierte mensajes OpenAI estructurados al formato interno.

    Devuelve None si el formato no es válido (para poder responder 400 con un
    mensaje claro en vez de reventar con un traceback).
    """
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
        messages.append({"role": role, "content": msg_text(content)})
    return messages


def err(status: int, message: str, type_: str = "invalid_request_error",
        code: str = "") -> "JSONResponse":
    """Error con el formato de OpenAI: {"error": {message, type, code}}."""
    body = {"error": {"message": message, "type": type_}}
    if code:
        body["error"]["code"] = code
    return JSONResponse(body, status_code=status)


def authed(request: "Request", api_key: str) -> bool:
    """Bearer (o X-Api-Key, que usan algunos clientes) contra la key de ropero.

    Comparación en tiempo constante: la key es un secreto compartido y el
    coste de hacerlo bien es cero.
    """
    if not api_key:
        return True
    # A bytes: compare_digest con str sólo admite ASCII, y una cabecera con
    # acentos no debe acabar en un 500.
    want = api_key.encode("utf-8")
    header = request.headers.get("authorization", "")
    if header.startswith("Bearer "):
        if hmac.compare_digest(header[7:].strip().encode("utf-8"), want):
            return True
    alt = request.headers.get("x-api-key", "").strip()
    if alt and hmac.compare_digest(alt.encode("utf-8"), want):
        return True
    return False


def parse_json(raw: bytes):
    """Devuelve (payload, error).

    Las rutas de OpenAI aceptan únicamente un objeto JSON como body. Separar
    aquí JSON inválido de un objeto vacío evita que una forma de request normal
    llegue a .get() y termine como un 500.
    """
    if not raw:
        return {}, None
    try:
        payload = json.loads(raw)
    except ValueError:
        return None, "JSON inválido"
    if not isinstance(payload, dict):
        return None, "el body JSON debe ser un objeto"
    return payload, None


def port_open(host: str, port: int, timeout: float = 0.5) -> bool:
    """¿Hay alguien escuchando en host:port?"""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(timeout)
    try:
        sock.connect((host, port))
        return True
    except OSError:
        return False
    finally:
        sock.close()


def resolve_binary(engine: str, override: str = "") -> str:
    """Ruta al ejecutable del backend, o "" si no se encuentra.

    El orden NO es arbitrario y es el mismo que usa bin/_engines.sh:
    primero lo que ropero linkeó ($BIN_DIR), después las rutas largas
    conocidas (venv de tensorrt, /opt/fastflowlm) y el PATH al final.
    """
    if override:
        return override
    bin_dir = os.environ.get("BIN_DIR") or os.path.expanduser("~/.local/bin")
    candidates = {
        "tensorrt": [
            os.path.join(bin_dir, "trtllm-serve"),
            os.path.expanduser("~/.venv-tensorrt/bin/trtllm-serve"),
        ],
        "flm": [
            "/opt/fastflowlm/bin/flm",
            os.path.join(bin_dir, "flm"),
        ],
        "mlx": [
            os.path.join(bin_dir, "mlx_lm.server"),
        ],
    }.get(engine, [])
    if engine == "mlx":
        for d in glob.glob(os.path.expanduser("~/.asdf/installs/python/*/bin")):
            cand = os.path.join(d, "mlx_lm.server")
            if os.path.isfile(cand) and os.access(cand, os.X_OK):
                return cand
    for cand in candidates:
        if os.path.isfile(cand) and os.access(cand, os.X_OK):
            return cand
    name = {"tensorrt": "trtllm-serve", "flm": "flm", "mlx": "mlx_lm.server"}.get(engine, "")
    found = shutil.which(name) if name else None
    return found or ""


# -----------------------------------------------------------------------------
# Backend airllm — genera EN ESTE PROCESO
# -----------------------------------------------------------------------------
class AirLLMBackend:
    """airllm es una biblioteca, no un servidor: aquí se carga y se genera.

    La lógica (carga capa a capa, sampling, render del prompt por chat
    template, generación con y sin streaming) es la de bin/_airllm_server.py.
    Se replica aquí en vez de importarse porque ese fichero lo está moviendo
    otro agente a bin/ y este endpoint tiene que ser autónomo.
    """

    engine = "airllm"
    label = "airllm"

    def __init__(self, args):
        self.args = args
        self.model = None
        self.tokenizer = None
        self.hf_style = True
        self.error = None
        self.ready_flag = False
        # Un lock global y no uno por request: airllm mueve el modelo entero
        # capa a capa. Dos generaciones a la vez no es "más rápido", es el
        # mismo throughput repartido y el doble de swap.
        self.lock = threading.Lock()

    # ── ciclo de vida ───────────────────────────────────────────────────────
    def start(self) -> None:
        # En segundo plano: cargar un modelo de 120B tarda minutos, y durante
        # ese tiempo el endpoint DEBE responder (/health, /v1/models) con un
        # 503 "model loading" en las rutas de inferencia. Si se cargara antes
        # de abrir el puerto, el slot parecería muerto durante la carga.
        threading.Thread(target=self._load, daemon=True).start()

    def _load(self) -> None:
        args = self.args
        kwargs = {
            "max_seq_len": args.context,
            "prefetching": not args.no_prefetch,
        }
        if args.compression != "none":
            kwargs["compression"] = args.compression
        if args.layer_shards:
            kwargs["layer_shards_saving_path"] = args.layer_shards
        if os.environ.get("HF_TOKEN"):
            kwargs["hf_token"] = os.environ["HF_TOKEN"]

        log("cargando %s (compression=%s max_seq_len=%d prefetch=%s)"
            % (args.model_path, args.compression, args.context, kwargs["prefetching"]))
        t0 = time.time()
        try:
            from airllm import AutoModel

            model = AutoModel.from_pretrained(args.model_path, **kwargs)
            self.model = model

            # El discriminador entre los dos contratos de generate():
            #   · AirLLMLlamaMlx (macOS) tiene generate(texto) -> texto y NO es
            #     un GenerationMixin.
            #   · AirLLMBaseModel (CUDA) sí es GenerationMixin y habla el
            #     contrato de HuggingFace.
            try:
                from transformers import GenerationMixin
                self.hf_style = isinstance(model, GenerationMixin)
            except Exception:
                self.hf_style = hasattr(model, "input_embeddings")

            tok = getattr(model, "tokenizer", None)
            if tok is None:
                from transformers import AutoTokenizer
                tok = AutoTokenizer.from_pretrained(args.model_path, trust_remote_code=True)
            self.tokenizer = tok
            self.ready_flag = True
            log("modelo listo en %.1fs (contrato HF=%s)" % (time.time() - t0, self.hf_style))

            try:
                archs = getattr(model.config, "architectures", None) or ["?"]
                log("architectures: %s" % ", ".join(archs))
                if not self.hf_style:
                    log("AVISO: macOS → airllm usa SIEMPRE AirLLMLlamaMlx. Este modelo "
                        "solo funciona si su arquitectura es estilo Llama.")
            except Exception:
                pass
        except Exception as exc:  # noqa: BLE001 — se reporta al cliente
            self.error = exc
            log("ERROR al cargar el modelo: %s: %s" % (type(exc).__name__, exc))
            if isinstance(exc, ImportError):
                log("Si es un ModuleNotFoundError de transformers, airllm 2.11.0 pide "
                    "transformers<5 (importa LlamaForCausalLM del nivel superior).")

    def stop(self) -> None:
        self.model = None
        self.tokenizer = None

    # ── estado ──────────────────────────────────────────────────────────────
    def alive(self) -> bool:
        return True

    def ready(self) -> bool:
        return self.ready_flag

    def status(self) -> dict:
        return {
            "kind": "in-process",
            "alive": True,
            "ready": self.ready_flag,
            "child_pid": 0,
            "error": ("%s: %s" % (type(self.error).__name__, self.error))
                     if self.error else None,
        }

    # ── inferencia ──────────────────────────────────────────────────────────
    def _sampling(self):
        return {
            "max_new_tokens": self.args.max_new_tokens,
            "temperature": self.args.temperature,
            "top_p": self.args.top_p,
        }

    def _render_prompt(self, messages):
        """Chat → texto, por el chat template del tokenizer.

        Es lo que hace que el modelo entienda de qué turno va la conversación.
        Sin template, a un modelo afinado para chat le llega la lista de
        mensajes cruda y contesta como si fuera un documento.
        """
        try:
            return self.tokenizer.apply_chat_template(
                messages, add_generation_prompt=True, tokenize=False
            )
        except Exception as exc:
            log("el chat template falló (%s); uso concatenación plana" % exc)
            out = []
            for message in messages:
                out.append("%s: %s" % (message.get("role", "user"), message.get("content", "")))
            out.append("assistant:")
            return "\n".join(out)

    def generate_text(self, messages):
        """Generación sin streaming. Devuelve (texto, n_tokens, n_prompt)."""
        prompt = self._render_prompt(messages)

        if not self.hf_style:
            # Contrato de AirLLMLlamaMlx: texto entra, texto sale.
            return self.model.generate(prompt, **self._sampling()), 0, 0

        ids = self.tokenizer.apply_chat_template(
            messages, add_generation_prompt=True, return_tensors="pt", return_dict=True
        )
        dev = getattr(self.model, "device", None)
        if dev is not None:
            ids = {k: v.to(dev) for k, v in ids.items()}
        n_in = ids["input_ids"].shape[-1]

        out = self.model.generate(**ids, do_sample=self.args.temperature > 0,
                                  **self._sampling())
        # out[0] es la secuencia COMPLETA (prompt + generado): si se decodifica
        # entera, el cliente recibe su propia pregunta como respuesta.
        gen = out[0][n_in:]
        text = self.tokenizer.decode(gen, skip_special_tokens=True)
        return text, int(gen.shape[-1]), int(n_in)

    def _locked_generate(self, messages):
        with self.lock:
            return self.generate_text(messages)

    def stream_text(self, messages):
        """Generador de tokens, para `stream: true`.

        Los dos caminos tienen equivalente de streaming, pero por mecanismos
        distintos:
          · HF:  TextIteratorStreamer + hilo aparte. Necesario porque
            generate() es bloqueante y bloquearía el server entero.
          · MLX: model_generate() ya es un generador; se consume directamente.
        """
        if not self.hf_style:
            prompt = self._render_prompt(messages)
            n = 0
            for tok in self.model.model_generate(prompt, temperature=self.args.temperature):
                # model_generate puede dar un tensor de MLX o un int ya resuelto.
                if hasattr(tok, "item"):
                    tok = tok.item()
                piece = self.tokenizer.decode([int(tok)], skip_special_tokens=True)
                if piece:
                    yield piece
                n += 1
                if n >= self.args.max_new_tokens:
                    break
            return

        from transformers import TextIteratorStreamer

        ids = self.tokenizer.apply_chat_template(
            messages, add_generation_prompt=True, return_tensors="pt", return_dict=True
        )
        dev = getattr(self.model, "device", None)
        if dev is not None:
            ids = {k: v.to(dev) for k, v in ids.items()}

        streamer = TextIteratorStreamer(
            self.tokenizer, skip_prompt=True, skip_special_tokens=True
        )
        kwargs = self._sampling()
        kwargs["streamer"] = streamer
        kwargs["do_sample"] = self.args.temperature > 0

        thread = threading.Thread(target=self.model.generate, kwargs=dict(ids, **kwargs),
                                  daemon=True)
        thread.start()
        for piece in streamer:
            yield piece
        thread.join()

    def _stream_worker(self, messages) -> "queue.Queue":
        """Puente generador-síncrono → asíncrono, para el streaming SSE.

        stream_text() es un generador BLOQUEANTE. FastAPI/uvicorn corren sobre
        asyncio: iterarlo directamente dentro de una ruta async bloquearía el
        loop entero y mataría la concurrencia (incluido /health mientras se
        genera). Por eso se consume en un hilo propio y las piezas se entregan
        con una queue.Queue clásica.
        """
        q = queue.Queue()

        def run():
            try:
                with self.lock:
                    for piece in self.stream_text(messages):
                        q.put(("piece", piece))
                q.put(("finished", None))
            except Exception as exc:  # noqa: BLE001 — se reporta al cliente
                q.put(("error", exc))

        threading.Thread(target=run, daemon=True).start()
        return q

    # ── dispatch ────────────────────────────────────────────────────────────
    async def dispatch(self, messages, payload, want_stream, upstream_path, raw_body,
                       request):
        """Genera aquí y devuelve la respuesta OpenAI.

        Los backends proxied devuelven lo que conteste el engine; aquí se
        construye la respuesta completa con el esquema de OpenAI.
        """
        if not self.ready():
            if self.error is not None:
                return err(503, "model loading (%s: %s)"
                           % (type(self.error).__name__, self.error),
                           "server_error", "model_loading")
            return err(503, "model loading", "server_error", "model_loading")

        if payload.get("tools"):
            # airllm no tiene template de herramientas: un endpoint de texto que
            # finge soportarlas hace que el cliente se cuelgue esperando un
            # tool_call que no va a llegar. Se dice explícitamente.
            return err(400,
                       "airllm no implementa tool calling (no es un motor de "
                       "inferencia, es un offloader de capas). Las tools del "
                       "request se ignorarían y el cliente colgaría.")

        cid = "chatcmpl-" + uuid.uuid4().hex[:24]
        created = int(time.time())
        model_id = self.args.model_id

        if want_stream:
            return StreamingResponse(
                self._sse(cid, created, model_id, messages),
                media_type="text/event-stream",
                headers={"Cache-Control": "no-cache"},
            )

        try:
            text, ntok, nprompt = await asyncio.to_thread(self._locked_generate, messages)
        except Exception as exc:  # noqa: BLE001 — al cliente le sirve el motivo
            log("error generando: %r" % (exc,))
            return err(500, "%s: %s" % (type(exc).__name__, exc), "server_error")

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
                "prompt_tokens": nprompt,
                "completion_tokens": ntok,
                "total_tokens": nprompt + ntok,
            },
        }

    async def _sse(self, cid, created, model_id, messages):
        def event(obj) -> bytes:
            return ("data: %s\n\n" % json.dumps(obj)).encode()

        q = self._stream_worker(messages)
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
            # El cierre del stream SSE de OpenAI. Los chunks que emite el
            # backend (trtllm, flm) ya traen su propio "data: [DONE]"; aquí lo
            # generamos nosotros porque airllm no tiene servidor.
            yield b"data: [DONE]\n\n"


# -----------------------------------------------------------------------------
# Backend proxied — trtllm-serve y flm serve
# -----------------------------------------------------------------------------
class ProxyBackend:
    """Lanza el engine como sub-proceso y hace de proxy inverso.

    El engine SIEMPRE escucha en 127.0.0.1: el único proceso expuesto es
    este. El nombre del modelo del body se reescribe al que el backend
    conoce, para que el cliente pueda mandar cualquier nombre.
    """

    def __init__(self, args, engine: str, label: str, argv_builder):
        self.args = args
        self.engine = engine
        self.label = label
        self.argv_builder = argv_builder
        self.child = None
        self.child_pid = 0
        self.error = None
        self.ready_flag = False
        self._stop = threading.Event()
        self._recent = []            # últimas líneas de stderr del sub-proceso
        self._retry_pending = True   # aún no se ha reintentado sin el flag
        self._client = None          # cliente httpx (si se usa)

    # ── ciclo de vida ───────────────────────────────────────────────────────
    def start(self) -> None:
        bin_path = resolve_binary(self.engine, self.args.backend_bin)
        if not bin_path:
            self.error = "no encuentro el ejecutable de %s" % self.engine
            log("ERROR: %s" % self.error)
            return
        self.bin_path = bin_path
        self._spawn(self.argv_builder(self.args, bin_path, True))
        threading.Thread(target=self._supervise, daemon=True).start()
        threading.Thread(target=self._poll_ready, daemon=True).start()

    def _spawn(self, argv) -> None:
        log("lanzando backend: %s" % " ".join(argv))
        try:
            self.child = subprocess.Popen(
                argv,
                stdout=sys.stderr,      # el log del engine va al logfile de ropero
                stderr=subprocess.PIPE, # pero se lee para detectar errores
                stdin=subprocess.DEVNULL,
            )
        except OSError as exc:
            self.error = "no se pudo lanzar %s: %s" % (self.engine, exc)
            log("ERROR: %s" % self.error)
            return
        self.child_pid = self.child.pid
        self._recent = []
        threading.Thread(target=self._pump_stderr, daemon=True).start()

    def _pump_stderr(self) -> None:
        """Vuelca el stderr del sub-proceso al nuestro y guarda las últimas
        líneas. Hace falta leerlo: si nadie lee el pipe, el sub-proceso se
        bloquea al llenarlo."""
        child = self.child
        if child is None or child.stderr is None:
            return
        try:
            for raw in iter(child.stderr.readline, b""):
                text = raw.decode("utf-8", "replace").rstrip()
                if not text:
                    continue
                sys.stderr.write(text + "\n")
                sys.stderr.flush()
                self._recent.append(text)
                del self._recent[:-40]
        except (OSError, ValueError):
            pass

    def _supervise(self) -> None:
        """Vigila al sub-proceso.

        Si muere con "no such option", se reintenta sin el flag opcional (el
        caso real: trtllm-serve 1.2.1 NO tiene --served_model_name, que sí
        tienen las versiones 1.x más nuevas). Si muere por otro motivo, se
        marca como caído y el endpoint responde 503 en vez de colgarse.
        """
        while not self._stop.is_set():
            child = self.child
            if child is None:
                return
            rc = child.wait()
            if self._stop.is_set():
                return
            recent = " ".join(self._recent).lower()
            if self._retry_pending and "no such option" in recent:
                self._retry_pending = False
                log("%s no admite el flag opcional; reintento sin él" % self.engine)
                self._spawn(self.argv_builder(self.args, self.bin_path, False))
                continue
            self.error = "el backend %s terminó (rc=%s)" % (self.engine, rc)
            log("ERROR: %s" % self.error)
            return

    def _poll_ready(self) -> None:
        """¿El backend ya escucha? Mientras no, el endpoint dice 'model loading'."""
        while not self._stop.is_set():
            if self.child is None:
                time.sleep(0.5)
                continue
            if self.child.poll() is not None:
                return
            if port_open("127.0.0.1", self.args.backend_port):
                self.ready_flag = True
                return
            time.sleep(0.5)

    def stop(self) -> None:
        self._stop.set()
        child = self.child
        if child is None or child.poll() is not None:
            return
        try:
            child.terminate()
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                log("el backend %s no respeta SIGTERM; SIGKILL" % self.engine)
                child.kill()
                with contextlib.suppress(Exception):
                    child.wait(timeout=5)
        except OSError as exc:
            log("no se pudo parar el backend: %s" % exc)

    # ── estado ──────────────────────────────────────────────────────────────
    def alive(self) -> bool:
        return self.child is not None and self.child.poll() is None

    def ready(self) -> bool:
        return self.ready_flag and self.alive()

    def status(self) -> dict:
        return {
            "kind": "subprocess",
            "alive": self.alive(),
            "ready": self.ready(),
            "child_pid": self.child_pid,
            "target_host": "127.0.0.1",
            "target_port": self.args.backend_port,
            "error": self.error,
        }

    # ── proxy ───────────────────────────────────────────────────────────────
    def _upstream_headers(self, request) -> dict:
        """Cabeceras hacia el backend.

        Se quitan las salto-a-salto, `host` (la pone el transporte) y
        `authorization` (el backend no la valida: reenviar una credencial a un
        componente que no la necesita sólo amplía la superficie). Se fija
        `accept-encoding: identity` para que los bytes que llegan al cliente
        sean los mismos que mandó el backend, sin recomprimir.
        """
        up = {}
        for key, value in request.headers.items():
            low = key.lower()
            if low in HOP_BY_HOP or low == "accept-encoding":
                continue
            # Todo en minúsculas: las cabeceras HTTP no distinguen mayúsculas
            # y así no puede colarse la misma cabecera dos veces ("Accept-
            # Encoding" del cliente y la nuestra).
            up[low] = value
        up["accept-encoding"] = "identity"
        up.setdefault("content-type", "application/json")
        return up

    def _backend_body(self, payload, messages) -> bytes:
        """Body que se manda al backend, con el `model` ya normalizado.

        El nombre al que se reescribe es el que el BACKEND conoce:
          · tensorrt → `--served_model_name` (o el id de ropero si el binario
            no admite ese flag).
          · flm      → el TAG con el que se lanzó `flm serve`. OJO: FLM
            valida el campo (su error es "Model X is not supported."), así que
            reescribirlo al id de ropero rompería el request.
        El campo se pone SIEMPRE, no sólo se reescribe si venía: trtllm lo
        exige en el esquema OpenAI (pydantic: `model: str`, obligatorio).
        """
        doc = dict(payload)
        if messages is not None:
            doc.pop("prompt", None)
            doc["messages"] = messages
        doc["model"] = self.args.rewrite_model
        return json.dumps(doc).encode("utf-8")

    async def dispatch(self, messages, payload, want_stream, upstream_path, raw_body,
                       request):
        if not self.alive():
            return err(503,
                       "el backend %s no está corriendo (se ha salido)" % self.engine,
                       "upstream_unavailable", "backend_down")
        if not self.ready():
            return err(503, "model loading", "server_error", "model_loading")

        body = self._backend_body(payload, messages)
        url = "http://127.0.0.1:%d%s" % (self.args.backend_port, upstream_path)
        headers = self._upstream_headers(request)

        if want_stream:
            return await self._stream(url, body, headers)
        return await self._buffered(url, body, headers)

    async def _buffered(self, url, body, headers):
        """Proxy de respuesta completa."""
        try:
            if self._use_httpx():
                resp = await self._client.request(
                    "POST", url, content=body, headers=headers,
                    # Lectura acotada para la respuesta completa: una
                    # generación puede tardar, pero no indefinidamente.
                    timeout=httpx.Timeout(connect=10.0, read=3600.0, write=None, pool=None),
                )
                return Response(
                    content=resp.content,
                    status_code=resp.status_code,
                    headers=_response_headers(resp.headers),
                )
            return await asyncio.to_thread(self._urllib_buffered, url, body, headers)
        except Exception as exc:  # noqa: BLE001 — se reporta al cliente
            return self._upstream_error(exc)

    async def _stream(self, url, body, headers):
        """Proxy de streaming (SSE).

        El upstream ya emite `data: [DONE]` al final (así lo hacen trtllm y
        flm), así que los bytes se reenvían tal cual: no se toca el formato.
        """
        try:
            if self._use_httpx():
                request = self._client.build_request(
                    "POST", url, content=body, headers=headers
                )
                resp = await self._client.send(request, stream=True)
            else:
                resp = await asyncio.to_thread(self._urllib_open, url, body, headers)
        except Exception as exc:  # noqa: BLE001 — se reporta al cliente
            return self._upstream_error(exc)

        status = getattr(resp, "status_code", None)
        if status is None:
            status = getattr(resp, "status", 200)
        raw_headers = getattr(resp, "headers", {})
        out_headers = _response_headers(raw_headers)
        out_headers.setdefault("Cache-Control", "no-cache")

        if self._use_httpx():
            async def gen():
                try:
                    async for chunk in resp.aiter_raw():
                        yield chunk
                finally:
                    with contextlib.suppress(Exception):
                        await resp.aclose()
        else:
            async def gen():
                try:
                    while True:
                        buf = await asyncio.to_thread(resp.read, 8192)
                        if not buf:
                            break
                        yield buf
                finally:
                    with contextlib.suppress(Exception):
                        resp.close()

        return StreamingResponse(gen(), status_code=status, headers=out_headers,
                                 media_type="text/event-stream")

    def _use_httpx(self) -> bool:
        if httpx is None:
            return False
        if os.environ.get("ROPERO_ENDPOINT_PROXY", "").lower() == "urllib":
            return False
        return self._client is not None

    async def _upstream_error(self, exc):
        """El backend no respondió. 503 con un motivo claro, nunca un cuelgue."""
        name = type(exc).__name__
        msg = str(exc)
        if isinstance(exc, (ConnectionRefusedError,)) or "ConnectError" in name:
            if self.alive():
                return err(503, "model loading", "server_error", "model_loading")
            return err(503, "el backend %s no está corriendo (se ha salido)"
                       % self.engine, "upstream_unavailable", "backend_down")
        log("error hablando con el backend: %s: %s" % (name, msg))
        return err(502, "no se pudo contactar con el backend %s (%s: %s)"
                   % (self.engine, name, msg), "upstream_unavailable", "backend_error")

    # ── urllib (fallback sin httpx) ─────────────────────────────────────────
    def _urllib_open(self, url, body, headers):
        req = urllib.request.Request(url, data=body, method="POST")
        for key, value in headers.items():
            req.add_header(key, value)
        try:
            return urllib.request.urlopen(req, timeout=None)
        except urllib.error.HTTPError as exc:
            # Un 4xx/5xx del backend también es una respuesta: se reenvía tal
            # cual, con su cuerpo y su código.
            return exc

    def _urllib_buffered(self, url, body, headers):
        resp = self._urllib_open(url, body, headers)
        try:
            data = resp.read()
            return Response(
                content=data,
                status_code=getattr(resp, "status", None) or resp.getcode(),
                headers=_response_headers(resp.headers),
            )
        finally:
            with contextlib.suppress(Exception):
                resp.close()


def _response_headers(headers) -> dict:
    """Cabeceras del backend → cliente.

    Se quitan las salto-a-salto y `content-length` (lo recalcula el
    transporte). El resto se reenvían tal cual.
    """
    out = {}
    items = headers.items() if hasattr(headers, "items") else headers
    for key, value in items:
        low = key.lower()
        if low in HOP_BY_HOP or low == "content-length":
            continue
        out[key] = value
    return out


# -----------------------------------------------------------------------------
# apply_extra_args(args)
#
# Los `[args].base` del TOML del modelo llegan aquí como `--extra-arg`, uno por
# flag. Qué hacer con ellos depende del backend:
#
#   tensorrt / flm → son flags del binario nativo: se añaden a su argv, tal
#                     cual y en el orden en que llegan.
#   airllm          → airllm NO es un proceso: es una biblioteca. Sus flags
#                     (--compression, --max-new-tokens, --layer-shards) se
#                     traducen a los parámetros de carga del modelo.
#
# Lo que no se reconoce en airllm se ignora con un aviso: preferimos un
# modelo que arranque con los parámetros que sí entendemos a un arranque que
# falla por un flag sobrante.
# -----------------------------------------------------------------------------
AIRLLM_EXTRA_FLAGS = {
    "--compression": "compression",
    "--max-new-tokens": "max_new_tokens",
    "--layer-shards": "layer_shards",
}


def apply_extra_args(args):
    extra = list(getattr(args, "extra_arg", None) or [])
    if not extra:
        return

    if args.backend != "airllm":
        # Se pasan tal cual al backend nativo.
        args.backend_extra_args = extra
        return

    # airllm: se emparejan flag + valor.
    args.backend_extra_args = []
    i = 0
    while i < len(extra):
        token = extra[i]
        key = AIRLLM_EXTRA_FLAGS.get(token)
        if key is None:
            if not token.startswith("-"):
                # Un valor suelto sin su flag: se ignora con aviso.
                log("aviso: argumento de airllm sin flag recognized: %s" % token)
            i += 1
            continue
        value = extra[i + 1] if i + 1 < len(extra) else None
        if value is None:
            log("aviso: %s sin valor; se ignora" % token)
            i += 1
            continue
        if key == "compression":
            if value not in ("none", "4bit", "8bit"):
                log("aviso: --compression %s no es un valor conocido; se ignora" % value)
            else:
                args.compression = value
        elif key == "max_new_tokens":
            try:
                args.max_new_tokens = int(value)
            except ValueError:
                log("aviso: --max-new-tokens %s no es un número; se ignora" % value)
        elif key == "layer_shards":
            args.layer_shards = value
        i += 2


# -----------------------------------------------------------------------------
# Construcción del argv de cada backend
# -----------------------------------------------------------------------------
def tensorrt_argv(args, bin_path: str, with_served_model_name: bool):
    """`trtllm-serve serve [OPTIONS] MODEL` (TensorRT-LLM 1.x).

    · `--served_model_name` es lo más parecido al `--alias` de llama-server:
      fija el nombre que el engine acepta. NO existe en trtllm-serve 1.2.1
      (verificado contra su `--help`), así que se pasa sólo si el binario lo
      admite; si no, el backend se relanza sin él (ver ProxyBackend._supervise).
    · NO existe `--api_key`: la auth la pone este proxy.
    """
    argv = [
        bin_path, "serve",
        "--model", str(args.model_path),
        "--host", "127.0.0.1",
        "--port", str(args.backend_port),
    ]
    if with_served_model_name:
        argv += ["--served_model_name", str(args.model_id)]
    if args.context:
        argv += ["--max_seq_len", str(args.context)]
    # Los flags que declara el modelo en su TOML, tal cual y en su orden.
    argv += list(getattr(args, "backend_extra_args", None) or [])
    return argv


def flm_argv(args, bin_path: str, with_served_model_name: bool):
    """`flm serve [model_tag] --port N` (FastFlowLM).

    · El modelo se identifica por TAG (`llama3.2:1b`), NO por ruta: `flm serve`
      habla con su propio almacén de modelos.
    · SÍ tiene `--host` (default 127.0.0.1); se pone explícito para que el
      backend no pueda salir a la red ni por accidente.
    · NO hay auth: su doc dice "the key is not checked".
    """
    argv = [
        bin_path, "serve", str(args.model_path),
        "--host", "127.0.0.1",
        "--port", str(args.backend_port),
    ]
    if args.context:
        argv += ["--ctx-len", str(args.context)]
    argv += ["--cors", "0"]
    # Los flags que declara el modelo en su TOML, tal cual y en su orden.
    argv += list(getattr(args, "backend_extra_args", None) or [])
    return argv


def mlx_argv(args, bin_path: str, with_served_model_name: bool):
    """`mlx_lm.server --model <path> --host 127.0.0.1 --port N` (MLX).

    · NO tiene --api-key ni --alias: la auth la pone este endpoint y el nombre
      se reescribe a "default_model" (mlx_lm.server solo acepta ese).
    · Los flags propios del modelo (los de [args].base en su TOML) se añaden
      tal cual y en su orden.
    """
    argv = [
        bin_path,
        "--model", str(args.model_path),
        "--host", "127.0.0.1",
        "--port", str(args.backend_port),
    ]
    if args.context:
        argv += ["--max-tokens", str(args.context)]
    argv += list(getattr(args, "backend_extra_args", None) or [])
    return argv



# -----------------------------------------------------------------------------
# App
# -----------------------------------------------------------------------------
def build_app(args, backend):
    """Construye la app ASGI. Los dos backends comparten la MISMA API pública."""

    @contextlib.asynccontextmanager
    async def lifespan(_app):
        backend.start()
        if isinstance(backend, ProxyBackend) and httpx is not None:
            # Sin timeout de lectura: una generación puede tardar minutos y el
            # stream tiene que poder seguir vivo. El de conexión sí es corto:
            # si el backend no está, hay que enterarse YA.
            backend._client = httpx.AsyncClient(
                timeout=httpx.Timeout(connect=10.0, read=None, write=None, pool=None),
                follow_redirects=False,
            )
        try:
            yield
        finally:
            if isinstance(backend, ProxyBackend) and backend._client is not None:
                with contextlib.suppress(Exception):
                    await backend._client.aclose()
            backend.stop()

    app = FastAPI(title="ropero-endpoint", docs_url=None, redoc_url=None, lifespan=lifespan)

    @app.get("/health")
    async def health():
        # SIN auth a propósito: es un chequeo de vida, no información. Y lo
        # abre ropero antes de que el modelo haya terminado de cargar, que es
        # justo cuando más lo necesitas.
        return {
            "status": "ok",
            "engine": backend.engine,
            "model": args.model_id,
            "backend": backend.status(),
        }

    @app.get("/v1/models")
    async def list_models(request: Request):
        if not authed(request, args.api_key):
            return err(401, "invalid api key", "invalid_request_error", "invalid_api_key")
        created = int(time.time())
        if os.path.exists(args.model_path):
            with contextlib.suppress(OSError):
                created = int(os.path.getmtime(args.model_path))
        return {
            "object": "list",
            "data": [{
                "id": args.model_id,
                "object": "model",
                "created": created,
                "owned_by": "ropero",
            }],
        }

    @app.get("/ropero/engine")
    async def ropero_engine(request: Request):
        # Ruta de diagnóstico para la columna ENGINE del status: sin ella
        # habría que deducir el engine del nombre del proceso, que aquí es un
        # `python3` y no dice nada. ropero la consulta con la cabecera de auth.
        if not authed(request, args.api_key):
            return err(401, "invalid api key", "invalid_request_error", "invalid_api_key")
        return {
            "engine": backend.label,
            "model": args.model_id,
            "target_host": "127.0.0.1" if isinstance(backend, ProxyBackend) else args.host,
            "target_port": args.backend_port if isinstance(backend, ProxyBackend) else args.port,
            "child_pid": backend.status().get("child_pid", 0),
            "child_alive": backend.alive(),
        }

    @app.post("/v1/chat/completions")
    @app.post("/v1/completions")
    async def completions(request: Request):
        if not authed(request, args.api_key):
            return err(401, "invalid api key", "invalid_request_error", "invalid_api_key")

        raw = await request.body()
        payload, error = parse_json(raw)
        if error:
            return err(400, error, "invalid_request_error", "invalid_json")

        is_completions = request.url.path == "/v1/completions"
        if is_completions:
            # /v1/completions habla en texto plano. Se traduce a chat para
            # que los TRES backends tengan la misma semántica: flm no
            # implementa /v1/completions (sólo /api/generate, /api/chat y
            # /v1/chat/completions).
            prompt = payload.get("prompt", "")
            if isinstance(prompt, list):
                prompt = prompt[0] if prompt else ""
            messages = [{"role": "user", "content": msg_text(prompt)}]
            upstream_path = "/v1/chat/completions"
        else:
            messages = normalise_messages(payload.get("messages", []))
            if messages is None:
                return err(400, "messages debe ser una lista de mensajes {role, content}",
                           "invalid_request_error", "invalid_messages")
            upstream_path = "/v1/chat/completions"

        want_stream = bool(payload.get("stream"))
        return await backend.dispatch(messages, payload, want_stream, upstream_path,
                                       raw, request)

    return app


# -----------------------------------------------------------------------------
# main
# -----------------------------------------------------------------------------
def main() -> int:
    ap = argparse.ArgumentParser(
        prog="endpoints/server.py",
        description="Endpoint OpenAI-compatible de ropero para airllm, tensorrt-llm "
                    "y fastflowlm (FastAPI + uvicorn).",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="El backend SIEMPRE escucha en 127.0.0.1; este proceso es el único "
               "que se expone y el único que exige la API key.",
    )
    ap.add_argument("--backend", required=True,
                    choices=["airllm", "tensorrt", "flm", "mlx"],
                    help="airllm: genera en este proceso. tensorrt: trtllm-serve. "
                         "flm: flm serve (el modelo es un TAG, no una ruta)")
    ap.add_argument("--model-path", required=True,
                    help="ruta del checkpoint (airllm, tensorrt) o TAG del modelo (flm)")
    ap.add_argument("--model-id", required=True,
                    help="nombre que se publica en /v1/models (el de ropero)")
    ap.add_argument("--host", default="0.0.0.0",
                    help="dirección de escucha del endpoint (default: 0.0.0.0)")
    ap.add_argument("--port", type=int, default=9999,
                    help="puerto del endpoint (default: 9999)")
    ap.add_argument("--api-key", default="",
                    help="si se pasa, TODAS las rutas menos /health exigen "
                         "Authorization: Bearer <clave>")
    ap.add_argument("--context", type=int, default=0,
                    help="contexto en tokens (0 = el que use cada engine)")
    ap.add_argument("--max-seq-len", type=int, default=0,
                    help="alias de --context con el nombre que usa airllm")
    ap.add_argument("--backend-port", type=int, default=0,
                    help="puerto INTERNO del backend en 127.0.0.1 "
                         "(default: puerto externo + 1000)")
    ap.add_argument("--backend-bin", default="",
                    help="ruta al ejecutable del backend (si no, se busca donde "
                         "ropero lo tiene: $BIN_DIR, venv de tensorrt, /opt/fastflowlm, PATH)")
    ap.add_argument("--rewrite-model", default="",
                    help="nombre al que se reescribe el campo 'model' del body "
                         "(vacío = el que conoce el backend: el id de ropero para "
                         "tensorrt, el TAG para flm)")
    ap.add_argument("--extra-arg", action="append", default=[],
                    help="argumento extra para el sub-proceso backend (repetible)")
    ap.set_defaults(backend_extra_args=[])    # Sampling del backend airllm (no aplica a los proxied).
    ap.add_argument("--max-new-tokens", type=int, default=4096)
    ap.add_argument("--temperature", type=float, default=0.0)
    ap.add_argument("--top-p", type=float, default=1.0)
    ap.add_argument("--compression", default="none", choices=["none", "4bit", "8bit"],
                    help="airllm: 4bit es ~4x más rápido. Exige bitsandbytes")
    ap.add_argument("--layer-shards", default="",
                    help="airllm: dónde guardar las capas troceadas (ponlo en SSD rápido)")
    ap.add_argument("--no-prefetch", action="store_true",
                    help="airllm: no prefetchear la capa siguiente")
    args = ap.parse_args()

    # --context y --max-seq-len son el mismo dato con dos nombres.
    context = args.max_seq_len or args.context
    if args.backend == "airllm" and not context:
        # El default de airllm es 512 tokens: inútil. Sin contexto explícito
        # se pone uno razonable.
        context = DEFAULT_CONTEXT
    args.context = context
    if not args.backend_port:
        args.backend_port = args.port + INTERNAL_PORT_OFFSET

    # Antes de tocar nada: sin fastapi/uvicorn no hay servidor que servir.
    if _IMPORT_ERROR is not None:
        sys.stderr.write("ERROR: faltan dependencias para el endpoint de ropero: %s\n"
                         % _IMPORT_ERROR)
        sys.stderr.write("pip install fastapi uvicorn\n")
        sys.stderr.flush()
        return 2

    if args.backend == "airllm":
        # airllm es una biblioteca: si no está, que lo diga CLARO y no con un
        # traceback. find_spec no ejecuta el módulo, así que es barato.
        import importlib.util
        try:
            spec = importlib.util.find_spec("airllm")
        except (ImportError, ValueError):
            spec = None
        if spec is None:
            sys.stderr.write("ERROR: el backend airllm necesita la biblioteca 'airllm' "
                             "instalada en este intérprete.\n")
            sys.stderr.write("  pip install airllm   (y transformers<5)\n")
            sys.stderr.flush()
            return 2
        if not os.path.exists(args.model_path):
            sys.stderr.write("ERROR: el modelo no existe: %s\n" % args.model_path)
            sys.stderr.flush()
            return 2

    if args.backend in ("tensorrt", "flm", "mlx"):
        # El nombre que el backend conoce. Por defecto:
        #   tensorrt → el id de ropero (lo que se le pasa como --served_model_name).
        #   flm      → el TAG: FLM valida el campo "model" (su error es
        #              "Model X is not supported."), así que el id de ropero
        #              no lo aceptaría.
        if not args.rewrite_model:
            args.rewrite_model = (args.model_id if args.backend == "tensorrt"
                                   else args.model_path)

    if not args.api_key:
        log("AVISO: sin --api-key el endpoint queda ABIERTO en %s:%d "
            "(cualquiera de la red puede usarlo)" % (args.host, args.port))

    # ── Los args del modelo (los de su TOML) ────────────────────────────────
    # `ropero` pasa los `[args].base` del TOML del modelo con `--extra-arg`,
    # uno por flag. Para tensorrt y flm son flags del backend nativo y se
    # añaden tal cual a su argv. Para airllm, que no es un proceso sino una
    # biblioteca, se traducen a los parámetros de carga del modelo.
    apply_extra_args(args)

    if args.backend == "airllm":
        backend = AirLLMBackend(args)
    else:
        if args.backend == "tensorrt":
            builder = tensorrt_argv
            label = "tensorrt-llm"
        elif args.backend == "mlx":
            builder = mlx_argv
            label = "mlx_lm"
        else:
            builder = flm_argv
            label = "fastflowlm"
        backend = ProxyBackend(args, args.backend, label, builder)
    app = build_app(args, backend)

    log("endpoint ropero: backend=%s model=%s en http://%s:%d/v1 (auth=%s)"
        % (args.backend, args.model_id, args.host, args.port,
           "on" if args.api_key else "off"))

    try:
        # Un solo worker: con el backend proxied el modelo vive en el
        # sub-proceso, y con airllm el modelo vive en ESTE proceso (no se
        # comparte entre workers sin recargarlo todo).
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
    finally:
        # El lifespan de FastAPI ya paró al backend, pero si uvicorn murió
        # antes de llegar al shutdown, aquí se garantiza.
        backend.stop()
    return 0


if __name__ == "__main__":
    sys.exit(main())
