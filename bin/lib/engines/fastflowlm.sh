#!/usr/bin/env bash
# =============================================================================
# engines/fastflowlm.sh — FastFlowLM, la NPU AMD (spec §6, fila 6)
# =============================================================================
# Linux. El único engine que no acelera una GPU: descarga el trabajo a la
# NPU (XDNA) de los Ryzen AI. `flm serve` trae server OpenAI pero NO
# comprueba la key ("the key is not checked", su doc): por eso va detrás de
# endpoint.py (--backend flm), que es quien la exige.
#
# El gate de NPU (amdxdna + XRT) NO está en compatible (la tabla §6 sólo
# exige Linux): está DENTRO de install, que instala el stack del driver y
# se niega a compilar sin NPU detectable — instalar flm en una máquina sin
# NPU no produce nada utilizable.
# =============================================================================

rp_eng_fastflowlm_label() {
    printf 'fastflowlm · NPU AMD (XDNA), tras endpoint.py\n'
}

rp_eng_fastflowlm_compatible() {
    local os
    os="$(rp_detect_os)"
    case "$os" in
        cachyos|arch|linux) return 0 ;;
        *)
            printf 'fastflowlm: sólo Linux (%s)\n' "$os" >&2
            return 1
            ;;
    esac
}

rp_eng_fastflowlm_deps() {
    printf 'git cmake ninja\n'
}

# _rp_eng_flm_has_npu — 0 si hay NPU AMD (XDNA) utilizable. Las tres
# señales, de más a menos fiable: el device DRM que ve `flm validate`, el
# XRT que usa `flm run`, y el silicon que anuncia lscpu.
_rp_eng_flm_has_npu() {
    [[ "$(uname -s)" == "Linux" ]] || return 1
    grep -qiE 'GenuineAMD|AuthenticAMD' /proc/cpuinfo 2>/dev/null || return 1
    if [[ -e /dev/accel/accel0 ]]; then return 0; fi
    if command -v xrt-smi >/dev/null 2>&1; then
        if xrt-smi examine 2>/dev/null | grep -qiE 'npu|accel'; then return 0; fi
    fi
    if lscpu 2>/dev/null | grep -qiE 'xdna|ryzen ai'; then return 0; fi
    return 1
}

rp_eng_fastflowlm_install() {
    local force=0 a
    for a in "$@"; do
        case "$a" in
            --force) force=1 ;;
            --*) : ;;
        esac
    done

    _rp_engine_deps_ok git cmake ninja || return 1

    if rp_eng_fastflowlm_resolve >/dev/null && [[ "$force" != "1" ]]; then
        rp_msg ok "FastFlowLM ya instalado: $(rp_eng_fastflowlm_resolve)"
        return 0
    fi
    if [[ "$force" == "1" ]]; then
        rp_msg info "reinstalación forzada (--force)"
    fi

    if ! _rp_eng_flm_has_npu; then
        rp_msg error "no veo NPU AMD (XDNA) en esta máquina: no instalo FastFlowLM"
        rp_msg info "  hace falta /dev/accel/accel0 (driver amdxdna) o NPU listada en 'xrt-smi examine'"
        return 1
    fi

    # ── 1. Stack del sistema: xrt-plugin-amdxdna es el que hace que `flm
    #       run` (XRT) vea la NPU; sin él validate pasa y run falla.
    local p
    if command -v pacman >/dev/null 2>&1; then
        for p in xrt xrt-plugin-amdxdna linux-firmware-other; do
            if ! pacman -Qi "$p" >/dev/null 2>&1; then
                rp_msg info "instalando $p (pacman)..."
                sudo pacman -S --needed --noconfirm "$p" || \
                    rp_msg warn "no se pudo instalar $p"
            fi
        done
    else
        rp_msg warn "sin pacman: instala xrt, xrt-plugin-amdxdna y linux-firmware-other a mano"
    fi

    # amdxdna-dkms (AUR): en kernels 6.19+ el driver in-tree con firmware
    # 1.1 puede hacer desaparecer la NPU. Orden: headers → DKMS → depmod →
    # REBOOT (el reboot no se puede hacer desde aquí; se avisa).
    if ! modinfo amdxdna >/dev/null 2>&1 || \
       { modinfo -F filename amdxdna 2>/dev/null | grep -q '/dkms/'; }; then
        rp_msg info "instalando amdxdna-dkms (AUR)..."
        local dkms_ok=0
        if command -v paru >/dev/null 2>&1; then
            paru -S --needed --noconfirm amdxdna-dkms && dkms_ok=1
        elif command -v yay >/dev/null 2>&1; then
            yay -S --needed --noconfirm amdxdna-dkms && dkms_ok=1
        else
            rp_msg warn "sin paru/yay: instala amdxdna-dkms a mano (AUR)"
        fi
        if [[ "$dkms_ok" == "1" ]]; then
            sudo dkms autoinstall -k "$(uname -r)" || rp_msg warn "dkms falló"
            sudo depmod -a || true
            rp_msg warn "REBOOT necesario para que cargue el módulo DKMS"
        fi
    fi

    # Límite de memoria bloqueada: la NPU reserva memoria no swapeable.
    local memlock
    memlock="$(ulimit -l 2>/dev/null || echo unknown)"
    if [[ "$memlock" != "unlimited" ]]; then
        rp_msg warn "memlock = $memlock (la NPU necesita 'unlimited')"
        rp_msg info "  añade a /etc/security/limits.conf: * soft/hard memlock unlimited"
    fi

    # ── 2. Compilar el runtime (preset linux-default → /opt/fastflowlm) ──
    local src="$HOME/FastFlowLM"
    if [[ ! -d "$src/.git" || "$force" == "1" ]]; then
        rp_step 1 2 "clonando FastFlowLM (con submódulos)..."
        rm -rf "$src"
        git clone -q --recursive https://github.com/ROCm/FastFlowLM.git "$src" || {
            rp_msg error "falló el clone de FastFlowLM"
            return 1
        }
    fi
    rp_step 2 2 "compilando FastFlowLM (10-20 min)..."
    if ! ( cd "$src/src" && cmake --preset linux-default && \
           cmake --build build -j"$(_rp_engine_nproc)" && sudo cmake --install build ); then
        rp_msg error "FastFlowLM no compiló"
        rp_msg info "  deps de build: ninja libavformat-dev libavutil-dev libavcodec-dev"
        rp_msg info "  libswresample-dev libswscale-dev libxrt-dev uuid-dev libdrm-dev"
        rp_msg info "  guía: https://github.com/ROCm/FastFlowLM/blob/main/docs/linux-getting-started.md"
        return 1
    fi

    mkdir -p "$BIN_DIR" || return 1
    ln -sf /opt/fastflowlm/bin/flm "$BIN_DIR/flm" || return 1
    rp_msg ok "FastFlowLM instalado: $BIN_DIR/flm"
    return 0
}

rp_eng_fastflowlm_resolve() {
    # PATH → BIN_DIR → instalación del preset (/opt/fastflowlm).
    local cand ruta
    ruta="$(command -v flm 2>/dev/null)"
    [[ -n "$ruta" ]] && { printf '%s\n' "$ruta"; return 0; }
    for cand in "$BIN_DIR/flm" /opt/fastflowlm/bin/flm; do
        if [[ -x "$cand" ]]; then
            printf '%s\n' "$cand"
            return 0
        fi
    done
    return 1
}

rp_eng_fastflowlm_probe() {
    local puerto="${1:-}"
    if ! rp_eng_fastflowlm_resolve >/dev/null; then
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

rp_eng_fastflowlm_native_auth() {
    printf 'no\n'
    return 0
}

rp_eng_fastflowlm_launch() {
    local modelo="$1" puerto="$2" logfile="$3"
    shift 3

    local py
    if ! py="$(_rp_engine_python_with fastapi uvicorn)"; then
        rp_msg error "el endpoint de fastflowlm necesita fastapi y uvicorn, y no hay un python3 que los tenga"
        return 1
    fi
    if ! rp_eng_fastflowlm_resolve >/dev/null; then
        rp_msg error "fastflowlm no está instalado (ropero install engines fastflowlm)"
        return 1
    fi
    if [[ -z "${LLAMA_API_KEY:-}" ]]; then
        rp_msg error "falta LLAMA_API_KEY: el launcher (04) la resuelve antes de llamar al engine"
        return 1
    fi
    modelo="$(rp_model_field "$modelo" real)"
    # El modelo de flm es un TAG de su almacén, NO una ruta: se pasa tal
    # cual lo declara el TOML ([model].path).
    local tag
    tag="$(rp_model_field "$modelo" path)" || return 1
    if [[ -z "$tag" ]]; then
        rp_msg error "'$modelo' no declara [model].path (el TAG del almacén de FLM)"
        return 1
    fi

    # Args del modelo envueltos en --extra-arg: los reenvía a `flm serve`.
    local -a cmd=("$py" "${ROPERO_ROOT}/bin/server/endpoint.py"
        --backend flm
        --model-path "$tag"
        --model-id "$modelo"
        --host "${HOST:-127.0.0.1}"
        --port "$puerto"
        --api-key "$LLAMA_API_KEY"
        --backend-port $(( puerto + 1000 )))
    local a
    for a in "$@"; do
        cmd+=(--extra-arg "$a")
    done

    # Sin el chequeo de versión, flm hace una llamada de red en cada
    # arranque (ruido en una máquina offline). El prefijo `env` no contamina
    # el entorno del shell padre.
    mkdir -p "$(dirname "$logfile")" 2>/dev/null
    env FLM_DISABLE_UPDATE_CHECK="${FLM_DISABLE_UPDATE_CHECK:-1}" \
    nohup ${cmd[@]+"${cmd[@]}"} >>"$logfile" 2>&1 &
    printf '%s\n' "$!"
    return 0
}
