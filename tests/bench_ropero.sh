#!/usr/bin/env bash
# bench_ropero.sh — versión robusta. No muere en silencio.
set -uo pipefail

trap 'echo; echo "[abort] limpiando…" >&2; ropero stop 2>/dev/null || true; \
      pkill -f llama-server 2>/dev/null || true; exit 130' INT TERM

BASE=${BASE:-$HOME/test}
OUT=$BASE/resultados
PROMPTS=${PROMPTS:-$BASE}
PUERTO=${PUERTO:-9999}
API_KEY=${API_KEY:-sk-local-dev-key}
MODELO_OC=${MODELO:-"marichocho/Puertourraco-512k"}
AGENTE_OC=${AGENTE:-build}
ESPERA_MAX=${ESPERA_MAX:-900}
TIMEOUT=${TIMEOUT:-1800}
ROPERO_TEMP=${TEMP:-0.15}

PRUEBAS=(prueba1 prueba2 prueba3 prueba_larga prueba_gorda)
MODELOS=(
  "nemotron  nemotron"
  "onirith   onirith"
  "qwencoder qwencoder"
)

log() { echo "[$(date +%H:%M:%S)] $*" >&2; }

# ── pre-requisitos ──────────────────────────────────────────
for bin in ropero opencode jq curl; do
  command -v "$bin" >/dev/null || { echo "falta '$bin' en PATH" >&2; exit 1; }
done
command -v timeout >/dev/null || log "[!] sin 'timeout' — sin límite por prueba"

mkdir -p "$OUT"

listo() {
  local rc
  rc=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 \
       -H "Authorization: Bearer $API_KEY" \
       -H "Content-Type: application/json" \
       -d '{"model":"x","messages":[{"role":"user","content":"x"}],"max_tokens":1}' \
       "http://127.0.0.1:$PUERTO/v1/chat/completions" 2>/dev/null) || true
  [[ "$rc" == 2* ]]
}

# sin pipefail: si jq falla, head -n1 sigue sacando lo que pueda
extraer_sid() {
  local f s=""
  for f in "$@"; do
    [ -f "$f" ] || continue
    s=$(jq -r 'select(.sessionID or .session_id) | (.sessionID // .session_id)' \
        "$f" 2>/dev/null | head -n1) || true
    [ -n "$s" ] && { echo "$s"; return; }
  done
}

for par in "${MODELOS[@]}"; do
  read -r dir romodel <<<"$par"
  work=$BASE/$dir
  mkdir -p "$work"
  cd "$work" || { log "cd $work falló"; continue; }

  nombre=${dir^^}
  resumen=$OUT/${nombre}_RESUMEN.txt
  : > "$resumen"

  log "================= $dir ================="
  ropero stop || true
  pkill -f llama-server 2>/dev/null || true
  sleep 2

  ROPERO_TEMP=$ROPERO_TEMP ropero "$romodel" > "$work/log_llama_global.log" 2>&1 &
  ropero_pid=$!

  t0=$(date +%s)
  ok_carga=0
  until listo; do
    if ! kill -0 "$ropero_pid" 2>/dev/null; then
      log "[x] $romodel: ropero murió. Tail del log:"
      tail -30 "$work/log_llama_global.log" >&2
      break
    fi
    if (( $(date +%s) - t0 > ESPERA_MAX )); then
      log "[x] $romodel no respondió en ${ESPERA_MAX}s"
      break
    fi
    sleep 5
  done
  listo && ok_carga=1
  if (( ok_carga == 0 )); then
    ropero stop || true
    pkill -f llama-server 2>/dev/null || true
    continue
  fi
  log "[+] $romodel cargado en $(( $(date +%s) - t0 ))s"

  for prueba in "${PRUEBAS[@]}"; do
    prompt=$PROMPTS/${prueba}.md
    pdir=$work/$prueba
    mkdir -p "$pdir"

    if [ ! -f "$prompt" ]; then
      log "  [!] falta $prompt, salto $prueba"
      continue
    fi

    log "  → $prueba (prompt $(wc -c <"$prompt") bytes)"
    marca=$(stat -c%s "$work/log_llama_global.log" 2>/dev/null || echo 0)
    t_prueba=$(date +%s)

    TO=()
    command -v timeout >/dev/null && TO=(timeout --signal=TERM --kill-after=30 "$TIMEOUT")

    set +e
    "${TO[@]}" opencode run "$(<"$prompt")" \
      --agent "$AGENTE_OC" \
      --model "$MODELO_OC" \
      --format json \
      > "$pdir/log_opencode.json" 2> "$pdir/log_opencode.err"
    rc=$?
    set -e 2>/dev/null || true

    dur=$(( $(date +%s) - t_prueba ))
    case $rc in
      0)   log "  ✓ $prueba OK (${dur}s)" ;;
      124) log "  [!] $prueba TIMEOUT ${TIMEOUT}s (${dur}s)" ;;
      137) log "  [!] $prueba matado tras TERM (${dur}s)" ;;
      *)   log "  [!] $prueba opencode rc=$rc (${dur}s)" ;;
    esac

    sleep 1
    if [ -f "$work/log_llama_global.log" ]; then
      tail -c +$((marca + 1)) "$work/log_llama_global.log" > "$pdir/log_llama.log" 2>/dev/null \
        || : > "$pdir/log_llama.log"
    else
      : > "$pdir/log_llama.log"
    fi

    sid=$(extraer_sid "$pdir/log_opencode.json" "$pdir/log_opencode.err")
    if [ -n "${sid:-}" ]; then
      log "  $prueba: sessionID=$sid"
      if ! opencode session export "$sid" > "$pdir/session_opencode.json" 2>/dev/null; then
        log "  [!] $prueba: export falló"
        echo "(export falló)" > "$pdir/session_opencode.json"
      fi
    else
      log "  [!] $prueba: sin sessionID"
      echo "(sin sessionID)" > "$pdir/session_opencode.json"
    fi

    {
      echo
      echo "############################################################"
      echo "## PRUEBA: $prueba  (rc=$rc, ${dur}s)"
      echo "############################################################"
      echo
      echo "----------------- log llama-server -----------------"
      cat "$pdir/log_llama.log"
      echo
      echo "-----------------   log opencode   -----------------"
      cat "$pdir/log_opencode.json"
      echo
      echo "----------------- stderr opencode  -----------------"
      cat "$pdir/log_opencode.err"
      echo
      echo "-----------------  sesion opencode -----------------"
      cat "$pdir/session_opencode.json"
      echo
    } >> "$resumen"
  done

  ropero stop || true
  pkill -f llama-server 2>/dev/null || true
  log "[+] $resumen"
done

ropero stop || true
pkill -f llama-server 2>/dev/null || true