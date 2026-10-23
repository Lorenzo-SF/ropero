#!/usr/bin/env bash
# bin/21-download-models.sh
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
# Modelos fuera del scope base movidos a models/<engine>/fired/ y
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
# Excludes: patrones a excluir de la descarga de un checkpoint. Sólo los
# modelos con checkpoint (safetensors) los tienen.
_ROPERO_CAT_EXCLUDES=()
_ROPERO_CAT_POSTCOPY=()

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
    # Los excludes son de los checkpoints (safetensors). Aun así hay que
    # empujar una posición: si _cat_gguf no lo hace, el array EXCLUDES queda
    # desfasado respecto a los demás, y el exclude de un checkpoint se leería
    # de la entrada equivocada.
    _ROPERO_CAT_EXCLUDES+=("")
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
# -----------------------------------------------------------------------------
# _cat_aux <key> <aux> — cuelga ficheros secundarios de una entrada existente.
#
# <aux> = "repo|ruta/en/repo|nombre final|tamaño|etiqueta;otro|..."
#
# ── POR QUÉ NO ES "UNA ENTRADA MÁS" DEL CATÁLOGO ─────────────────────────────
# Un MTP suelto en el catálogo es una entrada seleccionable, y eso miente:
# bajarse el MTP sin el modelo principal no sirve para nada. Y el caso que de
# verdad rompía era peor: las entradas de MTP/mmproj se declaraban DENTRO de
# un `if [[ -e "$GGUF_DIR/el-principal" ]]`, que se evalúa cuando se CONSTRUYE
# el catálogo — es decir, ANTES de descargar el principal. En la primera
# descarga nunca existían; en la segunda ya no hacía falta. Es decir: nunca.
#
# (En Gemma4 el `if` además comparaba "Gemma-4-..." con mayúscula contra el
# fichero real "gemma-4-...", así que en Linux la condición era falsa SIEMPRE.)
#
# El repo viene en CADA aux porque el TOML lo declara así: el MTP y el mmproj
# son ficheros del repo del modelo, pero declararlo en el dato evita que este
# fichero tenga que suponerlo.
# -----------------------------------------------------------------------------
_cat_aux() {
    local key="$1" aux="$2" i
    for ((i=0;i<${#_ROPERO_CAT_KEY[@]};i++)); do
        if [[ "${_ROPERO_CAT_KEY[$i]}" == "$key" ]]; then
            _ROPERO_CAT_AUX[$i]="$aux"
            return 0
        fi
    done
    ropero_err "no se pudo colgar el aux de '$key': no está en el catálogo"
    return 1
}

# -----------------------------------------------------------------------------
# _ropero_cat_aux_join <spec_actual> <spec_nuevo> <etiqueta>
#
# Concatena dos specs de aux. El spec del TOML viene como "repo|fichero|nombre
# |tamaño" (el formato del cargador) y el del catálogo como
# "repo|fichero|nombre|tamaño|etiqueta": sólo falta la etiqueta.
#
# Si el spec nuevo está vacío, devuelve el actual sin tocarlo.
# -----------------------------------------------------------------------------
_ropero_cat_aux_join() {
    local current="${1:-}" incoming="${2:-}" label="${3:-}"
    [[ -n "$incoming" ]] || { printf '%s' "$current"; return 0; }
    local repo file name size
    IFS='|' read -r repo file name size <<<"$incoming"
    [[ -n "$file" ]] || { printf '%s' "$current"; return 0; }
    [[ -n "$name" ]] && name="${name##*/}"
    local entry="$repo|$file|$name|$size|$label"
    if [[ -n "$current" ]]; then
        printf '%s;%s' "$current" "$entry"
    else
        printf '%s' "$entry"
    fi
}

# -----------------------------------------------------------------------------
# _ropero_cat_build — rellena el catálogo. Se llama una vez por proceso.
#
# ── POR QUÉ AQUÍ NO HAY NI UN DATO ───────────────────────────────────────────
#
# Antes este fichero ERA el catálogo: una llamada a `_cat_gguf` por modelo, con
# el repo, el nombre del fichero y el tamaño escritos a mano. Y los modelos
# estaban además en `bin/<modelo>.sh`, que declaraba el GGUF que iba a
# buscar. Dos listas del mismo modelo en dos sitios, y la consecuencia era
#Known: cuando una se actualizaba y la otra no, `ropero install models`
# descargaba un fichero que el modelo luego no encontraba. Un fallo que no se
# ve hasta que arrancas el modelo y llama-server se queja de que no hay
# pesos.
#
# Ahora el catálogo se construye con lo que el loader ya ha leído de
# `models/<engine>/*.toml`: el repo, el fichero, el tamaño, el nombre final en
# disco y los ficheros auxiliares (MTP, mmproj) salen de ahí. Una sola lista.
#
# Y sólo de los modelos ACTIVOS: los que están en `fired/` no se descargan,
# porque no se pueden arrancar.
#
# El `manual = true` de un TOML excluye el modelo del catálogo: existe y se
# puede arrancar, pero su descarga no se ofrece (son 130 GB de checkpoint).
# -----------------------------------------------------------------------------
_ropero_cat_build() {
    (( ${#_ROPERO_CAT_KEY[@]} == 0 )) || return 0

    local m key repo aux_spec
    for m in "${MODEL_NAMES[@]}"; do
        # Los aliases no son entradas: comparten el modelo físico, y su GGUF
        # ya está en la entrada física. Meterlos dos veces descargaría lo
        # mismo dos veces.
        [[ -n "${MODEL_ALIAS_TARGET[$m]:-}" ]] && continue

        # Descarga manual: no se ofrece en `ropero install models`.
        if [[ "${MODEL_MANUAL[$m]:-0}" == "1" && "${ROPERO_CAT_INCLUDE_MANUAL:-0}" != "1" ]]; then continue; fi

        repo="${MODEL_REPO[$m]:-}"
        [[ -n "$repo" ]] || continue

        case "${MODEL_FORMAT[$m]:-gguf}" in
            safetensors)
                # El key es el nombre del DIRECTORIO, que es lo que declara
                # [model].path del TOML y lo que comprueba el preflight.
                key="${MODEL_PATH[$m]##*/}"
                [[ -n "$key" ]] || continue
                _cat_safetensors "$key" "$repo" "$key" \
                                "${MODEL_SIZE[$m]:-?}" "" "${MODEL_EXCLUDES[$m]:-}"
                ;;
            *)
                # El key es el nombre FINAL del GGUF: lo que declara [files].gguf
                # y lo que el modelo va a buscar en $GGUF_DIR.
                key="${MODEL_GGUF[$m]:-}"
                [[ -n "$key" ]] || continue
                _cat_gguf "$key" "$repo" "${MODEL_FILE[$m]:-$key}" \
                          "${MODEL_ALIAS[$m]:-$m}" "${MODEL_SIZE[$m]:-?}" "$GGUF_DIR/${MODEL_ALIAS[$m]:-$m}" "$key"
                ;;
        esac

        # ── Ficheros auxiliares (MTP, mmproj) ────────────────────────────
        # Son parte del bundle: sin ellos el modelo arranca pero se cae al
        # primer uso (--model-draft y --mmproj apuntan a ficheros que no
        # existen). Por eso NO son entradas seleccionables del catálogo: se
        # cuelgan de la entrada que los declara.
        aux_spec=""
        aux_spec="$(_ropero_cat_aux_join "$aux_spec" "${MODEL_AUX_MTP[$m]:-}" "MTP")"
        aux_spec="$(_ropero_cat_aux_join "$aux_spec" "${MODEL_AUX_MMPROJ[$m]:-}" "mmproj")"
        aux_spec="$(_ropero_cat_aux_join "$aux_spec" "${MODEL_AUX_CHAT_TEMPLATE[$m]:-}" "chat_template")"
        if [[ -n "$aux_spec" ]]; then
            _cat_aux "$key" "$aux_spec"
        fi
    done
    return 0
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
    ROPERO_CAT_INCLUDE_MANUAL=1 _ropero_cat_build
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
    # Dos `local` y no uno: bash expande todas las palabras de un `local`
    # antes de asignar, así que el subíndice se expande con $i sin valor.
    local i="$1"
    local dest="${_ROPERO_CAT_DEST[$i]:-}"
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
        # Formato del aux: "repo|ruta/en/repo|nombre final|tamaño|etiqueta".
        # El nombre final es el TERCER campo.
        rest="${spec#*|}"          # quita el repo
        rest="${rest#*|}"          # quita la ruta dentro del repo
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
    local i="$1"
    local specs="${_ROPERO_CAT_AUX[$i]:-}"
    [[ -n "$specs" ]] || return 0

    local repo="${_ROPERO_CAT_REPO[$i]}" dest_dir="$(_ropero_cat_dest_dir "$i")"
    local spec aux_repo src name size label staged staged_file final rc=0
    local -a parts=()
    local IFS=';'
    # shellcheck disable=SC2206
    parts=($specs)
    unset IFS

    for spec in "${parts[@]}"; do
        [[ -n "$spec" ]] || continue
        # "repo|ruta/en/repo|nombre final|tamaño|etiqueta"
        aux_repo="${spec%%|*}"; spec="${spec#*|}"
        src="${spec%%|*}"; spec="${spec#*|}"
        name="${spec%%|*}"; spec="${spec#*|}"
        size="${spec%%|*}"; label="${spec#*|}"
        # El aux puede venir de otro repo: el que declara el TOML manda.
        [[ -n "$aux_repo" ]] || aux_repo="$repo"
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
        if ! ropero_download_gguf "$aux_repo" "$src" "$label" "$size" "$staged" "$(basename "$name")"; then
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

    local _name="${_ROPERO_CAT_NAME[$i]:-}"
    ropero_info "$_name: descargando..."

    if [[ "${_ROPERO_CAT_KIND[$i]}" == "safetensors" ]]; then
        if ropero_download_safetensors \
            "${_ROPERO_CAT_REPO[$i]}" "${_ROPERO_CAT_ARG2[$i]}" "${_ROPERO_CAT_SIZE[$i]}" \
            "${_ROPERO_CAT_DEST[$i]}" "${_ROPERO_CAT_EXCLUDES[$i]}"; then
            ropero_ok "$_name: 100% completado"
            return 0
        else
            ropero_err "$_name: error en la descarga"
            return 1
        fi
    fi

    local dest_dir dest_name final
    dest_dir="$(_ropero_cat_dest_dir "$i")"
    dest_name="$(_ropero_cat_dest_name "$i")"
    final="$dest_dir/$dest_name"

    if ! ropero_download_gguf \
        "${_ROPERO_CAT_REPO[$i]}" "${_ROPERO_CAT_ARG2[$i]}" "${_ROPERO_CAT_NAME[$i]}" \
        "${_ROPERO_CAT_SIZE[$i]}" "${_ROPERO_CAT_DEST[$i]}" "${_ROPERO_CAT_DNAME[$i]}"; then
        ropero_err "$_name: error en la descarga"
        return 1
    fi

    # El descargador devuelve 0 también cuando el usuario RECHAZA la descarga.
    if [[ ! -s "$final" ]]; then
        ropero_err "${_ROPERO_CAT_NAME[$i]}: el fichero principal ($dest_name) no está en disco"
        ropero_err "$_name: error en la descarga"
        return 1
    fi

    if _ropero_cat_download_aux "$i"; then
        ropero_ok "$_name: 100% completado"
        return 0
    else
        ropero_err "$_name: error en la descarga"
        return 1
    fi
}

# -----------------------------------------------------------------------------
# ropero_models_catalog — lista lo que `ropero --install models` puede instalar.
# Se usa también en el mensaje de error de `models <algo>`: sin esto, un typo
# responde "modelo desconocido" sin decir nunca qué SÍ es válido.
# -----------------------------------------------------------------------------
ropero_models_catalog() {
    local i n
    ROPERO_CAT_INCLUDE_MANUAL=1 _ropero_cat_build
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

    ROPERO_CAT_INCLUDE_MANUAL=1 _ropero_cat_build

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
    ROPERO_CAT_INCLUDE_MANUAL=1 _ropero_cat_build
    n=${#_ROPERO_CAT_KEY[@]}
    for ((i=0;i<n;i++)); do
        [[ "${_ROPERO_CAT_KIND[$i]}" == "safetensors" ]] || continue
        _ropero_cat_download "$i"
    done
}
