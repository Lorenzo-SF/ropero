#!/usr/bin/env bash
# =============================================================================
# aplicar-parche-motor.sh — aplica los parches del motor (llama.cpp) al clon
# que hay en $LLAMA_DIR.
#
# POR QUE ESTO VIVE EN ROEPO Y NO EN EL MOTOR
# ────────────────────────────────────────────
# Los parches se aplican a un clon de llama.cpp que puede ser de OTRO dueño
# (el oficial, ggml-org/llama.cpp). Con el token de este repo no se puede
# push ahi (ni admin), asi que los parches se delivering COMO FICHEROS y se
# aplican aqui, en la maquina de cada uno.
#
# NUESTRO MOTOR (Lorenzo-SF/ropero-llama-turboquant) YA LOS TIENE
# HORNEADOS: los dos parches son commits del propio fork. Ahi este script no
# hace nada (lo dice: "los dos parches ya estan aplicados") y el binario que
# se compila ya sale con ellos. Se sigue invocando por dos razones: que el
# clon sea de cualquiera (y entonces hay que parchearlo) y que el check
# (`--check`) sirva de comprobacion de que el motor que se va a usar los tiene.
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
# 2. parche-motor-compressed-tensors-mixed-precision.patch
#
#    conversion/base.py — los checkpoints "mixed-precision" (NVFP4 + FP8 en
#    el mismo repo, como el Groot).
#       El conversor decidia si el checkpoint era NVFP4 con un "all(...)"
#       sobre los grupos de "config_groups": con precision mixta, un grupo es
#       FP8 y otro NVFP4, el "all" daba falso y la conversion reventaba:
#         NotImplementedError: Can't handle multiple config groups for
#         compressed-tensors yet
#       Upstream usa "any(...)" y ademas dequantiza el grupo FP8 con un
#       "dequant_fp8" anidado (que el fork no tenia). El parche trae las dos
#       cosas: el "any(...)" y el "dequant_fp8" (el grupo NVFP4 lo repackea
#       "_generate_nvfp4_tensors", que ya sabia hacerlo).
#
# USO
# ───
#   ropero/quant/aplicar-parche-motor.sh            # aplica (y compila)
#   ropero/quant/aplicar-parche-motor.sh --check    # solo dice si falta
#   ropero/quant/aplicar-parche-motor.sh --revert   # vuelve atras
#
# El clon del motor se toma de $LLAMA_DIR (o del primer argumento).
# =============================================================================
# SIN `errexit` (-e) NI `nounset` (-u): un fallo implícito (un grep que no
# encuentra, una tubería cuyo primer tramo falla) no aborta el script; los
# abortos críticos son explícitos (`exit 1`) y sí lo terminan. Por eso
# estos scripts SE EJECUTAN, no se sourcean: su `exit` mata tu shell.
# NO reintroducir errexit ni nounset (`pipefail` no puede terminar una shell).
set -o pipefail

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
PATCH_MIXED="$PATCHES_DIR/parche-motor-compressed-tensors-mixed-precision.patch"
[[ -f "$PATCH" ]] || { err "falta el parche: $PATCH"; exit 1; }
[[ -f "$PATCH_MIXED" ]] || { err "falta el parche: $PATCH_MIXED"; exit 1; }

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
    ya_aplicado "conversion/base.py" "def dequant_fp8" || n=$((n+1))
    printf '%d' "$n"
}

faltan="$(estado)"

if [[ "$MODE" == "check" ]]; then
    if [[ "$faltan" == "0" ]]; then
        say "los parches estan aplicados"
        exit 0
    fi
    say "faltan $faltan parche(s):"
    ya_aplicado "src/llama-quant.cpp" "ftype == LLAMA_FTYPE_MOSTLY_NVFP4" || say "  - la regla del NVFP4 (src/llama-quant.cpp)"
    ya_aplicado "common/json-schema-to-grammar.cpp" "Empty pattern" || say "  - los patterns sin anclar (common/json-schema-to-grammar.cpp)"
    ya_aplicado "conversion/base.py" "def dequant_fp8" || say "  - los checkpoints mixed-precision (conversion/base.py)"
    exit 1
fi

if [[ "$MODE" == "revert" ]]; then
    if [[ "$faltan" == "0" ]]; then
        say "revierto con git (en el clon del motor):"
        ( cd "$LLAMA_DIR" && git checkout -- src/llama-quant.cpp common/json-schema-to-grammar.cpp conversion/base.py ) \
            && ok "revertido" || err "fallo el revert"
        exit $?
    fi
    warn "no estan todos los parches puestos: revierto lo que haya"
fi

if [[ "$faltan" == "0" ]]; then
    say "los parches ya estan aplicados. No hago nada."
    exit 0
fi

# ── aplicar ────────────────────────────────────────────────────────────────
# Se aplica con "patch" y no con "git apply" porque el clon puede tener
# otros cambios y "git apply" es mas rigido con el contexto.
# Con "-R" se revierte.
say "aplico el parche (copia de seguridad antes)..."
for f in src/llama-quant.cpp common/json-schema-to-grammar.cpp conversion/base.py; do
    if [[ -f "$LLAMA_DIR/$f" && ! -f "$LLAMA_DIR/$f.ropero.bak" ]]; then
        cp "$LLAMA_DIR/$f" "$LLAMA_DIR/$f.ropero.bak"
    fi
done

# El parche 1 (los dos ficheros de una vez). Cada parche se aplica por
# separado: si uno ya esta puesto, el otro se aplica igual (el "--forward"
# del "patch" hace que un hunk ya aplicado se salte en vez de fallar).
aplicar_patch() {
    local fichero="$1" etiqueta="$2" salida
    salida="$(patch -p1 -d "$LLAMA_DIR" --forward --batch <"$fichero" 2>&1)"
    local rc=$?
    if (( rc == 0 )); then
        ok "$etiqueta aplicado"
        return 0
    fi
    # Con "--forward", "patch" sale con 1 si el hunk ya estaba puesto.
    if grep -qE "already applied|Reversed \(or previously applied\)" <<<"$salida"; then
        warn "$etiqueta: ya estaba puesto"
        return 0
    fi
    warn "$etiqueta: no entra limpio"
    printf '%s\n' "$salida" | head -5 | sed 's/^/    /'
    return 1
}

# Cada parche se aplica solo si falta alguna de SUS piezas: si esta todo
# puesto, se dice y no se toca el fichero (el "patch" avisaria de "already
# applied" y quedaria como un fallo).
faltan_1=0
ya_aplicado "src/llama-quant.cpp" "ftype == LLAMA_FTYPE_MOSTLY_NVFP4" || faltan_1=1
ya_aplicado "common/json-schema-to-grammar.cpp" "Empty pattern" || faltan_1=1
if (( faltan_1 )); then
    aplicar_patch "$PATCH" "parche 1 (la regla del NVFP4 + los patterns sin anclar)" || true
else
    say "  el parche 1 ya esta aplicado (regla del NVFP4 + patterns sin anclar)"
fi

if ya_aplicado "conversion/base.py" "def dequant_fp8"; then
    say "  el parche 2 ya esta aplicado (checkpoints mixed-precision)"
else
    aplicar_patch "$PATCH_MIXED" "parche 2 (los checkpoints mixed-precision)" || true
fi

nuevos="$(estado)"
if [[ "$nuevos" != "0" ]]; then
    err "despues de aplicar, faltan $nuevos parche(s)"
    exit 1
fi
ok "los parches estan puestos"

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
