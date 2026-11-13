#!/usr/bin/env bash
# =============================================================================
# server/strata-run.sh — el proceso backend del engine "strata"
# =============================================================================
#
# POR QUÉ ESTE SCRIPT Y NO UN `strata launch` A SECO
#
# `strata launch --no-tui` hace lo que tiene que hacer (baja el modelo si hace
# falta, arranca el motor y su server) y DEVUELVE cuando el server ya está
# sirviendo. El proceso que queda vivo es el motor, y de ese proceso se
# ocupa Strata (profile, run config, state, presets).
#
# El backend de endpoint.py necesita un proceso que (a) no muera mientras el
# modelo sirve y (b) muera cuando hay que pararlo. Con `strata launch` a
# secas, el proceso que devuelve el launch se va enseguida: el proxy lo daría
# por caído ("el backend strata terminó") y el modelo seguiría ocupcando la
# VRAM sin que nadie lo supervise.
#
# Este script hace de puente:
#
#   1. lanza Strata y ESPERA a que termine (la descarga puede ser de 70 GB);
#   2. se queda vivo mientras el modelo sirve;
#   3. al recibir SIGTERM/SIGINT, para LO QUE ARRANCÓ y sale.
#
# POR QUÉ UN PRESET PROPIO
#
# `strata launch --stop` (sin más) para TODOS los launches que haya
# corriendo: si el usuario tiene otro Strata abierto (el suyo, con su
# modelo), pararlo sería una grosería. Por eso el launch se guarda con
# `--save-preset <nombre>` y al parar se para ESE preset y sólo ese:
#
#     strata launch --preset <nombre> --stop
#
# USO
#   strata-run.sh <strata> <familia> <modelo> <puerto> <contexto> <preset> [flags...]
#
#   <strata>    ruta al ejecutable (la resuelve el engine).
#   <familia>   --family de Strata (qwen, qwen36, gyro, coder, swift...).
#   <modelo>    --model de Strata (q2_0, UD-Q4_K_XL, Gyro-S, IQ1_M...).
#   <puerto>    puerto del server INTERNO (el proxy habla con él aquí).
#   <contexto>  contexto en tokens (0 = el que elija Strata).
#   <preset>    nombre del preset con el que se lanza (para poder pararlo).
#   [flags...]  flags extra de `strata launch` (los del [args] del TOML).
#
# bash 3.2 (macOS, ADR-1): sin namerefs ni arrays asociativos.
# =============================================================================
set -uo pipefail

STRATA="${1:-}"; FAMILY="${2:-}"; MODEL="${3:-}"; PORT="${4:-}"; CTX="${5:-0}"; PRESET="${6:-}"
if [[ $# -ge 6 ]]; then shift 6; else shift $#; fi

# Los flags extra se GUARDAN ANTES de reconstruir los posicionales: `set --`
# los machaca, y aquí es donde se pierden.
EXTRAS=()
for e in "$@"; do
    EXTRAS+=("$e")
done

if [[ -z "$STRATA" || ! -x "$STRATA" ]]; then
    printf 'strata-run: no encuentro el ejecutable de Strata (%s)\n' "${STRATA:-vacío}" >&2
    exit 2
fi
if [[ -z "$FAMILY" || -z "$MODEL" || -z "$PORT" ]]; then
    printf 'strata-run: faltan familia, modelo o puerto\n' >&2
    exit 2
fi

# _slug <nombre> — el mismo nombre de perfil que usa Strata (settings.py:
# minúsculas, todo lo que no sea [a-z0-9_-] pasa a "-", sin repetir guiones).
# El perfil de un preset se llama "launch-<slug>": hay que saber ese nombre
# exacto para preguntar por él en `strata launch --status`.
_slug() {
    local s
    s="$(printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]' | sed -e 's/[^a-z0-9_-]\{1,\}/-/g' -e 's/-\{2,\}/-/g' -e 's/^[-_]*//' -e 's/[-_]*$//')"
    [[ -z "$s" ]] && s="launch"
    case "$s" in
        [a-z]*) : ;;
        *) s="l$s" ;;
    esac
    printf '%s' "${s:0:48}"
}
PERFIL="launch-$(_slug "$PRESET")"

# ── Parar lo que arrancamos ────────────────────────────────────────────────
PARADO=0
parar() {
    [[ "$PARADO" == "1" ]] && return 0
    PARADO=1
    printf '\nstrata-run: parando el launch "%s"...\n' "$PERFIL" >&2
    "$STRATA" launch --preset "$PRESET" --stop >&2 2>&1 || true
}

trap 'parar; exit 0' TERM
trap 'parar; exit 130' INT

# ── 1. Lanzar ──────────────────────────────────────────────────────────────
#
# --access local: sólo esta máquina (el proxy es el que expone el puerto).
# --no-api-key: la clave la pone el proxy de ropero; el server de Strata
#                escucha en 127.0.0.1 y no hace falta que la compruebe.
# --no-tui: sin pantallas (aquí no hay terminal que las dibuje).
# --yes: no preguntar nada (si el modelo no cabe, avisa y sigue).
CMD=(launch
    --family "$FAMILY"
    --model "$MODEL"
    --port "$PORT"
    --access local
    --no-api-key
    --no-tui
    --yes
    --save-preset "$PRESET")
if [[ -n "${CTX:-}" && "${CTX:-0}" != "0" ]]; then
    CMD+=(--context "$CTX")
fi
# Los flags que da el TOML del modelo (los de [args].base). VAN AL FINAL a
# propósito: en argparse gana el último, y así el contexto de ropero (CTX)
# puede ir delante sin que el TOML lo pise... salvo que el TOML traiga otro
# --context, que entonces sí gana (es lo último).
for extra in ${EXTRAS[@]+"${EXTRAS[@]}"}; do
    CMD+=("$extra")
done

printf 'strata-run: %s %s\n' "$STRATA" "${CMD[*]}" >&2
"$STRATA" "${CMD[@]}" >&2
rc=$?
if [[ "$rc" != "0" ]]; then
    printf 'strata-run: el launch de Strata falló (rc=%s)\n' "$rc" >&2
    exit "$rc"
fi

printf 'strata-run: Strata sirve (perfil "%s", puerto %s)\n' "$PERFIL" "$PORT" >&2

# ── 2. Vivir mientras el modelo sirve ──────────────────────────────────────
#
# El trap NO se ejecuta mientras duerme un comando: bash lo APLAZA hasta que
# el comando acaba (medido). Por eso el sleep es de 1 s y no de 1 h: con un
# sleep largo, el SIGTERM del monitor o el Ctrl-C se comerían la espera.
ticks=0
while :; do
    sleep 1
    ticks=$(( ticks + 1 ))
    # Cada 15 s se comprueba que el modelo sigue en pie. Si Strata lo paró
    # (o murió el motor), el wrapper sale con error para que ropero limpie el
    # estado en vez de quedarse con un "modelo" que no existe.
    if (( ticks % 15 == 0 )); then
        estado="$("$STRATA" launch --status --output json 2>/dev/null || true)"
        if [[ -n "$estado" ]] && ! printf '%s' "$estado" | grep -q "\"$PERFIL\""; then
            printf 'strata-run: el launch "%s" ya no está corriendo\n' "$PERFIL" >&2
            exit 1
        fi
    fi
done
