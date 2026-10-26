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
# EL DISCO, QUE ES EL LÍMITE DE VERDAD
# ------------------------------------
# Cada linea descarga el repo de pesos entero (40-60 GB) y genera un GGUF
# intermedio del mismo tamaño: el pico son ~120 GB por modelo. Con los
# pesos en F32 el GGUF intermedio es la mitad.
#
# Por eso el gguffer, SIN --keep-source, borra los safetensors y el
# intermedio TANTO si todo va bien COMO si algo falla. En un trabajo de
# noche, dejar 60 GB por cada intento fallido llena el disco, y cuando el
# disco se llena falla tambien lo que ya estaba bien.
#
# Y por eso, al fallar un modelo, este script SIGUE con el siguiente en vez
# de pararse: un modelo que no se deja fabricar no puede impedir que los
# demas se fabricen.
#
# USO:
#   ./cuantizar-todos.sh                -> lista lo que hay y sale
#   ./cuantizar-todos.sh <modelo>       -> cuantiza ese
#   ./cuantizar-todos.sh todos          -> cuantiza TODOS y sigue tras los fallos
#   ./cuantizar-todos.sh todos --parar  -> cuantiza todos y para en el primer fallo
#   ./cuantizar-todos.sh <modelo> --diagnostico -> dice por que no se cuantiza
#
# Lo que hace cada tirada queda en $LOG_DIR/<modelo>.log, y al final un
# resumen con lo que se hizo y lo que fallo.
# =============================================================================
set -uo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TABLA="${TABLA:-$AQUI/modelos.tsv}"
GGUF_DIR="${GGUF_DIR:-$HOME/models/gguf}"
VARIANTES_DIR="$GGUF_DIR/_variantes"
LOG_DIR="${LOG_DIR:-$AQUI/logs}"
mkdir -p "$LOG_DIR"

# ¿Parar en el primer fallo? Por defecto NO: es un trabajo de noche.
PARAR=0
DIAGNOSTICO=0
MODELOS_PEDIDOS=()
for a in "$@"; do
    case "$a" in
        --parar)        PARAR=1 ;;
        --diagnostico)  DIAGNOSTICO=1 ;;
        --help|-h)      sed -n '2,45p' "$0"; exit 0 ;;
        todos)          MODELOS_PEDIDOS+=("todos") ;;
        *)              MODELOS_PEDIDOS+=("$a") ;;
    esac
done

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

# Cuanto hay libre en el disco del trabajo. El gguffer comprueba tambien que
# quepa la conversion, pero aqui se avisa ANTES de bajar 40 GB.
libre_en() {
    df -PB1 "$1" 2>/dev/null | awk 'NR==2 {print $4}'
}

listar() {
    printf '%-12s %-18s %s\n' MODELO QUANTS DONDE ACABA EL PRINCIPAL
    local n pesos q imr imf flags fuente dest
    while IFS=$'\t' read -r n pesos q imr imf flags fuente; do
        [[ -z "$n" || "$n" == \#* ]] && continue
        dest="$(destino_de "$n" 2>/dev/null || true)"
        [[ -z "$dest" ]] && dest="(no hay TOML para '$n')"
        printf '%-12s %-18s %s\n' "$n" "$q" "$dest"
    done < "$TABLA"
}

# ── el estado de cada modelo, para el resumen ─────────────────────────────
declare -A RESULTADO=()

cuantizar() {
    local n="$1" pesos="$2" q="$3" imr="$4" imf="$5" flags="$6" fuente="${7:-}"
    local final
    final="$(destino_de "$n" 2>/dev/null || true)"
    if [[ -z "$final" ]]; then
        echo "[$n] ropero no conoce el modelo '$n'" >&2
        echo "      mira 'ropero list' para los que hay" >&2
        RESULTADO["$n"]="SIN TOML"
        return 1
    fi

    # Los quants de la linea, separados por comas.
    local -a quants=()
    local IFS=','
    read -r -a quants <<< "$q"
    unset IFS

    # ── DONDE VA CADA QUANT ───────────────────────────────────────────────
    # El --out-principal va a la ruta que espera el TOML, PERO solo si esa
    # ruta lleva el nombre del quant: si el TOML dice "Qwen3.8-27B-IQ2_M"
    # y el quant principal es IQ3_XS, poner el IQ3_XS en ese fichero seria
    # un nombre mentiroso (y el launcher creeria que tiene el IQ2_M).
    #
    # Asi que: si el nombre de la ruta del TOML contiene el quant, alla va.
    # Si no, el quant va a la carpeta del modelo con su nombre propio
    # (<repo>-<quant>.gguf), que es lo que hace el gguffer.
    local qq nq="${#quants[@]}"
    local principal="${quants[0]:-}"
    local -a args=()
    if [[ -n "$fuente" ]]; then
        # La fuente es un GGUF YA HECHO (un repo de HF con el BF16, o un
        # fichero local). No se clonan pesos ni se convierte nada: el GGUF
        # ya esta y solo hay que cuantizarlo. Es el camino cuando el
        # conversor de pesos no sabe convertir el modelo.
        args+=(--gguf "$fuente")
    else
        args+=(--repo "$pesos")
    fi
    for (( qq=0; qq<nq; qq++ )); do
        args+=(--quant "${quants[$qq]}")
    done
    args+=(--imatrix-repo "$imr" --imatrix-file "$imf")

    local nombre_toml
    nombre_toml="$(basename "$final")"
    if [[ "$nombre_toml" == *"${principal}"* ]]; then
        args+=(--out-principal "$final")
    else
        # la carpeta del modelo, con el nombre que genera el gguffer
        mkdir -p "$(dirname "$final")"
        args+=(--out-principal "$(dirname "$final")/")
        echo "     (el TOML pide $nombre_toml, que no es el quant $principal:"
        echo "      el GGUF va a la carpeta con su nombre propio)"
    fi

    if (( nq > 1 )); then
        mkdir -p "$VARIANTES_DIR/$n"
        args+=(--out "$VARIANTES_DIR/$n")
    fi
    (( DIAGNOSTICO == 1 )) && args+=(--diagnostico)
    # KEEP=1 conserva los safetensors. Sin esto, el gguffer los borra
    # tanto si todo va bien como si algo falla: en un trabajo de noche, 60 GB
    # por cada intento llenan el disco, y cuando el disco se llena falla
    # tambien lo que ya estaba bien.
    [[ "${KEEP:-0}" == "1" ]] && args+=(--keep-source)

    echo "[$n] cuantizando a $q"
    if [[ -n "$fuente" ]]; then
        echo "     fuente:  $fuente  (GGUF ya hecho: no se convierte nada)"
    else
        echo "     pesos:   $pesos"
    fi
    echo "     imatrix: $imr/$imf  (lo baja el gguffer y lo cachea)"
    echo "     destino: $final"
    (( nq > 1 )) && echo "     variantes: $VARIANTES_DIR/$n"

    # El log de la tirada, para mirar la manana sin buscar en la consola.
    local log="$LOG_DIR/$n.log"
    echo "     log:     $log"
    ropero gguffer "${args[@]}" $flags 2>&1 | tee "$log"
    local rc=${PIPESTATUS[0]}

    if (( rc == 0 )); then
        RESULTADO["$n"]="OK"
    else
        RESULTADO["$n"]="FALLO (rc=$rc)"
    fi
    return $rc
}

# ── el recorrido ───────────────────────────────────────────────────────────
recorrer() {
    local filtrar="${1:-}"
    local n pesos q imr imf flags fuente
    while IFS=$'\t' read -r n pesos q imr imf flags fuente; do
        [[ -z "$n" || "$n" == \#* ]] && continue
        [[ -n "$filtrar" && "$filtrar" != "todos" && "$n" != "$filtrar" ]] && continue

        # El espacio ANTES de bajar 40 GB. El pico es el clon + el GGUF
        # intermedio: del orden de 120 GB si los pesos son BF16.
        local libre
        libre="$(libre_en "$GGUF_DIR")"
        if [[ -n "${libre:-}" && "${libre:-0}" -lt 120000000000 ]]; then
            echo "" >&2
            echo "[!] AVISO: solo quedan $(numfmt --to=iec-i --suffix=B "${libre:-0}" 2>/dev/null) en el disco de los GGUFs." >&2
            echo "    Una tirada puede necesitar ~120 GB (los pesos + el GGUF intermedio)." >&2
            echo "    El gguffer lo comprobara antes de convertir, pero el clon ya estara descargado." >&2
        fi

        echo ""
        echo "════════════════════════════════════════════════════════════"
        if ! cuantizar "$n" "$pesos" "$q" "$imr" "$imf" "$flags" "$fuente"; then
            if (( PARAR == 1 )); then
                echo "" >&2
                echo "PARADO en $n: fallo la cuantizacion." >&2
                echo "El clon de pesos se ha BORRADO (no se paso --keep-source)." >&2
                echo "Arregla eso y relanza: el gguffer vuelve a bajar el repo." >&2
                return 1
            fi
            echo "" >&2
            echo "[!] $n ha fallado. Sigo con el siguiente: el gguffer ya ha borrado" >&2
            echo "    los safetensors, asi que el disco no se llena." >&2
        fi
    done < "$TABLA"
    return 0
}

resumen() {
    echo ""
    echo "════════════════════════════════════════════════════════════"
    echo "RESUMEN"
    echo "════════════════════════════════════════════════════════════"
    local n
    for n in "${!RESULTADO[@]}"; do
        printf '  %-14s %s\n' "$n" "${RESULTADO[$n]}"
    done
    echo ""
    echo "  logs: $LOG_DIR/"
    echo "  sube lo verificado:  ./subir-hf.sh --listar && ./subir-hf.sh --subir"
}

if [[ ${#MODELOS_PEDIDOS[@]} -eq 0 ]]; then
    listar
    echo ""
    echo "Para cuantizar uno:  ./cuantizar-todos.sh <modelo>"
    echo "Para todos:         ./cuantizar-todos.sh todos"
    exit 0
fi

for m in "${MODELOS_PEDIDOS[@]}"; do
    recorrer "$m"
    if (( PARAR == 1 )); then break; fi
done
resumen
