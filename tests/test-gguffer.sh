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

# ── el conversor fake: escribe un GGUF minimo pero valido ────────────────
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import struct
import sys

args = sys.argv[1:]
outfile = args[args.index("--outfile") + 1]
src = args[0]

def s(x):
    b = x.encode()
    return struct.pack("<Q", len(b)) + b

body = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 1) + struct.pack("<Q", 2)
body += s("general.architecture") + struct.pack("<I", 8) + s("qwen35")
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 0)
idx = s("blk.0.weight") + struct.pack("<I", 1) + struct.pack("<Q", 4) + struct.pack("<I", 0) + struct.pack("<Q", 0)
body += idx
body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
with open(outfile, "wb") as f:
    f.write(body)
print("fake convert: %s -> %s" % (src, outfile))
PY

# ── el cuantizador fake ───────────────────────────────────────────────────
cat >"$LLAMA_DIR_TEST/build/bin/llama-quantize" <<'EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--help" ]]; then
    cat <<'H'
    19  or  IQ2_XXS :  2.06 bpw quantization
    22  or  IQ3_XS  :  3.3 bpw quantization
    30  or  IQ4_XS  :  4.25 bpw quantization
    38  or  MXFP4_MOE :  MXFP4 MoE
    39  or  NVFP4   :  4.50 bpw NVIDIA FP4
    44  or  TQ4_1S  :  5.00 bpw WHT-rotated
     1  or  F16     : 14.00G, +0.0020 ppl @ Mistral-7B
H
    exit 0
fi
if [[ -n "${FAKE_QUANTIZE_FAIL:-}" ]]; then
    echo "fake: fallo la cuantizacion" >&2
    exit 1
fi
if [[ -n "${FAKE_QUANTIZE_GARBAGE:-}" ]]; then
    printf 'esto no es un GGUF' > "$2"
    exit 0
fi
# El quantizador real (tools/quantize/quantize.cpp) solo parsea las opciones
# mientras el argumento empiece por "--", y para en el primero que no. Si una
# flag llega despues de los posicionales, se come el parseo del numero de
# hilos y muere con "invalid nthread". Este fake lo reproduce, para que el
# test lo pille.
imatrix=""
seen_positional=0
prev=""
for a in "$@"; do
    if [[ "$prev" == "--imatrix" ]]; then
        if (( seen_positional )); then
            echo "llama_quantize: invalid nthread '$a'" >&2
            exit 1
        fi
        imatrix="$a"
    fi
    [[ "$a" == --* ]] || seen_positional=1
    prev="$a"
done
if [[ -n "${FAKE_QUANTIZE_LOG:-}" ]]; then
    printf '%s\n' "$imatrix" > "$FAKE_QUANTIZE_LOG"
fi
# Los posicionales van despues de las opciones: <in> [out] <ftype>
pos=()
for a in "$@"; do
    [[ "$a" == --* ]] || pos+=("$a")
done
in="${pos[0]}"
out_f="${pos[1]:-${pos[0]}}"
if [[ -n "${FAKE_QUANTIZE_GARBAGE:-}" ]]; then
    printf 'esto no es un GGUF' > "$out_f"
    exit 0
fi
cp "$in" "$out_f"
exit 0
EOF
chmod +x "$LLAMA_DIR_TEST/build/bin/llama-quantize"

# ── el hf fake ────────────────────────────────────────────────────────────
cat >"$BIN_DIR/hf" <<'EOF'
#!/usr/bin/env bash
if [[ "${FAKE_HF_FAIL:-}" == "1" ]]; then
    echo "fake: fallo la descarga" >&2
    exit 1
fi
local_dir=""
prev=""
for a in "$@"; do
    [[ "$prev" == "--local-dir" ]] && local_dir="$a"
    prev="$a"
done
[[ -n "$local_dir" ]] || { echo "fake hf: sin --local-dir" >&2; exit 2; }
mkdir -p "$local_dir"
printf '{"model_type":"qwen35"}\n' > "$local_dir/config.json"
printf 'pesos falsos\n' > "$local_dir/model-00001-of-00001.safetensors"
exit 0
EOF
chmod +x "$BIN_DIR/hf"

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
        FAKE_QUANTIZE_FAIL="${FAKE_QUANTIZE_FAIL:-}" \
        FAKE_QUANTIZE_GARBAGE="${FAKE_QUANTIZE_GARBAGE:-}" \
        FAKE_QUANTIZE_LOG="${FAKE_QUANTIZE_LOG:-}" \
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
check "3b. conversion fallida -> conserva pesos" "$(cond '[ -d "$REPO_DIR" ]')" "no existe $REPO_DIR"
check "3c. conversion fallida -> dice como reintentar" \
      "$(cond 'printf %s "$out" | grep -q "reintenta solo la conversion"')" "$out"
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
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 0)
idx = s("blk.0.weight") + struct.pack("<I", 1) + struct.pack("<Q", 4) + struct.pack("<I", 0) + struct.pack("<Q", 0)
body += idx
body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
with open(outfile, "wb") as f:
    f.write(body)
PY
out="$(FAKE_QUANTIZE_FAIL=1 gguffer --repo owner/repo --quant IQ3_XS 2>&1)"; rc=$?
check "4a. cuantizacion fallida -> error"       "$(cond '[ $rc -ne 0 ]')" "rc=$rc"
check "4b. cuantizacion fallida -> conserva pesos" "$(cond '[ -d "$REPO_DIR" ]')"
check "4c. cuantizacion fallida -> conserva el F16" \
      "$(cond 'ls "${GGUF_OUT:?}"/*-F16.gguf >/dev/null 2>&1')" "$(ls -A "$GGUF_OUT")"
check "4d. cuantizacion fallida -> dice como reintentar" \
      "$(cond 'printf %s "$out" | grep -q "reintenta solo la cuantizacion"')" "$out"

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
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 0)
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
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 0)
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
body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 0)
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

# ── resumen ────────────────────────────────────────────────────────────────
printf '\n%s: %d correctos, %d fallos\n' "$(basename "$0")" "$PASS" "$FAIL"
if (( FAIL > 0 )); then
    printf 'Fallos:\n'
    for c in "${FAILED_CASES[@]}"; do printf '  - %s\n' "$c"; done
    exit 1
fi
exit 0
