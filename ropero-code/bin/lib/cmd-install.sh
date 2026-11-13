#!/usr/bin/env bash
# =============================================================================
# cmd-install.sh — install setup/engines/models/link/opencode-plugin
# (spec §8.13, fase 5)
# =============================================================================
# `ropero install` (bare) = setup + engines + models, INTERACTIVO: sin TTY y
# sin subcomando explícito es error de uso (exit 2). Los subcomandos sí
# admiten --yes para flujo desatendido granular.
#
# CUDA no se toca aquí: la instala el engine que la pida (install engines,
# spec §8.13). Este fichero tampoco instala opencode ni alaja (no-objetivo).
#
# `engines`/`link`/`setup` llegan además como comandos top-level desde el
# dispatcher (tabla de ruteo §2.1); `models` despacha a cmd-models.sh.
#
# bash 3.2: sin namerefs, sin arrays asociativos, sin mapfile (spec §3).
# Seguro de sourcear: sólo definiciones, cero ejecución.
# =============================================================================

if ! declare -F rp_engine_call >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/03-engines.sh"
fi

# -----------------------------------------------------------------------------
# install (el comando con subcomandos)
# -----------------------------------------------------------------------------
_rp_install_help() {
    local sub="${1:-}"
    case "$sub" in
        setup)    _rp_setup_help ;;
        engines)  _rp_engines_help ;;
        models)
            if ! declare -F rp_cmd_models >/dev/null 2>&1; then
                # shellcheck source=/dev/null
                . "${ROPERO_ROOT}/bin/lib/cmd-models.sh"
            fi
            _rp_models_help ;;
        link)     _rp_link_help ;;
        opencode-plugin) _rp_plugin_help ;;
        *)
            cat <<'EOF'
USO: ropero install [setup|engines|models|link|opencode-plugin] [opciones]

Sin subcomando: setup + engines + models, INTERACTIVO (menús). Sin TTY no
hay modo unattended: usa los subcomandos por separado.

  install setup [--yes] [--dry-run]
        instala las dependencias base (python3>=3.8, curl/wget, tar, git,
        uv, hf, jq, cmake, ninja; brew/pacman según SO). Sólo lo que falte.
  install engines [engine...] [--force]
        instala los engines compatibles elegidos (sus propias deps).
  install models [modelo...] [--download|--build|--prio download|build] [--force] [--dry-run]
        baja o fabrica modelos (ver `ropero models --help`).
  install link
        deja el layout de instalación (delega en install.sh: symlinks de
        ~/.local/bin/ropero y ~/.local/share/ropero → <repo>/bin) y enlaza
        los engines instalados en ~/.local/bin.
  install opencode-plugin
        copia el plugin de opencode a ~/.config/opencode/plugins/.

`ropero setup|engines|models|link` son alias top-level de los subcomandos.
EOF
            ;;
    esac
    return 0
}

# _rp_install_models — install models = `ropero models` (cmd-models.sh).
_rp_install_models() {
    if ! declare -F rp_cmd_models >/dev/null 2>&1; then
        if [[ ! -f "${ROPERO_ROOT}/bin/lib/cmd-models.sh" ]]; then
            rp_msg error "falta bin/lib/cmd-models.sh (fase 5)"
            return 1
        fi
        # shellcheck source=/dev/null
        . "${ROPERO_ROOT}/bin/lib/cmd-models.sh"
    fi
    rp_cmd_models "$@"
}

rp_cmd_install() {
    local sub="" a
    local -a flags=() sel=()
    local _skip=0
    for a in "$@"; do
        if (( _skip == 1 )); then
            flags+=("--prio=$a")
            _skip=0
            continue
        fi
        case "$a" in
            --help|-h)
                _rp_install_help "${sub:-install}"
                return 0 ;;
            setup|engines|models|link|opencode-plugin)
                [[ -z "$sub" ]] || \
                    rp_usage_error install "destino repetido: '$a' (ya tenía '$sub')"
                sub="$a" ;;
            --force|-f|--yes|-y|--dry-run|--download|--build)
                flags+=("$a") ;;
            --prio)
                _skip=1 ;;
            --prio=*|--prio)
                flags+=("$a") ;;
            --*)
                rp_usage_error install "opción desconocida: '$a'" ;;
            *)
                [[ -z "$sub" ]] && \
                    rp_usage_error install "destino desconocido: '$a' (setup|engines|models|link|opencode-plugin)"
                sel+=("$a") ;;
        esac
    done

    case "$sub" in
        setup)
            rp_cmd_setup ${flags[@]+"${flags[@]}"} ${sel[@]+"${sel[@]}"} ;;
        engines)
            rp_cmd_engines ${sel[@]+"${sel[@]}"} ${flags[@]+"${flags[@]}"} ;;
        models)
            _rp_install_models ${sel[@]+"${sel[@]}"} ${flags[@]+"${flags[@]}"} ;;
        link)
            rp_cmd_link ${sel[@]+"${sel[@]}"} ${flags[@]+"${flags[@]}"} ;;
        opencode-plugin)
            _rp_install_opencode_plugin ${sel[@]+"${sel[@]}"} ;;
        '')
            # install bare (§8.13): SIN modo unattended. Sin TTY, error de
            # uso: nada de menús que se tragan un EOF y eligen por su cuenta.
            if [[ ! -t 0 ]]; then
                rp_usage_error install "install es interactivo y no hay TTY: usa 'install setup|engines|models' o corre en una terminal"
            fi
            rp_cmd_setup || return $?
            rp_cmd_engines || return $?
            _rp_install_models || return $?
            return 0 ;;
    esac
    return $?
}

# -----------------------------------------------------------------------------
# install setup [--yes] [--dry-run]
# -----------------------------------------------------------------------------
_rp_setup_help() {
    cat <<'EOF'
USO: ropero install setup [--yes] [--dry-run]

Instala las dependencias base de ropero, SÓLO lo que falte:
  python3 >= 3.8, curl/wget, tar, git, uv, hf (uv tool install
  'huggingface_hub[cli]'), jq, cmake, ninja. macOS → brew (si falta, ofrece
  instalarlo con su script oficial); CachyOS/Arch → pacman (+ base-devel si
  no hay toolchain).

  bash >= 4: SÓLO aviso (3.2 es el objetivo soportado).
  CUDA: NO se instala aquí; la trae el engine que la pida (install engines).
  NO instala opencode ni alaja.

Opciones:
  --yes      no pregunta (elige sí en cada instalación)
  --dry-run  sólo lista lo que falta y lo que instalaría; no toca disco

Exit: 0 · 1 fallo de instalación · 2 uso.
EOF
}

# _rp_setup_py_ok — 0 si hay un python3 >= 3.8 en PATH.
_rp_setup_py_ok() {
    local py
    py="$(command -v python3 2>/dev/null)" || return 1
    "$py" -c 'import sys; sys.exit(0 if sys.version_info >= (3, 8) else 1)' 2>/dev/null
}

# _rp_setup_pkg <pkg-brew> <pkg-pacman> <nombre> — instala un paquete con el
# paquetero del SO. rc 1 si no hay receta para este SO.
_rp_setup_pkg() {
    local os
    os="$(rp_detect_os)"
    case "$os" in
        macos)
            brew install "$1" ;;
        cachyos|arch)
            sudo pacman -S --needed --noconfirm "$2" ;;
        *)
            rp_msg warn "no sé instalar '$3' en este SO ($os): instálalo a mano"
            return 1 ;;
    esac
    return $?
}

# _rp_setup_dep <cmd> <pkg-brew> <pkg-pacman> — una dependencia: check →
# (dry: listar / confirm: instalar) → recheck.
_rp_setup_dep() {
    local cmd="$1" brewpkg="$2" pacpkg="$3"
    local dry="${_RP_SETUP_DRY:-0}"
    if command -v "$cmd" >/dev/null 2>&1; then
        rp_msg ok "$cmd en su sitio"
        return 0
    fi
    if [[ "$dry" == "1" ]]; then
        rp_msg warn "falta $cmd → instalaría $brewpkg (brew) / $pacpkg (pacman)"
        return 0
    fi
    if ! rp_confirm "¿Instalar $cmd?"; then
        rp_msg info "$cmd omitido por el usuario"
        return 0
    fi
    _rp_setup_pkg "$brewpkg" "$pacpkg" "$cmd" || return 1
    if command -v "$cmd" >/dev/null 2>&1; then
        rp_msg ok "$cmd instalado"
        return 0
    fi
    rp_msg error "se instaló '$brewpkg/$pacpkg' pero '$cmd' sigue sin estar en PATH"
    return 1
}

rp_cmd_setup() {
    local a dry=0
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_setup_help; return 0 ;;
            --yes|-y)  export RP_YES=1 ;;
            --dry-run) dry=1 ;;
            *)         rp_usage_error setup "setup no acepta '$a' (sólo --yes y --dry-run)" ;;
        esac
    done
    _RP_SETUP_DRY="$dry"

    rp_header "setup de ropero"

    local os
    os="$(rp_detect_os)"

    # bash >= 4: SÓLO check+aviso (§8.13): 3.2 es el objetivo (ADR-1).
    if (( BASH_VERSINFO[0] >= 4 )); then
        rp_msg ok "bash $BASH_VERSION"
    else
        rp_msg warn "bash $BASH_VERSION (3.2 vale; >=4 recomendado para otras herramientas)"
    fi

    # SO y paquetero
    case "$os" in
        macos)
            if command -v brew >/dev/null 2>&1; then
                rp_msg ok "SO macOS · brew disponible"
            elif [[ "$dry" == "1" ]]; then
                rp_msg warn "falta brew → instalaría Homebrew (script oficial)"
            else
                rp_msg warn "SO macOS sin brew: casi todo lo demás se instala con brew"
                if rp_confirm "¿Instalar Homebrew con su script oficial?"; then
                    bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)" \
                        || { rp_msg error "falló la instalación de brew"; return 1; }
                    rp_msg ok "brew instalado"
                else
                    rp_msg info "brew omitido: instala las deps a mano o repite el setup"
                fi
            fi ;;
        cachyos|arch)
            if command -v pacman >/dev/null 2>&1; then
                rp_msg ok "SO $os · pacman disponible"
            else
                rp_msg warn "SO $os sin pacman: no puedo instalar paquetes"
            fi ;;
        *)
            rp_msg warn "SO no soportado por install setup: $os (deps a mano)" ;;
    esac

    # ── Dependencias base (sólo lo que falte) ───────────────────────────
    _rp_setup_dep curl curl curl || return 1
    _rp_setup_dep wget wget wget || return 1
    _rp_setup_dep tar tar tar || return 1
    _rp_setup_dep git git git || return 1
    _rp_setup_dep jq jq jq || return 1
    _rp_setup_dep cmake cmake cmake || return 1
    _rp_setup_dep ninja ninja ninja || return 1
    _rp_setup_dep uv uv uv || return 1

    # ── python3 >= 3.8 ──────────────────────────────────────────────────
    if _rp_setup_py_ok; then
        rp_msg ok "python3 $(python3 --version 2>&1) en su sitio"
    elif [[ "$dry" == "1" ]]; then
        rp_msg warn "falta python3>=3.8 → instalaría python (brew) / python (pacman)"
    else
        if rp_confirm "¿Instalar python3 (>= 3.8)?"; then
            _rp_setup_pkg python python python3 || return 1
        else
            rp_msg warn "python3 omitido: el catálogo de modelos no va a funcionar"
        fi
        if _rp_setup_py_ok; then
            rp_msg ok "python3 $(python3 --version 2>&1) en su sitio"
        else
            rp_msg error "python3 sigue sin responder (>= 3.8)"
            return 1
        fi
    fi

    # ── hf (huggingface_hub[cli]) vía uv tool ──────────────────────────
    if command -v hf >/dev/null 2>&1; then
        rp_msg ok "hf en su sitio"
    elif [[ "$dry" == "1" ]]; then
        rp_msg warn "falta hf → instalaría 'huggingface_hub[cli]' (uv tool install)"
    elif ! command -v uv >/dev/null 2>&1; then
        rp_msg warn "falta hf y no hay uv para instalarlo: uv tool install 'huggingface_hub[cli]'"
    elif rp_confirm "¿Instalar hf (uv tool install 'huggingface_hub[cli]')?"; then
        uv tool install 'huggingface_hub[cli]' || { rp_msg error "falló uv tool install"; return 1; }
        if command -v hf >/dev/null 2>&1; then
            rp_msg ok "hf instalado"
        else
            rp_msg error "uv terminó pero 'hf' no está en PATH (¿~/.local/bin en PATH?)"
            return 1
        fi
    else
        rp_msg info "hf omitido: las descargas de modelos no van a funcionar"
    fi

    # ── toolchain de C/C++ (sólo lo pide llama-server; base-devel en Arch,
    #    en macOS es xcode-select y NO lo instalamos aquí) ───────────────
    if [[ "$os" == "cachyos" || "$os" == "arch" ]] && ! command -v cc >/dev/null 2>&1; then
        if [[ "$dry" == "1" ]]; then
            rp_msg warn "falta toolchain → instalaría base-devel (pacman)"
        elif rp_confirm "¿Instalar base-devel (toolchain para compilar llama-server)?"; then
            sudo pacman -S --needed --noconfirm base-devel \
                || rp_msg warn "no se pudo instalar base-devel"
        fi
    fi
    if [[ "$os" == "macos" ]] && ! command -v cc >/dev/null 2>&1; then
        rp_msg warn "no hay toolchain C/C++ en macOS: xcode-select --install (no lo instalo aquí)"
    fi

    # Directorios de trabajo (en dry-run no se toca disco).
    if [[ "$dry" != "1" ]]; then
        rp_env_setup || { rp_msg error "no se pudieron crear los directorios de trabajo"; return 1; }
        rp_msg ok "directorios de trabajo listos"
    fi

    if [[ "$dry" == "1" ]]; then
        rp_msg ok "dry-run de setup completado (no se instaló nada)"
    else
        rp_msg ok "setup completado"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# install engines [engine...] [--force] — y alias top-level `ropero engines`
# -----------------------------------------------------------------------------
_rp_engines_help() {
    cat <<'EOF'
USO: ropero install engines [engine...] [--force]
     ropero engines ...               (alias)

Instala los engines elegidos, cada uno SUS dependencias (§6): llama-server
compila el fork turboquant, llama-server-official compila el llama.cpp de
upstream, vllm/mlx-serve/airllm/tensorrt-llm usan uv o brew, fastflowlm y
strata su canal oficial. CUDA (en Linux/NVIDIA) la instala el engine que la
pida: aquí no se toca nada más.

Sin engines abre un MENÚ multi-selección de los compatibles con este SO/hw;
sin TTY y sin selección explícita es error de uso.

Opciones:
  --force   reinstala aunque ya estén instalados

Engines: llama-server · llama-server-official · vllm · mlx-serve · airllm ·
         tensorrt-llm · fastflowlm · strata

Los binarios instalados quedan enlazados en ~/.local/bin (como los de
uv/brew). Los modelos de cada motor se distinguen por carpeta:
models/llama/ (fork) frente a models/llama-official/ (upstream).

Exit: 0 (aunque haya omitidos) · 1 si algún engine falla · 2 uso.
EOF
}

# _rp_engines_menu — lista humana de compatibles a stderr; pares "n|id" por
# stdout para el caller. (Mismo patrón que el menú de cmd-models.sh.)
_rp_engines_menu() {
    rp_header "engines compatibles"
    local id label n=0
    for id in $RP_ENGINE_IDS; do
        rp_engine_call "$id" compatible >/dev/null 2>&1 || continue
        label="$(rp_engine_call "$id" label 2>/dev/null)"
        n=$((n + 1))
        printf '  %d) %-14s %s\n' "$n" "$id" "${label:-}" >&2
        printf '%d|%s\n' "$n" "$id"
    done
    if (( n == 0 )); then
        rp_msg warn "ningún engine es compatible con este SO/hw"
    else
        rp_msg info "selección: números o ids separados por espacio · all · 0 para salir"
    fi
    return 0
}

# _rp_engines_menu_seleccion — pregunta hasta obtener selección válida;
# imprime los ids elegidos por stdout.
_rp_engines_menu_seleccion() {
    local menu
    menu="$(_rp_engines_menu)"
    [[ -n "$menu" ]] || return 0

    local -a nums=() ids=()
    local par n r
    while IFS= read -r par; do
        [[ "$par" == *"|"* ]] || continue
        n="${par%%|*}"
        r="${par#*|}"
        nums+=("$n")
        ids+=("$r")
    done <<<"$menu"

    local sel token id canon valido invalido
    while :; do
        printf 'Selecciona: ' >&2
        read -r sel || sel="0"
        [[ "$sel" == "0" ]] && return 0
        valido=0 invalido=0
        local -a elegidos=()
        # shellcheck disable=SC2086  # selección: tokens separados por espacio
        for token in $sel; do
            [[ "$token" == "all" ]] && { valido=1; continue; }
            id=""
            if [[ "$token" =~ ^[0-9]+$ ]]; then
                if (( token >= 1 && token <= ${#nums[@]} )); then
                    id="${ids[$((token - 1))]}"
                fi
            elif canon="$(rp_engine_canonical "$token" 2>/dev/null)"; then
                id="$canon"
            fi
            if [[ -n "$id" ]] && rp_engine_call "$id" compatible >/dev/null 2>&1; then
                elegidos+=("$id")
                valido=1
            else
                invalido=1
            fi
        done
        if (( valido == 1 && invalido == 0 )); then
            local -a unicos=() e
            for e in ${elegidos[@]+"${elegidos[@]}"}; do
                case " ${unicos[*]+"${unicos[*]}"} " in
                    *" $e "*) ;;
                    *) unicos+=("$e") ;;
                esac
            done
            printf '%s\n' ${unicos[@]+"${unicos[@]}"}
            return 0
        fi
        rp_msg warn "selección no válida o no compatible; prueba otra vez (0 para salir)"
    done
}

rp_cmd_engines() {
    local a force=0
    local -a pedidos=()
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_engines_help; return 0 ;;
            --force|-f) force=1 ;;
            --*)       rp_usage_error engines "opción desconocida: '$a' (sólo --force)" ;;
            *)         pedidos+=("$a") ;;
        esac
    done

    local -a sel=()
    if (( ${#pedidos[@]} == 0 )); then
        # Sin TTY no hay menú: exigir selección explícita (§8.13).
        if [[ ! -t 0 ]]; then
            rp_usage_error engines "sin TTY no hay selección: indica el engine(s) (ropero engines <id>...)"
        fi
        local linea
        while IFS= read -r linea; do
            [[ -n "$linea" ]] && sel+=("$linea")
        done < <(_rp_engines_menu_seleccion)
        (( ${#sel[@]} > 0 )) || { rp_msg info "nada seleccionado"; return 0; }
    else
        local id canon
        for id in ${pedidos[@]+"${pedidos[@]}"}; do
            canon="$(rp_engine_canonical "$id" 2>/dev/null)" || \
                rp_usage_error engines "engine desconocido: '$id' (válidos: $RP_ENGINE_IDS)"
            # De-duplicar: "ropero engines airllm airllm" es una instalación.
            case " ${sel[*]+"${sel[*]}"} " in
                *" $canon "*) ;;
                *) sel+=("$canon") ;;
            esac
        done
    fi

    local total=${#sel[@]}
    (( total > 0 )) || { rp_msg info "nada seleccionado"; return 0; }

    rp_env_setup || return 1

    local i=0 id rc=0 instalados=0 ya=0 omitidos=0 fallos=0
    for id in ${sel[@]+"${sel[@]}"}; do
        i=$((i + 1))
        rp_step "$i" "$total" "engine: $id"
        if ! rp_engine_call "$id" compatible >/dev/null 2>&1; then
            rp_msg warn "$id no es compatible con este SO/hw — omitido"
            omitidos=$((omitidos + 1))
            continue
        fi
        # Un engine sano no se reinstala salvo con --force.
        if [[ "$force" != "1" ]] && rp_engine_call "$id" resolve >/dev/null 2>&1; then
            rp_msg ok "$id ya estaba instalado: $(rp_engine_call "$id" resolve 2>/dev/null)"
            ya=$((ya + 1))
            continue
        fi
        if rp_engine_call "$id" install ${force:+--force}; then
            if rp_engine_call "$id" resolve >/dev/null 2>&1; then
                instalados=$((instalados + 1))
            else
                # El instalador dijo OK pero no queda utilizable: eso es un
                # fallo, no un éxito maquillado.
                rp_msg error "$id no quedó utilizable tras el install"
                fallos=$((fallos + 1))
                rc=1
            fi
        else
            fallos=$((fallos + 1))
            rc=1
        fi
    done

    rp_msg ok "engines: instalados=$instalados · ya estaban=$ya · omitidos=$omitidos · fallos=$fallos"
    return "$rc"
}

# -----------------------------------------------------------------------------
# install link — deja el LAYOUT de instalación de ropero
# -----------------------------------------------------------------------------
#
# LAYOUT (contrato lasaca v2; el mismo que deja install.sh):
#
#   <repo>/                    fuente de verdad (NO se mueve, NO se copia)
#     bin/ropero               entrypoint
#     bin/lib/                 módulos (.sh) que se sourcean
#   ~/.local/bin/ropero        → <repo>/bin/ropero   (lo único en el PATH)
#   ~/.local/share/ropero      → <repo>/bin          (symlink único)
#
# DÓNDE VAN LOS BINARIOS DE LOS ENGINES
#
# En $BIN_DIR (~/.local/bin), igual que los de uv/brew. La decisión vieja
# ("engines en el farm, fuera del PATH") se revierte con el layout v2: el
# farm ya no existe — la share es un symlink al repo y escribir dentro
# contaminaría el repo (R11) — y los hooks de install de los engines YA
# enlazaban en $BIN_DIR (llama-server.sh, llama-server-official.sh, vllm).
# La cadena de resolve de cada engine es PATH → $BIN_DIR → rutas largas,
# así que un motor enlazado en $BIN_DIR se resuelve igual que antes.
#
# `ropero install link` NO duplica la lógica del layout: delega en
# install.sh --local (migración incluida) y luego enlaza los engines.
# -----------------------------------------------------------------------------
_rp_link_help() {
    cat <<'EOF'
USO: ropero install link
     ropero link               (alias)

Deja el layout de instalación de ropero delegando en install.sh --local:
~/.local/bin/ropero → <repo>/bin/ropero y ~/.local/share/ropero → <repo>/bin
(migra el layout viejo si lo reconoce como suyo). Después enlaza en
~/.local/bin el binario de cada engine instalado, apuntando al binario
REAL resuelto.

Los binarios de los engines (llama-server, llama-server-official, vllm,
mlx-serve, trtllm-serve, flm) viven en ~/.local/bin, igual que los de
uv/brew: bajo la share nueva (un symlink al bin del repo) no puede
escribirse nada. airllm no lleva link: es una librería que sirve
bin/server/endpoint.py desde su intérprete.

No acepta selección: enlaza TODO lo que encuentre instalado. Es el comando
para reparar el PATH tras un cambio de máquina o de $HOME.

Exit: 0 · 1 fallo · 2 uso.
EOF
}

# _rp_link_real <ruta> — ruta real (siguiendo symlinks). SIN readlink -f: no
# existe en el bash 3.2 de macOS.
_rp_link_real() {
    local p="$1" dir base
    while [[ -L "$p" ]]; do
        dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
        base="$(readlink "$p")"
        [[ "$base" != /* ]] && base="$dir/$base"
        p="$base"
    done
    dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
    printf '%s\n' "$dir/$(basename "$p")"
}

# _rp_link_engines — enlaza el binario de cada engine instalado en
# $BIN_DIR (~/.local/bin), igual que hacen uv/brew. NUNCA dentro de la
# share: es un symlink al bin del repo y escribir dentro contaminaría el
# repo (R11).
_rp_link_engines() {
    local id bin real name dest

    for id in llama-server llama-server-official vllm mlx-serve tensorrt-llm fastflowlm strata; do
        if bin="$(rp_engine_call "$id" resolve 2>/dev/null)"; then
            [[ -n "$bin" && -x "$bin" ]] || continue
            name="${bin##*/}"
            dest="$BIN_DIR/$name"
            if [[ -e "$dest" && ! -L "$dest" ]]; then
                rp_msg warn "$dest existe y NO es un symlink: no lo he tocado"
                continue
            fi
            # Se apunta al binario REAL (symlinks resueltos): así el enlace
            # no se rompe si $BIN_DIR se vacía y se rellena.
            real="$(_rp_link_real "$bin" 2>/dev/null || true)"
            [[ -n "$real" ]] || real="$bin"
            if ln -sfn "$real" "$dest" 2>/dev/null; then
                rp_msg ok "$id → $dest"
            else
                rp_msg error "no se pudo enlazar $dest"
            fi
        else
            rp_msg info "$id: no instalado, nada que enlazar"
        fi
    done
    return 0
}

rp_cmd_link() {
    local a
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_link_help; return 0 ;;
            *)         rp_usage_error link "link no acepta selección (enlaza todo lo instalado; recibido: '$a')" ;;
        esac
    done

    rp_header "dejando el layout de ropero"

    local repo="${ROPERO_ROOT:-}"
    local rc=0

    if [[ -z "$repo" || ! -f "$repo/bin/ropero" ]]; then
        rp_msg error "no encuentro el entrypoint de ropero (ROPERO_ROOT=$repo)"
        return 1
    fi
    if [[ ! -f "$repo/install.sh" ]]; then
        rp_msg error "no encuentro el instalador del repo: $repo/install.sh"
        return 1
    fi

    # 1) El layout v2 (entrypoint + share + migración del viejo): lo hace
    #    install.sh. El override ROPERO_DIR/ROPERO_BIN_DIR se pasa explícito
    #    con env: aunque no esté exportado en la shell que llama, install.sh
    #    debe verlo (mismo patrón que tools/pacool --install).
    local rc_inst=0
    if [[ -n "${ROPERO_DIR:-}" && -n "${ROPERO_BIN_DIR:-}" ]]; then
        env ROPERO_DIR="$ROPERO_DIR" ROPERO_BIN_DIR="$ROPERO_BIN_DIR" \
            bash "$repo/install.sh" --local || rc_inst=1
    elif [[ -n "${ROPERO_DIR:-}" ]]; then
        env ROPERO_DIR="$ROPERO_DIR" bash "$repo/install.sh" --local || rc_inst=1
    elif [[ -n "${ROPERO_BIN_DIR:-}" ]]; then
        env ROPERO_BIN_DIR="$ROPERO_BIN_DIR" bash "$repo/install.sh" --local || rc_inst=1
    else
        bash "$repo/install.sh" --local || rc_inst=1
    fi
    if [[ "$rc_inst" == "0" ]]; then
        rp_msg ok "layout v2 dejado por install.sh --local"
    else
        rp_msg error "install.sh --local falló (ver su salida)"
        rc=1
    fi

    # 2) Los binarios de los engines → $BIN_DIR (nunca dentro de la share).
    mkdir -p "$BIN_DIR" || { rp_msg error "no se pudo crear $BIN_DIR"; return 1; }
    _rp_link_engines || rc=1

    # Comprobación final: el entrypoint responde.
    if bash "$repo/bin/ropero" --help >/dev/null 2>&1 </dev/null; then
        rp_msg ok "ropero --help responde"
    else
        rp_msg error "ropero --help NO responde: la instalación está rota"
        rc=1
    fi

    return "$rc"
}

# -----------------------------------------------------------------------------
# install opencode-plugin — copia (no symlink) el plugin de opencode
# -----------------------------------------------------------------------------
_rp_plugin_help() {
    cat <<'EOF'
USO: ropero install opencode-plugin

Copia opencode-plugin/index.ts a
~/.config/opencode/plugins/ropero-modelo/index.ts. COPIA, no symlink: el
watcher de opencode (inotify) no sigue symlinks, así que los cambios no se
recargarían. Si el destino difiere, se guarda backup .bak (numerado si ya
hubiera uno) antes de sobrescribir.

Al final comprueba si el config de opencode (~/.config/opencode/
opencode.jsonc o .json) declara el plugin "ropero-modelo" y avisa si no.

Exit: 0 · 1 fallo de copia · 2 uso.
EOF
}

_rp_install_opencode_plugin() {
    local a
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_plugin_help; return 0 ;;
            *)         rp_usage_error opencode-plugin "opencode-plugin no acepta argumentos (recibido: '$a')" ;;
        esac
    done

    local src="$ROPERO_ROOT/opencode-plugin/index.ts"
    local dst_dir="$HOME/.config/opencode/plugins/ropero-modelo"
    local dst="$dst_dir/index.ts"

    if [[ ! -f "$src" ]]; then
        rp_msg error "no encuentro el plugin en $src"
        rp_msg info "  (¿estás en una instalación de ropero completa?)"
        return 1
    fi

    mkdir -p "$dst_dir" || { rp_msg error "no pude crear $dst_dir"; return 1; }

    if [[ -f "$dst" ]] && cmp -s "$src" "$dst"; then
        rp_msg ok "el plugin ya está al día en $dst"
    else
        if [[ -f "$dst" ]]; then
            local bak="$dst.bak" n=0
            while [[ -e "$bak" ]]; do
                n=$((n + 1))
                bak="$dst.bak.$n"
            done
            cp -p "$dst" "$bak" 2>/dev/null && rp_msg info "copia anterior → $bak"
        fi
        if cp "$src" "$dst"; then
            rp_msg ok "plugin instalado en $dst"
        else
            rp_msg error "no pude copiar el plugin a $dst"
            return 1
        fi
    fi

    # ¿Lo declara el config de opencode?
    local cfg="$HOME/.config/opencode/opencode.jsonc"
    [[ -f "$cfg" ]] || cfg="$HOME/.config/opencode/opencode.json"
    if [[ -f "$cfg" ]]; then
        if grep -q "ropero-modelo" "$cfg"; then
            rp_msg ok "el config de opencode declara el plugin"
        else
            rp_msg warn "el config de opencode NO declara el plugin. Añade:"
            printf '    "plugins": [{ "package": "./plugins/ropero-modelo", "options": { "provider": "marichocho", "port": 9999 } }]\n' >&2
        fi
    else
        rp_msg warn "no encuentro el config de opencode (~/.config/opencode/opencode.jsonc)"
    fi
    return 0
}
