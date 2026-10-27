#!/usr/bin/env bash
# =============================================================================
# probar-bucle.sh — ¿el modelo se enrolla emitiendo la MISMA tool call?
# =============================================================================
# El fallo del fable: emitia la misma tool call 40 veces seguidas hasta que
# opencode abortaba con "MAXIMUM STEPS REACHED". Con el DRY puesto (el
# --dry-multiplier del TOML) deberia salir del bucle.
#
# Esto simula el bucle de opencode SIN opencode: manda la misma peticion con
# una tool, le devuelve un resultado, y repite. Si el modelo vuelve a pedir
# la MISMA tool otra vez (y otra vez), el bucle sigue.
#
# NO hace falta el prompt de 107K: 6 turnos bastan para verlo.
#
# USO:
#   ./probar-bucle.sh [puerto]     (por defecto, el que diga "ropero list")
#
# CODIGO DE SALIDA: 0 = sin bucle, 1 = bucle (o no hay server).
# =============================================================================
set -uo pipefail

# el ayudante python va al lado de este script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PUERTO="${1:-}"
if [[ -z "$PUERTO" ]]; then
    PUERTO="$(ropero list 2>/dev/null | awk '$1=="fable"{for(i=1;i<=NF;i++) if($i ~ /:[0-9]+$/){sub(/.*:/,"",$i); print $i; exit}}')"
fi
PUERTO="${PUERTO:-9999}"
BASE="http://127.0.0.1:${PUERTO}"
TURNOS="${TURNOS:-6}"

# El server va con --api-key, asi que TODO endpoint (tambien /props) pide
# el header Authorization. La key es la misma que usa ropero: la variable
# LLAMA_API_KEY, y si no esta, el default de ropero. Es lo mismo que hace
# el bench (tests/bench_ropero.sh).
API_KEY="${API_KEY:-${LLAMA_API_KEY:-sk-local-dev-key}}"
AUTH=(-H "Authorization: Bearer $API_KEY")

echo "servidor: $BASE"
echo ""

# ── 1. ¿esta el DRY activo? ───────────────────────────────────────────────
# /props devuelve los sampling por defecto del server. Si dry_multiplier es
# 0, el DRY esta apagado y el bucle no tiene por que romperse.
echo "=== 1. el sampler"
tmp="$(mktemp "${TMPDIR:-/tmp}/props.XXXXXX.json")"
code="$(curl -s -o "$tmp" -w '%{http_code}' --max-time 5 "${AUTH[@]}" "$BASE/props" 2>/dev/null)"
case "$code" in
    200) props="$(cat "$tmp")" ;;
    401|403)
        echo "  API KEY INCORRECTA (http $code)."
        echo "  El server va con --api-key; la key debe ser la misma que usa ropero:"
        echo "    LLAMA_API_KEY=$API_KEY"
        rm -f "$tmp"; exit 1 ;;
    000)
        echo "  NO HAY SERVER en $BASE. Arrancalo con: ropero fable"
        rm -f "$tmp"; exit 1 ;;
    *)
        echo "  /props devolvio http $code"
        rm -f "$tmp"; exit 1 ;;
esac
rm -f "$tmp"
printf '%s' "$props" | python3 -c '
import json, sys
d = json.load(sys.stdin)
p = d.get("default_generation_settings", {}).get("params", {})
print("  dry_multiplier   =", p.get("dry_multiplier"))
print("  dry_base         =", p.get("dry_base"))
print("  dry_allowed_len  =", p.get("dry_allowed_length"))
print("  repeat_penalty   =", p.get("repeat_penalty"))
print("  samplers         =", p.get("samplers"))
print("  temp / top_p     =", p.get("temperature"), "/", p.get("top_p"))
if not p.get("dry_multiplier"):
    print()
    print("  !! EL DRY ESTA APAGADO: el bucle no se va a romper solo.")
    print("     Revisa que el TOML del fable tenga --dry-multiplier.")
'
echo ""

# ── 2. el bucle ────────────────────────────────────────────────────────────
# Se le da UNA tool y se le devuelve SIEMPRE el mismo resultado. Un modelo
# sano pide la tool una vez y luego contesta. Uno enrollado la vuelve a
# pedir igual una y otra vez.
echo "=== 2. el bucle de tool calls ($TURNOS turnos)"

TOOLS='{"type":"function","function":{"name":"vault_index","description":"Indexa el vault y devuelve la lista de ficheros","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}}'
MSG_USER='{"role":"user","content":"Antes de escribir codigo, mira que hay en el vault. Usa la tool vault_index."}'

# el historial va en un fichero: es una LISTA de mensajes
hist="$(mktemp "${TMPDIR:-/tmp}/bucle.XXXXXX.json")"
trap 'rm -f "$hist"' EXIT
printf '[%s]' "$MSG_USER" > "$hist"

llamadas=0
misma=0
ultima=""
for i in $(seq 1 "$TURNOS"); do
    cuerpo="$(python3 "$SCRIPT_DIR/probar-bucle.py" body "$hist" "$TOOLS")"
    tmp="$(mktemp "${TMPDIR:-/tmp}/resp.XXXXXX.json")"
    code="$(curl -s -o "$tmp" -w '%{http_code}' --max-time 180 -X POST \
            "$BASE/v1/chat/completions" "${AUTH[@]}" \
            -H 'Content-Type: application/json' -d "$cuerpo" 2>/dev/null)"
    resp="$(cat "$tmp")"
    rm -f "$tmp"
    if [[ "$code" == "401" || "$code" == "403" ]]; then
        echo "  turno $i: API KEY INCORRECTA (http $code)"
        break
    fi
    if [[ "$code" != "200" || -z "$resp" ]]; then
        echo "  turno $i: el server devolvio http $code"
        break
    fi
    tool="$(printf '%s' "$resp" | python3 "$SCRIPT_DIR/probar-bucle.py" tool)"
    if [[ "$tool" == "-" ]]; then
        echo "  turno $i: responde en texto (sin tool) -> el bucle esta roto"
        break
    fi
    llamadas=$((llamadas+1))
    [[ "$tool" == "$ultima" ]] && misma=$((misma+1))
    echo "  turno $i: pide la tool '$tool'"
    ultima="$tool"
    # se inyecta el resultado de la tool, como haria opencode
    printf '%s' "$resp" | python3 "$SCRIPT_DIR/probar-bucle.py" push "$hist"
done

echo ""
if (( misma >= 2 )); then
    echo "RESULTADO: BUCLE. Pidio la misma tool $misma veces seguidas."
    echo "           El DRY no lo esta parando: hay que mirarlo mas a fondo."
    exit 1
fi
echo "RESULTADO: sin bucle (la misma tool seguida: $misma veces, llamadas: $llamadas)."
exit 0
