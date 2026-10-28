#!/usr/bin/env bash
# Tests de la sección [build] de los TOML de modelo: fabricar el GGUF en vez
# de descargarlo.
#
# Lo que se testea NO es que la cuantizacion salga bien (eso necesita una GPU
# y 60 GB), sino la SEMANTICA:
#
#   1. el [build] del TOML se lee y se traduce al argv del gguffer
#   2. el argv es EXACTAMENTE el que se escribiria a mano
#   3. un modelo con [build] entra en el catalogo como "build", no como
#      descarga, y con el tamano de los PESOS
#   4. la fabricacion deja el GGUF donde el launcher lo va a buscar
#   5. una clave desconocida en [build] se avisa (un typo no puede pasar
#      desapercibido: fabricaria el GGUF SIN el imatrix)
#   6. un modelo SIN [build] se sigue descargando como siempre

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

VERBOSE=0
[[ "${1:-}" == "-v" ]] && VERBOSE=1
PASS=0
FAIL=0
FAILED_CASES=()

ok()   { PASS=$((PASS+1)); [[ $VERBOSE -eq 1 ]] && printf '  ok   %s\n' "$1"; return 0; }
bad()  { FAIL=$((FAIL+1)); FAILED_CASES+=("$1"); printf '  FALLO %s\n' "$1"; [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; return 0; }
check() { # check <descripcion> <condicion-verdadera> [mensaje]
    if [[ "$2" == "1" ]]; then ok "$1"; else bad "$1" "${3:-}"; fi
}
cond() { if eval "$1" >/dev/null 2>&1; then echo 1; else echo 0; fi; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ropero-build.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

MODELS="$WORK/models"
mkdir -p "$MODELS/llama"
GGUF_DIR="$WORK/gguf"
mkdir -p "$GGUF_DIR"

# ── un modelo con [build] ──────────────────────────────────────────────────
cat >"$MODELS/llama/fabrica.toml" <<'EOF'
[model]
name    = "fabrica"
engine  = "llama-server"
context = 131072

[files]
gguf = "fabrica-IQ4_XS.gguf"

[download]
repo = "owner/fabrica-GGUF"
file = "fabrica-IQ4_XS.gguf"
size = "12.00 GB"

[build]
repo         = "Qwen/Qwen3-30B-A3B"
quant        = "IQ4_XS"
imatrix_repo = "bartowski/Qwen_Qwen3-30B-A3B-GGUF"
imatrix_file = "Qwen3-30B-A3B.imatrix"
keep_source  = true
size         = "61.1 GB"

[args]
base = ["--n-gpu-layers", "49"]
EOF

# ── un modelo SIN [build] (se descarga) ────────────────────────────────────
cat >"$MODELS/llama/descarga.toml" <<'EOF'
[model]
name    = "descarga"
engine  = "llama-server"
context = 131072

[files]
gguf = "descarga-Q4_K_M.gguf"

[download]
repo = "owner/descarga-GGUF"
file = "descarga-Q4_K_M.gguf"
size = "9.00 GB"

[args]
base = ["--n-gpu-layers", "49"]
EOF

# ── un modelo con [build] con una clave mal escrita ────────────────────────
cat >"$MODELS/llama/typo.toml" <<'EOF'
[model]
name    = "typo"
engine  = "llama-server"
context = 131072

[files]
gguf = "typo.gguf"

[download]
repo = "owner/typo-GGUF"
file = "typo.gguf"
size = "1.00 GB"

[build]
repo = "owner/typo"
imatrix_repos = "owner/imatrix"
quant = "IQ4_XS"

[args]
base = ["--n-gpu-layers", "49"]
EOF

MC="$REPO/bin/_modelconf.py"

# ══ 1. el argv del gguffer sale del [build] ════════════════════════════════
argv="$(python3 "$MC" build --root "$MODELS" --name fabrica 2>/dev/null)"
check "1a. el argv sale"                "$(cond '[ -n "$argv" ]')" "vacio"
check "1b. --repo del build"            "$(cond 'printf "%s\n" "$argv" | grep -qx -- "--repo"')" "$argv"
check "1c. el repo es el del build"     "$(cond 'printf "%s\n" "$argv" | grep -qx "Qwen/Qwen3-30B-A3B"')" "$argv"
check "1d. --quant del build"           "$(cond 'printf "%s\n" "$argv" | grep -qx -- "--quant"')" "$argv"
check "1e. el quant es el del build"    "$(cond 'printf "%s\n" "$argv" | grep -qx "IQ4_XS"')" "$argv"
check "1f. --imatrix-repo"              "$(cond 'printf "%s\n" "$argv" | grep -qx -- "--imatrix-repo"')" "$argv"
check "1g. el imatrix-repo es el del build" \
      "$(cond 'printf "%s\n" "$argv" | grep -qx "bartowski/Qwen_Qwen3-30B-A3B-GGUF"')" "$argv"
check "1h. --imatrix-file"              "$(cond 'printf "%s\n" "$argv" | grep -qx -- "--imatrix-file"')" "$argv"
check "1i. el imatrix-file es el del build" \
      "$(cond 'printf "%s\n" "$argv" | grep -qx "Qwen3-30B-A3B.imatrix"')" "$argv"
check "1j. --keep-source"               "$(cond 'printf "%s\n" "$argv" | grep -qx -- "--keep-source"')" "$argv"

# El "size" es INFORMATIVO: no puede salir como flag del gguffer, porque el
# gguffer no la entiende y moriria con "invalid argument".
check "1k. el size NO sale como flag"   "$(cond '! printf "%s\n" "$argv" | grep -qx -- "--size"')" "$argv"
check "1l. el size NO se pasa al gguffer" \
      "$(cond '! printf "%s\n" "$argv" | grep -qx "61.1 GB"')" "$argv"

# ══ 2. el destino lo pone quien fabrica ═══════════════════════════════════
argv2="$(python3 "$MC" build --root "$MODELS" --name fabrica --out "$GGUF_DIR/fabrica/fabrica-IQ4_XS.gguf" 2>/dev/null)"
check "2a. --out con el destino"        "$(cond 'printf "%s\n" "$argv2" | grep -qx -- "--out"')" "$argv2"
check "2b. el destino es el que se paso" \
      "$(cond 'printf "%s\n" "$argv2" | grep -qx "$GGUF_DIR/fabrica/fabrica-IQ4_XS.gguf"')" "$argv2"

# ══ 3. un modelo SIN [build] no se fabrica ════════════════════════════════
# El error va a stderr; stdout tiene que quedar VACIO (si no, el que lo
# llama leería un argv y creería que hay algo que fabricar).
out="$(python3 "$MC" build --root "$MODELS" --name descarga 2>/dev/null)"; rc=$?
err="$(python3 "$MC" build --root "$MODELS" --name descarga 2>&1 >/dev/null)"
check "3a. sin [build] -> stdout vacio"   "$(cond '[ -z "$out" ]')" "$out"
check "3b. sin [build] -> error"          "$(cond '[ $rc -ne 0 ]')" "rc=$rc"
check "3c. sin [build] -> lo explica"     "$(cond 'printf "%s\n" "$err" | grep -q "no se fabrica"')" "$err"

# ══ 4. una clave desconocida se avisa ══════════════════════════════════════
out="$(python3 "$MC" build --root "$MODELS" --name typo 2>&1 >/dev/null)"
check "4a. clave desconocida -> aviso"   "$(cond 'printf "%s\n" "$out" | grep -q "imatrix_repos"')" "$out"
check "4b. la clave mal escrita NO se pasa al gguffer" \
      "$(cond '! python3 "$MC" build --root "$MODELS" --name typo 2>/dev/null | grep -qx -- "--imatrix-repos"')" "se coló"

# ══ 5. el catalogo: build vs descarga ═════════════════════════════════════
cat >"$WORK/probe.sh" <<'SH'
set -uo pipefail
export SCRIPT_DIR="$REPO/bin"
export MODULES_DIR="$REPO/bin"
export MODELS_DIR="$MODELS"
export GGUF_DIR="$GGUF_DIR"
export SAFETENSORS_DIR="$WORK/st"
source "$REPO/bin/00-utils.sh"
source "$REPO/bin/22-models.sh"
source "$REPO/bin/21-download-models.sh"
ropero_models_load
_ropero_cat_build
for i in "${!_ROPERO_CAT_KEY[@]}"; do
  printf '%s\t%s\t%s\t%s\n' "${_ROPERO_CAT_KIND[$i]}" "${_ROPERO_CAT_KEY[$i]}" \
         "${_ROPERO_CAT_NAME[$i]}" "${_ROPERO_CAT_SIZE[$i]}"
done
SH
cat >"$WORK/probe.sh" <<SH
set -uo pipefail
export SCRIPT_DIR="$REPO/bin"
export MODULES_DIR="$REPO/bin"
export MODELS_DIR="$MODELS"
export GGUF_DIR="$GGUF_DIR"
export SAFETENSORS_DIR="$WORK/st"
source "$REPO/bin/00-utils.sh"
source "$REPO/bin/22-models.sh"
source "$REPO/bin/21-download-models.sh"
ropero_models_load
_ropero_cat_build
for i in "\${!_ROPERO_CAT_KEY[@]}"; do
  printf '%s\t%s\t%s\t%s\n' "\${_ROPERO_CAT_KIND[\$i]}" "\${_ROPERO_CAT_KEY[\$i]}" \\
         "\${_ROPERO_CAT_NAME[\$i]}" "\${_ROPERO_CAT_SIZE[\$i]}"
done
SH
catout="$(bash "$WORK/probe.sh" 2>/dev/null)"
check "5a. el modelo con build es kind=build" \
      "$(cond 'printf "%s\n" "$catout" | grep -q "^build	fabrica-IQ4_XS.gguf"')" "$catout"
check "5b. el modelo sin build es kind=gguf" \
      "$(cond 'printf "%s\n" "$catout" | grep -q "^gguf	descarga-Q4_K_M.gguf"')" "$catout"
# El tamano de una entrada de fabricacion es el de los PESOS, no el del GGUF
check "5c. el size es el de los pesos" \
      "$(cond 'printf "%s\n" "$catout" | grep -q "^build	fabrica-IQ4_XS.gguf	fabrica	61.1 GB"')" "$catout"
check "5d. el size de una descarga es el del fichero" \
      "$(cond 'printf "%s\n" "$catout" | grep -q "^gguf	descarga-Q4_K_M.gguf	descarga	9.00 GB"')" "$catout"

# ══ 6. el destino de la fabricacion es donde busca el launcher ════════════
# El launcher busca $GGUF_DIR/<carpeta del modelo>/<nombre del GGUF>. Si la
# fabricacion no cayera ahi, el modelo se fabricaria y no arrancaria.
check "6a. el destino es GGUF_DIR/<modelo>/<gguf>" \
      "$(cond 'printf "%s\n" "$catout" | grep -q "^build	fabrica-IQ4_XS.gguf"')" "$catout"

# ══ 7. los TOML de VERDAD del repo tienen [build] bien formado ════════════
# Los cinco modelos que se fabrican: nemotron, granite, qwen3, qwencoder, qwen
for m in nemotron granite qwen3 qwencoder qwen; do
  a="$(python3 "$MC" build --root "$REPO/models" --name "$m" 2>/dev/null)"
  check "7. $m -> el gguffer recibe un argv" "$(cond '[ -n "$a" ]')" "$a"
  check "7. $m -> tiene --quant"           "$(cond 'printf "%s\n" "$a" | grep -qx -- "--quant"')" "$a"
  check "7. $m -> NO tiene --size (no es flag del gguffer)" \
        "$(cond '! printf "%s\n" "$a" | grep -qx -- "--size"')" "$a"
done

# El nemotron NO puede fabricar desde el repo NVFP4 (ya viene cuantizado):
# es el fallo quedg- dio, y la receta tiene que apuntar al BF16.
a="$(python3 "$MC" build --root "$REPO/models" --name nemotron 2>/dev/null)"
check "8a. el nemotron fabrica desde el BF16" \
      "$(cond 'printf "%s\n" "$a" | grep -qx "nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16"')" "$a"
check "8b. el nemotron NO fabrica desde el NVFP4" \
      "$(cond '! printf "%s\n" "$a" | grep -q "NVFP4"')" "$a"
check "8c. el nemotron usa el imatrix de bartowski" \
      "$(cond 'printf "%s\n" "$a" | grep -qx "bartowski/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-GGUF"')" "$a"

# El qwen (Qwen3.8) se fabrica con --no-mtp, y por eso NO puede ofrecer el
# flag "mtp": el server aborta si se le pide un contexto MTP y el modelo no
# tiene la capa ("context type MTP requested but model doesn't contain MTP
# layers").
a="$(python3 "$MC" build --root "$REPO/models" --name qwen 2>/dev/null)"
check "9a. el qwen se fabrica con --no-mtp" \
      "$(cond 'printf "%s\n" "$a" | grep -qx -- "--no-mtp"')" "$a"
check "9b. el qwen NO tiene el flag mtp" \
      "$(cond '! grep -q "^mtp$" <<<"$(python3 "$MC" dump --root "$REPO/models" | cut -f18 | grep -v "^$")"')" \
      "el qwen ofrece el flag mtp con un GGUF sin MTP"

# ══ resumen ════════════════════════════════════════════════════════════════
printf '\ntest-build.sh: %d correctos, %d fallos\n' "$PASS" "$FAIL"
if (( FAIL > 0 )); then
  printf '  fallaron:\n'
  for c in "${FAILED_CASES[@]}"; do printf '    - %s\n' "$c"; done
  exit 1
fi
exit 0
