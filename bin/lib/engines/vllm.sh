#!/usr/bin/env bash
# =============================================================================
# engines/vllm.sh — vllm (spec §6, fila 2)
# =============================================================================
# Linux + CUDA (nvidia-smi OK). Server OpenAI propio con --api-key (auth
# nativa). Instalación: uv tool install vllm (aislado, sin tocar el python
# del sistema).
# =============================================================================

rp_eng_vllm_label() {
    printf 'vllm · server OpenAI, Linux+CUDA\n'
}

rp_eng_vllm_compatible() {
    local os
    os="$(rp_detect_os)"
    case "$os" in
        cachyos|arch|linux)
            if command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi >/dev/null 2>&1; then
                return 0
            fi
            printf 'vllm: Linux con driver CUDA operativo (nvidia-smi no responde)\n' >&2
            return 1
            ;;
        *)
            printf 'vllm: sólo Linux con NVIDIA (%s)\n' "$os" >&2
            return 1
            ;;
    esac
}

rp_eng_vllm_deps() {
    printf 'uv nvidia-smi\n'
}

rp_eng_vllm_install() {
    local force=0 a
    for a in "$@"; do
        case "$a" in
            --force) force=1 ;;
            --*) : ;;
        esac
    done

    _rp_engine_deps_ok uv || return 1
    if ! command -v nvidia-smi >/dev/null 2>&1 || ! nvidia-smi >/dev/null 2>&1; then
        rp_msg error "vllm exige un driver CUDA operativo (nvidia-smi)"
        return 1
    fi

    if rp_eng_vllm_resolve >/dev/null; then
        if [[ "$force" != "1" ]]; then
            rp_msg ok "vllm ya instalado: $(rp_eng_vllm_resolve)"
            return 0
        fi
        rp_msg info "reinstalación forzada (--force)"
    fi

    rp_step 1 1 "instalando vllm con uv tool..."
    if [[ "$force" == "1" ]]; then
        uv tool install --reinstall vllm || return 1
    else
        uv tool install vllm || return 1
    fi
    if ! rp_eng_vllm_resolve >/dev/null; then
        rp_msg error "uv terminó pero no encuentro el binario 'vllm'"
        return 1
    fi
    rp_msg ok "vllm instalado: $(rp_eng_vllm_resolve)"
    return 0
}

rp_eng_vllm_resolve() {
    # PATH → BIN_DIR (uv tool instala en ~/.local/bin = BIN_DIR por defecto).
    local ruta
    ruta="$(command -v vllm 2>/dev/null)"
    [[ -n "$ruta" ]] && { printf '%s\n' "$ruta"; return 0; }
    if [[ -x "$BIN_DIR/vllm" ]]; then
        printf '%s\n' "$BIN_DIR/vllm"
        return 0
    fi
    return 1
}

rp_eng_vllm_probe() {
    local puerto="${1:-}"
    if ! rp_eng_vllm_resolve >/dev/null; then
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

rp_eng_vllm_native_auth() {
    printf 'yes\n'
    return 0
}

rp_eng_vllm_launch() {
    local modelo="$1" puerto="$2" logfile="$3"
    shift 3

    local bin
    if ! bin="$(rp_eng_vllm_resolve)"; then
        rp_msg error "vllm no está instalado (ropero install engines vllm)"
        return 1
    fi
    if [[ -z "${LLAMA_API_KEY:-}" ]]; then
        rp_msg error "falta LLAMA_API_KEY: el launcher (04) la resuelve antes de llamar al engine"
        return 1
    fi
    local mpath
    if ! mpath="$(_rp_engine_model_path "$modelo")"; then
        rp_msg error "'$modelo' no declara [model].path (el checkpoint que vllm sirve)"
        return 1
    fi

    mkdir -p "$(dirname "$logfile")" 2>/dev/null
    nohup "$bin" serve "$mpath" \
        "$@" \
        --host "${HOST:-127.0.0.1}" \
        --port "$puerto" \
        --api-key "$LLAMA_API_KEY" >>"$logfile" 2>&1 &
    printf '%s\n' "$!"
    return 0
}
