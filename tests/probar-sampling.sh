#!/usr/bin/env bash
# =============================================================================
# probar-sampling.sh — ¿que sampling rompe el bucle de tool calls?
# =============================================================================
# El fable (y cualquier modelo muy cuantizado) se enrolla: pide la MISMA
# tool call una y otra vez, y opencode aborta con "MAXIMUM STEPS REACHED".
#
# Esto prueba VARIAS configuraciones de sampling, cada una en su propio
# server, y dice cuales rompen el bucle y a que velocidad. Para que dejar
# de discutir Sampling y empezar a medirlo.
#
# No es el prompt real de 107K: el bucle se reproduce con un prompt corto
# (el modelo pide la tool, recibe un resultado, y la vuelve a pedir). El ctx
# se deja pequeno para que arrancar cada variante sea rapido.
#
# USO:
#   ./probar-sampling.sh [modelo]        (por defecto, fable)
#   ./probar-sampling.sh fable 3          (solo las 3 primeras variantes)
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODELO="${1:-fable}"
LIMITE="${2:-99}"
KEY="${API_KEY:-${LLAMA_API_KEY:-sk-local-dev-key}}"
TURNOS="${TURNOS:-6}"
CTX="${CTX:-4096}"

GGUF="$(ropero path "$MODELO" 2>/dev/null)"
if [[ -z "$GGUF" || ! -f "$GGUF" ]]; then
    echo "ERROR: no se encuentra el GGUF de '$MODELO'." >&2
    echo "       'ropero path $MODELO' dice: ${GGUF:-<nada>}" >&2
    exit 1
fi

LLAMA_SERVER="${LLAMA_BIN:-}"
if [[ -z "$LLAMA_SERVER" ]]; then
    for c in "$HOME/llama.cpp/build/bin/llama-server" \
             "$HOME/llama.cpp/build/bin/llama-server.exe" \
             "$(command -v llama-server 2>/dev/null || true)"; do
        [[ -n "$c" && -x "$c" ]] && { LLAMA_SERVER="$c"; break; }
    done
fi
if [[ -z "$LLAMA_SERVER" ]]; then
    echo "ERROR: no se encuentra llama-server." >&2
    echo "       ponlo en LLAMA_BIN, o en ~/llama.cpp/build/bin/" >&2
    exit 1
fi

# El argv base del modelo: lo da el propio ropero, asi que el test usa
# EXACTAMENTE la config de produccion (mismos flags de GPU, de KV, de
# batching) y solo cambia el sampling.
BASE_ARGS=()
while IFS= read -r a; do
    BASE_ARGS+=("$a")
done < <(ropero args "$MODELO" 2>/dev/null)
if (( ${#BASE_ARGS[@]} == 0 )); then
    echo "ERROR: 'ropero args $MODELO' no devuelve nada." >&2
    exit 1
fi

# ── las variantes ──────────────────────────────────────────────────────────
# Cada una es una lista de flags de sampling. Se prueban de menos a mas
# agresivo.
#
# La clave de por que el DRY solo no basta (medido, no supuesto):
#   - el DRY se RINDE si rep_limit < dry_allowed_length (llama-sampler.cpp),
#     y los sequence breakers ("", /, :) cortan la repeticion justo antes
#     del token repetido. Con allowed_length=5 y un breaker a 2 tokens del
#     final, no penaliza nada.
#   - el sampler de PENALTIES no esta en la cadena si --samplers no lo
#     incluye, y ademas is_disabled() lo apaga con repeat=1.0 y freq/pres=0.
VARIANTES=(
    "sin nada|"
    "dry allowed_length=3|--dry-multiplier 0.8 --dry-base 1.75 --dry-allowed-length 3 --dry-penalty-last-n 1024 --dry-sequence-breaker [\"\\n\",\"\`\`\`\\n\",\":\",\"\\t\",\"\\\"\",\"|\",\"-\",\"\\}\",\"\\]\",\"/\",\"\\\\\"] --samplers min_p;top_k;top_p;temp;dry"
    "dry allowed_length=2|--dry-multiplier 0.8 --dry-base 1.75 --dry-allowed-length 2 --dry-penalty-last-n 1024 --dry-sequence-breaker [\"\\n\",\"\`\`\`\\n\",\":\",\"\\t\",\"\\\"\",\"|\",\"-\",\"\\}\",\"\\]\",\"/\",\"\\\\\"] --samplers min_p;top_k;top_p;temp;dry"
    "penalties 1.1|--repeat-penalty 1.1 --repeat-last-n 128 --samplers penalties;min_p;top_k;top_p;temp;dry"
    "penalties 1.1 + dry 3|--repeat-penalty 1.1 --repeat-last-n 128 --dry-multiplier 0.8 --dry-base 1.75 --dry-allowed-length 3 --dry-penalty-last-n 1024 --dry-sequence-breaker [\"\\n\",\"\`\`\`\\n\",\":\",\"\\t\",\"\\\"\",\"|\",\"-\",\"\\}\",\"\\]\",\"/\",\"\\\\\"] --samplers penalties;min_p;top_k;top_p;temp;dry"
    "freq 0.3|--repeat-penalty 1.1 --repeat-last-n 128 --frequency-penalty 0.3 --samplers penalties;min_p;top_k;top_p;temp;dry"
    "freq 0.6 + pres 0.2|--repeat-penalty 1.1 --repeat-last-n 128 --frequency-penalty 0.6 --presence-penalty 0.2 --samplers penalties;min_p;top_k;top_p;temp;dry"
    "temp 0.8|--temp 0.8 --top-k 40 --top-p 0.95 --min-p 0.05 --samplers min_p;top_k;top_p;temp;dry"
    "temp 1.0|--temp 1.0 --top-k 60 --top-p 0.95 --min-p 0.05 --samplers min_p;top_k;top_p;temp;dry"
    "freq 0.3 + dry 3|--repeat-penalty 1.1 --repeat-last-n 128 --frequency-penalty 0.3 --dry-multiplier 0.8 --dry-base 1.75 --dry-allowed-length 3 --dry-penalty-last-n 1024 --dry-sequence-breaker [\"\\n\",\"\`\`\`\\n\",\":\",\"\\t\",\"\\\"\",\"|\",\"-\",\"\\}\",\"\\]\",\"/\",\"\\\\\"] --samplers penalties;min_p;top_k;top_p;temp;dry"
)

# ── helpers ────────────────────────────────────────────────────────────────
puerto_libre() {
    python3 -c 'import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()'
}

# Los flags de sampling del base se quitan: los de la variante los pisan.
# Todos ellos llevan valor, asi que se saltar el flag Y el siguiente.
filtrar_base() {
    local -a out=()
    local saltar=0 a
    for a in "${BASE_ARGS[@]}"; do
        if (( saltar == 1 )); then
            saltar=0
            continue
        fi
        case "$a" in
            --temp|--top-k|--top-p|--min-p|--repeat-penalty|--repeat-last-n|\
            --dry-multiplier|--dry-base|--dry-allowed-length|--dry-penalty-last-n|\
            --dry-sequence-breaker|--samplers|--frequency-penalty|--presence-penalty|\
            --xtc-probability|--seed|--n-predict|--keep|--load-mode|--metrics|--perf|--poll)
                saltar=1
                continue ;;
        esac
        out+=("$a")
    done
    printf '%s\n' "${out[@]}"
}

# ── el bucle ───────────────────────────────────────────────────────────────
medir_bucle() {
    local url="$1" turnos="$2"
    python3 "$SCRIPT_DIR/probar-bucle.py" bucle "$url" "$turnos" "$KEY" 2>&1
}

echo "modelo:   $MODELO"
echo "gguf:     $GGUF"
echo "server:   $LLAMA_SERVER"
echo "ctx:      $CTX    (pequeno a proposito: el bucle no depende del ctx)"
echo "turnos:   $TURNOS"
echo ""

# ── el bucle ───────────────────────────────────────────────────────────────
n=0
declare -a NOMBRES=() ROMPE=() TPS=()
for v in "${VARIANTES[@]}"; do
    n=$((n+1))
    (( n > LIMITE )) && break
    nombre="${v%%|*}"
    flags="${v#*|}"

    PORT="$(puerto_libre)"
    URL="http://127.0.0.1:$PORT/v1/chat/completions"

    echo "──────────────────────────────────────────────────────────────────────"
    echo "[$n] $nombre"

    # ── el server con esta variante ──
    mapfile -t base_filtrado < <(filtrar_base)
    cmd=("$LLAMA_SERVER" --model "$GGUF" --host 127.0.0.1 --port "$PORT"
         --api-key "$KEY" --alias "$MODELO" --ctx-size "$CTX"
         --parallel 1)
    # los flags del modelo, y luego los de la variante (al final, para que
    # ganen: el server usa el ultimo valor de cada flag)
    cmd+=("${base_filtrado[@]}")
    if [[ -n "$flags" ]]; then
        # shellcheck disable=SC2206
        cmd+=($flags)
    fi

    "${cmd[@]}" >"/tmp/probar-sampling-$PORT.log" 2>&1 &
    SRV_PID=$!

    # esperar a que responda
    listo=0
    for _ in $(seq 1 90); do
        if curl -sf --max-time 2 -H "Authorization: Bearer $KEY" \
                "http://127.0.0.1:$PORT/v1/models" >/dev/null 2>&1; then
            listo=1; break
        fi
        sleep 1
    done
    if (( listo == 0 )); then
        echo "  (el server no arranco; ver /tmp/probar-sampling-$PORT.log)"
        kill "$SRV_PID" 2>/dev/null
        wait "$SRV_PID" 2>/dev/null
        continue
    fi

    # ── medir ──
    salida="$(medir_bucle "$URL" "$TURNOS")"
    resumen="$(printf '%s\n' "$salida" | tail -1)"
    # las lineas de turno (todo menos la ultima, que es el resumen JSON)
    printf '%s\n' "$salida" | head -n -1 | sed 's/^/  /' 

    romp="?"
    tps="?"
    if [[ "$resumen" == \{*\} ]]; then
        romp="$(printf '%s' "$resumen" | python3 -c 'import json,sys; d=json.load(sys.stdin); print("ROMPO" if d["rompe"] else "SIGUE")')"
        tps="$(printf '%s' "$resumen" | python3 -c 'import json,sys; print(json.load(sys.stdin)["tps"])')"
    fi
    echo "  => bucle: $romp    t/s: $tps"
    NOMBRES+=("$nombre"); ROMPE+=("$romp"); TPS+=("$tps")

    kill "$SRV_PID" 2>/dev/null
    wait "$SRV_PID" 2>/dev/null
done

# ── la tabla ───────────────────────────────────────────────────────────────
echo ""
echo "══════════════════════════════════════════════════════════════════════"
printf '%-28s %-10s %s\n' "VARIANTE" "BUCLE" "T/S"
printf '%-28s %-10s %s\n' "----------------------------" "----------" "----------"
i=0
while (( i < ${#NOMBRES[@]} )); do
    printf '%-28s %-10s %s\n' "${NOMBRES[$i]}" "${ROMPE[$i]}" "${TPS[$i]}"
    i=$((i+1))
done
echo ""
echo "ROMPO = el modelo deja de repetir la tool call (configuracion buena)."
echo "SIGUE = el bucle continua (configuracion que no sirve)."
