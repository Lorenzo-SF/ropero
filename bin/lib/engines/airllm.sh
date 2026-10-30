#!/usr/bin/env bash
# =============================================================================
# engines/airllm.sh — AirLLM (spec §6, fila 4)
# =============================================================================
# Librería PURA (no trae server): la sirve bin/server/endpoint.py en su
# mismo proceso (--backend airllm), que es quien exige la API key. macOS y
# Linux (RAM ≥ pesos del modelo: el check de [limits] lo hace el launcher).
#
# Instalación: uv tool install airllm --with fastapi --with uvicorn. Los
# --with no son un capricho: el endpoint importa airllm, fastapi y uvicorn
# en el MISMO intérprete; sin ellos habría que ensuciar el python global.
# =============================================================================

rp_eng_airllm_label() {
    printf 'airllm · librería, servida por endpoint.py\n'
}

rp_eng_airllm_compatible() {
    local os
    os="$(rp_detect_os)"
    case "$os" in
        macos|cachyos|arch|linux) return 0 ;;
        *)
            printf 'airllm: SO no soportado (%s)\n' "$os" >&2
            return 1
            ;;
    esac
}

rp_eng_airllm_deps() {
    printf 'uv python3\n'
}

rp_eng_airllm_install() {
    local force=0 a
    for a in "$@"; do
        case "$a" in
            --force) force=1 ;;
            --*) : ;;
        esac
    done

    _rp_engine_deps_ok uv || return 1

    if rp_eng_airllm_resolve >/dev/null; then
        if [[ "$force" != "1" ]]; then
            rp_msg ok "airllm ya instalado (importa con $(rp_eng_airllm_resolve))"
            return 0
        fi
        rp_msg info "reinstalación forzada (--force)"
    fi

    rp_step 1 1 "instalando airllm con uv tool (con fastapi y uvicorn para el endpoint)..."
    if [[ "$force" == "1" ]]; then
        uv tool install --reinstall airllm --with fastapi --with uvicorn || return 1
    else
        uv tool install airllm --with fastapi --with uvicorn || return 1
    fi
    if ! rp_eng_airllm_resolve >/dev/null; then
        rp_msg error "uv terminó pero ningún intérprete importa airllm+fastapi+uvicorn"
        return 1
    fi
    rp_msg ok "airllm instalado"
    return 0
}

# resolve: para una librería no hay binario propio; lo que se ejecuta es el
# intérprete que la importa (y que lanzará endpoint.py). "Instalado" = hay
# un python con airllm+fastapi+uvicorn importables Y existe el endpoint.
rp_eng_airllm_resolve() {
    [[ -f "${ROPERO_ROOT}/bin/server/endpoint.py" ]] || return 1
    local py
    py="$(_rp_engine_python_with airllm fastapi uvicorn)" || return 1
    printf '%s\n' "$py"
}

rp_eng_airllm_probe() {
    local puerto="${1:-}"
    if ! rp_eng_airllm_resolve >/dev/null; then
        printf 'missing\n'
        return 1
    fi
    if _rp_engine_http_ok "$puerto"; then
        printf 'ok\n'
        return 0
    fi
    printf 'down\n'
    return 1
}

rp_eng_airllm_native_auth() {
    printf 'no\n'
    return 0
}

rp_eng_airllm_launch() {
    local modelo="$1" puerto="$2" logfile="$3"
    shift 3

    local py
    if ! py="$(rp_eng_airllm_resolve)"; then
        rp_msg error "airllm no está instalado (ropero install engines airllm)"
        return 1
    fi
    if [[ -z "${LLAMA_API_KEY:-}" ]]; then
        rp_msg error "falta LLAMA_API_KEY: el launcher (04) la resuelve antes de llamar al engine"
        return 1
    fi
    # El id que anuncia /v1/models es el nombre REAL de ropero (no el alias).
    modelo="$(rp_model_field "$modelo" real)"
    local mpath
    if ! mpath="$(_rp_engine_model_path "$modelo")"; then
        rp_msg error "'$modelo' no declara [model].path (el checkpoint de airllm)"
        return 1
    fi

    # Los args del modelo son flags DIRECTOS del endpoint (--compression,
    # --max-new-tokens, ...): el endpoint los reconoce como propios. Si se
    # envolvieran en --extra-arg, argparse los rechazaría.
    mkdir -p "$(dirname "$logfile")" 2>/dev/null
    nohup "$py" "${ROPERO_ROOT}/bin/server/endpoint.py" \
        --backend airllm \
        --model-path "$mpath" \
        --model-id "$modelo" \
        --host "${HOST:-127.0.0.1}" \
        --port "$puerto" \
        --api-key "$LLAMA_API_KEY" \
        --backend-port $(( puerto + 1000 )) \
        "$@" >>"$logfile" 2>&1 </dev/null &
    printf '%s\n' "$!"
    return 0
}
