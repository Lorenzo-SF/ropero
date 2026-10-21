#!/usr/bin/env bash
# Contract and safety tests for ropero's v2 CLI parser.

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
BASH_BIN="$(command -v bash)"
# Un python que FUNCIONE. El shim de asdf puede estar en el PATH y estar roto
# ("No version is set for command python3"), así que se prueban los candidatos
# en vez de coger el primero que aparece.
PYTHON_BIN=""
for _cand in python3 python /usr/bin/python3 /usr/local/bin/python3 /bin/python3; do
    if "$_cand" -c 'import sys' >/dev/null 2>&1; then
        PYTHON_BIN="$("$_cand" -c 'import sys; print(sys.executable)')"
        break
    fi
done
if [[ -z "$PYTHON_BIN" ]]; then
    for _cand in "$HOME"/.asdf/installs/python/*/bin/python3; do
        if [[ -x "$_cand" ]] && "$_cand" -c 'import sys' >/dev/null 2>&1; then
            PYTHON_BIN="$("$_cand" -c 'import sys; print(sys.executable)')"
            break
        fi
    done
fi
[[ -n "$PYTHON_BIN" ]] || { printf 'FATAL: no hay python3 utilizable\n' >&2; exit 1; }

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
# python3 NO se bloquea: los modelos se configuran en TOML y su lector es un
# script de Python. Lo que se bloquea es todo lo que puede RED (curl, wget,
# git, hf, pip) o tocar el sistema. El lector de TOML no hace red.
for tool in curl wget git cmake ninja make brew pacman paru sudo pkg apt apt-get \
    dnf pip pip3 uv hf huggingface-cli flm; do
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
    # Estructura nueva: el entrypoint y los módulos van en bin/, los modelos en
    # models/ y el servidor de endpoints en endpoints/.
    # "cp -R origen destino" copia DENTRO de destino si destino ya existe
    # ($WORK/bin existe: es el BIN_DIR de pruebas). Por eso se copia el
    # contenido con "origen/." y no el directorio.
    # Ojo: $WORK/models YA existe (porque GGUF_DIR_TEST es $WORK/models/gguf).
    # "cp -R origen destino" con destino existente copia DENTRO de él, y
    # acabaría en $WORK/models/models/. Por eso se copia el contenido.
    mkdir -p "$WORK/models" "$WORK/endpoints"
    cp -R "$REPO/bin/." "$WORK/bin/"
    cp -R "$REPO/models/." "$WORK/models/"
    cp -R "$REPO/endpoints/." "$WORK/endpoints/"
    "$PYTHON_BIN" - "$WORK/bin/ropero" <<'PY'
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
        # El bundle deja además los ficheros SECUNDARIOS (MTP, mmproj). El stub
        # tiene que imitar al descargador real: si no, la comprobación de "el
        # artefacto está en disco" falla por los aux que faltan y el install
        # se da por roto con los pesos ahí.
        local aux
        while read -r aux; do
            [[ -n "$aux" ]] || continue
            mkdir -p "$(dirname "$aux")"
            printf stub >"$aux"
        done < <(_ropero_cat_aux_paths "$i")
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
    chmod +x "$WORK/bin/ropero"
}

if ! _instrument || ! "$BASH_BIN" -n "$WORK/bin/ropero"; then
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
        _watchdog "$BASH_BIN" "$WORK/bin/ropero" "$@" 2>&1
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

# check_not_rc DESC WANT_RC BAD_RE -- ARGS...
# Como check_not, pero fijando el código de salida: hay casos de uso que
# deben fallar (rc 2) Y no dejar rastro.
check_not_rc() {
    local desc="$1" want_rc="$2" bad_re="$3" out rc why=""
    shift 3
    [[ "${1:-}" == -- ]] && shift
    out="$(_ropero "$@")"; rc=$?
    [[ "$rc" == "$want_rc" ]] || why="rc esperado $want_rc, obtenido $rc"
    if grep -qE -- "$bad_re" <<<"$out"; then
        why="${why:+$why · }apareció /$bad_re/"
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
check '--help muestra help' 0 'install\.sh' -- --help
check 'help muestra help' 0 'install.*engines.*models.*link' -- help

# El entrypoint ya NO se autoinstala: eso lo hace install.sh (curl … | bash).
# `--install` y `--check` con guiones son, por tanto, opciones desconocidas, y
# tienen que fallar ANTES de cualquier efecto. OJO: `install` (sin guiones)
# sigue existiendo y es otra cosa: instala engines y modelos.
check '--install ya no existe' 2 'opción desconocida' -- --install
check '--install --force ya no existe' 2 'opción desconocida' -- --install --force
check '--install --help ya no existe' 2 'opción desconocida' -- --install --help
check '--install rechaza models antiguo' 2 'opción desconocida' -- --install models
check '--install rechaza engines antiguo' 2 'opción desconocida' -- --install engines
check '--install rechaza link antiguo' 2 'opción desconocida' -- --install link
check '--install rechaza all antiguo' 2 'opción desconocida' -- --install all
check_not_rc '--install no toca engines/modelos' 2 'ropero_install_|_ropero_cat_download|CALL ropero_link_self' -- --install

# install orchestration and aliases. Empty selection is deliberately non-TTY:
# all compatible engines and all selectable models must be reached.
check 'install sin target recorre engines y modelos noTTY' 0 'CALL ropero_install_(llama|airllm|mlx):' -- install
check 'install sin target alcanza modelos' 0 'CALL _ropero_cat_download:' -- install
check 'install engines selecciona múltiples nombres' 0 'CALL ropero_install_airllm:' -- install engines llama,airllm
check 'install engines también instala llama seleccionado' 0 'CALL ropero_install_llama:.*FORCE=0' -- install engines llama,airllm
check 'install engines propaga force' 0 'CALL ropero_install_airllm:.*FORCE=1' -- install engines llama,airllm --force
check_not 'install engines no descarga modelos' '_ropero_cat_download' -- install engines llama,airllm
check 'install models selecciona múltiples nombres' 0 '▸ \[2/2\]' -- install models gemma,gptoss
check 'install models alcanza hojas de catálogo' 0 'CALL _ropero_cat_download:.*FORCE=0' -- install models gemma,gptoss
check 'install models propaga force' 0 'CALL _ropero_cat_download:.*FORCE=1' -- install models gemma,gptoss --force
check_not 'install models no instala engines' 'CALL ropero_install_' -- install models gemma
check 'install link relinkea hojas' 0 'CALL ropero_link_airllm:' -- install link
check_not 'install link no instala ni descarga' 'CALL ropero_install_|_ropero_cat_download' -- install link

check 'alias engines' 0 'CALL ropero_install_airllm:' -- engines airllm
check 'alias models' 0 'CALL _ropero_cat_download:' -- models gemma
check 'alias link' 0 'CALL ropero_link_airllm:' -- link
check 'alias check despacha check' 0 '== ropero check ==' -- check
# `--check` (con guiones) ya no existe: la verificación la hace install.sh al
# instalar. `check` (sin guiones) sigue siendo el diagnóstico.
check '--check ya no existe' 2 'opción desconocida' -- --check

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
check 'help template' 0 'USO: ropero template' -- template --help
check_not 'help anidado no inicializa entorno' 'CALL ropero_env_setup' -- install models --help

# Usage errors: rc 2 and useful command usage.
check 'opción top-level inválida' 2 'opción desconocida' -- --bogus
check 'target install repetido' 2 'destino repetido' -- install engines models
check 'link rechaza selección' 2 'link no acepta selección' -- link extra
check 'check rechaza argumentos' 2 'check no acepta argumentos' -- check models
check 'list rechaza argumentos' 2 'list no acepta argumentos' -- list extra
check 'template rechaza argumentos' 2 'template no acepta argumentos' -- template extra
check 'config rechaza dos argumentos' 2 'argumento desconocido' -- config llama extra
check 'config engine desconocido devuelve 2' 2 'engine desconocido' -- config inexistente

# Model launcher parsing. start_model is the effect leaf; parsing remains real.
check 'launcher sin flags pasa defaults' 0 'CALL start_model: \[qwencoder\] \[0\] \[0\] \[0\] \[\]' -- qwencoder
check 'launcher combina flags y puerto' 0 'CALL start_model: \[qwencoder\] \[1\] \[1\] \[1\] \[54321\]' -- qwencoder --background --cpu --force --port 54321
check 'launcher --help' 0 'USO: ropero <modelo>' -- qwencoder --help
check 'launcher flag desconocido rc2' 2 'opción desconocida' -- qwencoder --foc
check 'launcher --port sin valor' 1 'requiere un número de puerto' -- qwencoder --port
check 'launcher --port inválido' 1 'no es un puerto válido' -- qwencoder --port=abc
check 'modelo inexistente' 1 'Modelo desconocido' -- inexistente

# -----------------------------------------------------------------------------
# NIVEL, FLAG Y OPCIÓN QUE DECLARA EL MODELO
#
# El CLI no tiene vocabulario propio: lo que el .sh del modelo no declara, es
# un error de uso. Estos casos fijan ese contrato:
#   · --high/--medium/--low   → nivel (gptoss, gemma)
#   · --vision                → flag booleano (gemma)
#   · --n-cpu-moe N           → opción con valor (gemma)
# -----------------------------------------------------------------------------
check 'launcher nivel del modelo' 0 'CALL start_model: \[gptoss\] \[0\] \[0\] \[0\] \[\] \[high\]' -- gptoss --high
check 'launcher nivel por defecto si no se pide' 0 'CALL start_model: \[gptoss\] \[0\] \[0\] \[0\] \[\] \[\]' -- gptoss
check 'launcher flag del modelo' 0 'CALL start_model: \[gemma\] \[0\] \[0\] \[0\] \[\] \[\] \[vision\]' -- gemma --vision
check 'launcher opcion del modelo' 0 'CALL start_model: \[gemma\] \[0\] \[0\] \[0\] \[\] \[\] \[\] \[n-cpu-moe=8\]' -- gemma --n-cpu-moe 8
check 'launcher opcion con igual' 0 'CALL start_model: \[gemma\].*\[n-cpu-moe=8\]' -- gemma --n-cpu-moe=8
check 'launcher nivel no declarado es error' 2 'opción desconocida' -- qwencoder --high
check 'launcher flag no declarado es error' 2 'opción desconocida' -- qwencoder --vision
check 'launcher opcion no declarada es error' 2 'opción desconocida' -- qwencoder --n-cpu-moe 8
check 'launcher opcion sin valor es error' 2 'requiere un valor' -- gemma --n-cpu-moe

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
