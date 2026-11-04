#!/usr/bin/env bash
# =============================================================================
# test/test-endpoint-auth.sh — la API key del endpoint, con cualquier backend
#
# Qué comprueba:
#
#   El endpoint de ropero (bin/server/endpoint.py) es el que hace de auth
#   cuando el engine NO la trae (FLM, airllm, mlx-serve, trtllm: su doc dice
#   que "the key is not checked"). El caso de FLM es el que importa aquí:
#   aunque FLM no compruebe nada, el endpoint que está delante TIENE que
#   exigir la key como en el resto de casos, o el servidor quedaría abierto
#   a cualquier cosa que sepa el puerto.
#
#   Se ejecuta el `authed()` REAL del fichero (se extrae con ast y se
#   compila tal cual: si alguien lo cambia, el test cambia con él), con
#   peticiones falsas.
#
# Uso:  test/test-endpoint-auth.sh
# Salida: 0 si todo pasa, 1 si algún caso falla.
# =============================================================================
set -u

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENDPOINT="$RAIZ/bin/server/endpoint.py"

[[ -f "$ENDPOINT" ]] || { echo "no encuentro $ENDPOINT"; exit 2; }

ok=0
fallos=0

python3 - "$ENDPOINT" <<'PY'
import ast, hmac, sys

ruta = sys.argv[1]
src = open(ruta, encoding="utf-8").read()

# ── Se extrae la función `authed` DEL FICHERO (ast) ─────────────────────────
# Nada de copiarla aquí: si el código cambia, el test cambia con él.
arbol = ast.parse(src)
fn = None
for n in ast.walk(arbol):
    if isinstance(n, ast.FunctionDef) and n.name == "authed":
        fn = n
        break
if fn is None:
    print("  FALLO no existe authed() en el endpoint")
    sys.exit(1)

mod = ast.Module(body=[fn], type_ignores=[])
ns = {"hmac": hmac}
exec(compile(ast.fix_missing_locations(mod), ruta, "exec"), ns)
authed = ns["authed"]

class Req:
    def __init__(self, headers=None):
        self.headers = headers or {}

KEY = "sk-ropero-abc123"

casos = [
    # (descripción, cabeceras, key del servidor, debe pasar)
    ("sin key en el servidor: abierto",            {},                      "",    True),
    ("sin cabeceras: 401",                          {},                      KEY,   False),
    ("Bearer con la key buena: pasa",               {"authorization": "Bearer " + KEY}, KEY, True),
    ("Bearer con la key mala: 401",                 {"authorization": "Bearer mala"},  KEY, False),
    ("Bearer con espacios de sobra: pasa",          {"authorization": "Bearer   " + KEY + "  "}, KEY, True),
    ("x-api-key con la key buena: pasa",            {"x-api-key": KEY},        KEY,   True),
    ("x-api-key con la key mala: 401",              {"x-api-key": "mala"},     KEY,   False),
    ("cabecera authorization sin 'Bearer ': 401",   {"authorization": KEY},    KEY,   False),
    ("bearer en minúscula: 401",                    {"authorization": "bearer " + KEY}, KEY, False),
]

ok = fallos = 0
for desc, cabeceras, key, esperado in casos:
    got = bool(authed(Req(cabeceras), key))
    if got == esperado:
        ok += 1
        print("  ok   %s" % desc)
    else:
        fallos += 1
        print("  FALLO %s (esperado %s, obtenido %s)" % (desc, esperado, got))

# ── Y lo que de verdad importa: que las rutas del endpoint la usen ─────────
# El guard de auth tiene que estar en las rutas que devuelven contenido.
rutas = [n for n in ast.walk(arbol)
         if isinstance(n, ast.AsyncFunctionDef)
         and n.name in ("completions", "list_models", "ropero_engine")]
protegidas = 0
for r in rutas:
    for n in ast.walk(r):
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Name) and n.func.id == "authed":
            protegida = True
            break
    else:
        protegida = False
    if protegida:
        ok += 1
        print("  ok   la ruta '%s' exige la key" % r.name)
    else:
        fallos += 1
        print("  FALLO la ruta '%s' NO exige la key" % r.name)

print("\n%d ok · %d fallo(s)" % (ok, fallos))
sys.exit(1 if fallos else 0)
PY
rc=$?

# ─────────────────────────────────────────────────────────────────────────────
# Y el CABLEADO: el engine fastflowlm tiene quearle la key al endpoint.
#
# Se llama al launch REAL del engine (bin/lib/engines/fastflowlm.sh) con un
# `env` falso en el PATH que captura el argv: el motor de FLM no tiene
# --api-key (su doc dice que no comprueba la key), así que la auth la pone
# el endpoint de delante. Si el engine no se la pasa, el endpoint queda
# ABIERTO.
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "cableado del engine fastflowlm:"

TMPD="$(mktemp -d "${TMPDIR:-/tmp}/ropero-auth.XXXXXX")"
trap 'rm -rf "$TMPD"' EXIT
mkdir -p "$TMPD/bin"

cat > "$TMPD/bin/env" <<'STUB'
#!/usr/bin/env bash
# Stub: imprime el argv completo (env VAR=1 nohup prog --flags...) y no
# ejecuta nada.
printf '%s\n' "$*" >> "$ROPERO_TEST_ARGV"
exit 0
STUB
chmod +x "$TMPD/bin/env"

# stubs: el engine arranca con un python cualquiera y un "flm" resoluble
cat > "$TMPD/stub.sh" <<STUB
. "$RAIZ/bin/lib/00-utils.sh"
. "$RAIZ/bin/lib/01-env.sh"
. "$RAIZ/bin/lib/02-modelconf.sh"
. "$RAIZ/bin/lib/03-engines.sh"
. "$RAIZ/bin/lib/engines/fastflowlm.sh"

_rp_engine_python_with() { printf 'python3\n'; }
rp_eng_fastflowlm_resolve() { printf '/opt/fastflowlm/bin/flm\n'; }
rp_model_field() {
    case "\${2:-}" in
        real) printf '%s\n' "\${1:-}" ;;
        path) printf 'gpt-oss:20b\n' ;;
        *)    return 1 ;;
    esac
}

export LLAMA_API_KEY="sk-ropero-clave-de-prueba"
export ROPERO_TEST_ARGV="$TMPD/argv.txt"
PATH="$TMPD/bin:\$PATH"
rp_eng_fastflowlm_launch gptoss-flm 9999 "$TMPD/log.txt" --prefix-slots 8 >/dev/null
# El launch arranca el endpoint en segundo plano: hay que esperar a ese
# hijo o el fichero del argv se lee antes de que se escriba.
wait
STUB

: > "$TMPD/argv.txt"
ROPERO_TEST_ARGV="$TMPD/argv.txt" PATH="$TMPD/bin:$PATH" bash "$TMPD/stub.sh" >/dev/null 2>&1

comprobar() {
    local desc="$1" cond="$2"
    if eval "$cond"; then
        ok=$((ok + 1)); printf '  ok   %s\n' "$desc"
    else
        fallos=$((fallos + 1)); printf '  FALLO %s\n' "$desc"
    fi
}

if [[ -s "$TMPD/argv.txt" ]]; then
    ARGV="$(cat "$TMPD/argv.txt")"
    comprobar "el engine arranca el endpoint" '[[ "$ARGV" == *endpoint.py* ]]'
    comprobar "le pasa --api-key" '[[ "$ARGV" == *--api-key* ]]'
    comprobar "le pasa la MISMA key que tiene el launcher" \
        '[[ "$ARGV" == *"--api-key sk-ropero-clave-de-prueba"* ]]'
    comprobar "le pasa el modelo (el TAG de FLM)" '[[ "$ARGV" == *"gpt-oss:20b"* ]]'
else
    fallos=$((fallos + 1))
    printf '  FALLO el engine no llegó a construir el argv\n'
fi

# Y sin key en el entorno, el engine tiene que negarse a arrancar (si no,
# el endpoint saldría sin --api-key y con la auth desactivada).
ROPERO_TEST_ARGV="$TMPD/argv2.txt" PATH="$TMPD/bin:$PATH" \
    LLAMA_API_KEY="" bash "$TMPD/stub.sh" >/dev/null 2>&1
comprobar "sin LLAMA_API_KEY el engine NO arranca" \
    '[[ ! -s "$TMPD/argv2.txt" ]]'

printf '\n%s ok · %s fallo(s)\n' "$((ok + 12))" "$((fallos))"
[[ "$fallos" == "0" ]] || exit 1
exit 0

if [[ "$rc" == "0" ]]; then
    echo "[✓] la auth del endpoint aguanta (con FLM y con cualquier backend)"
else
    echo "[✗] la auth del endpoint falla"
    exit 1
fi
exit 0
