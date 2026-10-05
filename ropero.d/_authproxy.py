#!/usr/bin/env python3
# =============================================================================
# ropero.d/_authproxy.py — proxy de autenticación OpenAI-compatible
# =============================================================================
# POR QUÉ EXISTE
#
# ropero expone todos sus engines con la MISMA API_KEY, IP y PUERTO, para que
# opencode.jsonc no tenga que saber qué engine hay detrás. Eso funciona gratis
# con llama-server, que trae `--api-key`. Con el resto no:
#
#   mlx_lm.server   NO tiene --api-key  (0.31.3, verificado en su --help y en
#                                       su server.py: ni flag ni variable de env)
#   trtllm-serve    NO tiene --api_key  (serve.py de TensorRT-LLM 1.x; el
#                                       api_key del cliente OpenAI es un campo
#                                       del SDK, no una comprobación del server)
#   flm serve       NO tiene auth       (su propia doc: "the key is not checked")
#
# Con HOST=0.0.0.0 y sin auth, esos tres quedarían abiertos a la LAN. Este proxy
# es lo que cierra esa puerta: es el ÚNICO proceso que escucha en la IP pública
# de ropero, exige el Bearer token, y el engine real escucha en 127.0.0.1 en un
# puerto interno que nadie de fuera conoce.
#
# POR QUÉ ESTEproxy ES PARENT DEL ENGINE
#
# `ropero stop` mata el PID que escucha en el puerto. Si el engine escuchara en
# ese puerto, moriría el engine y el proxy se quedaría huérfano reenviando a un
# puerto muerto, occupying el puerto para siempre. Con el proxy como padre:
# matar el puerto → muere el proxy → el proxy mata a su hijo → puerto libre.
# Un árbol de procesos por slot, con una sola Raíz que ropero puede matar.
#
# SIN DEPENDENCIAS
#
# Solo biblioteca estándar, a propósito: este script tiene que arrancar en
# cualquier Python ≥3.8 del sistema, incluido el del venv de tensorrt o el de
# asdf, sin instalar httpx/fastapi/uvicorn en ninguno de ellos.
#
# USO (lo llama start_model, no se usa a mano normalmente)
#   _authproxy.py --host 0.0.0.0 --port 9992 --target-port 10992 \
#                 --api-key "$LLAMA_API_KEY" --model-id metaloss --engine mlx_lm \
#                 --engine-cmd-file /tmp/argv --pid-file /tmp/x.pid \
#                 --rewrite-model default_model
#
# POR QUÉ `--rewrite-model`
#
# Hay engines que no aceptan el nombre de modelo que el cliente manda. mlx_lm
# es el caso: su ModelProvider registra un único nombre, "default_model", y
# cualquier otra cosa la trata como un repo de HuggingFace. El síntoma es un
# 404 cuyo texto habla de "Repository Not Found" y hace creer que falta la
# descarga, cuando el modelo está en disco y cargado. Como /v1/models anuncia
# el id de ropero (arriba), sin reescribir el proxy estaría anunciando un
# nombre que su propio engine rechaza. Ver Handler._rewrite_model_field.
#
# Sólo se pasa para los engines que lo necesitan (ropero_engine_pinned_model);
# con el flag vacío el body se reenvía byte a byte, como antes.
#
# RUTAS QUE NO SE REENVÍAN AL ENGINE (se responden aquí, y por eso no dependen
# de que el engine las implemente):
#   /v1/models      → id = --model-id. Es lo que hace que `ropero stop <modelo>`
#                     y la columna MODELO del status funcionen igual para todos
#                     los engines, sin que cada uno tenga que saber llamarse
#                     como el modelo de ropero.
#   /ropero/engine  → {"engine": "...", "model": "...", "target_port": N}. Lo
#                     usa _collect_slots para la columna ENGINE: el nombre del
#                     proceso del proxy es "python3", inservible para eso.
#   /health         → open, sin auth (diagnóstico).
# =============================================================================

import argparse
import errno
import http.client
import json
import os
import signal
import socket
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# Cabeceras que no se reenvían nunca: son salto-a-salto (RFC 7230 §6.1) o
# las fija el propio transporte.
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
    "content-length",  # lo recalculamos nosotros
}

STATE = {
    "model_id": "",
    "engine": "",
    "api_key": "",
    "target_host": "127.0.0.1",
    "target_port": 0,
    "child_pid": 0,
    "child": None,
    "shutdown": False,
    "server_port": 0,
    # Nombre al que se reescribe el campo "model" del body ("" = no se toca).
    # Ver _rewrite_model_field() para por qué hace falta.
    "rewrite_model": "",
    "rewrite_logged": False,
}


def log(msg):
    """Log a stderr con timestamp. Nunca a stdout: stdout lo consume ropero."""
    sys.stderr.write("[authproxy %s] %s\n" % (time.strftime("%H:%M:%S"), msg))
    sys.stderr.flush()


class Handler(BaseHTTPRequestHandler):
    # HTTP/1.1 para poder hacer keep-alive y para que los clientes de streaming
    # (SSE) no se quejen de que cortamos la conexión a mitad de un chunk.
    protocol_version = "HTTP/1.1"
    server_version = "ropero-authproxy"
    sys_version = ""

    # ── util ────────────────────────────────────────────────────────────────
    def log_message(self, fmt, *args):  # silencia el log a stdout de http.server
        pass

    def _authed(self):
        """Bearer / X-Api-Key contra la API key de ropero."""
        want = STATE["api_key"]
        if not want:
            return True
        hdr = self.headers.get("Authorization", "")
        if hdr.startswith("Bearer "):
            if hdr[7:].strip() == want:
                return True
        # /v1/embeddings y algunos clientes migros usan X-Api-Key.
        if self.headers.get("X-Api-Key", "").strip() == want:
            return True
        return False

    def _deny(self):
        body = json.dumps(
            {"error": {"message": "invalid api key", "type": "invalid_request_error"}}
        ).encode()
        self.send_response(401)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("WWW-Authenticate", 'Bearer realm="ropero"')
        self.end_headers()
        self.wfile.write(body)

    def _send_json(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _child_alive(self):
        """¿Sigue vivo el engine?

        Si no se lanzó ningún hijo desde este proxy, el engine es EXTERNO
        (ya escuchando, o alguien lo arrancó a mano): no hay nada que mirar y
        que sea el intento de conexión el que decida. Devolver False a ciegas
        en ese caso convertiría todo request en un 502 que no dice la verdad.
        """
        ch = STATE["child"]
        if ch is None:
            return True
        return ch.poll() is None

    # ── rutas locales ───────────────────────────────────────────────────────
    def _local_routes(self):
        path = self.path.split("?", 1)[0]
        if path == "/health":
            # Deliberadamente SIN auth: es un chequeo de vida, no información.
            # Y lo abre `ropero` antes de que el engine haya terminado de
            # cargar, que es justo cuando más lo necesitas.
            self._send_json(200, {"status": "ok"})
            return True
        if path == "/v1/models":
            if not self._authed():
                self._deny()
                return True
            self._send_json(
                200,
                {
                    "object": "list",
                    "data": [
                        {
                            "id": STATE["model_id"],
                            "object": "model",
                            "created": int(time.time()),
                            "owned_by": "ropero/%s" % STATE["engine"],
                        }
                    ],
                },
            )
            return True
        # Interceptar peticiones a HuggingFace para evitar que el engine intente
        # descargar el modelo. mlx_lm.server hace una llamada interna a
        # huggingface.co/api/models/<repo_id>/revision/main para verificar el repo,
        # y si el nombre no es un repo válido de HF, falla con 401.
        # Respondemos con un JSON válido de HF para que el engine use el path local.
        if path.startswith("/api/models/"):
            # Extraer el repo_id del path (ej: /api/models/gptoss/revision/main)
            parts = path.split("/")
            if len(parts) >= 3:
                repo_id = parts[2]
                # Responder con un JSON válido de HuggingFace
                self._send_json(
                    200,
                    {
                        "id": repo_id,
                        "modelId": repo_id,
                        "pipeline_tag": "text-generation",
                        "library_name": "mlx",
                        "private": False,
                        "sha": "0000000000000000000000000000000000000000",
                        "downloads": 0,
                        "likes": 0,
                        "tags": ["mlx", "text-generation"],
                        "siblings": [],
                    },
                )
                return True
        if path == "/ropero/engine":
            # Sin auth a propósito: lo consume `_collect_slots` en la misma
            # máquina. Si se exponía el proxy a la LAN, esto solo diría qué
            # engine hay detrás — que ya se deduce del puerto y del proceso.
            # Aun así se filtra por si algún día se pone detrás de TLS.
            self._send_json(
                200,
                {
                    "engine": STATE["engine"],
                    "model": STATE["model_id"],
                    "target_host": STATE["target_host"],
                    "target_port": STATE["target_port"],
                    "child_pid": STATE["child_pid"],
                    "child_alive": self._child_alive(),
                },
            )
            return True
        return False

    # ── reenvío ─────────────────────────────────────────────────────────────
    def _dispatch(self):
        """Punto de entrada de TODOS los verbos.

        La comprobación de rutas locales va AQUÍ y no dentro de _forward, y no
        por limpieza: si se pusiera dentro, y además `do_GET = _forward`
        apuntara directamente a _forward, las rutas locales no se consultarían
        nunca para GET/POST y /v1/models iría al engine — que es justo la ruta
        que el proxy existe para interceptar. Un solo camino, sin alias.
        """
        if self._local_routes():
            return
        self._forward()

    def _rewrite_model_field(self, body):
        """Reescribe el campo "model" del body al nombre que el engine EXIGE.

        ── POR QUÉ EXISTE ESTO ──────────────────────────────────────────────
        `mlx_lm.server` sólo acepta UN nombre de modelo: "default_model".
        En su `ModelProvider.__init__` el mapa es `{"default_model": args.model}`
        y `load()` hace `self._model_map.get(model_path, model_path)`: lo que no
        está en el mapa se usa TAL CUAL, como si fuera una ruta o un repo_id de
        HuggingFace. Y lo es. Con un cliente que manda "gptoss", huggingface_hub
        pide `api/models/gptoss/revision/main`, recibe 401 y el engine responde
        404 con un error que MIENTE sobre la causa: dice "Repository Not Found"
        cuando el repositorio no tiene nada que ver y lo que pasa es que el
        nombre no es el que el engine conoce. Medido en 0.32.x con gpt-oss-20b
        ya cargado en memoria:

            model="gptoss"         → 404 + GET huggingface.co/api/models/gptoss
            model="metaloss"       → 404 + GET huggingface.co/api/models/metaloss
            model="default_model"  → 200

        No hay flag para arreglarlo en el engine: `mlx_lm.server --help` no tiene
        `--alias`, ni `--served-model-name`, ni `--model-name` (a diferencia de
        llama-server `--alias`, airllm `--model-id` y trtllm-serve
        `--served_model_name`, que sí fijan el nombre que aceptan). Por eso lo
        arregla el proxy, que es el componente que ropero ya pone delante de los
        engines sin auth y que YA reescribe la identidad del modelo en
        /v1/models: sin esto el proxy anuncia un id que su propio engine
        rechaza, y el cliente (opencode, cualquier SDK) no tiene forma de
        saberlo porque la API no ofrece "usa el único nombre que acepto".

        Decisiones:
        · Sólo POST. GET/HEAD no llevan body y nada que reescribir.
        · Body que no es JSON, o que no trae "model", se reenvía INTACTO. Que el
          field no exista no es un problema: mlx_lm hace
          `body.get("model", "default_model")`, así que ya cae en el bueno. No
          se inventan campos.
        · No se toca ninguna cabecera: "content-length" está en HOP_BY_HOP (que
          es exactamente para esto, ver arriba) y http.client lo recalcula a
          partir del body. Añadirlo aquí a mano duplicaría la cabecera.
        """
        pin = STATE["rewrite_model"]
        if not pin or self.command != "POST" or not body:
            return body
        try:
            doc = json.loads(body)
        except Exception:
            return body  # no es JSON (multipart, audio, ...): sin tocar
        if not isinstance(doc, dict):
            return body
        got = doc.get("model")
        if not isinstance(got, str) or got == pin:
            return body
        doc["model"] = pin
        # Una vez por proceso, no por request: si no, cada stream de opencode
        # deja una línea de ruido en el log del modelo.
        if not STATE["rewrite_logged"]:
            STATE["rewrite_logged"] = True
            log("model '%s' -> '%s' (el engine sólo acepta ese nombre)" % (got, pin))
        return json.dumps(doc).encode("utf-8")

    def _forward(self):
        if not self._authed():
            self._deny()
            return

        if not self._child_alive():
            self._send_json(
                502,
                {
                    "error": {
                        "message": "engine %s no está corriendo (se ha salido)"
                        % STATE["engine"],
                        "type": "upstream_unavailable",
                    }
                },
            )
            return

        # Cuerpo de la petición.
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else None
        body = self._rewrite_model_field(body)

        # Cabeceras hacia arriba. Se quita Authorization a propósito: el engine
        # no la valida (por eso existe el proxy), así que reenviar una credencial
        # a un componente que no la necesita solo amplia la superficie en caso
        # de que ese engine algún día la registrara en un log.
        up = {}
        for k, v in self.headers.items():
            if k.lower() in HOP_BY_HOP or k.lower() == "authorization":
                continue
            up[k] = v

        conn = None
        try:
            conn = http.client.HTTPConnection(
                STATE["target_host"], STATE["target_port"], timeout=None
            )
            conn.request(self.command, self.path, body=body, headers=up)
            resp = conn.getresponse()

            self.send_response(resp.status)
            chunked = False
            for k, v in resp.getheaders():
                lk = k.lower()
                if lk in HOP_BY_HOP:
                    if lk == "transfer-encoding" and "chunked" in v.lower():
                        chunked = True
                    continue
                self.send_header(k, v)

            if resp.getheader("Transfer-Encoding", "").lower() == "chunked":
                chunked = True

            if chunked:
                # SSE / streaming: no hay Content-Length. Reempaquetamos en
                # chunks nosotros para poder cerrar limpio y no depender del
                # cierre de conexión del engine.
                self.send_header("Transfer-Encoding", "chunked")
                self.end_headers()
                while True:
                    buf = resp.read(8192)
                    if not buf:
                        break
                    self.wfile.write(b"%x\r\n" % len(buf) + buf + b"\r\n")
                    self.wfile.flush()
                self.wfile.write(b"0\r\n\r\n")
                self.wfile.flush()
            else:
                data = resp.read()
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                if self.command != "HEAD" and data:
                    self.wfile.write(data)
                    self.wfile.flush()
        except (ConnectionRefusedError, socket.timeout, OSError) as e:
            # 502 con motivo: durante el arranque la primera petición suele
            # llegar antes de que el engine esté escuchando, y un "502 opaque"
            # no dice si el problema es el modelo o el puerto.
            log("upstream %s:%s no responde (%s)" % (
                STATE["target_host"], STATE["target_port"], e))
            try:
                self._send_json(
                    502,
                    {
                        "error": {
                            "message": "no se pudo contactar con el engine %s en %s:%s (%s)"
                            % (STATE["engine"], STATE["target_host"],
                               STATE["target_port"], e),
                            "type": "upstream_unavailable",
                        }
                    },
                )
            except OSError:
                pass
        finally:
            if conn is not None:
                try:
                    conn.close()
                except OSError:
                    pass

    do_GET = _dispatch
    do_POST = _dispatch
    do_PUT = _dispatch
    do_DELETE = _dispatch
    do_PATCH = _dispatch
    do_HEAD = _dispatch

    def handle_one_request(self):
        try:
            BaseHTTPRequestHandler.handle_one_request(self)
        except (BrokenPipeError, ConnectionResetError):
            # El cliente se fue a mitad de stream. No es un error nuestro.
            pass


def wait_for_target(host, port, seconds):
    """Espera a que el engine acepte conexiones antes de abrir el proxy.

    Sin esto hay una ventana en la que el puerto de ropero está abierto pero
    todo request devuelve 502. `ropero status` la vería como "ON con modelo ?".
    """
    deadline = time.time() + seconds
    while time.time() < deadline:
        if STATE["child"] is not None and STATE["child"].poll() is not None:
            return False
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(0.5)
        try:
            s.connect((host, port))
            return True
        except OSError:
            time.sleep(0.25)
        finally:
            s.close()
    return False


def spawn_engine(argv):
    """Lanza el engine como hijo y guarda su PID.

    Sin `start_new_session`: los hijos se quedan en el process group del proxy
    para que un Ctrl+C en primer plano los baje a los dos. Con sesión propia,
    el Ctrl+C del terminal solo mataría al proxy y dejaría al engine huyendo.
    """
    log("lanzando engine: %s" % " ".join(argv))
    ch = subprocess.Popen(
        argv,
        stdout=sys.stderr,   # el log del engine va al logfile de ropero
        stderr=sys.stderr,
    )
    STATE["child"] = ch
    STATE["child_pid"] = ch.pid
    return ch


def main():
    ap = argparse.ArgumentParser(description="proxy de auth OpenAI para ropero")
    ap.add_argument("--host", default="0.0.0.0")
    ap.add_argument("--port", type=int, required=True)
    ap.add_argument("--target-host", default="127.0.0.1")
    ap.add_argument("--target-port", type=int, required=True)
    ap.add_argument("--api-key", default="")
    ap.add_argument("--model-id", default="")
    ap.add_argument(
        "--rewrite-model",
        default="",
        help="reescribe el campo 'model' del body a este valor (engines que sólo "
        "aceptan un nombre fijo, como mlx_lm -> default_model). Vacío = no tocar",
    )
    ap.add_argument("--engine", default="?")
    ap.add_argument("--engine-cmd-file", default="",
                    help="fichero con el argv del engine, un argumento por línea")
    ap.add_argument("--pid-file", default="")
    ap.add_argument("--wait", type=int, default=0,
                    help="segundos esperando a que el engine acepte conexiones")
    args = ap.parse_args()

    STATE["model_id"] = args.model_id or "ropero-model"
    STATE["rewrite_model"] = args.rewrite_model
    STATE["engine"] = args.engine
    STATE["api_key"] = args.api_key
    STATE["target_host"] = args.target_host
    STATE["target_port"] = args.target_port
    STATE["server_port"] = args.port

    if args.engine_cmd_file:
        with open(args.engine_cmd_file, "r", encoding="utf-8") as fh:
            argv = [ln.rstrip("\n") for ln in fh if ln.strip()]
        if not argv:
            log("ERROR: --engine-cmd-file vacío")
            return 2
        for key, val in _env_from_engine():
            os.environ[key] = val
        spawn_engine(argv)

    if args.pid_file:
        with open(args.pid_file, "w", encoding="utf-8") as fh:
            fh.write("%d\n" % os.getpid())

    def on_signal(signum, _frame):
        # Vamos a morir: el engine también. Si no, se queda sirviendo en un
        # puerto al que ya nadie llega, quemando la GPU/RAM/NPU.
        #
        # El guard es necesario, no cosmético: el signal handler y el `finally`
        # de serve_forever acaban en esta misma función, así que sin él el
        # log sale duplicado y se intenta parar un hijo ya parado.
        if STATE["shutdown"]:
            return
        STATE["shutdown"] = True
        ch = STATE["child"]
        if ch is not None and ch.poll() is None:
            try:
                ch.terminate()
                try:
                    ch.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    log("el engine no respeta SIGTERM; SIGKILL")
                    ch.kill()
            except OSError as e:
                if e.errno != errno.ESRCH:
                    log("no se pudo parar el engine: %s" % e)
        try:
            if args.pid_file and os.path.exists(args.pid_file):
                os.unlink(args.pid_file)
        except OSError:
            pass
        sys.exit(0)

    signal.signal(signal.SIGTERM, on_signal)
    signal.signal(signal.SIGINT, on_signal)

    if args.wait > 0:
        if not wait_for_target(args.target_host, args.target_port, args.wait):
            if STATE["child"] is not None and STATE["child"].poll() is not None:
                log("el engine murió durante el arranque (rc=%s)"
                    % STATE["child"].returncode)
                on_signal(signal.SIGTERM, None)
                return 1
            log("aviso: el engine no escucha en %s:%s tras %ds; abro igualmente"
                % (args.target_host, args.target_port, args.wait))

    class Server(ThreadingHTTPServer):
        # daemon_threads: un stream SSE abierto no puede impedir que el proceso
        # muera. Sin esto, `ropero stop` dejaría el puerto en uso.
        daemon_threads = True
        allow_reuse_address = True

    try:
        srv = Server((args.host, args.port), Handler)
    except OSError as e:
        log("no se pudo escuchar en %s:%s (%s)" % (args.host, args.port, e))
        on_signal(signal.SIGTERM, None)
        return 1

    log("escuchando en %s:%s -> %s:%s (engine=%s model=%s)"
        % (args.host, args.port, args.target_host, args.target_port,
           args.engine, STATE["model_id"]))

    # El engine puede morirse solo (OOM, crash). Si es así el puerto tiene que
    # liberarse: si no, `ropero` vería el slot ocupado por un proxy que no
    # sirve de nada y `--force` no podría rotarlo.
    watcher = threading.Thread(target=_watch_child, args=(srv,), daemon=True)
    watcher.start()

    try:
        srv.serve_forever(poll_interval=0.5)
    except KeyboardInterrupt:
        on_signal(signal.SIGINT, None)
    finally:
        on_signal(signal.SIGTERM, None)
    return 0


def _env_from_engine():
    """K=V lines from ROPERO_ENGINE_ENV, para el sub-proceso del engine.

    Así el launcher no tiene que distinguir entre "exportar en el proxy" y
    "exportar en el engine": se lo pasa al proxy y lo exporta él, porque el
    engine es su hijo y hereda su environment.
    """
    for ln in os.environ.get("ROPERO_ENGINE_ENV", "").split("\n"):
        if "=" in ln:
            k, v = ln.split("=", 1)
            if k:
                yield k.strip(), v


def _watch_child(srv):
    ch = STATE["child"]
    if ch is None:
        return
    rc = ch.wait()
    if STATE["shutdown"]:
        return
    log("el engine %s terminó (rc=%s); libero el puerto %d"
        % (STATE["engine"], rc, STATE["server_port"]))
    srv.shutdown()


if __name__ == "__main__":
    sys.exit(main())