#!/usr/bin/env bash
# =============================================================================
# tests/test-ggufcheck.sh — ggufcheck.py lee SOLO la cabecera en todos los modos
# =============================================================================
# Crea un GGUF sintetico pequeno con un "cuerpo" de datos grande y comprueba
# que cada modo funciona y que ninguno lee el cuerpo (limite de memoria
# virtual bajo + control de bytes leidos).
# =============================================================================
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK="$ROOT/bin/python/ggufcheck.py"
TMP="${TMPDIR:-/tmp/opencode}/ggufcheck-test.$$"
mkdir -p "$TMP" || exit 2
trap 'rm -rf "$TMP"' EXIT

python3 - "$TMP/sint.gguf" <<'EOF' || exit 2
import struct, sys
def s(x): b = x.encode(); return struct.pack("<Q", len(b)) + b
kv = [("general.architecture", 8, s("qwen35")), ("general.file_type", 4, struct.pack("<I", 1)),
      ("qwen35.block_count", 4, struct.pack("<I", 5)),
      ("qwen35.nextn_predict_layers", 4, struct.pack("<I", 1))]
h = b"GGUF" + struct.pack("<IQQ", 3, 2, len(kv))
for k, t, v in kv:
    h += s(k) + struct.pack("<I", t) + v
# 2 tensores F32 (tipo 0) de 8 elementos: 32 bytes cada uno
h += s("blk.0.w") + struct.pack("<IQIQ", 1, 8, 0, 0)
h += s("blk.4.w") + struct.pack("<IQIQ", 1, 8, 0, 32)
h += b"\0" * ((-len(h)) % 32)
open(sys.argv[1], "wb").write(h + b"\0" * 64)
EOF

fallos=0
ok()  { printf 'ok   %s\n' "$1"; }
mal() { printf 'FALLO %s\n' "$1" >&2; fallos=$((fallos + 1)); }
run() { python3 "$CHECK" "$TMP/sint.gguf" "$@" 2>/dev/null; }

out="$(run --meta)"
[[ "$out" == $'arch=qwen35\nblock_count=5\nnextn_predict_layers=1\nbloque_mtp=4' ]] && ok "--meta" || mal "--meta: $out"
[[ "$(run --tipo-predominante)" == "0" ]] && ok "--tipo-predominante" || mal "--tipo-predominante"
# El --file-type es el general.file_type (un llama_ftype: el mismo numero que
# lleva en el GGUF y que imprime "llama-quantize --help"). El --tipo-predominante
# es OTRO espacio (el ggml_type del tensor): para el GGUF de aqui son 1 y 0.
[[ "$(run --file-type)" == "1" ]] && ok "--file-type" || mal "--file-type: $(run --file-type)"
[[ "$(run --nombres | tr '\n' ' ')" == "blk.0.w blk.4.w " ]] && ok "--nombres" || mal "--nombres"
[[ "$(run --bloque 4)" == "blk.4.w" ]] && ok "--bloque" || mal "--bloque"
run --info | grep -q '"tensores": 2' && ok "--info" || mal "--info"
run >/dev/null && ok "validacion por defecto" || mal "validacion por defecto"

# Ningun modo debe leer el fichero entero: se trunca un tensor y se espera exit 1
# solo si la libggml esta disponible; aqui se comprueba que no se hace read() total.
python3 - "$ROOT/bin/python" "$TMP/sint.gguf" <<'EOF' && ok "sin read() completo" || mal "sin read() completo"
import sys, builtins
sys.path.insert(0, sys.argv[1])
import ggufcheck
grande = sys.argv[2] + ".grande"
with open(sys.argv[2], "rb") as a, open(grande, "wb") as b:
    b.write(a.read()); b.truncate(300 * 1024 * 1024)   # fichero disperso de 300 MiB
leidos = []
orig = builtins.open
class Esp:
    """Proxy de un fichero que anota cuanto devuelve cada read()."""
    def __init__(self, f): self._f = f
    def __enter__(self): return self
    def __exit__(self, *a): self._f.close()
    def read(self, n=-1):
        d = self._f.read(n); leidos.append(len(d)); return d
    def __getattr__(self, k): return getattr(self._f, k)
def abre(p, m="r", *a, **k):
    f = orig(p, m, *a, **k)
    return Esp(f) if "b" in m and "w" not in m else f
builtins.open = abre
g = ggufcheck.Gguf(grande)
assert g.tam == 300 * 1024 * 1024 and len(g.tensors) == 2
assert max(leidos) < 100 * 1024 * 1024, leidos
EOF

# ── model_card.py usa Gguf(): no debe fallar con TypeError ──────────────────
mc_err="$TMP/mc.err"
python3 "$ROOT/bin/python/model_card.py" --gguf "$TMP/sint.gguf" \
    --readme "$TMP/README.md" --condiciones "$TMP/CUANT.md" >"$TMP/mc.out" 2>"$mc_err"
rc=$?
if [[ $rc -eq 0 ]] && ! grep -q TypeError "$mc_err" && [[ -s "$TMP/README.md" && -s "$TMP/CUANT.md" ]]; then
    ok "model_card.py --gguf (rc=0, sin TypeError)"
else
    mal "model_card.py --gguf rc=$rc: $(cat "$mc_err")"
fi
# compatibilidad: el parametro deprecado solo_cabecera se acepta e ignora
python3 - "$ROOT/bin/python" "$TMP/sint.gguf" <<'EOF' && ok "Gguf(solo_cabecera=True) aceptado" || mal "Gguf(solo_cabecera=True)"
import sys
sys.path.insert(0, sys.argv[1])
import ggufcheck
assert len(ggufcheck.Gguf(sys.argv[2], solo_cabecera=True).tensors) == 2
EOF

# ── indices de tensores incompletos ─────────────────────────────────────────
python3 - "$TMP/incompleto.gguf" "$TMP/pocos.gguf" "$TMP/absurdo.gguf" <<'EOF' || exit 2
import struct, sys
def s(x): b = x.encode(); return struct.pack("<Q", len(b)) + b
kv = [("general.architecture", 8, s("qwen35"))]
def cab(n):
    h = b"GGUF" + struct.pack("<IQQ", 3, n, len(kv))
    for k, t, v in kv:
        h += s(k) + struct.pack("<I", t) + v
    return h
t1 = s("blk.0.w") + struct.pack("<IQIQ", 1, 8, 0, 0)
t2 = s("blk.4.w") + struct.pack("<IQIQ", 1, 8, 0, 32)
# declara 3; el tercero esta cortado a la mitad y el fichero acaba ahi
open(sys.argv[1], "wb").write(cab(3) + t1 + t2 + s("blk.9.w")[:10])
# declara 1000 tensores y trae 2: ni siquiera cabe el indice minimo
open(sys.argv[2], "wb").write(cab(1000) + t1 + t2)
# n_tensors = 2^40 en un fichero disperso de 1 GiB
with open(sys.argv[3], "wb") as f:
    f.write(cab(1 << 40) + t1 + t2); f.truncate(1 << 30)
EOF
msg="$(python3 "$CHECK" "$TMP/incompleto.gguf" 2>&1 >/dev/null)"; rc=$?
if [[ $rc -eq 1 && "$msg" == *"declara 3 tensores"*"solo tiene 2"* && "$msg" != *Traceback* ]]; then
    ok "indice incompleto: rc=1 y recuento real"
else
    mal "indice incompleto rc=$rc: $msg"
fi
msg="$(python3 "$CHECK" "$TMP/pocos.gguf" 2>&1 >/dev/null)"; rc=$?
if [[ $rc -eq 1 && "$msg" == *"declara 1000 tensores"* && "$msg" != *Traceback* ]]; then
    ok "n_tensors imposible para el fichero: rc=1 y mensaje claro"
else
    mal "n_tensors imposible rc=$rc: $msg"
fi
t0=$SECONDS
msg="$(python3 "$CHECK" "$TMP/absurdo.gguf" 2>&1 >/dev/null)"; rc=$?
dt=$((SECONDS - t0))
if [[ $rc -eq 1 && $dt -lt 5 && "$msg" == *"GGUF invalido"* ]]; then
    ok "n_tensors=2^40 falla rapido (${dt}s)"
else
    mal "n_tensors=2^40 rc=$rc t=${dt}s: $msg"
fi

[[ $fallos -eq 0 ]] && { echo "todo ok"; exit 0; }
exit 1
