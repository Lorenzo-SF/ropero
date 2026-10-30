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

# -----------------------------------------------------------------------------
# El engine tiene que ser HIJO del shell que arranca ropero.
#
# El engine arranca con "nohup ... &". Si la llamada va dentro de $( ), el
# proceso lo crea el subshell y al morir éste el engine queda huérfano: el
# `wait` de primer plano responde "pid N no es un proceso hijo de este shell"
# con rc 127. Como 127 != 0, ropero decía "el modelo terminó con código
# 127" y limpió el estado, mientras el engine seguía VIVO: el modelo
# arrancaba y roperoimentarya que había muerto.
#
# El test mete un engine falso que muere con 42. El código que tiene que
# ver ropero es 42 (el del engine), nunca 127 (el del `wait` sin hijo).
# -----------------------------------------------------------------------------
case_launch_foreground() {
  local d out rc
  d="$TMP/badengine"; mkdir -p "$d"
  cat >"$d/llama-server" <<'FAKE'
#!/usr/bin/env bash
echo "fake engine: muere con 42" >&2
exit 42
FAKE
  chmod +x "$d/llama-server"

  out="$(PATH="$d:$PATH" "$RP" qwen38 --port 41003 2>&1)"
  rc=$?

  if printf '%s\n' "$out" | grep -q 'terminó con código 42'; then
    ok "foreground: el código del engine llega a ropero"
  else
    fail "foreground: el código del engine llega a ropero" "rc=$rc · $out"
  fi
  # El aviso de bash sale en el idioma del locale: "is not a child of this
  # shell" en inglés, "no es un proceso hijo de este shell" en español. Un
  # grep de un idioma sólo no vigila nada en el otro: se buscan los dos.
  if printf '%s\n' "$out" | grep -qE 'is not a child of this shell|no es un proceso hijo de este shell'; then
    fail "foreground: el engine es hijo del shell" "$out"
  else
    ok "foreground: el engine es hijo del shell"
  fi
  if [[ "$rc" -ne 1 ]]; then
    fail "foreground: rc 1 cuando el engine muere" "rc=$rc"
  else
    ok "foreground: rc 1 cuando el engine muere"
  fi
}

# El escenario real del usuario: un engine que VIVE, arrancado en primer
# plano. Antes del fix, ropero decía "terminó con código 127" en los
# segundos siguientes y limpiaba el estado, mientras el engine seguía
# sirviendo: el modelo arrancaba y ropero decía que había muerto.
case_launch_foreground_vivo() {
  local out
  # Si una corrida anterior dejó el engine HUÉRFANO (el bug del wait), este
  # test falla por el puerto ocupado y no por lo que quiere comprobar. Se
  # limpia antes y el engine huérfano se mata al terminar.
  pkill -f "_fake_openai.py --host 127.0.0.1 --port 41004" 2>/dev/null || true
  sleep 1
  out="$(timeout -s TERM 6 "$RP" qwen38 --port 41004 2>&1)"
  pkill -f "_fake_openai.py --host 127.0.0.1 --port 41004" 2>/dev/null || true
  if printf '%s\n' "$out" | grep -qE 'terminó con código 127'; then
    fail "foreground con engine vivo: no muere" "$out"
  else
    ok "foreground con engine vivo: no muere"
  fi
  if printf '%s\n' "$out" | grep -q 'arrancando'; then
    ok "foreground con engine vivo: anuncia el arranque"
  else
    fail "foreground con engine vivo: anuncia el arranque" "$out"
  fi
  # Al matarlo con SIGTERM, el trap limpia y sale 130: tampoco 127.
  if printf '%s\n' "$out" | grep -qE 'terminó con código 127'; then
    fail "foreground con engine vivo: SIGTERM no da 127" "$out"
  else
    ok "foreground con engine vivo: SIGTERM no da 127"
  fi
}

# -----------------------------------------------------------------------------
# Un --chat-template-file que no existe tiene que fallar ANTES de arrancar,
# con una línea que lo diga. llama-server, en su lugar, aborta con el muro
# de sus plantillas built-in: el motivo real queda enterrado arriba.
# -----------------------------------------------------------------------------
case_chat_template() {
  local out
  out="$("$RP" args nemotron 2>/dev/null | grep -A1 -x -- '--chat-template-file' | tail -1)"
  if [[ -n "$out" && "$out" != *'{'* && "$out" != *'$'* ]]; then
    ok "el placeholder del chat template está expandido"
  else
    fail "el placeholder del chat template está expandido" "$out"
  fi
  out="$(python3 "$ROOT/bin/python/modelconf.py" aux --root "$ROOT/models" --name nemotron 2>/dev/null)"
  if printf '%s\n' "$out" | grep -q '^chat_template|.*|chat_template.jinja|.*|run$'; then
    ok "nemotron declara el chat template como aux"
  else
    fail "nemotron declara el chat template como aux" "$out"
  fi
  out="$(python3 "$ROOT/bin/python/modelconf.py" aux --root "$ROOT/models" --name granite 2>/dev/null)"
  if printf '%s\n' "$out" | grep -q '^chat_template|'; then
    ok "granite declara el chat template como aux"
  else
    fail "granite declara el chat template como aux" "$out"
  fi
}

echo "ropero suite 1.0"
case_help
case_catalogo
case_runtime
case_install
case_gguffer
case_test_cmd
case_launch_foreground
case_launch_foreground_vivo
case_chat_template
echo
echo "$N_OK ok · $N_FAIL fallo(s)"
[[ "$N_FAIL" -eq 0 ]]
