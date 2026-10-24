#!/usr/bin/env bash
# =============================================================================
# bench_ropero.sh — bateria de pruebas de los modelos locales de ropero
# =============================================================================
#
# Por cada carpeta de modelo:
#   1. arranca "ropero <modelo>" (salida en <resultados>/<modelo>/ropero.log)
#   2. espera a que el modelo responda en el puerto de los providers
#   3. por cada prompt: "opencode run" y la sesion exportada
#   4. para el modelo con "ropero stop <modelo>"
#
# QUE DEJA CADA MODELO (en <OUT_DIR>/<modelo>/):
#   ropero.log               el log del servidor
#   <prueba>/                la carpeta de trabajo de la sesion
#   opencode-<prueba>.json   los eventos del run (JSON)
#   opencode-<prueba>.log    su stderr
#   sesion-<prueba>.md       la sesion exportada
#   resumen.tsv              una linea por prueba
#
# Y EN LA RAIZ (este directorio):
#   resumen-global.tsv       una linea por (modelo, prueba)
#   juez/<modelo>/           el paquete para la IA juez: el prompt 3 junto
#                            con las transcripciones de las pruebas
#
# SEPARADO DONDE CORRE Y DONDE DEPOSITA:
#   · MODELOS_DIR  (por defecto ~/test)  -> las carpetas de los modelos, con
#     la configuracion de opencode de cada una. Ahi se ejecuta opencode run.
#   · OUT_DIR      (por defecto <este directorio>/resultados) -> donde quedan
#     los resultados. Todo junto, para comparar entre modelos. Va en .gitignore.
#
# USO:
#   ./bench_ropero.sh                # todos los modelos, pruebas 1 y 2
#   ./bench_ropero.sh gptoss qwen    # solo esos
#   PROMPTS="pruebas1.md" ./bench_ropero.sh gptoss     # solo la prueba 1
#   FORCE=1 ./bench_ropero.sh gptoss  # repetir aunque ya este probado
#
# El prompt 3 NO se le pasa al modelo evaluado: es el que va a la IA juez
# con las transcripciones. El script deja el paquete listo en juez/.
# =============================================================================

set -uo pipefail

AQUI="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# ── AJUSTES ────────────────────────────────────────────────────────────────
MODELOS_DIR="${MODELOS_DIR:-$HOME/test}"
OUT_DIR="${OUT_DIR:-$AQUI/resultados}"
PROMPTS_DIR="${PROMPTS_DIR:-$AQUI}"
PROMPTS=("pruebas1.md" "pruebas2.md")       # las que ve el modelo evaluado
PROMPT_JUEZ="pruebas3.md"                   # el que ve la IA juez
MODELO="${MODELO:-marichocho/Puertourraco}" # provider/model de opencode
AGENTE="${AGENTE:-build}"
PUERTO="${PUERTO:-9999}"                     # donde apuntan los providers
API_KEY="${API_KEY:-sk-local-dev-key}"
ESPERA="${ESPERA:-900}"                      # segundos esperando al modelo
TIMEOUT="${TIMEOUT:-3600}"                   # segundos maximos por sesion
FORCE="${FORCE:-0}"                          # 1 = repetir los ya probados

# Si el nombre de la carpeta no es el del modelo en ropero, se mapea aqui.
declare -A MODELO_ROPERO=(
  [agentas1]=agenta1
  [codernext]=qwen3-coder-next
)

# ── COLORES ────────────────────────────────────────────────────────────────
if [[ -t 1 ]]; then R=$'\033[31m'; G=$'\033[32m'; Y=$'\033[33m'; B=$'\033[1m'; D=$'\033[2m'; N=$'\033[0m'
else R=""; G=""; Y=""; B=""; D=""; N=""; fi
info() { printf '%s%s%s\n' "$D" "$*" "$N"; }
warn() { printf '%s[!] %s%s\n' "$Y" "$*" "$N" >&2; }
err()  { printf '%s[x] %s%s\n' "$R" "$*" "$N" >&2; }
ok()   { printf '%s[+] %s%s\n' "$G" "$*" "$N"; }
head2(){ printf '\n%s══ %s %s\n' "$B" "$*" "$N"; }

# ── HELPERS ────────────────────────────────────────────────────────────────
responde() {
  curl -sf --max-time 3 -H "Authorization: Bearer $API_KEY" \
    "http://127.0.0.1:$PUERTO/v1/models" >/dev/null 2>&1
}

esperar_modelo() {
  local deadline=$(( $(date +%s) + ESPERA ))
  while (( $(date +%s) < deadline )); do
    responde && return 0
    sleep 3
  done
  return 1
}

# id del modelo que sirve el puerto
modelo_en_puerto() {
  curl -sf --max-time 5 -H "Authorization: Bearer $API_KEY" \
    "http://127.0.0.1:$PUERTO/v1/models" 2>/dev/null \
    | grep -oE '"id"[[:space:]]*:[[:space:]]*"[^"]+"' | head -1 \
    | grep -oE '"[^"]+"$' | tr -d '"'
}

# id de la sesion en los eventos del run
id_sesion() {
  grep -oiE '"session_?id"[[:space:]]*:[[:space:]]*"[^"]+"' "$1" 2>/dev/null | head -1 \
    | grep -oE '"[^"]+"$' | tr -d '"'
}

# tokens/s de GENERACION. El log tiene dos timings: "prompt eval time" y
# "eval time". El que sirve es el segundo: hay que quitar el primero.
tokens_por_segundo() {
  grep -a "eval time" "$1" 2>/dev/null | grep -av "prompt eval" | tail -1 \
    | grep -oE '[0-9]+(\.[0-9]+)? tokens per second' | tail -1 | grep -oE '^[0-9.]+'
}

# ms por token (para comparar con el techo teorico)
ms_por_token() {
  grep -a "eval time" "$1" 2>/dev/null | grep -av "prompt eval" | tail -1 \
    | grep -oE '[0-9]+(\.[0-9]+)? ms per token' | tail -1 | grep -oE '^[0-9.]+'
}

# ── PRECHECK ───────────────────────────────────────────────────────────────
for b in ropero opencode curl; do
  command -v "$b" >/dev/null 2>&1 || { err "falta '$b' en el PATH"; exit 1; }
done
for p in "${PROMPTS[@]}"; do
  [[ -f "$PROMPTS_DIR/$p" ]] || { err "no encuentro el prompt $PROMPTS_DIR/$p"; exit 1; }
done
[[ -d "$MODELOS_DIR" ]] || { err "no existe el directorio de modelos $MODELOS_DIR"; exit 1; }
mkdir -p "$OUT_DIR" "$OUT_DIR/juez" || { err "no puedo escribir en $OUT_DIR"; exit 1; }

# ── CERROJO ───────────────────────────────────────────────────────────────
# Dos corridas a la vez se pisan los ficheros y el resumen. Va en /tmp para no
# dejar nada en la raiz.
LOCK="${TMPDIR:-/tmp}/.bench_ropero-$(id -u).lock"
if command -v flock >/dev/null 2>&1; then
  exec 9>"$LOCK"
  flock -n 9 || { err "ya hay otra corrida de bench en marcha"; exit 1; }
fi

# ── CARROJERAS ─────────────────────────────────────────────────────────────
carpetas=()
for d in "$MODELOS_DIR"/*/; do
  [[ -d "$d" ]] || continue
  c="$(basename "$d")"
  if [[ $# -gt 0 ]]; then
    keep=0
    for o in "$@"; do [[ "$c" == "$o" ]] && keep=1; done
    (( keep )) || continue
  fi
  carpetas+=("$c")
done

if [[ ${#carpetas[@]} -eq 0 ]]; then
  err "no hay carpetas que probar en $MODELOS_DIR"
  exit 1
fi

GLOBAL="$OUT_DIR/resumen-global.tsv"
printf 'modelo\tprueba\trc\tduracion_s\tsesion_id\tmodelo_servido\ttokens_s\tms_por_token\n' > "$GLOBAL"

# ── UNA PRUEBA POR CARPETA ─────────────────────────────────────────────────
n=0
for carpeta in "${carpetas[@]}"; do
  n=$((n+1))
  dir="$MODELOS_DIR/$carpeta"          # donde vive el modelo (y su config)
  out="$OUT_DIR/$carpeta"               # donde se depositan los resultados
  modelo="${MODELO_ROPERO[$carpeta]:-$carpeta}"
  mkdir -p "$out"

  head2 "[$n/${#carpetas[@]}] $carpeta  (ropero: $modelo)"
  [[ -d "$dir" ]] || { err "no existe la carpeta del modelo $dir"; continue; }

  # ── idempotente: si este modelo ya esta probado, no se repite ───────────
  # La marca se pone AL TERMINAR el modelo. Si la corrida se corto, no hay
  # marca y el modelo se vuelve a hacer (que es lo que quieres).
  if [[ -f "$out/.bench-hecho" && "$FORCE" != "1" ]]; then
    ok "ya estaba probado — me lo salto (borra $out/.bench-hecho para repetirlo)"
    continue
  fi
  rm -f "$out/.bench-hecho"

  # ── ropero ───────────────────────────────────────────────────────────────
  # En segundo plano y con la salida aqui: en primer plano ropero se queda
  # sirviendo el puerto, que es lo que queremos. Y el log del servidor queda
  # en el directorio de resultados.
  ( cd "$dir" && nohup ropero "$modelo" > "$out/ropero.log" 2>&1 & )
  sleep 1

  if grep -q "ya está corriendo" "$out/ropero.log" 2>/dev/null; then
    ok "ya estaba corriendo: lo uso y no lo paro al terminar"
  elif ! esperar_modelo; then
    err "el modelo no respondió en :$PUERTO tras ${ESPERA}s"
    tail -n 15 "$out/ropero.log" | sed 's/^/    /'
    continue
  else
    ok "servido como '$(modelo_en_puerto)' en :$PUERTO"
  fi

  # ── las pruebas ──────────────────────────────────────────────────────────
  printf 'prueba\trc\tduracion_s\tsesion_id\tmodelo_servido\ttokens_s\tms_por_token\n' > "$out/resumen.tsv"
  for p in "${PROMPTS[@]}"; do
    prueba="${p%.md}"
    prompt="$(cat "$PROMPTS_DIR/$p")"
    work="$dir/$prueba"                   # el trabajo ocurre junto al modelo
    rm -rf "$work"; mkdir -p "$work"

    info "  $p"
    t0=$(date +%s)
    ( cd "$work" && timeout "$TIMEOUT" opencode run "$prompt" \
        --model "$MODELO" --agent "$AGENTE" --format json ) \
      > "$out/opencode-$prueba.json" 2> "$out/opencode-$prueba.log"
    rc=$?
    dur=$(( $(date +%s) - t0 ))

    # la sesion, exportada
    sid="$(id_sesion "$out/opencode-$prueba.json")"
    if [[ -n "$sid" ]]; then
      if opencode session export "$sid" > "$out/sesion-$prueba.md" 2>> "$out/opencode-$prueba.log"; then
        ok "  ${dur}s — sesión $sid exportada"
      else
        warn "  la exportación falló (queda el json crudo)"
      fi
    else
      warn "  no encontré el id de sesión (queda el json crudo)"
    fi

    if [[ $rc -ne 0 ]]; then
      err "  opencode rc=$rc"
      tail -n 10 "$out/opencode-$prueba.log" | sed 's/^/    /'
    fi

    ts="$(tokens_por_segundo "$out/ropero.log")"; ts="${ts:-?}"
    ms="$(ms_por_token "$out/ropero.log")";     ms="${ms:-?}"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$p" "$rc" "$dur" "${sid:--}" "$(modelo_en_puerto)" "$ts" "$ms" \
      >> "$out/resumen.tsv"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$carpeta" "$p" "$rc" "$dur" "${sid:--}" "$(modelo_en_puerto)" "$ts" "$ms" \
      >> "$GLOBAL"
  done

  # ── paquete para la IA juez ──────────────────────────────────────────────
  # El prompt 3 va a otra IA (Claude, DeepSeek, MiniMax) con las
  # transcripciones. Se deja el paquete montado, no se lanza: cada juez tiene
  # su propio provider.
  if [[ -f "$PROMPTS_DIR/$PROMPT_JUEZ" ]]; then
    mkdir -p "$OUT_DIR/juez/$carpeta"
    cp "$PROMPTS_DIR/$PROMPT_JUEZ" "$OUT_DIR/juez/$carpeta/$PROMPT_JUEZ"
    for p in "${PROMPTS[@]}"; do
      prueba="${p%.md}"
      [[ -f "$out/sesion-$prueba.md" ]] && \
        cp "$out/sesion-$prueba.md" "$OUT_DIR/juez/$carpeta/sesion-$prueba.md"
    done
    # los ficheros que el modelo haya generado en la carpeta de trabajo
    for w in "$dir"/*/; do
      [[ -d "$w" ]] || continue
      nombre="$(basename "$w")"
      [[ "$nombre" == "sesion-*" ]] && continue
      if [[ -d "$w/.git" || -d "$w/node_modules" ]]; then continue; fi
      mkdir -p "$OUT_DIR/juez/$carpeta/ficheros/$nombre"
      find "$w" -maxdepth 2 -type f \
        ! -path '*/.git/*' ! -path '*/node_modules/*' \
        -exec cp --parents {} "$OUT_DIR/juez/$carpeta/ficheros/" \; 2>/dev/null || true
    done
    ok "  paquete del juez montado en $OUT_DIR/juez/$carpeta"
  fi

  # ── parar el modelo ──────────────────────────────────────────────────────
  ropero stop "$modelo" 2>&1 | sed 's/^/  /'

  # marca: este modelo ya esta. La proxima corrida se lo salta.
  date -Is > "$out/.bench-hecho"
done

head2 "resumen global"
cat "$GLOBAL"
head2 "paquetes del juez"
ls -1 "$OUT_DIR/juez" 2>/dev/null | sed 's/^/  /' || true
