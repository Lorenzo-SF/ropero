#!/usr/bin/env bash
# =============================================================================
# 01-env.sh — entorno de ropero 1.0 (spec §4)
# =============================================================================
# Source seguro: SOLO defaults, cero ejecución. Ni un mkdir: los directorios
# los crea rp_env_setup, y la llama sólo el comando que los necesita (help y
# --version no tocan disco). El entorno del usuario SIEMPRE gana sobre el
# default (patrón : "${VAR:=default}").
#
# bash 3.2, comentarios y mensajes en español (spec §3).
# =============================================================================

# ROPERO_ROOT: lo resuelve el dispatcher (bin/ropero) siguiendo los symlinks
# de $0. No se toca aquí: este fichero no ejecuta nada al sourcearse.

# GGUFs del catálogo + auxiliares de cada modelo en $GGUF_DIR/<modelo>/
: "${GGUF_DIR:=$HOME/models/gguf}"
# Checkpoints safetensors (mlx, airllm, vllm)
: "${SAFETENSORS_DIR:=$HOME/models/safetensors}"
# Logs de arranque, uno por modelo: <modelo>.log
: "${LOG_DIR:=$HOME/.logs/ropero}"
# Estado runtime: api_key y ports/<puerto>
: "${STATE_DIR:=$HOME/.local/state/ropero}"
# Clone del fork del motor llama (llama-server)
: "${LLAMA_DIR:=$HOME/llama.cpp}"
# Symlinks de binarios (ropero, llama-server, ...)
: "${BIN_DIR:=$HOME/.local/bin}"

# Bind de todos los servers (loopback: no exponemos nada a la red)
: "${HOST:=127.0.0.1}"
# Slot GPU/dGPU
: "${PORT:=9999}"
# Slot CPU. Sólo se GESTIONA en Linux; en macOS no existe el slot (spec §4,
# decisión de diseño: --cpu en macOS es error de uso, ver §5.5).
: "${CPU_PORT:=9998}"

# Timeout de arranque/espera de modelo, en segundos.
: "${ROPERO_START_TIMEOUT:=300}"

# LLAMA_API_KEY: SIN default aquí. La resuelve rp_api_key (04-launch.sh,
# spec §5.5): env si está fijada, si no $STATE_DIR/api_key, si no se genera.
#
# HF_TOKEN: la trae el entorno del usuario cuando hace falta; ropero sólo la
# pasa a `hf download`. Tampoco tiene default aquí.

# Sin iGPU: no existe puerto, variable ni rastro de la iGPU (spec §1).

# rp_env_setup — crea los directorios de trabajo. Devuelve 1 si mkdir falla
# (directorio sin permisos es error real, no algo que tragarse en silencio).
rp_env_setup() {
    mkdir -p "$GGUF_DIR" "$SAFETENSORS_DIR" "$LOG_DIR" "$STATE_DIR" \
        "$LLAMA_DIR" "$BIN_DIR" || return 1
}

# rp_detect_os — macos | cachyos | arch | linux | otro (spec §4).
# F5/doctor lo usan para elegir brew o pacman.
rp_detect_os() {
    case "$(uname -s)" in
        Darwin)
            printf 'macos\n'
            ;;
        Linux)
            if [[ -f /etc/cachyos-release ]] || grep -qi cachyos /etc/os-release 2>/dev/null; then
                printf 'cachyos\n'
            elif [[ -f /etc/arch-release ]]; then
                printf 'arch\n'
            else
                printf 'linux\n'
            fi
            ;;
        *)
            printf 'otro\n'
            ;;
    esac
}
