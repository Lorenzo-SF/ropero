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
    _ROPERO_CAT_EXCLUDES+=("${6:-}")
}
_ROPERO_CAT_EXCLUDES=()
_ROPERO_CAT_POSTCOPY=()

# _ropero_cat_build — rellena el catálogo. Se llama una vez por proceso.
_ropero_cat_build() {
    (( ${#_ROPERO_CAT_KEY[@]} == 0 )) || return 0

    # ── LANE DEL VAULT (posadero) ─────────────────────────────────────────────
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
    _cat_gguf "jina-code-embeddings-1.5b-Q8_0.gguf" \
        "jinaai/jina-code-embeddings-1.5b-GGUF" "jina-code-embeddings-1.5b-Q8_0.gguf" "jina-code 1.5B Q8" "1.6 GB"

    _cat_gguf "openai_gpt-oss-20b-MXFP4.gguf" \
        "bartowski/openai_gpt-oss-20b-GGUF-MXFP4-Experimental" "openai_gpt-oss-20b-MXFP4.gguf" "gpt-oss 20B MXFP4" "12.1 GB"

    _cat_gguf "fable-coder-35B-A3B-Q4_K_M.gguf" \
        "Achilles1089/fable-coder-35B-A3B-GGUF" "fable-coder-35B-A3B-Q4_K_M.gguf" "Fable Coder Q4" "21.7 GB"

    _cat_gguf "Qwen3.8-27B-UD-Q3_K_XL.gguf" \
        "unsloth/Qwen3.8-27B-GGUF" "Qwen3.8-27B-UD-Q3_K_XL.gguf" "Qwen3.8" "13.1 GB"

    # Los tres siguientes solo si el MTP ya está en disco: son el juego de
    # projector/multimodal del Qwen y sin el MTP base no sirven de nada.
    if [[ -e "$HOME/models/gguf/MTP/mtp-Qwen3.8-27B-Q4_0.gguf" ]]; then
        _cat_gguf "mtp-Qwen3.8-27B-Q4_0.gguf" \
            "unsloth/Qwen3.8-27B-GGUF" "MTP/mtp-Qwen3.8-27B-Q4_0.gguf" "Qwen 3.8 MTP (Q4_0)" "1.37 GB"
        _cat_gguf "qwen3.8-mmproj-BF16.gguf" \
            "unsloth/Qwen3.8-27B-GGUF" "mmproj-BF16.gguf" "Qwen 3.8 MMPROJ(BF16)" "0.9 GB"
        # El MTP además se copia a la raíz de GGUF: es donde lo buscan los
        # .sh de modelo, no dentro de MTP/.
        _ROPERO_CAT_POSTCOPY+=("$HOME/models/gguf/MTP/mtp-Qwen3.8-27B-Q4_0.gguf:$HOME/models/gguf/")
    fi

    _cat_gguf "Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf" \
        "unsloth/Qwen2.5-VL-7B-Instruct-GGUF" "Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf" "Qwen2.5-VL 7B" "4.79 GB"
    _cat_gguf "Qwen2.5-VL-mmproj.gguf" \
        "unsloth/Qwen2.5-VL-7B-Instruct-GGUF" "mmproj-F16.gguf" "Qwen2.5-VL mmproj" "1.35 GB" "$HOME/models/gguf" "Qwen2.5-VL-mmproj.gguf"

    _cat_gguf "Devstral-Small-2-24B-Instruct-2512-UD-Q4_K_XL.gguf" \
        "unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF" "Devstral-Small-2-24B-Instruct-2512-UD-Q4_K_XL.gguf" "Devstral Small UD-Q4_K_XL" "14.5 GB"
    # IQ3_XXS y no Q4_K_XL: 12,8 GB en vez de 17,7, que en una RTX 5080 de
    # 16 GB es lo que deja sitio para el KV cache del contexto largo. El
    # razonamiento está en qwencoder.sh; si cambias uno, cambia el otro.
    _cat_gguf "Qwen3-Coder-30B-A3B-Instruct-UD-IQ3_XXS.gguf" \
        "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF" "Qwen3-Coder-30B-A3B-Instruct-UD-IQ3_XXS.gguf" "Qwen3 Coder UD-IQ3_XXS" "12.8 GB"
    _cat_gguf "agentica-org_DeepCoder-14B-Preview-Q4_K_L.gguf" \
        "bartowski/agentica-org_DeepCoder-14B-Preview-GGUF" "agentica-org_DeepCoder-14B-Preview-Q4_K_L.gguf" "DeepCoder Q4_K_L" "9.57 GB"

    _cat_safetensors "Qwen3.8-27B-NVFP4" \
        "unsloth/Qwen3.8-27B-NVFP4" "Qwen3.8-27B-NVFP4" "23.4 GB"

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
# _ropero_cat_download <índice> — descarga una entrada del catálogo.
# -----------------------------------------------------------------------------
_ropero_cat_download() {
    local i="$1"
    _ropero_cat_build
    if [[ "${_ROPERO_CAT_KIND[$i]}" == "safetensors" ]]; then
        ropero_download_safetensors \
            "${_ROPERO_CAT_REPO[$i]}" "${_ROPERO_CAT_ARG2[$i]}" "${_ROPERO_CAT_SIZE[$i]}" \
            "${_ROPERO_CAT_DEST[$i]}" "${_ROPERO_CAT_EXCLUDES[$i]}"
    else
        ropero_download_gguf \
            "${_ROPERO_CAT_REPO[$i]}" "${_ROPERO_CAT_ARG2[$i]}" "${_ROPERO_CAT_NAME[$i]}" \
            "${_ROPERO_CAT_SIZE[$i]}" "${_ROPERO_CAT_DEST[$i]}" "${_ROPERO_CAT_DNAME[$i]}"
    fi
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
    local filter="${1:-}" os idx

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
        _ropero_cat_download "$idx"
        return $?
    fi

    os=$(ropero_detect_os 2>/dev/null || echo unknown)

    ropero_info "-- modelos GGUF (llama.cpp) --"
    local i n
    n=${#_ROPERO_CAT_KEY[@]}
    for ((i=0;i<n;i++)); do
        [[ "${_ROPERO_CAT_KIND[$i]}" == "gguf" ]] || continue
        _ropero_cat_download "$i"
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
            _ropero_cat_download "$i"
        done
    else
        ropero_info "safetensors/MLX: solo macOS (mlx_lm es Apple Silicon). Omitido en $os."
    fi

    _ropero_cat_postcopy
    return 0
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
        [[ -f "$src" ]] || continue
        cp "$src" "$dst" 2>/dev/null \
            && ropero_ok "$(basename "$src") copiado a $dst" \
            || ropero_warn "no se pudo copiar $src a $dst"
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