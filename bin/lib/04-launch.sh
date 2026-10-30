#!/usr/bin/env bash
# =============================================================================
# 04-launch.sh — runtime de ropero 1.0: arranque, espera, estado y parada
# (spec §5.5, fase 4)
# =============================================================================
# CONTRATO (spec §5.5):
#   rp_start_model <modelo> <bg> <cpu> <force> <port> <nivel> <flags> <opts>
#   rp_wait_ready  <puerto> <engine> [timeout] [esperado]
#   rp_stop_port   <puerto>          — mata POR PUERTO, nunca pkill -f
#   rp_managed_ports                 — puertos de §4, uno por línea
#   rp_model_on_port <puerto>        — model-id vía /v1/models; fallback estado
#   rp_port_listening <puerto>
#   rp_api_key                       — env > $STATE_DIR/api_key > genera
#   rp_state_clear <puerto>          — borra el estado del puerto
#
# ESTADO RUNTIME: $STATE_DIR/ports/<puerto>, líneas KEY=valor con
#   MODEL= ENGINE= PID= PORT= LEVEL= TS= LOG= STAMP=
# Es lo que hace que `status`, `stop` y la idempotencia de `use` funcionen
# desde OTRO proceso de ropero: lo que escucha en el puerto no dice a quién
# lanzó (y a veces ni siquiera publica un id mapeable al catálogo).
#
# POR QUÉ EL ESTADO Y NO SÓLO /v1/models
#   El id que publica un server no siempre casa con el catálogo (el fake de
#   la suite, un llama-server sin --served-model-name...). Ropero SABE a qué
#   modelo lanzó en cada puerto: eso es el fichero de estado. La jerarquía
#   de rp_model_on_port es: id mapeado al catálogo > estado (PID vivo) > id
#   crudo. Y rp_wait_ready acepta el estado como prueba de readiness SÓLO
#   si el probe del engine responde y el PID lanzado sigue vivo.
#
# bash 3.2: sin namerefs, sin arrays asociativos, sin mapfile (spec §3).
# Seguro de sourcear: sólo definiciones, cero ejecución.
# =============================================================================

if ! declare -F rp_engine_call >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/03-engines.sh"
fi

# El color de la salida en vivo del engine (06-styles.sh).
if ! declare -F rp_colorize >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/06-styles.sh"
fi

# Separador del dump de modelconf (0x1F), como constante bash para awk -F.
RP_LAUNCH_SEP=$'\x1f'

# ── Puertos y procesos ───────────────────────────────────────────────────────

# rp_port_listening <puerto> — 0 si algo escucha en el puerto (spec §5.5).
rp_port_listening() {
    lsof -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1
}

# rp_pid_for_port <puerto> — primer PID que escucha (vacío si nadie).
rp_pid_for_port() {
    lsof -t -iTCP:"$1" -sTCP:LISTEN 2>/dev/null | head -1
}

# _rp_launch_pids_for_port <puerto> — TODOS los PIDs que escuchan, uno por línea.
_rp_launch_pids_for_port() {
    lsof -t -iTCP:"$1" -sTCP:LISTEN 2>/dev/null
}

# _rp_launch_valid_port <valor> — 0 si es un puerto TCP usable.
_rp_launch_valid_port() {
    [[ "$1" =~ ^[0-9]+$ ]] && (( $1 >= 1 && $1 <= 65535 ))
}

# _rp_launch_puerto_destino <real> <override> <cpu> — el puerto donde servirá
# el modelo: --port > pineado en el TOML ([model].port) > slot CPU (--cpu,
# sólo Linux) > slot base $PORT (§5.5). Imprime el puerto; rc 1 si no resuelve.
_rp_launch_puerto_destino() {
    local real="$1" override="$2" cpu="$3"
    if [[ -n "$override" ]]; then
        printf '%s' "$override"
        return 0
    fi
    local idx
    idx="$(_rp_mc_idx "$real" 2>/dev/null)" || return 1
    local pineado="${MODEL_PORT[$idx]}"
    if [[ "$pineado" =~ ^[0-9]+$ ]]; then
        printf '%s' "$pineado"
        return 0
    fi
    if [[ "$cpu" == "1" ]]; then
        printf '%s' "$CPU_PORT"
    else
        printf '%s' "$PORT"
    fi
    return 0
}

# ── API key (spec §5.5) ──────────────────────────────────────────────────────

# rp_api_key — deja LLAMA_API_KEY resuelta y exportada (LA MISMA para todos los
# engines). Orden: env fijada > $STATE_DIR/api_key > genera sk-ropero-<hex> con
# openssl (fallback /dev/urandom+od), chmod 600, y la guarda para la próxima.
rp_api_key() {
    if [[ -n "${LLAMA_API_KEY:-}" ]]; then
        return 0
    fi
    local f="$STATE_DIR/api_key"
    if [[ -s "$f" ]]; then
        LLAMA_API_KEY="$(head -1 "$f" 2>/dev/null)"
        export LLAMA_API_KEY
        [[ -n "${LLAMA_API_KEY:-}" ]] && return 0
    fi
    mkdir -p "$STATE_DIR" 2>/dev/null || return 1
    local hex=""
    if command -v openssl >/dev/null 2>&1; then
        hex="$(openssl rand -hex 16 2>/dev/null)" || hex=""
    fi
    if [[ -z "$hex" ]]; then
        # od -An -N16 -tx1 da 32 dígitos hex: mismo ancho que openssl -hex 16.
        hex="$(od -An -N16 -tx1 /dev/urandom 2>/dev/null | tr -d ' \n')"
    fi
    [[ -n "$hex" ]] || return 1
    printf '%s\n' "sk-ropero-$hex" >"$f" 2>/dev/null || return 1
    chmod 600 "$f" 2>/dev/null || true
    LLAMA_API_KEY="sk-ropero-$hex"
    export LLAMA_API_KEY
    return 0
}

# ── Estado por puerto ────────────────────────────────────────────────────────

# _rp_state_path <puerto> — fichero de estado del puerto.
_rp_state_path() {
    printf '%s/ports/%s' "$STATE_DIR" "$1"
}

# rp_state_read <puerto> <campo> — valor del campo (MODEL/ENGINE/PID/PORT/
# LEVEL/TS/LOG/STAMP) por stdout. rc 1 si no hay estado o el campo está vacío.
rp_state_read() {
    local f
    f="$(_rp_state_path "$1")"
    [[ -f "$f" ]] || return 1
    local v
    v="$(grep "^$2=" "$f" 2>/dev/null | head -1 | cut -d= -f2-)"
    [[ -n "$v" ]] || return 1
    printf '%s' "$v"
}

# rp_state_write <puerto> CLAVE=valor... — escribe/actualiza campos CONSERVANDO
# los que ya estaban y no vienen en la llamada. Escritura atómica (tmp+mv):
# un status concurrente no lee un fichero a medias.
rp_state_write() {
    local puerto="$1"
    shift
    local f tmp clave a conserva
    f="$(_rp_state_path "$puerto")"
    mkdir -p "$(dirname "$f")" 2>/dev/null || return 1
    tmp="$f.tmp.$$"
    : >"$tmp" || return 1
    if [[ -f "$f" ]]; then
        while IFS= read -r conserva; do
            [[ "$conserva" == *=* ]] || continue
            clave="${conserva%%=*}"
            local reescrita=0
            for a in "$@"; do
                [[ "$a" == "$clave="* ]] && reescrita=1
            done
            [[ "$reescrita" == "1" ]] && continue
            printf '%s\n' "$conserva" >>"$tmp"
        done <"$f"
    fi
    for a in "$@"; do
        printf '%s\n' "$a" >>"$tmp"
    done
    mv "$tmp" "$f" 2>/dev/null || { rm -f "$tmp" 2>/dev/null; return 1; }
    return 0
}

# rp_state_clear <puerto> — borra el fichero de estado al parar (spec §5.5).
rp_state_clear() {
    rm -f "$(_rp_state_path "$1")" 2>/dev/null
    return 0
}

# rp_managed_ports — TODOS los puertos que ropero conoce, ordenados y sin
# duplicados (spec §4): $PORT + $CPU_PORT (sólo Linux) + los [model].port
# pineados de los TOML ACTIVOS + los puertos con fichero en $STATE_DIR/ports
# (los ad-hoc de --port y los de arranques en curso). Es la lista canónica de
# `status`, `stop all` y la idempotencia de `use`.
rp_managed_ports() {
    local -a puertos=("$PORT")
    if [[ "$(uname -s)" == "Linux" ]]; then
        puertos+=("$CPU_PORT")
    fi
    if declare -F rp_mc_load >/dev/null 2>&1 && rp_mc_load 2>/dev/null; then
        local i p
        for (( i = 0; i < ${#MODEL_NAMES[@]}; i++ )); do
            # Las filas de alias copian el puerto del modelo real: saltarlas
            # evita duplicados; los fired no se arrancan, no gestionan slot.
            [[ "${MODEL_ISALIAS[$i]}" == "0" ]] || continue
            [[ "${MODEL_ORIGIN[$i]}" == "active" ]] || continue
            p="${MODEL_PORT[$i]}"
            [[ "$p" =~ ^[0-9]+$ ]] && puertos+=("$p")
        done
    fi
    local f extra
    if [[ -d "$STATE_DIR/ports" ]]; then
        for f in "$STATE_DIR/ports"/*; do
            [[ -f "$f" ]] || continue
            extra="${f##*/}"
            [[ "$extra" =~ ^[0-9]+$ ]] && puertos+=("$extra")
        done
    fi
    printf '%s\n' ${puertos[@]+"${puertos[@]}"} | sort -n -u
}

# ── Qué hay sirviéndose en un puerto ────────────────────────────────────────

# _rp_http_models_ids <puerto> — los ids de GET /v1/models, uno por línea.
# Con Bearer: llama-server (y los endpoint) exigen la key hasta para listar.
# El grep tolera el espacio alrededor de los dos puntos: no todos los servers
# serializan JSON compacto (port de bin/ropero:245-261 del legado).
_rp_http_models_ids() {
    local puerto="$1"
    rp_api_key 2>/dev/null || true
    curl -fsS --max-time 3 \
        -H "Authorization: Bearer ${LLAMA_API_KEY:-}" \
        "http://${HOST:-127.0.0.1}:$puerto/v1/models" 2>/dev/null \
        | grep -oE '"id"[[:space:]]*:[[:space:]]*"[^"]*"' \
        | sed -E 's/.*"([^"]*)"$/\1/'
    return 0
}

# _rp_launch_id_casa <id> <real> — 0 si el id que publica un server puede ser
# el modelo <real>: por nombre, por el GGUF (con y sin extensión: llama-server
# publica el basename del fichero) o por la ruta del checkpoint (basename o
# ruta completa: vllm y mlx-serve publican según el engine).
_rp_launch_id_casa() {
    local id="$1" real="$2"
    [[ "$id" == "$real" ]] && return 0
    local gguf="" ruta=""
    gguf="$(rp_model_field "$real" gguf 2>/dev/null)" || gguf=""
    ruta="$(rp_model_field "$real" path 2>/dev/null)" || ruta=""
    if [[ -n "$gguf" ]]; then
        [[ "$id" == "$gguf" ]] && return 0
        [[ "$id" == "${gguf%.gguf}" ]] && return 0
    fi
    if [[ -n "$ruta" ]]; then
        [[ "$id" == "$ruta" ]] && return 0
        [[ "$id" == "${ruta##*/}" ]] && return 0
    fi
    return 1
}

# _rp_launch_model_from_ids <ids> — nombre ropero del primer id que mapee al
# catálogo (nombre, alias, gguf o path). Vacío si ninguno mapea.
_rp_launch_model_from_ids() {
    rp_mc_load 2>/dev/null || return 1
    local id i
    while IFS= read -r id; do
        [[ -n "$id" ]] || continue
        for (( i = 0; i < ${#MODEL_NAMES[@]}; i++ )); do
            if _rp_launch_id_casa "$id" "${MODEL_NAMES[$i]}"; then
                printf '%s' "${MODEL_REAL[$i]}"
                return 0
            fi
        done
    done <<<"$1"
    return 1
}

# rp_model_on_port <puerto> — QUÉ modelo se sirve en el puerto (spec §5.5):
#   1. id de /v1/models mapeado al catálogo (nombre/alias/gguf/path)
#   2. fichero de estado: ropero sabe a quién lanzó ahí (PID vivo)
#   3. id crudo tal cual lo publica el server
# rc 1 con salida vacía si el puerto no escucha (no hay nada que preguntar).
rp_model_on_port() {
    local puerto="$1"
    rp_port_listening "$puerto" || return 1
    local ids="" nombre="" pid="" modelo=""
    ids="$(_rp_http_models_ids "$puerto")"
    if [[ -n "$ids" ]]; then
        nombre="$(_rp_launch_model_from_ids "$ids")"
        if [[ -n "$nombre" ]]; then
            printf '%s' "$nombre"
            return 0
        fi
        pid="$(rp_state_read "$puerto" PID)" || pid=""
        modelo="$(rp_state_read "$puerto" MODEL)" || modelo=""
        if [[ -n "$modelo" && -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            printf '%s' "$modelo"
            return 0
        fi
        printf '%s' "$(printf '%s\n' "$ids" | head -1)"
        return 0
    fi
    # Puerto arriba pero /v1/models no responde (sin key, aún cargando, ...):
    # el estado con PID vivo es lo único fiable; si no, "?"
    pid="$(rp_state_read "$puerto" PID)" || pid=""
    modelo="$(rp_state_read "$puerto" MODEL)" || modelo=""
    if [[ -n "$modelo" && -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
        printf '%s' "$modelo"
        return 0
    fi
    printf '?'
    return 0
}

# ── Parada por puerto ────────────────────────────────────────────────────────

# rp_stop_port <puerto> — apaga lo que vive en <puerto> (árbol del PID del
# estado + el que escucha + sus hijos directos) y limpia el estado.
#
# POR PUERTO, NUNCA pkill -f GLOBAL (spec §5.5): un pkill por nombre mataría
# los slots paralelos. SIGTERM primero (deja que el proceso apague a sus
# hijos y suelte puertos internos), ≤10s de margen, SIGKILL de red de
# seguridad. rc 1 si el puerto ya estaba libre (no había nada que parar).
rp_stop_port() {
    local puerto="$1"
    if ! rp_port_listening "$puerto"; then
        rp_state_clear "$puerto"
        return 1
    fi

    # Recoger el árbol: PID del estado, PIDs que escuchan, hijos directos.
    # Dedup por string: el PID del estado suele ser también el que escucha.
    local -a objetivos=()
    local p hijo visto=""
    p="$(rp_state_read "$puerto" PID)" || p=""
    if [[ -n "$p" ]] && kill -0 "$p" 2>/dev/null; then
        objetivos+=("$p"); visto=" $p "
    fi
    while IFS= read -r p; do
        [[ -n "$p" ]] || continue
        [[ "$visto" == *" $p "* ]] && continue
        objetivos+=("$p"); visto="$visto$p "
    done < <(_rp_launch_pids_for_port "$puerto")
    local -a arbol=("${objetivos[@]+"${objetivos[@]}"}")
    for p in ${objetivos[@]+"${objetivos[@]}"}; do
        while IFS= read -r hijo; do
            [[ -n "$hijo" ]] || continue
            [[ "$visto" == *" $hijo "* ]] && continue
            arbol+=("$hijo"); visto="$visto$hijo "
        done < <(pgrep -P "$p" 2>/dev/null)
    done

    # SIGTERM a todo el árbol.
    for p in ${arbol[@]+"${arbol[@]}"}; do
        kill -TERM "$p" 2>/dev/null || true
    done

    # Margen de cortesía: hasta 10s esperando a que el puerto se libere.
    local esperado=0
    while (( esperado < 10 )) && rp_port_listening "$puerto"; do
        sleep 1
        esperado=$((esperado + 1))
    done

    # Red de seguridad: lo que ignore el TERM se lleva por delante el KILL.
    if rp_port_listening "$puerto"; then
        for p in ${arbol[@]+"${arbol[@]}"}; do
            kill -9 "$p" 2>/dev/null || true
        done
        sleep 1
    fi
    rp_state_clear "$puerto"
    return 0
}

# ── Lectura de campos del dump que 02-modelconf no expone ───────────────────
#
# El dump v1 tiene 31 campos, pero las arrays de 02-modelconf sólo copian los
# que usa el catálogo. El runtime necesita dos más: [aliases] (22) para
# resolver el nivel pedido y [limits] (27, b64 JSON) para el check de RAM.
# Se releen AQUÍ con awk en vez de tocar 02-modelconf.sh (fase 2, propietaria
# de otra fase): una llamada puntual, no un hot path.
# Posiciones (1-based, build_record de modelconf.py): 13 repo · 14 file ·
# 15 dest · 17 excludes · 22 aliases · 27 limits · 28 manual · 31 build.

# _rp_launch_dump_field <nombre-real> <posición> — valor del campo por stdout.
# rc 1 si el modelo no está en el dump.
_rp_launch_dump_field() {
    local nombre="$1" pos="$2"
    local py
    py="$(_rp_mc_python)" || return 1
    "$py" "$RP_MODELCONF_PY" dump --root "$RP_MODELS_DIR" --include-fired 2>/dev/null \
        | awk -v fs="$RP_LAUNCH_SEP" -v objetivo="$nombre" -v pos="$pos" '
            BEGIN { FS = fs }
            $1 == objetivo { print $pos; encontrado = 1; exit }
            END { exit (encontrado ? 0 : 1) }'
}

# _rp_launch_alias_nivel <real> <alias> — nivel canónico de un alias de nivel
# ([aliases] x=high). Vacío si <alias> no es un alias declarado.
_rp_launch_alias_nivel() {
    local real="$1" alias="$2" pares par clave valor
    pares="$(_rp_launch_dump_field "$real" 22)" || return 1
    [[ -n "$pares" ]] || return 1
    # Pares "k=v,k2=v2" separados por coma: sin IFS= (se quiere el split).
    for par in ${pares//,/ }; do
        clave="${par%%=*}"
        valor="${par#*=}"
        if [[ "$clave" == "$alias" ]]; then
            printf '%s' "$valor"
            return 0
        fi
    done
    return 1
}

# _rp_launch_total_ram_gb — RAM física total en GB (vacío si no se sabe).
# Es TOTAL, no libre: [limits].min_ram_gb pregunta "¿cabe el modelo en esta
# máquina", no "¿hay hueco ahora mismo" (macOS pagina en vez de fallar).
_rp_launch_total_ram_gb() {
    local bytes=""
    case "$(uname -s)" in
        Darwin) bytes="$(sysctl -n hw.memsize 2>/dev/null)" || bytes="" ;;
        Linux)
            if [[ -r /proc/meminfo ]]; then
                bytes="$(awk '/^MemTotal:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"
                [[ "$bytes" =~ ^[0-9]+$ ]] && bytes=$((bytes * 1024))
            fi
            ;;
    esac
    [[ "$bytes" =~ ^[0-9]+$ ]] || return 1
    awk -v b="$bytes" 'BEGIN { printf "%.0f", b / 1073741824 }'
}

# _rp_launch_check_limits <real> — [limits].min_ram_gb como AVISO, no como
# bloqueo: el dato del TOML es orientativo ("~48 GB") y la RAM libre varía
# por debajo de ropero. Si no cabe, el usuario debe poder intentarlo igual:
# se avisa y adelante (decisión de la fase: check orientativo).
_rp_launch_check_limits() {
    local real="$1"
    local b64 min_ram total
    b64="$(_rp_launch_dump_field "$real" 27)" || return 0
    [[ -n "$b64" ]] || return 0
    min_ram="$(printf '%s' "$b64" | base64 --decode 2>/dev/null \
        | grep -oE '"min_ram_gb"[[:space:]]*:[[:space:]]*[0-9]+([.][0-9]+)?' \
        | grep -oE '[0-9]+([.][0-9]+)?$')"
    [[ -n "$min_ram" ]] || return 0
    total="$(_rp_launch_total_ram_gb)"
    [[ -n "$total" ]] || return 0
    if awk -v t="$total" -v m="$min_ram" 'BEGIN { exit (t < m) ? 0 : 1 }'; then
        rp_msg warn "'$real' pide ~${min_ram} GB de RAM y esta máquina tiene ${total} GB (orientativo: macOS pagina)"
    fi
    return 0
}

# ── Espera de readiness ──────────────────────────────────────────────────────

# _rp_wait_fail <puerto> <modelo> <motivo> — error de espera + últimas líneas
# del log del arranque (es donde está la causa, no en el mensaje).
_rp_wait_fail() {
    local puerto="$1" modelo="$2" motivo="$3"
    rp_msg error "$motivo"
    local log=""
    log="$(rp_state_read "$puerto" LOG)" || log=""
    if [[ -n "$log" && -f "$log" ]]; then
        rp_msg info "  últimas líneas de $log:"
        tail -n 15 "$log" 2>/dev/null | sed 's/^/    /' >&2
    fi
    return 0
}

# rp_wait_ready <puerto> <engine> [timeout] [esperado]
#
# Espera al MODELO, no sólo al puerto (spec §5.5): hace poll del PROBE del
# engine y, cuando responde, exige que /v1/models publique el modelo pedido.
# Éxito cuando:
#   · algún id casa con <esperado> (nombre, gguf o path: ver _rp_launch_id_casa)
#   · O el fichero de estado del puerto dice MODEL=<esperado> con PID vivo:
#     el id publicado no mapea al catálogo (servers que no publican el nombre
#     de ropero), pero ropero SABE a quién lanzó en ese puerto.
# Si el proceso muere antes de escuchar, no se esperan los N segundos: fallo
# inmediato con el log (port de _ropero_run_engine del legado).
# Sin <esperado>: basta probe ok + ids publicados.
rp_wait_ready() {
    local puerto="$1" engine="$2"
    local timeout="${3:-${ROPERO_START_TIMEOUT:-300}}"
    local esperado="${4:-}"
    rp_api_key 2>/dev/null || true

    local transcurrido=0 ids pid modelo probe
    while (( transcurrido < timeout )); do
        pid="$(rp_state_read "$puerto" PID)" || pid=""
        # Proceso muerto y puerto libre: el arranque falló, esperar no arregla nada.
        if [[ -n "$pid" ]] && ! kill -0 "$pid" 2>/dev/null && ! rp_port_listening "$puerto"; then
            _rp_wait_fail "$puerto" "$esperado" "el proceso (pid $pid) murió durante el arranque en :$puerto"
            return 1
        fi
        probe="$(rp_engine_call "$engine" probe "$puerto" 2>/dev/null)"
        if [[ "$probe" == "ok" ]]; then
            ids="$(_rp_http_models_ids "$puerto")"
            if [[ -n "$ids" ]]; then
                if [[ -z "$esperado" ]]; then
                    return 0
                fi
                local id casa=0
                while IFS= read -r id; do
                    _rp_launch_id_casa "$id" "$esperado" && casa=1
                done <<<"$ids"
                if [[ "$casa" == "1" ]]; then
                    return 0
                fi
                modelo="$(rp_state_read "$puerto" MODEL)" || modelo=""
                if [[ "$modelo" == "$esperado" && -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
                    return 0
                fi
            fi
        fi
        sleep 2
        transcurrido=$((transcurrido + 2))
    done
    _rp_wait_fail "$puerto" "$esperado" "'${esperado:-$engine}' no responde tras ${timeout}s en :$puerto"
    return 1
}

# ── Arranque ─────────────────────────────────────────────────────────────────

# _rp_launch_stamp <argv...> — sello de la configuración de arranque: cksum de
# los args del modelo (port del "sello" del legado, con cksum en vez de
# sha256sum porque macOS no trae sha256sum). Sirve para detectar que el TOML
# cambió mientras el modelo ya corre: el server vivo queda con la config VIEJA
# y "ya está corriendo" sería mentir en silencio.
_rp_launch_stamp() {
    if [[ $# -eq 0 ]]; then
        printf ''
        return 0
    fi
    printf '%s\n' "$@" | cksum | awk '{print $1}'
}

# -----------------------------------------------------------------------------
# _rp_launch_check_chat_template <modelo> <argv...> — el fichero que
# --chat-template-file apunta, ¿existe?
#
# POR QUÉ ESTE CHECK
# llama-server, cuando el --chat-template-file no existe, NO dice "no existe
# ese fichero": aborta con el muro de las plantillas built-in (sesenta líneas)
# y un "(env: LLAMA_ARG_CHAT_TEMPLATE_FILE)". El error real queda enterrado
# ARRIBA del todo y lo que se lee (el final del log) no menciona el template.
# Se corta aquí, con una línea que sí dice qué pasa y dónde.
#
# Es un check de LECTURA: el fichero tiene que estar en disco antes de
# arrancar el engine. Si el path viene sin expandir (un "{gguf_dir}" que nadie
# expandió, o una ruta absoluta de otra máquina), esto lo delata.
# -----------------------------------------------------------------------------
_rp_launch_check_chat_template() {
    local real="${1:-}"
    shift || true
    local -a args=("$@")
    local i=0 n="${#args[@]}" v ruta
    while (( i < n )); do
        v="${args[$i]}"
        case "$v" in
            --chat-template-file=*)
                ruta="${v#*=}"
                ;;
            --chat-template-file)
                if (( i + 1 >= n )); then
                    rp_msg error "--chat-template-file sin valor (modelo '$real')"
                    return 1
                fi
                ruta="${args[$((i + 1))]}"
                ;;
            *) i=$((i + 1)); continue ;;
        esac
        if [[ -n "$ruta" && ! -f "$ruta" ]]; then
            rp_msg error "el chat template no existe: $ruta (modelo '$real')"
            rp_msg info "  (ropero install models $real lo descarga; si ya lo tienes, mira la ruta)"
            return 1
        fi
        return 0
    done
    return 0
}

# -----------------------------------------------------------------------------
# _rp_launch_kill_follow <pid...> — mata el tail y el colorizador del log en
# vivo. Con Ctrl-C también hay que cerrar esa boca: si no, el tail se queda
# bloqueado en inotify para siempre, sin dueño (medido en el monolito 1.0).
# -----------------------------------------------------------------------------
_rp_launch_kill_follow() {
    local p
    for p in "$@"; do
        [[ -n "$p" ]] || continue
        kill "$p" 2>/dev/null || true
    done
    return 0
}

# rp_start_model <modelo> <bg> <cpu> <force> <port> <nivel> <flags> <opts>
#
# Porta start_model del legado (bin/ropero:1600+) al esquema 1.0: el argv lo
# monta modelconf (rp_model_argv, placeholders resueltos), el engine añade
# SÓLO binario+host+puerto+auth (§5.4) y AQUÍ no se añade ningún flag del
# modelo a mano (el TOML es el dueño del argv: --model ${gguf} vive en
# [args].base del _template 1.0).
#
# Orden (importante): preflight ANTES de tocar el puerto — no se mata lo que
# corre por un lanzamiento condenado (gguf ausente, argv inválido, engine sin
# instalar); sólo entonces se gestiona el puerto y se arranca.
rp_start_model() {
    local modelo="$1" bg="$2" cpu="$3" force="$4" port="$5" nivel="$6" flags="$7" opts="$8"

    rp_mc_load || return 1

    # Resolución de nombre (alias → real) y estado del modelo.
    local idx
    idx="$(_rp_mc_idx "$modelo")" || {
        rp_msg error "modelo desconocido: '$modelo' (ropero list)"
        return 1
    }
    local real="${MODEL_REAL[$idx]}"
    if [[ "${MODEL_ORIGIN[$idx]}" == "fired" ]]; then
        rp_msg error "'$real' está retirado (models/*/fired/): no se puede arrancar"
        rp_msg info "  config: ${MODEL_TOML[$idx]}"
        return 1
    fi

    # Nivel efectivo: el pedido, o el default del TOML.
    [[ -n "$nivel" ]] || nivel="${MODEL_LEVEL_DEFAULT[$idx]}"

    # ── Preflight: engine instalado ────────────────────────────────────────
    local engine
    engine="$(rp_engine_for_model "$real")" || return 1
    if ! rp_engine_call "$engine" resolve >/dev/null 2>&1; then
        rp_msg error "engine '$engine' no está instalado (ropero install engines $engine)"
        return 1
    fi

    # --cpu sólo tiene sentido en llama-server (es --n-gpu-layers 0, un flag
    # SUYO); en macOS además es error de uso y lo caza cmd-runtime.
    if [[ "$cpu" == "1" && "$engine" != "llama-server" ]]; then
        rp_msg error "--cpu sólo funciona con engine=llama-server ('$real' usa '$engine')"
        return 1
    fi

    # ── Preflight: argv del modelo (modelo + nivel + flags + opciones) ─────
    # $flags/$opts SIN comillas es intencional: son listas separadas por
    # espacios de tokens declarados en el TOML (sin espacios dentro).
    local out rc=0
    out="$(rp_model_argv "$real" ${nivel:+"$nivel"} $flags $opts 2>&1)" || rc=$?
    if (( rc != 0 )); then
        rp_msg error "la configuración de '$real' no es válida:"
        printf '%s\n' "$out" >&2
        return 1
    fi
    local -a argv=()
    local linea
    while IFS= read -r linea; do
        [[ -n "$linea" ]] || continue
        argv+=("$linea")
    done <<<"$out"
    # ── Preflight: artefacto en disco ─────────────────────────────────────
    # El override ROPERO_<NOMBRE>_GGUF tiene que verse AQUÍ (mismo fichero
    # que resolverá ${gguf}) o el preflight daría por bueno otro cuantizado.
    local fmt="${MODEL_FORMAT[$idx]}"
    local gguf_path=""
    if [[ "$fmt" == "gguf" ]]; then
        local env_gguf
        env_gguf="ROPERO_$(printf '%s' "$real" | tr '[:lower:]-' '[:upper:]_' | tr -d '.')_GGUF"
        local gguf="${MODEL_GGUF[$idx]}"
        gguf_path="$GGUF_DIR/$real/$gguf"
        if [[ -n "${!env_gguf:-}" ]]; then
            gguf_path="$GGUF_DIR/$real/${!env_gguf}"
        fi
        if [[ ! -f "$gguf_path" ]]; then
            rp_msg error "falta el GGUF de '$real': $gguf_path"
            rp_msg info "  (ropero install models $real)"
            return 1
        fi
    elif [[ "$engine" != "fastflowlm" ]]; then
        # safetensors: checkpoint bajo $SAFETENSORS_DIR. flm recibe un TAG de
        # su propio almacén: no hay ruta local que comprobar.
        local mpath="${MODEL_PATH[$idx]}"
        if [[ -z "$mpath" ]]; then
            rp_msg error "'$real' (safetensors) no declara [model].path"
            return 1
        fi
        local chk
        case "$mpath" in
            /*) chk="$mpath" ;;
            *)  chk="$SAFETENSORS_DIR/$mpath" ;;
        esac
        if [[ ! -d "$chk" ]]; then
            rp_msg error "falta el checkpoint de '$real': $chk"
            rp_msg info "  (ropero install models $real)"
            return 1
        fi
    fi

    # llama-server necesita el artefacto en argv: el TOML declara flags, no
    # la ruta del GGUF (esa la resuelve ropero). Sin esto arranca en router
    # mode y no carga nada.
    if [[ "$fmt" == "gguf" && "$engine" == "llama-server" && -n "$gguf_path" ]]; then
        argv=("--model" "$gguf_path" ${argv[@]+"${argv[@]}"})
    fi
    # --cpu se añade AL FINAL (como el monolito): tiene que ganar a los
    # --n-gpu-layers / --threads que declare el TOML.
    if [[ "$cpu" == "1" && "$engine" == "llama-server" ]]; then
        local nproc_n
        nproc_n="$(command -v nproc >/dev/null 2>&1 && nproc || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 8)"
        argv+=(--n-gpu-layers 0 --threads "$nproc_n")
    fi
    local sello
    sello="$(_rp_launch_stamp ${argv[@]+"${argv[@]}"})"

    # El chat template tiene que estar en disco ANTES de arrancar: si no,
    # llama-server aborta con el muro de sus plantillas built-in.
    _rp_launch_check_chat_template "$real" ${argv[@]+"${argv[@]}"} || return 1

    # [limits]: aviso orientativo, nunca bloqueo (ver _rp_launch_check_limits).
    _rp_launch_check_limits "$real"

    # ── Puerto destino (§5.5: --port > pineado > slot) ─────────────────────
    if [[ -n "$port" ]] && ! _rp_launch_valid_port "$port"; then
        rp_msg error "--port $port no es un puerto válido (1-65535)"
        return 1
    fi
    local puerto
    puerto="$(_rp_launch_puerto_destino "$real" "$port" "$cpu")" || {
        rp_msg error "no se pudo resolver el puerto destino de '$real'"
        return 1
    }

    # ── Puerto ocupado ────────────────────────────────────────────────────
    if rp_port_listening "$puerto"; then
        local actual=""
        actual="$(rp_model_on_port "$puerto" 2>/dev/null)" || actual=""
        if [[ -n "$actual" && "$actual" == "$real" ]]; then
            # ¿corre con la config de AHORA? El sello son los args del TOML:
            # si cambió, "ya está corriendo" dejaría la config vieja para
            # siempre y sin avisar (port del sello del legado).
            local sello_viejo=""
            sello_viejo="$(rp_state_read "$puerto" STAMP)" || sello_viejo=""
            if [[ -z "$sello" || "$sello_viejo" == "$sello" ]]; then
                rp_msg ok "'$real' ya está corriendo en :$puerto"
                return 0
            fi
            rp_msg warn "'$real' corre en :$puerto con una configuración anterior; se reinicia"
            rp_stop_port "$puerto" || true
        elif [[ "$force" == "1" ]]; then
            rp_msg warn "--force: parando '${actual:-?}' en :$puerto"
            rp_stop_port "$puerto" || true
        else
            rp_msg error "puerto :$puerto ocupado por '${actual:-desconocido}'"
            rp_msg info "  libéralo con 'ropero stop all' (o 'ropero stop ${actual:-?}') o usa --force"
            return 1
        fi
        local espera=0
        while (( espera < 10 )) && rp_port_listening "$puerto"; do
            sleep 1
            espera=$((espera + 1))
        done
        if rp_port_listening "$puerto"; then
            rp_msg error "no se pudo liberar :$puerto"
            return 1
        fi
    fi

    # ── Lanzamiento ───────────────────────────────────────────────────────
    rp_api_key || {
        rp_msg error "no se pudo resolver la API key ($STATE_DIR/api_key)"
        return 1
    }
    local logfile="$LOG_DIR/$real.log"
    mkdir -p "$LOG_DIR" 2>/dev/null || {
        rp_msg error "no se pudo crear $LOG_DIR"
        return 1
    }
    # Cuántas líneas tiene el log ANTES de este arranque. El log en vivo
    # enseña lo NUEVO desde aquí: el log se acumula, y con "tail -n +1" cada
    # arranque repetía el historial entero (cinco intentos fallidos y el
    # bueno, todo junto, y parecía que seguía roto).
    local log_offset=0
    if [[ -f "$logfile" ]]; then
        log_offset="$(wc -l <"$logfile" 2>/dev/null || echo 0)"
        [[ "$log_offset" =~ ^[0-9]+$ ]] || log_offset=0
    fi

    rp_msg info "arrancando '$real' ($engine) en :$puerto — log: $logfile"

    # EL PID SE CAPTURA SIN SUSTITUCIÓN DE COMANDOS.
    #
    # El engine arranca con "nohup ... &". Si la llamada va dentro de $( ),
    # el proceso lo crea el SUBSHELL, no este shell: al morir el subshell el
    # engine queda huérfano y el `wait` de más abajo responde "pid N no es un
    # proceso hijo de este shell" con rc 127. Como 127 != 0, roperoledgeaba
    # "el modelo terminó con código 127" y limpiaba el estado, mientras el
    # engine seguía vivo: el modelo arrancaba y ropero decía que había
    # muerto. Con el PID en un fichero temporal, el proceso lo crea ESTE
    # shell, `wait` funciona y el código de salida es el del engine.
    local pidfile pid=""
    pidfile="$(mktemp "${TMPDIR:-/tmp}/ropero-launch.XXXXXX" 2>/dev/null)" || {
        rp_msg error "no pude crear el fichero temporal del PID"
        return 1
    }
    if ! rp_engine_call "$engine" launch "$real" "$puerto" "$logfile" \
            ${argv[@]+"${argv[@]}"} >"$pidfile"; then
        cat "$pidfile" >&2
        rm -f "$pidfile"
        rp_msg error "falló el lanzamiento de '$engine'"
        return 1
    fi
    pid="$(head -n 1 "$pidfile")"
    rm -f "$pidfile"
    if [[ ! "$pid" =~ ^[0-9]+$ ]]; then
        rp_msg error "el engine no devolvió un PID válido (recibido: '${pid:-vacío}')"
        return 1
    fi

    # Estado ANTES de esperar: es lo que consume rp_wait_ready (fallback por
    # estado) y lo que hace que `status` vea el arranque en curso.
    rp_state_write "$puerto" \
        "MODEL=$real" "ENGINE=$engine" "PID=$pid" "PORT=$puerto" \
        "LEVEL=$nivel" "TS=$(date +%s)" "LOG=$logfile" "STAMP=$sello"

    # ── Background: esperar al MODELO (no sólo al puerto) ─────────────────
    if [[ "$bg" == "1" ]]; then
        if rp_wait_ready "$puerto" "$engine" "" "$real"; then
            rp_msg ok "'$real' responde en :$puerto"
            return 0
        fi
        # Si el proceso murió por el camino, el estado ya no describe nada.
        if ! kill -0 "$pid" 2>/dev/null; then
            rp_state_clear "$puerto"
        fi
        return 1
    fi

    # ── Primer plano: log en vivo COLOREO + espera del proceso ─────────────
    # El engine siempre arranca en background con salida al logfile; aquí se
    # sigue el log en vivo y se espera al PID. Ctrl-C para el server, limpia
    # el estado y sale 130 (spec §8: 130 = SIGINT).
    #
    # EL LOG EN VIVO VA COLOREADO (06-styles.sh): sin color, la salida del
    # engine es un muro gris en el que no se distingue un error de un dato.
    # El colorizador va por un FIFO para que el tail y él sean los DOS hijos
    # directos y se puedan matar por PID (ver 06-styles.sh).
    rp_msg info "primer plano: Ctrl-C detiene el modelo (salida en $logfile)"
    rp_colorize_follow_start "$logfile" "$real" "$engine" "$log_offset"
    local tail_pid="$RP_COLOR_TAIL_PID" color_pid="$RP_COLOR_PID"
    # El trap mata los dos: sin esto, con Ctrl-C el tail se quedaba vivo
    # bloqueado en inotify (medido en el monolito 1.0).
    trap 'rp_stop_port "$puerto"; _rp_launch_kill_follow "$tail_pid" "$color_pid"; exit 130' INT
    trap 'rp_stop_port "$puerto"; _rp_launch_kill_follow "$tail_pid" "$color_pid"; exit 130' TERM
    wait "$pid"
    local rc_proceso=$?
    trap - INT TERM
    _rp_launch_kill_follow "$tail_pid" "$color_pid"
    rp_state_clear "$puerto"
    # El Ctrl-C lo mata la terminal, que manda SIGINT al GRUPO de procesos:
    # el engine y ropero. El engine muere con 130 y bash difiere el trap
    # INT hasta que el comando esperado acaba (por eso el trap puede no
    # llegar a correr). Se recognizes el 130 por el CÓDIGO, no por el trap.
    if (( rc_proceso == 130 )); then
        exit 130
    fi
    if (( rc_proceso != 0 )); then
        rp_msg error "'$real' terminó con código $rc_proceso; últimas líneas del log:"
        tail -n 15 "$logfile" 2>/dev/null | sed 's/^/    /' >&2
        return 1
    fi
    return 0
}
