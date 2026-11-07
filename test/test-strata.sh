#!/usr/bin/env bash
# =============================================================================
# test/test-strata.sh — el engine "strata" de ropero
# =============================================================================
# Strata no es un server: es un Terminal que baja un motor, deja el modelo en
# disco y sirve una API OpenAI-compatible. Por eso su engine tiene tres
# piezas que no tienen los demás, y este test las comprueba una a una:
#
#   1. La INSTALACIÓN es la oficial (el install.sh del release), no un
#      `git clone` de un repo que puede no compilar.
#   2. El ARRANQUE va detrás de endpoint.py (la clave la pone el proxy) y a
#      través del puente bin/server/strata-run.sh: `strata launch --no-tui`
#      DEVUELVE cuando el server ya sirve, así que el proceso que lo
#      supervisa tiene que ser otro (y ése es el que para el modelo).
#   3. El MODELO es un par familia/paquete del catálogo de Strata
#      ("qwen/q2_0"), NO una ruta: los archivos los baja Strata.
#
# El test es HERMETICO: no toca la red ni el disco del usuario.
#   · el CLI de Strata es falso (test/fakes/strata/strata);
#   · el endpoint.py es falso (anota su argv y se queda vivo);
#   · el puente es el de verdad (es lo que hay que probar).
#
# USO: ./test/test-strata.sh
# =============================================================================
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FAKES="$ROOT/test/fakes/strata"
TMP="${TMPDIR:-/tmp}/strata-test.$$"
mkdir -p "$TMP" || exit 2
trap 'rm -rf "$TMP"' EXIT

fallos=0
ok()  { printf 'ok   %s\n' "$1"; }
mal() { printf 'FALLO %s\n' "$1" >&2; fallos=$(( fallos + 1 )); }
comprobar() { if eval "$2"; then ok "$1"; else mal "$1"; fi; }

# ── el árbol falso ──────────────────────────────────────────────────────────
#
# ROPERO_ROOT apunta a un árbol con un endpoint.py falso: el engine arranca
# SIEMPRE $ROPERO_ROOT/bin/server/endpoint.py, así que basta con poner ahí
# un script que anote su argv. El puente (strata-run.sh) se copia tal cual:
# es justo lo que hay que probar.
export ROPERO_ROOT="$TMP/root"
export FAKE_ENDPOINT_ARGV="$TMP/endpoint.argv"
export FAKE_STRATA_LOG="$TMP/strata.log"
export FAKE_STRATA_PLAN="$TMP/plan.json"
export HOME="$TMP/home"
export BIN_DIR="$TMP/bin"
export FARM_DIR="$TMP/farm"
export PATH="$FAKES:$PATH"
mkdir -p "$HOME" "$BIN_DIR" "$FARM_DIR" "$ROPERO_ROOT/bin/server"
cp "$ROOT/bin/server/strata-run.sh" "$ROPERO_ROOT/bin/server/strata-run.sh"
chmod +x "$ROPERO_ROOT/bin/server/strata-run.sh"
cat >"$ROPERO_ROOT/bin/server/endpoint.py" <<'FAKE'
#!/usr/bin/env python3
# El endpoint falso: anota su argv y se queda vivo (como el proxy real).
# Es PYTHON a propósito: el engine lo arranca con un python3, así que un
# script de bash aquí no se ejecutaría (y el test no vería ningún argv).
import os, sys, time
with open(os.environ["FAKE_ENDPOINT_ARGV"], "w") as fh:
    fh.write("\n".join(sys.argv[1:]))
time.sleep(30)
FAKE
chmod +x "$ROPERO_ROOT/bin/server/endpoint.py"

# El plan que devuelve el strata falso (la forma oficial de preguntar).
cat >"$FAKE_STRATA_PLAN" <<'PLAN'
{"schema_version": 1, "ok": true, "plan": {"settings": {"family": "qwen", "model": "Q2_0"},
 "warnings": ["Qwen3.8-Flash-Next Q2_0 does not fit this computer."],
 "model": {"title": "Qwen3.8-Flash-Next Q2_0", "family": "qwen", "model": "Q2_0", "model_gb": 66.4},
 "download": {"gb": 72.5, "parts": [{"kind": "weights", "label": "the model", "gb": 66.4}]}}}
PLAN

# ── el engine, con lo mínimo de ropero para poder cargarlo ────────────────
#
# 03-engines.sh se sourcea ANTES: trae sus propias versiones de
# _rp_engine_python_with y compañía, y sourcearlo después machacaría los
# stubs de abajo (el engine acabaría buscando un python con fastapi de
# verdad, que en el sandbox no hay).
#
# shellcheck source=/dev/null
. "$ROOT/bin/lib/03-engines.sh" 2>/dev/null || true

rp_msg()  { printf '%s\n' "$*"; }
rp_step() { printf '%s\n' "$*"; }
rp_detect_os() { printf 'linux\n'; }
_rp_engine_deps_ok() { local d; for d in "$@"; do command -v "$d" >/dev/null 2>&1 || return 1; done; return 0; }
_rp_engine_http_ok() { return 1; }
_rp_engine_python_with() { printf 'python3\n'; }
# rp_model_field: el modelo de prueba declara path y context.
rp_model_field() {
    case "${2:-}" in
        path)    printf 'qwen/Q2_0' ;;
        context) printf '32768' ;;
        real)    printf 'strata-qwen38-q2' ;;
        *)       return 1 ;;
    esac
}
# shellcheck source=/dev/null
. "$ROOT/bin/lib/engines/strata.sh"

echo "== los ocho hooks =="
for hook in label compatible deps install resolve probe native_auth launch; do
    comprobar "existe rp_eng_strata_$hook" "declare -F rp_eng_strata_$hook >/dev/null"
done
# install_model es OPCIONAL en el contrato, pero sin él el install del modelo
# no puede decir nada: aquí se comprueba que existe y que es el hook del
# engine de strata.
comprobar "existe rp_eng_strata_install_model" "declare -F rp_eng_strata_install_model >/dev/null"

echo "== el id canónico =="
comprobar "strata es un engine conocido" "rp_engine_canonical strata >/dev/null"
comprobar "strata-engine se canóniza a strata" "[[ \$(rp_engine_canonical strata-engine) == strata ]]"
comprobar "strata aparece en la lista de engines" "rp_engine_list | grep -qx strata"

echo "== la etiqueta y las dependencias =="
comprobar "la etiqueta no está vacía" "[[ -n \$(rp_eng_strata_label) ]]"
comprobar "las deps son curl" "[[ \$(rp_eng_strata_deps) == curl ]]"
comprobar "la auth la pone el proxy (no el engine)" "[[ \$(rp_eng_strata_native_auth) == no ]]"
comprobar "es compatible en Linux" "rp_eng_strata_compatible"

echo "== resolver el ejecutable =="
comprobar "encuentra el strata del PATH" "[[ \$(rp_eng_strata_resolve) == *strata ]]"
( PATH="/nonexistent" BIN_DIR=/nonexistent FARM_DIR=/nonexistent
  if rp_eng_strata_resolve >/dev/null 2>&1; then exit 1; else exit 0; fi ) \
    && ok "sin Strata instalado no resuelve nada" \
    || mal "resuelve algo que no existe"

echo "== el probe =="
comprobar "con Strata pero sin puerto el probe dice down" "[[ \$(rp_eng_strata_probe 9999) == down ]]"
( PATH="/nonexistent" BIN_DIR=/nonexistent FARM_DIR=/nonexistent
  if [[ "$(rp_eng_strata_probe 9999 2>/dev/null)" == "missing" ]]; then exit 0; else exit 1; fi ) \
    && ok "sin Strata el probe dice missing" \
    || mal "sin Strata el probe no dice missing"

echo "== la instalación es la oficial =="
# Sin Strata en el PATH (para que el hook no diga "ya instalado") y con un
# curl falso: el instalador se descarga a un fichero y se ejecuta con sh.
export FAKE_STRATA_INSTALLED="$TMP/instalado.marca"
# El curl falso va en su propio directorio: el PATH de la prueba NO puede
# llevar el strata falso (si lo llevara, el hook diría "ya instalado" y no
# descargaría nada).
mkdir -p "$TMP/curl"
cp "$FAKES/curl" "$TMP/curl/curl"
chmod +x "$TMP/curl/curl"
salida="$( PATH="$TMP/curl:/usr/bin:/bin" BIN_DIR=/nonexistent FARM_DIR=/nonexistent \
            rp_eng_strata_install 2>&1)"
comprobar "avisa de que no encuentra el ejecutable tras instalar" \
    "printf '%s' \"\$salida\" | grep -qi 'no encuentro el ejecutable'"
comprobar "el instalador se ejecutó" "[[ -f \"\$FAKE_STRATA_INSTALLED\" ]]"

echo "== el install_model enseña el plan =="
salida="$(rp_eng_strata_install_model strata-qwen38-q2 2>&1)"
comprobar "dice el título del modelo" "printf '%s' \"\$salida\" | grep -q 'Qwen3.8-Flash-Next Q2_0'"
comprobar "dice lo que hay que bajar" "printf '%s' \"\$salida\" | grep -q '72.5'"
comprobar "dice que lo baja Strata al arrancar" "printf '%s' \"\$salida\" | grep -qi 'arranc'"

echo "== el arranque: el argv del endpoint =="
: >"$FAKE_ENDPOINT_ARGV"
pid="$(rp_eng_strata_launch strata-qwen38-q2 9998 "$TMP/strata.log" --vision off --tune skip 2>/dev/null)"
comprobar "devuelve un PID" "[[ \"\$pid\" =~ ^[0-9]+$ ]]"
# El endpoint falso escribe su argv y se queda vivo: hay que darle un momento.
i=0
while [[ ! -s "$FAKE_ENDPOINT_ARGV" ]] && [[ $i -lt 50 ]]; do
    sleep 0.1
    i=$(( i + 1 ))
done
argv="$(cat "$FAKE_ENDPOINT_ARGV" 2>/dev/null)"
comprobar "usa el backend strata" "printf '%s' \"\$argv\" | grep -qx -- '--backend'"
comprobar "el backend es strata" "printf '%s' \"\$argv\" | grep -A1 -x -- '--backend' | grep -qx 'strata'"
comprobar "el modelo es el par familia/paquete" "printf '%s' \"\$argv\" | grep -qx 'qwen/Q2_0'"
comprobar "el id publicado es el de ropero" "printf '%s' \"\$argv\" | grep -qx 'strata-qwen38-q2'"
comprobar "el puerto interno es el externo + 1000" "printf '%s' \"\$argv\" | grep -qx '10998'"
comprobar "el contexto es el del modelo" "printf '%s' \"\$argv\" | grep -qx '32768'"
comprobar "el puente es el binario del backend" "printf '%s' \"\$argv\" | grep -qx -- '--backend-bin'"
comprobar "los flags del TOML van como --extra-arg" "printf '%s' \"\$argv\" | grep -qx -- '--extra-arg'"
comprobar "el flag de visión llega al backend" "printf '%s' \"\$argv\" | grep -qx 'off'"
kill "$pid" 2>/dev/null || true

echo "== el puente (strata-run.sh) =="
: >"$FAKE_STRATA_LOG"
"$ROPERO_ROOT/bin/server/strata-run.sh" \
    "$(command -v strata)" qwen Q2_0 10998 32768 "ropero-strata-qwen38-q2" \
    --vision off >"$TMP/puente.log" 2>&1 &
puente_pid=$!
i=0
while [[ ! -s "$FAKE_STRATA_LOG" ]] && [[ $i -lt 50 ]]; do
    sleep 0.1
    i=$(( i + 1 ))
done
log="$(cat "$FAKE_STRATA_LOG" 2>/dev/null)"
comprobar "el puente lanza el launch con la familia" "printf '%s' \"\$log\" | grep -q -- '--family qwen'"
comprobar "el puente lanza el launch con el paquete" "printf '%s' \"\$log\" | grep -q -- '--model Q2_0'"
comprobar "el puente pasa el puerto" "printf '%s' \"\$log\" | grep -q -- '--port 10998'"
comprobar "el puente pasa el contexto" "printf '%s' \"\$log\" | grep -q -- '--context 32768'"
comprobar "el puente sólo local" "printf '%s' \"\$log\" | grep -q -- '--access local'"
comprobar "el puente sin clave (la pone el proxy)" "printf '%s' \"\$log\" | grep -q -- '--no-api-key'"
comprobar "el puente guarda un preset (para pararlo)" "printf '%s' \"\$log\" | grep -q -- '--save-preset ropero-strata-qwen38-q2'"
comprobar "los flags extra del TOML llegan" "printf '%s' \"\$log\" | grep -q -- '--vision off'"

# El puente tiene que SEGUIR VIVO mientras el modelo sirve.
sleep 0.5
comprobar "el puente sigue vivo con el modelo sirviendo" "kill -0 $puente_pid 2>/dev/null"

# Y al matarlo, tiene que PARAR lo que arrancó (y sólo eso).
: >"$FAKE_STRATA_LOG"
kill -TERM "$puente_pid" 2>/dev/null || true
i=0
while [[ $i -lt 30 ]] && ! grep -q -- '--stop' "$FAKE_STRATA_LOG" 2>/dev/null; do
    sleep 0.1
    i=$(( i + 1 ))
done
log="$(cat "$FAKE_STRATA_LOG" 2>/dev/null)"
comprobar "al morir para el preset que arrancó" "printf '%s' \"\$log\" | grep -q -- '--preset ropero-strata-qwen38-q2 --stop'"
wait "$puente_pid" 2>/dev/null || true

echo
if [[ "$fallos" -eq 0 ]]; then
    printf 'todo ok\n'
else
    printf '%s fallo(s)\n' "$fallos" >&2
fi
exit "$fallos"
