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
TIMEOUT="${TIMEOUT:-900}"                    # segundos maximos por sesion.
# 3600 era una hora y pico colgada sin decir nada; 900 avisa antes y deja
# margen para las tres pruebas de un modelo.
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

# ── VERIFICAR QUE EL MODELO ESTA ENTERO EN LA GPU ───────────────────────────
#
# El fitter de llama.cpp recorta SILENCIOSAMENTE: si el modelo + la KV + el
# compute no caben en la VRAM, quita capas y se las deja en la CPU. El
# servidor arranca igual, responde igual... y va a 9 t/s en vez de 30. Sin
# este check, el bench mide una configuracion rota y no lo dice.
#
# Las dos señales en el log:
#   · "cannot meet free memory target"  -> el fitter no pudo cumplir
#   · "offloaded X/Y layers"            -> X < Y = hay capas en la CPU
gpu_completo() {
    local log="$1" off="" mal=0 motivo=""

    # "cannot meet free memory target" solo es un FALLO si de verdad se
    # quedaron capas en la CPU. Con --n-gpu-layers fijo el fitter lo intenta
    # y no puede ajustar, pero el modelo entra entero igualmente: eso es un
    # aviso, no un fallo.
    local aviso=""
    if grep -qa "cannot meet free memory target" "$log" 2>/dev/null; then
        aviso="el fitter no pudo ajustar a la VRAM ($(grep -a -m1 'cannot meet free memory target' "$log" | sed 's/.*: //'))"
    fi

    off=$(grep -aoE "offloaded [0-9]+/[0-9]+ layers" "$log" 2>/dev/null | tail -1)
    if [[ -n "$off" ]]; then
        local x y
        x=$(echo "$off" | sed -E 's|offloaded ([0-9]+)/([0-9]+) layers|\1|')
        y=$(echo "$off" | sed -E 's|offloaded ([0-9]+)/([0-9]+) layers|\2|')
        if [[ "$x" -lt "$y" ]]; then
            mal=1
            motivo="solo $x de $y capas en la GPU (el resto en la CPU)"
            aviso=""
        else
            GPU_AVISO="modelo entero en la GPU ($x/$y capas)"
            [[ -n "$aviso" ]] && GPU_AVISO="$GPU_AVISO; $aviso"
            return 0
        fi
    fi

    # sin "offloaded X/Y" no se puede asegurar: si el fitter no pudo, puede
    # que se quedaran capas en la CPU sin que el log lo diga claro.
    if [[ -n "$aviso" && $mal -eq 0 ]]; then
        GPU_AVISO="$aviso (no se pudo comprobar el reparto de capas)"
        return 0
    fi

    if (( mal )); then
        GPU_AVISO="$motivo"
        return 1
    fi
    GPU_AVISO="modelo entero en la GPU"
    return 0
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
GLOBAL_PREV="$OUT_DIR/.resumen-global.prev.tsv"
# No se trunca: si ya existe, se conserva y al final se fusiona con lo nuevo
# (sin duplicar las filas de los modelos repetidos).
if [[ -f "$GLOBAL" ]]; then
    cp "$GLOBAL" "$GLOBAL_PREV"
    printf 'modelo\tprueba\trc\tduracion_s\tsesion_id\tmodelo_servido\ttokens_s\tms_por_token\tgpu\n' > "$GLOBAL"
else
    printf 'modelo\tprueba\trc\tduracion_s\tsesion_id\tmodelo_servido\ttokens_s\tms_por_token\tgpu\n' > "$GLOBAL"
fi

# ── LIMPIAR EL ESTADO ENTRE PRUEBAS ─────────────────────────────────────────
# Sin esto, la segunda prueba de un modelo arranca sobre lo que dejó la
# primera: el contexto del prompt anterior sigue en el servidor, la VRAM
# sigue ocupada y el opencode arrastra sesiones del run anterior.
limpiar_procesos() {
    local modelo="$1"

    # 1. el opencode que haya quedado vivo (el timeout mata el proceso, pero
    #    no siempre a los hijos ni al daemon de sesiones)
    pkill -f 'opencode run' 2>/dev/null || true
    pkill -f 'opencode.*--format json' 2>/dev/null || true
    sleep 0.3

    # 2. el servidor de llama.cpp: se para y se relanza limpio
    ropero stop "$modelo" >/dev/null 2>&1 || true
    sleep 1
}

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
  # ¿ya están TODAS las pruebas de este modelo? entonces se salta entero.
  # Si solo están algunas, se hacen las que faltan: una corrida interrumpida
  # no tira el trabajo hecho.
  if [[ "$FORCE" != "1" && -f "$out/resumen.tsv" ]]; then
    faltan=0
    for p in "${PROMPTS[@]}"; do
      pr="${p%.md}"
      grep -q "^$pr"$'\t' "$out/resumen.tsv" 2>/dev/null || faltan=$((faltan+1))
    done
    if (( faltan == 0 )); then
      ok "ya estaba probado entero — me lo salto (FORCE=1 para repetir)"
      continue
    elif (( faltan < ${#PROMPTS[@]} )); then
      ok "faltan $faltan prueba(s) — solo hago esas"
    fi
  fi
  rm -f "$out/.bench-hecho"

  # El arranque del server va DENTRO del bucle de pruebas: cada prueba
  # arranca con el server limpio (sin el contexto de la anterior), y su
  # log se guarda aparte -> los tokens/s de cada prueba son los suyos.

  # ── las pruebas ──────────────────────────────────────────────────────────
  # El resumen del modelo NO se trunca: si ya tiene filas de una prueba, esa
  # prueba se salta. Asi una corrida interrumpida no tira el trabajo hecho.
  if [[ ! -f "$out/resumen.tsv" ]]; then
    printf 'prueba\trc\tduracion_s\tsesion_id\tmodelo_servido\ttokens_s\tms_por_token\tgpu\n' > "$out/resumen.tsv"
  fi
  for p in "${PROMPTS[@]}"; do
    prueba="${p%.md}"
    prompt="$(cat "$PROMPTS_DIR/$p")"
    work="$dir/$prueba"                   # el trabajo ocurre junto al modelo

    # ya está hecha esta prueba para este modelo? -> no se repite
    if [[ -f "$out/resumen.tsv" ]] && grep -q "^$prueba"$'\t' "$out/resumen.tsv" 2>/dev/null; then
      ok "  $p ya estaba hecha — me la salto"
      continue
    fi

    # ── estado limpio entre pruebas ──────────────────────────────────────
    # El opencode deja procesos y el server de llama.cpp se queda con el
    # contexto de la prueba anterior (y con la VRAM). Si no se limpian, la
    # segunda prueba arranca sobre el estado de la primera: por eso el
    # servidor se para y se relanza, y el opencode se mata.
    limpiar_procesos "$modelo"

    rm -rf "$work"; mkdir -p "$work"

    # ── ropero, limpio para esta prueba ──────────────────────────────────
    # En segundo plano y con la salida aqui: en primer plano ropero se queda
    # sirviendo el puerto, que es lo que queremos.
    LOG_PRUEBA="$out/ropero-$prueba.log"
    ( cd "$dir" && nohup ropero "$modelo" > "$LOG_PRUEBA" 2>&1 & )
    sleep 1

    if grep -q "ya está corriendo" "$LOG_PRUEBA" 2>/dev/null; then
        err "el modelo ya estaba corriendo: no se puede medir en limpio"
        err "  -> para el ropero $modelo y vuelve a lanzarlo"
        continue
    elif ! esperar_modelo; then
        err "el modelo no respondió en :$PUERTO tras ${ESPERA}s"
        tail -n 15 "$LOG_PRUEBA" | sed 's/^/    /'
        continue
    fi
    ok "  servidor limpio para $prueba (log: $(basename "$LOG_PRUEBA"))"

    # El modelo esta entero en la VRAM? Si no, los tokens/s no valen.
    gpu_nota="ok"
    if gpu_completo "$LOG_PRUEBA"; then
        ok "  $GPU_AVISO"
    else
        gpu_nota="RECORTADO"
        err "  $GPU_AVISO"
        err "  -> los tokens/s estarian medidos con parte del modelo en la CPU"
        err "  -> baja el context del TOML o quita el mmproj; el fitter recorta solo"
    fi

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

    ts="$(tokens_por_segundo "$LOG_PRUEBA")"; ts="${ts:-?}"
    ms="$(ms_por_token "$LOG_PRUEBA")";     ms="${ms:-?}"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$p" "$rc" "$dur" "${sid:--}" "$(modelo_en_puerto)" "$ts" "$ms" "$gpu_nota" \
      >> "$out/resumen.tsv"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$carpeta" "$p" "$rc" "$dur" "${sid:--}" "$(modelo_en_puerto)" "$ts" "$ms" "$gpu_nota" \
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
      [[ -f "$out/ropero-$prueba.log" ]] && \
        cp "$out/ropero-$prueba.log" "$OUT_DIR/juez/$carpeta/ropero-$prueba.log"
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

# ── fusionar el global: lo viejo + lo nuevo, sin filas duplicadas ─────────
if [[ -f "$GLOBAL_PREV" ]]; then
    { cat "$GLOBAL_PREV"; tail -n +2 "$GLOBAL"; } | \
        awk -F'\t' 'NR==1{print; next} !seen[$1"\t"$2]++' > "$GLOBAL.nuevo"
    mv "$GLOBAL.nuevo" "$GLOBAL"
    rm -f "$GLOBAL_PREV"
fi

head2 "resumen global"
cat "$GLOBAL"
head2 "paquetes del juez"
ls -1 "$OUT_DIR/juez" 2>/dev/null | sed 's/^/  /' || true
