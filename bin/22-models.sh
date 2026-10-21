#!/usr/bin/env bash
# =============================================================================
# 22-models.sh — carga de los modelos de ropero (ficheros TOML)
# =============================================================================
# Seguro de sourcear (sólo definiciones + una función de carga).
#
# ── POR QUÉ LOS MODELOS SON TOML Y NO SHELL ────────────────────────────────────
#
# Antes cada modelo era un script bash que se sourceaba. Eso significaba que la
# "configuración" de un modelo era CÓDIGO: podía hacer cualquier cosa, y el
# loader no tenía forma de saberlo. Además obligaba a duplicar la información:
# el GGUF estaba en el .sh del modelo y otra vez en el catálogo de descargas, y
# si los dos no coincidían, `ropero install models` bajaba un fichero que el
# modelo luego no encontraba (un fallo que costó horas).
#
# Ahora un modelo es un fichero TOML con la MISMA forma para todos los engines:
#
#   models/<engine>/<modelo>.toml          → modelo ACTIVO
#   models/<engine>/fired/<modelo>.toml    → modelo RETIRADO (no se puede usar)
#
# El TOML sólo contiene DATOS. Nada de lo que hay dentro se ejecuta: este
# fichero los lee y los carga en variables. El esquema vive en
# `models/_template.toml`.
#
# ── EL FORMATO DEL DUMP ───────────────────────────────────────────────────────
#
# `bin/_modelconf.py dump` imprime un registro por modelo con los campos
# separados por el byte 0x1F (US). Se lee aquí con `IFS=$'\x1f' read -ra`.
#
# 0x1F y no el tabulador porque el tabulador es "espacio en blanco" para bash:
# `read -ra` con IFS=tabulador SE COME los campos vacíos, y aquí hay campos
# vacíos a propósito (un modelo sin mmproj no tiene mmproj). Con un separador
# que no es espacio, bash conserva los campos vacíos y el orden se mantiene.
# =============================================================================

# El cargador de modelos. Se resuelve por ruta real (readlink -f) para que
# funcione también a través de symlink, que es como se instala.
_ROPERO_MODELCONF=""
ropero_modelconf_path() {
    [[ -n "$_ROPERO_MODELCONF" ]] && { printf '%s' "$_ROPERO_MODELCONF"; return 0; }
    local cand
    for cand in "$SCRIPT_DIR/_modelconf.py" "$MODULES_DIR/_modelconf.py"; do
        if [[ -f "$cand" ]]; then
            _ROPERO_MODELCONF="$(readlink -f "$cand" 2>/dev/null || echo "$cand")"
            printf '%s' "$_ROPERO_MODELCONF"
            return 0
        fi
    done
    return 1
}

# ropero_python_ok → 0 si hay un python3 usable.
#
# Python es una dependencia dura desde que los modelos se configuran en TOML:
# no hay forma razonable de parsear TOML en bash sin reimplementar el parser y
# equivocarse en los detalles. Si falta, se dice claro y se sale: es mejor un
# error explícito que un launcher que carga la mitad de los modelos.
ropero_python_ok() {
    ropero_python_resolve >/dev/null 2>&1
}

# -----------------------------------------------------------------------------
# ropero_python_resolve → ruta a un intérprete de Python que FUNCIONE.
#
# ── POR QUÉ NO BASTA `command -v python3` ────────────────────────────────────
#
# En una máquina con asdf, el shim `python3` del PATH puede existir y estar
# ROTO: asdf responde "No version is set for command python3" y sale con
# código 1 cuando no hay un .tool-versions que fije la versión. `command -v`
# lo encuentra igual, así que comprobar sólo la existencia no dice nada.
#
# La regla es: un intérprete sirve si se puede EJECUTAR y es >= 3.8. Por eso
# aquí se prueba cada candidato, en vez de mirar si existe.
#
# El orden:
#   1. $ROPERO_PYTHON, si el usuario lo fija.
#   2. python3 / python del PATH, probados (el shim de asdf cae aquí y es
#      descartado por no funcionar).
#   3. Las rutas habituales del sistema.
#   4. Las instalaciones de asdf, que sí funcionan aunque su shim no.
# -----------------------------------------------------------------------------
ropero_python_resolve() {
    local cand p
    local -a candidates=()

    [[ -n "${ROPERO_PYTHON:-}" ]] && candidates+=("$ROPERO_PYTHON")

    for cand in python3 python; do
        p=$(command -v "$cand" 2>/dev/null) || continue
        [[ -n "$p" ]] && candidates+=("$p")
    done

    candidates+=(/usr/bin/python3 /usr/local/bin/python3 /opt/homebrew/bin/python3 /bin/python3)

    # Los glob que no casan se-literales: se filtran con el test -x.
    for cand in "$HOME"/.asdf/installs/python/*/bin/python3; do
        [[ -x "$cand" ]] && candidates+=("$cand")
    done

    for cand in "${candidates[@]}"; do
        [[ -x "$cand" ]] || continue
        if "$cand" -c 'import sys; sys.exit(0 if sys.version_info >= (3, 8) else 1)' 2>/dev/null; then
            printf '%s' "$cand"
            return 0
        fi
    done
    return 1
}

# -----------------------------------------------------------------------------
# ropero_model_abs_path <format> <path> → ruta absoluta del checkpoint.
#
# Los engines con checkpoint (airllm, mlx_lm, tensorrt-llm) guardan el modelo
# como un DIRECTORIO bajo $SAFETENSORS_DIR. El TOML guarda el nombre relativo
# ("gpt-oss-20b-MXFP4-Q8"); la ruta absoluta se compone aquí, una sola vez.
#
# Para format = "gguf" no hay checkpoint: se devuelve el valor tal cual.
# -----------------------------------------------------------------------------
ropero_model_abs_path() {
    local fmt="${1:-}" path="${2:-}"
    [[ -n "$path" ]] || { printf '%s' "$path"; return 0; }
    case "$fmt" in
        safetensors)
            if [[ "$path" == /* ]]; then printf '%s' "$path"
            else printf '%s/%s' "$SAFETENSORS_DIR" "$path"; fi
            ;;
        *) printf '%s' "$path" ;;
    esac
}

# -----------------------------------------------------------------------------
# _ropero_b64_decode <b64> → texto decodificado.
#
# GNU usa `base64 -d` y macOS/BSD usan `base64 -D`. Se prueban los dos: en
# GNU el primero funciona, en BSD el primero falla y entra el segundo.
# -----------------------------------------------------------------------------
_ropero_b64_decode() {
    local b64="${1:-}"
    [[ -n "$b64" ]] || return 0
    printf '%s' "$b64" | base64 -d 2>/dev/null && return 0
    printf '%s' "$b64" | base64 -D 2>/dev/null
}

# -----------------------------------------------------------------------------
# _ropero_json_num <json> <clave> → el número de esa clave, o vacío.
#
# El JSON que llega aquí es el que produce el cargador: un objeto plano con
# números, sin comas dentro de los valores. Un `sed` con una captura es
# suficiente y evita depender de `jq`, que no está en todas las máquinas.
# -----------------------------------------------------------------------------
_ropero_json_num() {
    local json="${1:-}" key="${2:-}"
    [[ -n "$json" && -n "$key" ]] || return 0
    printf '%s' "$json" \
        | sed -n "s/.*\"$key\"[[:space:]]*:[[:space:]]*\([0-9][0-9.]*\).*/\1/p"
}

# -----------------------------------------------------------------------------
# _ropero_model_limits <modelo> <limits_b64>
#
# Carga los límites declarados en [limits] del TOML:
#
#   min_ram_gb         RAM unificada mínima (GB) que necesita el modelo.
#   ram_fraction       fracción de la RAM unificada que se puede ocupar (0..1).
#   kv_kib_per_token   KV cache por token de contexto.
#
# El TOML es dato puro: aquí no hay ninguna decisión, sólo se copian los
# números a los arrays. La comprobación la hace el CLI (ropero_model_check_limits).
# -----------------------------------------------------------------------------
_ropero_model_limits() {
    local name="${1:-}" b64="${2:-}" json
    json="$(_ropero_b64_decode "$b64")"
    [[ -n "$json" ]] || return 0
    MODEL_MIN_RAM_GB["$name"]="$(_ropero_json_num "$json" min_ram_gb)"
    MODEL_RAM_FRACTION["$name"]="$(_ropero_json_num "$json" ram_fraction)"
    MODEL_KV_KIB_PER_TOKEN["$name"]="$(_ropero_json_num "$json" kv_kib_per_token)"
    return 0
}

# -----------------------------------------------------------------------------
# _ropero_register_alias <key> <nombre visible> [nivel]
#
# Registra un alias público: una key más que apunta al MODELO FÍSICO que se
# está leyendo en este momento.
#
# Lee las variables del bucle en curso (key, engine, ctx, gguf…) del ámbito
# dinámico de bash: por eso no lleva todos los datos como parámetros. Es lo
# que evita tener el mismo bloque de veinte asignaciones en los dos sitios
# donde hay alias (el simple y el de nivel) — que es exactamente como se
# desincroniza un alias del modelo al que apunta.
# -----------------------------------------------------------------------------
_ropero_register_alias() {
    local a_key="$1" a_display="$2" a_level="${3:-}"
    MODEL_NAMES+=("$a_key")
    MODEL_ALIAS_TARGET["$a_key"]="$key"
    MODEL_ALIAS["$a_key"]="$a_display"
    MODEL_ENGINE["$a_key"]="$engine"
    MODEL_CTX["$a_key"]="$ctx"
    MODEL_PORT["$a_key"]="$port"
    MODEL_FORMAT["$a_key"]="$fmt"
    MODEL_PATH["$a_key"]="${MODEL_PATH[$key]}"
    MODEL_GGUF["$a_key"]="$gguf"
    MODEL_MTP["$a_key"]="$mtp"
    MODEL_MMPROJ["$a_key"]="$mmproj"
    MODEL_LEVELS["$a_key"]="$levels"
    MODEL_LEVEL_DEFAULT["$a_key"]="$level_default"
    MODEL_FLAGS["$a_key"]="$flags"
    MODEL_OPTIONS["$a_key"]="$options"
    MODEL_REPO["$a_key"]="$repo"
    MODEL_FILE["$a_key"]="$file"
    MODEL_DEST["$a_key"]="$dest"
    MODEL_SIZE["$a_key"]="$size"
    MODEL_AUX_MTP["$a_key"]="$aux_mtp"
    MODEL_AUX_MMPROJ["$a_key"]="$aux_mmproj"
    MODEL_AUX_CHAT_TEMPLATE["$a_key"]="${MODEL_AUX_CHAT_TEMPLATE[$key]:-}"
    MODEL_MANUAL["$a_key"]="$manual"
    MODEL_MIN_RAM_GB["$a_key"]="${MODEL_MIN_RAM_GB[$key]:-}"
    MODEL_RAM_FRACTION["$a_key"]="${MODEL_RAM_FRACTION[$key]:-}"
    MODEL_KV_KIB_PER_TOKEN["$a_key"]="${MODEL_KV_KIB_PER_TOKEN[$key]:-}"
    if [[ -n "$a_level" ]]; then
        MODEL_ALIAS_LEVEL["$a_key"]="$a_level"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# ropero_models_load — lee el dump y puebla los arrays de modelos.
#
# Lanza el cargador UNA vez por proceso. Es un sub-proceso de python: Acceptable
# porque `ropero list`/`status`/`config` lo llaman una vez, y el arranque de un
# modelo ya es de por sí una operación lenta (cargar GB de pesos).
# -----------------------------------------------------------------------------
ropero_models_load() {
    [[ "${_ROPERO_MODELS_LOADED:-0}" == "1" ]] && return 0

    # ── Las tablas se declaran aquí si el que llama no lo ha hecho ──────────
    #
    # Sin esto, `MODEL_REPO["gemma"]="x"` sobre un array NO declarado crea un
    # array INDEXADO, y todas las claves caen en el índice 0: el modelo
    # "gemma" acabaría con los datos del último modelo leído, y no se vería
    # hasta que una descarga baixase el fichero equivocado. Es un fallo
    # silencioso de los que se descubren un mes después.
    #
    # `declare -p` dice cómo está declarada cada variable, así que se puede
    # corregir sin tocar lo que ya ha hecho el entrypoint.
    local -a _mc_tables=(
        MODEL_ENGINE MODEL_ALIAS MODEL_ALIAS_TARGET MODEL_ALIAS_LEVEL
        MODEL_CTX MODEL_PORT MODEL_FORMAT MODEL_PATH MODEL_GGUF MODEL_MTP
        MODEL_MMPROJ MODEL_LEVELS MODEL_LEVEL_DEFAULT MODEL_FLAGS MODEL_OPTIONS
        MODEL_REPO MODEL_FILE MODEL_DEST MODEL_SIZE MODEL_EXCLUDES MODEL_DL
        MODEL_AUX_MTP MODEL_AUX_MMPROJ MODEL_AUX_CHAT_TEMPLATE MODEL_TOML MODEL_MANUAL
        MODEL_MIN_RAM_GB MODEL_RAM_FRACTION MODEL_KV_KIB_PER_TOKEN
        ROPERO_RETIRED ROPERO_RETIRED_PATH ROPERO_RETIRED_NAME ROPERO_RETIRED_GGUF
    )
    local _t
    for _t in "${_mc_tables[@]}"; do
        if ! declare -p "$_t" 2>/dev/null | grep -q 'declare -A'; then
            declare -gA "$_t" 2>/dev/null || true
        fi
    done
    # MODEL_NAMES es una LISTA, no una tabla.
    if ! declare -p MODEL_NAMES 2>/dev/null | grep -q 'declare -a'; then
        declare -ga MODEL_NAMES 2>/dev/null || true
    fi

    local conf py
    if ! conf=$(ropero_modelconf_path); then
        ropero_err "no encuentro el cargador de modelos (_modelconf.py) junto al entrypoint"
        ropero_info "  esperado en: $SCRIPT_DIR/_modelconf.py"
        return 1
    fi
    if ! py=$(ropero_python_resolve); then
        ropero_err "ropero necesita python3 (>= 3.8) para leer la configuración TOML de los modelos"
        ropero_info "  los modelos están en TOML; no hay forma de cargarlos sin un parser TOML"
        ropero_info "  si tienes varios, fija el que quieras con ROPERO_PYTHON=/ruta/a/python3"
        return 1
    fi

    # Se piden TAMBIÉN los retirados: no para usarlos, sino para saber que
    # existen. Un modelo que se retiró tiene que responder "ya no está
    # disponible", no "modelo desconocido" — que es una mentira útil.
    #
    # stderr va a un fichero aparte, NO mezclado con stdout: el dump se lee
    # línea a línea y un "ERROR: …" intercalado se leería como un registro de
    # modelo (y el registro que viene después, desplazado, se carga en la
    # variable equivocada).
    local dump rc=0 errf
    errf="$(mktemp "${TMPDIR:-/tmp}/ropero_mcdump.XXXXXX")"
    dump=$("$py" "$conf" dump --root "$MODELS_DIR" --include-fired 2>"$errf") || rc=$?
    if [[ -s "$errf" ]]; then
        cat "$errf" >&2
    fi
    rm -f "$errf" 2>/dev/null || true
    if (( rc != 0 )); then
        ropero_err "no se pudo cargar la configuración de modelos:"
        return 1
    fi

    local -a fields=()
    local name engine alias origin engine_dir ctx port fmt path gguf mtp mmproj
    local repo file dest size excludes levels level_default flags options aliases
    local aux_mtp aux_mmproj aux_chat_template args_b64 opts_b64 limits_b64 manual toml_path
    local key a_key a_name a_level pair
    while IFS=$'\x1f' read -r -a fields; do
        [[ "${fields[0]:-}" == "ROPERO_MC_DUMP_VERSION=1" ]] && continue
        (( ${#fields[@]} >= 29 )) || continue
        name="${fields[0]}";          engine="${fields[1]}";      alias="${fields[2]}"
        origin="${fields[3]}";        engine_dir="${fields[4]}";  ctx="${fields[5]}"
        port="${fields[6]}";          fmt="${fields[7]}";         path="${fields[8]}"
        gguf="${fields[9]}";          mtp="${fields[10]}";        mmproj="${fields[11]}"
        repo="${fields[12]}";         file="${fields[13]}";       dest="${fields[14]}"
        size="${fields[15]}";         excludes="${fields[16]}";    levels="${fields[17]}"
        level_default="${fields[18]}"; flags="${fields[19]}";     options="${fields[20]}"
        aliases="${fields[21]}";      aux_mtp="${fields[22]}";    aux_mmproj="${fields[23]}"
        args_b64="${fields[24]}";     opts_b64="${fields[25]}";   limits_b64="${fields[26]}"
        manual="${fields[27]}";       toml_path="${fields[28]}"
        aux_chat_template="${fields[29]:-}"

        # La key de los arrays normaliza los guiones a guiones bajos: es lo que
        # permite que `ropero agents-a1-4b` y `ropero agents_a1_4b` sean el
        # mismo modelo, y lo que hace que los nombres de variable (ctx, port)
        # sean válidos.
        key="${name//-/_}"

        # ── Modelos RETIRADOS ────────────────────────────────────────────
        # Se indexan, pero NO se registran como modelos utilizables.
        if [[ "$origin" == "fired" ]]; then
            ROPERO_RETIRED["$key"]="$engine"
            ROPERO_RETIRED_PATH["$key"]="$toml_path"
            ROPERO_RETIRED_NAME["$key"]="$name"
            ROPERO_RETIRED_GGUF["$key"]="$gguf"
            continue
        fi

        MODEL_NAMES+=("$key")
        MODEL_ENGINE["$key"]="$engine"
        # El nombre "humano" del modelo es el suyo propio. Los alias públicos
        # se registran aparte, con su propia key.
        MODEL_ALIAS["$key"]="$name"
        MODEL_CTX["$key"]="$ctx"
        MODEL_PORT["$key"]="$port"
        MODEL_FORMAT["$key"]="$fmt"
        MODEL_PATH["$key"]="$(ropero_model_abs_path "$fmt" "$path")"
        MODEL_GGUF["$key"]="$gguf"
        MODEL_MTP["$key"]="$mtp"
        MODEL_MMPROJ["$key"]="$mmproj"
        MODEL_LEVELS["$key"]="$levels"
        MODEL_LEVEL_DEFAULT["$key"]="$level_default"
        MODEL_FLAGS["$key"]="$flags"
        MODEL_OPTIONS["$key"]="$options"
        MODEL_REPO["$key"]="$repo"
        MODEL_FILE["$key"]="$file"
        MODEL_DEST["$key"]="$dest"
        MODEL_SIZE["$key"]="$size"
        MODEL_EXCLUDES["$key"]="$excludes"
        MODEL_AUX_MTP["$key"]="$aux_mtp"
        MODEL_AUX_MMPROJ["$key"]="$aux_mmproj"
        MODEL_AUX_CHAT_TEMPLATE["$key"]="$aux_chat_template"
        MODEL_TOML["$key"]="$toml_path"
        # Descarga manual: el modelo existe pero `ropero install models` no lo
        # ofrece (checkpoints de más de 100 GB). Es un dato del TOML.
        MODEL_MANUAL["$key"]="$manual"

        # ── Límites declarados ────────────────────────────────────────────
        # [limits] del TOML: datos, no lógica. El guard los aplica el CLI.
        _ropero_model_limits "$key" "$limits_b64"

        # El hint de descarga: lo que el usuario tiene que hacer para
        # conseguir el modelo. Antes vivía en el .sh del modelo; ahora se
        # compone con los datos que ya están en el TOML.
        if [[ -n "$repo" ]]; then
            MODEL_DL["$key"]="ropero install models $name"
        fi

        # ── Alias públicos ────────────────────────────────────────────────
        # Un alias puede traer su nivel de razonamiento por defecto
        # ("verifier" → high). El alias se registra como una key más que
        # apunta al modelo físico: es lo que permite que `ropero verifier` y
        # `ropero gptoss` sean el mismo modelo sin dos ficheros que se
        # desincronicen.
        if [[ -n "$alias" ]]; then
            a_key="${alias//-/_}"
            # Un alias que es el nombre del propio modelo no aporta nada, y
            # choca con la comprobación de colisión (que avisaría de un
            # conflicto que no existe).
            if [[ "$a_key" == "$key" ]]; then
                :
            elif [[ " ${MODEL_NAMES[*]} " == *" $a_key "* ]]; then
                ropero_warn "$toml_path: el alias '$alias' ya lo usa otro modelo; se omite"
            else
                _ropero_register_alias "$a_key" "$alias"
            fi
        fi

        # Alias con nivel: "verifier=high,designer=medium"
        if [[ -n "$aliases" ]]; then
            while IFS= read -r pair; do
                [[ -n "$pair" ]] || continue
                a_name="${pair%%=*}"
                a_level="${pair#*=}"
                a_key="${a_name//-/_}"
                [[ "$a_key" == "$key" ]] && continue
                if [[ " ${MODEL_NAMES[*]} " == *" $a_key "* ]]; then
                    ropero_warn "$toml_path: el alias '$a_name' ya lo usa otro modelo; se omite"
                    continue
                fi
                _ropero_register_alias "$a_key" "$a_name"
                # Un alias sin nivel ("coder=verifier") no define nivel: usa el
                # default del modelo.
                if [[ -n "$a_level" && "$a_level" != "$a_name" ]]; then
                    MODEL_ALIAS_LEVEL["$a_key"]="$a_level"
                fi
            # OJO: `printf '%s'` SIN newline final. `read` devuelve error al
            # llegar al EOF sin separador, así que el último alias se quedaba
            # fuera del bucle sin avisar. Con '%s\n' la última línea entra.
            done < <(printf '%s\n' "$aliases" | tr ',' '\n')
        fi
    done < <(printf '%s\n' "$dump")

    _ROPERO_MODELS_LOADED=1
    return 0
}

# -----------------------------------------------------------------------------
# ropero_model_args <modelo> <nivel> [args...]
#
# Devuelve el argv final del modelo, UN ARGUMENTO POR LÍNEA (el mismo
# contrato que usaba get_model_args_*): el launcher lo lee con mapfile y lo
# ejecuta sin word-splitting, así que un argumento con espacios (un
# --chat-template-kwargs con JSON) no se pierde.
#
# Los `args` extra son flags ("vision") u opciones con valor ("n-cpu-moe=8").
# Se distinguen por el "=": un flag no lleva valor, una opción sí.
# -----------------------------------------------------------------------------
ropero_model_args() {
    local name="${1:-}" level="${2:-}"
    shift 2 2>/dev/null || true

    local conf py
    conf=$(ropero_modelconf_path) || { ropero_err "falta el cargador de modelos"; return 1; }
    py=$(ropero_python_resolve) || { ropero_err "hace falta python3 (>= 3.8)"; return 1; }

    local -a pflags=() popts=() pyargs=()
    local a
    for a in "$@"; do
        if [[ "$a" == *=* ]]; then popts+=(--option "$a"); else pflags+=(--flag "$a"); fi
    done
    [[ -n "$level" ]] && pyargs+=(--level "$level")

    "$py" "$conf" args --root "$MODELS_DIR" --name "$name" \
        "${pyargs[@]}" "${pflags[@]}" "${popts[@]}"
}

# -----------------------------------------------------------------------------
# ropero_model_ctx <modelo> → contexto efectivo.
#
# El override por entorno se llama ROPERO_<NOMBRE>_CTX, con el nombre del
# MODELO FÍSICO en mayúsculas y guiones convertidos en guiones bajos. Se
# normalizan los guiones: "qwen38-apex" → ROPERO_QWEN38_APEX_CTX. Antes el
# nombre se usaba tal cual, así que un modelo con guionNO tenía forma de
# sobreescribir su contexto.
# -----------------------------------------------------------------------------
ropero_model_ctx() {
    # OJO: `local key=... ctx="${MODEL_CTX[$key]}"` en UNA sola línea está
    # ROTO. bash expande TODAS las palabras de un `local` antes de hacer
    # ninguna asignación, así que $key todavía no existe cuando se expande el
    # subíndice: "bad array subscript" y ctx sale vacía. Son dos sentencias.
    local key="${1:-}"
    local ctx="${MODEL_CTX[$key]:-}"
    local env_name
    key="${key//-/_}"
    env_name="ROPERO_$(printf '%s' "$key" | tr '.-' '__' | tr '[:lower:]' '[:upper:]')_CTX"
    printf '%s' "${!env_name:-$ctx}"
}

# -----------------------------------------------------------------------------
# ropero_model_retired <nombre> → 0 si ese modelo está retirado ("fired").
#
# Lo que se llama desde el launcher cuando el modelo no está entre los
# disponibles: no es lo mismo "no existe" que "existe pero está retirado". El
# segundo caso tiene un mensaje con nombre y motor, que es lo que el usuario
# necesita para saber qué hacer.
# -----------------------------------------------------------------------------
ropero_model_retired() {
    local want="${1:-}"
    [[ -n "$want" ]] || return 1
    local k="${want//-/_}" r
    [[ -n "${ROPERO_RETIRED[$k]:-}" ]] && return 0
    # También por el nombre "humano" del TOML (p.ej. "tensorrt-llm").
    for r in "${!ROPERO_RETIRED[@]}"; do
        [[ "${ROPERO_RETIRED_NAME[$r]:-}" == "$want" ]] && return 0
    done
    return 1
}

# -----------------------------------------------------------------------------
# _ropero_model_dir <modelo> → subdirectorio bajo $GGUF_DIR donde viven los
# ficheros de ese modelo (el nombre del modelo, normalizado).
# -----------------------------------------------------------------------------
_ropero_model_dir() {
    local key="${1:-}"
    key="${key//-/_}"
    local target="${MODEL_ALIAS_TARGET[$key]:-}"
    [[ -n "$target" ]] && key="$target"
    printf '%s' "${MODEL_ALIAS[$key]:-$key}"
}

# -----------------------------------------------------------------------------
# ropero_model_check_limits <modelo> <ctx>
#
# Aplica los límites declarados en [limits] del TOML. Devuelve 1 si el modelo
# no debe arrancar.
#
# ── POR QUÉ ESTO EXISTE ──────────────────────────────────────────────────────
#
# En macOS un modelo que no cabe en la RAM unificada no avisa: carga lo que
# puede y empieza a paginar contra el disco. El resultado son ~0.2 tok/s, que
# parece un modelo roto pero es falta de RAM — y, peor, deja el resto de la
# máquina inutilizable. El aviso tiene que salir ANTES de arrancar.
#
# Antes esto era un `if` de bash dentro del script del modelo, con la
# aritmética en enteros de MB (a propósito: con LANG=es_ES.UTF-8 el locale de
# awk rompe las comparaciones de flotantes). Ahora el límite es DATO en el
# TOML y el cálculo vive aquí, una sola vez, igual para todos los modelos.
#
# Sólo se aplica en Darwin: es un problema de la memoria unificada de Apple.
# En Linux con swap la paginación es lenta pero noHi deja la máquina muerta.
# -----------------------------------------------------------------------------
ropero_model_check_limits() {
    local model_key="${1:-}" ctx="${2:-}"
    local min_ram="${MODEL_MIN_RAM_GB[$model_key]:-}"
    local frac="${MODEL_RAM_FRACTION[$model_key]:-}"
    local kv_kib="${MODEL_KV_KIB_PER_TOKEN[$model_key]:-}"
    [[ -n "$min_ram$frac" ]] || return 0
    [[ "$(uname -s)" == "Darwin" ]] || return 0

    # Escape hatch. Antes era por modelo (ROPERO_METALOSS_SKIP_RAMCHECK=1); ahora
    # hay uno general, y el antiguo sigue valiendo para no romper scripts.
    local skip="${ROPERO_SKIP_RAMCHECK:-${ROPERO_METALOSS_SKIP_RAMCHECK:-0}}"
    [[ "$skip" == "1" ]] && return 0

    local ram_mb weights_mb kv_mb need_mb limit_mb dir
    ram_mb=$(sysctl -n hw.memsize 2>/dev/null) || ram_mb=0
    ram_mb=$(( ram_mb / 1048576 ))          # bytes → MiB
    (( ram_mb > 0 )) || return 0

    # ── Peso del modelo ───────────────────────────────────────────────────
    weights_mb=0
    if [[ "${MODEL_FORMAT[$model_key]:-}" == "safetensors" ]]; then
        dir="${MODEL_PATH[$model_key]}"
        if [[ -d "$dir" ]]; then
            weights_mb=$(du -sm "$dir" 2>/dev/null | awk '{print $1; exit}')
            weights_mb="${weights_mb:-0}"
        fi
    else
        local _mdir; _mdir="$(_ropero_model_dir "$model_key")"
        local gguf_path="$GGUF_DIR/$_mdir/${MODEL_GGUF[$model_key]:-}"
        if [[ -f "$gguf_path" ]]; then
            # stat da bytes; a MiB con la misma aritmética entera.
            local bytes
            bytes=$(stat -c %s "$gguf_path" 2>/dev/null || stat -f %z "$gguf_path" 2>/dev/null || echo 0)
            weights_mb=$(( bytes / 1048576 ))
        fi
    fi

    # ── RAM mínima absoluta ───────────────────────────────────────────────
    if [[ -n "$min_ram" ]]; then
        local min_mb=$(( ${min_ram%%.*} * 1024 ))
        if (( ram_mb < min_mb )); then
            {
                echo "ERROR: '$model_key' necesita al menos ${min_ram} GB de RAM unificada." >&2
                printf '  esta máquina tiene: %d.%d GB\n' \
                    $((ram_mb/1024)) $(( (ram_mb%1024)*10/1024 )) >&2
                echo "  Solución: un modelo más pequeño, o más RAM." >&2
            } >&2
            return 1
        fi
    fi

    # ── ¿Cabe pesos + KV cache? ──────────────────────────────────────────
    [[ -n "$frac" ]] || return 0
    local kv_mb=0
    if [[ -n "$kv_kib" ]]; then
        kv_mb=$(( kv_kib * ctx / 1024 ))
    fi
    need_mb=$(( weights_mb + kv_mb ))
    # La fracción viene como decimal ("0.8"). Se pasa a milésimas para poder
    # multiplicar con aritmética entera de bash.
    #
    # LC_ALL=C en el awk: el problema del guard original no era awk en sí, sino
    # el locale (con LANG=es_ES.UTF-8 awk formatea con coma decimal y su
    # separador de campos no coincide con el de bash, así que la comparación
    # daba "no cabe" con 3 GB usados de 35 GB disponibles). Con LC_ALL=C, la
    # coma decimal deja de existir y awk se comporta.
    local frac_milli
    frac_milli=$(LC_ALL=C awk -v f="${frac:-0.8}" 'BEGIN{printf "%d", f*1000}' 2>/dev/null || echo 800)
    limit_mb=$(( ram_mb * frac_milli / 1000 ))

    if (( need_mb > limit_mb )); then
        {
            echo "ERROR: '$model_key' no cabe en la memoria unificada de esta máquina." >&2
            printf '  pesos   : %d.%d GB\n' \
                $((weights_mb/1024)) $(( (weights_mb%1024)*10/1024 )) >&2
            printf '  KV cache: %d.%d GB  (ctx=%s, %s KiB/token)\n' \
                $((kv_mb/1024)) $(( (kv_mb%1024)*10/1024 )) "$ctx" "${kv_kib:-?}" >&2
            printf '  TOTAL   : %d MiB  >  %d%% de %d MiB\n\n' "$need_mb" "$(( frac_milli / 10 ))" "$ram_mb" >&2
            echo "  macOS no avisa: paginaría y tendrías ~0.2 tok/s." >&2
            echo "  Soluciones:" >&2
            echo "    - bajar el contexto:  ROPERO_<MODELO>_CTX=16384 ropero $model_key" >&2
            echo "    - saltar el guard (sólo si sabes lo que haces):" >&2
            echo "        ROPERO_SKIP_RAMCHECK=1 ropero $model_key" >&2
        } >&2
        return 1
    fi

    printf '  %s: %d MiB de pesos + %d MiB de KV (ctx=%s) de %d MiB unificados\n' \
        "$model_key" "$weights_mb" "$kv_mb" "$ctx" "$ram_mb" >&2
    return 0
}
