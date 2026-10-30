#!/usr/bin/env bash
# =============================================================================
# tests/suite/run.sh — suite offline de ropero 1.0
#
# Sin red, sin GPU, sin tocar el HOME real: aísla HOME/GGUF_DIR/STATE_DIR...
# en un tmpdir y antepone tests/fakes al PATH (incluido un pkill falso: la
# suite NUNCA mata procesos reales).
# =============================================================================
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd -P)"
RP="$ROOT/bin/ropero"
FAKES="$ROOT/tests/fakes"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/ropero-suite.XXXXXX")" || exit 1
trap 'rm -rf "$TMP"' EXIT

export HOME="$TMP/home"
export GGUF_DIR="$TMP/gguf"
export SAFETENSORS_DIR="$TMP/safetensors"
export LOG_DIR="$TMP/log"
export STATE_DIR="$TMP/state"
export BIN_DIR="$TMP/bin"
export ROPERO_START_TIMEOUT=20
export NO_COLOR=1
export FAKE_LOG="$TMP/fakes.log"
export PATH="$FAKES:/usr/bin:/bin:/usr/sbin:/sbin"
mkdir -p "$HOME" "$GGUF_DIR/qwen38" "$SAFETENSORS_DIR" "$LOG_DIR" "$STATE_DIR" "$BIN_DIR"
touch "$GGUF_DIR/qwen38/Qwen3.8-27B-UD-IQ3_XXS.gguf"

N_OK=0; N_FAIL=0
ok()   { N_OK=$((N_OK+1)); printf '  ok   %s\n' "$1"; }
fail() { N_FAIL=$((N_FAIL+1)); printf '  FAIL %s\n' "$1"; [[ $# -gt 1 ]] && printf '       %s\n' "$2"; }
check() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$d"; else fail "$d"; fi; }

case_help() {
  local out
  out="$("$RP" --version 2>/dev/null)"
  [[ "$out" == "ropero 1.0.0" ]] && ok "version" || fail "version" "$out"
  "$RP" help >/dev/null 2>&1 && ok "help rc0" || fail "help rc0"
  "$RP" comando-inexistente >/dev/null 2>&1; [[ $? -eq 2 ]] && ok "desconocido rc2" || fail "desconocido rc2"
  /bin/bash "$RP" help >/dev/null 2>&1 && ok "bash 3.2 help" || fail "bash 3.2 help"
}

case_catalogo() {
  local out
  out="$("$RP" list 2>/dev/null)"
  printf '%s\n' "$out" | grep -Eq '^  [A-Za-z0-9._-]+[[:space:]]' && ok "list 2 espacios" || fail "list 2 espacios"
  printf '%s\n' "$out" | grep -q metalnemotron && ok "list mlx" || fail "list mlx"
  "$RP" list --all 2>/dev/null | grep -q 'retirado' && ok "list --all fired" || fail "list --all fired"
  [[ "$("$RP" levels qwen38 2>/dev/null | wc -l | tr -d ' ')" == "4" ]] && ok "levels" || fail "levels"
  "$RP" args qwen38 2>/dev/null | grep -qx -- '--n-gpu-layers' && ok "args" || fail "args"
  "$RP" args qwen38 --high >/dev/null 2>&1 && ok "args nivel" || fail "args nivel"
  "$RP" path qwen38 2>/dev/null | grep -q '^GGUF=' && ok "path" || fail "path"
  "$RP" template 2>/dev/null | head -1 | grep -q '^#' && ok "template" || fail "template"
  python3 "$ROOT/bin/python/modelconf.py" validate --root "$ROOT/models" >/dev/null 2>&1 \
    && ok "validate" || fail "validate"
}

case_runtime() {
  local out pid1 pid2
  out="$("$RP" use qwen38 --port 41001 2>/dev/null)"
  [[ $? -eq 0 && "$(printf '%s\n' "$out" | tail -1)" == "ROPERO_PORT=41001" ]] \
    && ok "use + ROPERO_PORT" || fail "use + ROPERO_PORT" "$out"
  pid1="$(awk -F= '/^PID=/{print $2}' "$STATE_DIR/ports/41001" 2>/dev/null)"
  out="$("$RP" use qwen38 --port 41001 2>/dev/null)"
  pid2="$(awk -F= '/^PID=/{print $2}' "$STATE_DIR/ports/41001" 2>/dev/null)"
  [[ -n "$pid1" && "$pid1" == "$pid2" ]] && ok "use idempotente" || fail "use idempotente" "$pid1/$pid2"
  "$RP" use qwen38 --port 41001 --nivel high >/dev/null 2>&1 && ok "use nivel reinicia" || fail "use nivel reinicia"
  "$RP" status qwen38 >/dev/null 2>&1 && ok "status modelo" || fail "status modelo"
  "$RP" stop >/dev/null 2>&1 && ok "stop" || fail "stop"
  "$RP" status qwen38 >/dev/null 2>&1; [[ $? -eq 1 ]] && ok "status parado rc1" || fail "status parado rc1"
}

case_install() {
  echo | "$RP" install >/dev/null 2>&1; [[ $? -eq 2 ]] && ok "install sin TTY rc2" || fail "install sin TTY rc2"
  "$RP" install setup --dry-run >/dev/null 2>&1 && ok "setup --dry-run" || fail "setup --dry-run"
  "$RP" install models qwen38 >/dev/null 2>&1; [[ $? -eq 2 ]] && ok "models ambiguo rc2" || fail "models ambiguo rc2"
  "$RP" install models gptoss --build >/dev/null 2>&1; [[ $? -eq 2 ]] && ok "models sin build rc2" || fail "models sin build rc2"
  "$RP" install opencode-plugin >/dev/null 2>&1 \
    && [[ -f "$HOME/.config/opencode/plugins/ropero-modelo/index.ts" ]] \
    && ok "install opencode-plugin" || fail "install opencode-plugin"
}

case_gguffer() {
  "$RP" gguffer --help >/dev/null 2>&1 && ok "gguffer --help" || fail "gguffer --help"
  "$ROOT/bin/ropero" gguffer >/dev/null 2>&1; [[ $? -eq 2 ]] && ok "gguffer sin args rc2" || fail "gguffer sin args rc2"
}

case_test_cmd() {
  local dir out
  dir="$TMP/test-run"; mkdir -p "$dir"
  out="$( cd "$dir" && "$RP" test qwen38 --prueba prueba1 --yes --timeout 20 --port 41002 2>&1 )"
  if [[ $? -eq 0 ]] && ls "$dir"/ropero-test-qwen38-prueba1-*.md >/dev/null 2>&1; then
    local f
    f="$(ls "$dir"/ropero-test-qwen38-prueba1-*.md | head -1)"
    grep -q '^## Juez' "$f" && grep -q '^## Prueba' "$f" \
      && grep -q '^## Log del engine' "$f" && grep -q '^## Salida de opencode' "$f" \
      && grep -q '^## Sesión exportada' "$f" \
      && ok "test informe autocontenido" || fail "test informe autocontenido"
  else
    fail "test flujo" "$out"
  fi
  grep -q 'pkill -9 -x llama-server' "$FAKE_LOG" 2>/dev/null && ok "test usa pkill" || fail "test usa pkill"
}

echo "ropero suite 1.0"
case_help
case_catalogo
case_runtime
case_install
case_gguffer
case_test_cmd
echo
echo "$N_OK ok · $N_FAIL fallo(s)"
[[ "$N_FAIL" -eq 0 ]]
