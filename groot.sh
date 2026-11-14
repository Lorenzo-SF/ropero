#!/usr/bin/env bash
#
# groot-strata.sh — preparar, afinar, lanzar y registrar los packs Gyro-M de Groot.
#
#   preparar            descarga + verifica + instala + pack + expert-profile
#                       + parchea cfg + afina (tune) + registra (launch)
#   reinstalar <id>     regenera el cfg (kv/vision/mtp/context) sin tocar pack/perfil
#   registrar <id>      parchea cfg e intenta guardar preset para strata launch
#   afinar <id>         mide este PC y guarda su preset (Strata lo aplica al arrancar)
#   lanzar <id>         arranca ese modelo (Ctrl-C para parar)
#   ciclo <id>          afinar + lanzar
#   todos               alias de preparar (que ya incluye tune + registro)
#   estado              qué hay instalado, quién tiene la GPU
#   parar               rechaza parar procesos sin propiedad verificable
#
# Variables de entorno:
#   CTX=131072     contexto en tokens (o con sufijo k/K)
#   MTP=on|off     multi-token prediction
#   VISION=on|off|cpu|gpu|none
#   RESERVA=512    MiB de VRAM a dejar libres (baja si la iGPU renderiza)
#   NO_TUNE=1      'preparar' no ejecuta el tune (solo prepara y registra)
#
set -euo pipefail

RAIZ="${RAIZ:-$HOME/models/gguf}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/share/strata/install}"
PUBLICO="Qwen3.8-Flash-Next-Gyro-M-TQ2_0.gguf"
FAMILIA="gyro"
MODELO="Gyro-M"

# --- Controles de usuario ---
CTX="${CTX:-131072}"
MTP="${MTP:-on}"
VISION="${VISION:-off}"
RESERVA="${RESERVA:-512}"
MODO_TUNE="${MODO_TUNE:-thorough}"
PUERTO="${PUERTO:-9999}"
NO_TUNE="${NO_TUNE:-0}"

# Inicializar para que 'set -u' no explote si alguna función usa las OPT antes
KV_OPT="${KV_OPT:-int8}"
MTP_OPT="${MTP_OPT:-off}"
VISION_OPT="${VISION_OPT:-no}"

MODELOS=(
  "groot-boxy|BoxyFit/Groot-65B-A6B-GyroM|Groot-65B-A6B-GyroM-Q2_0.gguf"
  "groot-heretic|phantomic12/Groot-65B-A6B-261023-Heretic-GyroM|Groot-Heretic-GyroM-Q2_0.gguf"
)

# ----------------------------------------------------------------------- avisos
say()  { printf '\n\033[1;36m== %s\033[0m\n' "$*"; }
info() { printf '   %s\n' "$*"; }
err()  { printf '\033[1;31m!! %s\033[0m\n' "$*" >&2; }
die()  { err "$*"; exit 1; }

# ------------------------------------------------------------------- utilidades
resolver_strata() {
  local launcher active
  launcher=$(command -v strata) || die "no encuentro strata en PATH"
  launcher=$(readlink -f "$launcher") || die "no puedo resolver el launcher"
  active=$(dirname "$(dirname "$launcher")")
  [[ -f "$active/strata.py" && -f "$active/setup.py" ]] || die "layout de Strata no reconocido: $launcher"
  [[ -z "${STRATA_DIR:-}" || "$(readlink -f "$STRATA_DIR")" == "$active" ]] \
    || die "STRATA_DIR no corresponde al launcher activo"
  STRATA_DIR="$active"
  STRATA_PY="${STRATA_PY:-${STRATA_PYTHON:-$active/python/bin/python3}}"
  [[ -x "$STRATA_PY" && "$(readlink -f "$STRATA_PY")" == "$(readlink -f "$active/python/bin/python3")" ]] \
    || die "se requiere el Python del mismo terminal activo: $active/python/bin/python3"
  export STRATA_PYTHON="$STRATA_PY"
}

hf_bin() {
  if command -v hf >/dev/null 2>&1; then echo hf
  elif command -v huggingface-cli >/dev/null 2>&1; then echo huggingface-cli
  else die "no está el cliente de Hugging Face: pip install -U 'huggingface_hub[hf_transfer]'"
  fi
}

tamano_remoto() {
  local repo="$1" file="$2"
  curl -sL --max-time 60 "https://huggingface.co/api/models/$repo/tree/main" \
  | python3 -c '
import json,sys
repo_file = sys.argv[1]
try:
    for e in json.load(sys.stdin):
        if e.get("path") == repo_file:
            print(e.get("size") or (e.get("lfs") or {}).get("size") or 0); break
    else:
        print(0)
except Exception:
    print(0)
' "$file"
}

modelo_por_id() {
  local id="$1" m
  for m in "${MODELOS[@]}"; do
    [[ "${m%%|*}" == "$id" ]] && { echo "$m"; return 0; }
  done
  die "id desconocido: $id (usa: ${MODELOS[*]%%|*})"
}

cfg_de()  { echo "$INSTALL_DIR/strata-$1.json"; }
gguf_de() { echo "$RAIZ/$1/$PUBLICO"; }

parar_motores() {
  die "no se paran procesos: este script no registra propiedad de motores; detenlos con su operador"
}

exigir_pack() {
  local id="$1"
  [[ -d "$RAIZ/$id/packs/gyro-gyro-m" ]] || die "falta el pack de $id (ejecuta: $0 preparar)"
  [[ -f "$RAIZ/$id/expert-profile.bin" ]] || die "falta el expert-profile de $id (ejecuta: $0 preparar)"
}

exigir_gpu_libre() {
  local procesos memoria fila pid vram usado idx=0
  procesos=$(nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits 2>/dev/null) \
    || die "no se puede comprobar quién ocupa la GPU"
  while IFS= read -r fila || [[ -n "$fila" ]]; do
    [[ -z "${fila//[[:space:]]/}" ]] && continue
    IFS=, read -r pid vram <<<"$fila"
    pid="${pid//[[:space:]]/}"; vram="${vram//[[:space:]]/}"
    [[ "$pid" =~ ^[0-9]+$ && "$vram" =~ ^[0-9]+$ ]] \
      || die "estado de procesos GPU desconocido: $fila"
    (( vram <= 500 )) || die "GPU ocupada por proceso $pid ($vram MiB); no se matan"
  done <<<"$procesos"
  memoria=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits) \
    || die "no se puede comprobar la VRAM"
  while IFS= read -r usado || [[ -n "$usado" ]]; do
    usado="${usado//[[:space:]]/}"
    [[ -z "$usado" ]] && continue
    [[ "$usado" =~ ^[0-9]+$ ]] || die "VRAM no válida: $usado"
    (( usado <= 500 )) || die "GPU $idx con $usado MiB ocupados; no se matan procesos"
    idx=$((idx + 1))
  done <<<"$memoria"
}

# ------------------------------------------------------------------- auto-ajuste
auto_ajustar() {
  local ctx_num
  ctx_num=$(echo "$CTX" | tr -d 'kK')
  if [[ ! "$ctx_num" =~ ^[0-9]+$ ]]; then
    die "CTX debe ser un número (ej. 131072 o 128k)"
  fi
  if [[ "$CTX" == *k* || "$CTX" == *K* ]]; then
    ctx_num=$((ctx_num * 1024))
  fi

  if (( ctx_num >= 131072 )); then
    KV_OPT="k8v4"
  elif (( ctx_num >= 65536 )); then
    KV_OPT="q4_0"
  else
    KV_OPT="int8"
  fi

  if [[ "$MTP" == "on" && $ctx_num -le 131072 ]]; then
    MTP_OPT="on"
  else
    MTP_OPT="off"
  fi

  # setup.py acepta --vision {yes,no,none,gpu,cpu}; NO 'off'.
  case "$VISION" in
    on|yes|gpu)   VISION_OPT="gpu" ;;
    cpu)          VISION_OPT="cpu" ;;
    none)         VISION_OPT="none" ;;
    off|no)       VISION_OPT="no" ;;
    *) die "VISION debe ser on|off|cpu|gpu|none (recibido: $VISION)" ;;
  esac

  info "Auto-ajuste: CTX=$ctx_num, KV=$KV_OPT, MTP=$MTP_OPT, vision=$VISION_OPT, reserva=${RESERVA} MiB"
  export KV_OPT MTP_OPT VISION_OPT
}

# ------------------------------------------------------------------- preparar
descargar() {
  local id="$1" repo="$2" file="$3"
  local dir="$RAIZ/$id"
  local destino="$dir/$PUBLICO"
  mkdir -p "$dir"
  local remoto; remoto=$(tamano_remoto "$repo" "$file")
  [[ "$remoto" == "0" ]] && die "no encuentro $file en $repo"

  local cand
  for cand in "$destino" "$dir/$file"; do
    if [[ -f "$cand" ]] && [[ "$(stat -c%s "$cand")" == "$remoto" ]]; then
      [[ "$cand" != "$destino" ]] && mv "$cand" "$destino"
      info "ya está ($remoto bytes): $destino"
      return 0
    fi
  done

  info "descargando $repo/$file ($(( remoto / 1000000 )) MB)"
  HF_HUB_ENABLE_HF_TRANSFER=1 "$(hf_bin)" download "$repo" "$file" --local-dir "$dir"
  [[ -f "$dir/$file" ]] || die "la descarga no dejó $dir/$file"
  mv "$dir/$file" "$destino"
  info "guardado como $PUBLICO (setup.py sólo encuentra ese nombre)"
}

verificar() {
  local id="$1"
  say "verificando $id"
  strata models import "$(gguf_de "$id")" --plan
}

instalar() {
  local id="$1"
  local privado="$RAIZ/$id/install"
  local generado="$privado/strata-gyro-gyro-m.json"
  resolver_strata
  [[ ! -L "$privado" ]] || die "install privado no puede ser un enlace: $privado"
  mkdir -p "$privado" "$INSTALL_DIR"
  say "preparando el profile de $id"
  STRATA_CONTROL_INSTALL=1 STRATA_INSTALL_DIR="$privado" PYTHONNOUSERSITE=1 \
    "$STRATA_PY" "$STRATA_DIR/setup.py" --setup --family "$FAMILIA" --model "$MODELO" \
    --gguf-dir "$RAIZ/$id" --data-dir "$RAIZ/$id" \
    --low-ram off --mtp "$MTP_OPT" --context "$CTX" --kv "$KV_OPT" --vision "$VISION_OPT" \
    --vram-reserve-mib "$RESERVA" \
    --no-start --yes

  [[ -f "$generado" && ! -L "$generado" ]] || die "setup no dejó el run config privado para $id"
  cp "$generado" "$(cfg_de "$id")"
  info "profile: $(cfg_de "$id")"
  # No parchear aquí: pack y expert-profile aún no existen.
}

# -----------------------------------------------------------------------------
# generar_profile <id> — expert-profile PROPIO de este GGUF (48x256)
# -----------------------------------------------------------------------------
generar_profile() {
  local id="$1"
  resolver_strata
  local pack="$RAIZ/$id/packs/gyro-gyro-m"
  local prof="$RAIZ/$id/expert-profile.bin"
  [[ -d "$pack" ]] || die "no hay pack para $id (primero: preparar_pack $id)"

  "$STRATA_PY" - "$pack" "$prof" <<'PYEOF2'
import re, struct, sys
pack, out = sys.argv[1], sys.argv[2]

layers = set()
with open(f"{pack}/index.txt", encoding="utf-8", errors="replace") as fh:
    for line in fh:
        m = re.match(r"blk\.(\d+)\.", line)
        if m:
            layers.add(int(m.group(1)))
nl = max(layers) + 1 if layers else 0

ne = 0
with open(f"{pack}/native_experts.txt", encoding="utf-8", errors="replace") as fh:
    for line in fh:
        m = re.search(r"n_expert (\d+)", line)
        if m:
            ne = int(m.group(1)); break

if not nl or not ne or layers != set(range(nl)) or nl > 65536 or ne > 65536:
    sys.exit(f"no se pudo deducir la geometría del pack: nl={nl} ne={ne}")

pairs = []
for e in range(ne):
    for l in range(nl):
        pairs.append((l, e))
n = len(pairs)
pos = {p: i for i, p in enumerate(pairs)}
inv = [pos[(l, e)] for l in range(nl) for e in range(ne)]

blob = bytearray()
blob += b"STRP"
blob += struct.pack("<5I", 1, nl, ne, n, n)
blob += struct.pack(f"<{2*n}H", *[x for p in pairs for x in p])
blob += struct.pack(f"<{n}I", *inv)
with open(out, "wb") as fh:
    fh.write(bytes(blob))
print(f"   profile {nl}x{ne} ({len(blob)} bytes): {out}")
PYEOF2
}

# -----------------------------------------------------------------------------
# preparar_pack <id> — pack PROPIO de este GGUF
# -----------------------------------------------------------------------------
preparar_pack() {
  local id="$1"
  resolver_strata
  local pack="$RAIZ/$id/packs/gyro-gyro-m"
  local setup_py="$STRATA_DIR/setup.py"
  [[ -f "$setup_py" ]] || die "no encuentro setup.py de Strata en $STRATA_DIR"
  local llama="$RAIZ/$id/install/third_party/llama.cpp"
  local llama_source="$STRATA_DIR/third_party/llama.cpp"
  [[ -f "$llama_source/ggml/CMakeLists.txt" && -d "$llama_source/gguf-py" ]] \
    || die "falta llama.cpp/gguf-py en $llama_source; instálalo previamente"
  if [[ -e "$llama" || -L "$llama" ]]; then
    [[ -L "$llama" && "$(readlink -f "$llama")" == "$(readlink -f "$llama_source")" ]] \
      || die "la ruta $llama ya existe y no apunta a $llama_source; no se modifica"
  else
    mkdir -p "$(dirname "$llama")"
    ln -sfn -- "$llama_source" "$llama"
  fi
  say "preparando el pack propio de $id"
  mkdir -p "$RAIZ/$id/packs"
  STRATA_CONTROL_INSTALL=1 STRATA_INSTALL_DIR="$RAIZ/$id/install" PYTHONNOUSERSITE=1 \
    "$STRATA_PY" "$setup_py" --prepare-pack "$pack" \
      --gguf-dir "$RAIZ/$id" --family "$FAMILIA" --model "$MODELO" \
      >"$RAIZ/$id/prepare-pack.log" 2>&1 \
    || { sed 's/^/    /' "$RAIZ/$id/prepare-pack.log" >&2; die "no se pudo preparar el pack de $id"; }
  info "pack: $pack"
}

# -----------------------------------------------------------------------------
# patch_python — pack/perfil propios + reserva + sampling anti-bucle
# -----------------------------------------------------------------------------
patch_python() {
  local id="$1" cfg="${2:-}"
  [[ -n "$cfg" ]] || cfg="$(cfg_de "$id")"
  python3 - "$cfg" "$RAIZ/$id" "$id" "$RESERVA" <<'EOF'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
model_dir = pathlib.Path(sys.argv[2])
model_id = sys.argv[3]
reserve = sys.argv[4]
if not reserve.isdecimal():
    sys.exit("RESERVA debe ser un entero no negativo (MiB)")

cfg = json.loads(p.read_text())

sampling_flags = {
    "--temp", "--temperature", "--top-p", "--top-k", "--min-p",
    "--presence-penalty", "--repeat-penalty", "--repetition-penalty",
    "--repeat-last-n",
}

own_pack = str(model_dir / "packs" / "gyro-gyro-m")
own_prof = str(model_dir / "expert-profile.bin")

base = cfg.get("args") or cfg.get("args_written") or []
if not isinstance(base, list) or not all(isinstance(a, str) for a in base):
    sys.exit("args debe ser una lista de strings")

def consume_val(i, flag):
    if i + 1 >= len(base) or base[i + 1].startswith("--"):
        sys.exit(f"falta valor para {flag}")
    return i + 2

out = []
i = 0
while i < len(base):
    a = base[i]
    if a == "--mmap-experts":
        i += 1; continue
    if a in ("--pack", "--expert-profile", "--vram-reserve-mib"):
        i = consume_val(i, a); continue
    if any(a.startswith(flag + "=") for flag in ("--pack", "--expert-profile", "--vram-reserve-mib")):
        i += 1; continue
    if a in sampling_flags:
        i = consume_val(i, a); continue
    if any(a.startswith(flag + "=") for flag in sampling_flags):
        i += 1; continue
    out.append(a); i += 1

out += ["--pack", own_pack, "--expert-profile", own_prof,
        "--vram-reserve-mib", reserve]

# Aviso: --mtp apuntando a otro modelo (bug o recurso compartido de setup.py).
for i, arg in enumerate(out):
    if arg == "--mtp" and i + 1 < len(out):
        val = out[i+1]
        if "/models/gguf/" in val:
            parts = val.split("/models/gguf/")
            if len(parts) == 2:
                src_id = parts[1].split("/")[0]
                if src_id != model_id:
                    print(f"   AVISO: --mtp={val} apunta a {src_id} (no a {model_id}); "
                          f"si el MTP es compartido, es correcto. Si no, ajústalo a mano.")
        break

cfg["args"] = out
cfg["tokenizer"] = str(model_dir / "packs" / "gyro-gyro-m" / "tokenizer")
cfg["model_name"] = p.stem.replace("strata-", "")
cfg["log"] = str(p.with_suffix(".log"))

# Sampling para Groot-65B-A6B: base de la model card + anti-bucle.
cfg["sampling"] = {
    "temperature": 1.0,
    "top_p": 0.95,
    "top_k": 20,
    "min_p": 0.05,
    "presence_penalty": 1.5,
    "repetition_penalty": 1.1,
}

p.write_text(json.dumps(cfg, indent=2))
print("   args:", " ".join(out))
print("   sampling:", json.dumps(cfg["sampling"]))
EOF
}

parchar() {
  local id="$1"
  exigir_pack "$id"
  [[ -f "$(cfg_de "$id")" ]] || die "no hay profile para $id"
  patch_python "$id" "$(cfg_de "$id")"
}

# ------------------------------------------------------------------- registrar
registrar() {
  local id="$1"
  modelo_por_id "$id" >/dev/null
  exigir_pack "$id"
  auto_ajustar
  parchar "$id"
  resolver_strata

  local preset_name="Groot ${id#groot-}"
  say "$id registrado en $INSTALL_DIR"
  info "cfg: $(cfg_de "$id")"

  # Construir el preset usando los flags reales de 'strata launch'.
  # --model-path apunta al GGUF; --family gyro y --model Gyro-M identifican
  # la arquitectura; el resto replica los valores del cfg.
  info "guardando preset \"$preset_name\" en strata launch…"
  if strata launch \
       --model-path "$(gguf_de "$id")" \
       --family "$FAMILIA" \
       --model "$MODELO" \
       --context "$CTX" \
       --kv "$KV_OPT" \
       --vision "$VISION_OPT" \
       --mtp "$MTP_OPT" \
       --tune skip \
       --save-preset "$preset_name" \
       --yes --no-tui 2>&1 | tail -5; then
    info "preset guardado: \"$preset_name\""
    info "se verá en 'strata launch' y en 'strata launch --list-presets'"
  else
    err "no se pudo guardar el preset automáticamente."
    info "guárdalo a mano desde el TUI: elige 'Use model files I already have'"
    info "y apunta a: $(gguf_de "$id")"
  fi

  # Diagnóstico
  if strata launch --list-presets 2>/dev/null | grep -q "$preset_name"; then
    info "confirmado: el preset aparece en 'strata launch --list-presets'"
  else
    info "no aparece en --list-presets (revisa la salida de arriba)"
  fi

  say "para lanzar: strata launch --preset \"$preset_name\""
  say "o:          $0 lanzar $id"
}


# --------------------------------------------------------------------- afinar
afinar() {
  local id="$1" cfg; cfg=$(cfg_de "$id")
  modelo_por_id "$id" >/dev/null
  [[ -f "$cfg" ]] || die "no hay profile para $id (primero: preparar)"
  exigir_pack "$id"
  resolver_strata
  say "afinando $id (modo $MODO_TUNE, GPU libre)"
  exigir_gpu_libre
  parchar "$id"

  mkdir -p "$RAIZ/$id"
  local jlog="$RAIZ/$id/tune.json" tlog="$RAIZ/$id/tune.log"
  local rc=0
  strata tune auto "$cfg" --mode "$MODO_TUNE" --output json --name "$id" \
    >"$jlog" 2>"$tlog" || rc=$?
  if [[ -s "$tlog" ]]; then tail -n 12 "$tlog" >&2; fi
  if ! python3 - "$jlog" "$rc" <<'PYEOF'
import json, sys
try:
    data = json.load(open(sys.argv[1], encoding="utf-8"))
    result = data.get("result") or {}
    outcome = result.get("outcome")
    if int(sys.argv[2]) or data.get("ok") is not True or outcome not in ("improved", "default_confirmed"):
        raise ValueError(f"exit={sys.argv[2]}, outcome={outcome}: {result.get('message', '')}")
    if not (result.get("preset") or {}).get("id"):
        raise ValueError("resultado medido pero sin preset guardado")
    print(f"   tune: {outcome}; preset guardado: {result['preset']['id']}")
except (OSError, ValueError, TypeError, AttributeError) as exc:
    sys.exit(f"tune no completado: {exc}")
PYEOF
  then
    err "afinación fallida; consulta $jlog y $tlog"
    (( rc != 0 )) && return "$rc"
    return 1
  fi
  parchar "$id"
}

# -------------------------------------------------------------------- lanzar
lanzar() {
  local id="$1" cfg; cfg=$(cfg_de "$id")
  modelo_por_id "$id" >/dev/null
  [[ -f "$cfg" ]] || die "no hay profile para $id (primero: preparar)"
  exigir_pack "$id"
  say "lanzando $id"
  resolver_strata
  exigir_gpu_libre
  info "profile: $cfg"
  info "modelo:  $(gguf_de "$id")"
  info "Ctrl-C para parar"
  exec strata serve --profile "$cfg" --port "$PUERTO"
}

# --------------------------------------------------------------------- estado
estado() {
  say "GPU"
  nvidia-smi --query-gpu=name,memory.used,memory.total --format=csv 2>/dev/null || true
  say "motores de Strata"
  pgrep -af 'components/engine/.*/strata|serve[.]server' || info "ninguno"
  say "profiles instalados"
  local _tmp
  if ! _tmp=$(mktemp "${TMPDIR:-/tmp}/strata-list.XXXXXX"); then
    err "no se pudo crear temporal"; return 0
  fi
  if strata models list --installed --output json >"$_tmp"; then
    if ! python3 - "$_tmp" <<'PYEOF2'
import json, sys
try:
    models = json.load(open(sys.argv[1], encoding="utf-8")).get("models") or []
    for m in models:
        print("   %-24s %-28s ctx=%-8s %s" % (m.get("id"), m.get("name"),
              m.get("context_tokens"), m.get("component_status")))
except Exception as e:
    sys.exit(f"no se pudo leer el listado: {e}")
PYEOF2
    then err "listado no válido"; fi
  else
    err "strata models list falló; estado parcial"
  fi
  rm -f "$_tmp"
  return 0
}

# ------------------------------------------------------------------------ main
cmd_preparar() {
  auto_ajustar
  local m id repo file
  for m in "${MODELOS[@]}"; do
    IFS='|' read -r id repo file <<<"$m"
    say "$id  ($repo)"
    descargar "$id" "$repo" "$file"
    verificar "$id"
    instalar "$id"
    preparar_pack "$id"
    generar_profile "$id"
    parchar "$id"
  done
  say "preparación terminada; ahora afino y registro"
  for m in "${MODELOS[@]}"; do
    IFS='|' read -r id _ _ <<<"$m"
    if [[ "$NO_TUNE" == "1" ]]; then
      info "NO_TUNE=1: se salta el tune de $id"
    else
      say "afinando $id (MODO_TUNE=$MODO_TUNE; si la GPU está ocupada fallará y continúo)"
      afinar "$id" || err "afinar $id falló; continúo con registrar"
    fi
    registrar "$id" || err "registrar $id falló"
  done
  say "listo: preparados, afinados y registrados"
  estado
}

reinstalar() {
  local id="$1"
  modelo_por_id "$id" >/dev/null
  exigir_pack "$id"
  auto_ajustar
  instalar "$id"
  parchar "$id"
  say "$id reinstalado (cfg regenerado, pack y profile intactos)"
}

cmd_todos() {
  cmd_preparar
}

case "${1:-}" in
  preparar)   cmd_preparar ;;
  reinstalar) [ $# -ge 2 ] || die "uso: $0 reinstalar <id>"; reinstalar "$2" ;;
  registrar)  [ $# -ge 2 ] || die "uso: $0 registrar <id>";  registrar "$2" ;;
  afinar)     [ $# -ge 2 ] || die "uso: $0 afinar <id>";    afinar "$2" ;;
  lanzar)     [ $# -ge 2 ] || die "uso: $0 lanzar <id>";    lanzar "$2" ;;
  ciclo)      [ $# -ge 2 ] || die "uso: $0 ciclo <id>"
              afinar "$2"; lanzar "$2" ;;
  todos)      cmd_todos ;;
  estado)     estado ;;
  parar)      parar_motores ;;
  *)          sed -n '2,/^[^#]/p' "$0" | sed 's/^# \{0,1\}//' | sed '$d'; exit 1 ;;
esac
