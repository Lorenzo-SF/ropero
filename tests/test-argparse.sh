#!/usr/bin/env bash
# Contract and safety tests for ropero's v2 CLI parser.

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
BASH_BIN="$(command -v bash)"
PYTHON_BIN="$(python3 -c 'import sys; print(sys.executable)')"

VERBOSE=0
[[ "${1:-}" == "-v" ]] && VERBOSE=1
PASS=0
FAIL=0
FAILED_CASES=()

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ropero-argparse.XXXXXX")"
REAL_HOME="${HOME:-}"
trap 'rm -rf "$WORK"' EXIT

RUN_DIR="$WORK/run"
HOME_DIR="$WORK/home"
GGUF_DIR_TEST="$WORK/models/gguf"
SAFETENSORS_DIR_TEST="$WORK/models/safetensors"
LOG_DIR_TEST="$WORK/logs"
LLAMA_DIR_TEST="$WORK/llama.cpp"
BIN_DIR_TEST="$WORK/bin"
BLOCK_DIR="$WORK/blocked-bin"
SENTINEL_LOG="$WORK/external-effects.log"
STATE_FILE="$WORK/engine-state"
mkdir -p "$RUN_DIR" "$HOME_DIR" "$GGUF_DIR_TEST" "$SAFETENSORS_DIR_TEST" \
    "$LOG_DIR_TEST" "$LLAMA_DIR_TEST" "$BIN_DIR_TEST" "$BLOCK_DIR" "$WORK/tmp"
: >"$SENTINEL_LOG"
: >"$STATE_FILE"

# A second safety net. If a production path escapes the leaf stubs, these
# executables record it and fail instead of touching the network or host.
for tool in curl wget git cmake ninja make brew pacman paru sudo pkg apt apt-get \
    dnf pip pip3 python python3 uv hf huggingface-cli flm; do
    cat >"$BLOCK_DIR/$tool" <<'EOF'
#!/usr/bin/env bash
printf '%s' "$(basename "$0")" >>"${ROPERO_TEST_SENTINEL:?}"
printf ' [%s]' "$@" >>"${ROPERO_TEST_SENTINEL:?}"
printf '\n' >>"${ROPERO_TEST_SENTINEL:?}"
exit 125
EOF
    chmod +x "$BLOCK_DIR/$tool"
done

# Copy production and inject overrides immediately before main. Appending after
# main would be too late; replacing function bodies is brittle around nested
# braces. The parser and its orchestration remain production code. Only leaves
# that perform work are replaced.
_instrument() {
    cp "$REPO/ropero" "$WORK/ropero"
    cp -R "$REPO/ropero.d" "$WORK/ropero.d"
    "$PYTHON_BIN" - "$WORK/ropero" <<'PY'
import io
import sys

path = sys.argv[1]
source = io.open(path, encoding="utf-8").read()
needle = '\nmain "$@"\n'
if source.count(needle) != 1:
    raise SystemExit("expected exactly one main call")

stubs = r'''
# test-only leaf overrides (injected into the temporary copy)
_ropero_test_call() {
    local name="$1"; shift
    printf 'CALL %s:' "$name"
    printf ' [%s]' "$@"
    printf ' FORCE=%s\n' "${ROPERO_FORCE:-0}"
}

ropero_env_setup() { _ropero_test_call ropero_env_setup "$@"; }
ropero_link_self() { _ropero_test_call ropero_link_self "$@"; }
ropero_link_llamacpp() { _ropero_test_call ropero_link_llamacpp "$@"; }
ropero_link_tensorrt() { _ropero_test_call ropero_link_tensorrt "$@"; }
ropero_link_airllm() { _ropero_test_call ropero_link_airllm "$@"; }
ropero_link_mlx() { _ropero_test_call ropero_link_mlx "$@"; }
ropero_link_fastflowlm() { _ropero_test_call ropero_link_fastflowlm "$@"; }
ropero_compile_llama() { _ropero_test_call ropero_compile_llama "$@"; }

_ropero_test_engine_installed() {
    printf '%s\n' "$1" >>"${ROPERO_TEST_STATE:?}"
}
ropero_install_llama() {
    _ropero_test_call ropero_install_llama "$@"
    _ropero_test_engine_installed llama-server
}
ropero_install_airllm() {
    _ropero_test_call ropero_install_airllm "$@"
    _ropero_test_engine_installed airllm
}
ropero_install_mlx() {
    _ropero_test_call ropero_install_mlx "$@"
    _ropero_test_engine_installed mlx_lm
}
ropero_install_tensorrt() {
    _ropero_test_call ropero_install_tensorrt "$@"
    _ropero_test_engine_installed tensorrt-llm
}
ropero_install_fastflowlm() {
    _ropero_test_call ropero_install_fastflowlm "$@"
    _ropero_test_engine_installed fastflowlm
}
ropero_engine_probe() {
    local engine="${1:-}"
    if grep -qx "$engine" "${ROPERO_TEST_STATE:?}" 2>/dev/null; then
        printf ok
        return 0
    fi
    printf missing
    return 1
}

# Legacy bulk phase contains package-manager calls before its installer leaves.
# v2 must not use it, but stubbing it makes an accidental regression observable
# rather than destructive.
_ropero_install_engines() { _ropero_test_call _ropero_install_engines "$@"; }

ropero_download_gguf() { _ropero_test_call ropero_download_gguf "$@"; }
ropero_download_safetensors() { _ropero_test_call ropero_download_safetensors "$@"; }
_ropero_cat_download() {
    local i="${1:?catalog index required}" kind dest name
    _ropero_cat_build
    _ropero_test_call _ropero_cat_download "$@"
    kind="${_ROPERO_CAT_KIND[$i]}"
    dest="${_ROPERO_CAT_DEST[$i]:-}"
    case "$dest" in
        '') [[ "$kind" == gguf ]] && dest="$GGUF_DIR" || dest="$SAFETENSORS_DIR/${_ROPERO_CAT_KEY[$i]}" ;;
        '~') dest="$HOME" ;;
        '~/'*) dest="$HOME/${dest#\~/}" ;;
    esac
    if [[ "$kind" == gguf ]]; then
        name="${_ROPERO_CAT_DNAME[$i]:-${_ROPERO_CAT_ARG2[$i]}}"
        mkdir -p "$dest/$(dirname "$name")"
        printf stub >"$dest/$name"
    else
        mkdir -p "$dest"
        printf stub >"$dest/model.safetensors"
        printf '{}\n' >"$dest/config.json"
    fi
}
_ropero_cat_postcopy() { _ropero_test_call _ropero_cat_postcopy "$@"; }

_kill_port() { _ropero_test_call _kill_port "$@"; }
_stop_port() { _ropero_test_call _stop_port "$@"; }
# Preserve production's unknown-model validation, which occurs inside this
# leaf, while replacing launch work for models that passed that validation.
eval "$(declare -f start_model | sed '1s/start_model/_ropero_real_start_model/')"
start_model() {
    local model_key="${1//-/_}"
    if [[ ! " ${MODEL_NAMES[*]} " =~ " $model_key " ]]; then
        _ropero_real_start_model "$@"
        return $?
    fi
    _ropero_test_call start_model "$@"
}
'''
source = source.replace(needle, "\n" + stubs + '\nmain "$@"\n')
io.open(path, "w", encoding="utf-8").write(source)
PY
    chmod +x "$WORK/ropero"
}

if ! _instrument || ! "$BASH_BIN" -n "$WORK/ropero"; then
    printf 'FATAL: no se pudo crear una copia instrumentada válida\n' >&2
    exit 1
fi

# Portable watchdog. Popen starts a new process group; timeout terminates and,
# if necessary, kills that whole group so grandchildren cannot survive a case.
_watchdog() {
    "$PYTHON_BIN" - "$@" <<'PY'
import os
import signal
import subprocess
import sys

cmd = sys.argv[1:]
proc = subprocess.Popen(cmd, start_new_session=True)
try:
    rc = proc.wait(timeout=30)
except subprocess.TimeoutExpired:
    os.killpg(proc.pid, signal.SIGTERM)
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
        proc.wait()
    print("TIMEOUT: " + " ".join(cmd), file=sys.stderr)
    raise SystemExit(124)
raise SystemExit(rc)
PY
}

_ropero() {
    (
        cd "$RUN_DIR" || exit 98
        export HOME="$HOME_DIR"
        export GGUF_DIR="$GGUF_DIR_TEST"
        export SAFETENSORS_DIR="$SAFETENSORS_DIR_TEST"
        export LOG_DIR="$LOG_DIR_TEST"
        export LLAMA_DIR="$LLAMA_DIR_TEST"
        export BIN_DIR="$BIN_DIR_TEST"
        export TMPDIR="$WORK/tmp"
        export PATH="$BLOCK_DIR:$BIN_DIR_TEST:/usr/bin:/bin:/usr/sbin:/sbin"
        export AUTO_YES=1 CI=1 ROPERO_GPU_PORT=59999 ROPERO_CPU_PORT=59998
        export ROPERO_TEST_SENTINEL="$SENTINEL_LOG"
        export ROPERO_TEST_STATE="$STATE_FILE"
        : >"$STATE_FILE"
        _watchdog "$BASH_BIN" "$WORK/ropero" "$@" 2>&1
    )
}

_pass() {
    PASS=$((PASS + 1))
    (( VERBOSE )) && printf '  ok   %s\n' "$1"
    return 0
}

_fail() {
    local desc="$1" why="$2" out="${3:-}"
    FAIL=$((FAIL + 1))
    FAILED_CASES+=("$desc — $why")
    printf '  FAIL %s\n       → %s\n' "$desc" "$why"
    [[ -n "$out" ]] && printf '       salida: %s\n' "$(printf '%s' "$out" | head -c 600 | tr '\n' '|')"
}

# check DESC RC REGEX -- ARGS...
check() {
    local desc="$1" want_rc="$2" want_re="$3" out rc why=""
    shift 3
    [[ "${1:-}" == -- ]] && shift
    out="$(_ropero "$@")"; rc=$?
    [[ "$rc" == "$want_rc" ]] || why="rc esperado $want_rc, obtenido $rc"
    if [[ -n "$want_re" ]] && ! grep -qE -- "$want_re" <<<"$out"; then
        why="${why:+$why · }no matchea /$want_re/"
    fi
    if [[ "$want_rc" == 2 ]]; then
        grep -q 'ERROR' <<<"$out" || why="${why:+$why · }falta ERROR"
    fi
    [[ -z "$why" ]] && _pass "$desc" || _fail "$desc" "$why" "$out"
}

# check_not DESC REGEX -- ARGS... (also requires rc 0)
check_not() {
    local desc="$1" bad_re="$2" out rc why=""
    shift 2
    [[ "${1:-}" == -- ]] && shift
    out="$(_ropero "$@")"; rc=$?
    [[ "$rc" == 0 ]] || why="rc esperado 0, obtenido $rc"
    if grep -qE -- "$bad_re" <<<"$out"; then
        why="${why:+$why · }apareció /$bad_re/"
    fi
    [[ -z "$why" ]] && _pass "$desc" || _fail "$desc" "$why" "$out"
}

echo '== ropero: contrato CLI v2 =='

# Global help.
check 'sin argumentos muestra help' 0 'USO: ropero <modelo>' --
check '--help muestra help' 0 'únicamente ropero' -- --help
check 'help muestra help' 0 'install.*engines.*models.*link' -- help

# --install is self-install only. Old v1 targets must fail before effects.
check '--install instala sólo ropero' 0 'CALL ropero_link_self:' -- --install
check '--install --force llega al self installer' 0 'CALL ropero_link_self:.*FORCE=1' -- --install --force
check '--install --help específico' 0 'USO: ropero --install.*solo ropero' -- --install --help
check '--install rechaza models antiguo' 2 'argumento desconocido para --install' -- --install models
check '--install rechaza engines antiguo' 2 'argumento desconocido para --install' -- --install engines
check '--install rechaza link antiguo' 2 'argumento desconocido para --install' -- --install link
check '--install rechaza all antiguo' 2 'argumento desconocido para --install' -- --install all
check_not '--install no toca engines/modelos' 'ropero_install_|_ropero_cat_download' -- --install

# install orchestration and aliases. Empty selection is deliberately non-TTY:
# all compatible engines and all selectable models must be reached.
check 'install sin target recorre engines y modelos noTTY' 0 'CALL ropero_install_(llama|airllm|mlx):' -- install
check 'install sin target alcanza modelos' 0 'CALL _ropero_cat_download:' -- install
check 'install engines selecciona múltiples nombres' 0 'CALL ropero_install_airllm:' -- install engines llama,airllm
check 'install engines también instala llama seleccionado' 0 'CALL ropero_install_llama:.*FORCE=0' -- install engines llama,airllm
check 'install engines propaga force' 0 'CALL ropero_install_airllm:.*FORCE=1' -- install engines llama,airllm --force
check_not 'install engines no descarga modelos' '_ropero_cat_download' -- install engines llama,airllm
check 'install models selecciona múltiples nombres' 0 '▸ \[2/2\]' -- install models embed,vector
check 'install models alcanza hojas de catálogo' 0 'CALL _ropero_cat_download:.*FORCE=0' -- install models embed,vector
check 'install models propaga force' 0 'CALL _ropero_cat_download:.*FORCE=1' -- install models embed,vector --force
check_not 'install models no instala engines' 'CALL ropero_install_' -- install models embed
check 'install link relinkea hojas' 0 'CALL ropero_link_airllm:' -- install link
check_not 'install link no instala ni descarga' 'CALL ropero_install_|_ropero_cat_download' -- install link

check 'alias engines' 0 'CALL ropero_install_airllm:' -- engines airllm
check 'alias models' 0 'CALL _ropero_cat_download:' -- models embed
check 'alias link' 0 'CALL ropero_link_airllm:' -- link
check 'alias check despacha check' 1 '== ropero --check ==' -- check
check '--check alias despacha check' 1 '== ropero --check ==' -- --check

# Nested help must be command-specific, rc 0, and effect-free.
check 'help install' 0 'USO: ropero install' -- install --help
check 'help install engines específico' 0 'USO: ropero engines' -- install --help engines
check 'help engines' 0 'USO: ropero engines' -- engines --help
check 'help models' 0 'USO: ropero models' -- models --help
check 'help link' 0 'USO: ropero link' -- link --help
check 'help check' 0 'USO: ropero check' -- check --help
check 'help config' 0 'USO: ropero config' -- config --help
check 'help status' 0 'USO: ropero status' -- status --help
check 'help stop' 0 'USO: ropero stop' -- stop --help
check 'help list' 0 'USO: ropero list' -- list --help
check 'help templates' 0 'USO: ropero templates' -- templates --help
check_not 'help anidado no inicializa entorno' 'CALL ropero_env_setup' -- install models --help

# Usage errors: rc 2 and useful command usage.
check 'opción top-level inválida' 2 'opción desconocida' -- --bogus
check 'target install repetido' 2 'destino repetido' -- install engines models
check 'link rechaza selección' 2 'link no acepta selección' -- link extra
check 'check rechaza argumentos' 2 'check no acepta argumentos' -- check models
check 'list rechaza argumentos' 2 'list no acepta argumentos' -- list extra
check 'templates rechaza argumentos' 2 'templates no acepta argumentos' -- templates extra
check 'config rechaza dos argumentos' 2 'argumento desconocido' -- config llama extra
check 'config engine desconocido devuelve 2' 2 'engine desconocido' -- config inexistente

# Model launcher parsing. start_model is the effect leaf; parsing remains real.
check 'launcher sin flags pasa defaults' 0 'CALL start_model: \[embed\] \[0\] \[0\] \[0\] \[\]' -- embed
check 'launcher combina flags y puerto' 0 'CALL start_model: \[embed\] \[1\] \[1\] \[1\] \[54321\]' -- embed --background --cpu --force --port 54321
check 'launcher --help' 0 'USO: ropero <modelo>' -- embed --help
check 'launcher flag desconocido rc2' 2 'opción desconocida' -- embed --foc
check 'launcher --port sin valor' 1 'requiere un número de puerto' -- embed --port
check 'launcher --port inválido' 1 'no es un puerto válido' -- embed --port=abc
check 'modelo inexistente' 1 'Modelo desconocido' -- inexistente

# Final safety assertions.
echo
if [[ -s "$SENTINEL_LOG" ]]; then
    _fail 'seguridad: ninguna herramienta externa peligrosa' 'se alcanzó una sentinel externa' "$(cat "$SENTINEL_LOG")"
else
    _pass 'seguridad: ninguna herramienta externa peligrosa'
fi

roots=("$HOME_DIR" "$GGUF_DIR_TEST" "$SAFETENSORS_DIR_TEST" "$LOG_DIR_TEST" "$LLAMA_DIR_TEST" "$BIN_DIR_TEST" "$RUN_DIR" "$WORK/tmp")
outside=0
for root in "${roots[@]}"; do
    case "$root" in "$WORK"/*) ;; *) outside=1 ;; esac
done
if (( outside == 0 )) && [[ "$REAL_HOME" != "$HOME_DIR" ]]; then
    _pass 'seguridad: ninguna ruta de escritura configurada sale de WORK'
else
    _fail 'seguridad: roots temporales' 'algún root no está bajo WORK'
fi

if (( FAIL == 0 )); then
    printf 'PASS — %d casos; sin sentinel externa ni escrituras configuradas fuera de WORK\n' "$PASS"
    exit 0
fi

printf 'FAIL — %d ok, %d fallos\n' "$PASS" "$FAIL"
printf '  - %s\n' "${FAILED_CASES[@]}"
exit 1
