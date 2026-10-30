#!/usr/bin/env bash
# =============================================================================
# engines/tensorrt-llm.sh — NVIDIA TensorRT-LLM (spec §6, fila 5)
# =============================================================================
# Linux + CUDA. El backend es trtllm-serve (server OpenAI, SIN --api-key:
# por eso va detrás de endpoint.py, que es quien exige la key). Instalación:
# venv propio (~/.venv-tensorrt) con la wheel de TRT-LLM vía uv — canal
# portado del legado (spec §14.8: canal exacto verificado al portar).
#
# El venv lleva TAMBIÉN fastapi+uvicorn: el endpoint que lo proxya tiene que
# correr con un intérprete que los importe, y el venv del engine es el
# candidato natural (los overlays de TRT hacen frágil mezclar pythons).
# =============================================================================

rp_eng_tensorrt-llm_label() {
    printf 'tensorrt-llm · trtllm-serve tras endpoint.py\n'
}

rp_eng_tensorrt-llm_compatible() {
    local os
    os="$(rp_detect_os)"
    case "$os" in
        cachyos|arch|linux)
            if command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi >/dev/null 2>&1; then
                return 0
            fi
            printf 'tensorrt-llm: Linux con driver CUDA operativo (nvidia-smi no responde)\n' >&2
            return 1
            ;;
        *)
            printf 'tensorrt-llm: sólo Linux con NVIDIA (%s)\n' "$os" >&2
            return 1
            ;;
    esac
}

rp_eng_tensorrt-llm_deps() {
    printf 'uv python3 nvidia-smi\n'
}

rp_eng_tensorrt-llm_install() {
    local force=0 a
    for a in "$@"; do
        case "$a" in
            --force) force=1 ;;
            --*) : ;;
        esac
    done

    _rp_engine_deps_ok uv || return 1
    if ! command -v nvidia-smi >/dev/null 2>&1 || ! nvidia-smi >/dev/null 2>&1; then
        rp_msg error "tensorrt-llm exige un driver CUDA operativo (nvidia-smi)"
        return 1
    fi

    local venv="$HOME/.venv-tensorrt"
    if rp_eng_tensorrt-llm_resolve >/dev/null && [[ "$force" != "1" ]]; then
        rp_msg ok "tensorrt-llm ya instalado: $(rp_eng_tensorrt-llm_resolve)"
        return 0
    fi
    if [[ "$force" == "1" ]]; then
        rp_msg info "reinstalación forzada (--force)"
    fi

    # Se recrea SIEMPRE en --force (o si está roto): un venv a medias de un
    # install anterior hace fallar `uv venv` con "already exists".
    if [[ "$force" == "1" && -d "$venv" ]]; then
        rm -rf "$venv" || return 1
    fi
    rp_step 1 2 "creando el venv de tensorrt-llm ($venv)..."
    uv venv "$venv" --python 3.12 || return 1

    rp_step 2 2 "instalando tensorrt-llm (20-40 min)..."
    if ! uv pip install --python "$venv/bin/python" --quiet \
            tensorrt-llm fastapi uvicorn; then
        rp_msg error "falló la instalación de tensorrt-llm"
        rp_msg info "  puede necesitar wheels CUDA manuales: https://nvidia.github.io/TensorRT-LLM/"
        return 1
    fi
    if [[ ! -x "$venv/bin/trtllm-serve" ]]; then
        rp_msg error "el venv no registró trtllm-serve"
        return 1
    fi
    rp_msg ok "tensorrt-llm instalado: $venv/bin/trtllm-serve"
    return 0
}

rp_eng_tensorrt-llm_resolve() {
    # PATH → BIN_DIR → venv del engine (spec §5.4: PATH→BIN_DIR→rutas largas).
    local cand ruta
    ruta="$(command -v trtllm-serve 2>/dev/null)"
    [[ -n "$ruta" ]] && { printf '%s\n' "$ruta"; return 0; }
    for cand in "$BIN_DIR/trtllm-serve" "$HOME/.venv-tensorrt/bin/trtllm-serve"; do
        if [[ -x "$cand" ]]; then
            printf '%s\n' "$cand"
            return 0
        fi
    done
    return 1
}

rp_eng_tensorrt-llm_probe() {
    local puerto="${1:-}"
    if ! rp_eng_tensorrt-llm_resolve >/dev/null; then
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

rp_eng_tensorrt-llm_native_auth() {
    printf 'no\n'
    return 0
}

rp_eng_tensorrt-llm_launch() {
    local modelo="$1" puerto="$2" logfile="$3"
    shift 3

    local py
    if ! py="$(_rp_engine_python_with fastapi uvicorn)"; then
        rp_msg error "el endpoint de tensorrt-llm necesita fastapi y uvicorn, y no hay un python3 que los tenga"
        rp_msg info "  (ropero install engines tensorrt-llm los deja en su venv)"
        return 1
    fi
    if ! rp_eng_tensorrt-llm_resolve >/dev/null; then
        rp_msg error "tensorrt-llm no está instalado (ropero install engines tensorrt-llm)"
        return 1
    fi
    if [[ -z "${LLAMA_API_KEY:-}" ]]; then
        rp_msg error "falta LLAMA_API_KEY: el launcher (04) la resuelve antes de llamar al engine"
        return 1
    fi
    modelo="$(rp_model_field "$modelo" real)"
    local mpath
    if ! mpath="$(_rp_engine_model_path "$modelo")"; then
        rp_msg error "'$modelo' no declara [model].path (el checkpoint de tensorrt)"
        return 1
    fi

    # Los args del modelo van ENVUELTOS en --extra-arg: el endpoint los
    # reenvía tal cual al trtllm-serve interno (no son flags del endpoint).
    local -a cmd=("$py" "${ROPERO_ROOT}/bin/server/endpoint.py"
        --backend tensorrt
        --model-path "$mpath"
        --model-id "$modelo"
        --host "${HOST:-127.0.0.1}"
        --port "$puerto"
        --api-key "$LLAMA_API_KEY"
        --backend-port $(( puerto + 1000 )))
    local a
    for a in "$@"; do
        cmd+=(--extra-arg "$a")
    done

    mkdir -p "$(dirname "$logfile")" 2>/dev/null
    nohup ${cmd[@]+"${cmd[@]}"} >>"$logfile" 2>&1 &
    printf '%s\n' "$!"
    return 0
}
