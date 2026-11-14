#!/usr/bin/env bash
#
# qwen-abliterated.sh — prep + tune + registro + launch de
# Qwen3.8-Flash-Next-GSQ-RCO-abliterated (IQ2_XS) con Strata.
#
# Uso:
#   ./qwen-abliterated.sh preparar            # descarga (shards + MTP) + prep + tune + registro
#   ./qwen-abliterated.sh preparar --no-mtp   # sin MTP
#   ./qwen-abliterated.sh reinstalar          # regenera cfg sin tocar pack
#   ./qwen-abliterated.sh afinar              # mide y guarda preset
#   ./qwen-abliterated.sh lanzar              # sirve (Ctrl-C para parar)
#   ./qwen-abliterated.sh ciclo               # afinar + lanzar
#   ./qwen-abliterated.sh registrar           # guarda preset en strata launch
#   ./qwen-abliterated.sh estado              # estado
#
# Env vars:
#   CTX=32768        contexto en tokens (nativo 262144; sube si la VRAM aguanta)
#   KV=k8v4          int8 | q4_0 | k8v4 | fp16
#   VISION=off       off | on | gpu | cpu | none
#   RESERVA=512      MiB de VRAM a dejar libres (baja si la iGPU renderiza)
#   MTP=on           on | off (descarga y usa el runtime de strata/rt/)
#   TUNE=quick       quick | thorough | skip
#   PORT=9999
#
set -euo pipefail

# --- Identidad del modelo ---
ID="qwen-abliterated"
REPO="SC117/Qwen3.8-Flash-Next-GSQ-RCO-abliterated-GGUF"
QUANT="IQ2_XS"
SHARD1="Qwen3.8-Flash-Next-GSQ-RCO-abliterated-IQ2_XS-00001-of-00002.gguf"
SHARD2="Qwen3.8-Flash-Next-GSQ-RCO-abliterated-IQ2_XS-00002-of-00002.gguf"
FAMILY="qwen"
MODEL="IQ2_XS"
MTP_SUBDIR="strata/rt"

# --- Rutas ---
RAIZ="${RAIZ:-$HOME/models/gguf}"
MODEL_DIR="$RAIZ/$ID"
GGUF="$MODEL_DIR/$SHARD1"
MTP_DIR="$MODEL_DIR/$MTP_SUBDIR"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/share/strata/install}"
CFG="$INSTALL_DIR/strata-$ID.json"
PRIVATE_INSTALL="$MODEL_DIR/install"
PACK_NAME="${FAMILY}-${MODEL,,}"
PACK="$MODEL_DIR/packs/$PACK_NAME"

# --- Sampling para Qwen3.8-Flash-Next (modo instruct) ---
# Temp 0.7 / top_p 0.80 / presence 1.5 / repetition 1.0 — el "instruct" de la
# model card. Para modo "think" sube a temp 1.0, top_p 0.95, presence 0.0.
SAMPLING='{"temperature":0.7,"top_p":0.80,"top_k":20,"min_p":0.0,"presence_penalty":1.5,"repetition_penalty":1.0}'

# --- Variables por defecto (sobrescribibles por env) ---
CTX="${CTX:-31072}"
KV="${KV:-int8}"
VISION="${VISION:-off}"
RESERVA="${RESERVA:-512}"
MTP="${MTP:-on}"
TUNE="${TUNE:-thorough}"
PORT="${PORT:-9999}"

# --- Helpers ---
say()  { printf '\n\033[1;36m== %s\033[0m\n' "$*"; }
info() { printf '   %s\n' "$*"; }
err()  { printf '\033[1;31m!! %s\033[0m\n' "$*" >&2; }
die()  { err "$*"; exit 1; }

cargar_vars() {
  CTX="${CTX:-32768}"
  KV="${KV:-k8v4}"
  RESERVA="${RESERVA:-512}"
  MTP="${MTP:-on}"
  TUNE="${TUNE:-quick}"
  PORT="${PORT:-9999}"
  case "${VISION:-off}" in
    on|yes)    VISION_OPT="yes";  VISION_LAUNCH="on"  ;;
    off|no)    VISION_OPT="no";   VISION_LAUNCH="off" ;;
    cpu)       VISION_OPT="cpu";  VISION_LAUNCH="cpu" ;;
    gpu)       VISION_OPT="gpu";  VISION_LAUNCH="gpu" ;;
    none)      VISION_OPT="none"; VISION_LAUNCH="off" ;;
    *) die "VISION inválido: $VISION (usa off|on|gpu|cpu|none)" ;;
  esac
  case "$MTP" in
    on|off) ;;
    *) die "MTP debe ser on|off (recibido: $MTP)" ;;
  esac
}

hf_bin() {
  if command -v hf >/dev/null 2>&1; then echo hf
  elif command -v huggingface-cli >/dev/null 2>&1; then echo huggingface-cli
  else die "instala huggingface_hub[hf_transfer]: pip install -U 'huggingface_hub[hf_transfer]'"
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
  local want_mtp="$1"
  mkdir -p "$MODEL_DIR"

  # Shards
  if [[ -f "$GGUF" && -f "$MODEL_DIR/$SHARD2" ]]; then
    info "shards ya presentes en $MODEL_DIR"
  else
    say "descargando shards ($QUANT)"
    HF_HUB_ENABLE_HF_TRANSFER=1 "$(hf_bin)" download "$REPO" \
      --include "$QUANT/*" --local-dir "$MODEL_DIR"
    # hf download conserva la estructura del repo: MODEL_DIR/IQ2_XS/*.gguf
    # → aplanar a MODEL_DIR/*.gguf
    if [[ -d "$MODEL_DIR/$QUANT" ]]; then
      mv "$MODEL_DIR/$QUANT/"*.gguf "$MODEL_DIR/" 2>/dev/null || true
      rmdir "$MODEL_DIR/$QUANT" 2>/dev/null || true
    fi
    [[ -f "$GGUF" ]] || die "falta $GGUF"
    [[ -f "$MODEL_DIR/$SHARD2" ]] || die "falta $MODEL_DIR/$SHARD2"
    info "shards OK ($(du -sh "$GGUF" | cut -f1) + $(du -sh "$MODEL_DIR/$SHARD2" | cut -f1))"
  fi

  # MTP
  if [[ "$want_mtp" == "on" ]]; then
    if [[ -f "$MTP_DIR/dense.bin" && -f "$MTP_DIR/experts.bin" ]]; then
      info "MTP ya presente: $MTP_DIR"
    else
      say "descargando MTP ($MTP_SUBDIR)"
      HF_HUB_ENABLE_HF_TRANSFER=1 "$(hf_bin)" download "$REPO" \
        --include "$MTP_SUBDIR/*" --local-dir "$MODEL_DIR"
      [[ -f "$MTP_DIR/dense.bin" ]] || die "MTP no descargado (falta dense.bin)"
      info "MTP OK: $MTP_DIR"
    fi
  else
    info "MTP=off: no se descarga ni se usa el runtime MTP"
  fi
}

# --- Verificar ---
verificar() {
  say "verificando $ID"
  strata models import "$GGUF" --plan
}

# --- Instalar (cfg desde setup.py) ---
instalar() {
  resolver_strata
  [[ ! -L "$PRIVATE_INSTALL" ]] || die "install privado no puede ser symlink"
  mkdir -p "$PRIVATE_INSTALL" "$INSTALL_DIR"

  say "instalando (setup.py --setup)"
  STRATA_CONTROL_INSTALL=1 STRATA_INSTALL_DIR="$PRIVATE_INSTALL" PYTHONNOUSERSITE=1 \
    "$STRATA_PY" "$STRATA_DIR/setup.py" --setup \
      --family "$FAMILY" --model "$MODEL" \
      --gguf-dir "$MODEL_DIR" --data-dir "$MODEL_DIR" \
      --low-ram off --mtp "$MTP" --context "$CTX" --kv "$KV" \
      --vision "$VISION_OPT" --vram-reserve-mib "$RESERVA" \
      --no-start --yes

  local generado
  generado=$(ls -1 "$PRIVATE_INSTALL"/strata-*.json 2>/dev/null | head -1 || true)
  [[ -n "$generado" && -f "$generado" ]] || die "setup.py no generó cfg en $PRIVATE_INSTALL"
  cp "$generado" "$CFG"
  info "cfg: $CFG"
}

# --- Pack nativo ---
preparar_pack() {
  resolver_strata
  local llama="$PRIVATE_INSTALL/third_party/llama.cpp"
  local llama_source="$STRATA_DIR/third_party/llama.cpp"
  [[ -f "$llama_source/ggml/CMakeLists.txt" && -d "$llama_source/gguf-py" ]] \
    || die "falta llama.cpp/gguf-py en $llama_source"
  if [[ -e "$llama" || -L "$llama" ]]; then
    [[ -L "$llama" && "$(readlink -f "$llama")" == "$(readlink -f "$llama_source")" ]] \
      || die "$llama existe y no apunta a $llama_source; no se modifica"
  else
    mkdir -p "$(dirname "$llama")"
    ln -sfn -- "$llama_source" "$llama"
  fi

  say "construyendo pack nativo de $ID"
  mkdir -p "$MODEL_DIR/packs"
  STRATA_CONTROL_INSTALL=1 STRATA_INSTALL_DIR="$PRIVATE_INSTALL" PYTHONNOUSERSITE=1 \
    "$STRATA_PY" "$STRATA_DIR/setup.py" --prepare-pack "$PACK" \
      --gguf-dir "$MODEL_DIR" --family "$FAMILY" --model "$MODEL" \
      >"$MODEL_DIR/prepare-pack.log" 2>&1 \
    || { sed 's/^/    /' "$MODEL_DIR/prepare-pack.log" >&2; die "no se pudo preparar el pack; mira $MODEL_DIR/prepare-pack.log"; }
  info "pack: $PACK"
}

# --- Parche ---
parchar() {
  [[ -f "$CFG" ]] || die "no hay cfg (preparar primero)"
  [[ -d "$PACK" ]] || die "no hay pack (preparar primero)"

  local mtp_path=""
  if [[ "$MTP" == "on" ]]; then
    [[ -f "$MTP_DIR/dense.bin" ]] || die "MTP=on pero $MTP_DIR/dense.bin no existe"
    mtp_path="$MTP_DIR"
  fi

  MTP_PATH="$mtp_path" SAMPLING_JSON="$SAMPLING" PACK_NAME="$PACK_NAME" \
    python3 - "$CFG" "$MODEL_DIR" "$RESERVA" <<'PYEOF'
import json, os, pathlib, sys
p = pathlib.Path(sys.argv[1])
model_dir = pathlib.Path(sys.argv[2])
reserve = sys.argv[3]
mtp_path = os.environ.get("MTP_PATH", "")
sampling_json = os.environ.get("SAMPLING_JSON", "")
pack_name = os.environ.get("PACK_NAME", "")

cfg = json.loads(p.read_text())
base = cfg.get("args") or cfg.get("args_written") or []
if not isinstance(base, list) or not all(isinstance(a, str) for a in base):
    sys.exit("args debe ser lista de strings")

sampling_flags = {"--temp","--temperature","--top-p","--top-k","--min-p",
                  "--presence-penalty","--repeat-penalty","--repetition-penalty",
                  "--repeat-last-n"}
# Los que reescribimos siempre (no confiar en setup.py si reutiliza config)
replace_flags = {"--pack", "--vram-reserve-mib", "--mtp"}
# --expert-profile NO se toca: para qwen, setup.py genera uno válido.
own_pack = str(model_dir / "packs" / pack_name)

out, i = [], 0
while i < len(base):
    a = base[i]
    if a == "--mmap-experts":
        i += 1; continue
    if a in replace_flags:
        i += 2; continue
    if any(a.startswith(f+"=") for f in replace_flags):
        i += 1; continue
    if a in sampling_flags:
        i += 2; continue
    if any(a.startswith(f+"=") for f in sampling_flags):
        i += 1; continue
    out.append(a); i += 1

out += ["--pack", own_pack, "--vram-reserve-mib", reserve]
if mtp_path:
    out += ["--mtp", mtp_path]

cfg["args"] = out
cfg["tokenizer"] = str(model_dir / "packs" / pack_name / "tokenizer")
cfg["model_name"] = p.stem.replace("strata-", "")
cfg["log"] = str(p.with_suffix(".log"))
if sampling_json:
    try:
        cfg["sampling"] = json.loads(sampling_json)
    except json.JSONDecodeError as e:
        sys.exit(f"SAMPLING no es JSON válido: {e}")
else:
    cfg.pop("sampling", None)

p.write_text(json.dumps(cfg, indent=2))
print("   args:", " ".join(out))
if sampling_json:
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
except (OSError, ValueError, TypeError, AttributeError) as exc:
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
  if [[ "$MTP" == "on" && -f "$MTP_DIR/dense.bin" ]]; then
    info "MTP: $MTP_DIR"
  else
    info "MTP: off"
  fi
  info "Ctrl-C para parar; un motor huérfano debe detenerlo su operador"
  exec strata serve --profile "$CFG" --port "$PORT"
}

# --- Registrar en strata launch ---
registrar() {
  [[ -f "$CFG" ]] || die "no hay cfg (preparar primero)"
  parchar
  resolver_strata
  say "registrando $ID en strata launch"
  local preset_name="Qwen-Abliterated IQ2_XS"
  info "guardando preset \"$preset_name\"…"
  if strata launch \
       --model-path "$GGUF" \
       --family "$FAMILY" \
       --model "$MODEL" \
       --context "$CTX" \
       --kv "$KV" \
       --vision "$VISION_LAUNCH" \
       --mtp "$MTP" \
       --tune skip \
       --save-preset "$preset_name" \
       --yes --no-tui 2>&1 | tail -5; then
    info "preset guardado: \"$preset_name\""
  else
    err "no se pudo guardar el preset; usa '$0 lanzar' o 'strata serve --profile $CFG'"
  fi
  if strata launch --list-presets 2>/dev/null | grep -q "$preset_name"; then
    info "confirmado: aparece en 'strata launch --list-presets'"
  fi
}

# --- Estado ---
estado() {
  say "GPU"
  nvidia-smi --query-gpu=name,memory.used,memory.total --format=csv 2>/dev/null || true
  say "motores de Strata"
  pgrep -af 'components/engine/.*/strata|serve[.]server' || info "ninguno"
  say "ficheros de $ID"
  [[ -f "$GGUF" ]] && info "shard 1: $GGUF" || info "shard 1: FALTA"
  [[ -f "$MODEL_DIR/$SHARD2" ]] && info "shard 2: $MODEL_DIR/$SHARD2" || info "shard 2: FALTA"
  [[ -d "$PACK" ]] && info "pack:    $PACK" || info "pack:    FALTA"
  [[ -f "$MTP_DIR/dense.bin" ]] && info "MTP:     $MTP_DIR" || info "MTP:     FALTA (o MTP=off)"
  [[ -f "$CFG" ]] && info "cfg:     $CFG" || info "cfg:     FALTA"
}

# --- Preparar (todo en uno) ---
cmd_preparar() {
  local want_mtp="$MTP"
  for arg in "$@"; do
    [[ "$arg" == "--no-mtp" ]] && want_mtp="off"
  done
  MTP="$want_mtp"

  say "preparando $ID"
  info "MTP=$MTP, CTX=$CTX, KV=$KV, VISION=$VISION_OPT, RESERVA=$RESERVA"

  descargar "$MTP"
  verificar
  instalar
  preparar_pack
  parchar

  if [[ "$TUNE" != "skip" ]]; then
    say "afinando (TUNE=$TUNE)"
    afinar || err "tune falló; el modelo está preparado igual"
  fi

  registrar || err "registro falló; usa '$0 lanzar' para servirlo igualmente"
  say "listo: $ID preparado"
  estado
}

# --- Reinstalar (solo cfg, sin tocar pack) ---
reinstalar() {
  [[ -d "$PACK" ]] || die "falta pack (preparar primero)"
  instalar
  parchar
  say "$ID reinstalado (cfg regenerado, pack y shards intactos)"
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
