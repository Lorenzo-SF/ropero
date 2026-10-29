#!/usr/bin/env python3
"""
bench_ropero.py — banco de pruebas para modelos servidos por ropero.

Uso:
  ./bench_ropero.py                    lista los modelos
  ./bench_ropero.py <m1> <m2> …        prueba esos
  ./bench_ropero.py all                todos
  FORCE=1 ./bench_ropero.py <m>        rehace
"""
import argparse, fcntl, json, os, re, shutil, subprocess, sys, tempfile, time
from datetime import datetime
from pathlib import Path
from urllib.request import Request, urlopen

HOME        = Path.home()
MODELOS_DIR = Path(os.environ.get("MODELOS_DIR", HOME / "test"))
OUT_DIR     = Path(os.environ.get("OUT_DIR", MODELOS_DIR / "resultados"))
PROMPTS_DIR = Path(__file__).resolve().parent

AGENTE    = os.environ.get("AGENTE", "build")
MODELO_OC = os.environ.get("MODELO", "marichocho/Puertourraco-256k")
PUERTO    = int(os.environ.get("PUERTO", "9999"))
API_KEY   = os.environ.get("API_KEY", "sk-local-dev-key")
TIMEOUT   = int(os.environ.get("TIMEOUT", "1800"))
ESPERA    = int(os.environ.get("ESPERA", "900"))
TEMP      = os.environ.get("TEMP", "0.15")
NIVEL     = os.environ.get("NIVEL", "high")
FORCE     = os.environ.get("FORCE", "0") == "1"

PRUEBAS = [
    {"id": "prueba1",      "file": "prueba1.md",      "split": False},
    {"id": "prueba2",      "file": "prueba2.md",      "split": False},
    {"id": "prueba3",      "file": "prueba3.md",      "split": False},
    {"id": "prueba_larga", "file": "prueba_larga.md", "split": True},
]
PROMPT_JUEZ = "juez.md"

if sys.stdout.isatty():
    R, G, Y, D, N = "\033[31m", "\033[32m", "\033[33m", "\033[2m", "\033[0m"
else:
    R = G = Y = D = N = ""

def info(m): print(f"{D}{m}{N}", flush=True)
def warn(m): print(f"{Y}[!] {m}{N}", file=sys.stderr, flush=True)
def err(m):  print(f"{R}[x] {m}{N}", file=sys.stderr, flush=True)
def ok(m):   print(f"{G}[+] {m}{N}", flush=True)


# ── HTTP contra el servidor local ─────────────────────────────────────

def modelo_activo():
    """id del modelo servido en :PUERTO, o None."""
    try:
        req = Request(f"http://127.0.0.1:{PUERTO}/v1/models",
                      headers={"Authorization": f"Bearer {API_KEY}"})
        with urlopen(req, timeout=3) as r:
            data = json.load(r)
            if data.get("data"):
                return data["data"][0]["id"]
    except Exception:
        pass
    return None


def completar_ok():
    """POST real con max_tokens=1. True si 2xx."""
    mid = modelo_activo()
    if not mid:
        return False
    body = json.dumps({
        "model": mid,
        "messages": [{"role": "user", "content": "x"}],
        "max_tokens": 1, "temperature": 0, "stream": False,
    }).encode()
    req = Request(
        f"http://127.0.0.1:{PUERTO}/v1/chat/completions",
        data=body,
        headers={"Authorization": f"Bearer {API_KEY}",
                 "Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urlopen(req, timeout=20) as r:
            return 200 <= r.status < 300
    except Exception:
        return False


# ── ropero / opencode ─────────────────────────────────────────────────

def nivel_flag(modelo):
    if not NIVEL:
        return None
    try:
        r = subprocess.run(["ropero", "levels", modelo],
                           capture_output=True, text=True, timeout=10)
        if NIVEL in r.stdout.split():
            return f"--{NIVEL}"
    except Exception:
        pass
    return None


def parar_modelo(modelo):
    subprocess.run(["ropero", "stop", modelo],
                   capture_output=True, timeout=30)
    time.sleep(1)


def arrancar_modelo(modelo, work_dir, log_path):
    """Arranca ropero en background. Lanza RuntimeError si falla."""
    parar_modelo(modelo)
    if modelo_activo():
        raise RuntimeError(f":{PUERTO} sigue ocupado tras 'ropero stop {modelo}'")

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
        tail = log_path.read_text(errors="replace")[-600:].strip()
        raise RuntimeError(f"ropero salió (rc={proc.returncode}):\n{tail}")
    return proc


def esperar_modelo(proc, timeout):
    """True cuando un POST real a /v1/chat/completions devuelve 2xx."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        if proc.poll() is not None:
            return False
        if completar_ok():
            return True
        time.sleep(3)
    return False


def exportar_sesion(sid, out_path):
    with open(out_path, "w") as f:
        r = subprocess.run(["opencode", "session", "export", sid],
                           stdout=f, stderr=subprocess.DEVNULL, timeout=120)
        return r.returncode == 0


def extraer_session_id(events_path):
    if not events_path.exists():
        return None
    for line in events_path.read_text(errors="replace").splitlines():
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(ev, dict):
            continue
        for d in (ev, ev.get("properties"), ev.get("data")):
            if isinstance(d, dict):
                for k in ("sessionID", "session_id", "sessionId"):
                    v = d.get(k)
                    if isinstance(v, str) and v:
                        return v
    return None


def lanzar_opencode(prompt, work_dir, events_path, stderr_path,
                    session_id=None, timeout=TIMEOUT):
    cmd = ["opencode", "run", prompt,
           "--model", MODELO_OC, "--agent", AGENTE, "--format", "json"]
    if session_id:
        cmd.extend(["--session", session_id])
    events_path.parent.mkdir(parents=True, exist_ok=True)
    work_dir.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    with open(events_path, "w") as outf, open(stderr_path, "w") as errf:
        try:
            r = subprocess.run(cmd, cwd=str(work_dir), stdout=outf,
                               stderr=errf, timeout=timeout)
            rc = r.returncode
        except subprocess.TimeoutExpired:
            rc = -1
    return rc, int(time.time() - t0)


# ── métricas ──────────────────────────────────────────────────────────

def metricas_llama(log_path):
    prompt, gen = [], []
    if log_path.exists():
        for line in log_path.read_text(errors="replace").splitlines():
            if "eval time" not in line:
                continue
            m = re.search(r"([\d.]+)\s*tokens per second", line)
            if not m:
                continue
            (prompt if "prompt eval" in line else gen).append(float(m.group(1)))
    return {
        "prompt_max": max(prompt) if prompt else None,
        "prompt_avg": sum(prompt) / len(prompt) if prompt else None,
        "gen_max":    max(gen)    if gen    else None,
        "gen_avg":    sum(gen)    / len(gen) if gen else None,
    }


def tokens_de_sesion(sesion_path):
    r = {"in": 0, "out": 0, "reasoning": 0}
    if not sesion_path.exists():
        return r
    try:
        data = json.loads(sesion_path.read_text(errors="replace"))
    except Exception:
        return r
    msgs = data if isinstance(data, list) else (
        data.get("messages") or data.get("parts") or data.get("events") or [])
    for m in msgs:
        if not isinstance(m, dict):
            continue
        u = m.get("usage") or {}
        if not isinstance(u, dict):
            continue
        r["in"]        += int(u.get("input_tokens")  or u.get("prompt_tokens")     or 0)
        r["out"]       += int(u.get("output_tokens") or u.get("completion_tokens") or 0)
        r["reasoning"] += int(u.get("reasoning_tokens") or 0)
    return r


def construir_metricas(llama_log, sesion, dur, status):
    s = metricas_llama(llama_log)
    t = tokens_de_sesion(sesion)
    return {
        "duracion_s":       dur,
        "tokens_in":        t["in"],
        "tokens_out":       t["out"],
        "tokens_reasoning": t["reasoning"],
        "prompt_max_tps":   s["prompt_max"],
        "prompt_avg_tps":   s["prompt_avg"],
        "gen_max_tps":      s["gen_max"],
        "gen_avg_tps":      s["gen_avg"],
        "status":           status,
    }


# ── corpus largo determinista ─────────────────────────────────────────

_CORPUS = None

def generar_corpus():
    global _CORPUS
    if _CORPUS is not None:
        return _CORPUS

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

    _CORPUS = "\n\n".join(parts)
    return _CORPUS


def inyectar_corpus(texto):
    if "<!-- CORPUS:INICIO -->" not in texto:
        return texto
    pre  = texto.split("<!-- CORPUS:INICIO -->", 1)[0]
    post = texto.split("<!-- CORPUS:FIN -->", 1)[1]
    return pre + generar_corpus() + post


def split_bloques(prompt_file):
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


AGENTS_MD = (
    "# Banco de pruebas de ropero\n\n"
    "Responde a la tarea. No inventes salidas de terminal ni resultados "
    "de tests: si dices que algo se ejecutó, tiene que haberse ejecutado "
    "de verdad. Si no puedes hacer una parte, dilo y sigue con el resto.\n"
)


# ── ejecución por prueba ──────────────────────────────────────────────

def ejecutar_prueba_simple(prompt_file, pr_dir, work_dir, llama_log):
    events = pr_dir / "opencode-events.json"
    stderr = pr_dir / "opencode-stderr.log"
    sesion = pr_dir / "sesion-export.json"

    prompt = inyectar_corpus(prompt_file.read_text())
    rc, dur = lanzar_opencode(prompt, work_dir, events, stderr)

    sid = extraer_session_id(events)
    if sid:
        exportar_sesion(sid, sesion)

    status = "ok" if rc == 0 else ("timeout" if rc == -1 else f"rc={rc}")
    metricas = construir_metricas(llama_log, sesion, dur, status)
    (pr_dir / "metricas.json").write_text(json.dumps(metricas, indent=2))
    return metricas


def ejecutar_prueba_larga(prompt_file, pr_dir, work_dir, llama_log):
    """Ejecuta la prueba por bloques, encadenando B2→B3 y B2→B6."""
    bloques = split_bloques(prompt_file)
    sessions = {}
    totales = {"in": 0, "out": 0, "reasoning": 0, "duracion": 0,
               "prompt_max": None, "prompt_avg_l": [],
               "gen_max": None, "gen_avg_l": []}
    statuses = []

    for bid, texto in bloques:
        texto = inyectar_corpus(texto)

        sid_in = sessions.get("drift") if bid in ("BLOQUE3", "BLOQUE6") else None

        sub = pr_dir / bid
        sub.mkdir(exist_ok=True)
        block_work = work_dir / bid
        block_work.mkdir(parents=True, exist_ok=True)
        (block_work / "AGENTS.md").write_text(AGENTS_MD)

        events = sub / "opencode-events.json"
        stderr = sub / "opencode-stderr.log"
        sesion = sub / "sesion-export.json"

        marca = llama_log.stat().st_size if llama_log.exists() else 0
        print(f"  {bid} …", flush=True)
        rc, dur = lanzar_opencode(texto, block_work, events, stderr,
                                  session_id=sid_in)

        # trozo de log del servidor para este bloque
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

        m = construir_metricas(log_bloque, sesion, dur, status)
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
                                  else max(totales["gen_max"], m["gen_max_tps"]))
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
        "status":           "ok" if all(s == "ok" for s in statuses) else "parcial",
    }
    (pr_dir / "metricas.json").write_text(json.dumps(agregado, indent=2))
    return agregado


# ── informes ──────────────────────────────────────────────────────────

def fmt(v, n=2):
    if v is None: return "—"
    if isinstance(v, float): return f"{v:.{n}f}"
    return str(v)


def informe_modelo(modelo, out_dir):
    lineas = [
        f"# Modelo `{modelo}`",
        "",
        f"- Fecha: {datetime.now().isoformat(timespec='seconds')}",
        f"- Modelo ropero: `{modelo}`",
        f"- Modelo opencode: `{MODELO_OC}`",
        f"- Agente: `{AGENTE}`  · Temperatura: `{TEMP}`  · "
        f"Nivel: `{NIVEL or 'default'}`",
        "",
        "## Métricas por prueba",
        "",
        "| Prueba | In | Out | Razon. | Prompt máx | Prompt med "
        "| Gen máx | Gen med | Duración | Estado |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    for p in PRUEBAS:
        mf = out_dir / p["id"] / "metricas.json"
        if not mf.exists():
            lineas.append(f"| {p['id']} | — | — | — | — | — | — | — | — | — |")
            continue
        m = json.loads(mf.read_text())
        lineas.append(
            f"| {p['id']} | {m['tokens_in']} | {m['tokens_out']} "
            f"| {m['tokens_reasoning']} "
            f"| {fmt(m['prompt_max_tps'])} | {fmt(m['prompt_avg_tps'])} "
            f"| {fmt(m['gen_max_tps'])} | {fmt(m['gen_avg_tps'])} "
            f"| {m['duracion_s']}s | {m['status']} |"
        )
    lineas.append("")
    lineas.append("Los logs completos (llama-server, eventos opencode, sesión) "
                  "están junto a cada `metricas.json`.")
    return "\n".join(lineas)


def resumen_global(modelos):
    md = ["# Resumen global", "",
          "| Modelo | Prueba | In | Out | Razon. "
          "| Prompt máx | Prompt med | Gen máx | Gen med "
          "| Duración | Estado |",
          "|---|---|---|---|---|---|---|---|---|---|---|"]
    for m in modelos:
        for p in PRUEBAS:
            mf = OUT_DIR / m / p["id"] / "metricas.json"
            if not mf.exists():
                continue
            d = json.loads(mf.read_text())
            md.append(
                f"| {m} | {p['id']} "
                f"| {d.get('tokens_in', 0)} | {d.get('tokens_out', 0)} "
                f"| {d.get('tokens_reasoning', 0)} "
                f"| {fmt(d.get('prompt_max_tps'))} | {fmt(d.get('prompt_avg_tps'))} "
                f"| {fmt(d.get('gen_max_tps'))} | {fmt(d.get('gen_avg_tps'))} "
                f"| {d.get('duracion_s', 0)}s | {d.get('status', '?')} |")
    (OUT_DIR / "resumen-global.md").write_text("\n".join(md) + "\n")


# ── main ──────────────────────────────────────────────────────────────

def listar_modelos():
    print(f"Modelos disponibles en {MODELOS_DIR}:")
    n = 0
    for d in sorted(MODELOS_DIR.iterdir()):
        if d.is_dir() and d.name != "resultados":
            print(f"  {d.name}")
            n += 1
    if n == 0:
        print("  (ninguna carpeta de modelo)")
    print("\nPara probar:   ./bench_ropero.py <modelo> [<modelo> …]")
    print("               ./bench_ropero.py all")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("modelos", nargs="*")
    args = ap.parse_args()

    if not args.modelos:
        listar_modelos()
        return

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

    modelos_arrancados = set()
    try:
        for idx, modelo in enumerate(modelos, 1):
            model_dir = MODELOS_DIR / modelo
            out_dir   = OUT_DIR / modelo
            out_dir.mkdir(parents=True, exist_ok=True)

            print(f"\n=== [{idx}/{len(modelos)}] {modelo} ===", flush=True)

            for p in PRUEBAS:
                pr_dir = out_dir / p["id"]
                if not FORCE and (pr_dir / "metricas.json").exists():
                    info(f"  {p['id']} ya estaba — me la salto")
                    continue

                pr_dir.mkdir(parents=True, exist_ok=True)
                log = pr_dir / "llama-server.log"

                info(f"  {p['id']}: arrancando {modelo} (espera {ESPERA}s)")
                try:
                    proc = arrancar_modelo(modelo, model_dir, log)
                except RuntimeError as e:
                    err(f"  {p['id']}: no arrancó — {e}")
                    continue
                modelos_arrancados.add(modelo)

                if not esperar_modelo(proc, ESPERA):
                    err(f"  {p['id']}: {modelo} no respondió en "
                        f":{PUERTO} en {ESPERA}s")
                    parar_modelo(modelo)
                    modelos_arrancados.discard(modelo)
                    continue

                work = model_dir / f"work-{p['id']}"
                work.mkdir(exist_ok=True)
                (work / "AGENTS.md").write_text(AGENTS_MD)

                info(f"  {p['id']}: lanzando opencode run")
                if p["split"]:
                    met = ejecutar_prueba_larga(PROMPTS_DIR / p["file"],
                                                pr_dir, work, log)
                else:
                    met = ejecutar_prueba_simple(PROMPTS_DIR / p["file"],
                                                 pr_dir, work, log)
                ok(f"  {p['id']}: {met['status']} en {met['duracion_s']}s")

                parar_modelo(modelo)
                modelos_arrancados.discard(modelo)

            informe = informe_modelo(modelo, out_dir)
            (out_dir / "informe.md").write_text(informe)
            (OUT_DIR / "juez" / f"{modelo}.md").write_text(informe)
            ok(f"  informe: {out_dir / 'informe.md'}")

    except KeyboardInterrupt:
        print("\nCtrl-C, parando…")
    finally:
        for m in list(modelos_arrancados):
            parar_modelo(m)
        lock_fd.close()

    resumen_global(modelos)
    juez_src = PROMPTS_DIR / PROMPT_JUEZ
    if juez_src.exists():
        shutil.copy(juez_src, OUT_DIR / "juez" / PROMPT_JUEZ)

    print("\n=== paquete del juez ===")
    for f in sorted((OUT_DIR / "juez").iterdir()):
        print(f"  {f.name}")
    print("\n=== resumen global ===")
    print((OUT_DIR / "resumen-global.md").read_text())


if __name__ == "__main__":
    main()