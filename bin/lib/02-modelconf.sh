#!/usr/bin/env bash
# =============================================================================
# 02-modelconf.sh — wrapper bash del catálogo de modelos (spec §5.3)
# =============================================================================
# Envuelve a bin/python/modelconf.py (la fuente de verdad del esquema TOML).
# Este módulo NO re-implementa el parser: lee SU dump (formato v1, separador
# 0x1F, 31 campos por registro) y lo deja en arrays paralelas.
#
# POR QUÉ ARRAYS PARALELAS Y NO ASSOCIATIVAS
#   bash 3.2 (ADR-1) no tiene `declare -A`. El índice es la POSICIÓN en
#   MODEL_NAMES y las filas son densas (sólo se añaden por el final), así que
#   un bucle C `for ((i=0; i<${#MODEL_NAMES[@]}; i++))` es seguro incluso con
#   `set -u` (la expansión de array vacío es lo que aborta en 3.2, y aquí no
#   se da: se comprueba la longitud antes de expandir).
#
# CONTRATO (spec §5.3):
#   rp_mc_load                        carga el catálogo (idempotente)
#   rp_model_exists <nombre>          0/1 (incluye alias y fired)
#   rp_model_field <nombre> <campo>   valor por stdout
#   rp_model_argv <n> [nivel] [flags] [k=v]...   argv, un argumento por línea
#   rp_model_build_argv <n> [--out r] argv del gguffer (passthrough)
#   rp_model_aux <n>                  líneas tipo|repo|file|name|size|use
#
# Seguro de sourcear: sólo definiciones y defaults, cero ejecución.
# =============================================================================

# Raíz de modelos y ruta del parser. $ROPERO_ROOT la resuelve el dispatcher.
: "${RP_MODELS_DIR:=${ROPERO_ROOT:-.}/models}"
: "${RP_MODELCONF_PY:=${ROPERO_ROOT:-.}/bin/python/modelconf.py}"

# rp_mc_load está ya hecho (idempotencia: un dump por proceso, no por llamada).
RP_MC_LOADED=0

# Arrays paralelas del catálogo. Índice = posición en MODEL_NAMES.
# Las filas de ALIAS van DETRÁS de las reales: copian los campos del modelo
# real y marcan MODEL_ISALIAS=1 / MODEL_REAL=<nombre real>.
MODEL_NAMES=()
MODEL_ENGINE=()
MODEL_FORMAT=()
MODEL_CONTEXT=()
MODEL_PORT=()
MODEL_PATH=()
MODEL_ALIAS=()
MODEL_ORIGIN=()
MODEL_GGUF=()
MODEL_MTP=()
MODEL_MMPROJ=()
MODEL_AUX_CHAT=()
MODEL_SIZE=()
MODEL_LEVELS=()
MODEL_LEVEL_DEFAULT=()
MODEL_FLAGS=()
MODEL_OPTIONS=()
MODEL_TOML=()
MODEL_REAL=()
MODEL_ISALIAS=()

# _rp_mc_python — intérprete para modelconf.py. $ROPERO_PYTHON gana; si no,
# el primer python3/python del PATH que ARRENQUE de verdad (un shim de asdf
# roto pasa `command -v` y luego falla: se prueba, no se supone).
# Cacheado en _RP_MC_PY.
_rp_mc_python() {
    if [[ -n "${_RP_MC_PY:-}" ]]; then
        printf '%s' "$_RP_MC_PY"
        return 0
    fi
    local cand ruta
    if [[ -n "${ROPERO_PYTHON:-}" ]]; then
        case "$ROPERO_PYTHON" in
            */*) [[ -x "$ROPERO_PYTHON" ]] && _RP_MC_PY="$ROPERO_PYTHON" ;;
            *)   ruta="$(command -v "$ROPERO_PYTHON" 2>/dev/null)" && _RP_MC_PY="$ruta" ;;
        esac
        if [[ -n "${_RP_MC_PY:-}" ]]; then
            printf '%s' "$_RP_MC_PY"
            return 0
        fi
    fi
    for cand in python3 python /usr/bin/python3 /usr/local/bin/python3 /opt/homebrew/bin/python3; do
        ruta="$(command -v "$cand" 2>/dev/null)" || continue
        [[ -n "$ruta" ]] || continue
        if "$ruta" -c 'import sys' >/dev/null 2>&1; then
            _RP_MC_PY="$ruta"
            printf '%s' "$_RP_MC_PY"
            return 0
        fi
    done
    return 1
}

# _rp_mc_append — añade una fila (real o de alias) a las arrays paralelas.
# Los 19 valores llegan como argumentos posicionales.
_rp_mc_append() {
    MODEL_NAMES+=("$1")
    MODEL_ENGINE+=("$2")
    MODEL_FORMAT+=("$3")
    MODEL_CONTEXT+=("$4")
    MODEL_PORT+=("$5")
    MODEL_PATH+=("$6")
    MODEL_ALIAS+=("$7")
    MODEL_ORIGIN+=("$8")
    MODEL_GGUF+=("$9")
    MODEL_MTP+=("${10}")
    MODEL_MMPROJ+=("${11}")
    MODEL_AUX_CHAT+=("${12}")
    MODEL_SIZE+=("${13}")
    MODEL_LEVELS+=("${14}")
    MODEL_LEVEL_DEFAULT+=("${15}")
    MODEL_FLAGS+=("${16}")
    MODEL_OPTIONS+=("${17}")
    MODEL_TOML+=("${18}")
    MODEL_REAL+=("${19}")
    MODEL_ISALIAS+=("${20}")
}

# _rp_mc_name_taken <nombre> — 0 si ya existe como nombre O alias de otra fila.
_rp_mc_name_taken() {
    local objetivo="$1" i
    for (( i = 0; i < ${#MODEL_NAMES[@]}; i++ )); do
        [[ "${MODEL_NAMES[$i]}" == "$objetivo" ]] && return 0
    done
    return 1
}

# rp_mc_load — vuelca el catálogo (activos + fired) en las arrays paralelas.
#
# Un SOLO dump del parser, leído con `while IFS=$'\x1f' read -r` en el shell
# actual (process substitution: las arrays tienen que sobrevivir al bucle).
# Los avisos del parser (engine legacy, [download].file en safetensors, ...)
# van a stderr tal cual: son presentación, no datos.
#
# Choques de alias: un alias que coincide con el nombre de OTRO modelo (o con
# el alias de otro) es ambigüedad real en la CLI (`ropero <alias>` no sabría
# a cuál ir) → exit 1 nombrando el conflicto (spec §5.3). Un alias igual al
# PROPIO nombre no añade nada: se ignora.
#
# Nombres duplicados activo+fired (p.ej. un modelo retirado y su relevo con
# el mismo nombre) NO son error: las dos filas se quedan y la PRIMERA gana en
# las búsquedas (el dump ordena activos antes que fired dentro de cada dir).
rp_mc_load() {
    [[ "$RP_MC_LOADED" == "1" ]] && return 0

    local py
    py=$(_rp_mc_python) || {
        rp_msg error "no hay un python3 utilizable para leer el catálogo"
        rp_msg info "  fija ROPERO_PYTHON=/ruta/a/python3 si está en un sitio raro"
        return 1
    }
    [[ -f "$RP_MODELCONF_PY" ]] || {
        rp_msg error "falta el parser del catálogo: $RP_MODELCONF_PY"
        return 1
    }
    [[ -d "$RP_MODELS_DIR" ]] || {
        rp_msg error "no existe el directorio de modelos: $RP_MODELS_DIR"
        return 1
    }

    # Los 31 campos del dump v1, en orden (build_record de modelconf.py).
    # La POSICIÓN es el dato: los que no se copian a las arrays se leen
    # igualmente para mantener el orden del registro.
    # shellcheck disable=SC2034
    local f_name f_engine f_alias f_origin f_engdir f_ctx f_port f_fmt f_path \
          f_gguf f_mtp f_mmproj f_repo f_file f_dest f_size f_excl \
          f_levels f_ldefault f_flags f_opts f_aliases f_auxmtp f_auxmmproj \
          f_spec f_optdef f_limits f_manual f_toml f_auxchat f_build

    # shellcheck disable=SC2034   # posición = dato: todo el registro se lee
    # entero para no desalinear los campos
    while IFS=$'\x1f' read -r \
        f_name f_engine f_alias f_origin f_engdir f_ctx f_port f_fmt f_path \
        f_gguf f_mtp f_mmproj f_repo f_file f_dest f_size f_excl \
        f_levels f_ldefault f_flags f_opts f_aliases f_auxmtp f_auxmmproj \
        f_spec f_optdef f_limits f_manual f_toml f_auxchat f_build; do
        # Cabecera del dump: versión del formato. Un consumidor nuevo ante un
        # dump viejo (o al revés) debe negarse a leer a ciegas.
        case "$f_name" in
            ROPERO_MC_DUMP_VERSION=*)
                if [[ "$f_name" != "ROPERO_MC_DUMP_VERSION=1" ]]; then
                    rp_msg error "formato de catálogo no soportado: $f_name (esperado =1)"
                    return 1
                fi
                continue
                ;;
        esac
        [[ -n "$f_name" ]] || continue
        _rp_mc_append "$f_name" "$f_engine" "$f_fmt" "$f_ctx" "$f_port" \
            "$f_path" "$f_alias" "$f_origin" "$f_gguf" "$f_mtp" "$f_mmproj" \
            "$f_auxchat" "$f_size" "$f_levels" "$f_ldefault" "$f_flags" \
            "$f_opts" "$f_toml" "$f_name" 0
    done < <("$py" "$RP_MODELCONF_PY" dump --root "$RP_MODELS_DIR" --include-fired)

    # ── Filas de alias ─────────────────────────────────────────────────────
    # Se añaden DESPUÉS de las reales: la fila real mantiene su índice y las
    # búsquedas por nombre no se descolocan.
    local i alias real total
    total=${#MODEL_NAMES[@]}
    for (( i = 0; i < total; i++ )); do
        alias="${MODEL_ALIAS[$i]}"
        real="${MODEL_NAMES[$i]}"
        [[ -n "$alias" ]] || continue
        [[ "$alias" != "$real" ]] || continue   # alias == propio nombre: nada
        if _rp_mc_name_taken "$alias"; then
            rp_msg error "choque de alias: '$alias' (de '$real') ya existe como nombre o alias de otro modelo"
            return 1
        fi
        _rp_mc_append "$alias" "${MODEL_ENGINE[$i]}" "${MODEL_FORMAT[$i]}" \
            "${MODEL_CONTEXT[$i]}" "${MODEL_PORT[$i]}" "${MODEL_PATH[$i]}" \
            "${MODEL_ALIAS[$i]}" "${MODEL_ORIGIN[$i]}" "${MODEL_GGUF[$i]}" \
            "${MODEL_MTP[$i]}" "${MODEL_MMPROJ[$i]}" "${MODEL_AUX_CHAT[$i]}" \
            "${MODEL_SIZE[$i]}" "${MODEL_LEVELS[$i]}" \
            "${MODEL_LEVEL_DEFAULT[$i]}" "${MODEL_FLAGS[$i]}" \
            "${MODEL_OPTIONS[$i]}" "${MODEL_TOML[$i]}" "$real" 1
    done

    RP_MC_LOADED=1
    return 0
}

# _rp_mc_idx <nombre> — imprime el índice (en MODEL_NAMES) de nombre o alias.
# Requiere el catálogo cargado (rp_mc_load lo llama el que lo necesita).
# Primera coincidencia gana (activos antes que fired dentro de cada engine).
_rp_mc_idx() {
    local objetivo="$1" i
    for (( i = 0; i < ${#MODEL_NAMES[@]}; i++ )); do
        [[ "${MODEL_NAMES[$i]}" == "$objetivo" ]] && { printf '%s' "$i"; return 0; }
    done
    return 1
}

# rp_model_exists <nombre> — 0 si el modelo (o su alias) existe en el catálogo.
rp_model_exists() {
    rp_mc_load || return 1
    _rp_mc_idx "$1" >/dev/null
}

# rp_model_field <nombre> <campo> — valor por stdout.
# Campos: name engine alias origin context port format path gguf mtp mmproj
#         aux-chat size levels level-default flags options toml real isalias
rp_model_field() {
    local nombre="$1" campo="$2" idx
    rp_mc_load || return 1
    idx=$(_rp_mc_idx "$nombre") || return 1
    case "$campo" in
        name)          printf '%s\n' "${MODEL_NAMES[$idx]}" ;;
        engine)        printf '%s\n' "${MODEL_ENGINE[$idx]}" ;;
        alias)         printf '%s\n' "${MODEL_ALIAS[$idx]}" ;;
        origin)        printf '%s\n' "${MODEL_ORIGIN[$idx]}" ;;
        context)       printf '%s\n' "${MODEL_CONTEXT[$idx]}" ;;
        port)          printf '%s\n' "${MODEL_PORT[$idx]}" ;;
        format)        printf '%s\n' "${MODEL_FORMAT[$idx]}" ;;
        path)          printf '%s\n' "${MODEL_PATH[$idx]}" ;;
        gguf)          printf '%s\n' "${MODEL_GGUF[$idx]}" ;;
        mtp)           printf '%s\n' "${MODEL_MTP[$idx]}" ;;
        mmproj)        printf '%s\n' "${MODEL_MMPROJ[$idx]}" ;;
        aux-chat)      printf '%s\n' "${MODEL_AUX_CHAT[$idx]}" ;;
        size)          printf '%s\n' "${MODEL_SIZE[$idx]}" ;;
        levels)        printf '%s\n' "${MODEL_LEVELS[$idx]}" ;;
        level-default) printf '%s\n' "${MODEL_LEVEL_DEFAULT[$idx]}" ;;
        flags)         printf '%s\n' "${MODEL_FLAGS[$idx]}" ;;
        options)       printf '%s\n' "${MODEL_OPTIONS[$idx]}" ;;
        toml)          printf '%s\n' "${MODEL_TOML[$idx]}" ;;
        real)          printf '%s\n' "${MODEL_REAL[$idx]}" ;;
        isalias)       printf '%s\n' "${MODEL_ISALIAS[$idx]}" ;;
        *)             rp_msg error "campo desconocido: '$campo'"; return 1 ;;
    esac
}

# rp_model_argv <nombre> [nivel] [flags...] [k=v...]
#
# El argv final del modelo, UN ARGUMENTO POR LÍNEA (el formato de
# modelconf.py args). Gramática de los argumentos tras el nombre:
#   · el primero que no es k=v (ni vacío) es el NIVEL
#   · los k=v son OPCIONES (--option k=v)
#   · el resto son FLAGS (--flag nombre), con o sin los "--" de delante
# El nivel/flags/opciones se validan en modelconf.py (contra el TOML); la
# validación de USO (¿está declarado?) la hace el comando (cmd-catalog).
rp_model_argv() {
    local nombre="${1:-}"
    [[ -n "$nombre" ]] || return 1
    shift
    rp_mc_load || return 1

    # Alias → nombre real: modelconf sólo conoce nombres reales.
    local idx real="$nombre"
    if idx=$(_rp_mc_idx "$nombre"); then
        real="${MODEL_REAL[$idx]}"
    fi

    local a nivel=""
    local -a mflags=() mopts=()
    for a in "$@"; do
        [[ -n "$a" ]] || continue
        a="${a#--}"
        case "$a" in
            *=*)
                mopts+=("$a")
                ;;
            *)
                if [[ -z "$nivel" ]]; then
                    nivel="$a"
                else
                    mflags+=("$a")
                fi
                ;;
        esac
    done

    local py
    py=$(_rp_mc_python) || return 1
    local -a cmd=("$py" "$RP_MODELCONF_PY" args --root "$RP_MODELS_DIR" --name "$real")
    [[ -n "$nivel" ]] && cmd+=(--level "$nivel")
    local f
    for f in ${mflags[@]+"${mflags[@]}"}; do
        cmd+=(--flag "$f")
    done
    for f in ${mopts[@]+"${mopts[@]}"}; do
        cmd+=(--option "$f")
    done
    ${cmd[@]+"${cmd[@]}"}
}

# rp_model_build_argv <nombre> [--out ruta] — argv del gguffer (passthrough).
rp_model_build_argv() {
    local nombre="${1:-}"
    [[ -n "$nombre" ]] || return 1
    shift
    rp_mc_load || return 1
    local idx real="$nombre"
    if idx=$(_rp_mc_idx "$nombre"); then
        real="${MODEL_REAL[$idx]}"
    fi
    local py
    py=$(_rp_mc_python) || return 1
    "$py" "$RP_MODELCONF_PY" build --root "$RP_MODELS_DIR" --name "$real" "$@"
}

# rp_model_aux <nombre> — líneas tipo|repo|file|name|size|use (passthrough).
rp_model_aux() {
    local nombre="${1:-}"
    [[ -n "$nombre" ]] || return 1
    rp_mc_load || return 1
    local idx real="$nombre"
    if idx=$(_rp_mc_idx "$nombre"); then
        real="${MODEL_REAL[$idx]}"
    fi
    local py
    py=$(_rp_mc_python) || return 1
    "$py" "$RP_MODELCONF_PY" aux --root "$RP_MODELS_DIR" --name "$real"
}
