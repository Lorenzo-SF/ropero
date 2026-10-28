#!/usr/bin/env bash
# Tests del comando 'ropero gguffer'.
#
# Lo que se testea NO es que quantifique bien (eso necesita una GPU y 60 GB),
# sino la SEMANTICA DE ESTADOS: que se borra cuando toca y que se conserva
# cuando algo falla, que es donde se perdian los 60 GB de los safetensors.
#
#   1. un quant invalido falla ANTES de descargar
#   2. si la descarga falla, se borra el clon a medias y la cache de HF
#   3. si la conversion falla, se conservan los safetensors
#   4. si la cuantizacion falla, se conservan los safetensors Y el F16
#   5. si el GGUF final no valida, no se borra nada
#   6. si todo va bien, el GGUF existe y los safetensors se borran
#   7. con --path, el directorio del usuario NO se toca
#   8. --force limpia lo cacheado y rehace

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

PYTHON_BIN=""
for _cand in python3 python /usr/bin/python3 /usr/local/bin/python3 /bin/python3; do
    if "$_cand" -c 'import sys' >/dev/null 2>&1; then
        PYTHON_BIN="$("$_cand" -c 'import sys; print(sys.executable)')"
        break
    fi
done
if [[ -z "$PYTHON_BIN" ]]; then
    printf 'FATAL: no hay python3 utilizable\n' >&2
    exit 1
fi

VERBOSE=0
[[ "${1:-}" == "-v" ]] && VERBOSE=1
PASS=0
FAIL=0
FAILED_CASES=()

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ropero-gguffer.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

HOME_DIR="$WORK/home"
GGUF_OUT="$WORK/models/gguf"
LLAMA_DIR_TEST="$WORK/llama.cpp"
WORK_ROOT="$WORK/work"
BIN_DIR="$WORK/bin"
mkdir -p "$HOME_DIR" "$GGUF_OUT" "$LLAMA_DIR_TEST/build/bin" "$WORK_ROOT" "$BIN_DIR"

# ── los dobles (conversor, cuantizador, hf, maker): compartidos ──
source "$HERE/_fakes.sh"

# ── correr el gguffer ─────────────────────────────────────────────────────
gguffer() {
    env -i \
        HOME="$HOME_DIR" \
        PATH="$BIN_DIR:$PATH" \
        LLAMA_DIR="$LLAMA_DIR_TEST" \
        GGUF_DIR="$GGUF_OUT" \
        GGUF_WORK_DIR="$WORK_ROOT" \
        HF_HOME="$WORK/hfcache" \
        FAKE_HF_FAIL="${FAKE_HF_FAIL:-}" \
        FAKE_HF_GGUF="${FAKE_HF_GGUF:-}" \
        FAKE_HF_GGUF_MAKER="${FAKE_HF_GGUF_MAKER:-$WORK/gguf_maker.py}" \
        FAKE_QUANTIZE_FAIL="${FAKE_QUANTIZE_FAIL:-}" \
        FAKE_QUANTIZE_GARBAGE="${FAKE_QUANTIZE_GARBAGE:-}" \
        FAKE_QUANTIZE_LOG="${FAKE_QUANTIZE_LOG:-}" \
        FAKE_QUANTIZE_MSG="${FAKE_QUANTIZE_MSG:-}" \
        FAKE_CONVERT_COUNT="${FAKE_CONVERT_COUNT:-}" \
        FAKE_QUANT_CONFIG="${FAKE_QUANT_CONFIG:-}" \
        bash "$REPO/bin/ropero" gguffer "$@"
}

ok()   { PASS=$((PASS+1)); [[ $VERBOSE -eq 1 ]] && printf '  ok   %s\n' "$1"; return 0; }
bad()  { FAIL=$((FAIL+1)); FAILED_CASES+=("$1"); printf '  FALLO %s\n' "$1"; [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; return 0; }
check() { # check <descripcion> <condicion-verdadera> [mensaje]
    if [[ "$2" == "1" ]]; then ok "$1"; else bad "$1" "${3:-}"; fi
}
cond() { if eval "$1" >/dev/null 2>&1; then echo 1; else echo 0; fi; }

REPO_DIR="$WORK_ROOT/owner__repo"
HF_CACHE_DIR="$WORK/hfcache/hub/models--owner--repo"

# ══ 1. quant invalido: falla antes de descargar ═══════════════════════════
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/repo --quant Q9_INVENTADO 2>&1)"; rc=$?
check "1a. quant invalido -> error"            "$(cond '[ $rc -ne 0 ]')" "rc=$rc"
check "1b. quant invalido -> no descarga nada"  "$(cond '[ ! -d "$REPO_DIR" ]')"
check "1c. quant invalido -> lo dice"           "$(cond 'printf %s "$out" | grep -q "quant no valido"')" "$out"

# ══ 2. la descarga falla: se limpia el clon y la cache ════════════════════
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
out="$(FAKE_HF_FAIL=1 gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "2a. descarga fallida -> error"           "$(cond '[ $rc -ne 0 ]')" "rc=$rc"
check "2b. descarga fallida -> no queda clon"   "$(cond '[ ! -d "$REPO_DIR" ]')"
check "2c. descarga fallida -> no queda cache"  "$(cond '[ ! -d "$HF_CACHE_DIR" ]')"
check "2d. descarga fallida -> lo dice"         "$(cond 'printf %s "$out" | grep -q "he limpiado"')" "$out"

# ══ 3. la conversion falla: se conservan los safetensors ══════════════════
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import sys
sys.stderr.write("fake: fallo la conversion\n")
sys.exit(1)
PY
out="$(gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "3a. conversion fallida -> error"          "$(cond '[ $rc -ne 0 ]')" "rc=$rc"
check "3b. conversion fallida -> borra los pesos" "$(cond '[ ! -d "$REPO_DIR" ]')" "sigue $REPO_DIR"
check "3c. conversion fallida -> dice que los borro" \
      "$(cond 'printf %s "$out" | grep -q "borro los pesos"')" "$out"
check "3d. conversion fallida -> no deja GGUF a medias" \
      "$(cond '[ -z "$(ls -A "$GGUF_OUT" 2>/dev/null)" ]')" "$(ls -A "$GGUF_OUT" 2>/dev/null)"

# ══ 4. la cuantizacion falla: se conservan pesos Y el F16 ══════════════════
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import struct
import sys

args = sys.argv[1:]
outfile = args[args.index("--outfile") + 1]

def s(x):
    b = x.encode()
    return struct.pack("<Q", len(b)) + b

body = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 1) + struct.pack("<Q", 2)
body += s("general.architecture") + struct.pack("<I", 8) + s("qwen35")
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 1)
idx = s("blk.0.weight") + struct.pack("<I", 1) + struct.pack("<Q", 4) + struct.pack("<I", 0) + struct.pack("<Q", 0)
body += idx
body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
with open(outfile, "wb") as f:
    f.write(body)
PY
out="$(FAKE_QUANTIZE_FAIL=1 gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "4a. cuantizacion fallida -> error"       "$(cond '[ $rc -ne 0 ]')" "rc=$rc"
check "4b. cuantizacion fallida -> borra los pesos" "$(cond '[ ! -d "$REPO_DIR" ]')" "sigue $REPO_DIR"
check "4c. cuantizacion fallida -> conserva el F16 (es lo caro de rehacer)" \
      "$(cond 'ls "${GGUF_OUT:?}"/*-F16.gguf >/dev/null 2>&1')" "$(ls -A "$GGUF_OUT")"
check "4d. cuantizacion fallida -> dice que los borro" \
      "$(cond 'printf %s "$out" | grep -q "borro los pesos"')" "$out"

# ══ 5. el GGUF final no valida: no se borra nada ═════════════════════════
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
out="$(FAKE_QUANTIZE_GARBAGE=1 gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "5a. GGUF invalido -> error"              "$(cond '[ $rc -ne 0 ]')" "rc=$rc"
check "5b. GGUF invalido -> conserva los pesos"  "$(cond '[ -d "$REPO_DIR" ]')"
check "5c. GGUF invalido -> lo dice"             "$(cond 'printf %s "$out" | grep -q "verificacion"')" "$out"

# ══ 6. todo bien: el GGUF existe y los safetensors se borran ══════════════
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "6a. todo bien -> sin error"               "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "6b. todo bien -> GGUF final"              "$(cond 'ls "${GGUF_OUT:?}"/*-IQ3_XS.gguf >/dev/null 2>&1')" "$(ls -A "$GGUF_OUT")"
check "6c. todo bien -> no queda el F16"         "$(cond '[ ! -e "$GGUF_OUT/repo-F16.gguf" ]')"
check "6d. todo bien -> safetensors borrados"    "$(cond '[ ! -d "$REPO_DIR" ]')"
check "6e. todo bien -> lo dice"                 "$(cond 'printf %s "$out" | grep -q "GGUF listo"')" "$out"

# ══ 7. con --path, el directorio del usuario no se toca ═══════════════════
rm -rf "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
MIO="$WORK/mis-pesos"; mkdir -p "$MIO"
printf 'pesos mios\n' > "$MIO/model.safetensors"
out="$(gguffer --path "$MIO" --quant IQ3_XS 2>&1)"; rc=$?
check "7a. --path -> sin error"                  "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "7b. --path -> el GGUF existe"             "$(cond 'ls "${GGUF_OUT:?}"/*-IQ3_XS.gguf >/dev/null 2>&1')"
check "7c. --path -> NO borra el dir del usuario" "$(cond '[ -f "$MIO/model.safetensors" ]')" "ha borrado el dir del usuario"

# ══ 8. --force limpia lo cacheado y rehace ═══════════════════════════════
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
gguffer --repo owner/repo --quant IQ3_XS >/dev/null 2>&1
mkdir -p "$REPO_DIR" "$HF_CACHE_DIR" "$GGUF_OUT"
printf 'basura\n' > "$REPO_DIR/viejo.txt"
out="$(gguffer --repo owner/repo --quant IQ3_XS --force 2>&1)"; rc=$?
check "8a. --force -> sin error"                 "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "8b. --force -> el clon viejo no esta"     "$(cond '[ ! -f "$REPO_DIR/viejo.txt" ]')"
check "8c. --force -> rehizo el GGUF"            "$(cond 'ls "${GGUF_OUT:?}"/*-IQ3_XS.gguf >/dev/null 2>&1')"

# ══ 9. --intermedio bf16: el conversor recibe el outtype pedido ══════════
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import struct
import sys

args = sys.argv[1:]
outfile = args[args.index("--outfile") + 1]
outtype = args[args.index("--outtype") + 1]
with open(outfile + ".outtype", "w") as f:
    f.write(outtype)

def s(x):
    b = x.encode()
    return struct.pack("<Q", len(b)) + b

body = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 1) + struct.pack("<Q", 2)
body += s("general.architecture") + struct.pack("<I", 8) + s("qwen35")
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 1)
idx = s("blk.0.weight") + struct.pack("<I", 1) + struct.pack("<Q", 4) + struct.pack("<I", 0) + struct.pack("<Q", 0)
body += idx
body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
with open(outfile, "wb") as f:
    f.write(body)
PY
out="$(gguffer --repo owner/repo --intermedio bf16 2>&1)"; rc=$?
check "9a. --intermedio bf16 -> sin error"        "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "9b. --intermedio bf16 -> el conversor recibe bf16" \
      "$(cond '[ "$(cat "${GGUF_OUT:?}"/*.outtype 2>/dev/null)" = "bf16" ]')" \
      "$(cat "${GGUF_OUT:?}"/*.outtype 2>/dev/null)"
check "9c. --intermedio bf16 -> el fichero se llama BF16" \
      "$(cond 'ls "${GGUF_OUT:?}"/*-BF16.gguf >/dev/null 2>&1')" "$(ls -A "$GGUF_OUT")"

# ══ 10. --no-mtp: se pasa al conversor solo si lo soporta ═══════════════════
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import struct
import sys

# este conversor de mentira sabe excluir la MTP: acepta --no-mtp
args = sys.argv[1:]
outfile = args[args.index("--outfile") + 1]
with open(outfile + ".args", "w") as f:
    f.write(" ".join(args))

def s(x):
    b = x.encode()
    return struct.pack("<Q", len(b)) + b

body = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 1) + struct.pack("<Q", 2)
body += s("general.architecture") + struct.pack("<I", 8) + s("qwen35")
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 1)
idx = s("blk.0.weight") + struct.pack("<I", 1) + struct.pack("<Q", 4) + struct.pack("<I", 0) + struct.pack("<Q", 0)
body += idx
body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
with open(outfile, "wb") as f:
    f.write(body)
PY
out="$(gguffer --repo owner/repo --no-mtp 2>&1)"; rc=$?
check "10a. --no-mtp -> sin error"                "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "10b. --no-mtp -> el conversor lo recibe"   \
      "$(cond 'grep -q -- "--no-mtp" "$GGUF_OUT"/*.args 2>/dev/null')" \
      "$(cat "$GGUF_OUT"/*.args 2>/dev/null)"

# ══ 11. sin --quant: avisa de que NO ha cuantizado ══════════════════════════
rm -rf "$REPO_DIR" "$GGUF_OUT"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/repo 2>&1)"; rc=$?
check "11a. sin --quant -> sin error"             "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "11b. sin --quant -> lo deja en F16"        "$(cond 'ls "$GGUF_OUT"/*-F16.gguf >/dev/null 2>&1')" "$(ls -A "$GGUF_OUT")"
check "11c. sin --quant -> AVISA de que no ha cuantizado" \
      "$(cond 'printf %s "$out" | grep -q "NO he cuantizado"')" "$out"
check "11d. sin --quant -> dice como cuantizarlo" \
      "$(cond 'printf %s "$out" | grep -q "ropero gguffer --repo"')" "$out"

# ══ 12. reutiliza un intermedio que ya haya (F16 o BF16) ═══════════════════
rm -rf "$REPO_DIR" "$GGUF_OUT"/* "$WORK/hfcache"; mkdir -p "$GGUF_OUT"
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import struct
import sys

args = sys.argv[1:]
outfile = args[args.index("--outfile") + 1]
with open(outfile + ".outtype", "w") as f:
    f.write(args[args.index("--outtype") + 1])

def s(x):
    b = x.encode()
    return struct.pack("<Q", len(b)) + b

body = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 1) + struct.pack("<Q", 2)
body += s("general.architecture") + struct.pack("<I", 8) + s("qwen35")
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 1)
idx = s("blk.0.weight") + struct.pack("<I", 1) + struct.pack("<Q", 4) + struct.pack("<I", 0) + struct.pack("<Q", 0)
body += idx
body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
with open(outfile, "wb") as f:
    f.write(body)
PY
# un BF16 de una corrida anterior: se debe reutilizar, no convertir
printf 'gguf viejo' > "$GGUF_OUT/repo-BF16.gguf"
out="$(gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "12a. reutiliza el BF16 que habia" "$(cond 'printf %s "$out" | grep -q "reutilizo el GGUF intermedio"')" "$out"
check "12b. no vuelve a convertir"          "$(cond '[ ! -f "$GGUF_OUT"/*.outtype ]')" "$(ls -A "$GGUF_OUT")"

# ── 13: el --imatrix va ANTES de los posicionales ─────────────────────────
# El cuantizador real solo parsea las opciones mientras empiecen por "--".
# Si la flag llega detras, se come el parseo del nthread y muere con
# "invalid nthread '--imatrix'". Este test lo fija.
printf 'imatrix falso\n' > "$WORK/imatrix_falso.gguf"
out="$(FAKE_QUANTIZE_LOG="$WORK/imatrix_log.txt" gguffer --repo owner/imatrix-repo --quant IQ3_XS \
        --imatrix "$WORK/imatrix_falso.gguf" 2>&1)"; rc=$?
check "13a. llega a la fase de cuantizacion" "$(cond 'printf %s "$out" | grep -q "cuantizo a IQ3_XS"')" "$out"
check "13b. el cuantizador recibe el imatrix" "$(cond 'grep -q "imatrix_falso.gguf" "$WORK/imatrix_log.txt" 2>/dev/null')" \
      "$(cat "$WORK/imatrix_log.txt" 2>/dev/null)"
check "13c. no dice invalid nthread"        "$(cond '! printf %s "$out" | grep -q "invalid nthread"')" "$out"

# ── 14: --out admite nombre, directorio o ruta exacta ──────────────────────
# El launcher busca cada modelo en $GGUF_DIR/<carpeta_del_modelo>/<fichero>.
# Con --out se puede poner el GGUF fabricado justo ahi, con el nombre que
# espera el TOML.
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/out-nombre --quant IQ3_XS --out "mi-modelo" 2>&1)"
check "14a. --out nombre: en el GGUF_DIR por defecto" \
      "$(cond '[ -f "$GGUF_OUT/mi-modelo-IQ3_XS.gguf" ]')" "$out"

rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/out-dir --quant IQ3_XS --out "$GGUF_OUT/agenta1" 2>&1)"
check "14b. --out directorio: crea la carpeta del modelo" \
      "$(cond '[ -f "$GGUF_OUT/agenta1/out-dir-IQ3_XS.gguf" ]')" "$out"

rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/out-fichero --quant IQ3_XS \
        --out "$GGUF_OUT/agenta1/Agents-A1-IQ2_M.gguf" 2>&1)"
check "14c. --out ruta exacta: el nombre del TOML" \
      "$(cond '[ -f "$GGUF_OUT/agenta1/Agents-A1-IQ2_M.gguf" ]')" "$out"
check "14d. --out ruta exacta: no inventa otro nombre" \
      "$(cond '[ ! -f "$GGUF_OUT/agenta1/out-fichero-IQ3_XS.gguf" ]')" "$out"

rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/out-sinquant --out "$GGUF_OUT/gemma/google_gemma-4-26B-A4B-it-IQ3_M.gguf" 2>&1)"
check "14e. --out ruta exacta sin quant: el fichero sale con ese nombre" \
      "$(cond '[ -f "$GGUF_OUT/gemma/google_gemma-4-26B-A4B-it-IQ3_M.gguf" ]')" "$out"

# ── 15: el gguffer baja el imatrix solo ───────────────────────────────────
# Antes habia que bajarlo a mano con hf y pasarle la ruta. Con
# --imatrix-repo/--imatrix-file se encarga el gguffer, y lo cachea.
rm -rf "$GGUF_OUT"/* "$WORK_ROOT"/*; mkdir -p "$GGUF_OUT"
out="$(FAKE_QUANTIZE_LOG="$WORK/imatrix_log.txt" gguffer --repo owner/imatrix-descarga \
        --quant IQ3_XS --imatrix-repo owner/imatrix-repo \
        --imatrix-file "imatrix/mi.imatrix.gguf" 2>&1)"; rc=$?
check "15a. baja el imatrix"        "$(cond 'printf %s "$out" | grep -q "bajo el imatrix de owner/imatrix-repo"')" "$out"
check "15b. lo pasa al cuantizador" "$(cond 'grep -q "mi.imatrix.gguf" "$WORK/imatrix_log.txt" 2>/dev/null')" \
      "$(cat "$WORK/imatrix_log.txt" 2>/dev/null)"
check "15c. lo deja en el subdirectorio" \
      "$(cond '[ -f "$WORK_ROOT/imatrix/owner__imatrix-repo/imatrix/mi.imatrix.gguf" ]')" "$out"

# la segunda vez lo reutiliza, no lo vuelve a bajar
out="$(FAKE_QUANTIZE_LOG="$WORK/imatrix_log2.txt" gguffer --repo owner/imatrix-descarga2 \
        --quant IQ3_XS --imatrix-repo owner/imatrix-repo \
        --imatrix-file "imatrix/mi.imatrix.gguf" 2>&1)"
check "15d. la segunda vez lo reutiliza" \
      "$(cond 'printf %s "$out" | grep -q "reutilizo el imatrix"')" "$out"

# si solo va una de las dos, error claro
out="$(gguffer --repo owner/imatrix-medio --quant IQ3_XS --imatrix-repo owner/imatrix-repo 2>&1)"; rc=$?
check "15e. --imatrix-repo sin --imatrix-file: error" \
      "$(cond '[ "$rc" != "0" ] && printf %s "$out" | grep -q "van juntos"')" "$out"

# ── 16: la cabeza MTP y el imatrix ─────────────────────────────────────────
# El imatrix se calcula sobre el modelo base (64 capas), pero el GGUF lleva
# la MTP como un bloque MAS. Ese bloque no esta en el imatrix, y con un quant
# very-low-bit el cuantizador aborta con "Missing importance matrix for
# tensor blk.64.attn_k.weight". La salida: cuantizar ese bloque en un quant
# que NO exija imatrix, que es lo que hizo bartowski con su IQ2_M.
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
mkdir -p "$WORK/mtp-src"; echo '{}' > "$WORK/mtp-src/config.json"
python3 "$WORK/gguf_maker.py" gguf "$GGUF_OUT/mtp-src-F16.gguf" 3
python3 "$WORK/gguf_maker.py" imatrix "$WORK/mtp.imatrix" 3
# el imatrix cubre 0..2, el GGUF tiene ademas el bloque 3 (la MTP)
out="$(FAKE_QUANTIZE_LOG="$WORK/mtp_log.txt" gguffer --path "$WORK/mtp-src" --quant IQ2_M \
        --imatrix "$WORK/mtp.imatrix" 2>&1)"
check "16a. ve que el imatrix no cubre el bloque de la MTP" \
      "$(cond 'printf %s "$out" | grep -q "el imatrix no cubre el bloque 3"')" "$out"
check "16b. lo cuantiza en Q4_0, que no exige imatrix" \
      "$(cond 'grep -q "tensor-type: blk.*3.*Q4_0" "$WORK/mtp_log.txt" 2>/dev/null')" \
      "$(cat "$WORK/mtp_log.txt" 2>/dev/null)"

# el log del cuantizador se guarda: si falla, el gguffer dice que tensor falta
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
mkdir -p "$WORK/mtp-src2"; echo '{}' > "$WORK/mtp-src2/config.json"
python3 "$WORK/gguf_maker.py" gguf "$GGUF_OUT/mtp-src2-F16.gguf" 3
out="$(FAKE_QUANTIZE_FAIL=1 FAKE_QUANTIZE_MSG='Missing importance matrix for tensor blk.3.attn_k.weight' \
       gguffer --path "$WORK/mtp-src2" --quant IQ2_M --imatrix "$WORK/mtp.imatrix" 2>&1)"
check "16c. si falla, dice que tensor no cubre el imatrix" \
      "$(cond 'printf %s "$out" | grep -q "el imatrix no cubre el tensor: blk.3.attn_k.weight"')" "$out"

# ── 16b: la MTP se localiza por la METADATA, no por el nombre del tensor ───
# El bug: la deteccion buscaba tensores con "nextn." en el nombre. En los
# GGUFs de verdad los tensores de la MTP se llaman como los de cualquier
# otro bloque ("blk.64.attn_k.weight"), SIN "nextn". Asi que la deteccion
# no encontraba nada, el fix no se aplicaba, y la cuantizacion moria con
# "Missing importance matrix for tensor blk.64.attn_k.weight".
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
mkdir -p "$WORK/mtp-src3"; echo '{}' > "$WORK/mtp-src3/config.json"
python3 "$WORK/gguf_maker.py" gguf "$GGUF_OUT/mtp-src3-F16.gguf" 3
out="$(FAKE_QUANTIZE_LOG="$WORK/mtp_log3.txt" gguffer --path "$WORK/mtp-src3" --quant IQ2_M \
        --imatrix "$WORK/mtp.imatrix" 2>&1)"
check "16b. encuentra la MTP por la metadata (block_count - nextn)" \
      "$(cond 'printf %s "$out" | grep -q "el imatrix no cubre el bloque 3"')" "$out"
check "16c. los tensores de la MTP NO llevan 'nextn' en el nombre" \
      "$(cond '! python3 "'"$REPO"'/bin/_ggufcheck.py" "$GGUF_OUT/mtp-src3-F16.gguf" --nombres 2>/dev/null | grep -q nextn')" \
      "un GGUF real no tiene tensores con 'nextn' en el nombre"

# un GGUF SIN MTP (nextn=0): no hay bloque MTP, y no se toca nada
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
mkdir -p "$WORK/sin-mtp"; echo '{}' > "$WORK/sin-mtp/config.json"
python3 "$WORK/gguf_maker.py" gguf-sin-mtp "$GGUF_OUT/sin-mtp-F16.gguf" 3
python3 "$WORK/gguf_maker.py" imatrix "$WORK/sin-mtp.imatrix" 3
out="$(FAKE_QUANTIZE_LOG="$WORK/sinmtp_log.txt" gguffer --path "$WORK/sin-mtp" --quant IQ2_M \
        --imatrix "$WORK/sin-mtp.imatrix" 2>&1)"
check "16d. sin MTP: no inventa un bloque" \
      "$(cond '! printf %s "$out" | grep -q "el imatrix no cubre el bloque"')" "$out"
check "16e. sin MTP: no mete un --tensor-type" \
      "$(cond '! grep -q "tensor-type" "$WORK/sinmtp_log.txt" 2>/dev/null')" \
      "$(cat "$WORK/sinmtp_log.txt" 2>/dev/null)"

# ── 17: el tokenizer de transformers v5 en un clon de transformers v4 ──────
# Hay repos guardados con transformers v5, cuyo tokenizer_class es
# "TokenizersBackend". Con transformers v4 la conversion muere con
# "Tokenizer class TokenizersBackend does not exist".
mkdir -p "$WORK/tok-src"
printf '{"tokenizer_class":"TokenizersBackend","model_max_length":262144}\n' \
    > "$WORK/tok-src/tokenizer_config.json"
printf '{"model_max_length":262144}\n' > "$WORK/tok-src/tokenizer.json"
# un conversor propio: el de transformers v4 no conoce "TokenizersBackend"
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import json, os, struct, sys
args = sys.argv[1:]
outfile = args[args.index("--outfile") + 1]
src = args[0]
cfg = os.path.join(src, "tokenizer_config.json")
if os.path.exists(cfg):
    try:
        with open(cfg, encoding="utf-8") as f:
            c = json.load(f)
    except Exception:
        c = {}
    if c.get("tokenizer_class") == "TokenizersBackend":
        sys.stderr.write("ValueError: Tokenizer class TokenizersBackend does not exist "
                         "or is not currently imported.\n")
        sys.exit(1)
def s(x):
    b = x.encode(); return struct.pack("<Q", len(b)) + b
body = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 1) + struct.pack("<Q", 2)
body += s("general.architecture") + struct.pack("<I", 8) + s("qwen35")
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 1)
idx = s("blk.0.weight") + struct.pack("<I", 1) + struct.pack("<Q", 4) + struct.pack("<I", 1) + struct.pack("<Q", 0)
body += idx
body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
with open(outfile, "wb") as f:
    f.write(body)
PY
out="$(gguffer --path "$WORK/tok-src" --quant IQ3_XS 2>&1)"
check "17a. cambia la clase del tokenizer" \
      "$(cond 'grep -q "PreTrainedTokenizerFast" "$WORK/tok-src/tokenizer_config.json"')" \
      "$(cat "$WORK/tok-src/tokenizer_config.json" 2>/dev/null)"
check "17b. guarda una copia del original" \
      "$(cond '[ -f "$WORK/tok-src/tokenizer_config.json.bak" ]')" "$out"
# El tokenizer se arregla ANTES de convertir, asi que la conversion ya va a
# la primera: no hay fallo ni reintento. El aviso sale igualmente.
check "17c. avisa de lo que ha hecho" \
      "$(cond 'printf %s "$out" | grep -q "Lo cambio a PreTrainedTokenizerFast"')" "$out"
check "17d. la conversion va a la primera (sin reintento)" \
      "$(cond '! printf %s "$out" | grep -q "does not exist"')" "$out"

# ── 18: el conversor que no admite --no-mtp para esta arch ─────────────────
# El nemotron: el conversor acepta el flag, pero no para NemotronHForCausalLM.
# Y no hace falta: sin el mixin de MTP, el conversor ya deja fuera la nextn.
mkdir -p "$WORK/nem-src"; echo '{}' > "$WORK/nem-src/config.json"
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import struct, sys
args = sys.argv[1:]
outfile = args[args.index("--outfile") + 1]
if "--no-mtp" in args:
    sys.stderr.write("ERROR: --mtp / --no-nextn are not supported for NemotronHForCausalLM\n")
    sys.exit(1)
def s(x):
    b = x.encode(); return struct.pack("<Q", len(b)) + b
body = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 1) + struct.pack("<Q", 2)
body += s("general.architecture") + struct.pack("<I", 8) + s("nemotron_h")
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 1)
idx = s("blk.0.weight") + struct.pack("<I", 1) + struct.pack("<Q", 4) + struct.pack("<I", 1) + struct.pack("<Q", 0)
body += idx
body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
with open(outfile, "wb") as f:
    f.write(body)
PY
out="$(gguffer --path "$WORK/nem-src" --quant IQ2_M --no-mtp 2>&1)"
check "18a. avisa de que la arch no admite --no-mtp" \
      "$(cond 'printf %s "$out" | grep -q "esta arquitectura no admite --no-mtp"')" "$out"
check "18b. reintenta sin el y convierte" \
      "$(cond 'printf %s "$out" | grep -q "GGUF listo"')" "$out"

# ── 19: VARIOS quants en una tirada ───────────────────────────────────────
# Bajar y convertir los pesos es lo caro (40-60 GB y una hora). Cuantizar es
# solo CPU. Por eso se pueden pedir varios quants de golpe: el GGUF
# intermedio se convierte UNA vez y se reutiliza para todos.
# El conversor fake cuenta las conversiones en un fichero.
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import os, struct, sys
args = sys.argv[1:]
outfile = args[args.index("--outfile") + 1]
contador = os.environ.get("FAKE_CONVERT_COUNT")
if contador:
    with open(contador, "a") as f:
        f.write("x\n")
def s(x):
    b = x.encode(); return struct.pack("<Q", len(b)) + b
body = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 1) + struct.pack("<Q", 2)
body += s("general.architecture") + struct.pack("<I", 8) + s("qwen35")
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 1)
idx = s("blk.0.weight") + struct.pack("<I", 1) + struct.pack("<Q", 4) + struct.pack("<I", 1) + struct.pack("<Q", 0)
body += idx
body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
with open(outfile, "wb") as f:
    f.write(body)
PY
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
rm -f "$WORK/convert_count.txt"
out="$(FAKE_CONVERT_COUNT="$WORK/convert_count.txt" gguffer --repo owner/multi \
        --quant IQ2_XXS --quant IQ3_XS 2>&1)"; rc=$?
check "19a. dos quants: sin error"           "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "19b. hace los dos ficheros" \
      "$(cond '[ -f "$GGUF_OUT/multi-IQ2_XXS.gguf" ] && [ -f "$GGUF_OUT/multi-IQ3_XS.gguf" ]')" \
      "$(ls -A "$GGUF_OUT")"
check "19c. convierte UNA sola vez" \
      "$(cond '[ "$(wc -l < "$WORK/convert_count.txt" 2>/dev/null || echo 0)" = "1" ]')" \
      "conversiones: $(cat "$WORK/convert_count.txt" 2>/dev/null | wc -l)"
check "19d. cuantiza a los dos" \
      "$(cond 'printf %s "$out" | grep -q "cuantizo a IQ2_XXS" && printf %s "$out" | grep -q "cuantizo a IQ3_XS"')" "$out"
check "19e. avisa de los dos GGUFs" \
      "$(cond 'printf %s "$out" | grep -q "GGUF listo" && [ "$(printf %s "$out" | grep -c "GGUF listo")" = "2" ]')" "$out"

# con comas es lo mismo
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
rm -f "$WORK/convert_count.txt"
out="$(FAKE_CONVERT_COUNT="$WORK/convert_count.txt" gguffer --repo owner/multi2 \
        --quant IQ2_XXS,IQ3_XS 2>&1)"
check "19f. con comas: hace los dos" \
      "$(cond '[ -f "$GGUF_OUT/multi2-IQ2_XXS.gguf" ] && [ -f "$GGUF_OUT/multi2-IQ3_XS.gguf" ]')" \
      "$(ls -A "$GGUF_OUT")"

# un quant invalido entre varios: error claro, y no se descarga nada
out="$(gguffer --repo owner/multi3 --quant IQ2_XXS --quant QUANT_INVENTADO 2>&1)"; rc=$?
check "19g. quant invalido: error" \
      "$(cond '[ "$rc" != "0" ] && printf %s "$out" | grep -q "quant no valido: QUANT_INVENTADO"')" "$out"

# con varios quants, la ruta exacta no vale (cada uno necesita la suya)
out="$(gguffer --repo owner/multi4 --quant IQ2_XXS --quant IQ3_XS \
        --out "$GGUF_OUT/exacto.gguf" 2>&1)"; rc=$?
check "19h. varios quants + ruta exacta: error claro" \
      "$(cond '[ "$rc" != "0" ] && printf %s "$out" | grep -q "ruta exacta"')" "$out"

# el intermedio se borra al FINAL: no antes, que el segundo quant lo necesita
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/multi5 --quant IQ2_XXS --quant IQ3_XS 2>&1)"
check "19i. el intermedio se borra al final" \
      "$(cond '[ ! -f "$GGUF_OUT/multi5-F16.gguf" ]')" "$(ls -A "$GGUF_OUT")"

# si uno de los quants YA existe, se respeta y se hace el otro
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
printf 'gguf viejo\n' > "$GGUF_OUT/multi6-IQ2_XXS.gguf"
out="$(gguffer --repo owner/multi6 --quant IQ2_XXS --quant IQ3_XS 2>&1)"
check "19j. el que ya existe: no lo rehace" \
      "$(cond 'printf %s "$out" | grep -q "ya existe"')" "$out"
check "19k. el que faltaba: lo hace" \
      "$(cond '[ -f "$GGUF_OUT/multi6-IQ3_XS.gguf" ]')" "$(ls -A "$GGUF_OUT")"

# si un quant falla a mitad, el otro no se hace y se conserva el intermedio
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
out="$(FAKE_QUANTIZE_FAIL=1 gguffer --repo owner/multi7 --quant IQ2_XXS --quant IQ3_XS 2>&1)"; rc=$?
check "19l. si un quant falla: error" "$(cond '[ "$rc" != "0" ]')" "rc=$rc: $out"
check "19m. si un quant falla: conserva el intermedio" \
      "$(cond '[ -f "$GGUF_OUT/multi7-F16.gguf" ]')" "$(ls -A "$GGUF_OUT")"

# ── 20: --out-principal: el primero va a la ruta exacta, los demas al dir ──
# Con varios quants, uno va a la carpeta del modelo (la que espera el TOML)
# y los demas se quedan ahi con su nombre propio, para comparar o subir.
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/principal --quant IQ2_XXS --quant IQ3_XS \
        --out-principal "$GGUF_OUT/mi-modelo/Modelos-IQ2_XXS.gguf" 2>&1)"; rc=$?
check "20a. el primero va a la ruta exacta" \
      "$(cond '[ -f "$GGUF_OUT/mi-modelo/Modelos-IQ2_XXS.gguf" ]')" "$(ls -A "$GGUF_OUT")"
check "20b. el segundo va con su nombre, en la carpeta del principal" \
      "$(cond '[ -f "$GGUF_OUT/mi-modelo/principal-IQ3_XS.gguf" ]')" "$(ls -A "$GGUF_OUT")"
check "20c. sin error" "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"

# --out-principal sin .gguf: error
out="$(gguffer --repo owner/principal2 --quant IQ2_XXS \
        --out-principal "$WORK/no-existe/mi-modelo" 2>&1)"; rc=$?
check "20d. --out-principal sin .gguf y sin directorio: error" \
      "$(cond '[ "$rc" != "0" ] && printf %s "$out" | grep -q "fichero .gguf"')" "$out"

# con --out-principal, el GGUF intermedio se queda en la carpeta del modelo
# (y no en la raiz del GGUF_DIR, que es donde lo dejaria sin --out)
rm -rf "$GGUF_OUT"/*; mkdir -p "$GGUF_OUT"
printf 'imatrix falso\n' > "$WORK/imatrix_falso.gguf"
out="$(FAKE_QUANTIZE_LOG="$WORK/principal_log.txt" gguffer --repo owner/principal3 \
        --quant IQ2_XXS --imatrix "$WORK/imatrix_falso.gguf" \
        --out-principal "$GGUF_OUT/mi-modelo/Modelos-IQ2_XXS.gguf" 2>&1)"
check "20e. el intermedio va a la carpeta del modelo" \
      "$(cond 'printf %s "$out" | grep -q "mi-modelo/principal3-F16.gguf"')" "$out"

# ── 21: "ya existe" no descarga 60 GB ──────────────────────────────────────
# Si el GGUF final ya esta de una corrida anterior, el gguffer tiene que
# cortarse ANTES de descargar los pesos. Es lo que evita rehacer 40 GB de
# descarga y una hora de conversion para nada.
rm -rf "$GGUF_OUT"/* "$REPO_DIR"; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/ya-existe --quant IQ3_XS 2>&1)"
check "21a. la primera vez lo hace" \
      "$(cond '[ -f "$GGUF_OUT/ya-existe-IQ3_XS.gguf" ]')" "$out"
rm -rf "$REPO_DIR"; rm -f "$WORK/convert_count.txt"
out="$(FAKE_CONVERT_COUNT="$WORK/convert_count.txt" gguffer --repo owner/ya-existe --quant IQ3_XS 2>&1)"; rc=$?
check "21b. la segunda vez dice que ya existe" \
      "$(cond 'printf %s "$out" | grep -q "ya existe"')" "$out"
check "21c. la segunda vez NO convierte" \
      "$(cond '[ ! -f "$WORK/convert_count.txt" ]')" \
      "conversiones: $(cat "$WORK/convert_count.txt" 2>/dev/null | wc -l)"
check "21d. la segunda vez NO borra el GGUF" \
      "$(cond '[ -f "$GGUF_OUT/ya-existe-IQ3_XS.gguf" ]')" "$(ls -A "$GGUF_OUT")"
# Un "command not found" en la salida significa un error de shell en el
# gguffer. Con "$force -eq 0" fuera de [[ ]] pasaba: bash intentaba ejecutar
# el comando "0", y el corte temprano NUNCA se cumplia (se re-descargaban
# 60 GB). Este test lo fija.
check "21e. sin errores de shell en la salida" \
      "$(cond '! printf %s "$out" | grep -q "command not found"')" "$out"

# con --force, rehace
out="$(FAKE_CONVERT_COUNT="$WORK/convert_count.txt" gguffer --repo owner/ya-existe --quant IQ3_XS --force 2>&1)"
check "21f. con --force lo rehace" \
      "$(cond '[ -f "$WORK/convert_count.txt" ]')" "$out"

# ══ 22. AL FALLAR, LOS SAFETENSORS SE BORRAN (salvo --keep-source) ═══════
# En un trabajo de noche, dejar 60 GB por cada intento fallido llena el
# disco, y cuando el disco se llena falla tambien lo que ya estaba bien.
REPO_DIR="$WORK_ROOT/owner__repo"
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
out="$(FAKE_QUANTIZE_FAIL=1 gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "22a. si la cuantizacion falla, da error" "$(cond '[ $rc -ne 0 ]')" "rc=$rc"
check "22b. y BORRA los safetensors"       "$(cond '[ ! -d "$REPO_DIR" ]')" "el clon sigue en $REPO_DIR"
check "22c. y lo dice"       "$(cond 'printf %s "$out" | grep -q "borro los pesos"')" "$out"

# con --keep-source, se conservan (para no volver a bajar 60 GB)
REPO_DIR="$WORK_ROOT/owner__repo"
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
out="$(FAKE_QUANTIZE_FAIL=1 gguffer --repo owner/repo --quant IQ3_XS --keep-source 2>&1)"; rc=$?
check "22d. con --keep-source, los safetensors se conservan"       "$(cond '[ -d "$REPO_DIR" ]')" "el clon no esta en $REPO_DIR"
check "22e. y lo dice"       "$(cond 'printf %s "$out" | grep -q "conservo los pesos"')" "$out"

# ══ 23. EL DIAGNOSTICO: por que no se cuantiza ═══════════════════════════
REPO_DIR="$WORK_ROOT/owner__repo"
rm -rf "$REPO_DIR" "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/repo --quant IQ3_XS --diagnostico 2>&1)"; rc=$?
check "23a. el diagnostico sale bien" "$(cond '[ $rc -eq 0 ]')" "$out"
check "23b. dice cuanto espacio hay"       "$(cond 'printf %s "$out" | grep -q "espacio libre"')" "$out"
check "23c. dice cuanto hace falta"       "$(cond 'printf %s "$out" | grep -q "espacio a porner"')" "$out"
check "23d. dice cual es el bloque MTP"       "$(cond 'printf %s "$out" | grep -q "bloque MTP"')" "$out"
check "23e. dice el argv del cuantizador"       "$(cond 'printf %s "$out" | grep -q "llama-quantize"')" "$out"
check "23f. y NO cuantiza"       "$(cond '! printf %s "$out" | grep -q "convierten a IQ3_XS"')" "$out"

# ══ 24. EL PRINCIPAL EN UNA CARPETA (el TOML pide otro quant) ═════════════
# Si el TOML pide "Qwen3.8-27B-IQ2_M" y fabricamos IQ3_XS, el IQ3_XS NO
# puede ir ahi con ese nombre: seria un nombre mentiroso, y el launcher
# creeria que el modelo es el IQ2_M.
mkdir -p "$GGUF_OUT/qwen"
rm -rf "$WORK_ROOT/owner__repo" "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/repo --quant IQ3_XS --out-principal "$GGUF_OUT/qwen/" 2>&1)"; rc=$?
check "24a. con --out-principal de un directorio, sale bien" "$(cond '[ $rc -eq 0 ]')" "$out"
check "24b. el GGUF va a la carpeta con su nombre propio"       "$(cond '[ -f "$GGUF_OUT/qwen/repo-IQ3_XS.gguf" ]')"       "$(ls -la "$GGUF_OUT/qwen/" 2>/dev/null)"

# y con la ruta exacta, sigue yendo ahi
rm -rf "$WORK_ROOT/owner__repo" "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT/qwen"
out="$(gguffer --repo owner/repo --quant IQ3_XS --out-principal "$GGUF_OUT/qwen/Mi-Nombre.gguf" 2>&1)"; rc=$?
check "24c. con la ruta exacta, va exactamente ahi"       "$(cond '[ -f "$GGUF_OUT/qwen/Mi-Nombre.gguf" ]')" \
      "$(ls -la "$GGUF_OUT/qwen/" 2>/dev/null)"

# ══ 25. CUANTIZAR UN GGUF QUE YA EXISTE (sin convertir) ══════════════════
# El conversor de pesos no sabe convertir los modelos multimodales con
# cabeza MTP: sus tensores ("mtp.*") no se pueden mapear y la conversion
# aborta. Cuando ya hay un GGUF sin quantizar publicado (el BF16 de
# bartowski), se cuantiza ese y no se toca el conversor.
mkdir -p "$WORK/gguf-entrada"
python3 "$WORK/gguf_maker.py" gguf "$WORK/gguf-entrada/Mi-Modelo-BF16.gguf" 3
rm -rf "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
out="$(FAKE_CONVERT_COUNT="$WORK/convert_count_25.txt" \
      gguffer --gguf "$WORK/gguf-entrada/Mi-Modelo-BF16.gguf" --quant IQ2_M 2>&1)"; rc=$?
check "25a. cuantiza el GGUF que ya estaba" "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "25b. y sale un GGUF" \
      "$(cond 'ls "${GGUF_OUT:?}"/*-IQ2_M.gguf >/dev/null 2>&1')" "$(ls -A "$GGUF_OUT")"
check "25c. y NO convierte los pesos" \
      "$(cond '[ ! -f "$WORK/convert_count_25.txt" ]')" "convirtio pesos"
check "25d. y dice cual es el GGUF de entrada" \
      "$(cond 'printf %s "$out" | grep -q "GGUF de entrada"')" "$out"

# el diagnostico de esa fuente
rm -rf "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
out="$(gguffer --gguf "$WORK/gguf-entrada/Mi-Modelo-BF16.gguf" --quant IQ2_M --diagnostico 2>&1)"; rc=$?
check "25e. el diagnostico de un GGUF existente sale bien" "$(cond '[ $rc -eq 0 ]')" "$out"
check "25f. y dice que no hay pesos que clonar" \
      "$(cond 'printf %s "$out" | grep -q "no hay pesos que clonar"')" "$out"
check "25g. y NO cuantiza" \
      "$(cond '! printf %s "$out" | grep -q "cuantizo a IQ2_M"')" "$out"

# ══ 26. EL GGUF DE ENTRADA DEL USUARIO NO SE TOCA NUNCA ═══════════════════
# Es un fichero suyo, en su carpeta. Que el gguffer lo borre al terminar
# (o al fallar, o con --force) seria perder el trabajo de otra persona.
mkdir -p "$WORK/entrada-del-usuario"
python3 "$WORK/gguf_maker.py" gguf "$WORK/entrada-del-usuario/Mi.gguf" 3
rm -rf "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
gguffer --gguf "$WORK/entrada-del-usuario/Mi.gguf" --quant IQ2_M >/dev/null 2>&1
check "26a. tras una tirada, el GGUF de entrada sigue ahi" \
      "$(cond '[ -f "$WORK/entrada-del-usuario/Mi.gguf" ]')" "desaparecio"

# y con --force, que limpia "todo lo cacheado"
gguffer --gguf "$WORK/entrada-del-usuario/Mi.gguf" --quant IQ2_M --force >/dev/null 2>&1
check "26b. con --force tampoco lo borra" \
      "$(cond '[ -f "$WORK/entrada-del-usuario/Mi.gguf" ]')" "desaparecio"
check "26c. ni borra la carpeta del usuario" \
      "$(cond '[ -d "$WORK/entrada-del-usuario" ]')" "desaparecio la carpeta"

# y si la cuantizacion falla
rm -rf "${GGUF_OUT:?}"/*; mkdir -p "$GGUF_OUT"
FAKE_QUANTIZE_FAIL=1 gguffer --gguf "$WORK/entrada-del-usuario/Mi.gguf" --quant IQ2_M >/dev/null 2>&1
check "26d. si falla, el GGUF de entrada sigue ahi" \
      "$(cond '[ -f "$WORK/entrada-del-usuario/Mi.gguf" ]')" "desaparecio"

# ══ 27. UN REPO CON SHARDS: se baja, se une y se cuantiza ════════════════
# El BF16 del Qwen3.8 va partido en dos ficheros, en una subcarpeta. El
# gguffer tiene que encontrarlo solo (es el unico sin cuantizar), bajar los
# dos shards y unirlos.
rm -rf "${GGUF_OUT:?}"/* "$WORK_ROOT"/gguf_*; mkdir -p "$GGUF_OUT"
out="$(FAKE_HF_GGUF=1 gguffer --gguf owner/Qwen3.8-27B-GGUF --quant IQ2_M 2>&1)"; rc=$?
check "27a. baja el repo y cuantiza" "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "27b. uno los shards" \
      "$(cond 'printf %s "$out" | grep -q "uno los 2 shards"')" "$out"
check "27c. el GGUF final tiene el nombre del MODELO (sin shards ni bf16)" \
      "$(cond '[ -f "$GGUF_OUT/Qwen3.8-27B-IQ2_M.gguf" ]')" "$(ls -A "$GGUF_OUT")"
check "27d. no baja el imatrix ni el mmproj" \
      "$(cond '! printf %s "$out" | grep -q "imatrix.gguf"')" "$out"

# ══ 28. EL REPO YA VIENE CUANTIZADO: se avisa ANTES de bajar ═════════════
# El fallo que duele: un repo de NVIDIA que ya viene cuantizado (trae
# hf_quant_config.json). El gguffer antes se bajaba 60 GB, los convertia, y
# solo al cuantizar el motor decia "requantizing from type nvfp4 is
# disabled", sin decir de donde venia el problema.
#
# Ahora se comprueba ANTES de descargar, y se dice cual es el repo hermano
# sin cuantizar que hay que usar en su lugar.
rm -rf "${GGUF_OUT:?}"/* "$WORK_ROOT"/gguf_*; mkdir -p "$GGUF_OUT"
out="$(FAKE_QUANT_CONFIG=1 gguffer --repo owner/modelo-NVFP4 --quant IQ3_XS 2>&1)"; rc=$?
check "28a. repo cuantizado -> error"      "$(cond '[ $rc -ne 0 ]')" "rc=$rc"
check "28b. repo cuantizado -> lo dice"    "$(cond 'printf %s "$out" | grep -q "YA VIENE CUANTIZADO"')" "$out"
check "28c. repo cuantizado -> NO descarga" "$(cond '[ ! -d "$WORK_ROOT/owner__modelo-NVFP4" ]')" "se bajó el clon"
check "28d. repo cuantizado -> dice requantizing" \
      "$(cond 'printf %s "$out" | grep -q "requantizing from type"')" "$out"
# El hermano sin cuantizar: el mismo nombre sin el sufijo del quant.
check "28e. propone el repo hermano sin cuantizar" \
      "$(cond 'printf %s "$out" | grep -q "ropero gguffer --repo owner/modelo-BF16"')" "$out"

# Y con un repo que NO viene cuantizado: el preflight no dice nada y se
# sigue el camino normal.
rm -rf "${GGUF_OUT:?}"/* "$WORK_ROOT"/gguf_*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "28f. repo normal -> no avisa"       "$(cond '! printf %s "$out" | grep -q "YA VIENE CUANTIZADO"')" "$out"
check "28g. repo normal -> cuantiza"       "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"

# Y con un repo que NO viene cuantizado: el preflight no dice nada y se
# sigue el camino normal.
rm -rf "${GGUF_OUT:?}"/* "$WORK_ROOT"/gguf_*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "28f. repo normal -> no avisa"       "$(cond '! printf %s "$out" | grep -q "YA VIENE CUANTIZADO"')" "$out"
check "28g. repo normal -> cuantiza"       "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"

# ══ 29. --list-quants CON UN REPO: tambien los de ESE modelo ════════════
# Los quants que el motor soporta son una cosa; los que hay hechas de este
# modelo, otra. Lo que decide si compensa fabricar es la segunda.
rm -rf "${GGUF_OUT:?}"/* "$WORK_ROOT"/gguf_*; mkdir -p "$GGUF_OUT"
out="$(gguffer --repo owner/Qwen3.8-27B --list-quants 2>&1)"; rc=$?
check "29a. list-quants -> ok"             "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "29b. list-quants -> los del motor"  "$(cond 'printf %s "$out" | grep -q "IQ2_XXS"')" "$out"
check "29c. list-quants -> los del modelo" "$(cond 'printf %s "$out" | grep -q "IQ2_M"')" "$out"
check "29d. list-quants -> dice de que repo" \
      "$(cond 'printf %s "$out" | grep -q "owner/Qwen3.8-27B"')" "$out"

# ══ 30. SI EL --out YA EXISTE, NO SE HACE NADA ════════════════════════════
# Rehacer un GGUF que ya esta es tirar una hora y 60 GB para nada, y
# encima pisa el fichero. El check va ANTES de descargar: si el destino
# ya esta, el gguffer dice que no hace nada y sale.
rm -rf "${GGUF_OUT:?}"/* "$WORK_ROOT"/gguf_*; mkdir -p "$GGUF_OUT"
# Con la ruta exacta del fichero, que es como lo llama el TOML.
mkdir -p "$GGUF_OUT/qwen"
echo "GGUF FALSO" > "$GGUF_OUT/qwen/repo-IQ3_XS.gguf"
out="$(gguffer --repo owner/repo --quant IQ3_XS --out "$GGUF_OUT/qwen/repo-IQ3_XS.gguf" 2>&1)"; rc=$?
check "30a. ya existe -> sale bien"        "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "30b. ya existe -> lo dice"          "$(cond 'printf %s "$out" | grep -q "ya existe"')" "$out"
check "30c. ya existe -> no hace nada"     "$(cond 'printf %s "$out" | grep -q "no se hace nada"')" "$out"
check "30d. ya existe -> NO descarga"      "$(cond '! printf %s "$out" | grep -q "descargo owner/repo"')" "se bajó el clon"
check "30e. ya existe -> NO lo toca"       "$(cond '[ "$(cat "$GGUF_OUT/qwen/repo-IQ3_XS.gguf")" = "GGUF FALSO" ]')" "el fichero se.overescribió"
# Con --force, si se rehace.
rm -rf "$WORK_ROOT/owner__repo"
out="$(gguffer --repo owner/repo --quant IQ3_XS --out "$GGUF_OUT/qwen/repo-IQ3_XS.gguf" --force 2>&1)"; rc=$?
check "30f. con --force, se rehace"         "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "30g. con --force, descarga"          "$(cond 'printf %s "$out" | grep -q "descargo owner/repo"')" "no se bajó el clon"
# Con un nombre (no ruta exacta), tambien avisa.
rm -rf "${GGUF_OUT:?}"/* "$WORK_ROOT"/gguf_*; mkdir -p "$GGUF_OUT"
# Con --out <nombre>, el fichero final es "<nombre>-<quant>.gguf" (el
# nombre es la BASE, no el nombre final).
echo "GGUF FALSO" > "$GGUF_OUT/repo-IQ3_XS-IQ3_XS.gguf"
out="$(gguffer --repo owner/repo --quant IQ3_XS --out repo-IQ3_XS 2>&1)"; rc=$?
check "30h. con --out <nombre>, ya existe"  "$(cond 'printf %s "$out" | grep -q "ya existe"')" "$out"
check "30i. y no hace nada"                 "$(cond '! printf %s "$out" | grep -q "descargo owner/repo"')" "se bajó el clon"

# ── resumen ────────────────────────────────────────────────────────────────
printf '\n%s: %d correctos, %d fallos\n' "$(basename "$0")" "$PASS" "$FAIL"
if (( FAIL > 0 )); then
    printf 'Fallos:\n'
    for c in "${FAILED_CASES[@]}"; do printf '  - %s\n' "$c"; done
    exit 1
fi
exit 0
