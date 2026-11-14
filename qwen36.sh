#!/usr/bin/env bash
#
# strata-35b.sh — prep + tune + launch de Qwen3.6-35B-A3B en Strata.
#
# Uso:
#   ./strata-35b.sh preparar          # descarga + prep + tune + registro
#   ./strata-35b.sh reinstalar        # regenera cfg sin re-descargar
#   ./strata-35b.sh afinar            # mide y guarda preset
#   ./strata-35b.sh lanzar            # sirve (Ctrl-C para parar)
#   ./strata-35b.sh ciclo             # afinar + lanzar
#   ./strata-35b.sh registrar         # guarda preset en strata launch
#   ./strata-35b.sh estado            # estado
#
# Env vars:
#   CTX=65536        contexto (nativo 262144; sube si la VRAM aguanta)
#   KV=k8v4          int8 | q4_0 | k8v4 | fp16
#   VISION=off       off | on | gpu | cpu | none
#   RESERVA=512      MiB de VRAM a dejar libres
#   TUNE=quick       quick | thorough | skip
#   PORT=9999
#
set -euo pipefail

# --- Identidad del modelo ---
ID="qwen36-35b"
REPO="unsloth/Qwen3.6-35B-A3B-MTP-GGUF"
QUANT="UD-Q4_K_XL"                      # recomendado por Strata
FILE="Qwen3.6-35B-A3B-${QUANT}.gguf"
FAMILY="qwen36"
MODEL="${QUANT}"                         # setup.py lo reconoce

# --- Rutas ---
RAIZ="${RAIZ:-$HOME/models/gguf}"
MODEL_DIR="$RAIZ/$ID"
GGUF="$MODEL_DIR/$FILE"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/share/strata/install}"
CFG="$INSTALL_DIR/strata-$ID.json"
PRIVATE_INSTALL="$MODEL_DIR/install"

# --- Sampling para Qwen3.6-35B-A3B (modo instruct) ---
# Thinking: temp 1.0 / top_p 0.95. Instruct: temp 0.7 / top_p 0.8.
SAMPLING='{"temperature":0.7,"top_p":0.8,"top_k":20,"min_p":0.0,"presence_penalty":1.5,"repetition_penalty":1.0}'

# --- Defaults ---
CTX="${CTX:-131072}"
KV="${KV:-k8v4}"
VISION="${VISION:-off}"
RESERVA="${RESERVA:-512}"
TUNE="${TUNE:-quick}"
PORT="${PORT:-9999}"

# --- Helpers ---
say()  { printf '\n\033[1;36m== %s\033[0m\n' "$*"; }
info() { printf '   %s\n' "$*"; }
err()  { printf '\033[1;31m!! %s\033[0m\n' "$*" >&2; }
die()  { err "$*"; exit 1; }

cargar_vars() {
  case "${VISION:-off}" in
    on|yes)    VISION_OPT="yes";  VISION_LAUNCH="on"  ;;
    off|no)    VISION_OPT="no";   VISION_LAUNCH="off" ;;
    cpu)       VISION_OPT="cpu";  VISION_LAUNCH="cpu" ;;
    gpu)       VISION_OPT="gpu";  VISION_LAUNCH="gpu" ;;
    none)      VISION_OPT="none"; VISION_LAUNCH="off" ;;
    *) die "VISION inválido: $VISION" ;;
  esac
}

hf_bin() {
  if command -v hf >/dev/null 2>&1; then echo hf
  elif command -v huggingface-cli >/dev/null 2>&1; then echo huggingface-cli
  else die "instala huggingface_hub[hf_transfer]"
  fi
}

resolver_strata() {
  local launcher active
  launcher=$(command -v strata) || die "no encuentro strata en PATH"
  launcher=$(readlink -f "$launcher")
  active=$(dirname "$(dirname "$launcher")")
  [[ -f "$active/strata.py" && -f "$active/setup.py" ]] || die "layout no reconocido: $launcher"
  STRATA_DIR="$active"
  STRATA_PY="${STRATA_PY:-${STRATA_PYTHON:-$active/python/bin/python3}}"
  [[ -x "$STRATA_PY" ]] || die "no existe el Python de Strata: $STRATA_PY"
  export STRATA_PYTHON="$STRATA_PY"
}

exigir_gpu_libre() {
  local procesos memoria fila pid vram usado idx=0
  procesos=$(nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits 2>/dev/null) \
    || die "no se puede comprobar quién ocupa la GPU"
  while IFS= read -r fila || [[ -n "$fila" ]]; do
    [[ -z "${fila//[[:space:]]/}" ]] && continue
    IFS=, read -r pid vram <<<"$fila"
    pid="${pid//[[:space:]]/}"; vram="${vram//[[:space:]]/}"
    [[ "$pid" =~ ^[0-9]+$ && "$vram" =~ ^[0-9]+$ ]] || die "estado GPU desconocido: $fila"
    (( vram <= 500 )) || die "GPU ocupada por proceso $pid ($vram MiB); no se matan"
  done <<<"$procesos"
  memoria=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits 2>/dev/null) \
    || die "no se puede comprobar VRAM"
  while IFS= read -r usado || [[ -n "$usado" ]]; do
    usado="${usado//[[:space:]]/}"
    [[ -z "$usado" ]] && continue
    [[ "$usado" =~ ^[0-9]+$ ]] || die "VRAM no válida: $usado"
    (( usado <= 500 )) || die "GPU $idx con $usado MiB ocupados; no se matan procesos"
    idx=$((idx+1))
  done <<<"$memoria"
}

# --- Descargar ---
descargar() {
  mkdir -p "$MODEL_DIR"
  if [[ -f "$GGUF" ]]; then
    info "GGUF ya presente: $GGUF"
    return 0
  fi
  say "descargando $FILE"
  HF_HUB_ENABLE_HF_TRANSFER=1 "$(hf_bin)" download "$REPO" "$FILE" --local-dir "$MODEL_DIR"
  [[ -f "$GGUF" ]] || die "la descarga no dejó $GGUF"
  info "GGUF OK: $GGUF"
}

# --- Instalar (setup.py) ---
instalar() {
  resolver_strata
  [[ ! -L "$PRIVATE_INSTALL" ]] || die "install privado no puede ser symlink"
  mkdir -p "$PRIVATE_INSTALL" "$INSTALL_DIR"

  say "instalando (setup.py --setup)"
  STRATA_CONTROL_INSTALL=1 STRATA_INSTALL_DIR="$PRIVATE_INSTALL" PYTHONNOUSERSITE=1 \
    "$STRATA_PY" "$STRATA_DIR/setup.py" --setup \
      --family "$FAMILY" --model "$MODEL" \
      --gguf-dir "$MODEL_DIR" --data-dir "$MODEL_DIR" \
      --low-ram off --mtp on --context "$CTX" --kv "$KV" \
      --vision "$VISION_OPT" --vram-reserve-mib "$RESERVA" \
      --no-start --yes

  local generado
  generado=$(ls -1 "$PRIVATE_INSTALL"/strata-*.json 2>/dev/null | head -1 || true)
  [[ -n "$generado" && -f "$generado" ]] || die "setup.py no generó cfg"
  cp "$generado" "$CFG"
  info "cfg: $CFG"
}

# --- Parche (sampling + reserva) ---
parchar() {
  [[ -f "$CFG" ]] || die "no hay cfg (preparar primero)"
  SAMPLING_JSON="$SAMPLING" python3 - "$CFG" "$RESERVA" <<'PYEOF'
import json, os, pathlib, sys
p = pathlib.Path(sys.argv[1])
reserve = sys.argv[2]
sampling_json = os.environ["SAMPLING_JSON"]

cfg = json.loads(p.read_text())
base = cfg.get("args") or cfg.get("args_written") or []
if not isinstance(base, list):
    sys.exit("args inválido")

sampling_flags = {"--temp","--temperature","--top-p","--top-k","--min-p",
                  "--presence-penalty","--repeat-penalty","--repetition-penalty",
                  "--repeat-last-n"}
replace_flags = {"--vram-reserve-mib"}

out, i = [], 0
while i < len(base):
    a = base[i]
    if a in replace_flags:
        i += 2; continue
    if any(a.startswith(f+"=") for f in replace_flags):
        i += 1; continue
    if a in sampling_flags:
        i += 2; continue
    if any(a.startswith(f+"=") for f in sampling_flags):
        i += 1; continue
    out.append(a); i += 1

out += ["--vram-reserve-mib", reserve]
cfg["args"] = out
cfg["log"] = str(p.with_suffix(".log"))
if sampling_json:
    try:
        cfg["sampling"] = json.loads(sampling_json)
    except json.JSONDecodeError as e:
        sys.exit(f"SAMPLING inválido: {e}")
p.write_text(json.dumps(cfg, indent=2))
print("   args:", " ".join(out))
print("   sampling:", sampling_json)
PYEOF
}

# --- Afinar ---
afinar() {
  [[ -f "$CFG" ]] || die "no hay cfg (preparar primero)"
  resolver_strata
  say "afinando $ID (modo $TUNE)"
  exigir_gpu_libre
  parchar

  mkdir -p "$MODEL_DIR"
  local jlog="$MODEL_DIR/tune.json" tlog="$MODEL_DIR/tune.log"
  local rc=0
  strata tune auto "$CFG" --mode "$TUNE" --output json --name "$ID" \
    >"$jlog" 2>"$tlog" || rc=$?
  [[ -s "$tlog" ]] && tail -n 12 "$tlog" >&2

  if ! python3 - "$jlog" "$rc" <<'PYEOF'
import json, sys
try:
    data = json.load(open(sys.argv[1], encoding="utf-8"))
    result = data.get("result") or {}
    outcome = result.get("outcome")
    if int(sys.argv[2]) or data.get("ok") is not True or outcome not in ("improved","default_confirmed"):
        raise ValueError(f"exit={sys.argv[2]}, outcome={outcome}: {result.get('message','')}")
    if not (result.get("preset") or {}).get("id"):
        raise ValueError("sin preset")
    print(f"   tune: {outcome}; preset: {result['preset']['id']}")
except Exception as exc:
    sys.exit(f"tune no completado: {exc}")
PYEOF
  then
    err "tune falló; consulta $jlog y $tlog"
    (( rc != 0 )) && return "$rc"
    return 1
  fi
  parchar
}

# --- Lanzar ---
lanzar() {
  [[ -f "$CFG" ]] || die "no hay cfg (preparar primero)"
  resolver_strata
  exigir_gpu_libre
  say "lanzando $ID"
  info "cfg: $CFG"
  info "modelo: $GGUF"
  info "Ctrl-C para parar"
  exec strata serve --profile "$CFG" --port "$PORT"
}

# --- Registrar en strata launch ---
registrar() {
  [[ -f "$CFG" ]] || die "no hay cfg (preparar primero)"
  parchar
  resolver_strata
  say "registrando $ID en strata launch"
  local preset_name="Qwen3.6-35B-A3B ${QUANT}"
  if strata launch \
       --model-path "$GGUF" \
       --family "$FAMILY" \
       --model "$MODEL" \
       --context "$CTX" \
       --kv "$KV" \
       --vision "$VISION_LAUNCH" \
       --mtp on \
       --tune skip \
       --save-preset "$preset_name" \
       --yes --no-tui 2>&1 | tail -5; then
    info "preset guardado: \"$preset_name\""
  else
    err "no se pudo guardar el preset; usa '$0 lanzar' o 'strata serve --profile $CFG'"
  fi
  if strata launch --list-presets 2>/dev/null | grep -q "$preset_name"; then
    info "confirmado en --list-presets"
  fi
}

# --- Estado ---
estado() {
  say "GPU"
  nvidia-smi --query-gpu=name,memory.used,memory.total --format=csv 2>/dev/null || true
  say "motores de Strata"
  pgrep -af 'components/engine/.*/strata|serve[.]server' || info "ninguno"
  say "ficheros de $ID"
  [[ -f "$GGUF" ]] && info "GGUF: $GGUF" || info "GGUF: FALTA"
  [[ -f "$CFG" ]] && info "cfg:  $CFG" || info "cfg:  FALTA"
}

# --- Preparar (todo en uno) ---
cmd_preparar() {
  cargar_vars
  say "preparando $ID"
  info "CTX=$CTX, KV=$KV, VISION=$VISION_OPT, RESERVA=$RESERVA, MTP=on (integrado)"

  descargar
  instalar
  parchar

  if [[ "$TUNE" != "skip" ]]; then
    afinar || err "tune falló; el modelo está preparado igual"
  fi

  registrar || err "registro falló; usa '$0 lanzar'"
  say "listo: $ID preparado"
  estado
}

reinstalar() {
  [[ -f "$GGUF" ]] || die "falta GGUF (preparar primero)"
  instalar
  parchar
  say "$ID reinstalado (cfg regenerado, GGUF intacto)"
}

# --- Dispatch ---
cargar_vars
case "${1:-}" in
  preparar)   shift; cmd_preparar "$@" ;;
  reinstalar) reinstalar ;;
  afinar)     afinar ;;
  lanzar)     lanzar ;;
  ciclo)      afinar; lanzar ;;
  registrar)  registrar ;;
  estado)     estado ;;
  parar)      die "no se paran procesos: usa el operador del motor" ;;
  *)          sed -n '2,/^[^#]/p' "$0" | sed 's/^# \{0,1\}//' | sed '$d'; exit 1 ;;
esac