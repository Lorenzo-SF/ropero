#!/usr/bin/env bash
# =============================================================================
# bin/lib/cmd-gguffer.sh — `ropero gguffer` (spec §8.15)
#
# ÚNICA implementación de fabricación de GGUFs (descarga → conversión →
# imatrix → cuantización → verificación → model card). La comparte el CLI y
# la ruta `[build]` de los TOML (vía rp_model_build_argv).
#
# Portado del monolito 0.x. Correcciones de 1.0:
#   · cuantización ya aplicada en el repo NO es error: se coloca el gguf y
#     se sale 0 sin borrar nada (ver _gguffer_repo_ya_cuantizado).
#   · bash 3.2: sin mapfile (bucles while IFS= read -r).
#   · ggufcheck.py vive en $ROPERO_ROOT/bin/python/.
# =============================================================================

_rp_gguffer_help() {
    cat <<'HELP'
USO: ropero gguffer (--path DIR | --repo OWNER/NAME | --gguf FICHERO|OWNER/REPO) [opciones]

Fabrica un GGUF desde pesos HF o reutiliza un GGUF ya publicado.

  --repo OWNER/NAME          repo de pesos safetensors a convertir
  --path DIR                 directorio local con los pesos
  --gguf FICHERO|OWNER/REPO  GGUF ya cuantizado (local o de HF)
  --gguf-file NOMBRE         fichero dentro del repo de --gguf
  --quant TIPO[,TIPO...]     cuantizaciones a fabricar (repetible)
  --imatrix FICHERO          importance matrix local
  --imatrix-repo OWNER/NAME  repo del imatrix
  --imatrix-file NOMBRE      fichero del imatrix dentro del repo
  --out NOMBRE|RUTA          nombre/carpeta/ruta de salida
  --out-principal RUTA       ruta del quant principal (los demas a su lado)
  --from-subdir DIR          subcarpeta del repo que contiene los pesos
  --include PATRON           incluir fichero(s) del repo
  --intermedio f16|bf16      precision del GGUF intermedio (default f16)
  --keep-source              no borra los pesos descargados
  --keep-intermedio          no borra el GGUF intermedio
  --no-mtp                   no fabrica el GGUF MTP
  --force                    rehace aunque exista
  --list-quants              lista quants del motor y publicados
  --diagnostico              diagnostico del entorno
  -h, --help                 esta ayuda
HELP
}

_gguffer_parse_out() {
    local v="${1:-}"
    ROPERO_GGUF_OUT_DIR=""
    ROPERO_GGUF_OUT_FILE=""
    case "$v" in
        "")
            echo "ERROR: --out no puede estar vacio" >&2
            return 2 ;;
        *.gguf)
            # Ruta exacta del fichero: el directorio es el de la ruta.
            ROPERO_GGUF_OUT_FILE="$v" ;;
        */*)
            # Un directorio: se le quita la "/" final si la lleva.
            ROPERO_GGUF_OUT_DIR="${v%/}" ;;
        *)
            # Un nombre suelto: el fichero se llama <nombre>-<quant>.gguf.
            # OJO: "out_name" es un local de cmd_gguffer, y el scoping de bash
            # es dinamico: esta asignacion la cambia. Por eso esta linea
            # funciona y no parece que toque nada.
            out_name="$v" ;;
    esac
    return 0
}

# -----------------------------------------------------------------------------
# _gguffer_parse_out_principal <ruta>
#
# --out-principal es la ruta EXACTA del primer quant de la tirada. Con varios
# quants, cada uno necesita la suya, y esta es la del que va a terminar en la
# carpeta del modelo (la que espera el TOML). Los demas se quedan en el
# directorio normal, con su nombre <repo>-<quant>.gguf.
# -----------------------------------------------------------------------------
_gguffer_parse_out_principal() {
    local v="${1:-}"
    ROPERO_GGUF_OUT_PRINCIPAL=""
    ROPERO_GGUF_OUT_PRINCIPAL_DIR=""
    case "$v" in
        "")
            echo "ERROR: --out-principal no puede estar vacio" >&2
            return 2 ;;
        *.gguf)  ROPERO_GGUF_OUT_PRINCIPAL="$v" ;;
        */)      ROPERO_GGUF_OUT_PRINCIPAL_DIR="${v%/}" ;;
        *)
            # Un directorio que ya existe: se trata como carpeta. Es lo que
            # necesita el caso "el TOML pide un quant que no es el que
            # fabricamos": el GGUF va a la carpeta del modelo con su nombre
            # propio, no a la ruta del TOML (que seria un nombre mentiroso).
            if [[ -d "$v" ]]; then
                ROPERO_GGUF_OUT_PRINCIPAL_DIR="${v%/}"
            else
                echo "ERROR: --out-principal necesita la ruta de un fichero .gguf" >&2
                echo "       (o un directorio existente, para que ponga el nombre el)" >&2
                return 2
            fi ;;
    esac
    return 0
}

# -----------------------------------------------------------------------------
# _gguffer_imatrix_cubre_bloque <imatrix> <bloque> <ggufcheck>
#
# Devuelve 0 si el imatrix tiene entradas para el bloque dado, 1 si no tiene
# ninguna.
#
# Por que importa: el imatrix se calcula sobre el modelo base (64 capas), pero
# el GGUF lleva la cabeza MTP como un bloque mas (el 65). Ese bloque no esta
# en el imatrix, y si el quant es very-low-bit (IQ2_M, IQ3_XXS...) el
# cuantizador aborta con "Missing importance matrix for tensor blk.64...".
# Es lo que hacia bartowski con su IQ2_M del Qwen3.8: dejo el bloque de la
# MTP en Q4_0, que no exige imatrix.
#
# Se compara el bloque entero (no tensor a tensor): los tensores que el
# imatrix no tiene son normalmente las normas f32, que no se cuantizan y no
# exigen imatrix. Lo que aborta el cuantizador es un bloque entero sin
# cobertura.
# -----------------------------------------------------------------------------
_gguffer_imatrix_cubre_bloque() {
    local imatrix="$1" bloque="$2" _ggufcheck="$3"
    local n_im
    n_im="$(python3 "$_ggufcheck" "$imatrix" --nombres 2>/dev/null \
            | sed 's/\.in_sum2$//; s/\.counts$//' \
            | grep -c "^blk\.${bloque}\." || true)"
    [[ "${n_im:-0}" -gt 0 ]]
}

# -----------------------------------------------------------------------------
# _gguffer_bloque_nextn <gguf> <ggufcheck>
#
# El numero del bloque donde vive la cabeza MTP, o vacio si el GGUF no la
# trae. Es el bloque siguiente al ultimo: el conversor suma los bloques de la
# MTP al block_count (conversion/qwen.py: self.block_count += n_mtp).
# -----------------------------------------------------------------------------
# OJO: NO se busca por el nombre del tensor. Los tensores de la MTP se
# llaman como los de cualquier otro bloque ("blk.64.attn_k.weight"), SIN
# "nextn" en el nombre. Buscar "nextn." en los nombres no encuentra nada,
# y por eso el fix del MTP no se aplicaba y la cuantizacion moria con
# "Missing importance matrix for tensor blk.64.attn_k.weight".
#
# Lo que SÍ dice donde esta la MTP es la metadata del GGUF: el conversor
# suma los bloques de la MTP al block_count (conversion/qwen.py:
# self.block_count += n_mtp), y la MTP son los ULTIMOS bloques:
#
#     bloque_mtp = block_count - nextn_predict_layers
#
# Qwen3.8 denso: block_count=65, nextn=1 -> bloque 64.
# fable:         block_count=41, nextn=1 -> bloque 40.
# nemotron:      nextn=0 -> no hay MTP.
_gguffer_bloque_nextn() {
    local gguf="$1" _ggufcheck="$2"
    local bloque tensores
    bloque="$(python3 "$_ggufcheck" "$gguf" --meta 2>/dev/null \
               | sed -n 's/^bloque_mtp=//p' | head -1)"
    [[ -n "$bloque" && "$bloque" != "0" ]] || return 1
    # Que el bloque exista de verdad en el GGUF.
    tensores="$(python3 "$_ggufcheck" "$gguf" --bloque "$bloque" 2>/dev/null | head -1)"
    [[ -n "$tensores" ]] || return 1
    printf '%s' "$bloque"
    return 0
}

# -----------------------------------------------------------------------------
# ─────────────────────────────────────────────────────────────────────────────
# CUANTIZAR UN GGUF QUE YA EXISTE (sin pasar por el conversor)
#
# Por que: el conversor de pesos de HuggingFace a GGUF no siempre sabe
# convertir un modelo. El caso real es el Qwen3.8-27B, que es multimodal
# (Qwen3_5ForConditionalGeneration) y lleva la cabeza MTP. Los tensores de
# la MTP se llaman "mtp.*" y el conversor los remapea a "model.layers.64.*"
# (el bloque 64). En el fork que usa ropero (atomic-llama-cpp-turboquant) la
# clase Qwen3_5TextModel NO usa el mixin de la MTP, asi que esos 15 tensores
# no se pueden mapear y la conversion aborta:
#     ValueError: Can not map tensor 'mtp.layers.0.attn_k.weight'
# Y con --no-mtp tampoco: esa clase ignora el flag.
#
# La salida es no convertir: hay gente que ya publico el GGUF sin quantizar
# (bartowski publica el BF16), y ese GGUF ya tiene la MTP bien metida (como
# "blk.64.nextn.*"). Cuantizarlo da exactamente lo mismo que convertir y
# cuantizar, sin tocar el conversor.
#
# Y si el GGUF de Bartowski esta partido en shards ("-00001-of-00002"), se
# bajan todos y se unen con llama-gguf-split --merge.
# ─────────────────────────────────────────────────────────────────────────────

# _gguffer_ficheros_hf <owner/repo>
# Los ficheros de un repo de HuggingFace, uno por linea.
_gguffer_ficheros_hf() {
    local repo="$1"
    curl -s --max-time 90 "https://huggingface.co/api/models/$repo" 2>/dev/null | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(1)
for f in d.get("siblings", []):
    print(f["rfilename"])
'
}

# _gguffer_es_json <fichero>
#
# 0 si el fichero existe y contiene JSON válido, 1 si no.
#
# Por qué hace falta: un 404 de HuggingFace NO es un JSON válido, pero
# curl con "-o" puede dejar el cuerpo de la respuesta (que es el texto del
# error) en el fichero. Sin esta comprobación, un repo que NO existe se
# detectaría como "cuantizado" (porque su config.json "tiene" algo), y el
# gguffer avisaría de un problema que no tiene.
# -----------------------------------------------------------------------------
_gguffer_es_json() {
    local f="${1:-}"
    [[ -s "$f" ]] || return 1
    python3 - "$f" <<'PY' 2>/dev/null
import json, sys
try:
    with open(sys.argv[1], encoding="utf-8") as fh:
        json.load(fh)
except Exception:
    sys.exit(1)
sys.exit(0)
PY
}

# _gguffer_repo_ya_cuantizado <owner/repo>
#
# Devuelve 0 si el repo de pesos YA Viene cuantizado, 1 si no (o si no se
# pudo saber). Imprime el motivo por stderr.
#
# ── POR QUÉ ESTE CHEQUEO ─────────────────────────────────────────────────────
#
# Cuantizar un repo que YA viene cuantizado no es un error raro: es lo que
# pasa con todos los repos de NVIDIA ("...-NVFP4", "...-FP8") y con los
# checkpoints de modelopt. Vienen con la metadata "hf_quant_config.json" o
# con "quantization_config" en el config.json.
#
# El fallo es CARO, porque se descubre tarde: el gguffer baja 40-60 GB, los
# convierte, y solo al cuantizar el motor dice
#
#     llama_model_quantize: failed to quantize: requantizing from type
#     nvfp4 is disabled
#
# que no dice de donde sale el problema. Y la respuesta correcta (usar el
# repo BF16) se puede saber ANTES de bajar nada.
#
# OJO: esto NO es el mismo caso que "el GGUF ya esta cuantizado", que se
# comprueba con el file_type del GGUF (que se ve en la fase 3). Aqui se
# mira el REPO, antes de descargar: el repo es la fuente, y la fuente dice
# si sus pesos estan cuantizados.
# -----------------------------------------------------------------------------
_gguffer_repo_ya_cuantizado() {
    local repo="$1"
    [[ -n "$repo" && "$repo" == */* ]] || return 1

    local _tmp
    _tmp="$(mktemp "${TMPDIR:-/tmp}/gguffer-qcfg.XXXXXX")" || return 1

    # hf_quant_config.json: la metadata que ponen los checkpoints de
    # modelopt (NVIDIA y compañía). Su presencia ya dice "cuantizado".
    local _hqc=1
    if curl -fsS --max-time 60 -o "$_tmp" \
            "https://huggingface.co/$repo/raw/main/hf_quant_config.json" 2>/dev/null \
       && _gguffer_es_json "$_tmp"; then
        _hqc=0
    fi

    # quantization_config en el config.json: la otra forma (AWQ, GPTQ,
    # bnb, y el mxfp4 del gpt-oss).
    local _qcfg=1
    if [[ $_hqc -ne 0 ]] && curl -fsS --max-time 60 -o "$_tmp" \
            "https://huggingface.co/$repo/raw/main/config.json" 2>/dev/null \
       && _gguffer_es_json "$_tmp"; then
        if python3 - "$_tmp" <<'PY' 2>/dev/null
import json, sys
try:
    with open(sys.argv[1], encoding="utf-8") as f:
        d = json.load(f)
except Exception:
    sys.exit(1)
sys.exit(0 if d.get("quantization_config") else 1)
PY
        then
            _qcfg=0
        fi
    fi
    rm -f "$_tmp" 2>/dev/null || true

    if [[ $_hqc -eq 0 ]]; then
        echo "el repo declara hf_quant_config.json (checkpoint ya cuantizado)" >&2
        return 0
    fi
    if [[ $_qcfg -eq 0 ]]; then
        echo "el config.json declara quantization_config (checkpoint ya cuantizado)" >&2
        return 0
    fi
    return 1
}

# _gguffer_hermanos_sin_quantizar <owner/repo>
#
# Imprime, uno por linea, los repos del MISMO owner que parecen la version
# SIN cuantizar del modelo. Es la respuesta que hay que dar cuando el repo
# pedido ya viene cuantizado: "usa este otro".
#
# El criterio: mismo owner, y el nombre del repo pedido con el sufijo del
# quant quitado. "nvidia/...-NVFP4" -> "nvidia/...-BF16".
# -----------------------------------------------------------------------------
_gguffer_hermanos_sin_quantizar() {
    local repo="$1"
    local owner="${repo%%/*}" name="${repo##*/}"
    [[ -n "$owner" && -n "$name" ]] || return 0

    # Los sufijos de quant que aparecen en el nombre del repo. Si el repo no
    # termina en ninguno, no se puede adivinar el nombre del hermano: mejor
    # no decir nada que decir uno inventado.
    local base="" sufijos=(
        "-NVFP4" "-MXFP4" "-FP8" "-FP4" "-INT4" "-INT8" "-AWQ" "-GPTQ"
        "-W4A16" "-W4A8" "-W8A8" "-NF4" "-4BIT" "-8BIT" "-BF16" "-F16"
    )
    local s
    for s in ${sufijos[@]+"${sufijos[@]}"}; do
        if [[ "$name" == *"$s" ]]; then
            base="${name%"$s"}"
            break
        fi
    done
    [[ -n "$base" ]] || return 0

    # Los candidatos: el mismo nombre base, sin cuantizar.
    local -a candidos=("$base-BF16" "$base-F16" "$base")
    local c
    for c in ${candidos[@]+"${candidos[@]}"}; do
        local hf_repo="$owner/$c"
        # Se comprueba que EXISTE y que NO viene cuantizado: un hermano
        # que también está cuantizado no sirve.
        #
        # La existencia se mira por el codigo de salida de curl (-f), y NO
        # por "el fichero esta": un 404 tambien se puede escribir en el
        # fichero de salida, y daria un falso positivo.
        if curl -fsS --max-time 30 -o /dev/null \
                "https://huggingface.co/$hf_repo/raw/main/config.json" 2>/dev/null; then
            if ! _gguffer_repo_ya_cuantizado "$hf_repo"; then
                printf '%s\n' "$hf_repo"
            fi
        fi
    done
    return 0
}

# _gguffer_quants_publicados <owner/repo>
#
# Imprime, uno por linea, los quants que YA estan publicados para ese modelo
# base: "QUANT<TAB>repo". Es la respuesta a "que cuantizaciones soporta
# esto": no las que el motor soporta en abstracto, sino las que hay hechas
# de este modelo (que es lo que decide si vale la pena fabricar otra).
#
# Se busca por la API de HF con el filtro base_model:, que es como se
# declaran los repos derivados de un checkpoint, y luego se leen los
# nombres de los ficheros de cada repo (el quant va en el nombre).
# -----------------------------------------------------------------------------
_gguffer_quants_publicados() {
    local repo="$1"
    [[ -n "$repo" && "$repo" == */* ]] || return 1

    local -a candidatos=()
    candidatos=()
    while IFS= read -r _l; do
        [[ -n "$_l" ]] && candidatos+=("$_l")
    done < <(curl -s --max-time 90 \
        "https://huggingface.co/api/models?filter=base_model:$repo&limit=60" 2>/dev/null \
        | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(0)
if not isinstance(d, list):
    sys.exit(0)
for m in d:
    mid = m.get("id")
    if mid:
        print(mid)
' 2>/dev/null)

    ((${#candidatos[@]} > 0)) || return 1

    local c
    for c in ${candidatos[@]+"${candidatos[@]}"}; do
        # Solo los repos de GGUF: los demas no tienen quant que ofrecer.
        case "$c" in
            *-GGUF|*GGUF*) ;;
            *) continue ;;
        esac
        local q
        while IFS= read -r q; do
            [[ -n "$q" ]] && printf '%s\t%s\n' "$q" "$c"
        done < <(_gguffer_quants_de_repo "$c" 2>/dev/null)
    done
    return 0
}

# _gguffer_quants_de_repo <owner/repo>
#
# Los quants concretos publicados en un repo de GGUF, leidos de los nombres
# de los ficheros. Un repo de GGUF publica el quant en el nombre del
# fichero: "Qwen3.8-27B-UD-IQ3_XS.gguf" -> IQ3_XS.
# -----------------------------------------------------------------------------
_gguffer_quants_de_repo() {
    local repo="$1"
    local -a ficheros=()
    ficheros=()
    while IFS= read -r _l; do
        [[ -n "$_l" ]] && ficheros+=("$_l")
    done < <(_gguffer_ficheros_hf "$repo")
    ((${#ficheros[@]} > 0)) || return 1
    printf '%s\n' ${ficheros[@]+"${ficheros[@]}"} | python3 -c '
import re, sys
pat = re.compile(
    r"(?:^|[-_.])(IQ[1-4](?:_[A-Z]+)+|Q[1-6]_K(?:_[A-Z]+)*|Q[1-6]_0|Q[1-6]_1"
    r"|Q8_0|Q8_1|BF16|F16|F32|MXFP4(?:_MOE)?|NVFP4|TQ[12]_[0-9])(?=[-_.]|\.gguf$)")
vistos = []
for line in sys.stdin:
    line = line.strip()
    if not line.endswith(".gguf"):
        continue
    # El imatrix y el mmproj no son quants del modelo.
    if "imatrix" in line.lower() or "mmproj" in line.lower():
        continue
    for m in pat.finditer(line):
        q = m.group(1)
        if q not in vistos:
            vistos.append(q)
for q in sorted(vistos):
    print(q)
'
    return 0
}

# _gguffer_prepara_gguf <origen> <gguf_file> <dest_dir>
#
# Deja en <dest_dir> un GGUF UNICO y lo dice por stdout (la ruta). Si algo
# falla, el error va a stderr y el codigo de salida es != 0.
#
#   origen     un .gguf local, o "owner/repo" (con ":fichero" opcional)
#   gguf_file  el fichero dentro del repo (opcional)
_gguffer_prepara_gguf() {
    local origen="$1" gguf_file="$2" dest="$3"
    local repo="" fichero=""

    # ── caso A: un fichero local ────────────────────────────────────────────
    if [[ -f "$origen" ]]; then
        echo "$origen"
        return 0
    fi

    # ── caso B: un repo de HuggingFace ─────────────────────────────────────
    if [[ "$origen" == *:* ]]; then
        repo="${origen%%:*}"
        fichero="${origen#*:}"
    else
        repo="$origen"
    fi
    [[ "$repo" == */* ]] || { echo "ERROR: --gguf debe ser un fichero o owner/repo" >&2; return 2; }

    local -a ficheros=()
    ficheros=()
    while IFS= read -r _l; do
        [[ -n "$_l" ]] && ficheros+=("$_l")
    done < <(_gguffer_ficheros_hf "$repo")
    ((${#ficheros[@]} > 0)) || {
        echo "ERROR: no se pudo leer el repo $repo de HuggingFace" >&2
        return 1
    }

    # Que GGUF del repo: el que se pidio, o el unico que serve (el grande,
    # sin quantizar, sin imatrix y sin mmproj).
    if [[ -z "$fichero" ]]; then
        local -a candidatos=()
        local -a shard_grupos=()
        local f
        for f in ${ficheros[@]+"${ficheros[@]}"}; do
            [[ "$f" == *.gguf ]] || continue
            [[ "$f" == *imatrix* ]] && continue
            [[ "$f" == *mmproj* ]] && continue
            if [[ "$f" == *"-0000"* ]]; then
                # Un shard. El grupo es el prefijo ("X-00001-of-00002"): de
                # cada grupo se ofrece el PRIMER shard, y el gguffer baja el
                # grupo entero y lo une.
                local _pre="${f%.gguf}"
                _pre="${_pre%-0000[0-9]-of-[0-9]*}"
                local _dup=0 _g
                for _g in ${shard_grupos[@]+"${shard_grupos[@]}"}; do [[ "$_g" == "$_pre" ]] && _dup=1; done
                (( _dup )) || shard_grupos+=("$_pre")
            else
                candidatos+=("$f")
            fi
        done
        # Los shards tambien son candidatos: el BF16 del Qwen3.8 va partido
        # en dos ficheros, en una subcarpeta. De cada grupo se ofrece el
        # PRIMER shard, y el gguffer baja el grupo entero y lo une.
        for f in ${ficheros[@]+"${ficheros[@]}"}; do
            [[ "$f" == *"-00001-of-"* ]] || continue
            [[ "$f" == *imatrix* ]] && continue
            [[ "$f" == *mmproj* ]] && continue
            candidatos+=("$f")
        done
        if ((${#candidatos[@]} == 0)); then
            echo "ERROR: el repo $repo no tiene un GGUF sin quantizar." >&2
            echo "       Dile cual con --gguf-file." >&2
            return 2
        fi
        if ((${#candidatos[@]} > 1)); then
            # Hay varios: el que interesa es el SIN cuantizar (bf16/f16).
            # Los demas ya son quants de otra gente, y cuantizar encima de
            # un quant es dequantizar y re-cuantizar.
            local -a sin_quantizar=()
            for f in ${candidatos[@]+"${candidatos[@]}"}; do
                case "$(basename "$f")" in
                    *bf16*|*BF16*|*f16*|*F16*) sin_quantizar+=("$f") ;;
                esac
            done
            if ((${#sin_quantizar[@]} == 1)); then
                candidatos=("${sin_quantizar[0]}")
            elif ((${#sin_quantizar[@]} == 0)); then
                echo "ERROR: el repo $repo no tiene ningun GGUF sin quantizar" >&2
                echo "       (lo que hay son quants: cuantizar encima no aporta nada)" >&2
                echo "       Dile cual con --gguf-file." >&2
                return 2
            else
                echo "ERROR: el repo $repo tiene varios GGUFs sin quantizar:" >&2
                printf '       %s\n' ${sin_quantizar[@]+"${sin_quantizar[@]}"} >&2
                echo "       Dile cual con --gguf-file." >&2
                return 2
            fi
        fi
        fichero="${candidatos[0]}"
    fi

    # ── los shards ─────────────────────────────────────────────────────────
    # "X-00001-of-00002.gguf" son trozos del mismo GGUF: hay que bajarlos
    # todos y unirlos (llama-gguf-split --merge).
    local -a shards=()
    local base="${fichero%.gguf}"
    if [[ "$base" =~ ^(.*)-0000[0-9]+-of-[0-9]+$ ]]; then
        local pre="${BASH_REMATCH[1]}"
        local total="${base##*-of-}"
        local i
        for ((i=1; i<=total; i++)); do
            local n; n="$(printf "%05d" "$i")"
            local cand="$pre-$n-of-$total.gguf"
            local ok=0
            for f in ${ficheros[@]+"${ficheros[@]}"}; do [[ "$f" == "$cand" ]] && ok=1; done
            (( ok )) && shards+=("$cand")
        done
        ((${#shards[@]} == total)) || {
            echo "ERROR: el repo $repo no tiene todos los shards de $fichero" >&2
            echo "       encontrados: ${#shards[@]} de $total" >&2
            return 1
        }
    else
        shards=("$fichero")
    fi

    # ── la descarga ────────────────────────────────────────────────────────
    mkdir -p "$dest" || return 1
    command -v hf >/dev/null 2>&1 || {
        echo "ERROR: falta 'hf' (huggingface_hub) para bajar el GGUF" >&2
        return 1
    }
    echo "[gguffer] bajo ${#shards[@]} fichero(s) de $repo..." >&2
    local -a hf_args=()
    local s
    for s in ${shards[@]+"${shards[@]}"}; do hf_args+=("$s"); done
    if ! hf download "$repo" ${hf_args[@]+"${hf_args[@]}"} --local-dir "$dest" >&2; then
        echo "ERROR: no se pudo bajar de $repo: ${shards[*]+"${shards[*]}"}" >&2
        return 1
    fi

    # ── unir los shards ────────────────────────────────────────────────────
    if ((${#shards[@]} > 1)); then
        local _split_bin="${LLAMA_DIR:-$HOME/llama.cpp}/build/bin/llama-gguf-split"
        if [[ ! -x "$_split_bin" ]]; then
            echo "ERROR: falta llama-gguf-split para unir los shards ($_split_bin)" >&2
            echo "       compila llama.cpp, o baja el GGUF sin partir." >&2
            return 1
        fi
        local merged="$dest/${repo##*/}-merged.gguf"
        echo "[gguffer] uno los ${#shards[@]} shards en $merged..." >&2
        if ! "$_split_bin" --merge "$dest/${shards[0]}" "$merged" >&2; then
            echo "ERROR: no se pudieron unir los shards" >&2
            return 1
        fi
        echo "$merged"
        return 0
    fi

    echo "$dest/$fichero"
    return 0
}

# _gguffer_bloques_sin_imatrix <gguf> <imatrix> <ggufcheck>
#
# Que bloques del GGUF NO aparecen en el imatrix. La respuesta es exacta y
# no hay que adivinar nada: el imatrix se indexa por nombre de tensor, y un
# bloque entero aparece en el o no aparece.
#
# Por que importa: el imatrix se calcula sobre el modelo base (los bloques
# 0..63 del Qwen3.8) y el GGUF lleva la cabeza MTP como un bloque MAS. Ese
# bloque no esta en el imatrix, y con un quant very-low-bit el cuantizador
# aborta:
#     "Missing importance matrix for tensor blk.64.attn_k.weight"
# La salida es dejar ese bloque en un quant que NO exija imatrix (Q4_0), que
# es lo que hizo bartowski con su IQ3_XS del Qwen3.8.
#
# OJO: no se comparan TENSOR a tensor, sino BLOQUE a bloque. Las normas
# (input_layernorm, ssm_alpha...) no estan en ningun imatrix porque no se
# cuantizan, asi que un bloque "incompleto" no significa nada. Lo que
# significa algo es un bloque que no aparece ENTERO.
_gguffer_bloques_sin_imatrix() {
    local gguf="$1" imatrix="$2" ggufcheck="$3"
    local -A bloques_gguf=() bloques_imx=()
    local n b

    while IFS= read -r n; do
        [[ "$n" == blk.*.* ]] || continue
        b="${n#blk.}"; b="${b%%.*}"
        [[ "$b" =~ ^[0-9]+$ ]] || continue
        bloques_gguf["$b"]=1
    done < <("$ggufcheck" "$gguf" --nombres 2>/dev/null)

    while IFS= read -r n; do
        # el imatrix guarda "<tensor>.in_sum2" y "<tensor>.counts"
        n="${n%.in_sum2}"; n="${n%.counts}"
        [[ "$n" == blk.*.* ]] || continue
        b="${n#blk.}"; b="${b%%.*}"
        [[ "$b" =~ ^[0-9]+$ ]] || continue
        bloques_imx["$b"]=1
    done < <("$ggufcheck" "$imatrix" --nombres 2>/dev/null)

    local -a faltan=()
    for b in "${!bloques_gguf[@]}"; do
        [[ -n "${bloques_imx[$b]:-}" ]] || faltan+=("$b")
    done
    ((${#faltan[@]} > 0)) || return 0
    printf '%s\n' ${faltan[@]+"${faltan[@]}"} | sort -n
    return 0
}

# _gguffer_escribe_fuente <gguf> <nombre> <repo_base> <imatrix> <imatrix_repo>
#                         <imatrix_file> <bloque_mtp> <intermedio>
#
# Escribe <gguf>.fuente.json al lado del GGUF: el registro de COMO se hizo
# esa cuantizacion.
#
# Sin esto, un GGUF es un fichero opaco: no se sabe de que modelo salio, con
# que importance matrix, ni si la cabeza MTP quedo en un quant aparte. Y eso
# es justo lo que hay que contar en el model card de HuggingFace, para que
# quien lo baja sepa lo que tiene entre manos.
#
# El fichero va en JSON plano (python3 lo lee sin dependencias) y lo escribe
# el shell: los valores son repo, quant, nombres de fichero y numeros, y no
# llevan comillas ni saltos de linea. Por si acaso, se escapan.
# -----------------------------------------------------------------------------
_gguffer_esc() {
    local s="${1:-}"
    s="${s//\\/\\\\}"
    s="${s//\"/\\\"}"
    printf '%s' "$s"
}

_gguffer_escribe_fuente() {
    local gguf="$1" nombre="$2" repo_base="$3" imatrix="$4"
    local imatrix_repo="$5" imatrix_file="$6" bloque_mtp="$7" intermedio="$8"
    local _ggufcheck
    _ggufcheck="$ROPERO_ROOT/bin/python/ggufcheck.py"

    local dest="${gguf}.fuente.json"
    local meta="" base_model="" base_name="" quant="" arch="" block_count=""
    if [[ -f "$gguf" ]]; then
        meta="$(python3 "$_ggufcheck" "$gguf" --meta 2>/dev/null || true)"
        base_model="$(printf '%s\n' "$meta" | sed -n 's/^base_model=//p' | head -1)"
        base_name="$(printf '%s\n' "$meta" | sed -n 's/^base_model_name=//p' | head -1)"
        quant="$(printf '%s\n' "$meta" | sed -n 's/^file_type=//p' | head -1)"
        arch="$(printf '%s\n' "$meta" | sed -n 's/^arch=//p' | head -1)"
        block_count="$(printf '%s\n' "$meta" | sed -n 's/^block_count=//p' | head -1)"
    fi
    # El nombre del quant tambien sale del nombre del fichero: "x.Q2_K.gguf".
    [[ -n "$quant" ]] || quant="${nombre##*.}"

    local imx_sha=""
    [[ -n "$imatrix" && -f "$imatrix" ]] && imx_sha="$(sha256sum "$imatrix" 2>/dev/null | cut -d' ' -f1)"

    local mtp_json="null"
    if [[ -n "$bloque_mtp" ]]; then
        mtp_json="{\"bloque\": ${bloque_mtp}, \"quant\": \"${MTP_QUANT:-Q4_0}\", \"nota\": \"la cabeza MTP no la cubre el imatrix (se calcula sobre el modelo base), asi que queda en este quant\"}"
    fi

    local imx_json="null"
    if [[ -n "$imatrix" ]]; then
        imx_json="{\"fichero\": \"$(basename "$imatrix")\""
        [[ -n "$imatrix_repo" ]] && imx_json+=", \"repo\": \"$(_gguffer_esc "$imatrix_repo")\""
        [[ -n "$imx_sha" ]] && imx_json+=", \"sha256\": \"$imx_sha\""
        imx_json+="}"
    fi

    {
        printf '{\n'
        printf '  "gguf": "%s",\n' "$(_gguffer_esc "$nombre.gguf")"
        printf '  "quant": "%s",\n' "$(_gguffer_esc "$quant")"
        printf '  "fuente_repo": "%s",\n' "$(_gguffer_esc "$repo_base")"
        printf '  "base_model": "%s",\n' "$(_gguffer_esc "$base_model")"
        printf '  "base_model_name": "%s",\n' "$(_gguffer_esc "$base_name")"
        printf '  "arch": "%s",\n' "$(_gguffer_esc "$arch")"
        printf '  "block_count": "%s",\n' "$(_gguffer_esc "$block_count")"
        printf '  "intermedio": "%s",\n' "$(_gguffer_esc "$intermedio")"
        printf '  "imatrix": %s,\n' "$imx_json"
        printf '  "mtp": %s,\n' "$mtp_json"
        printf '  "herramienta": "ropero gguffer (llama.cpp llama-quantize)",\n'
        printf '  "fecha": "%s"\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        printf '}\n'
    } > "$dest" 2>/dev/null || { rm -f "$dest"; return 1; }

    echo "[gguffer] procedencia: $dest"
    return 0
}

# -----------------------------------------------------------------------------
# _gguffer_meter_chat_template <tokenizer_config.json> <chat_template.jinja>
#
# Mete el contenido del .jinja dentro del tokenizer_config.json, que es de
# donde el conversor de llama.cpp lee el chat template. Si el JSON ya trae
# "chat_template", no se toca (el del repo manda).
# -----------------------------------------------------------------------------
_gguffer_meter_chat_template() {
    local cfg="$1" tpl="$2"
    [[ -f "$cfg" && -f "$tpl" ]] || return 1
    python3 - "$cfg" "$tpl" <<'PY' || return 1
import json, sys
cfg, tpl = sys.argv[1], sys.argv[2]
try:
    with open(cfg, encoding="utf-8") as f:
        d = json.load(f)
except Exception:
    sys.exit(1)
if d.get("chat_template"):
    sys.exit(0)   # ya tiene uno: no se toca
try:
    with open(tpl, encoding="utf-8") as f:
        d["chat_template"] = f.read()
except Exception:
    sys.exit(1)
with open(cfg, "w", encoding="utf-8") as f:
    json.dump(d, f, indent=2, ensure_ascii=False)
PY
}

# -----------------------------------------------------------------------------
# _gguffer_arreglar_tokenizer <dir>
#
# Parchea el tokenizer_config.json de un clon de pesos para que lo entienda
# un transformers v4.
#
# Por que: hay repos guardados con transformers v5, cuyo tokenizer_class es
# "TokenizersBackend". La v4 no tiene esa clase y la conversion del GGUF
# muere con "ValueError: Tokenizer class TokenizersBackend does not exist or
# is not currently imported". El tokenizer.json es un fast tokenizer estandar,
# asi que con poner "PreTrainedTokenizerFast" (que si conoce la v4) vale.
#
# Se toca el CLON (una copia local), no el repo de HuggingFace, y se guarda
# un .bak por si acaso.
# -----------------------------------------------------------------------------
_gguffer_arreglar_tokenizer() {
    # OJO: "local dir="$1" cfg="$dir/.."" NO vale: en bash las expansiones de
    # una misma declaracion local se hacen ANTES de asignar, y $dir sale
    # vacio. Por eso van en dos lineas.
    local dir="$1"
    local cfg="$dir/tokenizer_config.json"
    # El chat template que se pasa por --chat-template (ROPERO_CHAT_TEMPLATE).
    # La funcion lo recibe por la variable de entorno porque su firma la
    # comparten varios sitios y no quiero anadir otro parametro posicional
    # que se pueda confundir con el directorio.
    local ROPERO_CHAT_TEMPLATE="${ROPERO_CHAT_TEMPLATE:-}"
    [[ -f "$cfg" ]] || return 1

    # ── EL CHAT TEMPLATE ───────────────────────────────────────────────────
    # El conversor de llama.cpp lee el chat template del tokenizer.json o del
    # tokenizer_config.json. NUNCA del "chat_template.jinja" del repo.
    #
    # Y hay repos que lo publican SOLO en ese fichero (NVIDIA lo hace con el
    # Nemotron 3.5). Entonces el GGUF se queda con el template de la
    # comunidad de llama.cpp, que puede ser de OTRO modelo: para el Nemotron
    # 3.5 hay un "NVIDIA-Nemotron-3-Nano-30B-A3B-BF16.jinja" (el del NANO, no
    # el del 3.5 Lightning), y con ese template el modelo responde en HTML y
    # no razona bien.
    #
    # Se arregla metiendo el chat_template.jinja dentro del
    # tokenizer_config.json, que es de donde el conversor lo lee.
    # El chat template puede venir de tres sitios, por orden:
    #   1. el que se pasa con --chat-template (lo declara el TOML)
    #   2. el chat_template.jinja del clon (lo publica el repo de pesos)
    #   3. el tokenizer_config.json del clon (si ya trae "chat_template")
    if [[ -n "$ROPERO_CHAT_TEMPLATE" && -f "$ROPERO_CHAT_TEMPLATE" ]]; then
        _gguffer_meter_chat_template "$cfg" "$ROPERO_CHAT_TEMPLATE"
        if grep -q '"chat_template"' "$cfg" 2>/dev/null; then
            echo "[gguffer] chat template: el que declaraste ($(basename "$ROPERO_CHAT_TEMPLATE"))"
        fi
    elif [[ -f "$dir/chat_template.jinja" ]]; then
        _gguffer_meter_chat_template "$cfg" "$dir/chat_template.jinja"
        if grep -q '"chat_template"' "$cfg" 2>/dev/null; then
            echo "[gguffer] el repo trae chat_template.jinja y el conversor no lo lee:"
            echo "           lo meto en el tokenizer_config.json para que lo coja."
        fi
    elif [[ -f "$dir/../chat_template.jinja" ]]; then
        # con --from-subdir, el source_dir es un subdirectorio del clon
        _gguffer_meter_chat_template "$cfg" "$dir/../chat_template.jinja"
        if grep -q '"chat_template"' "$cfg" 2>/dev/null; then
            echo "[gguffer] el repo trae chat_template.jinja y el conversor no lo lee:"
            echo "           lo meto en el tokenizer_config.json para que lo coja."
        fi
    fi

    grep -q '"tokenizer_class"' "$cfg" || return 1
    [[ -f "$cfg.bak" ]] || cp "$cfg" "$cfg.bak"
    local _res
    _res="$(python3 - "$cfg" <<'PY'
import json, os, sys
ruta = sys.argv[1]
try:
    with open(ruta, encoding="utf-8") as f:
        d = json.load(f)
except Exception:
    sys.exit(1)
viejo = d.get("tokenizer_class")
if viejo == "TokenizersBackend":
    d["tokenizer_class"] = "PreTrainedTokenizerFast"
    with open(ruta, "w", encoding="utf-8") as f:
        json.dump(d, f, indent=2, ensure_ascii=False)
    print("CAMBIADO")
    sys.exit(0)
# Otra clase desconocida: si el repo trae tokenizer.json, el fast generico
# tambien vale.
if os.path.exists(os.path.join(os.path.dirname(ruta), "tokenizer.json")):
    d["tokenizer_class"] = "PreTrainedTokenizerFast"
    with open(ruta, "w", encoding="utf-8") as f:
        json.dump(d, f, indent=2, ensure_ascii=False)
    print("CAMBIADO")
    sys.exit(0)
print("NO")
sys.exit(1)
PY
)"
    if [[ "$_res" == "CAMBIADO" ]]; then
        echo "[gguffer] el tokenizer del repo usa una clase que tu transformers no conoce."
        echo "           (TokenizersBackend es de transformers v5)"
        echo "           Lo cambio a PreTrainedTokenizerFast."
    fi
    return 0
}

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------
# -----------------------------------------------------------------------------
# _gguffer_espacio <dir>
#
# Bytes libres en el sistema de ficheros que contiene <dir>. Se mide con df
# sobre el DIRECTORIO (no sobre la raiz): si el trabajo esta en otro disco,
# el espacio es el de ese disco.
# -----------------------------------------------------------------------------
_gguffer_espacio() {
    local dir="$1"
    df -PB1 "$dir" 2>/dev/null | awk 'NR==2 {print $4}'
}

# -----------------------------------------------------------------------------
# _gguffer_necesita_espacio <work> <out_f16>
#
# Cuanto hace falta para convertir, con el tamano REAL de los pesos.
#
# El GGUF intermedio es aproximadamente del tamano de los safetensors: si
# los pesos son BF16 o F16, el GGUF es igual de grande; si son F32, es la
# mitad. Sin mirar los pesos no se puede afinar, asi que se toma el peor
# caso (el GGUF tan grande como los pesos) y se le anade un margen.
#
# Se anade el margen porque el GGUF lleva la metadata y los tensores con
# padding: un 5 % de mas nunca viene mal.
# -----------------------------------------------------------------------------
_gguffer_necesita_espacio() {
    local work="$1" out_f16="$2" bytes=0 tam=0
    if [[ -d "$work" ]]; then
        tam="$(du -sb "$work" 2>/dev/null | cut -f1)"
        bytes="${tam:-0}"
    fi
    # el destino puede estar en otro disco
    local dest_dir
    dest_dir="$(dirname "$out_f16")"
    local libre
    libre="$(_gguffer_espacio "$dest_dir")"
    [[ -n "$libre" ]] || libre=0
    # el margen: 5 % de los pesos, y 512 MiB de suelo (la metadata, el log)
    local margen=$(( bytes / 20 + 536870912 ))
    printf '%s %s' "$libre" "$margen"
}

# -----------------------------------------------------------------------------
# _gguffer_diagnostico <modelo_dir> <out_f16> <imatrix> <imatrix_repo>
#                      <imatrix_file> <cuant>
#
# El informe de por que NO se esta cuantizando, SIN cuantizar nada.
#
# Esto es lo que hace falta cuando la cuantizacion falla a las tres de la
# madrugada: en vez de adivinar, el gguffer dice cuanto espacio hay, cuanto
# hace falta, cual es el bloque MTP, si el imatrix lo cubre, y el argv
# EXACTO que le pasaria al cuantizador.
# -----------------------------------------------------------------------------
_gguffer_diagnostico() {
    local dir="$1" out_f16="$2" imatrix="$3" imatrix_repo="$4" imatrix_file="$5" quant="$6" origen_gguf="${7:-}"
    local chat_template="" tokenizer_extra=""
    local _ggufcheck
    _ggufcheck="$ROPERO_ROOT/bin/python/ggufcheck.py"
    local _libre _margen _pesos _nextn_bloque _cubre
    _libre="$(_gguffer_espacio "$dir")"
    _pesos="$(du -sb "$dir" 2>/dev/null | cut -f1)"
    _margen=$(( ${_pesos:-0} / 20 + 536870912 ))

    echo "── DIAGNOSTICO ──────────────────────────────────────────────"
    if [[ -n "$origen_gguf" ]]; then
        # No hay pesos clonados: el GGUF ya venia hecho. Lo que hay que
        # mirar es que el fichero esta y cuanto ocupa.
        echo "  GGUF de entrada:  $origen_gguf"
        echo "  tamano:           $(numfmt --to=iec-i --suffix=B "$(stat -c %s "$origen_gguf" 2>/dev/null || echo 0)" 2>/dev/null || echo "?")"
        echo "  (no hay pesos que clonar: no se convierte nada)"
    else
        echo "  pesos clonados:   $dir"
        echo "  tamano:           $(numfmt --to=iec-i --suffix=B "${_pesos:-0}" 2>/dev/null || echo "${_pesos:-0} B")"
    fi
    echo "  espacio libre:    $(numfmt --to=iec-i --suffix=B "${_libre:-0}" 2>/dev/null || echo "${_libre:-0} B")"
    echo "  espacio a porner: $(numfmt --to=iec-i --suffix=B "$_margen" 2>/dev/null || echo "$_margen B")  (GGUF intermedio + margen)"
    if [[ -n "${_libre:-}" && "${_libre:-0}" -lt "$_margen" ]]; then
        echo "  ==> NO CABE. Borra el clon o usa otro disco: GGUF_WORK_DIR / GGUF_DIR."
    fi
    echo ""
    echo "  GGUF intermedio:  $out_f16"
    if [[ -f "$out_f16" ]]; then
        echo "    ya existe (se reutiliza)"
        _nextn_bloque="$(_gguffer_bloque_nextn "$out_f16" "$_ggufcheck" 2>/dev/null || true)"
        echo "    bloque MTP:      ${_nextn_bloque:-ninguno}"
        echo "    file_type:       $(python3 "$_ggufcheck" "$out_f16" --tipo-predominante 2>/dev/null | head -1)"
    else
        echo "    NO existe: hay que convertir antes"
        _nextn_bloque=""
    fi
    echo ""
    echo "  imatrix:          ${imatrix:-NINGUNO}"
    if [[ -n "$imatrix" ]]; then
        [[ -f "$imatrix" ]] && echo "    fichero: existe ($(numfmt --to=iec-i --suffix=B "$(stat -c %s "$imatrix" 2>/dev/null)" 2>/dev/null))" \
                           || echo "    fichero: NO EXISTE"
        [[ -n "$imatrix_repo" ]] && echo "    repo:     $imatrix_repo"
        [[ -n "$imatrix_file" ]] && echo "    fichero:  $imatrix_file"
        if [[ -n "$_nextn_bloque" ]]; then
            _cubre="no"
            _gguffer_imatrix_cubre_bloque "$imatrix" "$_nextn_bloque" "$_ggufcheck" 2>/dev/null && _cubre="si"
            echo "    cubre el bloque MTP ($_nextn_bloque): $_cubre"
            if [[ "$_cubre" == "no" ]]; then
                echo "    => se anadira --tensor-type "blk\.$_nextn_bloque\..*=${MTP_QUANT:-Q4_0}""
            fi
        fi
    fi
    echo ""
    echo "  quant pedido:     ${quant:-?}"
    echo "  argv del cuantizador (lo que se ejecutaria):"
    local _args=(--imatrix "$imatrix")
    [[ -n "$_nextn_bloque" ]] && ! _gguffer_imatrix_cubre_bloque "$imatrix" "$_nextn_bloque" "$_ggufcheck" 2>/dev/null \
        && _args+=(--tensor-type "blk\.${_nextn_bloque}\..*=${MTP_QUANT:-Q4_0}")
    echo "    llama-quantize ${_args[*]} <f16> <salida> $quant"
    echo "─────────────────────────────────────────────────────────────"
}

rp_cmd_gguffer() {
    local path="" repo="" out_name="" imatrix="" from_subdir="" include=""
    local gguf_origen="" gguf_file=""
    local -a quants=()
    local imatrix_repo="" imatrix_file=""
    local force=0 keep_source=0 keep_inter=0 list_only=0 no_mtp=0 diagnostico=0
    local inter="f16"
    ROPERO_GGUF_OUT_DIR=""; ROPERO_GGUF_OUT_FILE=""; ROPERO_GGUF_OUT_PRINCIPAL=""
    ROPERO_GGUF_OUT_PRINCIPAL_DIR=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --gguf)        [[ $# -ge 2 ]] || { echo "ERROR: --gguf requiere un fichero o un repo" >&2; return 2; }; gguf_origen="$2"; shift 2 ;;
            --gguf=*)      gguf_origen="${1#--gguf=}"; shift ;;
            --gguf-file)   [[ $# -ge 2 ]] || { echo "ERROR: --gguf-file requiere un nombre" >&2; return 2; }; gguf_file="$2"; shift 2 ;;
            --gguf-file=*) gguf_file="${1#--gguf-file=}"; shift ;;
            --path)        [[ $# -ge 2 ]] || { echo "ERROR: --path requiere un directorio" >&2; return 2; }; path="$2"; shift 2 ;;
            --path=*)      path="${1#--path=}"; shift ;;
            --repo)        [[ $# -ge 2 ]] || { echo "ERROR: --repo requiere un repo (owner/name)" >&2; return 2; }; repo="$2"; shift 2 ;;
            --repo=*)      repo="${1#--repo=}"; shift ;;
            --quant)
                if [[ $# -eq 1 ]]; then list_only=1; shift
                else
                    # Admite varios: "--quant IQ2_M --quant IQ3_XS" o
                    # "--quant IQ2_M,IQ3_XS". Lo caro es bajar y convertir los
                    # pesos (40-60 GB, una hora); cuantizar es solo CPU, asi
                    # que varios quants en una tirada reutilizan el F16.
                    IFS=',' read -r -a _qs <<< "$2"
                    for _q in ${_qs[@]+"${_qs[@]}"}; do
                        [[ -n "$_q" ]] && quants+=("$_q")
                    done
                    shift 2; fi ;;
            --quant=*)
                IFS=',' read -r -a _qs <<< "${1#--quant=}"
                for _q in ${_qs[@]+"${_qs[@]}"}; do
                    [[ -n "$_q" ]] && quants+=("$_q")
                done
                shift ;;
            --imatrix)     [[ $# -ge 2 ]] || { echo "ERROR: --imatrix requiere un fichero" >&2; return 2; }; imatrix="$2"; shift 2 ;;
            --chat-template) [[ $# -ge 2 ]] || { echo "ERROR: --chat-template requiere un fichero" >&2; return 2; }; chat_template="$2"; shift 2 ;;
            --tokenizer)   [[ $# -ge 2 ]] || { echo "ERROR: --tokenizer requiere un fichero" >&2; return 2; }; tokenizer_extra="$2"; shift 2 ;;
            --imatrix=*)   imatrix="${1#--imatrix=}"; shift ;;
            --imatrix-repo)  [[ $# -ge 2 ]] || { echo "ERROR: --imatrix-repo requiere un repo (owner/name)" >&2; return 2; }; imatrix_repo="$2"; shift 2 ;;
            --imatrix-repo=*) imatrix_repo="${1#--imatrix-repo=}"; shift ;;
            --imatrix-file)  [[ $# -ge 2 ]] || { echo "ERROR: --imatrix-file requiere un fichero" >&2; return 2; }; imatrix_file="$2"; shift 2 ;;
            --imatrix-file=*) imatrix_file="${1#--imatrix-file=}"; shift ;;
            --out)         [[ $# -ge 2 ]] || { echo "ERROR: --out requiere un nombre o una ruta" >&2; return 2; }; _gguffer_parse_out "$2" || return 2; shift 2 ;;
            --out=*)       _gguffer_parse_out "${1#--out=}" || return 2; shift ;;
            --out-principal)  [[ $# -ge 2 ]] || { echo "ERROR: --out-principal requiere una ruta" >&2; return 2; }; _gguffer_parse_out_principal "$2" || return 2; shift 2 ;;
            --out-principal=*) _gguffer_parse_out_principal "${1#--out-principal=}" || return 2; shift ;;
            --from-subdir) [[ $# -ge 2 ]] || { echo "ERROR: --from-subdir requiere un directorio" >&2; return 2; }; from_subdir="$2"; shift 2 ;;
            --from-subdir=*) from_subdir="${1#--from-subdir=}"; shift ;;
            --include)     [[ $# -ge 2 ]] || { echo "ERROR: --include requiere un patrón" >&2; return 2; }; include="$2"; shift 2 ;;
            --include=*)   include="${1#--include=}"; shift ;;
            --intermedio)    [[ $# -ge 2 ]] || { echo "ERROR: --intermedio requiere f16 o bf16" >&2; return 2; }; inter="$2"; shift 2 ;;
            --intermedio=*)  inter="${1#--intermedio=}"; shift ;;
            --keep-source)    keep_source=1; shift ;;
            --keep-intermedio) keep_inter=1; shift ;;
            --force)       force=1; shift ;;
            --list-quants) list_only=1; shift ;;
            --no-mtp)      no_mtp=1; shift ;;
            --diagnostico)  diagnostico=1; shift ;;
            -h|--help)     _rp_gguffer_help; return 0 ;;
            *) echo "ERROR: argumento desconocido: $1" >&2; return 2 ;;
        esac
    done

    # ── ¿de donde sale el motor? ────────────────────────────────────────────
    local _llama_dir="${LLAMA_DIR:-$HOME/llama.cpp}"
    local _convert="$_llama_dir/convert_hf_to_gguf.py"
    local _quantize
    _quantize="$(command -v llama-quantize 2>/dev/null || echo "$_llama_dir/build/bin/llama-quantize")"
    local _ggufcheck
    _ggufcheck="$ROPERO_ROOT/bin/python/ggufcheck.py"
    local _libggml
    for c in "$_llama_dir/build/bin/libggml.so" "$_llama_dir/build/bin/libggml.dylib" "$_llama_dir/build/lib/libggml.so"; do
        [[ -f "$c" ]] && { _libggml="$c"; break; }
    done
    local _gguf_split
    for c in "$(command -v llama-gguf-split 2>/dev/null || true)" "$_llama_dir/build/bin/llama-gguf-split"; do
        [[ -n "$c" && -x "$c" ]] && { _gguf_split="$c"; break; }
    done

    if (( list_only == 1 )); then
        # ── Los quants del MOTOR ────────────────────────────────────────────
        echo "Quants disponibles (motor: $_quantize):"
        "$_quantize" --help 2>&1 | grep -E "or  [A-Z0-9]" | sed 's/^/  /'
        # ── Y los de ESTE modelo, si se sabe de donde viene ───────────────
        #
        # Los quants que el motor soporta son una cosa, y los que hay hechas
        # de este modelo otra. Lo que decide si vale la pena fabricar es la
        # segunda: un quant que ya esta publicado se puede bajar, y uno que no
        # es el hueco que justifica una noche de cuantizacion.
        local _modelo_para_listar="$repo"
        [[ -n "$_modelo_para_listar" ]] || _modelo_para_listar="${gguf_origen%%:*}"
        if [[ -n "$_modelo_para_listar" && "$_modelo_para_listar" == */* ]]; then
            echo ""
            echo "Quants ya publicados para $_modelo_para_listar:"
            local _q _repo _n=0
            while IFS=$'\t' read -r _q _repo; do
                [[ -n "$_q" ]] || continue
                printf '  %-12s %s\n' "$_q" "$_repo"
                _n=$((_n+1))
            done < <(_gguffer_quants_publicados "$_modelo_para_listar" 2>/dev/null | sort -u)
            (( _n > 0 )) || echo "  (ninguno encontrado)"
        fi
        return 0
    fi

    if [[ -z "$path" && -z "$repo" && -z "$gguf_origen" ]]; then
        echo "USO: ropero gguffer --path <dir> | --repo <owner/name> | --gguf <fichero|owner/repo> [--quant TIPO]" >&2
        return 2
    fi
    if [[ -n "$path" && -n "$repo" ]]; then
        echo "ERROR: usa --path O --repo, no ambos" >&2
        return 2
    fi
    if [[ -n "$repo" && "$repo" != */* ]]; then
        echo "ERROR: --repo debe ser owner/name (no '$repo')" >&2
        return 2
    fi

    # ── los quants del motor: numero y nombre ───────────────────────────────
    # El numero es el GGML_TYPE, que es lo que lleva el GGUF en
    # general.file_type: con eso se puede saber si un GGUF ya esta en el
    # quant pedido (y saltarse la cuantizacion).
    local quants_txt quants_num
    quants_txt="$("$_quantize" --help 2>&1 | grep -E "or  [A-Z0-9]" || true)"
    if [[ -z "$quants_txt" ]]; then
        echo "ERROR: no se puede leer la lista de quants de $_quantize" >&2
        return 1
    fi
    # "    19  or  IQ2_XXS : ..."  ->  "19=IQ2_XXS"
    quants_num="$(printf '%s\n' "$quants_txt" | sed -nE 's/^[[:space:]]*([0-9]+)[[:space:]]+or[[:space:]]+([A-Z0-9_]+).*/\1=\2/p' | tr '\n' ' ')"

    # ── validar el quant ANTES de descargar 60 GB ───────────────────────────
    local -a qnums=()
    if (( ${#quants[@]} > 0 )); then
        local q qnum
        for q in ${quants[@]+"${quants[@]}"}; do
            qnum="$(printf '%s' "$quants_num" | tr ' ' '\n' | grep -E "=${q}$" | head -1 | cut -d= -f1)"
            if [[ -z "$qnum" ]]; then
                echo "ERROR: quant no valido: $q" >&2
                echo "Usa 'ropero gguffer --list-quants' para ver la lista." >&2
                return 1
            fi
            qnums+=("$qnum")
        done
        # Con varios quants, --out no puede ser la ruta exacta de un fichero:
        # cada quant necesita la suya.
        if (( ${#quants[@]} > 1 )) && [[ -n "$ROPERO_GGUF_OUT_FILE" ]]; then
            echo "ERROR: con varios quants, --out debe ser un nombre o un directorio" >&2
            echo "       (la ruta exacta es de un solo fichero)" >&2
            return 2
        fi
    fi

    # ── donde vive cada cosa ───────────────────────────────────────────────
    local work_root="${GGUF_WORK_DIR:-$HOME/.cache/ropero/gguffer}"
    local out_dir="${GGUF_DIR:-$HOME/models/gguf}"
    # Si --out trae un directorio, el GGUF va ahi. Si trae la ruta exacta del
    # fichero, el directorio es el de esa ruta. El launcher busca cada modelo
    # en $GGUF_DIR/<carpeta_del_modelo>/, asi que esto es lo que hace que el
    # GGUF fabricationado caiga donde el TOML lo va a buscar.
    # Con --out-principal, y sin --out, el directorio es el del principal:
    # asi el GGUF intermedio se queda en la carpeta del modelo y no suelta en
    # la raiz del GGUF_DIR.
    if [[ -n "$ROPERO_GGUF_OUT_DIR" ]]; then
        out_dir="$ROPERO_GGUF_OUT_DIR"
    elif [[ -n "$ROPERO_GGUF_OUT_FILE" ]]; then
        out_dir="$(dirname "$ROPERO_GGUF_OUT_FILE")"
    elif [[ -n "$ROPERO_GGUF_OUT_PRINCIPAL" ]]; then
        out_dir="$(dirname "$ROPERO_GGUF_OUT_PRINCIPAL")"
    elif [[ -n "$ROPERO_GGUF_OUT_PRINCIPAL_DIR" ]]; then
        # el principal va a una CARPETA: el nombre lo pone el gguffer
        out_dir="$ROPERO_GGUF_OUT_PRINCIPAL_DIR"
    fi
    mkdir -p "$out_dir"

    local work="" source_dir="" nombre="" hf_cache="" _gguf_entrada=""
    local _gguf_local=0
    # El work se puede borrar SOLO si es del gguffer (lo ha descargado el).
    # Con --gguf de un fichero local, el work es la carpeta del usuario.
    local _puede_borrar_work=1
    if [[ -n "$gguf_origen" ]]; then
        # El GGUF ya hecho (local o de un repo de HF). No hay pesos que
        # clonar: el work es la carpeta donde se deja el GGUF.
        if [[ "$gguf_origen" == */* && ! -f "$gguf_origen" && "$gguf_origen" != *:* ]]; then
            work="$work_root/gguf_${gguf_origen//\//__}"
        else
            work="$work_root/gguf_$(printf '%s' "$gguf_origen" | tr -c 'a-zA-Z0-9._-' '_')"
        fi
        # Si el GGUF es local, no hay nada que descargar: el work es su
        # carpeta, que es donde se mira si hace falta.
        if [[ -f "$gguf_origen" ]]; then
            work="$(dirname "$gguf_origen")"
            _gguf_local=1
            _puede_borrar_work=0
        fi
        source_dir="$work"
        nombre="${gguf_origen##*/}"
        nombre="${nombre%.gguf}"
    elif [[ -n "$repo" ]]; then
        work="$work_root/${repo//\//__}"
        source_dir="$work"
        nombre="${repo##*/}"
        hf_cache="${HF_HOME:-$HOME/.cache/huggingface}/hub/models--${repo//\//--}"
    else
        work="$path"
        source_dir="$path"
        nombre="$(basename "$path")"
    fi
    [[ -n "$out_name" ]] && nombre="$out_name"
    local sentinel="$work/.gguffer-complete"

    # ── SI YA EXISTE, NO SE HACE NADA ───────────────────────────────────────
    # El GGUF de salida puede ser que ya este ahi: entonces la tirada
    # entera (bajar 60 GB, convertir, cuantizar) es trabajo para nada, y
    # encima se pisa un fichero que puede ser hours de trabajo.
    #
    # Este check va ANTES de descargar, y no solo antes de convertir: la
    # descarga es la parte cara (una hora y 60 GB). Por eso las rutas
    # finales se calculan aqui, con el nombre base que ya se sabe, y no
    # despues de la conversion.
    #
    # Con --force se rehace (que es lo que quiere quien pide --force).
    if [[ -n "$repo" && $force -eq 0 && -z "$path" ]] && (( ${#quants[@]} > 0 )); then
        local -a _previas=()
        local _qp _qi=0
        for _qp in ${quants[@]+"${quants[@]}"}; do
            if (( _qi == 0 )) && [[ -n "$ROPERO_GGUF_OUT_PRINCIPAL" ]]; then
                _previas+=("$ROPERO_GGUF_OUT_PRINCIPAL")
            elif (( _qi == 0 )) && [[ -n "$ROPERO_GGUF_OUT_PRINCIPAL_DIR" ]]; then
                _previas+=("$ROPERO_GGUF_OUT_PRINCIPAL_DIR/${nombre}-${_qp}.gguf")
            elif [[ -n "$ROPERO_GGUF_OUT_FILE" ]]; then
                _previas+=("$ROPERO_GGUF_OUT_FILE")
            else
                _previas+=("$out_dir/${nombre}-${_qp}.gguf")
            fi
            _qi=$((_qi+1))
        done
        local _falta=0 _op
        for _op in ${_previas[@]+"${_previas[@]}"}; do
            [[ -f "$_op" ]] || _falta=1
        done
        if (( _falta == 0 )); then
            if (( ${#_previas[@]} == 1 )); then
                echo "[gguffer] ya existe: ${_previas[0]}"
            else
                echo "[gguffer] ya existen todos los quants pedidos:"
                for _op in ${_previas[@]+"${_previas[@]}"}; do echo "           $_op"; done
            fi
            echo "           no se hace nada (usa --force para rehacerlo)"
            return 0
        fi
    fi

    # ── el imatrix: si viene de un repo, se baja aqui ───────────────────────
    # El imatrix es un fichero aparte del modelo (unos 200 MB) y lo publica
    # otra gente. Antes habia que bajarlo a mano con hf y pasarle la ruta;
    # con --imatrix-repo/--imatrix-file se baja solo y se cachea, asi que el
    # gguffer se encarga de todo el proceso.
    if [[ -n "$imatrix_repo" || -n "$imatrix_file" ]]; then
        [[ -n "$imatrix_repo" && -n "$imatrix_file" ]] || {
            echo "ERROR: --imatrix-repo y --imatrix-file van juntos" >&2; return 2; }
        if ! command -v hf >/dev/null 2>&1; then
            echo "ERROR: falta 'hf' (huggingface_hub) para bajar el imatrix" >&2
            return 1
        fi
        # El cache va por repo de imatrix, no por repo de pesos: el mismo
        # imatrix sirve para cualquier quant del mismo modelo base.
        local im_dir="$work_root/imatrix/${imatrix_repo//\//__}"
        # El fichero puede venir con subdirectorio ("imatrix/muse-glimmer.imatrix.gguf"):
        # hf lo deja en <dir>/<subdir>/<fichero>.
        local im_dest="$im_dir/$imatrix_file"
        if [[ -f "$im_dest" ]]; then
            echo "[gguffer] reutilizo el imatrix: $im_dest"
            imatrix="$im_dest"
        else
            echo "[gguffer] bajo el imatrix de $imatrix_repo/$imatrix_file..."
            mkdir -p "$im_dir"
            if ! hf download "$imatrix_repo" "$imatrix_file" --local-dir "$im_dir"; then
                echo "ERROR: no se pudo bajar el imatrix de $imatrix_repo/$imatrix_file" >&2
                return 1
            fi
            if [[ ! -f "$im_dest" ]]; then
                echo "ERROR: el imatrix no esta en $im_dest" >&2
                echo "       revisa el nombre del fichero en $imatrix_repo" >&2
                return 1
            fi
            imatrix="$im_dest"
        fi
    fi
    if [[ -n "$imatrix" && ! -f "$imatrix" ]]; then
        echo "ERROR: el imatrix no existe: $imatrix" >&2
        return 1
    fi

    # ── --force: limpiarlo todo y empezar de cero ───────────────────────────
    if (( force == 1 )); then
        echo "[gguffer] --force: limpio lo cacheado de ${repo:-$path}"
        [[ -n "$repo" ]] && rm -rf "$hf_cache"
        # Con --gguf de un fichero local, el work es la CARPETA DEL
        # USUARIO: no se toca. Lo que el usuario ha traido no se borra.
        if (( _puede_borrar_work == 1 )); then
            rm -rf "$work"
        else
            echo "           (el GGUF de entrada es tuyo: no lo borro)"
        fi
        if [[ -n "$ROPERO_GGUF_OUT_FILE" ]]; then
            [[ "$ROPERO_GGUF_OUT_FILE" == "$_gguf_entrada" ]] || rm -f "$ROPERO_GGUF_OUT_FILE"
        else
            for _f in "$out_dir/${nombre}"*.gguf; do
                [[ -e "$_f" ]] || continue
                # El GGUF de entrada es del usuario: jamas se borra.
                [[ "$_f" == "$_gguf_entrada" ]] && continue
                rm -f "$_f"
            done
        fi
    fi

    # ── FASE 0: el PREFLIGHT ────────────────────────────────────────────────
    #
    # Todo lo que se puede comprobar ANTES de bajar 40-60 GB. Bajar los pesos
    # es la parte cara e irreversible del proceso (una hora y decenas de GB),
    # y hay fallos que se saben de antemano:
    #
    #   · el repo ya viene CUANTIZADO (los de NVIDIA, los de modelopt): el
    #     cuantizador no puede re-cuantizar y falla al final, cuando ya se ha
    #     gastado todo;
    #   · el motor no tiene el cuant que se pide;
    #   · falta el imatrix que ese quant exige.
    #
    # El primero es el que mas duele, porque el error del motor
    # ("requantizing from type nvfp4 is disabled") no dice de donde sale el
    # problema, y la respuesta (usar el repo BF16) se puede buscar aqui.
    if [[ -n "$repo" && -z "$gguf_origen" ]]; then
        local _motivo=""
        if _motivo="$(_gguffer_repo_ya_cuantizado "$repo" 2>&1)"; then
            echo "ERROR: el repo $repo YA VIENE CUANTIZADO ($_motivo)." >&2
            echo "       Cuantizar encima de eso es dequantizar y re-cuantizar:" >&2
            echo "       se pierde calidad sin ganar nada, y el motor lo rechaza:" >&2
            echo "         llama_model_quantize: failed to quantize:" >&2
            echo "         requantizing from type nvfp4 is disabled" >&2
            local _hermano
            _hermanos=()
            while IFS= read -r _l; do
                [[ -n "$_l" ]] && _hermanos+=("$_l")
            done < <(_gguffer_hermanos_sin_quantizar "$repo" 2>/dev/null)
            if (( ${#_hermanos[@]} > 0 )); then
                echo "       Usa el repo SIN cuantizar:" >&2
                local _h
                for _h in ${_hermanos[@]+"${_hermanos[@]}"}; do
                    echo "         ropero gguffer --repo $_h --quant <QUANT> ..." >&2
                done
            else
                echo "       No he encontrado un repo hermano sin cuantizar." >&2
                echo "       Busca uno del mismo modelo en BF16 o F16." >&2
            fi
            return 1
        fi
    fi

    # ── FASE 1: la descarga ─────────────────────────────────────────────────
    if [[ -n "$gguf_origen" ]]; then
        # Ya hay un GGUF: se baja (si viene de un repo), se unen los shards
        # si viene partido, y no se convierte NADA. El conversor de pesos
        # es justo lo que falla con los modelos multimodales con cabeza MTP.
        local _preparado
        _preparado="$(_gguffer_prepara_gguf "$gguf_origen" "$gguf_file" "$work")" || return 1
        if [[ ! -f "$_preparado" ]]; then
            echo "ERROR: no se pudo preparar el GGUF de $gguf_origen" >&2
            return 1
        fi
        echo "[gguffer] GGUF de entrada: $_preparado"
        _gguf_entrada="$_preparado"
        # El nombre del fichero final: el del repo sin el sufijo "-GGUF"
        # (que es como se llaman los repos de cuantizaciones), para que el
        # GGUF no se llame "Qwen3.8-27B-GGUF-MXFP4.gguf".
        if [[ -n "$gguf_file" ]]; then
            nombre="${gguf_file##*/}"; nombre="${nombre%.gguf}"
        fi
        nombre="${nombre%-GGUF}"
        # Sin el sufijo del shard ("-00001-of-00002") ni el del tipo
        # ("-bf16"): el nombre es el del MODELO, no el del fichero de
        # entrada. El GGUF final se llama "Qwen3.8-27B-MXFP4.gguf".
        nombre="$(printf '%s' "$nombre" | sed -E 's/-[0-9]{5}-of-[0-9]+$//; s/-(bf16|BF16|f16|F16)$//')"
    elif [[ -n "$repo" ]]; then
        if [[ -f "$sentinel" ]]; then
            echo "[gguffer] reutilizo el clon de $repo en $work"
        else
            if ! command -v hf >/dev/null 2>&1; then
                echo "ERROR: falta 'hf' (huggingface_hub) para descargar" >&2
                return 1
            fi
            mkdir -p "$work"
            echo "[gguffer] descargo $repo en $work..."
            local hf_args=(download "$repo" --local-dir "$work")
            [[ -n "$include" ]] && hf_args+=(--include "$include")
            if ! hf ${hf_args[@]+"${hf_args[@]}"}; then
                # Si la descarga falla no queda nada utilizable: se limpia
                # el clon a medias y la cache de HF, que son justo lo que
                # dejaria a medias occupying disco.
                echo "ERROR: fallo la descarga de $repo" >&2
                rm -rf "$work"
                [[ -n "$hf_cache" ]] && rm -rf "$hf_cache"
                echo "       (he limpiado el clon parcial y la cache de HF)" >&2
                return 1
            fi
            touch "$sentinel"
        fi
    fi

    if [[ ! -d "$source_dir" ]]; then
        echo "ERROR: directorio no encontrado: $source_dir" >&2
        return 1
    fi

    # Los pesos pueden venir ya cuantizados (el gpt-oss viene en MXFP4).
    # Cuantizar encima de eso es dequantizar y re-cuantizar: se pierde
    # calidad sin ganar nada. Si el repo trae 'original/', ahi estan los
    # pesos sin quantizar.
    if [[ -n "$from_subdir" ]]; then
        source_dir="$work/$from_subdir"
        [[ -d "$source_dir" ]] || { echo "ERROR: no existe $source_dir" >&2; return 1; }
    fi

    # ── FASE 2: conversion a GGUF ───────────────────────────────────────────
    local inter_up="${inter^^}"
    [[ "$inter_up" == "F16" || "$inter_up" == "BF16" ]] || { echo "ERROR: --intermedio solo admite f16 o bf16" >&2; return 2; }
    # El intermedio se reutiliza SEA CUAL SEA el que haya (F16 o BF16): si
    # te has quedado con uno del paso anterior, no se vuelve a convertir.
    # Importa porque el nombre depende de --intermedio y cambiar la flag
    # entre pasos dejaba el intermedio viejo huérfano.
    local out_f16="" _cand
    for _cand in "$out_dir/${nombre}-F16.gguf" "$out_dir/${nombre}-BF16.gguf"; do
        if [[ -f "$_cand" ]]; then out_f16="$_cand"; break; fi
    done
    [[ -z "$out_f16" ]] && out_f16="$out_dir/${nombre}-${inter_up}.gguf"
    # Con --gguf el intermedio es el GGUF que trajimos, no uno del out_dir:
    # gana el que ya esta.
    [[ -n "$_gguf_entrada" ]] && out_f16="$_gguf_entrada"
    # Las rutas finales, una por quant. Con varios quants en una tirada cada
    # uno tiene la suya; con uno solo, --out puede fijar la ruta exacta (por
    # ejemplo, el nombre que espera el TOML, que no siempre es
    # <repo>-<quant>.gguf).
    local -a out_paths=()
    # El nombre base del fichero final: el del repo, o el que se pida con
    # --out <nombre> (que ya ha sustituido a $nombre mas arriba).
    local nombre_base="$nombre"
    if (( ${#quants[@]} == 0 )); then
        # Sin quant, la salida es el intermedio: se convierte directamente
        # ahi, en vez de convertir a un F16 aparte y luego renombrar.
        if [[ -n "$ROPERO_GGUF_OUT_FILE" ]]; then
            out_f16="$ROPERO_GGUF_OUT_FILE"
            out_paths=("$ROPERO_GGUF_OUT_FILE")
        else
            out_paths=("$out_f16")
        fi
    else
        local _qp _qi2=0
        for _qp in ${quants[@]+"${quants[@]}"}; do
            if (( _qi2 == 0 )) && [[ -n "$ROPERO_GGUF_OUT_PRINCIPAL" ]]; then
                # El primer quant de la tirada va exactamente ahi.
                out_paths+=("$ROPERO_GGUF_OUT_PRINCIPAL")
            elif (( _qi2 == 0 )) && [[ -n "$ROPERO_GGUF_OUT_PRINCIPAL_DIR" ]]; then
                # El primero va a una CARPETA (el TOML pedia otro quant):
                # el nombre lo pone el gguffer, no se falsea el del TOML.
                # OJO: se usa el directorio DEL PRINCIPAL, no $out_dir (que
                # es el de los secundarios, que puede ser otro).
                out_paths+=("$ROPERO_GGUF_OUT_PRINCIPAL_DIR/${nombre_base}-${_qp}.gguf")
            elif [[ -n "$ROPERO_GGUF_OUT_FILE" ]]; then
                out_paths+=("$ROPERO_GGUF_OUT_FILE")
            else
                out_paths+=("$out_dir/${nombre_base}-${_qp}.gguf")
            fi
            _qi2=$((_qi2+1))
        done
    fi

    # El intermedio y el final pueden ser el mismo fichero (--quant F16), y
    # en ese caso el "ya existe" no aplica: es el intermedio del que
    # speak, no un resultado de una corrida anterior.
    # Con VARIOS quants, el "ya existe" se comprueba uno por uno dentro del
    # bucle de cuantizacion (los que faltan se hacen, los que estan se
    # respetan). Aqui solo se corta antes de descargar 60 GB si NO falta
    # ninguno.
    if (( ${#quants[@]} > 0 )); then
        local _falta=0 _hay_otro=0 _op
        for _op in ${out_paths[@]+"${out_paths[@]}"}; do
            # El F16 como resultado (--quant F16) lo gestiona el flujo normal.
            [[ "$_op" == "$out_f16" ]] && continue
            _hay_otro=1
            [[ -f "$_op" ]] || _falta=1
        done
        # OJO: $force -eq 0 va dentro de [[ ]]. Fuera, bash lo parsea como
        # "ejecutar el comando 0" (por el &&) y el if NUNCA se cumple.
        if (( _falta == 0 && _hay_otro == 1 )) && [[ $force -eq 0 && -z "$path" ]]; then
            echo "[gguffer] ya existen todos los quants pedidos:"
            for _op in ${out_paths[@]+"${out_paths[@]}"}; do echo "           $_op"; done
            echo "           (usa --force para rehacerlos)"
            return 0
        fi
    elif [[ -f "$out_f16" && $force -eq 0 && -z "$path" && "${out_paths[0]}" != "$out_f16" ]]; then
        echo "[gguffer] ya existe: ${out_paths[0]} (usa --force para rehacerlo)"
        return 0
    fi

    # ── EL ESPACIO, ANTES DE CONVERTIR ─────────────────────────────────────
    # Convertir un modelo de 27B escribe un GGUF de tensores de pesos de
    # decenas de GB. Si el disco se llena a mitad, el conversor muere con un
    # error que no dice "te has quedado sin disco", y el trabajo de la noche
    # se pierde. Se comprueba ANTES.
    if [[ ! -f "$out_f16" && -n "$repo" ]]; then
        local _libre _margen
        read -r _libre _margen <<< "$(_gguffer_necesita_espacio "$work" "$out_f16")"
        if [[ -n "${_libre:-}" && "${_libre:-0}" -lt "${_margen:-0}" ]]; then
            echo "ERROR: no hay espacio para convertir." >&2
            echo "       libre:  $(numfmt --to=iec-i --suffix=B "${_libre:-0}" 2>/dev/null)" >&2
            echo "       hace falta: $(numfmt --to=iec-i --suffix=B "${_margen:-0}" 2>/dev/null) (el GGUF intermedio + margen)" >&2
            echo "       El GGUF intermedio es del tamano de los pesos (o la mitad si son F32)." >&2
            echo "       Libera espacio, o pon GGUF_WORK_DIR y GGUF_DIR en otro disco." >&2
            return 1
        fi
    fi

    if [[ ! -f "$out_f16" ]]; then
        # --no-mtp solo si el conversor lo entiende: no todas las versiones
        # de llama.cpp lo tienen, y pasar un flag que no existe hace que el
        # conversor falle con "unrecognized arguments".
        local conv_args=("$source_dir" --outfile "$out_f16" --outtype "$inter")
        if (( no_mtp == 1 )); then
            if grep -qE "no-mtp|no_nextn" "$_convert" 2>/dev/null; then
                conv_args+=(--no-mtp)
            else
                echo "[gguffer] aviso: este conversor no sabe excluir la MTP (--no-mtp); sigo sin el"
            fi
        fi
        # ── EL TOKENIZER EXTRA ────────────────────────────────────────────
        # Hay repos cuyo tokenizer NO esta en el repo de los pesos (el
        # tokenizer va aparte). Si se pasa, se copia al clon antes de
        # convertir: el conversor lo lee de ahi.
        if [[ -n "$tokenizer_extra" ]]; then
            if [[ -s "$tokenizer_extra" ]]; then
                local _tbase _tdest
                _tbase="$(basename "$tokenizer_extra")"
                case "$_tbase" in
                    tokenizer.json|tokenizer_config.json|*.jinja) _tdest="$source_dir/$_tbase" ;;
                    *) _tdest="$source_dir/$_tbase" ;;
                esac
                if cp -f "$tokenizer_extra" "$_tdest"; then
                    echo "[gguffer] tokenizer extra copiado al clon: $_tbase"
                else
                    echo "ERROR: no se pudo copiar el tokenizer extra a $_tdest" >&2
                    return 1
                fi
            else
                echo "ERROR: el tokenizer extra no existe: $tokenizer_extra" >&2
                return 1
            fi
        fi

        echo "[gguffer] convierto $source_dir -> $out_f16 ($inter_up)..."
        # El tokenizer se arregla ANTES de convertir, no solo cuando la
        # conversion falla: el chat template que falta NO hace fallar nada,
        # pero deja el GGUF con el template de la comunidad de llama.cpp
        # (que puede ser de otro modelo). Y el tokenizer_class de
        # transformers v5 tampoco tiene que fallar: se arregla igualmente.
        ROPERO_CHAT_TEMPLATE="$chat_template" _gguffer_arreglar_tokenizer "$source_dir" || true
        local conv_log="${TMPDIR:-/tmp}/gguffer-conv.$$.log"
        local conv_ok=0
        if python3 "$_convert" ${conv_args[@]+"${conv_args[@]}"} >"$conv_log" 2>&1; then
            conv_ok=1
        elif grep -q "Tokenizer class .* does not exist" "$conv_log" 2>/dev/null \
             && ROPERO_CHAT_TEMPLATE="$chat_template" _gguffer_arreglar_tokenizer "$source_dir"; then
            # Algunos repos se han guardado con transformers v5, cuyo
            # tokenizer_class es "TokenizersBackend". Con transformers v4 esa
            # clase no existe y la conversion muere con "Tokenizer class
            # TokenizersBackend does not exist". El tokenizer.json es un fast
            # tokenizer estandar, asi que basta con poner la clase que si
            # conoce la v4, y reintentar.
            echo "[gguffer] el tokenizer del repo usa una clase que tu transformers no conoce."
            echo "           (TokenizersBackend es de transformers v5)"
            echo "           Lo cambio a PreTrainedTokenizerFast y reintento..."
            if python3 "$_convert" ${conv_args[@]+"${conv_args[@]}"} >"$conv_log" 2>&1; then
                conv_ok=1
            fi
        elif grep -q -- "--mtp / --no-nextn are not supported" "$conv_log" 2>/dev/null \
             && [[ " ${conv_args[*]} " == *" --no-mtp "* ]]; then
            # El conversor acepta --no-mtp, pero no para ESTA arquitectura.
            # Y no hace falta: si la arch no tiene el mixin de MTP
            # (conversion/nemotron.py no lo tiene, conversion/qwen.py si), el
            # conversor ya deja fuera la capa nextn. El GGUF sale con 52
            # bloques y SIN la metadata nextn_predict_layers, que es
            # justamente lo que hace que el server lo cargue bien.
            echo "[gguffer] aviso: esta arquitectura no admite --no-mtp."
            echo "           Reintento sin el: el conversor ya deja fuera la MTP."
            local _i
            for _i in "${!conv_args[@]}"; do
                [[ "${conv_args[$_i]}" == "--no-mtp" ]] && unset 'conv_args[_i]'
            done
            if python3 "$_convert" ${conv_args[@]+"${conv_args[@]}"} >"$conv_log" 2>&1; then
                conv_ok=1
            fi
        fi
        if (( conv_ok == 0 )); then
            tail -20 "$conv_log" >&2
            rm -f "$out_f16"
            echo "ERROR: fallo la conversion a GGUF" >&2
            # Por que fallo, que el log del conversor no lo dice claro.
            if grep -qiE "no space left|disk quota|write error" "$conv_log" 2>/dev/null; then
                echo "       ==> se ha quedado SIN ESPACIO en disco." >&2
                echo "       El GGUF intermedio necesita tanto como los pesos (o la mitad si son F32)." >&2
            elif grep -qiE "MemoryError|OutOfMemory|Cannot allocate memory|Killed" "$conv_log" 2>/dev/null; then
                echo "       ==> se ha quedado SIN MEMORIA (el conversor se ha comido la RAM)." >&2
            elif grep -qiE "does not exist|not found|No such file" "$conv_log" 2>/dev/null; then
                echo "       ==> faltan ficheros en el clon: la descarga esta incompleta." >&2
                echo "       borra $work y vuelve a lanzar, o baja el repo a mano." >&2
            fi
            if (( keep_source == 1 )); then
                echo "       conservo los pesos en: $work (--keep-source)" >&2
                echo "       reintenta solo la conversion:" >&2
                echo "         python3 $_convert ${conv_args[*]}" >&2
            else
                # Sin --keep-source, los pesos se borran: 60 GB que se quedan
                # ahi para siempre y un trabajo de noche que se llena el disco
                # es peor que volver a bajar. Se avisa de como pedirlos otra
                # vez, porque el repo es publico.
                if (( _puede_borrar_work == 1 )); then
                    echo "       borro los pesos (60 GB) para no dejar el disco lleno." >&2
                    echo "       no se guardan porque no se paso --keep-source." >&2
                    rm -rf "$work"
                else
                    # El GGUF de entrada es un fichero del usuario, en su
                    # carpeta: eso no se borra, ni con --force ni al fallar.
                    echo "       el GGUF de entrada es tuyo: no lo borro." >&2
                fi
            fi
            return 1
        fi
    else
        echo "[gguffer] reutilizo el GGUF intermedio: $out_f16"
    fi

    # ── EL DIAGNOSTICO: POR QUE NO SE CUANTIZA ─────────────────────────────
    # Con --diagnostico no se cuantiza: se imprime el informe y se sale. Es
    # lo que hay que mirar cuando la cuantizacion falla a las tres de la
    # madrugada, en vez de adivinar.
    if (( diagnostico == 1 )); then
        _gguffer_diagnostico "$source_dir" "$out_f16" "$imatrix" "$imatrix_repo" \
                            "$imatrix_file" "${quants[0]:-}" "$_gguf_entrada"
        return 0
    fi

    # ── FASE 3: la cuantizacion ────────────────────────────────────────────
    # El file_type del GGUF dice en que quant estan los pesos. Si ya estan
    # cuantizados (el gpt-oss viene en MXFP4), cuantizar encima es
    # dequantizar y re-cuantizar: se pierde calidad sin ganar nada. Por eso
    # se mira ANTES, con el dato real y no con una suposicion.
    local file_type
    file_type="$(python3 "$_ggufcheck" "$out_f16" --tipo-predominante 2>/dev/null | head -1 | tr -dc '0-9')"
    # El nombre del tipo predominante: el numero solo no vale (el 30 es BF16
    # y IQ4_XS a la vez), asi que se busca en la lista del motor.
    local tipo_nombre=""
    [[ -n "$file_type" ]] && tipo_nombre="$(printf '%s' "$quants_num" | tr ' ' '\n' | grep -E "^${file_type}=" | head -1 | cut -d= -f2)"

    # ── la cabeza MTP y el imatrix ─────────────────────────────────────────
    # El imatrix se calcula sobre el modelo base, pero el GGUF lleva la MTP
    # como un bloque MAS (el siguiente al ultimo). Ese bloque no esta en el
    # imatrix, y con un quant very-low-bit el cuantizador aborta:
    #   "Missing importance matrix for tensor blk.64.attn_k.weight".
    # La salida es dejar ese bloque en un quant que NO exija imatrix (Q4_0),
    # que es lo que hizo bartowski con su IQ2_M del Qwen3.8. Es lo mismo para
    # todos los quants de la tirada, asi que se calcula UNA vez.
    local _nextn_bloque="" _qargs_comunes=()
    [[ -n "$imatrix" ]] && _qargs_comunes+=(--imatrix "$imatrix")
    if [[ -n "$imatrix" && ${#quants[@]} -gt 0 ]]; then
        # Que bloques del GGUF no estan en el imatrix. Se comparan BLOQUE a
        # BLOQUE (no tensor a tensor: las normas no estan en ningun imatrix
        # porque no se cuantizan, y eso no significa nada).
        # El bloque de la MTP se localiza por DOS vias, y se usan las dos:
        #
        #  a) POR LA METADATA (block_count - nextn_predict_layers). Es lo que
        #     dice el GGUF. Funciona siempre que el conversor haya escrito
        #     la metadata.
        #  b) POR COMPARACION con el imatrix: los bloques que el GGUF tiene y
        #     el imatrix no. Funciona siempre que la metadata falte, que es
        #     justo lo que pasaba (el conversor de la MTP no la escribia y
        #     el bloque se quedaba sin cubrir).
        #
        # Se cogen los dos y se unen: si los dos coinciden, se aplica una
        # sola vez (que es lo normal).
        local -a _sin_imx=()
        local _b
        while read -r _b; do
            [[ -n "$_b" ]] || continue
            _sin_imx+=("$_b")
        done < <(_gguffer_bloques_sin_imatrix "$out_f16" "$imatrix" "$_ggufcheck" 2>/dev/null || true)
        _nextn_bloque="$(_gguffer_bloque_nextn "$out_f16" "$_ggufcheck" 2>/dev/null || true)"
        if [[ -n "$_nextn_bloque" ]]; then
            local _ya=0
            for _b in ${_sin_imx[@]+"${_sin_imx[@]}"}; do [[ "$_b" == "$_nextn_bloque" ]] && _ya=1; done
            (( _ya )) || _sin_imx+=("$_nextn_bloque")
        fi
        if ((${#_sin_imx[@]} > 0)); then
            local _lista
            _lista="$(printf '%s ' ${_sin_imx[@]+"${_sin_imx[@]}"})"
            if ((${#_sin_imx[@]} == 1)); then
                echo "[gguffer] aviso: el imatrix no cubre el bloque ${_sin_imx[0]} (la cabeza MTP)."
            else
                echo "[gguffer] aviso: el imatrix no cubre los bloques ${_lista% } (la cabeza MTP)."
            fi
            echo "           Lo(s) cuantizo en ${MTP_QUANT:-Q4_0}, que no exige imatrix."
            for _b in ${_sin_imx[@]+"${_sin_imx[@]}"}; do
                _qargs_comunes+=(--tensor-type "blk\.${_b}\..*=${MTP_QUANT:-Q4_0}")
            done
        fi
    fi

    # ── el bucle de quants ─────────────────────────────────────────────────
    # Bajar y convertir los pesos es lo caro (40-60 GB y una hora). Cuantizar
    # es solo CPU. Por eso se pueden pedir VARIOS quants en una tirada: el
    # F16 se convierte una vez y se reutiliza para todos.
    local -a out_finals=()
    local qi quant_i qnum_i
    for qi in "${!quants[@]}"; do
        quant_i="${quants[$qi]}"
        qnum_i="${qnums[$qi]}"
        local out_q="${out_paths[$qi]}"
        out_finals+=("$out_q")

        if [[ -n "$tipo_nombre" && "$tipo_nombre" != "$qnum_i" &&
              "$tipo_nombre" != "F16" && "$tipo_nombre" != "BF16" && "$tipo_nombre" != "F32" ]]; then
            echo "[gguffer] aviso: los pesos del repo ya venian en el quant $tipo_nombre ($file_type)."
            echo "           Cuantizar a $quant_i encima es dequantizar y re-cuantizar:"
            echo "           se pierde calidad sin ganar nada."
            if [[ -d "$work/original" ]]; then
                echo "           Usa --from-subdir original para cuantizar de los pesos SIN quantizar."
            fi
        fi

        if [[ -n "$file_type" && "$file_type" == "$qnum_i" ]]; then
            # El GGUF ya esta en ese quant (el caso del gpt-oss, que se
            # convierte a MXFP4 sin cuantizar). Cuantizar encima seria
            # dequantizar y re-cuantizar: se pierde calidad sin ganar nada.
            echo "[gguffer] el GGUF ya esta en $quant_i (file_type=$file_type): no cuantizo otra vez"
            if [[ "$out_q" != "$out_f16" ]]; then
                rm -f "$out_q"
                mv "$out_f16" "$out_q"
            fi
            continue
        fi

        # El que ya existe de una corrida anterior: no se rehace.
        if [[ -f "$out_q" && $force -eq 0 && -z "$path" ]]; then
            echo "[gguffer] ya existe: $out_q (usa --force para rehacerlo)"
            continue
        fi

        echo "[gguffer] cuantizo a $quant_i -> $out_q..."
        # La carpeta del destino puede no existir (con --out-principal es
        # distinta de la del resto de quants).
        mkdir -p "$(dirname "$out_q")" 2>/dev/null || true
        # OJO: llama-quantize solo parsea las opciones mientras el argumento
        # empiece por "--", y para en el primero que no. Las flags van ANTES
        # de los posicionales: si van detras, "--imatrix" se come el parseo
        # del numero de hilos y el cuantizador muere con
        # "invalid nthread '--imatrix'".
        local qargs=(${_qargs_comunes[@]+"${_qargs_comunes[@]}"})
        qargs+=("$out_f16" "$out_q" "$quant_i")
        # La salida del cuantizador se guarda: si falla, su ultimo error dice
        # por que (el mensaje del gguffer solo no alcanza).
        local _gguffer_qlog="${TMPDIR:-/tmp}/gguffer-quant.$$.log"
        if ! "$_quantize" ${qargs[@]+"${qargs[@]}"} >"$_gguffer_qlog" 2>&1; then
            tail -20 "$_gguffer_qlog" >&2
            # Aqui NO se borra nada: el clon de los safetensors y el GGUF
            # intermedio se quedan, y se dice como reintentar solo la
            # cuantizacion (que es la parte cara).
            rm -f "$out_q"
            echo "ERROR: fallo la cuantizacion a $quant_i" >&2
            if (( keep_source == 1 )); then
                echo "       conservo los pesos en: $work (--keep-source)" >&2
                echo "       conservo el GGUF intermedio en: $out_f16" >&2
                echo "       reintenta solo la cuantizacion:" >&2
                echo "         $_quantize ${qargs[*]}" >&2
            else
                # Sin --keep-source, los pesos se borran. Un trabajo de noche
                # que falla many veces seguidas no puede dejar 60 GB por cada
                # intento: el disco se llena y luego falla TODO, incluso lo
                # que ya estaba bien.
                if (( _puede_borrar_work == 1 )); then
                    echo "       borro los pesos (60 GB) para no dejar el disco lleno." >&2
                    echo "       no se guardan porque no se paso --keep-source." >&2
                    rm -rf "$work"
                else
                    # El GGUF de entrada es un fichero del usuario, en su
                    # carpeta: eso no se borra, ni con --force ni al fallar.
                    echo "       el GGUF de entrada es tuyo: no lo borro." >&2
                fi
            fi
            # El error mas comun: el imatrix no cubre un tensor. Se dice cual,
            # porque el mensaje del cuantizador ("Missing importance matrix
            # for tensor blk.64.attn_k.weight") no dice de donde sale.
            if grep -q "Missing importance matrix for tensor" "$_gguffer_qlog" 2>/dev/null; then
                local _t_missing
                _t_missing="$(sed -n 's/.*Missing importance matrix for tensor \([^ ]*\).*/\1/p' "$_gguffer_qlog" | head -1)"
                echo "       el imatrix no cubre el tensor: ${_t_missing:-?}" >&2
                if [[ -n "$_nextn_bloque" && "${_t_missing:-}" == blk."$_nextn_bloque".* ]]; then
                    echo "       es el bloque $_nextn_bloque, el de la cabeza MTP." >&2
                    echo "       El imatrix se calcula sobre el modelo base, que no la tiene." >&2
                    echo "       Opciones: dejar ese bloque en ${MTP_QUANT:-Q4_0} (--tensor-type)," >&2
                    echo "       o convertir sin MTP (--no-mtp) y renombrar el GGUF." >&2
                fi
            fi
            return 1
        fi
        # El intermedio se borra al FINAL de todos los quants, no aqui: con
        # varios quants en una tirada, el siguiente lo necesita.
    done

    # ── FASE 4: verificar los GGUFs ─────────────────────────────────────────
    # Solo si el GGUF esta bien se borran los safetensors: si esta
    # corrupto, tenerlos vale mas que 60 GB de disco.
    local out_final verif_ok _q
    for _q in ${out_finals[@]+"${out_finals[@]}"}; do
        out_final="$_q"
        verif_ok=0
        if [[ -n "$_gguf_split" ]]; then
            if "$_gguf_split" --dry-run "$out_final" "$out_dir/.gguffer-verif" >/dev/null 2>&1; then
                verif_ok=1
            fi
        fi
        if (( verif_ok == 0 )); then
            local chk_args=("$out_final")
            [[ -n "$_libggml" ]] && chk_args+=(--lib "$_libggml")
            if python3 "$_ggufcheck" ${chk_args[@]+"${chk_args[@]}"}; then
                verif_ok=1
            fi
        fi
        if (( verif_ok == 0 )); then
            echo "ERROR: el GGUF no supera la verificacion: $out_final" >&2
            echo "       NO borro los pesos: estan en $work" >&2
            echo "       (usa --force para rehacerlo todo desde cero)" >&2
            return 1
        fi
    done

    # ── FASE 5: limpiar ─────────────────────────────────────────────────────
    if [[ -n "$repo" && $keep_source -eq 0 ]]; then
        echo "[gguffer] el GGUF esta bien: borro el clon de los safetensors ($work)"
        rm -rf "$work"
    elif [[ -n "$repo" ]]; then
        echo "[gguffer] el GGUF esta bien; conservo los safetensors en $work (--keep-source)"
    fi
    rm -f "$out_dir/.gguffer-verif"
    # El intermedio se borra DESPUES de todos los quants (si no, el segundo
    # cuant se queda sin su F16). Pero si el F16 es el resultado (--quant F16),
    # no se toca: es el fichero final.
    if (( keep_inter == 0 )) && (( ${#quants[@]} > 0 )); then
        local _op _es_resultado=0
        for _op in ${out_finals[@]+"${out_finals[@]}"}; do
            [[ "$_op" == "$out_f16" ]] && _es_resultado=1
        done
        # El GGUF de entrada lo ha traido el usuario: no se borra jamas.
        if [[ "$out_f16" == "$_gguf_entrada" ]]; then
            echo "[gguffer] el GGUF de entrada se queda donde esta (es tuyo)"
        else
            (( _es_resultado == 0 )) && rm -f "$out_f16"
        fi
    fi

    # ── FASE 6: la procedencia ─────────────────────────────────────────────
    # Un GGUF sin saber COMO se hizo no se puede publicar con verguenza: de
    # donde salio el modelo, con que quant, con que importance matrix, y si
    # la cabeza MTP quedo en un quant aparte. Eso es lo que va al model card.
    local _q _qname
    for _q in ${out_finals[@]+"${out_finals[@]}"}; do
        out_final="$_q"
        _qname="$(basename "$out_final" .gguf)"
        _gguffer_escribe_fuente "$out_final" "$_qname" "$repo" "$imatrix" \
            "$imatrix_repo" "$imatrix_file" "$_nextn_bloque" "$inter_up"
    done

    local _sz
    for _q in ${out_finals[@]+"${out_finals[@]}"}; do
        out_final="$_q"
        _sz="$(du -h "$out_final" 2>/dev/null | cut -f1)"
        echo "[gguffer] GGUF listo: $out_final (${_sz:-?})"
    done
    if (( ${#quants[@]} == 0 )); then
        echo "[gguffer] AVISO: NO he cuantizado nada: el GGUF se ha quedado en $inter_up."
        echo "           Para cuantizarlo (reutilizando este mismo intermedio, sin volver"
        echo "           a descargar los pesos):"
        echo "             ropero gguffer --repo ${repo:-(<repo>)} --quant <QUANT>"
    fi
    return 0
}

