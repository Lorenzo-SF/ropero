#!/usr/bin/env bash
# Test del bench_ropero.sh, con fakes. Sin GPU y sin opencode de verdad.
#
# Lo que se comprueba es lo que el bench hace MAL, no lo que hace bien:
#
#   1. el modelo se arranca UNA VEZ por modelo, no en frio en cada tarea
#      (arrancar en frio son 12-20 GB de pesos por tarea: el bench se comia
#      el tiempo esperando al modelo en vez de medirlo)
#   2. los tokens/s de cada tarea son los de ESA tarea, no los de la ultima
#      (el log del servidor es uno por modelo y las tareas se acumulan)
#   3. si el modelo no arranca, se salta el modelo entero y lo dice
#   4. la espera se escala con el ctx y nunca es 0
#   5. un modelo que no declara el nivel pedido se mide con el suyo, y no
#      se le pasa un flag que haria que NO ARRANQUE
#
# Los fakes: ropero (cuenta los arranques y "carga" un server falso),
# curl (responde en /v1/models), opencode (escribe un JSON de sesion).

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

VERBOSE=0
[[ "${1:-}" == "-v" ]] && VERBOSE=1
PASS=0
FAIL=0
FAILED_CASES=()

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ropero-bench.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

ok()   { PASS=$((PASS+1)); [[ $VERBOSE -eq 1 ]] && printf '  ok   %s\n' "$1"; return 0; }
bad()  { FAIL=$((FAIL+1)); FAILED_CASES+=("$1"); printf '  FALLA %s\n' "$1"; [[ -n "${2:-}" ]] && printf '       %s\n' "$2"; return 0; }
check() { if [[ "$2" == "1" ]]; then ok "$1"; else bad "$1" "${3:-}"; fi; }
cond() { if eval "$1" >/dev/null 2>&1; then echo 1; else echo 0; fi; }

BIN="$WORK/bin"
mkdir -p "$BIN"

# ── el fake de ropero ───────────────────────────────────────────────────────
cat > "$BIN/ropero" <<'EOF'
#!/usr/bin/env bash
# Fake de ropero. "ropero <modelo> [--nivel]" arranca un server: escribe en
# el log y anota el arranque. "ropero stop <modelo>" para. "ropero levels" y
# "ropero list" contestan lo que el bench necesita.
case "${1:-}" in
  stop)
    echo "parado"
    exit 0 ;;
  levels)
    # el modelo declara el nivel, o no
    if [[ -n "${FAKE_NIVELES:-}" ]]; then echo "$FAKE_NIVELES"; fi
    exit 0 ;;
  list)
    echo "  modelo-de-prueba"
    exit 0 ;;
  check)
    exit 0 ;;
esac
modelo="${1:-}"
nivel="${2:-}"
[[ "$nivel" == --* ]] && nivel="${nivel#--}"
# el arranque
echo "arranque" >> "$FAKE_ARRANQUES"
echo "nivel=$nivel" >> "$FAKE_ARRANQUES"
if [[ -n "$FAKE_FALLA_ARRANQUE" ]]; then
    echo "ERROR: no se pudo cargar el modelo" >&2
    rm -f "$FAKE_MODELS_JSON"      # el server no arranca: el puerto no contesta
    exit 1
fi
# El log del servidor ES el stdout del bench (el bench hace "> $log 2>&1"),
# asi que se escribe ahi. Para saber la ruta, /proc/$$/fd/1: con
# /proc/self/fd/1 no vale, porque dentro de un command substitution el fd 1
# es la PIPE de la captura y no el log.
# El stdout del fake ES el log del bench (el bench hace "> $log 2>&1"), asi
# que las lineas del servidor se escriben ahi, tal cual.
#
# Para que el fake de opencode sepa DONDE esta ese log (sus timings tienen
# que acabar en el log del servidor, no en su stdout, que el bench captura
# como JSON), se averigua la ruta del fd 1. OJO: con "1>/dev/null" y no con
# un command substitution, que cambiaria el fd 1 antes de resolver la ruta.
ls -l /proc/$$/fd/1 2>/dev/null | sed 's/.*-> //' > "$FAKE_LOG_PATH"
LOG_REAL="$(cat "$FAKE_LOG_PATH" 2>/dev/null)"
[[ -f "$LOG_REAL" ]] || LOG_REAL="$FAKE_LOG_SERVIDOR"
export FAKE_LOG_SERVIDOR="$LOG_REAL"
cat <<LOG
main: server started, n_ctx = 131072
llama_model_load: n_layer = 41, n_gpu_layers = 41
LOG
# el server "vive": responde en /v1/models con el id del modelo
cat > "$FAKE_MODELS_JSON" <<JSON
{"object":"list","data":[{"id":"$modelo","object":"model"}]}
JSON
# el bench espera a que el puerto responda: aqui se marca que ya esta
sleep 0.2
exit 0
EOF
chmod +x "$BIN/ropero"

# ── el fake de curl ────────────────────────────────────────────────────────
cat > "$BIN/curl" <<'EOF'
#!/usr/bin/env bash
# Fake de curl: solo sabe responder en /v1/models, que es lo que el bench
# usa para saber si el modelo esta listo.
for a in "$@"; do
  case "$a" in
    *"/v1/models")
      [[ -f "$FAKE_MODELS_JSON" ]] || exit 7
      cat "$FAKE_MODELS_JSON"
      exit 0 ;;
  esac
done
exit 7
EOF
chmod +x "$BIN/curl"

# ── el fake de opencode ────────────────────────────────────────────────────
cat > "$BIN/opencode" <<'EOF'
#!/usr/bin/env bash
# Fake de opencode run: escribe los eventos JSON y, en el log del servidor,
# los timings de la tarea (que es de donde el bench saca los tokens/s).
# El numero de tokens lo decide FAKE_TPS: cambia por tarea para poder
# comprobar que el bench NO lee siempre el ultimo.
n_ps=0
for a in "$@"; do
  case "$a" in
    run) n_ps=1 ;;
  esac
done
if (( n_ps == 0 )); then exit 0; fi

# Cada invocacion del "run" es una tarea. El numero de la tarea va en un
# contador: es mas fiable que mirar el texto del prompt (que es multilinea
# y lo mete el bench con sus propias inyecciones).
contador=0
[[ -f "$FAKE_CONTADOR" ]] && contador="$(cat "$FAKE_CONTADOR" 2>/dev/null || echo 0)"
[[ "$contador" =~ ^[0-9]+$ ]] || contador=0
contador=$((contador + 1))
printf '%s' "$contador" > "$FAKE_CONTADOR"
case "$contador" in
  1) tps=11 ;;
  2) tps=22 ;;
  *) tps=33 ;;
esac

sid="sesion-falsa-$$"
cat <<JSON
{"type":"session.id","sessionID":"$sid","properties":{"id":"$sid"}}
{"type":"message","role":"assistant","parts":[{"type":"text","text":"hecho. esto es una respuesta real de mas de cuarenta caracteres para que el bench la acepte."]}
JSON
# los timings en el log del servidor. La ruta se lee del fichero de estado
# que dejo el fake de ropero: los dos fakes son procesos distintos y no se
# ven entre si.
LOG_SERVER="$(cat "$FAKE_LOG_PATH" 2>/dev/null)"
[[ -f "$LOG_SERVER" ]] || LOG_SERVER="$FAKE_LOG_SERVIDOR"
cat >> "$LOG_SERVER" <<LOG
llama_perf_context_print: prompt eval time = 100.00 ms / 10 tokens ( 100.00 tokens per second)
llama_perf_context_print:        eval time = 1000.00 ms / $tps runs ( $(awk "BEGIN{printf \"%.2f\", 1000.0/$tps}") ms per token, $tps tokens per second)
LOG
exit 0
EOF
chmod +x "$BIN/opencode"

# ── el escenario ───────────────────────────────────────────────────────────
MOD="$WORK/modelos/modelo-de-prueba"
mkdir -p "$MOD"
cat > "$WORK/prueba.md" <<'EOF'
# Prueba

**S1 (algo)**
Contenido de la tarea 1.

**S2 (algo)**
Contenido de la tarea 2.
EOF

export FAKE_ARRANQUES="$WORK/arranques"
export FAKE_LOG_SERVIDOR="$WORK/servidor.log"
export FAKE_MODELS_JSON="$WORK/models.json"
export FAKE_SALIDA_JSON="$WORK/opencode.json"
export FAKE_LOG_PATH="$WORK/log-path.txt"
export FAKE_CONTADOR="$WORK/contador.txt"
: > "$FAKE_ARRANQUES"
: > "$FAKE_LOG_SERVIDOR"
: > "$FAKE_CONTADOR"

# ── 1. el modelo se arranca UNA VEZ ────────────────────────────────────────
out="$(PATH="$BIN:$PATH" \
      MODELOS_DIR="$WORK/modelos" \
      PROMPTS_DIR="$WORK" \
      OUT_DIR="$WORK/resultados" \
      ESPERA=20 \
      PROMPTS="prueba.md" \
      bash "$REPO/tests/bench_ropero.sh" modelo-de-prueba 2>&1)"; rc=$?
check "1a. el bench termina bien" "$(cond '[ $rc -eq 0 ]')" "$out"

arranques="$(grep -c '^arranque$' "$FAKE_ARRANQUES" 2>/dev/null || echo 0)"
check "1b. el modelo se arranca UNA VEZ (no por tarea)" \
      "$(cond '[ "$arranques" -eq 1 ]')" "arranques=$arranques"
check "1c. y hay dos tareas en el prompt" \
      "$(cond 'grep -q "S1" "$WORK/resultados/modelo-de-prueba/resumen.tsv" && grep -q "S2" "$WORK/resultados/modelo-de-prueba/resumen.tsv"')" \
      "$(cat "$WORK/resultados/modelo-de-prueba/resumen.tsv" 2>/dev/null)"

# ── 2. los tokens/s son los de CADA tarea ──────────────────────────────────
resumen="$WORK/resultados/modelo-de-prueba/resumen.tsv"
ts_t1="$(awk -F'\t' '$1=="S1"{print $6}' "$resumen" 2>/dev/null)"
ts_t2="$(awk -F'\t' '$1=="S2"{print $6}' "$resumen" 2>/dev/null)"
check "2a. la tarea 1 mide sus propios tokens/s (11)" \
      "$(cond '[ "$ts_t1" = "11" ]')" "ts_t1=$ts_t1"
check "2b. la tarea 2 mide los suyos (22), no los de la ultima de la otra" \
      "$(cond '[ "$ts_t2" = "22" ]')" "ts_t2=$ts_t2"

# ── 3. si el modelo no arranca, se salta el modelo entero ─────────────────
: > "$FAKE_ARRANQUES"
: > "$FAKE_CONTADOR"
out2="$(PATH="$BIN:$PATH" \
       MODELOS_DIR="$WORK/modelos" \
       PROMPTS_DIR="$WORK" \
       OUT_DIR="$WORK/resultados2" \
       ESPERA=20 \
       PROMPTS="prueba.md" \
       FAKE_FALLA_ARRANQUE=1 \
       bash "$REPO/tests/bench_ropero.sh" modelo-de-prueba 2>&1)"; rc2=$?
check "3a. avisa de que el modelo no arranca" \
      "$(cond 'printf %s "$out2" | grep -qi "no arranca"')" "$out2"
check "3b. no registra ninguna tarea como medida" \
      "$(cond '[ "$(wc -l < "$WORK/resultados2/modelo-de-prueba/resumen.tsv" 2>/dev/null || echo 0)" -le 1 ]')" \
      "$(cat "$WORK/resultados2/modelo-de-prueba/resumen.tsv" 2>/dev/null)"

# ── 4. la espera nunca es 0 ────────────────────────────────────────────────
check "4a. el codigo no puede dejar la espera en 0" \
      "$(cond '! grep -qE "ESPERA=0|espera=0" "$REPO/tests/bench_ropero.sh"')"
check "4b. la espera por defecto es de 900 s o mas" \
      "$(cond 'grep -qE "ESPERA_MIN:-900" "$REPO/tests/bench_ropero.sh"')"

# ── 5. el nivel que el modelo no declara ───────────────────────────────────
: > "$FAKE_ARRANQUES"
: > "$FAKE_CONTADOR"
out3="$(PATH="$BIN:$PATH" \
       MODELOS_DIR="$WORK/modelos" \
       PROMPTS_DIR="$WORK" \
       OUT_DIR="$WORK/resultados3" \
       ESPERA=20 \
       PROMPTS="prueba.md" \
       NIVEL=high \
       FAKE_NIVELES="" \
       bash "$REPO/tests/bench_ropero.sh" modelo-de-prueba 2>&1)"; rc3=$?
check "5a. avisa de que el modelo no declara el nivel" \
      "$(cond 'printf %s "$out3" | grep -q "no declara el nivel"')" "$out3"
check "5b. y arranca igualmente (con su nivel por defecto)" \
      "$(cond '[ "$(grep -c "^arranque$" "$FAKE_ARRANQUES")" -eq 1 ]')" \
      "$(cat "$FAKE_ARRANQUES")"
check "5c. sin pasarle un flag que haria que no arrancase" \
      "$(cond '! grep -q "^nivel=--high$" "$FAKE_ARRANQUES"')" \
      "$(cat "$FAKE_ARRANQUES")"

# con el nivel declarado, se pasa
: > "$FAKE_ARRANQUES"
: > "$FAKE_CONTADOR"
PATH="$BIN:$PATH" MODELOS_DIR="$WORK/modelos" PROMPTS_DIR="$WORK" \
    OUT_DIR="$WORK/resultados4" ESPERA=20 PROMPTS="prueba.md" NIVEL=high FAKE_NIVELES="high" \
    bash "$REPO/tests/bench_ropero.sh" modelo-de-prueba >/dev/null 2>&1
check "5d. con el nivel declarado, se pasa el flag" \
      "$(cond 'grep -q "^nivel=high$" "$FAKE_ARRANQUES"')" \
      "$(cat "$FAKE_ARRANQUES")"

# ── resumen ────────────────────────────────────────────────────────────────
printf '\n%s: %d correctos, %d fallos\n' "$(basename "$0")" "$PASS" "$FAIL"
if (( FAIL > 0 )); then
    printf 'Fallos:\n'
    for c in "${FAILED_CASES[@]}"; do printf '  - %s\n' "$c"; done
    exit 1
fi
exit 0
