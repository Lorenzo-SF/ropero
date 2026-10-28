#!/usr/bin/env bash
# =============================================================================
# aplicar-parche-motor.sh — aplica los parches del motor (llama.cpp) al clon
# que hay en $LLAMA_DIR.
#
# POR QUE ESTO VIVE EN ROEPO Y NO EN EL MOTOR
# ────────────────────────────────────────────
# Los parches se aplican a un clon de llama.cpp que es de OTRO dueño (el
# atomic, con TurboQuant). Con el token de este repo no se puede push ahi
# (ni admin), asi que los parches se delivering COMO FICHEROS y se aplican
# aqui, en la maquina de cada uno.
#
# QUE HACE CADA PARCHE
# ────────────────────
# 1. parche-motor-nvfp4-y-pattern.patch (los dos, en un fichero)
#
#    a) src/llama-quant.cpp — la regla del NVFP4.
#       El motor tiene una regla para el MXFP4_MOE ("los tensores de
#       expertos (ne[2] > 1) a MXFP4, el resto a Q8_0") y NO la tiene para
#       el NVFP4. Sin ella, el NVFP4 de llama.cpp cuantiza a 4 bits TODO lo
#       que tiene 2 o mas dimensiones: la atencion, las proyecciones del
#       Mamba (ssm_in/ssm_out) y el embedding. El modelo arranca, pero
#       "habla cosas raras": el estado recurrente del Mamba se
#       desincroniza.
#       Medido en el NVFP4 fabricado SIN la regla: 163 tensores a 4 bits
#       (46 expertos + 117 mas) y solo 1 en q8_0.
#
#    b) common/json-schema-to-grammar.cpp — los patterns sin anclar.
#       El autoparser de templates (el que convierte el JSON schema de las
#       tools en gramatica) rechazaba cualquier "pattern" que no empezara
#       por "^" y acabara en "$", y tiraba la peticion ENTERA con un 400:
#         "Pattern must start with '^' and end with '$'"
#       El sintoma no es del modelo ni de la plantilla: es el schema de la
#       tool. El motor SIEMPRE ha exigido patterns anclados, asi que
#       tratarlos como anclados no cambia nada: solo deja de rechazar los
#       que no lo estan.
#
# USO
# ───
#   ropero/quant/aplicar-parche-motor.sh            # aplica (y compila)
#   ropero/quant/aplicar-parche-motor.sh --check    # solo dice si falta
#   ropero/quant/aplicar-parche-motor.sh --revert   # vuelve atras
#
# El clon del motor se toma de $LLAMA_DIR (o del primer argumento).
# =============================================================================
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PATCHES_DIR="$HERE/patches"
LLAMA_DIR="${LLAMA_DIR:-${1:-$HOME/llama.cpp}}"
MODE="apply"

for a in "$@"; do
    case "$a" in
        --check)  MODE="check" ;;
        --revert) MODE="revert" ;;
    esac
done

say()  { printf '%s\n' "$*"; }
ok()   { printf '  ok    %s\n' "$*"; }
warn() { printf '  AVISO %s\n' "$*"; }
err()  { printf '  ERROR %s\n' "$*" >&2; }

if [[ ! -d "$LLAMA_DIR" ]]; then
    err "no existe el clon del motor: $LLAMA_DIR"
    err "  ponlo en \$LLAMA_DIR o pasalo como argumento"
    exit 1
fi
say "motor: $LLAMA_DIR"

PATCH="$PATCHES_DIR/parche-motor-nvfp4-y-pattern.patch"
[[ -f "$PATCH" ]] || { err "falta el parche: $PATCH"; exit 1; }

# ── ¿estan aplicados? ───────────────────────────────────────────────────────
# Se mira el CODIGO, no el git: el clon puede ser un fork con otros
# cambios, y el "git apply --check" falla si el parche ya esta puesto.
ya_aplicado() {
    local fichero="$1" needle="$2"
    [[ -f "$LLAMA_DIR/$fichero" ]] || return 1
    grep -qF "$needle" "$LLAMA_DIR/$fichero"
}

estado() {
    local n=0
    ya_aplicado "src/llama-quant.cpp" "ftype == LLAMA_FTYPE_MOSTLY_NVFP4" || n=$((n+1))
    ya_aplicado "common/json-schema-to-grammar.cpp" "Empty pattern" || n=$((n+1))
    printf '%d' "$n"
}

faltan="$(estado)"

if [[ "$MODE" == "check" ]]; then
    if [[ "$faltan" == "0" ]]; then
        say "los dos parches estan aplicados"
        exit 0
    fi
    say "faltan $faltan parche(s):"
    ya_aplicado "src/llama-quant.cpp" "ftype == LLAMA_FTYPE_MOSTLY_NVFP4" || say "  - la regla del NVFP4 (src/llama-quant.cpp)"
    ya_aplicado "common/json-schema-to-grammar.cpp" "Empty pattern" || say "  - los patterns sin anclar (common/json-schema-to-grammar.cpp)"
    exit 1
fi

if [[ "$MODE" == "revert" ]]; then
    if [[ "$faltan" == "0" ]]; then
        say "revierto con git (en el clon del motor):"
        ( cd "$LLAMA_DIR" && git checkout -- src/llama-quant.cpp common/json-schema-to-grammar.cpp ) \
            && ok "revertido" || err "fallo el revert"
        exit $?
    fi
    warn "no estan los dos parches puestos: revierto lo que haya"
fi

if [[ "$faltan" == "0" ]]; then
    say "los dos parches ya estan aplicados. No hago nada."
    exit 0
fi

# ── aplicar ────────────────────────────────────────────────────────────────
# Se aplica con "patch" y no con "git apply" porque el clon puede tener
# otros cambios y "git apply" es mas rigido con el contexto.
# Con "-R" se revierte.
say "aplico el parche (copia de seguridad antes)..."
for f in src/llama-quant.cpp common/json-schema-to-grammar.cpp; do
    if [[ -f "$LLAMA_DIR/$f" && ! -f "$LLAMA_DIR/$f.ropero.bak" ]]; then
        cp "$LLAMA_DIR/$f" "$LLAMA_DIR/$f.ropero.bak"
    fi
done

if patch -p1 -d "$LLAMA_DIR" --forward --batch < "$PATCH" >/dev/null 2>&1; then
    ok "parche aplicado"
elif [[ "$faltan" == "1" ]]; then
    # A lo mejor solo estaba puesto uno de los dos
    warn "el parche completo no entra (igual ya estaba puesto uno de los dos)"
    if patch -p1 -d "$LLAMA_DIR" --forward --batch < "$PATCH" 2>&1 | head -5; then
        :
    fi
else
    err "el parche no entra limpio"
    err "  las copias .ropero.bak estan junto a los ficheros"
    exit 1
fi

nuevos="$(estado)"
if [[ "$nuevos" != "0" ]]; then
    err "despues de aplicar, faltan $nuevos parche(s)"
    exit 1
fi
ok "los dos parches estan puestos"

# ── compilar ───────────────────────────────────────────────────────────────
# Sin recompilar, el parche no sirve de nada: el motor que corre es el binario.
say "recompilo el motor (hace falta: el parche es de codigo)..."
if ! command -v cmake >/dev/null 2>&1; then
    warn "no hay cmake: compila el motor a mano"
    exit 0
fi
build_dir="$LLAMA_DIR/build"
[[ -d "$build_dir" ]] || build_dir="$LLAMA_DIR"
if ( cd "$build_dir" && cmake --build . --config Release -j "$(nproc)" ) >/tmp/ropero-parche-build.log 2>&1; then
    ok "motor recompilado"
else
    err "fallo la compilacion (log: /tmp/ropero-parche-build.log)"
    exit 1
fi
say "ya puedes arrancar el modelo. Si el server ya estaba corriendo, parralo:"
say "  ropero stop <modelo>"
