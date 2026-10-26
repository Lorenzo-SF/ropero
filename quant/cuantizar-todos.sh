#!/usr/bin/env bash
# =============================================================================
# cuantizar-todos.sh — fabricar los GGUFs con ropero gguffer
# =============================================================================
# La tabla de QUÉ fabricar es quant/modelos.tsv (no este script: la tabla es
# la fuente de verdad y se puede editar sin tocar codigo).
#
# El gguffer se encarga de TODO: baja los pesos, los convierte, baja el
# imatrix (y lo cachea en ~/.cache/ropero/gguffer/imatrix/) y deja el GGUF
# en la carpeta que espera el .toml. Este script solo le dice qué fabricar.
#
# VARIOS QUANTS EN UNA TIRADA
# ----------------------------
# En la tabla, la columna de quants puede tener mas de uno, separados por
# comas. Bajar y convertir los pesos es lo caro (40-60 GB y una hora);
# cuantizar es solo CPU. Por eso el gguffer convierte el GGUF intermedio
# UNA vez y lo reutiliza para todos los quants de la tirada: dos quants
# cuestan lo mismo que uno, en disco y en tiempo.
#
# El PRIMER quant de la lista es el "principal": ese va exactamente a la
# ruta que espera el TOML (--out-principal). Los demas quedan en
# $GGUF_DIR/_variantes/<modelo>/, con su nombre <repo>-<quant>.gguf.
#
# OJO CON EL DISCO: cada linea descarga el repo de pesos entero (40-60 GB) y
# genera un GGUF intermedio del mismo tamaño. El pico son ~120 GB por
# modelo. El gguffer borra el clon de los pesos y el intermedio cuando el
# GGUF final valida, y reutiliza el clon si vuelves a lanzar la misma linea.
#
# USO:
#   ./cuantizar-todos.sh                -> lista lo que hay y sale
#   ./cuantizar-todos.sh fable          -> cuantiza ese
#   ./cuantizar-todos.sh todos          -> cuantiza todos (mucho disco y rato)
#   ./cuantizar-todos.sh --sin-imagen   -> sin la imagen de docker (no se usa)
# =============================================================================
set -uo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TABLA="${TABLA:-$AQUI/modelos.tsv}"
GGUF_DIR="${GGUF_DIR:-$HOME/models/gguf}"
VARIANTES_DIR="$GGUF_DIR/_variantes"

if [[ ! -f "$TABLA" ]]; then
    echo "ERROR: no se encuentra la tabla: $TABLA" >&2
    exit 1
fi

# La ruta donde va cada GGUF NO se saca de aqui: la dice el propio ropero
# ("ropero path <modelo>"), que es quien la resuelve al arrancar. Asi este
# script no depende de donde este el repo de ropero ni de como se llame cada
# .toml. Y un lector de TOMLs propio se queda viejo en cuanto se renombra
# un modelo o se cambia su [files] gguf.
destino_de() {
    local modelo="$1" ruta
    ruta="$(ropero path "$modelo" 2>/dev/null)" || return 1
    [[ -n "$ruta" ]] || return 1
    printf '%s' "$ruta"
}

listar() {
    printf '%-12s %-18s %s\n' MODELO QUANTS DONDE ACABA EL PRINCIPAL
    local m n pesos q imr imf flags dest
    while IFS=$'\t' read -r n pesos q imr imf flags; do
        [[ -z "$n" || "$n" == \#* ]] && continue
        dest="$(destino_de "$n" 2>/dev/null || true)"
        [[ -z "$dest" ]] && dest="(no hay TOML para '$n')"
        printf '%-12s %-18s %s\n' "$n" "$q" "$dest"
    done < "$TABLA"
}

cuantizar() {
    local n="$1" pesos="$2" q="$3" imr="$4" imf="$5" flags="$6"
    local final
    final="$(destino_de "$n" 2>/dev/null || true)"
    if [[ -z "$final" ]]; then
        echo "[$n] ropero no conoce el modelo '$n'" >&2
        echo "      mira 'ropero list' para los que hay" >&2
        return 1
    fi

    # Los quants de la linea, separados por comas.
    local -a quants=()
    local IFS=','
    read -r -a quants <<< "$q"
    unset IFS

    local -a args=(--repo "$pesos" --out-principal "$final")
    local qq nq="${#quants[@]}"
    for (( qq=0; qq<nq; qq++ )); do
        args+=(--quant "${quants[$qq]}")
    done
    args+=(--imatrix-repo "$imr" --imatrix-file "$imf")
    if (( nq > 1 )); then
        mkdir -p "$VARIANTES_DIR/$n"
        args+=(--out "$VARIANTES_DIR/$n")
    fi

    echo "[$n] cuantizando a $q"
    echo "     imatrix: $imr/$imf  (lo baja el gguffer y lo cachea)"
    echo "     destino: $final"
    (( nq > 1 )) && echo "     variantes: $VARIANTES_DIR/$n"
    ropero gguffer "${args[@]}" $flags
}

case "${1:-}" in
    "")       listar; echo ""; echo "Para cuantizar uno:  ./cuantizar-todos.sh <modelo>"; exit 0 ;;
    "todos")
        # Para en el PRIMER fallo: si uno falla, el disco y el tiempo ya
        # gastados son enormes y conviene ver el error antes de seguir.
        while IFS=$'\t' read -r n pesos q imr imf flags; do
            [[ -z "$n" || "$n" == \#* ]] && continue
            if ! cuantizar "$n" "$pesos" "$q" "$imr" "$imf" "$flags"; then
                echo "" >&2
                echo "PARADO en $n: fallo la cuantizacion." >&2
                echo "Arregla eso y relanza; el clon de pesos y el GGUF" >&2
                echo "intermedio de $n se conservan para reintentar." >&2
                exit 1
            fi
        done < "$TABLA"
        ;;
    *)
        while IFS=$'\t' read -r n pesos q imr imf flags; do
            [[ -z "$n" || "$n" == \#* ]] && continue
            [[ "$n" == "$1" ]] && { cuantizar "$n" "$pesos" "$q" "$imr" "$imf" "$flags"; exit $?; }
        done < "$TABLA"
        echo "no conozco el modelo '$1'"; listar; exit 1 ;;
esac
