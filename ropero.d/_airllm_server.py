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
# USO
#   airllm-server --model ~/models/safetensors/gpt-oss-120b \
#                  --model-id airgptoss --host 0.0.0.0 --port 9999 \
#                  --max-seq-len 131072 --compression 4bit --api-key sk-...
# =============================================================================

import argparse
import json
import os
import sys
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# Un lock global y no uno por request: airllm mueve el modelo entero capa a
# capa por la GPU/RAM. Dos generaciones a la vez no es "más rápido", es
# "el mismo throughput repartido y el doble de swap".
_LOCK = threading.Lock()

_MODEL = None
_TOKENIZER = None
_HF_STYLE = True     # True → GenerationMixin (HF contract). False → AirLLMLlamaMlx
_ARGS = None


def log(msg):
    sys.stderr.write("[airllm-server %s] %s\n" % (time.strftime("%H:%M:%S"), msg))
    sys.stderr.flush()


# -----------------------------------------------------------------------------
# Carga
# -----------------------------------------------------------------------------
def load_backend():
    """Carga el modelo una vez al arrancar.

    Se hace en el arranque y no en el primer request a propósito: si fallase,
    es en el arranque donde quieres enterarte, con el stack completo, y no
    tres minutos después con un opencode esperando el primer token.
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
    n_in = ids["input_ids"].shape[-1]

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


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "airllm-server"
    sys_version = ""

    def log_message(self, fmt, *args):
        pass

    def _authed(self):
        want = _ARGS.api_key
        if not want:
            return True
        h = self.headers.get("Authorization", "")
        return h.startswith("Bearer ") and h[7:].strip() == want

    def _json(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _body(self):
        n = int(self.headers.get("Content-Length") or 0)
        if not n:
            return {}
        try:
            return json.loads(self.rfile.read(n))
        except ValueError:
            return {}

    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path == "/health":
            self._json(200, {"status": "ok", "engine": "airllm"})
            return
        if path == "/v1/models":
            if not self._authed():
                self._json(401, {"error": {"message": "invalid api key"}})
                return
            self._json(200, {"object": "list", "data": [{
                "id": _ARGS.model_id,
                "object": "model",
                "created": int(os.path.getmtime(_ARGS.model)
                               if os.path.exists(_ARGS.model) else time.time()),
                "owned_by": "ropero/airllm",
            }]})
            return
        if path == "/ropero/engine":
            # Ruta de diagnóstico para `_collect_slots`: sin ella, la columna
            # ENGINE del status tendría que deducir el engine del nombre del
            # proceso, que aquí es un `python3` y no dice nada. Los engines sin
            # auth propia (mlx_lm, trtllm-serve, flm) la tienen a través del
            # proxy; airllm sirve en el puerto de ropero directamente, así que
            # la publica él mismo.
            self._json(200, {
                "engine": "airllm",
                "model": _ARGS.model_id,
                "target_host": self.server.server_address[0],
                "target_port": self.server.server_address[1],
                "child_pid": 0,
                "child_alive": True,
            })
            return
        self._json(404, {"error": {"message": "no such route: %s" % path}})

    def do_POST(self):
        path = self.path.split("?", 1)[0]
        if not self._authed():
            self._json(401, {"error": {"message": "invalid api key"}})
            return
        if path == "/v1/completions":
            payload = self._body()
            prompt = payload.get("prompt", "")
            if isinstance(prompt, list):
                prompt = prompt[0] if prompt else ""
            messages = [{"role": "user", "content": _msg_text(prompt)}]
            return self._chat(messages, payload)
        if path in ("/v1/chat/completions", "/v1/responses"):
            payload = self._body()
            if path == "/v1/responses":
                # /v1/responses lleva `input` en vez de `messages`.
                payload.setdefault("messages", payload.pop("input", []))
            msgs = [{"role": m.get("role", "user"),
                     "content": _msg_text(m.get("content", ""))}
                    for m in payload.get("messages", [])]
            return self._chat(msgs, payload)
        self._json(404, {"error": {"message": "no such route: %s" % path}})

    # ── el chat de verdad ──────────────────────────────────────────────────
    def _chat(self, messages, payload):
        if not messages:
            return self._json(400, {"error": {"message": "messages vacío"}})
        if payload.get("tools"):
            # airllm no tiene template de herramientas: un endpoint de texto que
            # finge soportarlas hace que el cliente se cuelgue esperando un
            # tool_call que no va a llegar. Se dice explícitamente.
            return self._json(400, {"error": {
                "message": "airllm no implementa tool calling (no es un motor "
                           "de inferencia, es un offloader de capas). Las tools "
                           "del request se ignorarían y el cliente colgaría."}})

        cid = "chatcmpl-" + uuid.uuid4().hex[:24]
        created = int(time.time())
        model_id = _ARGS.model_id
        want_stream = bool(payload.get("stream"))

        if want_stream:
            return self._stream(cid, created, model_id, messages)

        try:
            with _LOCK:
                text, ntok = generate_text(messages, _ARGS)
        except Exception as e:  # noqa: BLE001 — al cliente le sirve el motivo
            log("error generando: %r" % (e,))
            return self._json(500, {"error": {
                "message": "%s: %s" % (type(e).__name__, e), "type": "server_error"}})

        self._json(200, {
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
        })

    def _stream(self, cid, created, model_id, messages):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()

        def send(obj):
            payload = ("data: %s\n\n" % json.dumps(obj)).encode()
            self.wfile.write(b"%x\r\n" % len(payload) + payload + b"\r\n")
            self.wfile.flush()

        try:
            with _LOCK:
                for piece in stream_text(messages, _ARGS):
                    send({"id": cid, "object": "chat.completion.chunk",
                          "created": created, "model": model_id,
                          "choices": [{"index": 0,
                                       "delta": {"content": piece},
                                       "finish_reason": None}]})
            send({"id": cid, "object": "chat.completion.chunk",
                  "created": created, "model": model_id,
                  "choices": [{"index": 0, "delta": {},
                               "finish_reason": "stop"}]})
        except Exception as e:  # noqa: BLE001
            log("error en stream: %r" % (e,))
            send({"error": {"message": "%s: %s" % (type(e).__name__, e),
                            "type": "server_error"}})
        finally:
            send({"done": True})
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()


def main():
    ap = argparse.ArgumentParser(
        description="Servidor OpenAI-compatible sobre airllm (lo que airllm no trae)")
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

    class Server(ThreadingHTTPServer):
        daemon_threads = True
        allow_reuse_address = True

    srv = Server((args.host, args.port), Handler)
    log("OpenAI-compatible en http://%s:%d/v1 (model=%s, auth=%s)"
        % (args.host, args.port, args.model_id, "on" if args.api_key else "off"))
    try:
        srv.serve_forever(poll_interval=0.5)
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())