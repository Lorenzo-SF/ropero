#!/usr/bin/env bash
# =============================================================================
# tests/test-models-symlink.sh — el symlink de GGUF entre modelos
#
# Qué comprueba (y por qué importa):
#
#   Dos TOMLs pueden pedir el MISMO GGUF. El caso real del catálogo:
#   'gptoss' (nuestro fork) y 'gptoss-o' (el llama.cpp de upstream) usan el
#   mismo fichero del mismo repo. Bajarlo dos veces son 12,7 GB tontos: lo
#   que tiene que pasar es que el segundo modelo se ENLACE al directorio del
#   primero.
#
#   El enlace sólo es válido si el fichero es el MISMO: se compara repo +
#   nombre (campos 13 y 15 del dump), no sólo el nombre. "mmproj-BF16.gguf"
#   aparece en repos distintos con contenidos distintos.
#
# Uso:  tests/test-models-symlink.sh
# Salida: 0 si todo pasa, 1 si algún caso falla.
# =============================================================================
set -u

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROPERO_ROOT="$RAIZ"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/ropero-symlink.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

export HOME="$TMP/home"
export GGUF_DIR="$HOME/models/gguf"
mkdir -p "$HOME" "$GGUF_DIR"

# ── el ropero mínimo que necesitan las funciones ────────────────────────────
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/00-utils.sh"
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/01-env.sh"
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/02-modelconf.sh"
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/03-engines.sh"
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/05-download.sh"
# shellcheck source=/dev/null
. "$RAIZ/bin/lib/cmd-models.sh"

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

# Los dos modelos del catálogo que comparten GGUF (mismo repo, mismo nombre).
MOD_A="gptoss"
MOD_B="gptoss-o"
GGUF="gpt-oss-20b-MXFP4_MOE.gguf"

repo_a="$(_rp_models_dump_field "$MOD_A" 13)" || repo_a=""
dest_a="$(_rp_models_dump_field "$MOD_A" 15)" || dest_a=""
file_a="$(_rp_models_dump_field "$MOD_A" 14)" || file_a=""

printf 'catálogo: %s y %s · repo=%s · destino=%s\n' "$MOD_A" "$MOD_B" "$repo_a" "$dest_a"

comprobar "el catálogo tiene los dos modelos" '[[ -n "$repo_a" && -n "$dest_a" ]]'
comprobar "el destino en disco coincide con el que espera el launcher" \
    '[[ "$dest_a" == "$GGUF" ]]'

# ── 1. el GGUF está en el modelo A: el B se enlaza, sin descargar ──────────
mkdir -p "$GGUF_DIR/$MOD_A"
: > "$GGUF_DIR/$MOD_A/$GGUF"

if _rp_models_symlink_gguf "$MOD_B" "$repo_a" "$dest_a" "$file_a"; then
    enlazado=1
else
    enlazado=0
fi
comprobar "el segundo modelo se resuelve con symlink" '[[ "$enlazado" == "1" ]]'
comprobar "existe el enlace $GGUF_DIR/$MOD_B" "[[ -L '$GGUF_DIR/$MOD_B' ]]"
comprobar "el enlace apunta al directorio del primero" \
    "[[ \"\$(readlink '$GGUF_DIR/$MOD_B')\" == '$GGUF_DIR/$MOD_A' ]]"
comprobar "el GGUF se ve a través del enlace" \
    "[[ -f '$GGUF_DIR/$MOD_B/$GGUF' ]]"

# ── 2. idempotente: repetir no rompe nada ───────────────────────────────────
if _rp_models_symlink_gguf "$MOD_B" "$repo_a" "$dest_a" "$file_a"; then
    reenlazado=1
else
    reenlazado=0
fi
comprobar "repetir el symlink no vuelve a enlazar (el enlace existe)" \
    '[[ "$reenlazado" == "0" ]]'
comprobar "el enlace sigue en pie" "[[ -L '$GGUF_DIR/$MOD_B' ]]"

# ── 3. --force NO enlaza: el usuario quiere el fichero aquí ─────────────────
rm -f "$GGUF_DIR/$MOD_B"
if ROPERO_FORCE=1 _rp_models_symlink_gguf "$MOD_B" "$repo_a" "$dest_a" "$file_a"; then
    forzado=1
else
    forzado=0
fi
comprobar "con --force no se enlaza nada" '[[ "$forzado" == "0" ]]'
comprobar "con --force no se crea el enlace" "[[ ! -e '$GGUF_DIR/$MOD_B' ]]"

# ── 4. repo distinto: mismo nombre de fichero, contenido distinto ──────────
# No debe enlazar: el fichero de otro repo NO es el mismo.
mkdir -p "$GGUF_DIR/otro-modelo"
: > "$GGUF_DIR/otro-modelo/$GGUF"
if _rp_models_symlink_gguf "$MOD_B" "otro/repo-distinto" "$dest_a" "$file_a"; then
    ajeno=1
else
    ajeno=0
fi
comprobar "un repo distinto NO se enlaza (el fichero es otro)" '[[ "$ajeno" == "0" ]]'
comprobar "no se crea el enlace con un repo distinto" "[[ ! -e '$GGUF_DIR/$MOD_B' ]]"

# ── 5. nada en disco: se sigue con la descarga ──────────────────────────────
rm -rf "$GGUF_DIR/$MOD_A" "$GGUF_DIR/otro-modelo"
if _rp_models_symlink_gguf "$MOD_B" "$repo_a" "$dest_a" "$file_a"; then
    vacio=1
else
    vacio=0
fi
comprobar "sin GGUF en disco no hay symlink (se descarga)" '[[ "$vacio" == "0" ]]'

# ── 6. el fichero en disco con el nombre del repo: aviso, no enlace roto ───
mkdir -p "$GGUF_DIR/$MOD_A"
: > "$GGUF_DIR/$MOD_A/$file_a"
salida="$(_rp_models_symlink_gguf "$MOD_B" "$repo_a" "$dest_a" "$file_a" 2>&1)"
comprobar "fichero con otro nombre: no se enlaza" '[[ ! -e "$GGUF_DIR/$MOD_B" ]]'
comprobar "fichero con otro nombre: se explica cómo arreglarlo" \
    '[[ "$salida" == *"renómbralo"* ]]'

# ── 7. el catálogo valida ───────────────────────────────────────────────────
if python3 "$RAIZ/bin/python/modelconf.py" validate --root "$RAIZ/models" >/dev/null 2>&1; then
    valida=1
else
    valida=0
fi
comprobar "el catálogo valida (modelconf validate)" '[[ "$valida" == "1" ]]'

printf '\n%s ok · %s fallo(s)\n' "$ok" "$fallos"
[[ "$fallos" == "0" ]] || exit 1
exit 0
