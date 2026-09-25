#!/usr/bin/env bash
# =============================================================================
# 00-utils.sh — réplica mínima de setup 00-utils.sh para ropero (spec migración)
# Solo lo que ropero necesita: logging a stderr, prompts, detect_os.
# Seguro de sourcear (solo definiciones, cero ejecución).
# =============================================================================

_ropero_log() { printf '%s\n' "$*" >&2; }
ropero_info() { _ropero_log "  [i] $*"; }
ropero_ok() { _ropero_log "  [✓] $*"; }
ropero_warn() { _ropero_log "  [!] $*"; }
ropero_err() { _ropero_log "  [✗] $*"; }

# respeta CI / AUTO_YES / --full desatendido (igual que setup ask_*)
# LECTURA DIRECTA de AUTO_YES / SETUP_MODE / CI sin pasar por la lógica
# original. Si cualquiera está activo, no preguntamos. El entrypoint ropero
# también parchó esto, pero lo hacemos aquí en la fuente para que NO HAYA
# FORMA de que un parche posterior se pierda.
ropero_ask() {
    # Traza diagnóstica: siempre a stderr, fácil de grep. Dice qué modo
    # estamos viendo. Útil para debug si la pregunta sigue saliendo.
    printf '  [trace-ropero_ask] prompt="%s" AUTO_YES=%s SETUP_MODE=%s CI=%s tty=%s\n' \
        "$1" "${AUTO_YES:-0}" "${SETUP_MODE:-}" "${CI:-0}" \
        "$([[ -t 0 ]] && echo yes || echo no)" >&2
    if [[ "${AUTO_YES:-0}" == "1" || "${SETUP_MODE:-}" == "full" || "${CI:-0}" == "1" ]]; then
        printf '  [trace-ropero_ask] → auto-yes (sin prompt)\n' >&2
        return 0
    fi
    if [[ ! -t 0 ]]; then
        printf '  [trace-ropero_ask] → auto-yes (no-TTY)\n' >&2
        return 0
    fi
    local prompt="$1"
    local ans
    printf '%s [S/n]: ' "$prompt" >&2
    read -r ans || ans="s"
    ans="$(printf '%s' "$ans" | tr '[:upper:]' '[:lower:]')"
    [[ -z "$ans" || "$ans" == "s" || "$ans" == "y" ]]
}

# destructivo (rm -rf, reclone): convención setup ask_if_reinstall —
# en desatendido CONSERVA (no), en interactivo default N. Nunca auto-yes.
ropero_ask_keep() {
    if [[ "${AUTO_YES:-0}" == "1" || "${SETUP_MODE:-}" == "full" || "${CI:-0}" == "1" ]]; then
        return 1
    fi
    if [[ ! -t 0 ]]; then
        return 1
    fi
    local prompt="$1"
    local ans
    printf '%s (s/N): ' "$prompt" >&2
    read -r ans || ans="n"
    ans="$(printf '%s' "$ans" | tr '[:upper:]' '[:lower:]')"
    [[ "$ans" == "s" || "$ans" == "y" ]]
}

ropero_detect_os() {
    if [[ -n "${PREFIX:-}" && -d "${PREFIX:-}" ]] && command -v pkg &>/dev/null; then echo "termux"; return 0; fi
    if [[ "$(uname -s)" == "Darwin" ]]; then echo "macos"; return 0; fi
    # WSL2 ANTES de cachy: una WSL2 cachyoseada sigue siendo wsl2 (sin paquetes
    # gaming/GPU bare metal). Detectamos por /proc/sys/kernel/osrelease con
    # "microsoft" o "WSL" en el kernel string — Microsoft usa "microsoft-standard-WSL2"
    # en kernels recientes.
    if grep -qiE 'microsoft|wsl' /proc/sys/kernel/osrelease 2>/dev/null \
       || [[ -f /run/WSL1 ]] || [[ -f /run/WSL2 ]] \
       || [[ -f /proc/version ]] && grep -qiE 'microsoft|wsl' /proc/version 2>/dev/null; then
        echo "wsl2"; return 0
    fi
    if [[ -f /etc/cachyos-release ]] || grep -qi cachyos /etc/os-release 2>/dev/null; then echo "cachy"; return 0; fi
    if [[ -f /etc/arch-release ]]; then echo "arch"; return 0; fi
    echo "unknown"
}

# descarga gguf idempotente con check individual (spec: omitir si existe)
# $6 opcional: renombrar al descargar (mmproj-F16.gguf choca entre repos)
#
# IMPORTANTE: bajo `set -u` (entrypoint ropero lo tiene), `local var=$1` con
# $1 vacío falla con "var: unbound variable" en algunas versiones de bash.
# Por eso validamos primero que los args requeridos no estén vacíos, y
# usamos asignaciones que absorben el unset (`:="${var:-default}"`).
ropero_download_gguf() {
    local repo="${1:-}" filename="${2:-}" name="${3:-}" size="${4:-}"
    local dest_dir="${5:-${GGUF_DIR:-$HOME/models/gguf}}"
    local dest_name="${6:-$filename}"

    if [[ -z "$repo" || -z "$filename" || -z "$name" ]]; then
        ropero_err "ropero_download_gguf: argumentos incompletos (repo='$repo' filename='$filename' name='$name')"
        return 1
    fi
    if [[ -z "$dest_dir" ]]; then
        ropero_err "ropero_download_gguf: dest_dir vacío"; return 1
    fi

    local dest="$dest_dir/$filename"
    local final="$dest_dir/$dest_name"
    if [[ "$dest_name" != "$filename" && -f "$final" && -s "$final" ]]; then
        ropero_ok "$dest_name ya descargado"; return 0
    fi
    if [[ -f "$dest" && -s "$dest" ]]; then
        ropero_ok "$name ya descargado"; return 0
    fi
    if ! ropero_ask "Descargar $name (~$size)? Necesario para ropero"; then
        ropero_info "$name omitido por el usuario"; return 0
    fi
    mkdir -p "$dest_dir"
    if command -v hf &>/dev/null; then
        hf download "$repo" "$filename" --local-dir "$dest_dir" || { ropero_err "falló $name"; return 1; }
    else
        curl -fL "https://huggingface.co/$repo/resolve/main/$filename" -o "$dest" || { ropero_err "falló $name"; return 1; }
    fi
    if [[ "$dest_name" != "$filename" ]]; then
        mv "$dest" "$final" || { ropero_err "no se pudo renombrar a $dest_name"; rm -f "$dest"; return 1; }
    fi
    ropero_ok "$name descargado"
}


ropero_download_safetensors() {
    local repo="${1:-}"
    local modelname="${2:-}"
    local size="${3:-}"

    local dest_dir="${4:-${SAFETENSORS_DIR:-$HOME/models/safetensors}/$modelname}"
    local marker="$dest_dir/.download_complete"

    if [[ -z "$repo" || -z "$modelname" ]]; then
        ropero_err "ropero_download_safetensors: argumentos incompletos (repo='$repo' modelname='$modelname')"
        return 1
    fi

    if [[ -z "$dest_dir" ]]; then
        ropero_err "ropero_download_safetensors: dest_dir vacío"
        return 1
    fi

    if [[ -f "$marker" ]]; then
        ropero_ok "$modelname ya descargado"
        return 0
    fi

    if ! ropero_ask "Descargar $modelname (~$size)? Necesario para ropero"; then
        ropero_info "$modelname omitido por el usuario"
        return 0
    fi

    mkdir -p "$dest_dir"

    if command -v hf &>/dev/null; then
        hf download "$repo" \
            --local-dir "$dest_dir" \
            || {
                ropero_err "falló la descarga de $modelname"
                return 1
            }
    else
        ropero_err "hf no disponible; no se puede descargar $modelname"
        return 1
    fi

    touch "$marker"

    ropero_ok "$modelname descargado"
}

# -----------------------------------------------------------------------------
# Resolver herramientas de build ignorando asdf shims (bypass homebrew/system)
# Réplica del setup_resolve_build_tool de setup.d/lib/common.sh para que ropero
# sea standalone (no depender del sistema setup). Mismo orden de búsqueda:
# /opt/homebrew/bin (Apple Silicon brew), /usr/local/bin (Intel brew), /usr/bin,
# y como fallback `command -v`. Lo usa 20-compile-llama.sh en la rama macOS
# para evitar que cmake/ninja caigan en un shim de asdf sin .tool-versions.
# -----------------------------------------------------------------------------
setup_resolve_build_tool() {
    local tool="$1"
    [[ -n "$tool" ]] || { ropero_err "setup_resolve_build_tool: nombre de tool vacío"; return 1; }

    for d in /opt/homebrew/bin /usr/local/bin /usr/bin; do
        if [[ -x "$d/$tool" ]]; then
            echo "$d/$tool"
            return 0
        fi
    done

    local found
    found=$(command -v "$tool" 2>/dev/null) || {
        ropero_err "setup_resolve_build_tool: '$tool' no encontrado (ni en /opt/homebrew/bin, /usr/local/bin, /usr/bin ni en PATH)"
        return 1
    }
    echo "$found"
    return 0
}