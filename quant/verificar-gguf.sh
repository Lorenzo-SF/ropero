#!/usr/bin/env bash
# =============================================================================
# verificar-gguf.sh — comprobar rapido que un GGUF fabricado esta bien
# =============================================================================
# El gguffer ya verifica el GGUF al final (que se pueda cargar). Esto es
# la comprobacion POSTERIOR, la que se puede hacer sin GPU y en segundos:
# mira la metadata y la compara con lo que se pedia.
#
# USO:
#   ./verificar-gguf.sh <fichero.gguf> [QUANT_ESPERADO]
#
# CODIGO DE SALIDA: 0 si esta bien, 1 si no.
# =============================================================================
set -uo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROPERO="$(cd "$AQUI/.." && pwd)"
GGUFCHECK="$REPO_ROPERO/bin/_ggufcheck.py"

FICHERO="${1:-}"
ESPERADO="${2:-}"

if [[ -z "$FICHERO" ]]; then
    echo "USO: $0 <fichero.gguf> [QUANT_ESPERADO]" >&2
    exit 2
fi

fallos=0
error() { echo "  [x] $1"; fallos=$((fallos+1)); }
ok()    { echo "  [ok] $1"; }

if [[ ! -f "$FICHERO" ]]; then
    echo "  [x] no existe: $FICHERO"
    exit 1
fi

# El tamano: un GGUF de un modelo de 30B son varios GB. Menos de 100 MB
# significa que la cuantizacion salio mal.
tam="$(stat -c %s "$FICHERO" 2>/dev/null || echo 0)"
if (( tam < 100 * 1024 * 1024 )); then
    error "el fichero es demasiado pequeno: $tam bytes"
else
    ok "tamano: $((tam / 1024 / 1024)) MiB"
fi

# La metadata
info="$(python3 "$GGUFCHECK" "$FICHERO" --info 2>/dev/null)"
if [[ -z "$info" ]]; then
    error "no se puede leer la metadata del GGUF"
    exit 1
fi

lee() { printf '%s' "$info" | python3 -c '
import json, sys
d = json.load(sys.stdin)
v = d.get(sys.argv[1])
print("" if v is None else v)
' "$1" 2>/dev/null; }

arch="$(lee arch)"
tensores="$(lee tensores)"
ft="$(lee file_type)"
block_count="$(lee block_count)"
nextn="$(lee nextn)"

[[ -n "$arch" && "$arch" != "?" ]] && ok "arch: $arch" || error "sin arch en la metadata"
[[ -n "$tensores" && "$tensores" != "0" ]] && ok "tensores: $tensores" || error "el GGUF no tiene tensores"
[[ -n "$block_count" && "$block_count" != "None" ]] && ok "block_count: $block_count" || error "sin block_count"
[[ -n "$nextn" && "$nextn" != "None" ]] && ok "nextn_predict_layers: $nextn" || ok "nextn_predict_layers: (ninguno)"

# El quant esperado: general.file_type es el NUMERO del tipo. El nombre lo
# saca la lista del motor.
if [[ -n "$ESPERADO" && -n "$ft" ]]; then
    # shellcheck disable=SC1091
    source "$AQUI/quant-tipos.sh"
    num="$(quant_numero "$ESPERADO")"
    if [[ -z "$num" ]]; then
        error "el quant $ESPERADO no existe"
    elif [[ "$num" == "$ft" ]]; then
        ok "quant: $ESPERADO (file_type=$ft)"
    else
        error "el quant NO es el esperado: file_type=$ft, $ESPERADO seria $num"
    fi
fi

if (( fallos > 0 )); then
    echo "  FALLO: $fallos problema(s)"
    exit 1
fi
echo "  OK"
exit 0
