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
# El launcher resuelve el motor por PATH primero (rp_eng_llama-server_resolve
# → `command -v llama-server`), así que basta con poner el binario que se
# quiere medir al frente del PATH y arrancar con ropero NORMAL. El argv, el
# modelo, el puerto y el resto de la configuración son los del catálogo: la
# comparación es limpia por construcción, no por promesa.
#
# USO
#   tests/bench-motor.sh --modelo nemotron \
#       --motor-a ~/.local/bin/llama-server \
#       --motor-b ~/atomic/llama.cpp/build/bin/llama-server
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
API_KEY="sk-bench-local"

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
        --help|-h)     sed -n '2,60p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "opción desconocida: $1" >&2; exit 2 ;;
    esac
done

[[ -n "$MODELO" ]] || { echo "falta --modelo" >&2; exit 2; }
[[ "$REPETICIONES" =~ ^[0-9]+$ ]] || { echo "--repeticiones debe ser un número" >&2; exit 2; }

log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }

mkdir -p "$OUT"

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

# El motor A por defecto: el que ropero usaría.
if [[ -z "$MOTOR_A" ]]; then
    MOTOR_A="$(command -v llama-server 2>/dev/null || true)"
    [[ -n "$MOTOR_A" ]] || MOTOR_A="$ROPERO_ROOT/bin/llama-server"
fi
[[ -n "$MOTOR_B" ]] || MOTOR_B="$MOTOR_A"

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

# pedir <puerto> <n_predict> — una completion y sus timings.
pedir() {
    local puerto="$1" n="$2" cuerpo resp
    cuerpo="$(printf '{"prompt":%s,"n_predict":%s,"stream":false,"cache_prompt":false}' \
        "$(printf '%s' "$PROMPT" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')" "$n")"
    resp="$(curl -fsS --max-time 600 \
        -H "Authorization: Bearer $API_KEY" \
        -H "Content-Type: application/json" \
        -d "$cuerpo" \
        "http://127.0.0.1:$puerto/v1/completion" 2>/dev/null)"
    [[ -n "$resp" ]] || return 1
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

# arrancar <binario> <puerto> — arranca el server con ropero (PATH al frente).
arrancar() {
    local motor="$1" puerto="$2" dir
    dir="$(dirname "$motor")"
    [[ -x "$motor" ]] || { log "el motor no es ejecutable: $motor"; return 1; }
    PATH="$dir:$PATH" \
    ROPERO_START_TIMEOUT="${ROPERO_START_TIMEOUT:-600}" \
        bash "$ROPERO_ROOT/bin/ropero" "$MODELO" --port "$puerto" --background \
        >>"$OUT/arranque-$(basename "$motor").txt" 2>&1
}

# parar <puerto>
parar() {
    local puerto="$1"
    bash "$ROPERO_ROOT/bin/ropero" stop all >>"$OUT/stop.txt" 2>&1 || true
    # Por si acaso: matar lo que escuche en el puerto (nunca pkill global).
    local pids
    pids="$(lsof -t -iTCP:"$puerto" -sTCP:LISTEN 2>/dev/null || true)"
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
        return 1
    fi
    if ! esperar_listo "$puerto"; then
        log "$etiq: el server no llegó a estar listo"
        parar "$puerto"
        printf 'ERROR\tno estuvo listo\n' >>"$fich"
        return 1
    fi
    log "$etiq: listo"

    for rep in $(seq 1 "$REPETICIONES"); do
        gpu_estado "$etiq-ronda$rep-antes" >>"$OUT/gpu.txt"
        if ! out="$(pedir "$puerto" "$N_PREDICT")"; then
            printf 'ERROR\tla petición falló\n' >>"$fich"
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

    parar "$puerto"
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
