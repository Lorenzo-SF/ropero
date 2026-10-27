#!/usr/bin/env bash
# =============================================================================
# todo.sh — cuantizar, verificar y subir a HuggingFace, en ese orden
# =============================================================================
# El guion completo de una fabricacion:
#
#   1. CUANTIZAR  (quant/cuantizar-todos.sh)   baja los pesos, convierte y
#      saca todos los quants de la tirada.
#   2. VERIFICAR  (quant/verificar-gguf.sh)    comprueba la metadata de
#      cada GGUF sin necesidad de GPU.
#   3. SUBIR      (quant/subir-hf.sh)          sube a HF lo que no tenga el
#      marcador .hf-uploaded, y crea el marcador.
#
# Si un GGUF no pasa la verificacion, NO se sube: es mejor no subir que
# subir un GGUF roto.
#
# USO:
#   ./todo.sh                 -> todo, de la tabla
#   ./todo.sh fable           -> solo ese modelo
#   ./todo.sh --sin-subir     -> cuantiza y verifica, pero no sube
#   ./todo.sh --solo-subir    -> no cuantiza: solo sube lo pendiente
# =============================================================================
set -uo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GGUF_DIR="${GGUF_DIR:-$HOME/models/gguf}"

SUBIR=1
CUANTIZAR=1
MODELOS=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --sin-subir)  SUBIR=0; shift ;;
        --solo-subir) CUANTIZAR=0; shift ;;
        *)            MODELOS="$1"; shift ;;
    esac
done

echo "══════════════════════════════════════════════════════════════════════"
echo " 1. CUANTIZAR"
echo "══════════════════════════════════════════════════════════════════════"
if (( CUANTIZAR == 1 )); then
    if [[ -n "$MODELOS" ]]; then
        "$AQUI/cuantizar-todos.sh" "$MODELOS" || {
            echo "" >&2
            echo "PARADO: fallo la cuantizacion. Los pesos y el GGUF intermedio" >&2
            echo "de lo que se estaba haciendo se conservan para reintentar." >&2
            exit 1
        }
    else
        "$AQUI/cuantizar-todos.sh" todos || exit 1
    fi
else
    echo "  (saltado)"
fi

echo ""
echo "══════════════════════════════════════════════════════════════════════"
echo " 2. VERIFICAR"
echo "══════════════════════════════════════════════════════════════════════"
malos=0
n=0
while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    n=$((n+1))
    echo "[$n] ${f#"$GGUF_DIR"/}"
    if ! "$AQUI/verificar-gguf.sh" "$f"; then
        malos=$((malos+1))
    fi
done < <(find "$GGUF_DIR" -type f -name '*.gguf' ! -path '*/_variantes/*' 2>/dev/null | sort)

if (( malos > 0 )); then
    echo "" >&2
    echo "$malos GGUF(s) NO pasan la verificacion: no se suben." >&2
    exit 1
fi
echo "todos los GGUFs verificados"

echo ""
echo "══════════════════════════════════════════════════════════════════════"
echo " 3. SUBIR A HUGGINGFACE"
echo "══════════════════════════════════════════════════════════════════════"
if (( SUBIR == 1 )); then
    "$AQUI/subir-hf.sh" --subir || exit 1
else
    echo "  (saltado)"
fi
