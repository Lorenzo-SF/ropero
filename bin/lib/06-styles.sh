#!/usr/bin/env bash
# =============================================================================
# 06-styles.sh — el COLOR de la salida en vivo del engine (spec §5.2)
# =============================================================================
# POR QUÉ EXISTE
#   El engine escribe su salida en el logfile y, en primer plano, ese log se
#   sigue en vivo por la terminal. Sin color, la salida del engine es un muro
#   gris: los errores no se distinguen de los avisos y los tokens/s no se ven.
#   El monolito 0.x lo tenía (bin/_common.sh: colorize) y el refactor 1.0 lo
#   perdió al pasar de "engine en primer plano por una tubería" a "engine en
#   background + tail del log".
#
# CONTRATO
#   rp_colorize <tema> <engine>   filtra stdin → stdout coloreando por
#                                 semántica (error, rendimiento, aviso, HTTP).
#                                 Sin ANSI si NO_COLOR o stderr sin TTY.
#
#   rp_colorize_follow <fichero> <tema> <engine>
#                                 sigue el fichero y lo colorea. UN SOLO
#                                 proceso: el tail y el colorizador van por un
#                                 FIFO, no por una tubería (ver el módulo de
#                                 arranque), para que los dos sean hijos
#                                 directos del shell y se puedan matar por PID.
#
# Seguro de sourcear: sólo definiciones, cero ejecución.
# bash 3.2: sin namerefs, sin arrays asociativos, sin mapfile (spec §3).
# =============================================================================

if ! declare -F _rp_ui_ensure >/dev/null 2>&1; then
    # 00-utils.sh es siempre (lo carga el dispatcher). Si no está, alaja no
    # hay y el color sale plano. OJO: el helper se llama _rp_ui_ensure (con
    # guion bajo). Comprobar el nombre SIN guion hace que esta rama se
    # ejecute siempre y ponga RP_UI_ANSI=0: el color, apagado.
    RP_UI_ANSI=0
    _rp_ui_ensure() { :; }
fi

# -----------------------------------------------------------------------------
# rp_colorize <tema> <engine> — stdin → stdout, coloreado.
#
# La paleta por SEMántica (no por modelo): lo que el usuario necesita ver de
# un vistazo es qué línea es un error y cuál es un dato de rendimiento. El
# "tema" sólo cambia el color de las líneas informative del engine.
#
# Sin ANSI cuando NO_COLOR está fijado o cuando stderr no es un TTY: la
# misma convención que el resto de la presentación (00-utils.sh).
# -----------------------------------------------------------------------------
rp_colorize() {
    local tema="${1:-default}" engine="${2:-llama-server}"

    _rp_ui_ensure
    if [[ "${RP_UI_ANSI:-0}" != "1" ]]; then
        cat
        return 0
    fi

    awk -v t="$tema" -v e="$engine" '
    BEGIN {
        RST = "\033[0m"
        C1 = "\033[38;5;208m"
        C2 = "\033[38;5;214m"
        C3 = "\033[38;5;36m"
        C4 = "\033[38;5;51m"
        C5 = "\033[38;5;105m"
        C6 = "\033[38;5;201m"
        C7 = "\033[38;5;141m"
        C8 = "\033[38;5;228m"
        DEF = "\033[1;32m"
        BRED = "\033[1;31m"
        YEL = "\033[1;33m"
        MAG = "\033[1;35m"
        CYN = "\033[2;36m"
        GRY = "\033[90m"

        # Tema por modelo: el color de las líneas "informativas" del engine.
        if (t == "gptoss") MAIN = C1
        else if (t == "qwen" || t == "qwen36" || t == "qwen38") MAIN = C2
        else if (t == "coder" || t == "qwencoder") MAIN = C3
        else if (t == "coder30b") MAIN = C4
        else if (t == "codernext" || t == "coder30a3b") MAIN = C5
        else if (t == "embed") MAIN = C6
        else if (t == "muse") MAIN = C7
        else if (t == "glmf" || t == "groot" || t == "nemotron" || t == "granite" || t == "ornith") MAIN = C8
        else if (t ~ /air/) MAIN = C7
        else if (e == "tensorrt-llm") MAIN = C6
        else if (e == "mlx-serve") MAIN = C5
        else MAIN = DEF
    }
    function c(color, msg) { printf "%s%s%s\n", color, msg, RST }
    {
        line = $0
        lower = tolower(line)
        # 1. Errores: lo primero, para que se vean por encima de todo.
        if (lower ~ /(oom|out of memory|cuda error|metal.*error|segfault|abort|fatal|traceback|error|unrecognized arguments|[[:space:]]5[0-9][0-9]([[:space:]]|$))/)
            { c(BRED, line); next }
        # 2. Rendimiento: el dato que se mira mientras infiere.
        if (lower ~ /(tok\/s|tokens per second|throughput|eval time|prompt eval)/)
            { c(MAG, line); next }
        # 3. Lo propio de cada engine.
        if (e == "tensorrt-llm" && line ~ /^\[TRT-LLM\][[:space:]]*[IWE]/)
            { c(MAIN, line); next }
        if (e == "airllm" && line ~ /(uvicorn|FastAPI|airllm-server|Uvicorn running|Application startup|HTTP\/)/)
            { c(MAIN, line); next }
        if (e == "mlx-serve" && line ~ /(mlx|throughput|tokens\/s|memory|HTTP\/)/)
            { c(MAIN, line); next }
        if (e == "fastflowlm" && line ~ /(flm|FastFlowLM|serve|listening)/)
            { c(MAIN, line); next }
        # 4. Arranque y carga del modelo.
        if (line ~ /(authproxy|proxy.*(listening|started)|loaded|llm_load|CUDA|Metal|GPU|server listening|ready|main: server|model loading|build finished)/)
            { c(MAIN, line); next }
        # 5. Avisos.
        if (line ~ /(WRN|warning|deprecated)/)
            { c(YEL, line); next }
        # 6. Peticiones HTTP.
        if (line ~ /(HTTP|GET|POST|chat|srv)/)
            { c(CYN, line); next }
        c(GRY, line)
    }'
}

# -----------------------------------------------------------------------------
# rp_colorize_follow_start <fichero> <tema> <engine>
#
# Deja el log en vivo COLOREA en stderr, y deja los dos PIDs en las globales
# RP_COLOR_TAIL_PID y RP_COLOR_PID para que quien llama los pueda matar.
#
# POR QUÉ UN FIFO Y NO UNA TUBERÍA
#   En "tail -F f | rp_colorize", $! es el PID del ÚLTIMO (el colorizador).
#   El tail queda como hijo del shell, pero su PID se pierde: no hay forma
#   limpia de matarlo, y se queda bloqueado en inotify para siempre. Medido
#   en el monolito 1.0: con Ctrl-C el tail se quedaba vivo, sin dueño.
#
#   Con un FIFO, el tail y el colorizador son los DOS hijos directos: sus PIDs
#   se conocen y se matan juntos. El FIFO se borra en cuanto ambos lo tienen
#   abierto (los descriptores lo mantienen vivo).
#
# Si mkfifo no existiera, degrada a "log en vivo sin color" en vez de fallar.
# -----------------------------------------------------------------------------
RP_COLOR_TAIL_PID=""
RP_COLOR_PID=""

rp_colorize_follow_start() {
    local fichero="${1:-}" tema="${2:-default}" engine="${3:-llama-server}"
    local fifo="${TMPDIR:-/tmp}/ropero-color.$$"

    RP_COLOR_TAIL_PID=""
    RP_COLOR_PID=""
    [[ -n "$fichero" ]] || return 1

    if ! mkfifo "$fifo" 2>/dev/null; then
        # Sin FIFO: el log en vivo, sin color. Es peor, pero funciona.
        tail -n +1 -F "$fichero" >&2 2>/dev/null &
        RP_COLOR_TAIL_PID=$!
        return 0
    fi

    tail -n +1 -F "$fichero" >"$fifo" 2>/dev/null &
    RP_COLOR_TAIL_PID=$!
    rp_colorize "$tema" "$engine" <"$fifo" >&2 &
    RP_COLOR_PID=$!
    rm -f "$fifo"
    return 0
}
