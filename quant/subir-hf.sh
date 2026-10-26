#!/usr/bin/env bash
# =============================================================================
# subir-hf.sh — subir a HuggingFace los GGUFs de ~/models/gguf
# =============================================================================
# TODOS los GGUFs de $GGUF_DIR son cuantizaciones hechas por ti (no descargas
# de HF), asi que todos son candidatos a subirse.
#
# El marcador ".hf-uploaded": junto al GGUF, en su misma carpeta, hay un
# fichero llamado .hf-uploaded con el repo de HF donde se subio. Si existe,
# ese GGUF NO se vuelve a subir. Si no existe, se sube.
#
# El repo de destino sale solo: <HF_USER>/<nombre-del-modelo>-<QUANT>-GGUF.
# El nombre del modelo, el quant y el base_model salen de la METADATA del
# propio GGUF (general.basename, general.file_type y
# general.base_model.0.repo_url), asi que no hay que mantener ninguna tabla
# al lado.
#
# USO:
#   ./subir-hf.sh                 -> lista lo que hay y lo que se subiria
#   ./subir-hf.sh --subir         -> sube lo que no tenga el marcador
#   ./subir-hf.sh --subir <fichero>  -> sube solo ese
#
# El usuario de HF sale de quant/hf.env (HF_USER=...) o del entorno.
# =============================================================================
set -uo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROPERO="$(cd "$AQUI/.." && pwd)"
GGUF_DIR="${GGUF_DIR:-$HOME/models/gguf}"
GGUFCHECK="$REPO_ROPERO/bin/_ggufcheck.py"

# El usuario de HF: del entorno, o del fichero de configuracion.
HF_USER="${HF_USER:-}"
if [[ -z "$HF_USER" && -f "$AQUI/hf.env" ]]; then
    # shellcheck disable=SC1091
    HF_USER="$(sed -n 's/^HF_USER=//p' "$AQUI/hf.env" | head -1)"
fi

MARCADOR=".hf-uploaded"
ACCION="listar"
FICHERO=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --subir)  ACCION="subir"; shift ;;
        --listar) ACCION="listar"; shift ;;
        --user)   HF_USER="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0 ;;
        *)  FICHERO="$1"; ACCION="subir"; shift ;;
    esac
done

if [[ ! -d "$GGUF_DIR" ]]; then
    echo "ERROR: no existe el directorio de GGUFs: $GGUF_DIR" >&2
    exit 1
fi
if ! command -v hf >/dev/null 2>&1; then
    echo "ERROR: falta 'hf' (huggingface_hub): pip install -U huggingface_hub" >&2
    exit 1
fi
if [[ -z "$HF_USER" ]]; then
    echo "ERROR: no se sabe tu usuario de HuggingFace." >&2
    echo "       ponlo en $AQUI/hf.env:  HF_USER=tu-usuario" >&2
    echo "       o exporta HF_USER antes de rodar." >&2
    exit 1
fi

# ── los tipos de quant (numero -> nombre) ─────────────────────────────────
# El GGUF lleva general.file_type, que es el NUMERO del tipo. La lista la
# da el motor, o la de respaldo si el motor no esta (quant/quant-tipos.sh).
# shellcheck disable=SC1091
source "$AQUI/quant-tipos.sh"
QUANTS_NUM="$(quants_num)"

# ── la metadata de un GGUF ────────────────────────────────────────────────
meta() {
    local f="$1" k="$2"
    python3 "$GGUFCHECK" "$f" --info 2>/dev/null \
        | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(0)
k = sys.argv[1]
v = d.get(k)
print("" if v is None else v)
' "$k" 2>/dev/null
}

# El nombre del modelo, sin la extension del quant.
nombre_modelo() {
    local f="$1"
    local base sufijo
    base="$(basename "$f" .gguf)"
    # el quant va como sufijo: "fable-coder-35B-A3B.Q2_K" -> "fable-coder-35B-A3B"
    #
    # OJO: "sufijo" se DECLARA FUERA del if. Con `set -u`, declarada dentro
    # de un bloque, la variable no llega a existir al usarla, y bash dice
    # "sufijo: unbound variable" sin senalar que el problema es el `local`.
    sufijo="${base##*.}"
    if [[ "$base" == *.* ]] && es_quant "$sufijo"; then
        base="${base%.*}"
    fi
    printf '%s' "$base"
}

quant_de() {
    local f="$1" ft
    ft="$(meta "$f" file_type)"
    [[ -z "$ft" ]] && { printf ''; return 0; }
    quant_nombre "$ft"
}

# El imatrix del gguffer, si hay uno para ese modelo. La cache es
# ~/.cache/ropero/gguffer/imatrix/<owner>__<repo>/<fichero>.
imatrix_reciente() {
    local base="$1" dir cache
    cache="${GGUF_IMATRIX_CACHE:-$HOME/.cache/ropero/gguffer/imatrix}"
    [[ -d "$cache" ]] || return 0
    # se busca el subdirectorio cuyo nombre (owner__repo) coincide con el
    # base_model (owner/repo)
    local owner="${base%%/*}" repo="${base##*/}"
    dir="$cache/${owner}__${repo}"
    if [[ ! -d "$dir" ]]; then
        # fallback: el mas reciente de la cache
        dir="$(find "$cache" -mindepth 1 -maxdepth 1 -type d -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)"
    fi
    [[ -n "$dir" && -d "$dir" ]] || return 0
    find "$dir" -type f \( -name '*.gguf' -o -name '*.gguf_file' -o -name '*.imatrix' \) 2>/dev/null \
        | head -1
}

# ── la lista de GGUFs ─────────────────────────────────────────────────────
listar_ggufs() {
    if [[ -n "$FICHERO" ]]; then
        printf '%s\n' "$FICHERO"
        return
    fi
    find "$GGUF_DIR" -type f -name '*.gguf' ! -path '*/_variantes/*' 2>/dev/null | sort
}

subir_uno() {
    local f="$1"
    local carpeta base quant destino base_model repo_url

    if [[ ! -f "$f" ]]; then
        echo "  [x] no existe: $f" >&2
        return 1
    fi
    carpeta="$(dirname "$f")"
    base="$(nombre_modelo "$f")"
    quant="$(quant_de "$f")"
    [[ -z "$quant" ]] && quant="desconocido"
    destino="$HF_USER/${base}-${quant}-GGUF"

    # ── el marcador ──
    if [[ -f "$carpeta/$MARCADOR" ]]; then
        echo "  ya subido: $f  (marcador en $carpeta/$MARCADOR)"
        return 0
    fi

    # El base_model sale de la metadata del GGUF: general.base_model.0.repo_url
    base_model="$(python3 "$GGUFCHECK" "$f" --info 2>/dev/null | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
except Exception:
    print(""); raise SystemExit
u = d.get("base_model_repo") or ""
n = d.get("base_model_name") or ""
if u:
    # https://huggingface.co/<owner>/<repo>
    partes = u.rstrip("/").split("/")
    if len(partes) >= 2:
        print("%s/%s" % (partes[-2], partes[-1]))
elif n:
    print(n)
' 2>/dev/null)"

    echo "  subiendo: $(basename "$f")"
    echo "     destino: $destino"
    [[ -n "$base_model" ]] && echo "     base:    $base_model"

    # ── el repo ──
    if ! hf repo info "$destino" >/dev/null 2>&1; then
        hf repo create "$destino" --repo-type model || return 1
    fi

    # ── el model card ──
    local card="$carpeta/README.md"
    {
        cat <<EOF
---
base_model: ${base_model:-DESCONOCIDO}
library_name: llama-cpp
tags:
  - gguf
  - ${quant}
---

# ${base} — ${quant}

GGUF fabricado a mano con \`ropero gguffer\` (llama.cpp, cuantizador oficial).

- **Quant**: ${quant}
- **Base model**: ${base_model:-DESCONOCIDO}
EOF
        if [[ -n "$NOTA" ]]; then
            printf '\n%s\n' "$NOTA"
        fi
        cat <<'EOF'

Cuantizado con el importance matrix que va en este repo, sobre el corpus
habitual.
EOF
    } > "$card"

    hf upload "$destino" "$f" --repo-type model || return 1
    # El imatrix, si esta en la cache del gguffer. Se sube al repo para que
    # la gente pueda refinar sus propias cuantizaciones con el mismo.
    local imx
    imx="$(imatrix_reciente "$base_model")"
    if [[ -n "$imx" ]]; then
        echo "     imatrix: $imx"
        hf upload "$destino" "$imx" --repo-type model || return 1
    fi
    hf upload "$destino" "$card" --repo-type model --include "README.md" || return 1

    # ── el marcador ──
    printf '%s\n' "$destino" > "$carpeta/$MARCADOR"
    echo "  ok: $destino"
    return 0
}

NOTA="${NOTA:-}"
IMATRIX_NAME="${IMATRIX_NAME:-}"

case "$ACCION" in
    listar)
        echo "GGUF_DIR: $GGUF_DIR"
        echo "HF_USER:  $HF_USER"
        echo ""
        printf '%-8s %-62s %s\n' ESTADO FICHERO DESTINO
        while IFS= read -r f; do
            [[ -z "$f" ]] && continue
            carpeta="$(dirname "$f")"
            base="$(nombre_modelo "$f")"
            quant="$(quant_de "$f")"
            [[ -z "$quant" ]] && quant="?"
            if [[ -f "$carpeta/$MARCADOR" ]]; then
                estado="subido"
            else
                estado="PENDIENTE"
            fi
            printf '%-8s %-62s %s\n' "$estado" "${f#"$GGUF_DIR"/}" "$HF_USER/${base}-${quant}-GGUF"
        done < <(listar_ggufs)
        ;;
    subir)
        fallos=0
        while IFS= read -r f; do
            [[ -z "$f" ]] && continue
            subir_uno "$f" || fallos=$((fallos+1))
        done < <(listar_ggufs)
        if (( fallos > 0 )); then
            echo "" >&2
            echo "$fallos fichero(s) sin subir." >&2
            exit 1
        fi
        ;;
esac
