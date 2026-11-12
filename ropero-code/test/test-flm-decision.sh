#!/usr/bin/env bash
# =============================================================================
# test/test-flm-decision.sh — el TAG del motor de decisión (FLM)
# =============================================================================
#
# Qué comprueba (y por qué importa):
#
#   1. Un modelo cuyo [model].path está VACÍO (d3mini: `flm serve` sin tag no
#      sirve un modelo de chat, sólo la decisión) baja el TAG que va en
#      `--decision <tag>` de sus argumentos.
#
#   2. Un modelo con path (lfm3) baja SU tag Y el de --decision, porque su
#      servidor expone las dos cosas: el chat y /v1/systemone.
#
#   3. Si no hay tag en ningún sitio, no se intenta `flm pull ""`: se avisa y
#      se sale sin error.
#
#   Por qué importa: sin el build NPU2 en el almacén, el servidor arranca y
# no sabe clasificar nada. El síntoma es "funciona todo y el D3 no responde".
#
# Uso:  test/test-flm-decision.sh
# Salida: 0 si todo pasa, 1 si algún caso falla.
# =============================================================================
set -u

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROPERO_ROOT="$RAIZ"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/ropero-flm-decision.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

export HOME="$TMP/home"
mkdir -p "$HOME"

# ── un flm falso que anota lo que le piden ─────────────────────────────────
mkdir -p "$TMP/bin"
cat > "$TMP/bin/flm" <<'FAKE'
#!/usr/bin/env bash
printf 'flm %s\n' "$*" >> "$FLM_LOG"
exit 0
FAKE
chmod +x "$TMP/bin/flm"
export FLM_LOG="$TMP/flm.log"
: > "$FLM_LOG"
export PATH="$TMP/bin:$PATH"

# shellcheck source=/dev/null
. "$RAIZ/bin/lib/00-utils.sh"
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/01-env.sh"
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/02-modelconf.sh"
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/03-engines.sh"
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/engines/fastflowlm.sh"

ok=0
fallos=0
comprobar() {
    local desc="$1" cond="$2"
    if eval "$cond"; then
        ok=$((ok + 1))
        printf '  ok   %s\n' "$desc"
    else
        fallos=$((fallos + 1))
        printf '  FALLO %s\n' "$desc"
    fi
}

# ── 1. d3mini: path vacío, el tag sale de --decision ───────────────────────
path_d3mini="$(rp_model_field d3mini path 2>/dev/null || true)"
comprobar "d3mini no declara [model].path (sólo decisión)" '[[ -z "$path_d3mini" ]]'

: > "$FLM_LOG"
rp_eng_fastflowlm_install_model d3mini >/dev/null 2>&1
comprobar "d3mini baja el tag de --decision" \
    'grep -q "flm pull d3:mini" "$FLM_LOG"'
comprobar "d3mini no intenta bajar un tag vacío" \
    '! grep -q "flm pull *$" "$FLM_LOG"'

# ── 2. lfm3: su tag Y el de --decision ─────────────────────────────────────
: > "$FLM_LOG"
rp_eng_fastflowlm_install_model lfm3 >/dev/null 2>&1
comprobar "lfm3 baja su propio tag" 'grep -q "flm pull lfm3" "$FLM_LOG"'
comprobar "lfm3 baja también el tag de la decisión" \
    'grep -q "flm pull d3:mini" "$FLM_LOG"'

# ── 3. el catálogo: el decisor es d3mini y no queda d3nano ─────────────────
comprobar "el catálogo tiene d3mini" 'rp_model_exists d3mini'
comprobar "el catálogo NO tiene d3nano (el decisor único es d3mini)" \
    '! rp_model_exists d3nano'

printf '  %d ok, %d fallos\n' "$ok" "$fallos"
[[ "$fallos" -eq 0 ]]
