#!/usr/bin/env bash
# =============================================================================
# tests/test-argparse.sh — el contrato de parsing de la CLI de ropero.
# =============================================================================
#
# POR QUÉ ESTE FICHERO EXISTE
#
# No es tests-por-deporte. Es la red de seguridad de UN bug concreto: hasta
# 2026-10, `ropero --install models` ejecutaba el install entero —compilando
# llama.cpp si hacía falta— porque cmd_install nunca miraba "$@". Todos los
# subcomandos aceptaban cualquier cosa detrás y la tiraban a la basura sin
# decir nada. El síntoma (media hora de compilación en vez de una descarga) no
# apuntaba ni de lejos a la causa, porque el mensaje de error que podría haberlo
# delatado no existía.
#
# Por eso el battery comprueba DOS cosas de cada invocación:
#   1. a qué función se despachó y con qué argumentos (el parseo)
#   2. el código de salida (que un error de uso sea 2 y no 0)
# El segundo importa tanto como el primero: un `--install mdoels` que sale con
# 0 es indistinguible de un `--install mdoels` que salió bien.
#
# CÓMO FUNCIONA (y por qué no puede tocar tu máquina)
#
# Copia `ropero` + `ropero.d/` a un temporal e INSTRUMENTA la copia: cada
# cmd_* y cada ropero_install_*/link_* se sustituye por una función que
# imprime `CALL <nombre> <args>` y devuelve. Así se puede preguntar "¿qué haría
# este comando?" sin que la respuesta sea "compila 40 minutos".
#
# Uso: tests/test-argparse.sh [-v]     (exit 0 = todo PASS, 1 = algún FAIL)
# =============================================================================

set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

VERBOSE=0
[[ "${1:-}" == "-v" ]] && VERBOSE=1

PASS=0
FAIL=0
FAILED_CASES=()

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ropero-argparse.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

# -----------------------------------------------------------------------------
# _instrument — copia instrumentada de ropero en $WORK/ropero
# -----------------------------------------------------------------------------
# Sustituye el cuerpo de cada función por un eco. Se hace por patrón de
# definición (`^nombre() {` … hasta la llave de cierre de nivel superior) y no
# por números de línea, porque los números se mueven en cuanto se edita el
# entrypoint y un test que hay que recolocar cada vez es un test que nadie
# actualiza.
_instrument() {
    local src="$REPO/ropero" dst="$WORK/ropero"
    mkdir -p "$WORK"
    cp "$src" "$dst"
    cp -r "$REPO/ropero.d" "$WORK/ropero.d"

    python3 - "$dst" <<'PYEOF'
import io, re, sys

path = sys.argv[1]
s = io.open(path, encoding='utf-8').read()

# Funciones cuyo TRABAJO se sustituye por un `CALL <nombre> <args>` inofensivo.
#
# El criterio es "esta función hace algo", NO "esta función es un comando". Los
# cmd_* NO se stubbean: su parseo es justo lo que este fichero comprueba, y un
# test que sustituye la función que quiere verificar no verifica nada — Sólo
# comprueba que el stub se ha llamado. Lo que se stubea son las capas de abajo
# (arrancar servers, compilar, descargar 50 GB, matar procesos) para que la
# pregunta "¿qué haría este comando?" se pueda contestar sin contestarla de verdad.
STUBS = [
    # apagado: sin esto, `ropero stop all` mataría lo que hubiera vivo
    '_kill_port', '_stop_port',
    # fases del install
    '_ropero_install_engines', '_ropero_do_models',
    'ropero_download_default_models', 'ropero_env_setup', 'ropero_link_self',
    'ropero_compile_llama',
    'ropero_install_llama', 'ropero_install_airllm', 'ropero_install_tensorrt',
    'ropero_install_mlx', 'ropero_install_fastflowlm',
    'ropero_link_llamacpp', 'ropero_link_tensorrt', 'ropero_link_airllm',
    'ropero_link_mlx', 'ropero_link_fastflowlm',
    # el probe real tarda ~1s por engine y no es lo que este test comprueba
    'ropero_engine_probe',
]

# Encuentra `^fn() {` y avanza hasta su llave de cierre de primer nivel,
# contando llaves: los bodies tienen if/for anidados, así que contar líneas
# no vale y una llave de más deja el resto del fichero dentro del stub.
def cut(s, name):
    m = re.search(r'^%s\(\) \{$' % re.escape(name), s, re.M)
    if not m:
        return None
    i = m.end()
    depth = 1
    while i < len(s) and depth:
        if s[i] == '{':
            depth += 1
        elif s[i] == '}':
            depth -= 1
        i += 1
    return m.start(), i

for fn in STUBS:
    span = cut(s, fn)
    if span is None:
        continue
    a, b = span
    stub = ('%s() { printf "CALL %s:"; printf " [%%s]" "$@"; echo; return 0; }'
            % (fn, fn))
    s = s[:a] + stub + s[b:]

io.open(path, 'w', encoding='utf-8').write(s)
PYEOF
    chmod +x "$dst"
}

if ! _instrument; then
    echo "FATAL: no se pudo instrumentar la copia de ropero" >&2
    exit 1
fi

if ! bash -n "$WORK/ropero"; then
    echo "FATAL: la copia instrumentada no parsea" >&2
    exit 1
fi

# -----------------------------------------------------------------------------
# _ropero <args...> — invoca la copia instrumentada con un entorno que no
# puede tocar nada real: GGUF_DIR y LOG_DIR al temporal, AUTO_YES para que
# ninguna ropero_ask pueda quedarse esperando una tecla.
# -----------------------------------------------------------------------------
_ropero() {
    AUTO_YES=1 \
    GGUF_DIR="$WORK/nogguf" \
    SAFETENSORS_DIR="$WORK/nosa" \
    LOG_DIR="$WORK/logs" \
    LLAMA_DIR="$WORK/no-llama" \
    ROPERO_GPU_PORT=9999 \
    ROPERO_CPU_PORT=9998 \
    timeout 30 bash "$WORK/ropero" "$@" 2>&1
}

# -----------------------------------------------------------------------------
# check <descripción> <rc esperado> <grep esperado> -- <args de ropero>
# -----------------------------------------------------------------------------
check() {
    local desc="$1" want_rc="$2" want_grep="$3"
    shift 3
    [[ "${1:-}" == "--" ]] && shift

    (( VERBOSE )) && printf '  ... %s
' "$desc"
    local out rc
    out="$(_ropero "$@")"
    rc=$?

    local ok=1 why=""

    if [[ "$want_rc" != "*" && "$rc" != "$want_rc" ]]; then
        ok=0; why="rc esperado $want_rc, obtenido $rc"
    fi
    if [[ -n "$want_grep" ]] && ! grep -qE -- "$want_grep" <<<"$out"; then
        ok=0; why="${why:+$why · }no matchea /$want_grep/"
    fi
    # Un error de USO (rc 2) tiene que decir ERROR y enseñar el uso: es lo
    # que un humano (o un wrapper) lee para saber qué escribir después. Un rc
    # de ejecución (p. ej. "modelo desconocido", rc 1) tiene su propio mensaje
    # y no se le exige la palabra ERROR.
    if [[ "$want_rc" == "2" ]]; then
        grep -qE 'ERROR' <<<"$out" || { ok=0; why="${why:+$why · }aborta sin mensaje ERROR"; }
        grep -qE 'USO:' <<<"$out" || { ok=0; why="${why:+$why · }aborta sin decir cuál es el uso"; }
    fi

    if (( ok == 1 )); then
        PASS=$((PASS + 1))
        (( VERBOSE )) && printf '  ok   %s\n' "$desc"
    else
        FAIL=$((FAIL + 1))
        FAILED_CASES+=("$desc — $why")
        printf '  FAIL %s\n       → %s\n' "$desc" "$why"
        printf '       salida: %s\n' "$(head -c 400 <<<"$out" | tr '\n' '|')"
    fi
}

# -----------------------------------------------------------------------------
# check_not <descripción> <grep que NO debe aparecer> -- <args>
# -----------------------------------------------------------------------------
# La mitad del valor de este battery es negativa. Que `--install models` NO
# llame a la fase de engines no se comprueba mirando que llame a la de
# modelos: se comprueba mirando que la otra no aparece. Un `--install models`
# que además compilara llama.cpp habría "pasado" el test de arriba.
check_not() {
    local desc="$1" bad_grep="$2"
    shift 2
    [[ "${1:-}" == "--" ]] && shift

    local out
    out="$(_ropero "$@")"
    if grep -qE -- "$bad_grep" <<<"$out"; then
        FAIL=$((FAIL + 1))
        FAILED_CASES+=("$desc — pasó algo que no debía: /$bad_grep/")
        printf '  FAIL %s\n       →不该 pasar: %s\n' "$desc" "$bad_grep"
        printf '       salida: %s\n' "$(head -c 400 <<<"$out" | tr '\n' '|')"
    else
        PASS=$((PASS + 1))
        (( VERBOSE )) && printf '  ok   %s\n' "$desc"
    fi
}

# -----------------------------------------------------------------------------
# Los casos
# -----------------------------------------------------------------------------
echo "== ropero: contrato de parsing =="

# ── el bug que motivó todo ───────────────────────────────────────────────────
check "--install models → fase de modelos"          0 "_ropero_do_models"             -- --install models
check "--install engines → fase de engines"         0 "_ropero_install_engines"       -- --install engines
check "--install link → fase de relink"             '*' "relinkeando ejecutables"    -- --install link
check "--install (sin target) = all"                0 "_ropero_install_engines"       -- --install
check "--install all → engines Y modelos"           0 "_ropero_do_models"             -- --install all

# ── la mitad del contrato: lo que un target NO hace ───────────────────────
# Sin esto, `--install models` que además compila llama.cpp pasaría el test de
# arriba. Es exactamente el bug original con un disfraz nuevo.
check_not "--install models NO toca engines"        "_ropero_install_engines"  -- --install models
check_not "--install engines NO baja pesos"         "_ropero_do_models"        -- --install engines
check_not "--install link NO instala nada"          "_ropero_install_engines|_ropero_do_models" -- --install link
check_not "models NO toca engines"                  "_ropero_install_engines"  -- models
check_not "ningún target llama a pacman/sudo"       "pacman|brew install"      -- --install models
check_not "models NO pide sudo"                     "sudo"                     -- models

# ── --install --force: antes se ignoraba en silencio ────────────────────────
check "--install --force no es error"               0 "_ropero_install_engines"       -- --install --force
check "--install --force no salta los pesos" 0 "_ropero_do_models"     -- --install --force
check "--install -f (alias corto)"                  0 "_ropero_install_engines"       -- --install -f
check "--install models --force (orden indiferente)" 0 "_ropero_do_models"                -- --install models --force

# ── typos: fallan ANTES de instalar nada ───────────────────────────────────
check "typo de target (mdoels)"                     2 "target.*desconocido|argumento desconocido" -- --install mdoels
check "flag inventado en --install"                 2 "ERROR"                        -- --install --por
check "--install --help muestra el help"            0 "USO|ropero"                    -- --install --help

# ── targets que existen pero no ought combinarse ───────────────────────────
check "--check no acepta target"                    2 "ERROR"                        -- --check models
check "list no acepta argumentos"                   2 "no acepta argumentos"          -- list extra
check "templates no acepta argumentos"              2 "no acepta argumentos"          -- templates extra
check "models no acepta argumentos"                 2 "no acepta argumentos"          -- models extra

# ── ropero models: subcomando top-level ────────────────────────────────────
check "models descarga los pesos"                   0 "_ropero_do_models"             -- models

# ── stop ───────────────────────────────────────────────────────────────────
check "stop sin args = all (no es error de uso)"      0 ""                             -- stop
check "stop <modelo> recorre los puertos"            '*' "no encontrado en ningún puerto" -- stop embed
check "stop con guion (nombre de script)"           '*' "no encontrado en ningún puerto" -- stop gptoss-high
check "stop con flag = error de uso"                2 "ERROR"                        -- stop embed --force
check "stop con 2 args = error de uso"              2 "como mucho un argumento"       -- stop embed gptoss-high
check "stop de modelo inexistente = error"          2 "modelo desconocido"           -- stop noexiste

# ── opciones desconocidas ──────────────────────────────────────────────────
check "--badflag = opción desconocida"              2 "opción desconocida"           -- --badflag
check "opción de modelo desconocida"                2 "opción desconocida"           -- --foc
check "modelo inexistente = Modelo desconocido"     1 "Modelo desconocido"           -- noexiste

# ── status: el que YA funcionaba, fijado para que no se rompa ──────────────
check "status --badopt = error de uso (unificado)" 2 "opción desconocida"          -- status --badopt
check "status --watch abc = error de ms"           2 "requiere milisegundos"        -- status --watch abc

# ── --port ─────────────────────────────────────────────────────────────────
check "embed --port (sin valor) = error"            1 "requiere un número de puerto"  -- embed --port
check "embed --port=abc = puerto inválido"          1 "puerto válido"                -- embed --port=abc
check "--port repetido avisa y gana el último"      '*' "AVISO: --port repetido"      -- embed --port 9995 --port 9996

# ── modelo <--help> ────────────────────────────────────────────────────────
check "embed --help responde con el uso"            0 "USO: ropero <modelo>"         -- embed --help

# ── help ───────────────────────────────────────────────────────────────────
check "help"                                        0 "ropero"                       -- help
check "--help"                                      0 "ropero"                       -- --help
check "sin argumentos = help"                       0 "ropero"                       --

# -----------------------------------------------------------------------------
echo
if (( FAIL == 0 )); then
    printf 'PASS — %d casos\n' "$PASS"
    exit 0
fi
printf 'FAIL — %d ok, %d fallos\n' "$PASS" "$FAIL"
exit 1