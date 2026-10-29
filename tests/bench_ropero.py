#!/usr/bin/env python3
"""
bench_ropero.py — banco de pruebas para modelos locales servidos por ropero.

Estructura:
  ~/test/<modelo>/                     config del modelo (la lee ropero)
  ~/test/<modelo>/work-<prueba>/       directorio de trabajo de opencode
  ~/test/resultados/<modelo>/          resultados por prueba + informe.md
  ~/test/resultados/juez/<modelo>.md   el paquete que ve el juez
  ~/test/resultados/resumen-global.*   tabla agregada

Uso:
  ./bench_ropero.py                    lista los modelos
  ./bench_ropero.py <m1> <m2> …        prueba esos
  ./bench_ropero.py all                todos
  FORCE=1 ./bench_ropero.py <m>        rehace
  DEBUG=1 ./bench_ropero.py <m>        sondeos de puerto/log y stderr si falla

Variables de entorno:
  MODELOS_DIR OUT_DIR PROMPTS_DIR
  AGENTE MODELO PUERTO API_KEY
  TEMP NIVEL TIMEOUT ESPERA_MIN FORCE DEBUG

Notas:
  · NIVEL="" desactiva --<nivel>.
  · Si :PUERTO sigue ocupado antes de arrancar: `ropero stop` y, si no basta,
    `pkill -f llama-server`. Si aún así no se libera, la prueba se marca como
    no arrancada (nunca como ok).
  · La espera NO se conforma con que responda /v1/models: exige un POST real
    a /v1/chat/completions con max_tokens=1. Así no se mide un servidor a
    medio cargar.
"""
import argparse
import fcntl
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import datetime
from pathlib import Path
from urllib.request import Request, urlopen

# ── Configuración ─────────────────────────────────────────────────────────

HOME         = Path.home()
MODELOS_DIR  = Path(os.environ.get("MODELOS_DIR", HOME / "test"))
OUT_DIR      = Path(os.environ.get("OUT_DIR", MODELOS_DIR / "resultados"))
PROMPTS_DIR  = Path(os.environ.get("PROMPTS_DIR", Path(__file__).resolve().parent))

AGENTE        = os.environ.get("AGENTE", "build")
MODELO_OC     = os.environ.get("MODELO", "marichocho/Puertourraco-256k")
PUERTO        = int(os.environ.get("PUERTO", "9999"))
API_KEY       = os.environ.get("API_KEY", "sk-local-dev-key")
TIMEOUT       = int(os.environ.get("TIMEOUT", "1800"))     # por prueba
ESPERA_MIN    = int(os.environ.get("ESPERA_MIN", "900"))    # carga en frío
TEMP          = os.environ.get("TEMP", "0.15")
NIVEL         = os.environ.get("NIVEL", "high")
FORCE         = os.environ.get("FORCE", "0") == "1"
DEBUG         = os.environ.get("DEBUG", "0") == "1"
FORCE_MODELOS = set(os.environ.get("FORCE_MODELO", "").split())

PRUEBAS = [
    {"id": "prueba1",      "file": "prueba1.md",      "split": False},
    {"id": "prueba2",      "file": "prueba2.md",      "split": False},
    {"id": "prueba3",      "file": "prueba3.md",      "split": False},
    {"id": "prueba_larga", "file": "prueba_larga.md", "split": True},
]
PROMPT_JUEZ = "juez.md"

# ── Colores y logging ─────────────────────────────────────────────────────

if sys.stdout.isatty():
    R, G, Y, B, D, N = ("\033[31m", "\033[32m", "\033[33m",
                        "\033[1m", "\033[2m", "\033[0m")
else:
    R = G = Y = B = D = N = ""

def info(m): print(f"{D}{m}{N}", flush=True)
def warn(m): print(f"{Y}[!] {m}{N}", file=sys.stderr, flush=True)
def err(m):  print(f"{R}[x] {m}{N}", file=sys.stderr, flush=True)
def ok(m):   print(f"{G}[+] {m}{N}", flush=True)
def head2(m): print(f"\n{B}══ {m}{N}", flush=True)


# ── ropero / opencode ─────────────────────────────────────────────────────

def binario(nombre):
    return shutil.which(nombre) is not None


def ropero_list():
    try:
        r = subprocess.run(["ropero", "list"], capture_output=True,
                           text=True, timeout=15)
        return r.stdout if r.returncode == 0 else ""
    except Exception:
        return ""


def ctx_de_modelo(modelo):
    for line in ropero_list().splitlines():
        if line.startswith(modelo + " ") or line.startswith(modelo + "\t"):
            for tok in line.split():
                if tok.startswith("ctx="):
                    try:    return int(tok[4:])
                    except ValueError: return 0
    return 0


def nivel_flag(modelo):
    """'--high' si ropero levels lo declara, si no ''."""
    if not NIVEL:
        return ""
    try:
        r = subprocess.run(["ropero", "levels", modelo],
                           capture_output=True, text=True, timeout=10)
        if NIVEL in r.stdout.split():
            return f"--{NIVEL}"
    except Exception:
        pass
    return ""


def modelo_en_puerto():
    try:
        req = Request(f"http://127.0.0.1:{PUERTO}/v1/models",
                      headers={"Authorization": f"Bearer {API_KEY}"})
        with urlopen(req, timeout=5) as r:
            data = json.load(r)
            if data.get("data"):
                return data["data"][0].get("id")
    except Exception:
        pass
    return None


def _puerto_ocupado():
    return modelo_en_puerto() is not None


def completion_ok(timeout=20):
    """POST real a /v1/chat/completions con max_tokens=1.
    True solo si el modelo responde 200."""
    mid = modelo_en_puerto()
    if not mid:
        return False
    body = json.dumps({
        "model": mid,
        "messages": [{"role": "user", "content": "x"}],
        "max_tokens": 1,
        "temperature": 0,
        "stream": False,
    }).encode()
    req = Request(
        f"http://127.0.0.1:{PUERTO}/v1/chat/completions",
        data=body,
        headers={
            "Authorization": f"Bearer {API_KEY}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urlopen(req, timeout=timeout) as r:
            if DEBUG:
                info(f"    completion: HTTP {r.status}")
            return 200 <= r.status < 300
    except Exception as e:
        if DEBUG:
            info(f"    completion: {type(e).__name__}: {e}")
        return False


def parar_modelo(modelo):
    try:
        subprocess.run(["ropero", "stop", modelo],
                       capture_output=True, timeout=30)
    except Exception:
        pass
    time.sleep(1)


def liberar_puerto(modelo):
    """Deja :PUERTO vacío. True si lo consigue."""
    parar_modelo(modelo)
    if not _puerto_ocupado():
        return True
    warn(f"  {modelo}: :{PUERTO} sigue ocupado tras 'ropero stop {modelo}'")
    try:
        subprocess.run(["pkill", "-f", "llama-server"],
                       capture_output=True, timeout=10)
    except Exception:
        pass
    for _ in range(10):
        time.sleep(1)
        if not _puerto_ocupado():
            return True
    return False


def arrancar_modelo(modelo, work_dir, log_path):
    """Arranca ropero en background. Devuelve (proc|None, motivo)."""
    if not liberar_puerto(modelo):
        return None, f":{PUERTO} no se pudo liberar"

    log_path.parent.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["ROPERO_TEMP"] = TEMP
    cmd = ["ropero", modelo]
    nf = nivel_flag(modelo)
    if nf:
        cmd.append(nf)
    info(f"    cmd: {' '.join(cmd)}")
    logf = open(log_path, "w")
    proc = subprocess.Popen(cmd, stdout=logf, stderr=subprocess.STDOUT,
                            cwd=str(work_dir), env=env, start_new_session=True)

    time.sleep(3)
    if proc.poll() is not None:
        try:
            tail = log_path.read_text(errors="replace")[-600:].strip()
        except Exception:
            tail = ""
        return None, f"ropero salió inmediatamente (rc={proc.returncode}):\n{tail}"

    try:
        txt = log_path.read_text(errors="replace")
    except Exception:
        txt = ""
    if "ya está corriendo" in txt or "already running" in txt.lower():
        return None, "ropero dice que ya estaba corriendo (no se puede medir en limpio)"

    return proc, ""


LOADED_RE = re.compile(
    r"(all slots are idle|server is listening|HTTP server listening|"
    r"model loaded|server is running)",
    re.I,
)
DEADLY_RE = re.compile(
    r"(error loading model|failed to load model|unknown option|"
    r"opción desconocida|cannot meet free memory target)",
    re.I,
)


def esperar_modelo(modelo, timeout, proc=None, log_path=None):
    """Espera a que el modelo esté REALMENTE listo.
    · proc.poll() para detectar muerte
    · log: marcadores de 'cargado' y de error fatal
    · completion real: POST /v1/chat/completions con max_tokens=1
    """
    deadline = time.time() + max(1, timeout)
    n = 0
    log_ready = False
    while time.time() < deadline:
        n += 1
        if proc is not None and proc.poll() is not None:
            err(f"    ropero murió durante la carga (rc={proc.returncode})")
            if log_path and log_path.exists():
                err(f"    log:\n{log_path.read_text(errors='replace')[-800:]}")
            return False

        if log_path and log_path.exists():
            try:
                txt = log_path.read_text(errors="replace")
            except Exception:
                txt = ""
            m = DEADLY_RE.search(txt)
            if m:
                err(f"    el log contiene «{m.group(0)}»: no va a cargar")
                return False
            if not log_ready and LOADED_RE.search(txt):
                log_ready = True
                if DEBUG:
                    info(f"    [{n}] log: modelo cargado")

        # Hasta que veamos el marcador o pasemos de 5 sondeos,
        # no lanzamos completion para no martillear el servidor.
        if log_ready or n >= 5:
            if completion_ok(timeout=20):
                info(f"    modelo listo (sondeo {n})")
                return True

        time.sleep(3)
    return False


def exportar_sesion(sid, out_path):
    try:
        with open(out_path, "w") as f:
            r = subprocess.run(["opencode", "session", "export", sid],
                               stdout=f, stderr=subprocess.DEVNULL,
                               timeout=120)
            return r.returncode == 0
    except Exception:
        return False


def extraer_session_id(events_path):
    if not events_path.exists():
        return None
    with open(events_path, errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not isinstance(ev, dict):
                continue
            for k in ("sessionID", "session_id", "sessionId"):
                v = ev.get(k)
                if isinstance(v, str) and v:
                    return v
            for wrap in ("properties", "data"):
                w = ev.get(wrap)
                if isinstance(w, dict):
                    for k in ("sessionID", "session_id", "sessionId"):
                        v = w.get(k)
                        if isinstance(v, str) and v:
                            return v
                    info_ = w.get("info")
                    if isinstance(info_, dict):
                        for k in ("sessionID", "session_id", "sessionId", "id"):
                            v = info_.get(k)
                            if isinstance(v, str) and v.startswith("ses"):
                                return v
    return None


def lanzar_opencode(prompt, work_dir, events_path, stderr_path,
                    session_id=None, timeout=TIMEOUT):
    """Ejecuta `opencode run` y guarda eventos + stderr. Devuelve (rc, duración)."""
    cmd = ["opencode", "run", prompt,
           "--model", MODELO_OC,
           "--agent", AGENTE,
           "--format", "json"]
    if session_id:
        cmd.extend(["--session", session_id])
    events_path.parent.mkdir(parents=True, exist_ok=True)
    work_dir.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    with open(events_path, "w") as outf, open(stderr_path, "w") as errf:
        try:
            r = subprocess.run(cmd, cwd=str(work_dir),
                               stdout=outf, stderr=errf,
                               timeout=timeout)
            rc = r.returncode
        except subprocess.TimeoutExpired:
            rc = -1
    dur = int(time.time() - t0)

    # diagnóstico si falla rápido (probable config error, no problema del modelo)
    if rc != 0 and dur < 5:
        try:
            er_txt = stderr_path.read_text(errors="replace")[-1000:]
            if er_txt.strip():
                err(f"    opencode falló rápido (rc={rc}) — stderr:\n{er_txt}")
            ev_txt = events_path.read_text(errors="replace")[-500:] \
                if events_path.exists() else ""
            if ev_txt.strip():
                err(f"    opencode events:\n{ev_txt}")
        except Exception:
            pass

    return rc, dur


# ── Métricas ─────────────────────────────────────────────────────────────

def metricas_de_log_llama(log_path):
    """Extrae prompt/eval t/s del log de llama-server."""
    prompt_speeds, gen_speeds = [], []
    if log_path.exists():
        with open(log_path, errors="replace") as f:
            for line in f:
                if "eval time" not in line:
                    continue
                m = re.search(r"([\d.]+)\s*tokens per second", line)
                if not m:
                    continue
                v = float(m.group(1))
                if "prompt eval" in line:
                    prompt_speeds.append(v)
                else:
                    gen_speeds.append(v)
    return {
        "prompt_samples": len(prompt_speeds),
        "gen_samples": len(gen_speeds),
        "prompt_max": max(prompt_speeds) if prompt_speeds else None,
        "prompt_avg": (sum(prompt_speeds) / len(prompt_speeds)
                       if prompt_speeds else None),
        "gen_max": max(gen_speeds) if gen_speeds else None,
        "gen_avg": (sum(gen_speeds) / len(gen_speeds)
                    if gen_speeds else None),
    }


def tokens_de_sesion(sesion_path):
    """Suma input/output/reasoning tokens de la sesión exportada."""
    result = {"in": 0, "out": 0, "reasoning": 0}
    if not sesion_path.exists():
        return result
    try:
        data = json.loads(sesion_path.read_text(errors="replace"))
    except Exception:
        return result
    if isinstance(data, dict):
        msgs = (data.get("messages")
                or data.get("parts")
                or data.get("events"))
    else:
        msgs = data
    if not isinstance(msgs, list):
        return result
    for m in msgs:
        if not isinstance(m, dict):
            continue
        u = m.get("usage") or m.get("tokens") or {}
        if not isinstance(u, dict):
            continue
        result["in"] += int(
            u.get("input_tokens") or u.get("prompt_tokens")
            or u.get("input") or u.get("prompt") or 0
        )
        result["out"] += int(
            u.get("output_tokens") or u.get("completion_tokens")
            or u.get("output") or u.get("completion") or 0
        )
        result["reasoning"] += int(
            u.get("reasoning_tokens") or u.get("reasoning") or 0
        )
    return result


def construir_metricas(llama_log, events_path, sesion_path, duracion, status):
    speed = metricas_de_log_llama(llama_log)
    tok   = tokens_de_sesion(sesion_path)
    return {
        "duracion_s":       duracion,
        "tokens_in":        tok["in"],
        "tokens_out":       tok["out"],
        "tokens_reasoning": tok["reasoning"],
        "prompt_max_tps":   speed["prompt_max"],
        "prompt_avg_tps":   speed["prompt_avg"],
        "gen_max_tps":      speed["gen_max"],
        "gen_avg_tps":      speed["gen_avg"],
        "prompt_samples":   speed["prompt_samples"],
        "gen_samples":      speed["gen_samples"],
        "status":           status,
    }


# ── Corpus largo determinista ─────────────────────────────────────────────

_CORPUS_CACHE = None

def generar_corpus():
    """Corpus Elixir determinista ~80 KB con 5 datos plantados."""
    global _CORPUS_CACHE
    if _CORPUS_CACHE is not None:
        return _CORPUS_CACHE

    parts = ["""# Tienda — aplicación Elixir/OTP de comercio electrónico
# Código fuente completo del proyecto.

defmodule Tienda.Config do
  @moduledoc "Configuración central de la tienda."

  # magicko: constante usada por el motor de precios.
  @magicko 0.0731
  def magicko, do: @magicko
  def get(key), do: Application.get_env(:tienda, key)
end
""", """defmodule Tienda.Queue do
  @moduledoc "Cola de trabajo interna basada en Broadway."

  # puerto en el que la cola de trabajo expone su API interna
  @port 7463
  def port, do: @port

  def child_spec(_), do: %{id: __MODULE__, start: {__MODULE__, :start_link, []}}
  def start_link, do: Broadway.start_link(Tienda.Queue.Pipeline, name: __MODULE__)
end
""", """defmodule Tienda.SessionCache do
  @moduledoc "Caché de sesiones con ETS."

  # TTL de la caché de sesiones, en segundos
  @ttl_seconds 847
  def ttl_seconds, do: @ttl_seconds

  def child_spec(_), do: %{id: __MODULE__, start: {__MODULE__, :start_link, []}}
  def start_link, do: GenServer.start_link(__MODULE__, [], name: __MODULE__)
  def init(_), do: {:ok, :ets.new(:sesiones, [:set, :public, :named_table])}
end
""", """defmodule Tienda.Activity do
  @moduledoc "Registro de actividad de usuarios."

  # tabla donde se registra la actividad
  @table :tienda_activity_log_2024
  def table, do: @table

  def log(user_id, action) do
    :ets.insert(@table, {System.system_time(:second), user_id, action})
  end
end
""", """defmodule Tienda.Bank.Reconciler do
  @moduledoc "Reconciliación de saldos con extractos bancarios."

  @doc \"\"\"
  Reconcilia el saldo local con el extracto del banco.
  Devuelve {:ok, ajuste} o {:error, motivo}.
  \"\"\"
  def reconcilia_saldo_local_con_extracto_banco(saldo_local, extracto) do
    diff = extracto - saldo_local
    if abs(diff) < 0.01, do: {:ok, 0}, else: {:ok, diff}
  end
end
"""]

    ops = ["create", "update", "delete", "find", "list",
           "count", "sum", "avg", "fetch", "store"]
    i = 0
    while len("\n\n".join(parts)) < 80_000:
        lines = [f"defmodule Tienda.Mod{i:03d} do",
                 f'  @moduledoc "Módulo auxiliar {i}."']
        for j in range(15):
            op = ops[(i + j) % len(ops)]
            lines += [f"  def {op}_{j}(x) do",
                      f"    # operación {j} del módulo {i}",
                      f"    x * {j + 1} + {i}",
                      f"  end"]
        lines.append("end")
        parts.append("\n".join(lines))
        i += 1

    _CORPUS_CACHE = "\n\n".join(parts)
    return _CORPUS_CACHE


def inyectar_corpus(texto):
    if "<!-- CORPUS:INICIO -->" not in texto:
        return texto
    pre  = texto.split("<!-- CORPUS:INICIO -->", 1)[0]
    post = texto.split("<!-- CORPUS:FIN -->", 1)[1]
    return pre + generar_corpus() + post


# ── Partir la prueba larga ───────────────────────────────────────────────

def split_bloques(prompt_file):
    """Divide el prompt por `## BLOQUE N`. Devuelve [(id, texto), …]."""
    text = prompt_file.read_text()
    matches = list(re.finditer(r"^##\s+(BLOQUE\s*\d+)", text, re.M))
    if not matches:
        return [("TODO", text)]
    out = []
    for i, m in enumerate(matches):
        start = m.start()
        end   = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        out.append((m.group(1).replace(" ", ""), text[start:end].strip()))
    return out


# ── Ficha del modelo ─────────────────────────────────────────────────────

def encontrar_gguf(llama_log, modelo):
    if llama_log.exists():
        txt = llama_log.read_text(errors="replace")
        m = re.search(r"['\"]?([^\s'\"]+\.gguf)['\"]?", txt)
        if m:
            return m.group(1)
    for line in ropero_list().splitlines():
        if line.startswith(modelo + " ") or line.startswith(modelo + "\t"):
            for tok in line.split():
                if tok.endswith(".gguf"):
                    return tok
    return None


def tamano_legible(n):
    if n is None:
        return "?"
    for u in ("B", "KB", "MB", "GB", "TB"):
        if n < 1024:
            return f"{n:.1f} {u}"
        n /= 1024
    return f"{n:.1f} PB"


AGENTS_MD = (
    "# Banco de pruebas de ropero\n\n"
    "Responde a la tarea. No inventes salidas de terminal ni resultados "
    "de tests: si dices que algo se ejecutó, tiene que haberse ejecutado "
    "de verdad. Si no puedes hacer una parte, dilo y sigue con el resto.\n"
)


# ── Ejecución por prueba ─────────────────────────────────────────────────

def ejecutar_prueba_simple(modelo, out_dir, prompt_file, work_dir, llama_log):
    """Una prueba de un solo run (prueba1/2/3)."""
    pr_dir = out_dir
    pr_dir.mkdir(parents=True, exist_ok=True)
    events = pr_dir / "opencode-events.json"
    stderr = pr_dir / "opencode-stderr.log"
    sesion = pr_dir / "sesion-export.json"

    prompt = inyectar_corpus(prompt_file.read_text())
    rc, dur = lanzar_opencode(prompt, work_dir, events, stderr)

    sid = extraer_session_id(events)
    if sid:
        exportar_sesion(sid, sesion)
    else:
        warn(f"  {modelo}/{prompt_file.stem}: no encontré session id")

    status = "ok" if rc == 0 else ("timeout" if rc == -1 else f"rc={rc}")
    metricas = construir_metricas(llama_log, events, sesion, dur, status)
    (pr_dir / "metricas.json").write_text(json.dumps(metricas, indent=2))
    return metricas


def ejecutar_prueba_larga(modelo, out_dir, prompt_file, work_dir, llama_log):
    """Prueba larga por bloques, encadenando sesiones B2→B3 y B2→B6."""
    pr_dir = out_dir
    pr_dir.mkdir(parents=True, exist_ok=True)

    bloques = split_bloques(prompt_file)
    sessions = {}
    totales = {"in": 0, "out": 0, "reasoning": 0, "duracion": 0,
               "prompt_max": None, "prompt_avg_l": [],
               "gen_max": None, "gen_avg_l": []}
    statuses = []

    for bid, texto in bloques:
        texto = inyectar_corpus(texto)

        sid_in = None
        if bid in ("BLOQUE3", "BLOQUE6"):
            sid_in = sessions.get("drift")

        sub = pr_dir / bid
        sub.mkdir(exist_ok=True)
        block_work = work_dir / bid
        block_work.mkdir(parents=True, exist_ok=True)
        # AGENTS.md por bloque (opencode lo busca junto al cwd)
        (block_work / "AGENTS.md").write_text(AGENTS_MD)

        events = sub / "opencode-events.json"
        stderr = sub / "opencode-stderr.log"
        sesion = sub / "sesion-export.json"

        marca = llama_log.stat().st_size if llama_log.exists() else 0

        print(f"  {bid} …", flush=True)
        rc, dur = lanzar_opencode(texto, block_work, events, stderr,
                                  session_id=sid_in)

        # volcar el trozo del log del servidor correspondiente a este bloque
        time.sleep(1)
        log_bloque = sub / "llama-server.log"
        try:
            with open(llama_log, "rb") as f:
                f.seek(marca)
                log_bloque.write_bytes(f.read())
        except Exception:
            log_bloque.write_text("")

        sid = extraer_session_id(events)
        if sid:
            exportar_sesion(sid, sesion)

        status = "ok" if rc == 0 else ("timeout" if rc == -1 else f"rc={rc}")
        statuses.append(status)

        m = construir_metricas(log_bloque, events, sesion, dur, status)
        (sub / "metricas.json").write_text(json.dumps(m, indent=2))

        totales["in"]        += m["tokens_in"]
        totales["out"]       += m["tokens_out"]
        totales["reasoning"] += m["tokens_reasoning"]
        totales["duracion"]  += m["duracion_s"]
        if m["prompt_max_tps"] is not None:
            totales["prompt_max"] = (m["prompt_max_tps"]
                                     if totales["prompt_max"] is None
                                     else max(totales["prompt_max"],
                                              m["prompt_max_tps"]))
        if m["gen_max_tps"] is not None:
            totales["gen_max"] = (m["gen_max_tps"]
                                  if totales["gen_max"] is None
                                  else max(totales["gen_max"],
                                           m["gen_max_tps"]))
        if m["prompt_avg_tps"] is not None:
            totales["prompt_avg_l"].append(m["prompt_avg_tps"])
        if m["gen_avg_tps"] is not None:
            totales["gen_avg_l"].append(m["gen_avg_tps"])

        if bid == "BLOQUE2" and sid:
            sessions["drift"] = sid

    agregado = {
        "duracion_s":       totales["duracion"],
        "tokens_in":        totales["in"],
        "tokens_out":       totales["out"],
        "tokens_reasoning": totales["reasoning"],
        "prompt_max_tps":   totales["prompt_max"],
        "prompt_avg_tps":   (sum(totales["prompt_avg_l"])
                             / len(totales["prompt_avg_l"])
                             if totales["prompt_avg_l"] else None),
        "gen_max_tps":      totales["gen_max"],
        "gen_avg_tps":      (sum(totales["gen_avg_l"])
                             / len(totales["gen_avg_l"])
                             if totales["gen_avg_l"] else None),
        "status":           ("ok" if all(s == "ok" for s in statuses)
                             else "parcial"),
    }
    (pr_dir / "metricas.json").write_text(json.dumps(agregado, indent=2))
    return agregado


# ── Informe por modelo ───────────────────────────────────────────────────

def fmt(v, n=2):
    if v is None: return "—"
    if isinstance(v, float): return f"{v:.{n}f}"
    return str(v)


def informe_modelo(modelo, out_dir, gguf):
    lineas = [
        f"# Modelo `{modelo}`",
        "",
        f"- Fecha: {datetime.now().isoformat(timespec='seconds')}",
        f"- Modelo ropero: `{modelo}`",
        f"- Modelo opencode: `{MODELO_OC}`",
        f"- Agente: `{AGENTE}`",
        f"- Temperatura: `{TEMP}`",
        f"- Nivel de razonamiento: `{NIVEL or 'default'}`",
        f"- GGUF: `{gguf or '?'}`",
    ]
    if gguf and Path(gguf).exists():
        lineas.append(f"- Tamaño: {tamano_legible(Path(gguf).stat().st_size)}")
    lineas.append("")

    lineas.append("## Métricas por prueba")
    lineas.append("")
    lineas.append("| Prueba | Entrada | Salida | Razonamiento "
                  "| Prompt máx (t/s) | Prompt med (t/s) "
                  "| Gen máx (t/s) | Gen med (t/s) | Duración | Estado |")
    lineas.append("|---|---|---|---|---|---|---|---|---|---|")
    for p in PRUEBAS:
        mf = out_dir / p["id"] / "metricas.json"
        if not mf.exists():
            lineas.append(f"| {p['id']} | — | — | — | — | — | — | — | — | — |")
            continue
        m = json.loads(mf.read_text())
        lineas.append(
            f"| {p['id']} | {m.get('tokens_in', 0)} | {m.get('tokens_out', 0)} "
            f"| {m.get('tokens_reasoning', 0)} "
            f"| {fmt(m.get('prompt_max_tps'))} | {fmt(m.get('prompt_avg_tps'))} "
            f"| {fmt(m.get('gen_max_tps'))} | {fmt(m.get('gen_avg_tps'))} "
            f"| {m.get('duracion_s', '?')}s | {m.get('status', '?')} |"
        )
    lineas.append("")

    for p in PRUEBAS:
        pr_dir = out_dir / p["id"]
        lineas.append(f"\n## {p['id']}")
        lineas.append("")

        if p["id"] == "prueba_larga" and pr_dir.is_dir():
            subs = [d for d in sorted(pr_dir.iterdir())
                    if d.is_dir() and d.name.startswith("BLOQUE")]
            for sub in subs:
                lineas.append(f"### {sub.name} — llama-server")
                lineas.append("```")
                log = sub / "llama-server.log"
                lineas.append(log.read_text(errors="replace")
                              if log.exists() else "(sin log)")
                lineas.append("```")
                lineas.append(f"### {sub.name} — eventos opencode")
                ev = sub / "opencode-events.json"
                lineas.append("```json")
                lineas.append(ev.read_text(errors="replace")
                              if ev.exists() else "(sin eventos)")
                lineas.append("```")
                ses = sub / "sesion-export.json"
                lineas.append(f"### {sub.name} — sesión")
                lineas.append("```json")
                lineas.append(ses.read_text(errors="replace")
                              if ses.exists() else "(sin sesión)")
                lineas.append("```")
            continue

        lineas.append("### llama-server")
        log = pr_dir / "llama-server.log"
        lineas.append("```")
        lineas.append(log.read_text(errors="replace")
                      if log.exists() else "(sin log)")
        lineas.append("```")

        lineas.append("### Eventos opencode")
        ev = pr_dir / "opencode-events.json"
        lineas.append("```json")
        lineas.append(ev.read_text(errors="replace")
                      if ev.exists() else "(sin eventos)")
        lineas.append("```")

        lineas.append("### stderr opencode")
        er = pr_dir / "opencode-stderr.log"
        lineas.append("```")
        lineas.append(er.read_text(errors="replace")
                      if er.exists() else "(vacío)")
        lineas.append("```")

        lineas.append("### Sesión exportada")
        ses = pr_dir / "sesion-export.json"
        lineas.append("```json")
        lineas.append(ses.read_text(errors="replace")
                      if ses.exists() else "(sin sesión)")
        lineas.append("```")

    return "\n".join(lineas)


# ── Resumen global ───────────────────────────────────────────────────────

def resumen_global(modelos):
    filas = []
    for m in modelos:
        for p in PRUEBAS:
            mf = OUT_DIR / m / p["id"] / "metricas.json"
            if not mf.exists():
                continue
            d = json.loads(mf.read_text())
            filas.append((m, p["id"], d))

    tsv = ["modelo\tprueba\ttokens_in\ttokens_out\ttokens_reasoning"
           "\tprompt_max_tps\tprompt_avg_tps\tgen_max_tps\tgen_avg_tps"
           "\tduracion_s\tstatus"]
    for m, pid, d in filas:
        tsv.append("\t".join(str(x) for x in (
            m, pid,
            d.get("tokens_in", 0), d.get("tokens_out", 0),
            d.get("tokens_reasoning", 0),
            d.get("prompt_max_tps"), d.get("prompt_avg_tps"),
            d.get("gen_max_tps"), d.get("gen_avg_tps"),
            d.get("duracion_s", 0), d.get("status", "?"),
        )))
    (OUT_DIR / "resumen-global.tsv").write_text("\n".join(tsv) + "\n")

    md = ["# Resumen global", "",
          "| Modelo | Prueba | In | Out | Razon. "
          "| Prompt máx | Prompt med | Gen máx | Gen med "
          "| Duración | Estado |",
          "|---|---|---|---|---|---|---|---|---|---|---|"]
    for m, pid, d in filas:
        md.append(
            f"| {m} | {pid} "
            f"| {d.get('tokens_in', 0)} | {d.get('tokens_out', 0)} "
            f"| {d.get('tokens_reasoning', 0)} "
            f"| {fmt(d.get('prompt_max_tps'))} | {fmt(d.get('prompt_avg_tps'))} "
            f"| {fmt(d.get('gen_max_tps'))} | {fmt(d.get('gen_avg_tps'))} "
            f"| {d.get('duracion_s', 0)}s | {d.get('status', '?')} |")
    (OUT_DIR / "resumen-global.md").write_text("\n".join(md) + "\n")


# ── Listado / precheck ──────────────────────────────────────────────────

def listar_modelos():
    print(f"Modelos disponibles en {MODELOS_DIR}:")
    n = 0
    for d in sorted(MODELOS_DIR.iterdir()):
        if not d.is_dir() or d.name == "resultados":
            continue
        ctx = ctx_de_modelo(d.name)
        marca = f" (ctx={ctx})" if ctx else ""
        print(f"  {d.name}{marca}")
        n += 1
    if n == 0:
        print("  (ninguna carpeta de modelo)")
    print()
    print("Para probar:   ./bench_ropero.py <modelo> [<modelo> …]")
    print("               ./bench_ropero.py all")


def precheck():
    for b in ("ropero", "opencode"):
        if not binario(b):
            err(f"falta '{b}' en el PATH")
            sys.exit(1)
    faltan = [p["file"] for p in PRUEBAS
              if not (PROMPTS_DIR / p["file"]).exists()]
    if faltan:
        err(f"faltan prompts en {PROMPTS_DIR}: {', '.join(faltan)}")
        sys.exit(1)
    if not (PROMPTS_DIR / PROMPT_JUEZ).exists():
        warn(f"no encuentro el prompt del juez ({PROMPT_JUEZ}); "
             "el paquete del juez no lo incluirá")


# ── main ────────────────────────────────────────────────────────────────

def main():
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("modelos", nargs="*")
    args = ap.parse_args()

    if not args.modelos:
        listar_modelos()
        return

    if DEBUG:
        print(f"[debug] PUERTO={PUERTO} MODELO={MODELO_OC} "
              f"AGENTE={AGENTE} TIMEOUT={TIMEOUT} ESPERA_MIN={ESPERA_MIN} "
              f"TEMP={TEMP} NIVEL={NIVEL!r}")

    precheck()
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    (OUT_DIR / "juez").mkdir(exist_ok=True)

    if args.modelos == ["all"]:
        modelos = sorted(d.name for d in MODELOS_DIR.iterdir()
                         if d.is_dir() and d.name != "resultados")
    else:
        modelos = args.modelos

    for m in modelos:
        if not (MODELOS_DIR / m).is_dir():
            err(f"no hay carpeta para '{m}' en {MODELOS_DIR}")
            return

    lock = Path(tempfile.gettempdir()) / f".bench_ropero-{os.getuid()}.lock"
    lock_fd = open(lock, "w")
    try:
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        err("ya hay otra corrida en marcha")
        return

    todos_arrancados = []
    try:
        for idx, modelo in enumerate(modelos, 1):
            model_dir = MODELOS_DIR / modelo
            out_dir   = OUT_DIR / modelo
            out_dir.mkdir(parents=True, exist_ok=True)

            head2(f"[{idx}/{len(modelos)}] {modelo}")

            hecho = out_dir / ".bench-hecho"
            if not FORCE and modelo not in FORCE_MODELOS and hecho.exists():
                ok("ya estaba probado — me lo salto (FORCE=1 para repetir)")
                continue
            hecho.unlink(missing_ok=True)

            for p in PRUEBAS:
                pr_dir = out_dir / p["id"]

                if not FORCE and modelo not in FORCE_MODELOS \
                        and (pr_dir / "metricas.json").exists():
                    info(f"  {p['id']} ya estaba — me la salto")
                    continue

                pr_dir.mkdir(parents=True, exist_ok=True)
                log = pr_dir / "llama-server.log"

                ctx = ctx_de_modelo(modelo)
                espera = ESPERA_MIN
                if ctx:
                    espera = ESPERA_MIN + ((ctx + 32767) // 32768) * 60
                info(f"  {p['id']}: arrancando {modelo} (espera {espera}s)")

                proc, motivo = arrancar_modelo(modelo, model_dir, log)
                if proc is None:
                    err(f"  {p['id']}: no arrancó — {motivo}")
                    parar_modelo(modelo)
                    if modelo in todos_arrancados:
                        todos_arrancados.remove(modelo)
                    continue
                todos_arrancados.append(modelo)

                if not esperar_modelo(modelo, espera, proc, log):
                    err(f"  {p['id']}: {modelo} no respondió en :{PUERTO} "
                        f"en {espera}s")
                    parar_modelo(modelo)
                    if modelo in todos_arrancados:
                        todos_arrancados.remove(modelo)
                    continue

                work = model_dir / f"work-{p['id']}"
                work.mkdir(exist_ok=True)
                (work / "AGENTS.md").write_text(AGENTS_MD)

                info(f"  {p['id']}: lanzando opencode run")
                if p["split"]:
                    met = ejecutar_prueba_larga(modelo, pr_dir,
                                                PROMPTS_DIR / p["file"],
                                                work, log)
                else:
                    met = ejecutar_prueba_simple(modelo, pr_dir,
                                                 PROMPTS_DIR / p["file"],
                                                 work, log)
                ok(f"  {p['id']}: {met['status']} en {met['duracion_s']}s")

                parar_modelo(modelo)
                if modelo in todos_arrancados:
                    todos_arrancados.remove(modelo)

            gguf = encontrar_gguf(out_dir / "prueba1" / "llama-server.log",
                                  modelo)
            informe = informe_modelo(modelo, out_dir, gguf)
            (out_dir / "informe.md").write_text(informe)
            (OUT_DIR / "juez" / f"{modelo}.md").write_text(informe)
            ok(f"  informe: {out_dir / 'informe.md'}")

            hecho.write_text(datetime.now().isoformat())

    except KeyboardInterrupt:
        print("\nCtrl-C, parando…")
    finally:
        for m in list(todos_arrancados):
            parar_modelo(m)
        lock_fd.close()

    resumen_global(modelos)
    if (PROMPTS_DIR / PROMPT_JUEZ).exists():
        shutil.copy(PROMPTS_DIR / PROMPT_JUEZ, OUT_DIR / "juez" / PROMPT_JUEZ)

    head2("paquete del juez")
    for f in sorted((OUT_DIR / "juez").iterdir()):
        print(f"  {f.name}")
    head2("resumen global")
    print((OUT_DIR / "resumen-global.md").read_text())


if __name__ == "__main__":
    main()