#!/usr/bin/env bash
# Tests del directorio quant/: el model card y los niveles de razonamiento.
#
#   1. el README dice que el modelo NO es mio (la nota va arriba)
#   2. el base_model sale normalizado (owner/repo, no una URL entera)
#   3. el quant sale con NOMBRE (Q2_K, no 10)
#   4. el CUANTIZATION.md tiene las condiciones: imatrix, MTP, como repetirla
#   5. sin procedencia, avisa pero no inventa
#   6. la subida sube el GGUF, el README y el CUANTIZATION.md, y marca
#   7. el fable y el nemotron declaran el nivel "think" y lo aplican bien
#
# Nada de esto necesita GPU ni red: el GGUF de prueba es un fichero con la
# cabecera minima, y el 'hf' es un falso que solo anota lo que se le pide.

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

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ropero-quant.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

ok()   { PASS=$((PASS+1)); [[ $VERBOSE -eq 1 ]] && printf '  ok   %s\n' "$1"; return 0; }
bad()  { FAIL=$((FAIL+1)); FAILED_CASES+=("$1"); printf '  FALLA %s\n' "$1"; [[ -n "${2:-}" ]] && printf '       %s\n' "$2"; return 0; }
check() { if [[ "$2" == "1" ]]; then ok "$1"; else bad "$1" "${3:-}"; fi; }
cond() { if eval "$1" >/dev/null 2>&1; then echo 1; else echo 0; fi; }

# ══ un GGUF de prueba con la cabecera minima ══════════════════════════════
# Se construye con el propio _ggufcheck.py como referencia de formato: una
# cabecera GGUF v3 con la metadata que el card lee.
GGUF="$WORK/fable-coder-35B-A3B.Q2_K.gguf"
FUENTE="$GGUF.fuente.json"
CARPETA="$WORK/gguf/fable"
mkdir -p "$CARPETA"

"$PYTHON_BIN" - "$GGUF" <<'PY'
import struct, sys
ruta = sys.argv[1]

def kv_str(k, v):
    b = v.encode()
    # GGUF: key (uint64 len + bytes) | type (uint32) | value
    return struct.pack("<Q", len(k.encode())) + k.encode() + struct.pack("<I", 8) + struct.pack("<Q", len(b)) + b

def kv_u32(k, v):
    return struct.pack("<Q", len(k.encode())) + k.encode() + struct.pack("<I", 4) + struct.pack("<I", v)

def kv_arr_str(k, vals):
    b = k.encode()
    out = struct.pack("<Q", len(b)) + b + struct.pack("<I", 9)
    out += struct.pack("<I", 8)          # tipo: string
    out += struct.pack("<Q", len(vals))
    for v in vals:
        vb = v.encode()
        out += struct.pack("<Q", len(vb)) + vb
    return out

kv = b""
kv += kv_str("general.architecture", "qwen35moe")
kv += kv_u32("general.file_type", 10)          # Q2_K
# OJO: la clave es LITERAL, con el ".0." en el nombre. No es un array
# anidado: el conversor de llama.cpp escribe "general.base_model.0.repo_url"
# como una clave normal (gguf-py/gguf/gguf_writer.py, add_base_model_repo_url).
kv += kv_str("general.base_model.0.repo_url", "https://huggingface.co/Achilles1089/fable-coder-35B-A3B")
kv += kv_str("general.base_model.0.name", "Fable Coder 35B A3B")
kv += kv_str("general.name", "fable-coder-35B-A3B")
kv += kv_u32("qwen35moe.block_count", 41)
kv += kv_u32("qwen35moe.nextn_predict_layers", 1)

n_kv = 7          # architecture, file_type, base_model.0.repo_url, .name, name, block_count, nextn
cab = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 0) + struct.pack("<Q", n_kv) + kv
with open(ruta, "wb") as f:
    f.write(cab)
PY

cat > "$FUENTE" <<'EOF'
{
  "gguf": "fable-coder-35B-A3B.Q2_K.gguf",
  "quant": "Q2_K",
  "fuente_repo": "Achilles1089/fable-coder-35B-A3B",
  "base_model": "https://huggingface.co/Achilles1089/fable-coder-35B-A3B",
  "base_model_name": "Fable Coder 35B A3B",
  "arch": "qwen35moe",
  "block_count": "41",
  "intermedio": "F16",
  "imatrix": {
    "fichero": "imatrix.gguf",
    "repo": "bartowski/fable-coder-35B-A3B-GGUF",
    "sha256": "0123456789abcdef"
  },
  "mtp": { "bloque": 40, "quant": "Q4_0" },
  "herramienta": "ropero gguffer (llama.cpp llama-quantize)",
  "fecha": "2026-10-27T09:00:00Z"
}
EOF

# el GGUF y su procedencia, en la carpeta donde los mira el subidor
cp "$GGUF" "$CARPETA/" || exit 1
cp "$FUENTE" "$CARPETA/"

# El GGUF de prueba TIENE que leerse: si no, el card se construiria solo con
# el .fuente.json y el test pasaria sin probar nada de la metadata.
info="$("$PYTHON_BIN" "$REPO/bin/_ggufcheck.py" "$GGUF" --info 2>&1)"
check "0d. el GGUF de prueba se lee" \
      "$(cond 'printf %s "$info" | grep -q "\"file_type\": 10"')" "$info"
check "0e. y trae el base_model" \
      "$(cond 'printf %s "$info" | grep -q "Achilles1089/fable-coder-35B-A3B"')" "$info"

CARD="$WORK/README.md"
COND="$WORK/CUANTIZATION.md"
MC="$REPO/quant/model-card.py"

out="$("$PYTHON_BIN" "$MC" --gguf "$GGUF" --fuente "$FUENTE" \
        --readme "$CARD" --condiciones "$COND" --usuario "Lorenzo-SF" 2>&1)"; rc=$?
check "0a. el card se genera"                "$(cond '[ $rc -eq 0 ]')" "$out"
check "0b. el README existe"                 "$(cond '[ -s "$CARD" ]')"
check "0c. el CUANTIZATION.md existe"        "$(cond '[ -s "$COND" ]')"

# ══ 1. la nota de que el modelo no es mio ════════════════════════════════
check "1a. el README avisa de que no es el autor" \
      "$(cond 'grep -qi "no soy el autor del modelo" "$CARD"')" "$CARD"
check "1b. la nota va ARRIBA (antes de la mitad)" \
      "$(cond '[ "$(head -40 "$CARD" | grep -c "No soy el autor")" -ge 1 ]')"
check "1c. nombra al autor original" \
      "$(cond 'grep -q "Achilles1089" "$CARD"')"
check "1d. dice quien cuantiza" \
      "$(cond 'grep -q "Lorenzo-SF" "$CARD"')"
check "1e. dice que use el GGUF oficial si existe" \
      "$(cond 'grep -qi "autor original publica" "$CARD"')"

# ══ 2. el base_model normalizado ══════════════════════════════════════════
check "2a. el frontmatter lleva owner/repo" \
      "$(cond 'grep -q "^base_model: Achilles1089/fable-coder-35B-A3B$" "$CARD"')" \
      "$(grep '^base_model:' "$CARD")"
check "2b. no hay una URL dentro de una URL" \
      "$(cond '! grep -q "huggingface.co/https" "$CARD"')"
check "2c. el enlace al modelo es correcto" \
      "$(cond 'grep -q "https://huggingface.co/Achilles1089/fable-coder-35B-A3B" "$CARD"')"

# ══ 3. el quant con nombre, no con numero ════════════════════════════════
check "3a. el quant sale con nombre" \
      "$(cond 'grep -q "Q2_K" "$CARD"')" "$CARD"
check "3b. no sale el numero crudo como quant" \
      "$(cond '! grep -q "cuantizado a \`10\`" "$CARD"')"

# ══ 4. las condiciones ════════════════════════════════════════════════════
check "4a. las condiciones nombran el imatrix" \
      "$(cond 'grep -q "imatrix.gguf" "$COND"')" "$COND"
check "4b. las condiciones dicen de donde sale el imatrix" \
      "$(cond 'grep -q "bartowski/fable-coder-35B-A3B-GGUF" "$COND"')"
check "4c. las condiciones explican el MTP" \
      "$(cond 'grep -qi "MTP" "$COND"')"
check "4d. las condiciones dicen el quant del MTP" \
      "$(cond 'grep -q "Q4_0" "$COND"')"
check "4e. las condiciones explica como repetirla" \
      "$(cond 'grep -q "ropero gguffer" "$COND"')"
check "4f. las condiciones dicen que el modelo es de otro" \
      "$(cond 'grep -qi "de otro autor" "$COND"')"

# ══ 5. sin procedencia: avisa, no inventa ══════════════════════════════════
out="$("$PYTHON_BIN" "$MC" --gguf "$GGUF" --fuente "$WORK/no-existe.json" \
        --readme "$WORK/card2.md" --condiciones "$WORK/cond2.md" 2>&1)"; rc=$?
check "5a. sin procedencia, avisa" \
      "$(cond 'printf %s "$out" | grep -q "no hay procedencia"')" "$out"
check "5b. sin procedencia, aun asi escribe el card" \
      "$(cond '[ -s "$WORK/card2.md" ]')"
check "5c. sin procedencia, dice que no se puede cuantizar con matriz" \
      "$(cond 'grep -qi "Sin importance matrix" "$WORK/cond2.md"')" "$WORK/cond2.md"

# ══ 6. la subida ══════════════════════════════════════════════════════════
FAKEBIN="$WORK/bin"
mkdir -p "$FAKEBIN"
cat > "$FAKEBIN/hf" <<'EOF'
#!/usr/bin/env bash
echo "$@" >> "${FAKE_LOG:-/dev/null}"
case "$1" in
    repo) exit 0 ;;
    upload) exit 0 ;;
esac
exit 0
EOF
chmod +x "$FAKEBIN/hf"
export FAKE_LOG="$WORK/hf.log"
: > "$FAKE_LOG"

GGUF_DIR="$WORK/gguf" PATH="$FAKEBIN:$PATH" HF_USER="usuario-de-prueba" \
    "$REPO/quant/subir-hf.sh" --subir >"$WORK/subir.out" 2>&1; rc=$?
check "6a. la subida va"                     "$(cond '[ $rc -eq 0 ]')" "$(cat "$WORK/subir.out")"
check "6a2. el destino es <modelo>-<quant>-GGUF" \
      "$(cond 'grep -q "usuario-de-prueba/fable-coder-35B-A3B-Q2_K-GGUF" "$FAKE_LOG"')" \
      "$(cat "$FAKE_LOG")"
check "6b. sube el GGUF"                    "$(cond 'grep -q "upload.*\.gguf" "$FAKE_LOG"')"
check "6c. sube el README"                  "$(cond 'grep -q "README.md" "$FAKE_LOG"')"
check "6d. sube el CUANTIZATION.md"         "$(cond 'grep -q "CUANTIZATION.md" "$FAKE_LOG"')"
check "6e. deja el marcador"                "$(cond '[ -f "$CARPETA/.hf-uploaded" ]')"
check "6f. el marcador dice donde subio"    "$(cond 'grep -q "usuario-de-prueba" "$CARPETA/.hf-uploaded"')"

# la segunda pasada no vuelve a subir
: > "$FAKE_LOG"
GGUF_DIR="$WORK/gguf" PATH="$FAKEBIN:$PATH" HF_USER="usuario-de-prueba" \
    "$REPO/quant/subir-hf.sh" --subir >"$WORK/subir2.out" 2>&1; rc=$?
check "6g. la segunda pasada no sube nada"  "$(cond '! grep -q "upload" "$FAKE_LOG"')"
check "6h. y lo dice"                      "$(cond 'grep -q "ya subido" "$WORK/subir2.out"')"

# ══ 6-bis. el imatrix: se sube EL de esta cuantizacion ════════════════════
# El GGUF no lleva el base_model en su metadata (el conversor de llama.cpp no
# lo escribe), asi que el imatrix se busca por la procedencia. Si no, se
# sube "el mas reciente de la cache", que con varios es el equivocado.
CACHE="$WORK/imcache"
mkdir -p "$CACHE/bartowski__fable-coder-35B-A3B-GGUF" "$CACHE/otro__modelo"
: > "$CACHE/bartowski__fable-coder-35B-A3B-GGUF/fable-imatrix.gguf"
: > "$CACHE/otro__modelo/otro-imatrix.gguf"
rm -f "$CARPETA/.hf-uploaded"
: > "$FAKE_LOG"
GGUF_DIR="$WORK/gguf" GGUF_IMATRIX_CACHE="$CACHE" PATH="$FAKEBIN:$PATH" \
    HF_USER="usuario-de-prueba" "$REPO/quant/subir-hf.sh" --subir >"$WORK/subir3.out" 2>&1; rc=$?
check "6i. con imatrix en la cache, la subida va" "$(cond '[ $rc -eq 0 ]')" "$(cat "$WORK/subir3.out")"
check "6j. sube el imatrix de ESTA cuantizacion" \
      "$(cond 'grep -q "fable-imatrix.gguf" "$FAKE_LOG"')" "$(cat "$FAKE_LOG")"
check "6k. y NO sube el de otro modelo" \
      "$(cond '! grep -q "otro-imatrix.gguf" "$FAKE_LOG"')" "$(cat "$FAKE_LOG")"

# ══ 7. los niveles de razonamiento del fable y del nemotron ═══════════════
for m in fable nemotron; do
    out="$(cd "$REPO" && "$PYTHON_BIN" bin/_modelconf.py args --root models --name "$m" 2>&1)"; rc=$?
    check "7a-$m. los args se montan"        "$(cond '[ $rc -eq 0 ]')" "$out"
    check "7b-$m. declara el nivel think" \
          "$(cond 'cd "$REPO" && "$PYTHON_BIN" bin/_modelconf.py args --root models --name "$m" --level think >/dev/null 2>&1')" "$out"
    check "7c-$m. sin nivel, el thinking esta apagado" \
          "$(cond 'printf %s "$out" | grep -q "enable_thinking.: false"')" "$out"
    out2="$(cd "$REPO" && "$PYTHON_BIN" bin/_modelconf.py args --root models --name "$m" --level think 2>&1)"
    check "7d-$m. con think, el thinking se enciende" \
          "$(cond 'printf %s "$out2" | grep -q "enable_thinking.: true"')" "$out2"
    check "7e-$m. con think, el false va ANTES que el true (gana el ultimo)" \
          "$(cond 'printf %s "$out2" | grep -n "enable_thinking" | head -1 | grep -q "false"')" "$out2"
done

# el nivel think solo existe en estos dos: el gptoss no lo acepta
check "7f. el gptoss no acepta el nivel think" \
      "$(cond 'cd "$REPO" && ! "$PYTHON_BIN" bin/_modelconf.py args --root models --name gptoss --level think >/dev/null 2>&1')"

# ── resumen ────────────────────────────────────────────────────────────────
printf '\n%s: %d correctos, %d fallos\n' "$(basename "$0")" "$PASS" "$FAIL"
if (( FAIL > 0 )); then
    printf 'Fallos:\n'
    for c in "${FAILED_CASES[@]}"; do printf '  - %s\n' "$c"; done
    exit 1
fi
exit 0
