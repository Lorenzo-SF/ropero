#!/usr/bin/env bash
# =============================================================================
# engines/strata.sh — Strata (spec §6, fila 7)
# =============================================================================
# Strata NO es un server: es un Terminal (python) que baja un motor (el
# binario que habla con la GPU), deja el modelo en disco y sirve una API
# OpenAI-compatible. Lo instala el instalador oficial del propio proyecto:
#
#     curl -fsSL https://github.com/Niko1221/Strata/releases/latest/download/install.sh \
#         -o install.sh && sh install.sh
#
# El instalador NO necesita administrador: mete todo en el HOME
# (~/.local/share/strata) y deja un enlace `strata` en ~/.local/bin. El motor
# y el modelo NO vienen con el instalador: los baja `strata launch` la primera
# vez (del mismo release, con la firma comprobada).
#
# POR QUÉ VA DETRÁS DE endpoint.py
#
# · La clave: la pone el proxy. Strata se arranca con --access local
#   --no-api-key (sólo 127.0.0.1, sin red), así que la clave de ropero es la
#   de siempre y no hay dos claves que contradecir.
# · El nombre del modelo: el proxy reescribe el campo "model" al id de ropero.
#   El server de Strata lo acepta (sirve cualquier nombre: "Other names are
#   still served"), pero así el cliente ve el id de ropero.
# · El ciclo de vida: `strata launch --no-tui` DEVUELVE cuando el server ya
#   sirve (el motor queda vivo, pero lo gestiona Strata). El backend del
#   proxy necesita un proceso que siga vivo: de eso se encarga
#   bin/server/strata-run.sh, que además para lo que arrancó al morir.
#
# LOS MODELOS NO SON DE ROPERO
#
# El catálogo de modelos es el de Strata (el --family y el --model de su
# plan). Un modelo de ropero con engine=strata es un PUNTERO:
# [model].path = "<familia>/<paquete>" (qwen/q2_0, gyro/Gyro-S, ...). Los
# archivos los baja y los prepara Strata, en su carpeta, no en la de ropero.
#
# La InstALACIÓN DEL MODELO la hace Strata en el arranque: no hay modo de
# "descargar sin arrancar". El hook install_model enseña el PLAN (lo que se
# va a bajar) y lo dice, en vez de fingir que ha bajado nada.
# =============================================================================

rp_eng_strata_label() {
    printf 'strata · Strata (MoE entre VRAM/RAM/SSD), tras endpoint.py\n'
}

rp_eng_strata_compatible() {
    local os
    os="$(rp_detect_os)"
    case "$os" in
        cachyos|arch|linux|macos) return 0 ;;
        *)
            printf 'strata: sólo Linux y macOS (%s)\n' "$os" >&2
            return 1
            ;;
    esac
}

rp_eng_strata_deps() {
    # curl (el instalador) — wget sirve de alternativa, pero el instalador
    # oficial usa curl y es lo que se comprueba en la doc de Strata.
    printf 'curl\n'
}

# _rp_eng_strata_installer — la URL del instalador oficial (una sola fuente
# de verdad: la que usa la doc de Strata para el Terminal).
_RP_STRATA_INSTALLER="https://github.com/Niko1221/Strata/releases/latest/download/install.sh"

rp_eng_strata_install() {
    local force=0 a
    for a in "$@"; do
        case "$a" in
            --force) force=1 ;;
            --*) : ;;
        esac
    done

    _rp_engine_deps_ok curl || return 1

    if rp_eng_strata_resolve >/dev/null && [[ "$force" != "1" ]]; then
        rp_msg ok "Strata ya instalado: $(rp_eng_strata_resolve)"
        return 0
    fi
    if [[ "$force" == "1" ]]; then
        rp_msg info "reinstalación forzada (--force)"
    fi

    # El instalador se descarga a un fichero temporal y se ejecuta con sh:
    # un `curl | sh` no deja ver qué se va a ejecutar, y aquí el script
    # descarga un runtime privado (200 MB) y escribe en el HOME.
    local tmp
    tmp="$(mktemp "${TMPDIR:-/tmp}/strata-install.XXXXXX.sh")" || {
        rp_msg error "no pude crear el fichero temporal del instalador"
        return 1
    }
    rp_step 1 2 "descargando el instalador de Strata..."
    if ! curl -fsSL "$_RP_STRATA_INSTALLER" -o "$tmp"; then
        rm -f "$tmp"
        rp_msg error "no se pudo descargar el instalador de Strata"
        rp_msg info "  url: $_RP_STRATA_INSTALLER"
        return 1
    fi
    rp_step 2 2 "instalando el Terminal de Strata (sin permisos de root)..."
    # STRATA_BIN_DIR: el instalador por defecto mete el enlace en
    # ~/.local/bin, que es donde ropero lo busca (y donde el usuario lo
    # encuentra en el PATH).
    if ! env STRATA_BIN_DIR="$BIN_DIR" sh "$tmp"; then
        rm -f "$tmp"
        rp_msg error "el instalador de Strata falló"
        rp_msg info "  se puede instalar a mano: curl -fsSL $_RP_STRATA_INSTALLER -o install.sh && sh install.sh"
        return 1
    fi
    rm -f "$tmp"

    if ! rp_eng_strata_resolve >/dev/null; then
        rp_msg warn "el instalador terminó pero no encuentro el ejecutable 'strata'"
        rp_msg info "  mira si ~/.local/bin está en el PATH"
        return 1
    fi
    rp_msg ok "Strata instalado: $(rp_eng_strata_resolve)"
    rp_msg info "  el motor y el modelo los baja 'strata launch' la primera vez"
    return 0
}

rp_eng_strata_resolve() {
    # PATH → BIN_DIR → FARM_DIR → el enlace del instalador → la versión
    # concreta que hay en la carpeta de Strata.
    local cand
    cand="$(command -v strata 2>/dev/null)"
    [[ -n "$cand" ]] && { printf '%s\n' "$cand"; return 0; }
    for cand in "$BIN_DIR/strata" "$FARM_DIR/strata" \
                "$HOME/.local/bin/strata" \
                "$HOME/.local/share/strata/terminal/current/bin/strata"; do
        if [[ -x "$cand" ]]; then
            printf '%s\n' "$cand"
            return 0
        fi
    done
    return 1
}

rp_eng_strata_probe() {
    local puerto="${1:-}"
    if ! rp_eng_strata_resolve >/dev/null; then
        printf 'missing\n'
        return 1
    fi
    # El probe mira el PUERTO DE ROPERO (el proxy), no el interno de Strata:
    # lo que tiene que contestar es el endpoint de ropero, con su /health.
    if _rp_engine_http_ok "$puerto"; then
        printf 'ok\n'
        return 0
    fi
    printf 'down\n'
    return 1
}

rp_eng_strata_native_auth() {
    # NO: la clave la pone el proxy de ropero. Strata arranca con
    # --access local --no-api-key (sólo 127.0.0.1), así que no hace falta
    # que él la compruebe.
    printf 'no\n'
    return 0
}

# _rp_eng_strata_pack <modelo> — "<familia>/<paquete>" del [model].path.
#
# OJO: [model].path de un modelo de Strata NO es una ruta (es el par
# familia/paquete del catálogo de Strata), así que aquí NO se usa
# _rp_engine_model_path (que le pegaría el prefijo de $SAFETENSORS_DIR).
_rp_eng_strata_pack() {
    local modelo="${1:-}" pack
    pack="$(rp_model_field "$modelo" path 2>/dev/null)" || return 1
    [[ -n "$pack" ]] || return 1
    printf '%s' "$pack"
}

# _rp_eng_strata_plan <familia> <modelo> — el plan de Strata en JSON.
#
# Es la forma oficial de preguntar "qué vas a hacer": qué modelo resuelve,
# de qué familia, cuánto se descarga y con qué flags lo arrancaría. Con
# --plan no se cambia NADA.
_rp_eng_strata_plan() {
    local familia="${1:-}" modelo="${2:-}" bin
    bin="$(rp_eng_strata_resolve)" || return 1
    "$bin" launch --plan --family "$familia" --model "$modelo" --output json 2>/dev/null
}

rp_eng_strata_install_model() {
    local modelo="${1:-}" pack familia paquete plan
    [[ -n "$modelo" ]] || { rp_msg error "install_model: falta el modelo"; return 1; }

    if ! pack="$(_rp_eng_strata_pack "$modelo")"; then
        rp_msg error "'$modelo' no declara [model].path (el par familia/paquete de Strata)"
        return 1
    fi
    familia="${pack%%/*}"
    paquete="${pack##*/}"
    if [[ "$familia" == "$pack" || -z "$familia" || -z "$paquete" ]]; then
        rp_msg error "'$modelo': [model].path debe ser \"<familia>/<paquete>\" (p.ej. qwen/q2_0)"
        return 1
    fi

    if ! rp_eng_strata_resolve >/dev/null; then
        rp_msg error "Strata no está instalado (ropero install engines strata)"
        return 1
    fi

    # El plan es la única forma de saber qué se va a bajar: no hay modo de
    # "descargar sin arrancar" (el modelo se baja al arrancar el motor).
    plan="$(_rp_eng_strata_plan "$familia" "$paquete" 2>/dev/null)" || plan=""
    if [[ -z "$plan" ]]; then
        rp_msg warn "no pude pedirle el plan a Strata para '$familia/$paquete'"
        return 0
    fi
    local gb titulo avisos
    # El plan es JSON de UNA LÍNEA y con espacios tras los dos puntos
    # ("download": {"gb": 72.5}). Los patrones toleratean ese espacio: con
    # "s/.*"download":{.../" no casaba y el plan parecía vacío.
    # El "gb" que se busca es el de la descarga (el primero), NO el de una
    # pieza: con [^}]* (que es GREEDY) se cogía el "gb" de "parts" (66,4 en
    # vez de 72,5). Por eso el patrón no deja pasar una "{".
    gb="$(printf '%s' "$plan" | sed -n 's/.*"download" *: *{[^"{]*"gb" *: *\([0-9.]*\).*/\1/p')"
    titulo="$(printf '%s' "$plan" | sed -n 's/.*"model" *: *{[^}]*"title" *: *"\([^"]*\)".*/\1/p')"
    avisos="$(printf '%s' "$plan" | sed -n 's/.*"warnings" *: *\["\([^"]*\)".*/\1/p')"
    rp_msg ok "Strata: ${titulo:-$familia/$paquete}"
    if [[ -n "$gb" && "$gb" != "0" ]]; then
        rp_msg info "  hay que bajar ${gb} GB: lo baja Strata al arrancar (no hay modo de descargar sin servir)"
    else
        rp_msg info "  no hay nada que bajar: el modelo ya está en disco"
    fi
    if [[ -n "$avisos" ]]; then
        rp_msg warn "  Strata avisa: $avisos"
    fi
    rp_msg warn "  no se ha bajado NADA: el modelo lo baja Strata al arrancar"
    rp_msg info "  para bajarlo ahora: ROPERO_START_TIMEOUT=7200 ropero use $modelo"
    return 0
}

rp_eng_strata_launch() {
    local modelo="$1" puerto="$2" logfile="$3"
    shift 3

    local py
    if ! py="$(_rp_engine_python_with fastapi uvicorn)"; then
        rp_msg error "el endpoint de strata necesita fastapi y uvicorn, y no hay un python3 que los tenga"
        rp_msg info "  (ropero install engines strata los deja en su venv)"
        return 1
    fi
    local bin
    if ! bin="$(rp_eng_strata_resolve)"; then
        rp_msg error "Strata no está instalado (ropero install engines strata)"
        return 1
    fi

    local pack familia paquete
    if ! pack="$(_rp_eng_strata_pack "$modelo")"; then
        rp_msg error "'$modelo' no declara [model].path (el par familia/paquete de Strata)"
        return 1
    fi
    familia="${pack%%/*}"
    paquete="${pack##*/}"

    # El contexto: [model].context es el que se le pide a Strata.
    local ctx=""
    ctx="$(rp_model_field "$modelo" context)" || ctx=""

    # El preset con el que se lanza: su nombre ES el mecanismo para pararlo
    # sin tocar los demás Strata del usuario (ver bin/server/strata-run.sh).
    local preset="ropero-${modelo}"

    # El backend del proxy: el puente que lanza Strata y se queda vivo.
    local puente="${ROPERO_ROOT}/bin/server/strata-run.sh"
    if [[ ! -x "$puente" ]]; then
        rp_msg error "falta el puente del engine strata: $puente"
        return 1
    fi

    local -a cmd=("$py" "${ROPERO_ROOT}/bin/server/endpoint.py"
        --backend strata
        --model-path "$familia/$paquete"
        --model-id "$modelo"
        --host "${HOST:-0.0.0.0.1}"
        --port "$puerto"
        --api-key "${LLAMA_API_KEY:-}"
        --backend-port $(( puerto + 1000 ))
        --backend-bin "$puente")
    if [[ -n "$ctx" && "$ctx" != "0" ]]; then
        cmd+=(--context "$ctx")
    fi
    # Los flags del [args] del TOML: son flags de `strata launch` (--vision
    # off, --mtp off, --tune quick, --kv q4_0, --drafting mtp, --set ...).
    # El puente los pone AL FINAL, después del contexto.
    local a
    for a in "$@"; do
        cmd+=(--extra-arg "$a")
    done

    mkdir -p "$(dirname "$logfile")" 2>/dev/null
    nohup ${cmd[@]+"${cmd[@]}"} >>"$logfile" 2>&1 </dev/null &
    printf '%s\n' "$!"
    return 0
}
