#!/usr/bin/env bash
# =============================================================================
# engines/mlx-serve.sh — mlx-serve sobre MLX (spec §6, fila 3; ADR-3)
# =============================================================================
# macOS arm64. Server OpenAI propio; va DIRECTO (sin endpoint.py). Auth
# nativa vía --api-key-env ROPERO_MLX_API_KEY + --api-key-strict: la key
# NUNCA va en argv (la tabla de procesos es world-readable) y /health queda
# abierto, así que el probe de readiness no necesita key.
#
# Instalación EN ESTE ORDEN (spec §6): 1) mlx-lm por uv 2) mlx-serve por
# brew (tap ddalcu/mlx-serve). mlx-serve depende de mlx-lm: al revés, brew
# deja un binario que no arranca.
# =============================================================================

rp_eng_mlx-serve_label() {
    printf 'mlx-serve · MLX en Apple Silicon\n'
}

rp_eng_mlx-serve_compatible() {
    if [[ "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]]; then
        return 0
    fi
    printf 'mlx-serve: exige macOS arm64 (Metal/Apple Silicon)\n' >&2
    return 1
}

rp_eng_mlx-serve_deps() {
    printf 'uv brew\n'
}

rp_eng_mlx-serve_install() {
    local force=0 a
    for a in "$@"; do
        case "$a" in
            --force) force=1 ;;
            --*) : ;;
        esac
    done

    _rp_engine_deps_ok uv brew || return 1

    if rp_eng_mlx-serve_resolve >/dev/null && [[ "$force" != "1" ]]; then
        rp_msg ok "mlx-serve ya instalado: $(rp_eng_mlx-serve_resolve)"
        return 0
    fi
    if [[ "$force" == "1" ]]; then
        rp_msg info "reinstalación forzada (--force)"
    fi

    # ── 1. mlx-lm ANTES que mlx-serve (mlx-serve depende de él) ──────────
    rp_step 1 2 "instalando mlx-lm con uv tool..."
    if [[ "$force" == "1" ]]; then
        uv tool install --reinstall mlx-lm || return 1
    else
        uv tool install mlx-lm || return 1
    fi

    # ── 2. mlx-serve por brew (tap propio: el repo del tap no sigue la
    #       convención homebrew-*, hace falta URL explícita e idempotente) ─
    rp_step 2 2 "instalando mlx-serve con brew (ddalcu/mlx-serve)..."
    brew tap ddalcu/mlx-serve https://github.com/ddalcu/mlx-serve || return 1
    if [[ "$force" == "1" ]]; then
        brew reinstall --cask mlx-serve || return 1
    else
        brew install --cask mlx-serve || return 1
    fi

    if ! rp_eng_mlx-serve_resolve >/dev/null; then
        rp_msg error "brew terminó pero no encuentro el binario 'mlx-serve'"
        return 1
    fi
    rp_msg ok "mlx-serve instalado: $(rp_eng_mlx-serve_resolve)"
    return 0
}

rp_eng_mlx-serve_resolve() {
    # PATH → BIN_DIR.
    local ruta
    ruta="$(command -v mlx-serve 2>/dev/null)"
    [[ -n "$ruta" ]] && { printf '%s\n' "$ruta"; return 0; }
    if [[ -x "$BIN_DIR/mlx-serve" ]]; then
        printf '%s\n' "$BIN_DIR/mlx-serve"
        return 0
    fi
    return 1
}

rp_eng_mlx-serve_probe() {
    local puerto="${1:-}"
    if ! rp_eng_mlx-serve_resolve >/dev/null; then
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

rp_eng_mlx-serve_native_auth() {
    printf 'yes\n'
    return 0
}

rp_eng_mlx-serve_launch() {
    local modelo="$1" puerto="$2" logfile="$3"
    shift 3

    local bin
    if ! bin="$(rp_eng_mlx-serve_resolve)"; then
        rp_msg error "mlx-serve no está instalado (ropero install engines mlx-serve)"
        return 1
    fi
    if [[ -z "${LLAMA_API_KEY:-}" ]]; then
        rp_msg error "falta LLAMA_API_KEY: el launcher (04) la resuelve antes de llamar al engine"
        return 1
    fi
    local mpath
    if ! mpath="$(_rp_engine_model_path "$modelo")"; then
        rp_msg error "'$modelo' no declara [model].path (el checkpoint MLX)"
        return 1
    fi

    # La key va por ENTORNO, nunca en argv (ADR-3): --api-key-env la lee del
    # proceso y --api-key-strict la exige incluso desde loopback.
    export ROPERO_MLX_API_KEY="$LLAMA_API_KEY"

    mkdir -p "$(dirname "$logfile")" 2>/dev/null
    nohup "$bin" serve \
        "$@" \
        --host "${HOST:-127.0.0.1}" \
        --port "$puerto" \
        --model "$mpath" \
        --api-key-env ROPERO_MLX_API_KEY \
        --api-key-strict >>"$logfile" 2>&1 </dev/null &
    printf '%s\n' "$!"
    return 0
}
