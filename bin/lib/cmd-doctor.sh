#!/usr/bin/env bash
# =============================================================================
# cmd-doctor.sh — comandos de diagnóstico: doctor (alias check) y config
# (spec §8.9 y §8.12, fase 3)
# =============================================================================
# doctor verifica la instalación y sale 0/1 según haya o no críticos:
#   crítico (✗) = python3<3.8 · directorios no escribibles · NINGÚN engine
#   compatible instalado. Todo lo demás (binarios de setup, engines
#   individuales no instalados, modelos fuera de disco, API key) es aviso.
#
# config pinta la tabla ENGINE · COMPATIBLE · INSTALADO · MODELOS (los 6) o
# el detalle de un engine. Es SILENCIOSA ante engines rotos: un engine cuyo
# resolve/probe falla no tumba la tabla.
#
# Toda la salida de estos comandos es humana → stderr (00-utils). El
# veredicto va en el exit code.
# =============================================================================

if ! declare -F rp_engine_list >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/03-engines.sh"
fi

# ── Contadores del doctor ────────────────────────────────────────────────────
RP_DOCTOR_BAD=0      # críticos: deciden el exit
RP_DOCTOR_WARN=0     # avisos

_rp_doctor_ok()   { rp_msg ok "$*"; }
_rp_doctor_warn() { RP_DOCTOR_WARN=$((RP_DOCTOR_WARN + 1)); rp_msg warn "$*"; }
_rp_doctor_bad()  { RP_DOCTOR_BAD=$((RP_DOCTOR_BAD + 1)); rp_msg error "$*"; }

# _rp_doctor_cmd_ok <cmd> — ✓ si el comando existe; ⚠ si falta.
_rp_doctor_cmd_ok() {
    local cmd="$1"
    if command -v "$cmd" >/dev/null 2>&1; then
        _rp_doctor_ok "$cmd en su sitio"
    else
        _rp_doctor_warn "falta '$cmd' (ropero install setup)"
    fi
}

# _rp_doctor_dir_ok <var> <ruta> — directorio creado y escribible (crítico).
_rp_doctor_dir_ok() {
    local nombre="$1" ruta="$2" prueba
    if ! mkdir -p "$ruta" 2>/dev/null; then
        _rp_doctor_bad "$nombre no se puede crear: $ruta"
        return 1
    fi
    prueba="$ruta/.rp-doctor.$$"
    if ! touch "$prueba" 2>/dev/null; then
        _rp_doctor_bad "$nombre no es escribible: $ruta"
        return 1
    fi
    rm -f "$prueba" 2>/dev/null
    _rp_doctor_ok "$nombre escribible: $ruta"
}

# -----------------------------------------------------------------------------
# doctor / check
# -----------------------------------------------------------------------------
_rp_doctor_help() {
    cat <<'EOF'
USO: ropero doctor
     ropero check     (alias)

Verifica la instalación: bash, python3>=3.8, binarios de setup (curl/wget,
tar, git, uv, hf, jq, cmake, ninja), SO y paquetero, directorios de trabajo
(GGUF/SAFETENSORS/LOG/STATE), API key, engines (compatibles, instalados y
qué hay en el slot :$PORT) y modelos activos presentes en disco.

Salida ✓ (bien) / ⚠ (aviso) / ✗ (crítico). Exit 0 si no hay críticos, 1 si
hay. Crítico: python3, directorios, o que NINGÚN engine compatible esté
instalado. Un engine individual sin instalar es un aviso, no un fallo.
EOF
}

rp_cmd_doctor() {
    local a
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_doctor_help; return 0 ;;
            *)         rp_usage_error doctor "doctor no acepta argumentos (recibido: '$a')" ;;
        esac
    done

    RP_DOCTOR_BAD=0
    RP_DOCTOR_WARN=0

    rp_header "doctor de ropero ${ROPERO_VERSION:-}"

    # ── Shell ─────────────────────────────────────────────────────────────
    # bash 3.2 es el objetivo (ADR-1): 3.2 no es fallo. bash>=4 es una
    # recomendación (algunas herramientas del ecosistema lo suponen).
    if (( BASH_VERSINFO[0] >= 4 )); then
        _rp_doctor_ok "bash $BASH_VERSION"
    else
        _rp_doctor_warn "bash $BASH_VERSION (3.2 vale; >=4 recomendado para otras herramientas)"
    fi

    # ── python3 (crítico: sin él no hay catálogo ni endpoints) ───────────
    local py=""
    if [[ -n "${ROPERO_PYTHON:-}" ]]; then
        py="$(command -v "$ROPERO_PYTHON" 2>/dev/null || printf '%s' "$ROPERO_PYTHON")"
    fi
    if [[ -z "$py" ]]; then
        py="$(command -v python3 2>/dev/null)"
    fi
    if [[ -z "$py" ]]; then
        _rp_doctor_bad "no hay python3 (ropero necesita >= 3.8)"
    elif ! "$py" -c 'import sys; sys.exit(0 if sys.version_info >= (3, 8) else 1)'; then
        _rp_doctor_bad "python3 demasiado viejo: $("$py" --version 2>&1) (hace falta >= 3.8)"
    else
        _rp_doctor_ok "python3 $($py --version 2>&1) ($py)"
    fi

    # ── Binarios de setup (avisos) ───────────────────────────────────────
    if command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1; then
        _rp_doctor_ok "cliente HTTP (curl/wget) disponible"
    else
        _rp_doctor_warn "ni curl ni wget: los probes de readiness no funcionan"
    fi
    local c
    for c in tar git uv hf jq cmake ninja; do
        _rp_doctor_cmd_ok "$c"
    done

    # ── SO y paquetero ───────────────────────────────────────────────────
    local os
    os="$(rp_detect_os)"
    case "$os" in
        macos)
            if command -v brew >/dev/null 2>&1; then
                _rp_doctor_ok "SO macOS · brew disponible"
            else
                _rp_doctor_warn "SO macOS sin brew (ropero install setup lo ofrece)"
            fi
            ;;
        cachyos|arch)
            if command -v pacman >/dev/null 2>&1; then
                _rp_doctor_ok "SO $os · pacman disponible"
            else
                _rp_doctor_warn "SO $os sin pacman"
            fi
            ;;
        *)
            _rp_doctor_warn "SO no reconocido para install setup: $os"
            ;;
    esac

    # ── Directorios (críticos) ───────────────────────────────────────────
    _rp_doctor_dir_ok GGUF_DIR "$GGUF_DIR"
    _rp_doctor_dir_ok SAFETENSORS_DIR "$SAFETENSORS_DIR"
    _rp_doctor_dir_ok LOG_DIR "$LOG_DIR"
    _rp_doctor_dir_ok STATE_DIR "$STATE_DIR"

    # ── API key (aviso: el primer arranque la genera, §5.5) ──────────────
    if [[ -n "${LLAMA_API_KEY:-}" ]]; then
        _rp_doctor_ok "API key fijada en el entorno (LLAMA_API_KEY)"
    elif [[ -f "$STATE_DIR/api_key" ]]; then
        _rp_doctor_ok "API key en $STATE_DIR/api_key"
    else
        _rp_doctor_warn "sin API key: la generará el primer arranque ($STATE_DIR/api_key)"
    fi

    # ── Engines: compatible / instalado / probe del slot ─────────────────
    # Crítico sólo si NINGÚN engine compatible está instalado: un engine
    # individual sin instalar es un aviso (se instala bajo demanda).
    local id compatibles_instalados=0 bin
    for id in $RP_ENGINE_IDS; do
        if rp_engine_call "$id" compatible >/dev/null 2>&1; then
            if bin="$(rp_engine_call "$id" resolve 2>/dev/null)"; then
                compatibles_instalados=$((compatibles_instalados + 1))
                _rp_doctor_ok "engine $id instalado ($bin)"
                # Probe del slot: informativo. "down" no es fallo — es que
                # no hay nada sirviéndose AHORA (eso lo gestiona `status`).
                local estado
                estado="$(rp_engine_call "$id" probe "$PORT" 2>/dev/null)"
                if [[ "$estado" == "ok" ]]; then
                    _rp_doctor_ok "engine $id sirviéndose en :$PORT"
                fi
            else
                _rp_doctor_warn "engine $id compatible pero NO instalado (ropero install engines $id)"
            fi
        else
            rp_msg info "engine $id no compatible en este SO/hw — omitido"
        fi
    done
    if (( compatibles_instalados == 0 )); then
        _rp_doctor_bad "NINGÚN engine compatible instalado (ropero install engines)"
    fi

    # ── Modelos activos en disco (avisos; los instala `ropero models`) ───
    if rp_mc_load; then
        local i nombre faltan=0
        for (( i = 0; i < ${#MODEL_NAMES[@]}; i++ )); do
            [[ "${MODEL_ISALIAS[$i]}" == "0" ]] || continue
            [[ "${MODEL_ORIGIN[$i]}" == "active" ]] || continue
            nombre="${MODEL_NAMES[$i]}"
            case "${MODEL_FORMAT[$i]}" in
                gguf)
                    if [[ -n "${MODEL_GGUF[$i]}" && ! -e "$GGUF_DIR/$nombre/${MODEL_GGUF[$i]}" ]]; then
                        _rp_doctor_warn "modelo '$nombre' sin su GGUF: $GGUF_DIR/$nombre/${MODEL_GGUF[$i]}"
                        faltan=$((faltan + 1))
                    fi
                    ;;
                safetensors)
                    if [[ "${MODEL_ENGINE[$i]}" == "fastflowlm" ]]; then
                        # El modelo de flm es un TAG de su almacén: no se
                        # puede comprobar en disco local.
                        continue
                    fi
                    if [[ -n "${MODEL_PATH[$i]}" && ! -d "$SAFETENSORS_DIR/${MODEL_PATH[$i]}" ]]; then
                        _rp_doctor_warn "modelo '$nombre' sin checkpoint: $SAFETENSORS_DIR/${MODEL_PATH[$i]}"
                        faltan=$((faltan + 1))
                    fi
                    ;;
            esac
        done
        if (( faltan == 0 )); then
            _rp_doctor_ok "modelos activos presentes en disco"
        fi
    else
        _rp_doctor_warn "no se pudo cargar el catálogo de modelos"
    fi

    # ── Resumen y veredicto ──────────────────────────────────────────────
    if (( RP_DOCTOR_BAD > 0 )); then
        rp_msg error "doctor: ${RP_DOCTOR_BAD} crítico(s), ${RP_DOCTOR_WARN} aviso(s)"
        return 1
    fi
    if (( RP_DOCTOR_WARN > 0 )); then
        rp_msg warn "doctor: sin críticos, ${RP_DOCTOR_WARN} aviso(s)"
    else
        rp_msg ok "doctor: todo en orden"
    fi
    return 0
}

# check — alias de doctor (spec §8.12). Comando propio en la tabla de ruteo.
rp_cmd_check() {
    rp_cmd_doctor "$@"
}

# -----------------------------------------------------------------------------
# config [engine]
# -----------------------------------------------------------------------------
_rp_config_help() {
    cat <<'EOF'
USO: ropero config [engine]

Sin argumentos: tabla de los 6 engines — ENGINE · COMPATIBLE · INSTALADO ·
MODELOS (cuántos modelos activos del catálogo usan cada engine). Con el id
de un engine: su detalle (etiqueta, compatibilidad con la razón, binario
resuelto, dependencias, auth y modelos).

Ejemplos:
  ropero config
  ropero config mlx-serve
EOF
}

# _rp_config_models_of <engine-id> — nº de modelos ACTIVOS de ese engine.
_rp_config_models_of() {
    rp_mc_load || return 1
    local motor="$1" i n=0
    for (( i = 0; i < ${#MODEL_NAMES[@]}; i++ )); do
        [[ "${MODEL_ISALIAS[$i]}" == "0" ]] || continue
        [[ "${MODEL_ORIGIN[$i]}" == "active" ]] || continue
        [[ "${MODEL_ENGINE[$i]}" == "$motor" ]] && n=$((n + 1))
    done
    printf '%s' "$n"
}

rp_cmd_config() {
    local a
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_config_help; return 0 ;;
        esac
    done

    if [[ $# -gt 1 ]]; then
        rp_usage_error config "config acepta UN engine como máximo (recibidos: $*)"
    fi

    # ── Detalle de un engine ─────────────────────────────────────────────
    if [[ $# -eq 1 ]]; then
        local id="$1"
        rp_engine_load "$id" || return 2
        local canon
        canon="$(rp_engine_canonical "$id")"

        rp_header "engine: $canon"
        rp_msg info "etiqueta:  $(rp_engine_call "$canon" label)"
        if rp_engine_call "$canon" compatible >/dev/null 2>&1; then
            rp_msg ok "compatible con este SO/hw"
        else
            rp_msg warn "NO compatible con este SO/hw — motivo:"
            # stdout fuera; la razón del engine sale por stderr (spec §5.4)
            rp_engine_call "$canon" compatible >/dev/null
        fi
        local bin
        if bin="$(rp_engine_call "$canon" resolve 2>/dev/null)"; then
            rp_msg ok "instalado: $bin"
        else
            rp_msg warn "no instalado (ropero install engines $canon)"
        fi
        rp_msg info "auth nativa: $(rp_engine_call "$canon" native_auth)"
        local deps faltan="" d
        deps="$(rp_engine_call "$canon" deps)"
        for d in $deps; do
            command -v "$d" >/dev/null 2>&1 || faltan="${faltan:+$faltan }$d"
        done
        if [[ -n "$faltan" ]]; then
            rp_msg warn "dependencias ausentes: $faltan"
        else
            rp_msg ok "dependencias al día: $deps"
        fi
        rp_msg info "modelos activos: $(_rp_config_models_of "$canon")"
        return 0
    fi

    # ── Tabla de los 6 (silenciosa ante engines rotos: un hook que falla
    #    deja la celda en "error", no rompe la tabla) ─────────────────────
    rp_header "engines de ropero"
    printf '%-15s %-12s %-10s %s\n' 'ENGINE' 'COMPATIBLE' 'INSTALADO' 'MODELOS' >&2
    local id comp inst modelos
    for id in $RP_ENGINE_IDS; do
        if rp_engine_call "$id" compatible >/dev/null 2>&1; then
            comp='sí'
        else
            comp='no'
        fi
        if rp_engine_call "$id" resolve >/dev/null 2>&1; then
            inst='sí'
        else
            inst='no'
        fi
        modelos="$(_rp_config_models_of "$id" 2>/dev/null || printf '0')"
        printf '%-15s %-12s %-10s %s\n' "$id" "$comp" "$inst" "$modelos" >&2
    done
    rp_msg info "detalle por engine: ropero config <engine>"
    return 0
}
