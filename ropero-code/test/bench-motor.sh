#!/usr/bin/env bash
# =============================================================================
# bench-motor.sh — A/B de motores: el MISMO modelo y el MISMO argv, cambiando
# sólo el binario de llama-server.
#
# POR QUÉ ESTE Y NO EL BANCO DE OPENCODE
#
# El banco de siempre (tests/bench_ropero.sh, borrado en el refactor a 1.0)
# mide calidad con opencode y un juez. Para la pregunta "¿el merge me quitó
# rendimiento?" eso es ruido: mete al modelo, al cliente, al prompt y al
# juez en el medio, y un cambio de 3 t/s se esconde en la varianza.
#
# Aquí la medición es directa: el server con el argv del TOML, el mismo
# prompt, n tokens, y los timings que el propio server devuelve
# (timings.predicted_per_second). Lo único que cambia entre A y B es el
# binario.
#
# Y COMO SE HACE QUE SOLO CAMBIE EL BINARIO
#
# Arrancando con ropero NO vale: el argv sale del catálogo, y el catálogo
# pasa flags que sólo entiende el fork (--moe-cache, --cache-type-k/v
# turbo3). El motor OFICIAL abortaría al arrancar y la comparación sería
# imposible (o, peor, mediría dos veces el mismo motor).
#
# Así que el banco arranca el binario DIRECTO, con el argv MÍNIMO y COMÚN
# a los dos: --model, --host, --port, --api-key, -c y --n-gpu-layers. El
# resto son los defaults de cada motor. Lo único que cambia entre A y B es
# el binario: la comparación es limpia por construcción, no por promesa.
#
# USO
#   tests/bench-motor.sh --modelo agenta-tiny \
#       --motor-a ~/.local/bin/llama-server \
#       --motor-b ~/atomic/llama.cpp/build/bin/llama-server
#
#   (agenta-tiny es el mejor sujeto para el A/B: 4B, cabe entero en la
#    VRAM, carga en segundos y su decode es un número limpio.)
#
#   tests/bench-motor.sh --modelo nemotron --repeticiones 8 --descanso 30
#       (8 rondas con 30 s entre medias: sirve para ver si el rendimiento SE
#        degrada con el tiempo en uso, que es la otra pregunta)
#
# OPCIONES
#   --modelo <n>        modelo del catálogo de ropero (obligatorio)
#   --motor-a <ruta>    binario A (por defecto: el que ropero usaría)
#   --motor-b <ruta>    binario B (por defecto: el mismo que A; útil para
#                       medir la varianza del propio banco)
#   --puerto <n>        puerto base (por defecto 19980; cada motor usa uno)
#   --ctx <n>           contexto para los dos motores (por defecto 8192:
#                       el banco mide el decode con contexto CORTO; el
#                       contexto enorme es otro problema, con otro banco)
#   --ngpu <n>          capas a la GPU para los dos motores (por defecto 99)
#   --args <modo>       comun (por defecto: la intersección de flags de A y B,
#                       el mismo argv para los dos) | catalogo (el argv del
#                       catálogo tal cual, sin filtrar: para medir un motor
#                       solo) | minimos (sólo lo común)
#   --n-predict <n>     tokens a generar (por defecto 256)
#   --prompt <texto>    prompt (por defecto: un texto fijo)
#   --repeticiones <n>  rondas por motor (por defecto 3)
#   --descanso <s>      segundos entre rondas (por defecto 0)
#   --out <dir>         dónde escribir el informe (por defecto /tmp/bench-motor)
#
# SALIDA
#   Una tabla por motor (t/s de decode y de prompt por ronda) y el estado de
#   la GPU en cada ronda (relojes, potencia, temperatura, razones de
#   throttle). Al final, el veredicto: deltas y si la GPU se degradó.
# =============================================================================
set -uo pipefail

# ── Locales ──────────────────────────────────────────────────────────────────
SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROPERO_ROOT="${ROPERO_ROOT:-$(cd "$SELF_DIR/.." && pwd)}"

MODELO=""
MOTOR_A=""
MOTOR_B=""
PUERTO_BASE="${PUERTO_BASE:-19980}"
N_PREDICT="${N_PREDICT:-256}"
REPETICIONES="${REPETICIONES:-3}"
DESCANSO="${DESCANSO:-0}"
OUT="${OUT:-/tmp/bench-motor}"
PROMPT="${PROMPT:-Explica en un párrafo por qué el mar es salado.}"
# La clave de la API.
#
# ropero arranca el server CON `--api-key` (auth nativa): el launcher la
# resuelve por env > $STATE_DIR/api_key > generada (sk-ropero-<hex>). Si el
# bench manda una clave distinta, el server responde 401 y TODAS las
# peticiones fallan con "la petición falló" (que es lo que pasaba: 8/8
# errores con los dos motores, y la GPU al 0 % el tiempo entero).
#
# La solución: el bench FIJA LLAMA_API_KEY en el entorno antes de arrancar
# los servers. Así ropero usa esa misma (env > fichero > generada) y el
# bench manda la misma.
export LLAMA_API_KEY="${LLAMA_API_KEY:-sk-bench-local}"
API_KEY="$LLAMA_API_KEY"
# El banco compara MOTORES, no configuraciones de catálogo. El catálogo de
# ropero pasa flags que sólo entiende el fork (--moe-cache, --cache-type-k/v
# turbo3): si el banco arrancara con ellos, el motor OFICIAL abortaría al
# arrancar y la comparación sería imposible. Por eso aquí se arranca el
# binario DIRECTO con lo mínimo y COMÚN, y la diferencia que se mide es la
# del motor.
GGUF_DIR="${GGUF_DIR:-$HOME/models/gguf}"
CTX="${CTX:-}"
NGPU="${NGPU:-99}"
NIVEL="${NIVEL:-}"
# Los args del motor:
#   comun    (por defecto) la intersección de los flags que entienden A y B:
#            mismo argv para los dos → la comparación es justa por construcción
#   catalogo el argv del catálogo TAL CUAL (sin filtrar): para medir UN motor
#            (p. ej. el coste de la moe-cache, que el fork de Atomic no tiene)
#   minimos  sólo --model --host --port --api-key -c: la línea de base
MODO_ARGS="${MODO_ARGS:-comun}"
LISTAR=0

# ── Dónde está el motor ─────────────────────────────────────────────────────
#
# ropero lo resuelve por: PATH → $BIN_DIR → $FARM_DIR → $LLAMA_DIR/build/bin.
# Si no lo encuentra, el motor NO está instalado para ropero (por eso el
# doctor dice "engine llama-server no está instalado"), y el banco tiene que
# decirlo claro en vez de "el motor no es ejecutable".
candidatos_motor() {
    local shopt_save
    shopt_save="$(shopt -p nullglob)"
    shopt -s nullglob
    printf '%s\n' \
        "$(command -v llama-server 2>/dev/null || true)" \
        "$HOME/.local/bin/llama-server" \
        "$HOME/.local/share/ropero/llama-server" \
        "$HOME/llama.cpp/build/bin/llama-server" \
        "$HOME"/atomic*/llama.cpp/build/bin/llama-server \
        "$HOME"/llama*/build/bin/llama-server
    eval "$shopt_save"
}

resolver_motor() {
    local c
    while IFS= read -r c; do
        [[ -n "$c" && -x "$c" ]] && { printf '%s' "$c"; return 0; }
    done < <(candidatos_motor)
    return 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --modelo)      MODELO="$2"; shift 2 ;;
        --motor-a)     MOTOR_A="$2"; shift 2 ;;
        --motor-b)     MOTOR_B="$2"; shift 2 ;;
        --puerto)      PUERTO_BASE="$2"; shift 2 ;;
        --n-predict)   N_PREDICT="$2"; shift 2 ;;
        --prompt)      PROMPT="$2"; shift 2 ;;
        --repeticiones) REPETICIONES="$2"; shift 2 ;;
        --descanso)    DESCANSO="$2"; shift 2 ;;
        --out)         OUT="$2"; shift 2 ;;
        --ctx)         CTX="$2"; shift 2 ;;
        --nivel)       NIVEL="$2"; shift 2 ;;
        --args)        MODO_ARGS="$2"; shift 2 ;;
        --ngpu)        NGPU="$2"; shift 2 ;;
        --listar)      LISTAR=1; shift ;;
        --help|-h)     sed -n '2,60p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "opción desconocida: $1" >&2; exit 2 ;;
    esac
done

if (( LISTAR == 1 )); then
    echo "candidatos de llama-server:"
    candidatos_motor | sed 's/^/    /'
    exit 0
fi
[[ -n "$MODELO" ]] || { echo "falta --modelo" >&2; exit 2; }
[[ "$REPETICIONES" =~ ^[0-9]+$ ]] || { echo "--repeticiones debe ser un número" >&2; exit 2; }

log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }

# De qué checkout se está midiendo, y de qué motores. Sin esto, un A/B con
# dos binariosunknown se parece a cualquier otro y no se puede auditar.
log "ropero en: $ROPERO_ROOT ($(git -C "$ROPERO_ROOT" rev-parse --short HEAD 2>/dev/null || echo 'sin git') $(git -C "$ROPERO_ROOT" log -1 --format=%cd --date=short 2>/dev/null || echo ''))"

mkdir -p "$OUT"

# contexto_catalogo — el contexto que declara el catálogo (campo 6 del dump).
#
# Es el MISMO que usa ropero, y importa: el KV come VRAM y la VRAM decide
# cuánto puede usar la moe-cache. Medir con otro contexto es medir otra cosa.
contexto_catalogo() {
    local dump sep
    dump="$(python3 "$ROPERO_ROOT/bin/python/modelconf.py" dump \
        --root "$ROPERO_ROOT/models" --include-fired 2>/dev/null || true)"
    sep=$'\x1f'
    grep -m1 "^${MODELO}" <<< "$dump" | cut -d"$sep" -f6
}

# flags_comunes — los flags que ENTIENDEN LOS DOS motores (la intersección).
#
# Se usa el mismo argv para A y B, así que la comparación es justa por
# construcción. Y de paso reproduce el "antes": el fork de Atomic no
# conoce --moe-cache, así que ese flag se cae de los dos → los dos se
# miden sin moe-cache, que es como estaba antes del merge.
preparar_flags_comunes() {
    local fa fb tmp
    fa="$OUT/flags-A.txt"; fb="$OUT/flags-B.txt"; tmp="$OUT/flags-tmp.txt"
    "$MOTOR_A" --help >"$fa" 2>&1 || true
    "$MOTOR_B" --help >"$fb" 2>&1 || true
    log "A: $MOTOR_A → $("$MOTOR_A" --version 2>&1 | head -1)"
    log "B: $MOTOR_B → $("$MOTOR_B" --version 2>&1 | head -1)"
    grep -oE '(^|[[:space:]])--[a-z0-9][a-z0-9-]*' "$fa" | tr -d ' \t' | sort -u >"$tmp" 2>/dev/null || true
    grep -oE '(^|[[:space:]])--[a-z0-9][a-z0-9-]*' "$fb" | tr -d ' \t' | sort -u | comm -12 "$tmp" - >"$OUT/flags-comunes.txt" 2>/dev/null || true
    local n
    n="$(grep -c . "$OUT/flags-comunes.txt" 2>/dev/null || echo 0)"
    log "flags que entienden los dos motores: $n"
}

soporta_flag() {
    local flag="$1"
    grep -q -- "--$flag" "$OUT/flags-comunes.txt" 2>/dev/null
}

# filtrar_args <motor> <args...> — deja sólo lo que el motor entiende.
filtrar_args() {
    local out=() flag
    while (( $# )); do
        flag="$1"; shift
        if [[ "$flag" == --* ]]; then
            if ! soporta_flag "${flag#--}"; then
                # el flag no existe en este binario: se cae él y su valor
                if (( $# )) && [[ "$1" != --* ]]; then shift; fi
                continue
            fi
        fi
        out+=("$flag")
    done
    printf '%s\n' "${out[@]+"${out[@]}"}"
}

# resolver_motor_binario <ruta> — si es un directorio, busca el binario
# dentro. Con el banco viejo esto era una trampa silenciosa: se metía el
# directorio en el PATH, ropero NO encontraba llama-server ahí y acababa
# usando el motor de siempre → los dos "motores" comparados eran el MISMO
# binario, y el A/B no midía nada.
resolver_motor_binario() {
    local motor="$1" cand
    if [[ ! -d "$motor" ]]; then
        printf '%s' "$motor"
        return 0
    fi
    for cand in "$motor/build/bin/llama-server" "$motor/bin/llama-server" "$motor/llama-server"; do
        [[ -x "$cand" ]] && { printf '%s' "$cand"; return 0; }
    done
    return 1
}

# ── El modelo y su argv (los del catálogo: la config es la de siempre) ───────
if ! ARGV_MODELO="$(cd "$ROPERO_ROOT" && bash bin/ropero args "$MODELO" 2>/dev/null)"; then
    log "no se pudo obtener el argv de '$MODELO' (¿existe en el catálogo?)"
    exit 1
fi
GGUF="$(cd "$ROPERO_ROOT" && bash bin/ropero path "$MODELO" 2>/dev/null | sed -n 's/^GGUF=//p' | head -1)"
if [[ -z "$GGUF" || ! -f "$GGUF" ]]; then
    log "no se encontró el GGUF de '$MODELO' (ropero path $MODELO)"
    exit 1
fi
log "modelo: $MODELO"
log "gguf:   $GGUF"
if [[ -z "$CTX" ]]; then
    CTX="$(contexto_catalogo)"
    [[ -n "$CTX" ]] && log "contexto: $CTX (el del catálogo)"
fi

if [[ -z "$MOTOR_A" || "$MOTOR_A" == "auto" ]]; then
    MOTOR_A="$(resolver_motor || true)"
fi
if [[ -z "$MOTOR_A" ]]; then
    log "no encuentro ningún llama-server. Se ha buscado en:"
    candidatos_motor | sed 's/^/    /' | while IFS= read -r l; do log "$l"; done
    log "instálalo con 'ropero install engines llama-server' o pásalo con --motor-a <ruta>"
    exit 1
fi
[[ -n "$MOTOR_B" ]] || MOTOR_B="$MOTOR_A"
# Se resuelven UNA VEZ (un directorio → el binario que hay dentro). Si no,
# la lista de flags se calcularía sobre un directorio y saldría vacía.
if [[ -d "$MOTOR_A" ]]; then
    _res="$(resolver_motor_binario "$MOTOR_A")" && MOTOR_A="$_res" || {
        log "$MOTOR_A es un directorio y no tiene build/bin/llama-server dentro"
        find "$MOTOR_A" -maxdepth 3 -type f -name 'llama-server' -perm -u+x 2>/dev/null | head -5 | while IFS= read -r l; do log "    $l"; done
        exit 1
    }
fi
if [[ -d "$MOTOR_B" ]]; then
    _res="$(resolver_motor_binario "$MOTOR_B")" && MOTOR_B="$_res" || {
        log "$MOTOR_B es un directorio y no tiene build/bin/llama-server dentro"
        find "$MOTOR_B" -maxdepth 3 -type f -name 'llama-server' -perm -u+x 2>/dev/null | head -5 | while IFS= read -r l; do log "    $l"; done
        exit 1
    }
fi
preparar_flags_comunes

# ── Utilidades ───────────────────────────────────────────────────────────────

# gpu_estado <etiqueta> — una línea con el estado de la GPU (para el informe).
gpu_estado() {
    local etiq="$1"
    if ! command -v nvidia-smi >/dev/null 2>&1; then
        printf '%s\t%s\n' "$etiq" "nvidia-smi no disponible"
        return 0
    fi
    local q clocks power temp util throttle
    clocks="$(nvidia-smi --query-gpu=clocks.sm --format=csv,noheader,nounits 2>/dev/null | head -1)"
    power="$(nvidia-smi --query-gpu=power.draw --format=csv,noheader,nounits 2>/dev/null | head -1)"
    temp="$(nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader,nounits 2>/dev/null | head -1)"
    util="$(nvidia-smi --query-gpu=utilization.gpu --format=csv,noheader,nounits 2>/dev/null | head -1)"
    # Las razones de throttle SOLO se obtienen en texto largo: es el dato que
    # explica una caída de rendimiento (y el que el setup performance no toca).
    throttle="$(nvidia-smi -q -d PERFORMANCE 2>/dev/null \
        | grep -iE "^\s+(SW Thermal Slowdown|HW Slowdown|SW Power Cap|Applications Clocks Setting|Display Clock Setting)\s*:" \
        | sed 's/^[[:space:]]*//' | tr '\n' ' ')"
    printf '%s\tSM %s MHz\t%s W\t%s C\t%s%%\t%s\n' \
        "$etiq" "${clocks:-?}" "${power:-?}" "${temp:-?}" "${util:-?}" "${throttle:-sin reasons}"
}

# _bench_error <mensaje> — el motivo del fallo (visible en la tabla).
_bench_error() {
    printf 'ERROR\t%s\n' "${1:-la petición falló}" >&2
}

# tag_motor <ruta del binario> — una etiqueta única por binario.
#
# Los tres motores se llaman igual (llama-server): con sólo el basename los
# ficheros de A y B se pisan (arranque-llama-server.txt, pid-...) y sus logs
# quedan entremezclados, sin poder atribuir un arranque a un motor.
#
# El discriminante es el CHECKOUT, no la carpeta donde vive el binario: los
# motores están en <checkout>/build/bin, así que se suben las carpetas
# genéricas (bin/, build/) hasta llegar al checkout: llama-server@llama.cpp,
# llama-server@llama.cpp.TURBO, llama-server@llama.cpp-official.
tag_motor() {
    local m="$1" d
    d="$(dirname "$m")"
    while [[ "$(basename "$d")" == "bin" || "$(basename "$d")" == "build" ]]; do
        d="$(dirname "$d")"
    done
    printf '%s' "$(basename "$m")@$(basename "$d")"
}

# pedir <puerto> <n_predict> — una completion y sus timings.
pedir() {
    local puerto="$1" n="$2" cuerpo resp
    cuerpo="$(printf '{"prompt":%s,"n_predict":%s,"stream":false,"cache_prompt":false}' \
        "$(printf '%s' "$PROMPT" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')" "$n")"
    # -w para saber el código HTTP: "la petición falló" sin más no dice si
    # fue un 401 (clave), un 404 (modelo) o un 503 (el server aún no está).
    local codigo
    # La ruta es /completion (legacy): /v1/completion NO existe en estos
    # builds de llama-server y devuelve HTTP 404 ("File Not Found").
    resp="$(curl -sS --max-time 600 \
        -w '\n%{http_code}' \
        -H "Authorization: Bearer $API_KEY" \
        -H "Content-Type: application/json" \
        -d "$cuerpo" \
        "http://127.0.0.1:$puerto/completion" 2>/dev/null)"
    codigo="${resp##*$'\n'}"
    resp="${resp%$'\n'*}"
    if [[ "$codigo" != "200" ]]; then
        _bench_error "HTTP $codigo${resp:+ · $(printf '%s' "$resp" | head -c 160)}"
        return 1
    fi
    [[ -n "$resp" ]] || { _bench_error "respuesta vacía"; return 1; }
    printf '%s' "$resp" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(1)
t = d.get("timings") or {}
print("%s\t%s\t%s" % (
    t.get("predicted_per_second", "?"),
    t.get("prompt_per_second", "?"),
    t.get("predicted_n", "?")))
' 2>/dev/null
}

# arrancar <binario> <puerto> — arranca el motor DIRECTO (mismo argv para
# todos: lo que se compara es el motor, no el catálogo).
arrancar() {
    local motor="$1" puerto="$2" dir
    local resuelto
    if ! resuelto="$(resolver_motor_binario "$motor")"; then
        log "  $motor es un directorio y no tiene build/bin/llama-server dentro"
        log "    candidatos:"
        find "$motor" -maxdepth 3 -type f -name 'llama-server' -perm -u+x 2>/dev/null | head -5 | while IFS= read -r l; do log "      $l"; done
        return 1
    fi
    motor="$resuelto"
    dir="$(dirname "$motor")"
    if [[ ! -e "$motor" ]]; then
        log "no existe el motor: $motor"
        log "  candidatos:"
        candidatos_motor | sed 's/^/    /' | while IFS= read -r l; do log "$l"; done
        return 1
    fi
    if [[ ! -x "$motor" ]]; then
        log "el motor no es ejecutable: $motor (chmod +x)"
        return 1
    fi
    local gguf="$GGUF"
    log "  modelo: $gguf"
    # El "&" va AQUÍ (no dentro de un $(...)): así el proceso es hijo de
    # este shell y su PID sirve para pararlo. En un subshell quedaría
    # huérfano.
    local -a argv
    argv=(--model "$gguf" --host 127.0.0.1 --port "$puerto" --api-key "$API_KEY")
    if [[ -n "$CTX" ]]; then argv+=(-c "$CTX"); fi
    if [[ "$MODO_ARGS" != "minimos" ]]; then
        # Sin mapfile: el repo declara bash 3.2 como soportado.
        local -a cat=() filtrados=() a
        while IFS= read -r a; do cat+=("$a"); done <<< "$ARGV_MODELO"
        if (( ${#cat[@]} )); then
            if [[ "$MODO_ARGS" == "catalogo" ]]; then
                filtrados=("${cat[@]}")
            else
                while IFS= read -r a; do filtrados+=("$a"); done < <(filtrar_args "${cat[@]}")
            fi
            argv+=("${filtrados[@]}")
            log "  args: ${#cat[@]} -> ${#filtrados[@]} (modo $MODO_ARGS)"
            # Lo que se ha caído: sin esto no se sabe qué se midió de verdad.
            local -a caidos=() flag v
            while IFS= read -r flag; do
                [[ -n "$flag" ]] || continue
                if ! printf '%s\n' "${filtrados[@]}" | grep -qx -- "$flag"; then
                    caidos+=("$flag")
                fi
            done <<< "$ARGV_MODELO"
            if (( ${#caidos[@]} )); then
                log "  flags que este motor NO entiende (se caen): ${caidos[*]}"
            fi
        fi
    else
        argv+=(--n-gpu-layers "$NGPU")
    fi
    # El argv, a disco: para poder auditar la medición después.
    printf '%s\n' "${argv[@]}" >"$OUT/argv-$(tag_motor "$motor").txt"
    "$motor" "${argv[@]}" \
        >>"$OUT/arranque-$(tag_motor "$motor").txt" 2>&1 &
    local pid=$!
    printf '%s' "$pid" >"$OUT/pid-$(tag_motor "$motor").txt"
    log "  pid: $pid"
}

# parar <puerto> [motor]
parar() {
    local puerto="$1" motor="${2:-}"
    # El PID que arrancó este bench (nunca un pkill global).
    local pf pid
    pf=""
    [[ -n "$motor" ]] && pf="$OUT/pid-$(tag_motor "$motor").txt"
    if [[ -r "$pf" ]]; then
        pid="$(cat "$pf")"
        [[ -n "$pid" ]] && { kill "$pid" 2>/dev/null || true; sleep 1; kill -9 "$pid" 2>/dev/null || true; }
        rm -f "$pf"
    fi
    # Por si acaso: matar lo que escuche en el puerto (nunca pkill global).
    # lsof NO está en todas las máquinas (en la del usuario no lo hay, y
    # el "kill" se comía el error en silencio): ss → fuser.
    local pids
    pids="$(fuser -n tcp "$puerto" 2>/dev/null | tr -s ' ' '\n' | grep -E '^[0-9]+$' || true)"
    if [[ -z "$pids" ]]; then
        pids="$(ss -lptnH "sport = :$puerto" 2>/dev/null | grep -oE 'pid=[0-9]+' | cut -d= -f2 | sort -u || true)"
    fi
    for p in $pids; do kill -9 "$p" 2>/dev/null || true; done
}

esperar_listo() {
    local puerto="$1" i
    for i in $(seq 1 300); do
        if curl -fsS --max-time 2 -H "Authorization: Bearer $API_KEY" \
             "http://127.0.0.1:$puerto/health" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
    done
    return 1
}

# ── El ciclo de un motor ─────────────────────────────────────────────────────
# medir_motor <etiqueta> <binario> <puerto> <fichero de resultados>
medir_motor() {
    local etiq="$1" motor="$2" puerto="$3" fich="$4"
    local rep ps pp pn
    : >"$fich"

    log "=== $etiq: $motor (puerto $puerto) ==="
    if ! arrancar "$motor" "$puerto"; then
        log "$etiq: el arranque falló (ver $OUT/arranque-*.txt)"
        printf 'ERROR\tarranque falló\n' >>"$fich"
        parar "$puerto" "$motor"
        return 1
    fi
    if ! esperar_listo "$puerto"; then
        log "$etiq: el server no llegó a estar listo"
        parar "$puerto" "$motor"
        printf 'ERROR\tno estuvo listo\n' >>"$fich"
        return 1
    fi
    log "$etiq: listo"

    for rep in $(seq 1 "$REPETICIONES"); do
        gpu_estado "$etiq-ronda$rep-antes" >>"$OUT/gpu.txt"
        if ! out="$(pedir "$puerto" "$N_PREDICT")"; then
            # el motivo va en el stderr de pedir(); aquí se deja el hueco
            printf 'ERROR\tla petición falló (mira el stderr: HTTP)\n' >>"$fich"
            gpu_estado "$etiq-ronda$rep-despues" >>"$OUT/gpu.txt"
            continue
        fi
        ps="$(printf '%s' "$out" | cut -f1)"
        pp="$(printf '%s' "$out" | cut -f2)"
        pn="$(printf '%s' "$out" | cut -f3)"
        printf 'ronda%s\t%s\t%s\t%s\n' "$rep" "$ps" "$pp" "$pn" >>"$fich"
        gpu_estado "$etiq-ronda$rep-despues" >>"$OUT/gpu.txt"
        log "$etiq ronda $rep: ${ps} t/s decode · ${pp} t/s prompt · ${pn} tokens"
        (( DESCANSO > 0 )) && sleep "$DESCANSO"
    done

    parar "$puerto" "$motor"
    return 0
}

# ── Arranque ─────────────────────────────────────────────────────────────────
: >"$OUT/gpu.txt"
log "salida en $OUT"

medir_motor "A" "$MOTOR_A" "$PUERTO_BASE" "$OUT/A.tsv"
medir_motor "B" "$MOTOR_B" "$((PUERTO_BASE + 1))" "$OUT/B.tsv"

# ── Informe ──────────────────────────────────────────────────────────────────
echo
echo "════════════════════════════════════════════════════════════════════"
echo " A/B de motores · modelo: $MODELO · $N_PREDICT tokens · $REPETICIONES rondas"
echo "════════════════════════════════════════════════════════════════════"
printf '\n%-6s %-10s %-12s %-12s %s\n' "motor" "ronda" "decode t/s" "prompt t/s" "tokens"
for etiqueta in A B; do
    fich="$OUT/$etiqueta.tsv"
    [[ -f "$fich" ]] || continue
    while IFS=$'\t' read -r r ps pp pn; do
        [[ -n "$r" ]] || continue
        printf '%-6s %-10s %-12s %-12s %s\n' "$etiqueta" "$r" "$ps" "$pp" "$pn"
    done <"$fich"
done

# Medias (sólo de las rondas con número).
python3 - "$OUT" <<'PY'
import sys, os
out = sys.argv[1]
def vals(etq):
    p = os.path.join(out, etq + ".tsv")
    if not os.path.exists(p): return []
    r = []
    for line in open(p):
        parts = line.rstrip("\n").split("\t")
        if len(parts) == 4 and parts[0].startswith("ronda"):
            try: r.append(float(parts[1]))
            except ValueError: pass
    return r
a, b = vals("A"), vals("B")
if a and b:
    ma, mb = sum(a)/len(a), sum(b)/len(b)
    print("\nmedia decode:  A %.2f t/s   B %.2f t/s   ->  B %+.1f%%" % (ma, mb, (mb-ma)/ma*100 if ma else 0))
    print("rondas:        A %s" % ", ".join("%.1f" % x for x in a))
    print("               B %s" % ", ".join("%.1f" % x for x in b))
    if len(a) > 1 and len(b) > 1:
        # ¿Se degrada con el tiempo? (la primera ronda vs la última)
        print("primera vs última:  A %+.1f%%   B %+.1f%%" % (
            (a[-1]-a[0])/a[0]*100, (b[-1]-b[0])/b[0]*100))
PY

echo
echo "─── estado de la GPU durante las rondas ───"
cat "$OUT/gpu.txt" 2>/dev/null | sed 's/^/  /'
echo
echo "informes: $OUT"
