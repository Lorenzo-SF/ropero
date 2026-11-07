#!/usr/bin/env bash
# =============================================================================
# test/test-perfil.sh — el perfil de energía mientras hay un modelo sirviendo
# =============================================================================
# El perfil de energía es del USUARIO (el setup no lo toca: docs §2.9). Pero
# con un modelo cargándose la CPU hace trabajo de verdad (el prompt y, sobre
# todo, el DEQUANT del modo low-RAM, donde los expertos del SSD se cuantizan
# en la CPU), y en un portátil el governor "balanced" ahorra batería a costa
# de la inferencia.
#
# Por eso ropero puede moverlo, pero SÓLO si se pide (ROPERO_PERFIL_SERVE) y
# SIEMPRE devolviendo el anterior al parar. Si el launch muere con -9, el
# valor anterior queda en el fichero de estado y `ropero stop` lo devuelve
# igual.
#
# El test es HERMETICO: el power-profiles-daemon es falso (un fichero que
# guarda el perfil actual) y no se toca el perfil real.
#
# USO: ./test/test-perfil.sh
# =============================================================================
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="${TMPDIR:-/tmp}/perfil-test.$$"
mkdir -p "$TMP" || exit 2
trap 'rm -rf "$TMP"' EXIT

fallos=0
ok()  { printf 'ok   %s\n' "$1"; }
mal() { printf 'FALLO %s\n' "$1" >&2; fallos=$(( fallos + 1 )); }
comprobar() { if eval "$2"; then ok "$1"; else mal "$1"; fi; }

# ── el power-profiles-daemon falso ──────────────────────────────────────────
export FAKE_PPD_STATE="$TMP/perfil"
mkdir -p "$TMP/bin"
cat >"$TMP/bin/powerprofilesctl" <<'FAKE'
#!/usr/bin/env bash
# El power-profiles-daemon falso: guarda el perfil en un fichero.
case "${1:-}" in
    get) cat "${FAKE_PPD_STATE:-/dev/null}" 2>/dev/null || echo balanced ;;
    set) printf '%s\n' "${2:-}" >"${FAKE_PPD_STATE:-/dev/null}" ;;
    list) echo "balanced"; echo "performance" ;;
    *) : ;;
esac
exit 0
FAKE
chmod +x "$TMP/bin/powerprofilesctl"

# ── el entorno ──────────────────────────────────────────────────────────────
export HOME="$TMP/home"
export ROPERO_ROOT="$ROOT"
# 01-env.sh exige la clave (con "set -u"): aquí no se usa para nada.
export LLAMA_API_KEY="sk-test-perfil"
export STATE_DIR="$TMP/state"
export PATH="$TMP/bin:$PATH"
mkdir -p "$HOME" "$STATE_DIR"
printf 'balanced\n' >"$FAKE_PPD_STATE"

# Los módulos de ropero (sólo definiciones: no ejecutan nada al sourcearse).
# shellcheck source=/dev/null
. "$ROOT/bin/lib/01-env.sh"
# shellcheck source=/dev/null
. "$ROOT/bin/lib/04-launch.sh"

perfil_actual() { cat "$FAKE_PPD_STATE" 2>/dev/null; }

echo "== los helpers existen =="
for h in _rp_perfil_actual _rp_perfil_poner _rp_perfil_restaurar; do
    comprobar "existe $h" "declare -F $h >/dev/null"
done

echo "== leer el perfil actual =="
comprobar "lee el perfil actual" "[[ \$(_rp_perfil_actual) == balanced ]]"

echo "== mover el perfil y guardarlo =="
_rp_perfil_poner 9999 performance >/dev/null 2>&1
comprobar "el perfil pasa a performance" "[[ \$(perfil_actual) == performance ]]"
comprobar "el perfil ANTERIOR queda guardado en el estado" \
    "grep -q '^PROFILE_PREV=balanced' \"\$(rp_state_path 9999 2>/dev/null || echo \$STATE_DIR/ports/9999)\""

echo "== devolverlo =="
_rp_perfil_restaurar 9999 >/dev/null 2>&1
comprobar "el perfil vuelve a balanced" "[[ \$(perfil_actual) == balanced ]]"

echo "== si ya está en el perfil pedido, no se toca =="
_rp_perfil_poner 9999 balanced >/dev/null 2>&1
# El campo se vacía (no se borra la línea): lo que no puede quedar es un
# PROFILE_PREV CON VALOR, que al parar movería el perfil sin motivo.
comprobar "no queda un PROFILE_PREV con valor" \
    "! grep -q '^PROFILE_PREV=.' \"\$(rp_state_path 9999 2>/dev/null || echo \$STATE_DIR/ports/9999)\""

echo "== sin powerprofilesctl no pasa nada =="
( PATH="/usr/bin:/bin" _rp_perfil_poner 9999 performance >/dev/null 2>&1 ) \
    && ok "poner sin el daemon no rompe nada" \
    || mal "poner sin el daemon falla"
comprobar "el perfil real no se ha tocado" "[[ \$(perfil_actual) == balanced ]]"

echo "== el ajuste es opt-in =="
comprobar "ROPERO_PERFIL_SERVE es lo que lo activa (no se toca por defecto)" \
    "grep -q 'ROPERO_PERFIL_SERVE' \"$ROOT/bin/lib/04-launch.sh\""

echo
if [[ "$fallos" -eq 0 ]]; then
    printf 'todo ok\n'
else
    printf '%s fallo(s)\n' "$fallos" >&2
fi
exit "$fallos"
