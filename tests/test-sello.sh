#!/usr/bin/env bash
# =============================================================================
# test-sello.sh — el "sello" de config: si el TOML cambio, el server se
# reinicia en vez de decir "ya esta corriendo" y quedarse con la config
# vieja.
# =============================================================================
# El bug: `ropero <modelo>` con el modelo YA corriendo decia "ya esta
# corriendo" y se iba. Asi que un cambio en el TOML (el sampling, un flag,
# el ctx) no se aplicaba NUNCA, sin avisar. Con el fable y su
# --dry-multiplier fue exactamente lo que paso: el TOML lo tenia, el server
# de siempre habia arrancado antes, y el DRY seguia apagado.
#
# Este test lo comprueba de punta a punta: arranca un server fake de verdad
# en un puerto, cambia el TOML, y mira si el server se reinicia.
# =============================================================================
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

PASS=0
FAIL=0
FAILED_CASES=()
ok()  { PASS=$((PASS+1)); printf '  ok   %s\n' "$1"; return 0; }
bad() { FAIL=$((FAIL+1)); FAILED_CASES+=("$1"); printf '  FALLO %s\n' "$1"; [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; return 0; }
check() { if [[ "$2" == "1" ]]; then ok "$1"; else bad "$1" "${3:-}"; fi; }
# OJO: el >/dev/null va DENTRO del eval, no en la redireccion de la funcion.
# Si no, se come el "echo 1" y todos los checks dan 0.
cond() { eval "$1" >/dev/null 2>&1; if [[ $? -eq 0 ]]; then echo 1; else echo 0; fi; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ropero-sello.XXXXXX")"
# El server fake se queda escuchando: el trap lo mata POR PUERTO, que es
# como sobrevive (el PID que se guarda es el del wrapper, no el del python).
trap 'rm -rf "$WORK"; lsof -t -iTCP:"${TEST_PORT:-0}" -sTCP:LISTEN 2>/dev/null | xargs -r kill 2>/dev/null' EXIT
# Un puerto libre, para que una ejecucion anterior no estorbe a esta.
TEST_PORT="$(python3 -c 'import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()')"
HOME_DIR="$WORK/home"
GGUF_DIR_TEST="$WORK/gguf"
LOG_DIR_TEST="$WORK/logs"
BIN_DIR_TEST="$WORK/bin"
mkdir -p "$HOME_DIR" "$GGUF_DIR_TEST" "$LOG_DIR_TEST" "$BIN_DIR_TEST"

# ── el server fake: abre el puerto y responde como un llama-server ────────
cat > "$BIN_DIR_TEST/llama-server" <<EOF
#!/usr/bin/env bash
# Un server fake: escucha en el puerto y sirve /v1/models.
PORT="$TEST_PORT"
python3 - "$TEST_PORT" <<'PYEOF' &
import http.server, json, sys
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        body = json.dumps({"id": "testmodel", "object": "model"}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
PYEOF
SERVER_PID=\$!
wait \$SERVER_PID
EOF
chmod +x "$BIN_DIR_TEST/llama-server"

# ── un modelo de prueba, con su TOML ───────────────────────────────────────
MODELS_DIR_TEST="$WORK/models"
mkdir -p "$MODELS_DIR_TEST/llama" "$GGUF_DIR_TEST/testmodel"
printf 'gguf de prueba\n' > "$GGUF_DIR_TEST/testmodel/testmodel.gguf"
cat > "$MODELS_DIR_TEST/llama/testmodel.toml" <<EOF
[model]
name    = "testmodel"
engine  = "llama-server"
context = 4096
port    = $TEST_PORT

[files]
gguf   = "testmodel.gguf"

[args]
base = [
    "--temp", "0.6",
    "--top-k", "20",
]
EOF

ropero_test() {
    env -i \
        HOME="$HOME_DIR" \
        PATH="$BIN_DIR_TEST:$PATH" \
        GGUF_DIR="$GGUF_DIR_TEST" \
        LOG_DIR="$LOG_DIR_TEST" \
        ROPERO_MODELS_DIR="$MODELS_DIR_TEST" \
        LLAMA_BIN="$BIN_DIR_TEST/llama-server" \
        ROPERO_START_TIMEOUT=10 \
        bash "$REPO/bin/ropero" "$@"
}

# ── 1. las funciones del sello ────────────────────────────────────────────
cp -r "$REPO/bin" "$WORK/bin-ropero"
sed -i 's/^main "\$@"$/:/' "$WORK/bin-ropero/ropero"
# shellcheck disable=SC1090
source "$WORK/bin-ropero/ropero"

a="$(_ropero_sello_calc --temp 0.6 --top-k 20)"
b="$(_ropero_sello_calc --temp 0.6 --top-k 20)"
c="$(_ropero_sello_calc --temp 0.6 --top-k 40)"
check "1a. el sello es determinista"      "$(cond '[ "$a" = "$b" ]')" "$a / $b"
check "1b. el sello cambia si cambian los args" "$(cond '[ "$a" != "$c" ]')" "$a / $c"
check "1c. sin args, no hay sello"         "$(cond '[ -z "$(_ropero_sello_calc)" ]')" "sello vacio"
check "1d. el sello va al LOG_DIR" \
      "$(cond '[[ "$(_ropero_sello_path 9999)" == *"/args-9999.sha" ]]')" "$(_ropero_sello_path 9999)"

# ── 2. arrancar el server de verdad ───────────────────────────────────────
out="$(ropero_test testmodel --background 2>&1)"; rc=$?
check "2a. arranca el server"        "$(cond '[ $rc -eq 0 ]')" "rc=$rc: $out"
check "2b. el puerto queda escuchando" "$(cond 'lsof -iTCP:'"$TEST_PORT"' -sTCP:LISTEN >/dev/null 2>&1')" "$out"

# el sello se guarda al arrancar
check "2c. se guarda el sello" \
      "$(cond '[ -f "'"$LOG_DIR_TEST"'/args-'"$TEST_PORT"'.sha" ]')" \
      "$(ls -A "$LOG_DIR_TEST" 2>/dev/null)"

# el sello con el que arranco
sello_antes="$(cat "$LOG_DIR_TEST/args-$TEST_PORT.sha" 2>/dev/null || true)"

# ── 3. volver a arrancar con el MISMO TOML: no se toca ─────────────────────
pid_antes="$(lsof -t -iTCP:"$TEST_PORT" -sTCP:LISTEN 2>/dev/null | head -1)"
out="$(ropero_test testmodel --background 2>&1)"
check "3a. con el mismo TOML: dice que ya corre" \
      "$(cond 'printf %s "$out" | grep -q "ya está corriendo"')" "$out"
check "3b. con el mismo TOML: NO lo reinicia" \
      "$(cond '[ "$pid_antes" = "$(lsof -t -iTCP:'"$TEST_PORT"' -sTCP:LISTEN 2>/dev/null | head -1)" ]')" \
      "pid antes: $pid_antes"

# ── 4. cambiar el TOML: ahora SI se reinicia ──────────────────────────────
sed -i 's/"--top-k", "20"/"--top-k", "40"/' "$MODELS_DIR_TEST/llama/testmodel.toml"
out="$(ropero_test testmodel --background 2>&1)"; rc=$?
check "4a. con el TOML cambiado: avisa" \
      "$(cond 'printf %s "$out" | grep -q "config ANTERIOR"')" "$out"
check "4b. con el TOML cambiado: reinicia" \
      "$(cond '[ "$pid_antes" != "$(lsof -t -iTCP:'"$TEST_PORT"' -sTCP:LISTEN 2>/dev/null | head -1)" ]')" \
      "pid antes: $pid_antes"

# y el sello guardado es el nuevo
nuevo="$(cat "$LOG_DIR_TEST/args-$TEST_PORT.sha" 2>/dev/null || true)"
check "4c. el sello guardado cambia con el TOML" \
      "$(cond '[ -n "$nuevo" ] && [ "$nuevo" != "$sello_antes" ]')" "antes: $sello_antes / ahora: $nuevo"

# ── 5. sin sello (server arrancado por una version vieja): reinicia ───────
# El caso de la transicion: el server lleva la config de antes y no hay
# forma de saberlo (no hay sello). Se reinicia para que lleve la actual.
# Solo pasa una vez: al arrancar, el sello se guarda.
rm -f "$LOG_DIR_TEST/args-$TEST_PORT.sha"
pid_antes="$(lsof -t -iTCP:"$TEST_PORT" -sTCP:LISTEN 2>/dev/null | head -1)"
out="$(ropero_test testmodel --background 2>&1)"
check "5a. sin sello: avisa de que no puede saber" \
      "$(cond 'printf %s "$out" | grep -q "no hay forma de saber"')" "$out"
check "5b. sin sello: reinicia" \
      "$(cond '[ -n "$pid_antes" ] && [ "$pid_antes" != "$(lsof -t -iTCP:'"$TEST_PORT"' -sTCP:LISTEN 2>/dev/null | head -1)" ]')" \
      "pid antes: $pid_antes"
check "5c. sin sello: deja el sello guardado" \
      "$(cond '[ -f "'"$LOG_DIR_TEST"'/args-'"$TEST_PORT"'.sha" ]')" "$(ls -A "$LOG_DIR_TEST")"

# ── resumen ────────────────────────────────────────────────────────────────
printf '\n%s: %d correctos, %d fallos\n' "$(basename "$0")" "$PASS" "$FAIL"
if (( FAIL > 0 )); then
    printf 'Fallos:\n'
    for c in "${FAILED_CASES[@]}"; do printf '  - %s\n' "$c"; done
    exit 1
fi
exit 0
