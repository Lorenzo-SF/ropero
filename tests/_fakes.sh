#!/usr/bin/env bash
# =============================================================================
# tests/_fakes.sh — los dobles de los tests, compartidos.
#
# Los usa test-gguffer.sh (el gguffer) y test-extras.sh (el instalador):
# el conversor, el cuantizador, el hf y el maker de GGUF son los mismos, y
# duplicarlos haria que un cambio en uno no llegase al otro (la misma verdad
# en dos sitios siempre diverge).
#
# El que lo sourcea tiene que definir ANTES:
#   WORK            el directorio de trabajo
#   LLAMA_DIR_TEST  el clon de llama.cpp falso
#   BIN_DIR         donde van los ejecutables falsos
#   WORK_ROOT       el work del gguffer
#
# Y puede definir (opcionales):
#   FAKE_HF_FAIL, FAKE_HF_GGUF, FAKE_HF_GGUF_MAKER, FAKE_QUANTIZE_FAIL,
#   FAKE_QUANTIZE_GARBAGE, FAKE_QUANTIZE_LOG, FAKE_QUANTIZE_MSG,
#   FAKE_CONVERT_COUNT, FAKE_QUANT_CONFIG
# =============================================================================
# ── el conversor fake: escribe un GGUF minimo pero valido ────────────────
cat >"$LLAMA_DIR_TEST/convert_hf_to_gguf.py" <<'PY'
import struct
import sys

args = sys.argv[1:]
outfile = args[args.index("--outfile") + 1]
src = args[0]

# El conversor real pide el tokenizer con AutoTokenizer, y con transformers
# v4 la clase "TokenizersBackend" (de la v5) no existe. Este fake reproduce
# ese error si el clon trae esa clase, para que se pueda probar el arreglo.
import json, os
cfg = os.path.join(src, "tokenizer_config.json")
if os.path.exists(cfg):
    try:
        with open(cfg, encoding="utf-8") as f:
            c = json.load(f)
    except Exception:
        c = {}
    if c.get("tokenizer_class") == "TokenizersBackend":
        sys.stderr.write(
            "ValueError: Tokenizer class TokenizersBackend does not exist "
            "or is not currently imported.\n")
        sys.exit(1)

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
print("fake convert: %s -> %s" % (src, outfile))
PY

# ── generadores de GGUF e imatrix para el caso de la MTP ─────────────────
# Un GGUF con N bloques y, opcionalmente, un bloque de MTP (nextn) que el
# imatrix NO cubre: es el caso real del Qwen3.8-27B.
cat >"$WORK/gguf_maker.py" <<'PY'
import struct, sys

def s(x):
    b = x.encode(); return struct.pack("<Q", len(b)) + b

def escribe(path, tensores, arch="qwen35", nextn=0):
    # n_kv: architecture + file_type + block_count (+ nextn)
    n_kv = 4 if nextn else 3
    body = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", len(tensores)) + struct.pack("<Q", n_kv)
    body += s("general.architecture") + struct.pack("<I", 8) + s(arch)
    body += s("general.file_type") + struct.pack("<I", 4) + struct.pack("<I", 1)
    body += s("%s.block_count" % arch) + struct.pack("<I", 4) + struct.pack("<I", 4)
    if nextn:
        body += s("%s.nextn_predict_layers" % arch) + struct.pack("<I", 4) + struct.pack("<I", nextn)
    for n, dims in tensores:
        body += s(n) + struct.pack("<I", 2)
        for d in dims: body += struct.pack("<Q", d)
        body += struct.pack("<I", 1) + struct.pack("<Q", 0)
    body = body.ljust((len(body) + 31) // 32 * 32, b"\x00") + b"\x00" * 16
    open(path, "wb").write(body)

def imatrix(path, tensores):
    # Un imatrix es un GGUF con pares <tensor>.in_sum2 + <tensor>.counts
    salida = []
    for n, dims in tensores:
        salida.append((n + ".in_sum2", [dims[0], 1]))
        salida.append((n + ".counts", [1, 1]))
    escribe(path, salida, arch="imatrix")

if __name__ == "__main__":
    modo = sys.argv[1]
    ruta = sys.argv[2]
    nbloques = int(sys.argv[3])
    if modo == "gguf-sin-mtp":
        ts = []
        for i in range(nbloques):
            ts += [("blk.%d.attn_k.weight" % i, [5120, 1024]),
                   ("blk.%d.ffn_down.weight" % i, [17408, 5120])]
        escribe(ruta, ts, nextn=0)
    elif modo == "nombres":
        for t in sys.argv[4:]:
            print(t)
    elif modo == "gguf":
        ts = []
        for i in range(nbloques):
            ts += [("blk.%d.attn_k.weight" % i, [5120, 1024]),
                   ("blk.%d.ffn_down.weight" % i, [17408, 5120])]
        # El bloque de la MTP. OJO: sus tensores se llaman como los de
        # cualquier otro bloque. NO llevan "nextn" en el nombre: el
        # conversor crea el bloque de la MTP con los nombres NORMALES
        # ("blk.64.attn_k.weight"). Por eso la MTP se localiza con la
        # METADATA (block_count - nextn_predict_layers) y no buscando
        # "nextn." en los nombres: asi se encuentra en los GGUFs de verdad.
        ts += [("blk.%d.attn_k.weight" % nbloques, [5120, 1024]),
               ("blk.%d.ffn_down.weight" % nbloques, [17408, 5120]),
               ("blk.%d.ffn_up.weight" % nbloques, [17408, 5120])]
        escribe(ruta, ts, nextn=1)
    else:
        # el imatrix solo cubre los bloques 0..n-1: el de la MTP no
        ts = []
        for i in range(nbloques):
            ts += [("blk.%d.attn_k.weight" % i, [5120, 1024]),
                   ("blk.%d.ffn_down.weight" % i, [17408, 5120])]
        imatrix(ruta, ts)
PY

# ── el cuantizador fake ───────────────────────────────────────────────────
cat >"$LLAMA_DIR_TEST/build/bin/llama-quantize" <<'EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--help" ]]; then
    cat <<'H'
    19  or  IQ2_XXS :  2.06 bpw quantization
    29  or  IQ2_M   :  2.70 bpw quantization
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
    if [[ -n "${FAKE_QUANTIZE_MSG:-}" ]]; then
        echo "$FAKE_QUANTIZE_MSG" >&2
    else
        echo "fake: fallo la cuantizacion" >&2
    fi
    exit 1
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
    # los overrides de tipo por tensor, tal cual los recibe
    prev=""
    for a in "$@"; do
        [[ "$prev" == "--tensor-type" ]] && printf 'tensor-type: %s\n' "$a" >> "$FAKE_QUANTIZE_LOG"
        prev="$a"
    done
fi
# Los posicionales van despues de las opciones: <in> [out] <ftype>
# OJO: el VALOR de una opcion no es un posicional. "--tensor-type
# blk\.64\..*=Q4_0" es una opcion con su valor; si se contara el valor
# como posicional, el cuantizador creeria que el fichero de entrada se
# llama "blk\.64\..*=Q4_0" (y de ahi el fichero basura en el repo).
pos=()
skip=0
for a in "$@"; do
    if (( skip )); then skip=0; continue; fi
    case "$a" in
        --imatrix|--tensor-type|--output-tensor-type|--token-embedding-type|--tensor-type-file)
            skip=1; continue ;;
    esac
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
fichero=""
sub=0
for a in "$@"; do
    [[ "$prev" == "--local-dir" ]] && local_dir="$a"
    prev="$a"
done
[[ -n "$local_dir" ]] || { echo "fake hf: sin --local-dir" >&2; exit 2; }
# "hf download <repo> [<fichero>] --local-dir <dir>": si hay fichero, se crea
# ese (respetando su subdirectorio, como hace el hf de verdad). Si no hay,
# se finge la descarga del repo entero.
if [[ "${1:-}" == "download" && -n "${3:-}" && "${3}" != --* ]]; then
    fichero="$3"
fi
mkdir -p "$local_dir"
if [[ -n "$fichero" ]]; then
    mkdir -p "$local_dir/$(dirname "$fichero")"
    if [[ "${FAKE_HF_GGUF:-}" == "1" ]]; then
        # Un GGUF de verdad (lo hace el generador de test).
        python3 "$FAKE_HF_GGUF_MAKER" gguf "$local_dir/$fichero" 3
    else
        printf 'imatrix falso\n' > "$local_dir/$fichero"
    fi
    printf '{"model_type":"qwen35"}\n' > "$local_dir/config.json"
    printf 'pesos falsos\n' > "$local_dir/model-00001-of-00001.safetensors"
    exit 0
fi
printf '{"model_type":"qwen35"}\n' > "$local_dir/config.json"
printf 'pesos falsos\n' > "$local_dir/model-00001-of-00001.safetensors"
exit 0
EOF
chmod +x "$BIN_DIR/hf"

# ── un curl fake: la API de HuggingFace con la lista de ficheros de un repo ─
# El gguffer usa la API para saber que ficheros tiene un repo (y si el GGUF
# va partido en shards). Con este curl se falsea SOLO esa llamada.
cat >"$BIN_DIR/curl" <<'EOF'
#!/usr/bin/env bash
# Un curl fake: la API de HuggingFace con la lista de ficheros de un repo.
#
# Honra el "-o FICHERO" como el curl de verdad: si no, el codigo que
# comprueba un JSON ("_gguffer_es_json") receberia un fichero vacio y
# pensaria que el repo no existe.
_out=""
_prev=""
for a in "$@"; do
    [[ "$_prev" == "-o" ]] && _out="$a"
    _prev="$a"
done
_emit() { # _emit <contenido>
    if [[ -n "$_out" ]]; then
        printf '%s\n' "$1" > "$_out"
    else
        printf '%s\n' "$1"
    fi
}
for a in "$@"; do
    case "$a" in
        https://huggingface.co/api/models/*)
            repo="${a##*/}"
            _emit '{"siblings":[
 {"rfilename":"README.md"},
 {"rfilename":"Qwen3.8-27B-imatrix.gguf"},
 {"rfilename":"mmproj-Qwen3.8-27B-bf16.gguf"},
 {"rfilename":"Qwen3.8-27B-IQ2_M.gguf"},
 {"rfilename":"Qwen3.8-27B-Q8_0.gguf"},
 {"rfilename":"Qwen3.8-27B-bf16/Qwen3.8-27B-bf16-00001-of-00002.gguf"},
 {"rfilename":"Qwen3.8-27B-bf16/Qwen3.8-27B-bf16-00002-of-00002.gguf"}
]}'
            exit 0 ;;
        # El filtro base_model: de la API (los quants ya publicados).
        "https://huggingface.co/api/models?filter=base_model:"*)
            _emit '[{"id":"bartowski/Qwen3.8-27B-GGUF","tags":["gguf","base_model:Qwen/Qwen3.8-27B"]}]'
            exit 0 ;;
        # El config.json de un repo. FAKE_QUANT_CONFIG=1 lo hace declara
        # cuantizado (que es como se detecta un checkpoint ya cuantizado).
        # Solo existe para los repos "owner/...": para el resto, 404, como
        # en la vida real (un repo que no existe NO tiene config.json).
        https://huggingface.co/*/raw/main/config.json)
            case "$a" in
                https://huggingface.co/owner/*) ;;
                *) exit 22 ;;
            esac
            # FAKE_QUANT_CONFIG=1 declara cuantizado SOLO el repo con
            # sufijo de quant: el hermano sin cuantizar tiene que existir
            # y estar limpio, que es justo lo que se busca.
            # El repo se saca de la URL: "owner/modelo-NVFP4/raw/main/config.json".
            _repo="${a#https://huggingface.co/}"; _repo="${_repo%%/raw/*}"
            if [[ "${FAKE_QUANT_CONFIG:-0}" == "1" && "$_repo" == *-NVFP4 ]]; then
                _emit '{"model_type":"qwen3","quantization_config":{"quant_method":"modelopt"}}'
            else
                _emit '{"model_type":"qwen3"}'
            fi
            exit 0 ;;
        # El hf_quant_config.json: si existe, el checkpoint ya viene cuantizado.
        https://huggingface.co/*/raw/main/hf_quant_config.json)
            case "$a" in
                https://huggingface.co/owner/*) ;;
                *) exit 22 ;;
            esac
            _repo="${a#https://huggingface.co/}"; _repo="${_repo%%/raw/*}"
            if [[ "${FAKE_QUANT_CONFIG:-0}" == "1" && "$_repo" == *-NVFP4 ]]; then
                _emit '{"quantization":{"quant_algo":"W4A16_NVFP4"}}'
                exit 0
            fi
            exit 22 ;;
    esac
done
exec /usr/bin/curl "$@"
EOF
chmod +x "$BIN_DIR/curl"

# ── un llama-gguf-split fake: une los shards ─────────────────────────────
mkdir -p "$LLAMA_DIR_TEST/build/bin"
cat >"$LLAMA_DIR_TEST/build/bin/llama-gguf-split" <<'EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--merge" ]]; then
    # El merge de verdad: un solo GGUF con los tensores de los shards. Aqui
    # vale con concatenar el primero (lo que importa es que el gguffer lo
    # llame con los shards en el orden correcto y con el nombre de salida).
    out="${3:-merged.gguf}"
    cat "$2" > "$out"
    echo "fake merge: $2 -> $out"
    exit 0
fi
if [[ "${1:-}" == "--dry-run" ]]; then
    # El dry-run comprueba si el fichero es un GGUF que se puede partir.
    # Si no lo es, falla (como el de verdad).
    f="${2:-}"
    if [[ -f "$f" ]] && head -c 4 "$f" | grep -q GGUF; then exit 0; fi
    exit 1
fi
exit 0
EOF
chmod +x "$LLAMA_DIR_TEST/build/bin/llama-gguf-split"
