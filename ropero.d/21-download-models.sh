#!/usr/bin/env bash
# ropero.d/21-download-models.sh
# Catálogo de modelos descargables por ropero.
#
# Idempotente: si el modelo ya está, skip, salvo con ROPERO_FORCE=1 (lo borra y
# lo vuelve a bajar).
#
# Llamadas:
#   ropero_download_default_models [filtro]
#       Sin filtro → el catálogo entero, preguntando uno a uno.
#       Con filtro → solo el modelo indicado (alias o script), sin preguntar.
#
# DOS TIPOS DE MODELO, DOS FUNCIONES
#
#   ropero_download_gguf <repo> <fichero> <nombre> <tamaño> [dest_dir] [dest_name]
#       Un FICHERO suelto dentro del repo. Para llama.cpp, que carga GGUF.
#
#   ropero_download_safetensors <repo> <dir> <tamaño> [dest_dir] [excludes]
#       Un CHECKPOINT COMPLETO (pesos + config + tokenizer) a un directorio.
#       Para los engines con servidor propio: mlx_lm, airllm, tensorrt-llm.
#
# La razón de que hagan falta las dos: un GGUF no se puede meter en mlx_lm (que
# quiere el layout de pesos de MLX) ni en airllm (que hace AutoModel.from_pretrained
# y espera safetensors), y un checkpoint de HF tampoco se puede meter en llama.cpp.
# Son formatos incompatibles, no variantes del mismo.
#
# POR QUÉ HAY UN CATÁLOGO Y NO UNA LISTA DE LLAMADAS
#
# Para que `ropero --install models <modelo>` instale UNO, hace falta poder
# casar lo que pide el usuario con lo que hay que bajar. La clave de casado es
# el NOMBRE DEL FICHERO FINAL, porque es exactamente lo que declara MODEL_GGUF
# en el .sh del modelo (lo que `ropero list` imprime en la columna gguf=). Así
# no hay una segunda tabla de alias que pueda desincronizarse de la primera:
# el catálogo se indexa por el mismo dato que el loader de modelos ya carga.
#
# UN MODELO = UN BUNDLE, NO UN FICHERO
#
# Un modelo multimodal con MTP no se despliega con un fichero. Gemma4 son tres:
# el GGUF principal, el MTP (decoding especulativo) y el mmproj (proyector de
# visión). Los tres se descargan JUNTOS, con un único consentimiento, porque
# con dos de tres el modelo no arranca: llama-server acepta --model-draft y
# --mmproj con un fichero que no existe y se cae al usarlo.
#
# Por eso el catálogo tiene dos cosas:
#   · la entrada (kind/key/repo/filename/...), que es el fichero principal;
#   · los ficheros SECUNDARIOS (aux) de esa entrada, que son parte del mismo
#     bundle y se bajan con él.
#
# Los aux NO son entradas del catálogo: no se pueden seleccionar por separado
# y no aparecen en la tabla. Son parte de la entrada que los declara.
#
# SCOPE BASE v4.1 (2026-09-10):
#   - qwen     (Qwen3.8-27B, broad multi-stack, único 8/8 bloques)
#   - fable    (Fable Coder 35B-A3B, MTP nativo, elliot/coder)
#   - gptoss_* (mismo GGUF, reasoning high/medium/low, adam/edna/randall)
#   - embed    (jina-code, soporte graphify/RAG)
#
# Modelos fuera del scope base movidos a ~/bin/ropero.d/fired/ y
# ~/models/gguf/fired/. Recuperables: ver cabecera de cada script en fired/.

# ── Catálogo ────────────────────────────────────────────────────────────────
_ROPERO_CAT_KIND=()    # gguf | safetensors
_ROPERO_CAT_KEY=()     # gguf: nombre final · safetensors: nombre del dir
_ROPERO_CAT_REPO=()
_ROPERO_CAT_ARG2=()    # filename (gguf) | modelname (safetensors)
_ROPERO_CAT_NAME=()    # nombre legible
_ROPERO_CAT_SIZE=()
_ROPERO_CAT_DEST=()
_ROPERO_CAT_DNAME=()
# Ficheros SECUNDARIOS de cada entrada (MTP, mmproj...). Formato por fichero:
#   "ruta/en/repo|nombre final|tamaño|etiqueta"
# y la lista de ficheros separada por ";". Vacío = la entrada no tiene aux.
#
# Se guardan aparte de las entradas a propósito: un aux NO es seleccionable
# ni aparece en la tabla. Es parte del bundle de la entrada que lo declara.
_ROPERO_CAT_AUX=()

# _cat_gguf <fichero_final> <repo> <filename> <nombre> <size> [dest_dir] [dest_name]
# El key (1er argumento) es el nombre FINAL del fichero, que es lo que declara
# MODEL_GGUF en el .sh del modelo. Si no se pasa dest_name, se usa el filename.
_cat_gguf() {
    local key="$1" repo="$2" filename="$3" name="$4" size="$5"
    local dest_dir="${6:-}" dest_name="${7:-$filename}"
    _ROPERO_CAT_KIND+=("gguf")
    _ROPERO_CAT_KEY+=("$key")
    _ROPERO_CAT_REPO+=("$repo")
    _ROPERO_CAT_ARG2+=("$filename")
    _ROPERO_CAT_NAME+=("$name")
    _ROPERO_CAT_SIZE+=("$size")
    _ROPERO_CAT_DEST+=("$dest_dir")
    _ROPERO_CAT_DNAME+=("$dest_name")
    _ROPERO_CAT_AUX+=("")
}

# _cat_safetensors <dir_name> <repo> <modelname> <size> [dest_dir] [excludes]
_cat_safetensors() {
    _ROPERO_CAT_KIND+=("safetensors")
    _ROPERO_CAT_KEY+=("$1")
    _ROPERO_CAT_REPO+=("$2")
    _ROPERO_CAT_ARG2+=("$3")
    _ROPERO_CAT_NAME+=("$3")
    _ROPERO_CAT_SIZE+=("$4")
    _ROPERO_CAT_DEST+=("${5:-}")
    _ROPERO_CAT_DNAME+=("")
    _ROPERO_CAT_AUX+=("")
    _ROPERO_CAT_EXCLUDES+=("${6:-}")
}
_ROPERO_CAT_EXCLUDES=()
_ROPERO_CAT_POSTCOPY=()

# _cat_aux <key> <aux> — cuelga ficheros secundarios de una entrada existente.
#
# <aux> = "ruta/en/repo|nombre final|tamaño|etiqueta;otro|..."
#
# El repo es el de la entrada: los aux son ficheros del MISMO repo (el MTP y el
# mmproj de Gemma4 están junto al GGUF principal, no en un repo aparte).
#
# POR QUÉ NO ES "UNA ENTRADA MÁS" DEL CATÁLOGO
# Un MTP suelto en el catálogo es una entrada seleccionable, y eso miente:
# bajarse el MTP sin el modelo principal no sirve para nada. Y el caso que de
# verdad rompía era peor: las entradas de MTP/mmproj se declaraban DENTRO de
# un `if [[ -e "$GGUF_DIR/el-principal" ]]`, que se evalúa cuando se CONSTRUYE
# el catálogo — es decir, ANTES de descargar el principal. En la primera
# descarga nunca existían; en la segunda ya no hacía falta. Es decir: nunca.
#
# (En Gemma4 el `if` además comparaba "Gemma-4-..." con mayúscula contra el
# fichero real "gemma-4-...", así que en Linux la condición era falsa SIEMPRE.)
_cat_aux() {
    local key="$1" aux="$2" i
    for ((i=0;i<${#_ROPERO_CAT_KEY[@]};i++)); do
        if [[ "${_ROPERO_CAT_KEY[$i]}" == "$key" ]]; then
            _ROPERO_CAT_AUX[$i]="${_ROPERO_CAT_AUX[$i]:+${_ROPERO_CAT_AUX[$i]};}$aux"
            return 0
        fi
    done
    ropero_err "no se pudo colgar el aux de '$key': no está en el catálogo"
    return 1
}

# _ropero_cat_build — rellena el catálogo. Se llama una vez por proceso.
_ropero_cat_build() {
    (( ${#_ROPERO_CAT_KEY[@]} == 0 )) || return 0

    # ── GEMMA4 ──────────────────────────────────────────────────────────────
    # El GGUF principal son 14.2 GB, pero Gemma4 NO es un fichero: el MTP
    # (decoding especulativo) y el mmproj (proyector de visión) son parte del
    # modelo. Los tres se bajan con UN consentimiento, porque con dos de tres
    # llama-server arranca igual y se cae al primer uso (--model-draft y
    # --mmproj apuntan a ficheros que no existen).
    #
    # Los tamaños son los REALES del repo, medidos contra la API de HF:
    #   principal 14.25 GB + mmproj 1.19 GB + mtp 0.25 GB = 15.70 GB
    _cat_gguf "gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf" \
        "unsloth/gemma-4-26B-A4B-it-qat-GGUF" "gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf" "Gemma4" "15.7 GB"

    # El MTP se llama igual en su repo y en el destino (gemma4.sh lo busca por
    # ese nombre). El mmproj se llama "mmproj-BF16.gguf" en TODOS los repos de
    # GGUF multimodal —Qwen3.8 también tiene uno con ese nombre exacto—, así
    # que su nombre final lleva el prefijo del modelo. Si no, el último
    # mmproj descargado pisa al anterior y el primer modelo deja de ver visión.
    _cat_aux "gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf" \
        "mtp-gemma-4-26B-A4B-it.gguf|mtp-gemma-4-26B-A4B-it.gguf|252 MB|Gemma4 MTP;mmproj-BF16.gguf|gemma-4-26B-A4B-it-mmproj-BF16.gguf|1.19 GB|Gemma4 mmproj"    # ── LANE DEL VAULT (posadero) ─────────────────────────────────────────────
    # Los cuatro viven en CPU por diseño (ver los .sh: --n-gpu-layers 0), así
    # que no compiten por la VRAM con el coder. La key de cada entrada es el
    # nombre FINAL del fichero, que es lo que declara MODEL_GGUF en el .sh —
    # si uno de los dos lados cambia y el otro no, `ropero --install models`
    # descarga un fichero que el modelo luego no encuentra.
    _cat_gguf "Qwen3-Embedding-0.6B-Q8_0.gguf" \
        "Qwen/Qwen3-Embedding-0.6B-GGUF" "Qwen3-Embedding-0.6B-Q8_0.gguf" "Qwen3 Embedding 0.6B Q8" "639 MB"

    _cat_gguf "qwen3-reranker-0.6b-q8_0.gguf" \
        "ggml-org/Qwen3-Reranker-0.6B-Q8_0-GGUF" "qwen3-reranker-0.6b-q8_0.gguf" "Qwen3 Reranker 0.6B Q8" "639 MB"

    _cat_gguf "Ministral-3-14B-Instruct-2512-UD-Q5_K_XL.gguf" \
        "unsloth/Ministral-3-14B-Instruct-2512-GGUF" "Ministral-3-14B-Instruct-2512-UD-Q5_K_XL.gguf" "Ministral 3 14B UD-Q5_K_XL" "9.64 GB"

    _cat_gguf "gpt-oss-20b-UD-Q8_K_XL.gguf" \
        "unsloth/gpt-oss-20b-GGUF" "gpt-oss-20b-UD-Q8_K_XL.gguf" "gpt-oss 20B UD-Q8_K_XL" "13.2 GB"

    # El projector de visión de Ministral 3 va aparte y NO se descarga por
    # defecto: son ~800 MB para algo que el 95% de las notas no usa. Para
    # activarlo, descomenta esto y añade `--mmproj` en ministral.sh (que lo
    # documenta):
    #   _cat_gguf "mmproj-ministral-3-14b-f16.gguf" \
    #       "unsloth/Ministral-3-14B-Instruct-2512-GGUF" "mmproj-F16.gguf" "Ministral 3 mmproj" "800 MB"

    # Embeddings + chat
    # _cat_gguf "jina-code-embeddings-1.5b-Q8_0.gguf" \
    #     "jinaai/jina-code-embeddings-1.5b-GGUF" "jina-code-embeddings-1.5b-Q8_0.gguf" "jina-code 1.5B Q8" "1.6 GB"

    # _cat_gguf "openai_gpt-oss-20b-MXFP4.gguf" \
    #     "bartowski/openai_gpt-oss-20b-GGUF-MXFP4-Experimental" "openai_gpt-oss-20b-MXFP4.gguf" "gpt-oss 20B MXFP4" "12.1 GB"

    # _cat_gguf "fable-coder-35B-A3B-Q4_K_M.gguf" \
    #     "Achilles1089/fable-coder-35B-A3B-GGUF" "fable-coder-35B-A3B-Q4_K_M.gguf" "Fable Coder Q4" "21.7 GB"

    # Qwen3.8 también es un bundle: el MTP (que usa fired/qwen.sh con
    # --model-draft) y el mmproj de visión. El MTP vive en el subdirectorio
    # MTP/ del repo, pero el .sh del modelo lo busca en la RAÍZ de GGUF_DIR,
    # así que el nombre final es el plano.
    #
    # Tamaños reales: 13.15 GB + 1.37 GB (MTP) + 0.93 GB (mmproj) = 15.45 GB
    # _cat_gguf "Qwen3.8-27B-UD-Q3_K_XL.gguf" \
    #     "unsloth/Qwen3.8-27B-GGUF" "Qwen3.8-27B-UD-Q3_K_XL.gguf" "Qwen3.8" "15.4 GB"

    # El mmproj de Qwen3.8 se llama "mmproj-BF16.gguf" igual que el de Gemma4:
    # si aterrizara en la raíz de GGUF_DIR, pisaría al de Gemma4 (o al revés).
    # Por eso el nombre final lleva el prefijo del modelo.
    # _cat_aux "Qwen3.8-27B-UD-Q3_K_XL.gguf" \
    #     "MTP/mtp-Qwen3.8-27B-Q4_0.gguf|mtp-Qwen3.8-27B-Q4_0.gguf|1.37 GB|Qwen 3.8 MTP (Q4_0);mmproj-BF16.gguf|qwen3.8-mmproj-BF16.gguf|931 MB|Qwen 3.8 MMPROJ(BF16)"

    _cat_gguf "Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf" \
        "unsloth/Qwen2.5-VL-7B-Instruct-GGUF" "Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf" "Qwen2.5-VL 7B" "6.1 GB"
    # El mmproj también es parte del bundle: el .sh del modelo pasa --mmproj
    # siempre, así que sin él el modelo arranca pero no ve. Su nombre final
    # lleva el prefijo porque "mmproj-F16.gguf" existe también en el repo de
    # Ministral: sin renombrar, uno pisa al otro.
    _cat_aux "Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf" \
        "mmproj-F16.gguf|Qwen2.5-VL-mmproj.gguf|1.35 GB|Qwen2.5-VL mmproj"

    # _cat_gguf "Devstral-Small-2-24B-Instruct-2512-UD-Q4_K_XL.gguf" \
    #     "unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF" "Devstral-Small-2-24B-Instruct-2512-UD-Q4_K_XL.gguf" "Devstral Small UD-Q4_K_XL" "14.5 GB"
    # # IQ3_XXS y no Q4_K_XL: 12,8 GB en vez de 17,7, que en una RTX 5080 de
    # 16 GB es lo que deja sitio para el KV cache del contexto largo. El
    # razonamiento está en qwencoder.sh; si cambias uno, cambia el otro.
    _cat_gguf "Qwen3-Coder-30B-A3B-Instruct-UD-IQ3_XXS.gguf" \
        "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF" "Qwen3-Coder-30B-A3B-Instruct-UD-IQ3_XXS.gguf" "Qwen3 Coder UD-IQ3_XXS" "12.8 GB"
    _cat_gguf "agentica-org_DeepCoder-14B-Preview-Q4_K_L.gguf" \
        "bartowski/agentica-org_DeepCoder-14B-Preview-GGUF" "agentica-org_DeepCoder-14B-Preview-Q4_K_L.gguf" "DeepCoder Q4_K_L" "9.57 GB"

    # _cat_safetensors "Qwen3.8-27B-NVFP4" \
    #     "unsloth/Qwen3.8-27B-NVFP4" "Qwen3.8-27B-NVFP4" "23.4 GB"

    # macOS → mlx_lm. El destino tiene que llamarse EXACTAMENTE como MODEL_PATH en
    # ropero.d/<modelo>.sh, porque el preflight de ropero comprueba ese directorio y
    # dice "no encontrado" si no existe. Si cambias uno, cambias el otro.
    #
    # Por qué ESTE repo y no otro, medido sobre los config.json reales:
    #   · Los expertos MoE son ~19B de los 21B parámetros y van a MXFP4-4 en
    #     todos los candidatos. Es el formato en el que OpenAI post-entrenó el
    #     modelo y con el que hizo TODAS sus evaluaciones.
    #   · "Q8" es los otros 122 tensores (q/k/v/o, router, embeddings, lm_head) a
    #     8 bits en vez de 4. Cuesta 0.8 GB y es donde se nota la cuantización.
    # El directorio DEBE llamarse gpt-oss-20b-MXFP4-Q8.
    _cat_safetensors "gpt-oss-20b-MXFP4-Q8" \
        "mlx-community/gpt-oss-20b-MXFP4-Q8" "gpt-oss-20b-MXFP4-Q8" "12.1 GB"

    # ── Lo que NO se descarga aquí, y por qué ───────────────────────────────
    # ropero.d/airgptoss.sh (engine airllm) necesita ~/models/safetensors/
    # gpt-oss-120b, pero son ~130-240 GB según la variante: no es una descarga
    # que un `--install` pueda hacer por sorpresa. Se deja a mano, y el .sh
    # avisa si no está:
    #
    #   ropero_download_safetensors "openai/gpt-oss-120b" "gpt-oss-120b" "130 GB" \
    #       "$SAFETENSORS_DIR/gpt-oss-120b" "original/*"
    #
    # El 5º argumento (excludes) importa: `openai/gpt-oss-120b` trae `original/`
    # con los pesos bf16 SIN cuantizar junto a la versión cuantizada. Sin excluirlo
    # descargas los dos juegos: el doble de disco para nada.
}

# -----------------------------------------------------------------------------
# ropero_model_key <nombre> — resuelve un nombre de modelo a su key en los
# arrays del loader, o vacío si no es un modelo.
#
# Acepta tanto el nombre del script (devstral) como el alias público
# (coder_lite) y el nombre "humano" que imprime `ropero list`. Devuelve la key
# FÍSICA, no la del alias: para casar contra el catálogo interesa MODEL_GGUF,
# que solo existe en la key física.
#
# Sin esto, `ropero --install models coder_lite` decía "modelo desconocido"
# sobre un modelo que sale en `ropero list` justo encima.
# -----------------------------------------------------------------------------
ropero_model_key() {
    local want="${1:-}" mk
    [[ -n "$want" ]] || return 1
    for mk in "${MODEL_NAMES[@]}"; do
        # La key física se normaliza igual que al cargar (guiones → guiones bajos).
        if [[ "$mk" == "${want//-/_}" ]]; then
            printf '%s\n' "$mk"; return 0
        fi
    done
    for mk in "${MODEL_NAMES[@]}"; do
        if [[ "${MODEL_ALIAS_TARGET[$mk]:-}" == "${want//-/_}" ]]; then
            printf '%s\n' "$mk"; return 0
        fi
    done
    for mk in "${MODEL_NAMES[@]}"; do
        if [[ "${MODEL_ALIAS[$mk]:-$mk}" == "$want" ]]; then
            printf '%s\n' "$mk"; return 0
        fi
    done
    return 1
}

# -----------------------------------------------------------------------------
# _ropero_cat_index <filtro> — imprime los índices del catálogo que casan con
# el filtro. Sin filtro, todos. El filtro puede ser:
#   · el nombre final del fichero (Qwen3.8-27B-UD-Q3_K_XL.gguf)
#   · el nombre del directorio de un checkpoint
#   · el nombre legible ("Qwen3.8", "DeepCoder Q4_K_L")
#   · un alias o script de modelo (qwen, devstral, coder_lite…) → se resuelve
#     antes contra MODEL_GGUF / MODEL_PATH
# -----------------------------------------------------------------------------
_ropero_cat_index() {
    local filter="${1:-}" i n key matched
    _ropero_cat_build
    n=${#_ROPERO_CAT_KEY[@]}

    [[ -n "$filter" ]] || { for ((i=0;i<n;i++)); do printf '%s\n' "$i"; done; return 0; }

    # 1) El filtro puede ser un alias/script de modelo. Entonces lo que vale
    #    es su MODEL_GGUF (o MODEL_PATH para los de servidor propio), no el
    #    nombre del script.
    local mk
    mk="$(ropero_model_key "$filter")"
    if [[ -n "$mk" ]]; then
        local want_gguf="${MODEL_GGUF[$mk]:-}" want_path="${MODEL_PATH[$mk]:-}"
        local base_gguf="" base_dir=""
        [[ -n "$want_gguf" ]] && base_gguf="$(basename "$want_gguf")"
        [[ -n "$want_path" ]] && base_dir="$(basename "$want_path")"

        for ((i=0;i<n;i++)); do
            key="${_ROPERO_CAT_KEY[$i]}"
            if [[ -n "$base_gguf" && "$key" == "$base_gguf" ]] \
               || [[ -n "$base_dir"  && "$key" == "$base_dir" ]]; then
                printf '%s\n' "$i"
                return 0
            fi
        done
        # Un modelo que existe pero cuyo GGUF no está en el catálogo: no es un
        # error de quien escribe, es un modelo que ropero no descarga. Se dice
        # explícitamente en vez de "modelo desconocido", que mentiría.
        ropero_warn "'$filter' es un modelo conocido pero su fichero (${base_gguf:-${base_dir:-?}}) no está en el catálogo de ropero"
        return 1
    fi

    # 2) Casado directo contra clave o nombre legible.
    local lf; lf="$(printf '%s' "$filter" | tr '[:upper:]' '[:lower:]')"
    for ((i=0;i<n;i++)); do
        key="${_ROPERO_CAT_KEY[$i]}"
        local lname; lname="$(printf '%s' "${_ROPERO_CAT_NAME[$i]}" | tr '[:upper:]' '[:lower:]')"
        if [[ "$(printf '%s' "$key" | tr '[:upper:]' '[:lower:]')" == "$lf" || "$lname" == "$lf" ]]; then
            printf '%s\n' "$i"
            return 0
        fi
    done
    matched=""
    return 1
}

# -----------------------------------------------------------------------------
# _ropero_cat_dest_dir <índice> — directorio final de una entrada, ya resuelto
# (default $GGUF_DIR y "~/..." expandido). Lo usan tanto la descarga como las
# comprobaciones de "ya está en disco", para que ambas miren el MISMO sitio.
# -----------------------------------------------------------------------------
_ropero_cat_dest_dir() {
    local i="$1" dest="${_ROPERO_CAT_DEST[$i]:-}"
    [[ -n "$dest" ]] || dest="${GGUF_DIR:-$HOME/models/gguf}"
    case "$dest" in
        "~")     dest="$HOME" ;;
        "~/"*)   dest="$HOME/${dest#\~/}" ;;
    esac
    printf '%s\n' "$dest"
}

# _ropero_cat_dest_name <índice> — nombre final del fichero principal.
_ropero_cat_dest_name() {
    printf '%s\n' "${_ROPERO_CAT_DNAME[$1]:-${_ROPERO_CAT_ARG2[$1]:-}}"
}

# -----------------------------------------------------------------------------
# _ropero_cat_aux_paths <índice> — rutas finales de los ficheros SECUNDARIOS
# de la entrada, uno por línea. Vacío si la entrada no tiene aux.
# -----------------------------------------------------------------------------
_ropero_cat_aux_paths() {
    local i="$1" spec dest_dir rest name
    local specs="${_ROPERO_CAT_AUX[$i]:-}"
    [[ -n "$specs" ]] || return 0
    dest_dir="$(_ropero_cat_dest_dir "$i")"
    local -a list=()
    local IFS=';'
    # shellcheck disable=SC2206
    local -a parts=($specs)
    unset IFS
    for spec in "${parts[@]}"; do
        [[ -n "$spec" ]] || continue
        rest="${spec#*|}"
        name="${rest%%|*}"
        [[ -n "$name" ]] || continue
        printf '%s/%s\n' "$dest_dir" "$name"
    done
}

# -----------------------------------------------------------------------------
# _ropero_cat_download_aux <índice> — descarga los ficheros SECUNDARIOS (MTP,
# mmproj) de una entrada.
#
# POR QUÉ UN STAGING Y NO DIRECTO AL DESTINO
# El projector de Gemma4 se llama "mmproj-BF16.gguf" en su repo y el de
# Qwen3.8 se llama EXACTAMENTE IGUAL en el suyo. Descargados directos a
# $GGUF_DIR, el segundo sobrescribe al primero: dos modelos, un fichero, y el
# que se rompió no dice nada (además el nombre "mmproj-BF16.gguf" no es el que
# busca ningún .sh de modelo, así que el fallo aparece al arrancar y no al
# descargar). Por eso se baja a un temporal y de ahí se mueve al nombre final
# con el prefijo del modelo.
#
# EL CONSENTIMIENTO ES EL DEL PRINCIPAL
# `ropero install models gemma4` pregunta UNA vez por el bundle entero: si
# aceptas el GGUF de 14 GB, aceptas el MTP de 252 MB y el mmproj de 1.19 GB
# que sin él no sirven para nada. Por eso AUTO_YES=1 en local durante el
# bundle: si no, el descargador preguntaría por cada fichero.
#
# Si el principal NO está (usuario lo rechaza, o la descarga falla), los aux
# no se tocan: un MTP sin modelo es un fichero de 252 MB que no sirve para
# nada y confunde el resumen.
# -----------------------------------------------------------------------------
_ropero_cat_download_aux() {
    local i="$1" specs="${_ROPERO_CAT_AUX[$i]:-}"
    [[ -n "$specs" ]] || return 0

    local repo="${_ROPERO_CAT_REPO[$i]}" dest_dir="$(_ropero_cat_dest_dir "$i")"
    local spec src name size label staged staged_file final rc=0
    local -a parts=()
    local IFS=';'
    # shellcheck disable=SC2206
    parts=($specs)
    unset IFS

    for spec in "${parts[@]}"; do
        [[ -n "$spec" ]] || continue
        src="${spec%%|*}"; spec="${spec#*|}"
        name="${spec%%|*}"; spec="${spec#*|}"
        size="${spec%%|*}"; label="${spec#*|}"
        [[ -n "$src" && -n "$name" ]] || continue
        final="$dest_dir/$name"

        if [[ -s "$final" && "${ROPERO_FORCE:-0}" != "1" ]]; then
            ropero_ok "$label ya descargado"
            continue
        fi

        staged="$(mktemp -d "${TMPDIR:-/tmp}/ropero-aux.XXXXXXXX")" \
            || { ropero_err "no se pudo crear el temporal para $label"; return 1; }
        # `local` + subshell: el consentimiento del principal cubre el bundle.
        local AUTO_YES=1
        if ! ropero_download_gguf "$repo" "$src" "$label" "$size" "$staged" "$(basename "$name")"; then
            rm -rf "$staged" 2>/dev/null || true
            return 1
        fi
        staged_file="$staged/$(basename "$name")"
        if [[ ! -s "$staged_file" ]]; then
            ropero_err "$label: no aparece $(basename "$name") tras la descarga"
            rm -rf "$staged" 2>/dev/null || true
            return 1
        fi
        if [[ ! -d "$dest_dir" ]] && ! mkdir -p "$dest_dir"; then
            ropero_err "no se pudo crear $dest_dir"
            rm -rf "$staged" 2>/dev/null || true
            return 1
        fi
        # mv y no cp: el fichero descargado no se queda ahí occupying el doble.
        if ! mv -f "$staged_file" "$final"; then
            ropero_err "no se pudo mover $label a $final"
            rm -rf "$staged" 2>/dev/null || true
            return 1
        fi
        rm -rf "$staged" 2>/dev/null || true
        [[ -s "$final" ]] || { ropero_err "$label: no quedó en $final"; return 1; }
        ropero_ok "$label → $name"
    done
    return "$rc"
}

# -----------------------------------------------------------------------------
# _ropero_cat_download <índice> — descarga una entrada del catálogo: el
# fichero principal y, si tiene, sus ficheros secundarios (bundle completo).
# -----------------------------------------------------------------------------
_ropero_cat_download() {
    local i="$1"
    _ropero_cat_build

    if [[ "${_ROPERO_CAT_KIND[$i]}" == "safetensors" ]]; then
        ropero_download_safetensors \
            "${_ROPERO_CAT_REPO[$i]}" "${_ROPERO_CAT_ARG2[$i]}" "${_ROPERO_CAT_SIZE[$i]}" \
            "${_ROPERO_CAT_DEST[$i]}" "${_ROPERO_CAT_EXCLUDES[$i]}"
        return $?
    fi

    local dest_dir dest_name final
    dest_dir="$(_ropero_cat_dest_dir "$i")"
    dest_name="$(_ropero_cat_dest_name "$i")"
    final="$dest_dir/$dest_name"

    ropero_download_gguf \
        "${_ROPERO_CAT_REPO[$i]}" "${_ROPERO_CAT_ARG2[$i]}" "${_ROPERO_CAT_NAME[$i]}" \
        "${_ROPERO_CAT_SIZE[$i]}" "${_ROPERO_CAT_DEST[$i]}" "${_ROPERO_CAT_DNAME[$i]}" || return 1

    # El descargador devuelve 0 también cuando el usuario RECHAZA la descarga.
    # Sin este check, un "no" del usuario se tomaría por un bundle descargado
    # y los aux se bajarían igualmente (252 MB + 1.19 GB que nadie pidió).
    if [[ ! -s "$final" ]]; then
        ropero_err "${_ROPERO_CAT_NAME[$i]}: el fichero principal ($dest_name) no está en disco"
        return 1
    fi

    _ropero_cat_download_aux "$i"
}

# -----------------------------------------------------------------------------
# ropero_models_catalog — lista lo que `ropero --install models` puede instalar.
# Se usa también en el mensaje de error de `models <algo>`: sin esto, un typo
# responde "modelo desconocido" sin decir nunca qué SÍ es válido.
# -----------------------------------------------------------------------------
ropero_models_catalog() {
    local i n
    _ropero_cat_build
    n=${#_ROPERO_CAT_KEY[@]}
    for ((i=0;i<n;i++)); do
        printf '  %-46s %-8s %s\n' "${_ROPERO_CAT_KEY[$i]}" \
            "${_ROPERO_CAT_SIZE[$i]}" "${_ROPERO_CAT_NAME[$i]}"
    done
}

# -----------------------------------------------------------------------------
# ropero_download_default_models [filtro]
#
# Sin filtro: el catálogo entero, preguntando uno a uno (los que ya están se
# saltan solos). Con filtro: solo ese, y SIN preguntar — es lo que pide
# explícitamente un nombre, y es lo que hace Lack-of-preguntas automatizable
# desde repos.yaml.
# -----------------------------------------------------------------------------
ropero_download_default_models() {
    local filter="${1:-}" os idx rc=0

    _ropero_cat_build

    if [[ -n "$filter" ]]; then
        if ! idx="$(_ropero_cat_index "$filter")"; then
            ropero_err "no sé qué modelo es '$filter'"
            ropero_info "  los que ropero puede descargar:"
            ropero_models_catalog >&2
            return 1
        fi
        ropero_info "descargando solo: ${_ROPERO_CAT_NAME[$idx]} (${_ROPERO_CAT_KEY[$idx]})"
        ROPERO_FORCE="${ROPERO_FORCE:-0}"
        _ropero_cat_download "$idx" || return $?
        _ropero_cat_postcopy
        return $?
    fi

    os=$(ropero_detect_os 2>/dev/null || echo unknown)

    ropero_info "-- modelos GGUF (llama.cpp) --"
    local i n
    n=${#_ROPERO_CAT_KEY[@]}
    for ((i=0;i<n;i++)); do
        [[ "${_ROPERO_CAT_KIND[$i]}" == "gguf" ]] || continue
        _ropero_cat_download "$i" || rc=1
    done

    # El set de safetensors es PLATAFORM-DEPENDIENTE y va aparte a propósito:
    # el MLX solo corre en Apple Silicon y un checkpoint de 12 GB que en Cachy no
    # sirve de nada no se descarga. Meterlo en la lista común convertiría
    # `ropero --install` en 12 GB de descarga inútil en Linux, que es justo lo
    # que un install no debe hacer.
    if [[ "$os" == "macos" ]]; then
        ropero_info "-- checkpoints safetensors (mlx_lm) --"
        for ((i=0;i<n;i++)); do
            [[ "${_ROPERO_CAT_KIND[$i]}" == "safetensors" ]] || continue
            _ropero_cat_download "$i" || rc=1
        done
    else
        ropero_info "safetensors/MLX: solo macOS (mlx_lm es Apple Silicon). Omitido en $os."
    fi

    (( rc == 0 )) || return "$rc"
    _ropero_cat_postcopy
}

# -----------------------------------------------------------------------------
# _ropero_cat_postcopy — copias que hacen falta DESPUÉS de tener el fichero.
# El MTP se deja además en la raíz de GGUF porque es donde lo buscan los .sh de
# modelo. Antes vivía dentro del bloque condicional que lo descargaba; al
# mover la descarga a un catálogo, la copia tenía que moverse con ella o se
# perdía en silencio.
# -----------------------------------------------------------------------------
_ropero_cat_postcopy() {
    local spec src dst
    (( ${#_ROPERO_CAT_POSTCOPY[@]} == 0 )) && return 0
    for spec in "${_ROPERO_CAT_POSTCOPY[@]}"; do
        src="${spec%%:*}"; dst="${spec#*:}"
        if [[ ! -f "$src" ]]; then
            ropero_err "falta el origen requerido para postcopy: $src"
            return 1
        fi
        if ! cp "$src" "$dst" 2>/dev/null; then
            ropero_err "no se pudo copiar $src a $dst"
            return 1
        fi
        if [[ ! -f "$dst/$(basename "$src")" ]]; then
            ropero_err "postcopy no dejó $(basename "$src") en $dst"
            return 1
        fi
        ropero_ok "$(basename "$src") copiado a $dst"
    done
    return 0
}

# -----------------------------------------------------------------------------
# Compatibilidad: la fase llama a _ropero_download_safetensors_mlx. Se mantiene
# como nombre para que nada que la invoque directamente se rompa.
# -----------------------------------------------------------------------------
_ropero_download_safetensors_mlx() {
    local os="${1:-}" i n
    [[ "$os" == "macos" ]] || return 0
    _ropero_cat_build
    n=${#_ROPERO_CAT_KEY[@]}
    for ((i=0;i<n;i++)); do
        [[ "${_ROPERO_CAT_KIND[$i]}" == "safetensors" ]] || continue
        _ropero_cat_download "$i"
    done
}
