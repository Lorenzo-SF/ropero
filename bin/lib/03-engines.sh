#!/usr/bin/env bash
# =============================================================================
# 03-engines.sh — cargador de engines (spec §5.4, fase 3)
# =============================================================================
# Un engine = bin/lib/engines/<id>.sh con OCHO funciones obligatorias:
#
#   rp_eng_<id>_label        → etiqueta humana por stdout
#   rp_eng_<id>_compatible   → 0 si el SO/hw lo soporta (razón por stderr)
#   rp_eng_<id>_deps         → comandos externos, separados por espacio
#   rp_eng_<id>_install      → [--force] instala SUS deps (rp_msg/rp_step)
#   rp_eng_<id>_resolve      → ruta absoluta del binario (PATH→BIN_DIR→…)
#   rp_eng_<id>_probe <p>    → ok | down | missing (0 sólo si ok)
#   rp_eng_<id>_native_auth  → yes | no
#   rp_eng_<id>_launch <modelo> <puerto> <logfile> <argv...> → PID por stdout
#
# El cargador verifica las OCHO; si falta una, error nombrándola.
#
# NOMBRES CON GUIONES: `rp_eng_llama-server_label` es un identificador bash
# legal (bash admite guiones en nombres de función; POSIX sh no, pero no
# estamos en modo POSIX). Los ids de engine van con guión porque así los
# nombra la spec (§6); no se traducen.
#
# bash 3.2: sin namerefs ni arrays asociativos. Seguro de sourcear.
# =============================================================================

if ! declare -F rp_mc_load >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/02-modelconf.sh"
fi

# Ids canónicos (§6). El orden es el de la tabla de la spec.
RP_ENGINE_IDS="llama-server vllm mlx-serve airllm tensorrt-llm fastflowlm"

# Los ocho hooks que TODO engine debe definir (spec §5.4).
RP_ENGINE_HOOKS="label compatible deps install resolve probe native_auth launch"

# rp_engine_canonical <id> — id canónico (legacy mlx_lm → mlx-serve).
# Imprime el id canónico; rc 1 si no se conoce.
rp_engine_canonical() {
    case "${1:-}" in
        llama-server|llama|llamacpp|llama-cpp|llama_cpp) printf 'llama-server' ;;
        vllm)                                            printf 'vllm' ;;
        mlx-serve|mlx_lm|mlx|mlx-lm)                     printf 'mlx-serve' ;;
        airllm)                                          printf 'airllm' ;;
        tensorrt-llm|tensorrt|trtllm|tensorrt_llm)       printf 'tensorrt-llm' ;;
        fastflowlm|flm|fastflow_lm)                      printf 'fastflowlm' ;;
        *)                                               return 1 ;;
    esac
    return 0
}

# rp_engine_list — un id por línea.
rp_engine_list() {
    local id
    for id in $RP_ENGINE_IDS; do
        printf '%s\n' "$id"
    done
}

# rp_engine_load <id> — sourcea engines/<id>.sh (si hace falta) y verifica
# los ocho hooks. Error nombrando el hook que falte.
rp_engine_load() {
    local id="${1:-}"
    local canon
    canon="$(rp_engine_canonical "$id")" || {
        rp_msg error "engine desconocido: '$id' (válidos: $RP_ENGINE_IDS)"
        return 1
    }
    id="$canon"

    local ficha="${ROPERO_ROOT}/bin/lib/engines/${id}.sh"
    if ! declare -F "rp_eng_${id}_label" >/dev/null 2>&1; then
        if [[ ! -f "$ficha" ]]; then
            rp_msg error "no existe el fichero del engine: bin/lib/engines/${id}.sh"
            return 1
        fi
        # shellcheck source=/dev/null
        . "$ficha"
    fi

    local hook
    for hook in $RP_ENGINE_HOOKS; do
        if ! declare -F "rp_eng_${id}_${hook}" >/dev/null 2>&1; then
            rp_msg error "engine '${id}': falta el hook rp_eng_${id}_${hook}"
            return 1
        fi
    done
    return 0
}

# rp_engine_call <id> <hook> [args...] — carga y despacha.
rp_engine_call() {
    local id="${1:-}" hook="${2:-}"
    shift 2
    [[ -n "$id" && -n "$hook" ]] || {
        rp_msg error "rp_engine_call <engine> <hook> [args...]"
        return 1
    }
    rp_engine_load "$id" || return 1
    local canon
    canon="$(rp_engine_canonical "$id")"
    "rp_eng_${canon}_${hook}" "$@"
}

# rp_engine_for_model <modelo> — lee el engine del TOML (vía catálogo) y lo
# canóniza. Imprime el id; rc 1 si el modelo o el engine no se conocen.
rp_engine_for_model() {
    rp_mc_load || return 1
    local motor
    motor="$(rp_model_field "$1" engine)" || return 1
    rp_engine_canonical "$motor" || {
        rp_msg error "el modelo '$1' declara un engine desconocido: '$motor'"
        return 1
    }
}

# ── Helpers compartidos por los engines ─────────────────────────────────────

# _rp_engine_python_with <módulo>... — intérprete que IMPORTA esos módulos.
# El endpoint (bin/server/endpoint.py) necesita fastapi+uvicorn, y el backend
# airllm además la biblioteca airllm, todo en el MISMO proceso. En una máquina
# con varios Pythons (asdf, venv de tensorrt, tools de uv) el primero del PATH
# no tiene por qué ser el bueno: se PRUEBA cada candidato, no se supone.
_rp_engine_python_with() {
    [[ $# -ge 1 ]] || return 1
    local joined cand p
    # "import a b" no es Python válido: hace falta "import a, b".
    joined="$(printf '%s,' "$@")"
    joined="${joined%,}"
    [[ -n "$joined" ]] || return 1

    local -a candidates=()
    if [[ -n "${ROPERO_PYTHON:-}" ]]; then
        candidates+=("$ROPERO_PYTHON")
    fi
    # Venvs de engines (tensorrt) y herramientas de uv (airllm, mlx-lm).
    for cand in "$HOME"/.venv-tensorrt/bin/python3 "$HOME"/.venv/bin/python3 \
                "$HOME"/.local/share/uv/tools/*/bin/python3; do
        [[ -x "$cand" ]] && candidates+=("$cand")
    done
    for cand in python3 python; do
        p="$(command -v "$cand" 2>/dev/null)" && candidates+=("$p")
    done
    candidates+=(/usr/bin/python3 /usr/local/bin/python3 /opt/homebrew/bin/python3 /bin/python3)

    for cand in ${candidates[@]+"${candidates[@]}"}; do
        # Un nombre sin ruta (ROPERO_PYTHON=python3) se resuelve aquí.
        case "$cand" in
            */*) ;;
            *)  p="$(command -v "$cand" 2>/dev/null)" || continue; cand="$p" ;;
        esac
        [[ -x "$cand" ]] || continue
        if "$cand" -c "import $joined" >/dev/null 2>&1; then
            printf '%s' "$cand"
            return 0
        fi
    done
    return 1
}

# _rp_engine_deps_ok <cmd>... — 0 si todos los comandos existen; si no,
# error con la lista y rc 1 (NO exit: un engine roto no tumba el install).
_rp_engine_deps_ok() {
    local faltan="" d
    for d in "$@"; do
        command -v "$d" >/dev/null 2>&1 || faltan="${faltan:+$faltan }$d"
    done
    if [[ -n "$faltan" ]]; then
        rp_msg error "faltan dependencias del engine: $faltan"
        rp_msg info "  (ropero install setup las instala)"
        return 1
    fi
    return 0
}

# _rp_engine_http_ok <puerto> — 0 si /health responde en $HOST:<puerto>.
# curl primero (está en macOS y Linux); wget como fallback. Sin cliente HTTP
# no hay probe: rc 1.
_rp_engine_http_ok() {
    local puerto="$1" host="${HOST:-127.0.0.1}"
    if command -v curl >/dev/null 2>&1; then
        curl -fsS --max-time 2 "http://$host:$puerto/health" >/dev/null 2>&1 && return 0
        return 1
    fi
    if command -v wget >/dev/null 2>&1; then
        wget -q -O /dev/null -T 2 "http://$host:$puerto/health" 2>/dev/null && return 0
    fi
    return 1
}

# _rp_engine_port_listening <puerto> — 0 si algo escucha en el puerto.
_rp_engine_port_listening() {
    lsof -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1
}

# _rp_engine_model_path <modelo> — ruta del checkpoint bajo $SAFETENSORS_DIR
# (o ruta absoluta tal cual si el TOML la trae). Vacío si no declara path.
_rp_engine_model_path() {
    local mpath
    mpath="$(rp_model_field "$1" path)" || return 1
    [[ -n "$mpath" ]] || return 1
    case "$mpath" in
        /*) printf '%s' "$mpath" ;;
        *)  printf '%s' "$SAFETENSORS_DIR/$mpath" ;;
    esac
}

# _rp_engine_nproc — trabajos de compilación (nproc, sysctl hw.ncpu, 8).
_rp_engine_nproc() {
    if command -v nproc >/dev/null 2>&1; then
        nproc
    elif command -v sysctl >/dev/null 2>&1; then
        sysctl -n hw.ncpu 2>/dev/null || echo 8
    else
        echo 8
    fi
}
