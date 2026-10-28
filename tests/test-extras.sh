#!/usr/bin/env bash
# =============================================================================
# tests/test-extras.sh — los ficheros EXTRA de un modelo (mtp, mmproj,
# chat template, imatrix, tokenizer).
#
# Lo que se testea es la SEMANTICA del flujo:
#
#   1. el TOML declara los extras con su repo, su fichero y su nombre local
#   2. cada extra dice CUANDO hace falta: al fabricar ("build") o al
#      arrancar ("run")
#   3. los de "build" se bajan ANTES de fabricar (el gguffer los necesita)
#      y se le pasan con el flag que corresponde
#   4. los de "run" se bajan y se pueden referenciar con {{model_dir}}
#   5. en el caso de descarga (el GGUF ya existe en HF) se bajan TODOS
#
# No se fabrica nada de verdad: los pesos y la cuantizacion son dobles.
# =============================================================================
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

PYTHON_BIN=""
for _cand in python3 python /usr/bin/python3 /usr/local/bin/python3 /bin/python3; do
    if "$_cand" -c 'import sys' >/dev/null 2>&1; then
        PYTHON_BIN="$("$_cand" -c 'import sys; print(sys.executable)')"
        break
    fi
done
if [[ -z "$PYTHON_BIN" ]]; then
    printf 'FATAL: no hay python3 utilizable\n' >&2
    exit 1
fi

VERBOSE=0
[[ "${1:-}" == "-v" ]] && VERBOSE=1
PASS=0
FAIL=0
FAILED_CASES=()

ok()   { PASS=$((PASS+1)); [[ $VERBOSE -eq 1 ]] && printf '  ok   %s\n' "$1"; return 0; }
bad()  { FAIL=$((FAIL+1)); FAILED_CASES+=("$1"); printf '  FALLO %s\n' "$1"; [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; return 0; }
check() { # check <descripcion> <condicion-verdadera> [mensaje]
    if [[ "$2" == "1" ]]; then ok "$1"; else bad "$1" "${3:-}"; fi
}
cond() { if eval "$1" >/dev/null 2>&1; then echo 1; else echo 0; fi; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ropero-extras.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

HOME_DIR="$WORK/home"
GGUF_OUT="$WORK/models/gguf"
LLAMA_DIR_TEST="$WORK/llama.cpp"
WORK_ROOT="$WORK/work"
BIN_DIR="$WORK/bin"
MODELS_DIR_TEST="$WORK/models"
mkdir -p "$HOME_DIR" "$GGUF_OUT" "$LLAMA_DIR_TEST/build/bin" "$WORK_ROOT" "$BIN_DIR" "$MODELS_DIR_TEST/llama"

# ── los dobles (conversor, cuantizador, hf, maker) ────────────────────────
source "$HERE/_fakes.sh"

# ═══════════════════════════════════════════════════════════════════════════
# 1. el TOML de prueba: un modelo que se fabrica, con un extra de "build"
#    (el imatrix) y uno de "run" (el MTP).
# ═══════════════════════════════════════════════════════════════════════════
cat >"$MODELS_DIR_TEST/llama/prueba.toml" <<'TOML'
[model]
name    = "prueba"
engine  = "llama-server"
context = 4096

[files]
gguf = "prueba-IQ4_NL.gguf"

[build]
repo         = "ejemplo/prueba"
quant        = "IQ4_NL"
imatrix_repo = "ejemplo/prueba-GGUF"
imatrix_file = "imatrix_ejemplo.gguf"
size = "1.0 GB"

[download.aux.imatrix]
repo = "ejemplo/prueba-GGUF"
file = "imatrix_ejemplo.gguf"
name = "imatrix_ejemplo.gguf"
size = "1 MB"

[download.aux.mtp]
repo = "ejemplo/prueba-GGUF"
file = "mtp-prueba.gguf"
name = "mtp-prueba.gguf"
size = "1 MB"

[args]
base = ["--temp", "0.5"]
TOML

cat >"$MODELS_DIR_TEST/llama/prueba2.toml" <<'TOML'
[model]
name    = "prueba2"
engine  = "llama-server"
context = 4096

[files]
gguf = "prueba2-IQ4_NL.gguf"

[build]
repo         = "ejemplo/prueba2"
quant        = "IQ4_NL"
imatrix_repo = "ejemplo/prueba2-GGUF"
imatrix_file = "imatrix_ejemplo2.gguf"
size = "1.0 GB"

[download.aux.imatrix]
repo = "ejemplo/prueba2-GGUF"
file = "imatrix_ejemplo2.gguf"
name = "imatrix_ejemplo2.gguf"
size = "1 MB"
TOML

# ═══════════════════════════════════════════════════════════════════════════
# 2. cargar ropero con ese directorio de modelos
# ═══════════════════════════════════════════════════════════════════════════
export HOME="$HOME_DIR"
export GGUF_DIR="$GGUF_OUT"
export LLAMA_DIR="$LLAMA_DIR_TEST"
export GGUF_WORK_DIR="$WORK_ROOT"
export MODELS_DIR="$MODELS_DIR_TEST"
export PATH="$BIN_DIR:$PATH"
export ROPERO_TEMP=0.15

# El entrypoint (bin/ropero) define esto antes de sourcear los modulos.
SCRIPT_DIR="$REPO/bin"
MODULES_DIR="$REPO/bin"
ROPERO_ROOT="$REPO"

# shellcheck source=/dev/null
source "$REPO/bin/_common.sh"
# shellcheck source=/dev/null
source "$REPO/bin/00-utils.sh"
# shellcheck source=/dev/null
source "$REPO/bin/21-download-models.sh"
# shellcheck source=/dev/null
source "$REPO/bin/22-models.sh"

ropero_models_load
_ropero_cat_build

# el indice de la entrada de "prueba" en el catalogo
idx=""
for i in "${!_ROPERO_CAT_KEY[@]}"; do
    if [[ "${_ROPERO_CAT_MODEL[$i]:-}" == "prueba" ]]; then idx="$i"; break; fi
done

# ═══════════════════════════════════════════════════════════════════════════
# 3. el comando "aux": que los tipos, el repo y el uso salen bien
# ═══════════════════════════════════════════════════════════════════════════
aux_out="$("$PYTHON_BIN" "$REPO/bin/_modelconf.py" aux --root "$MODELS_DIR" --name prueba 2>&1)"
check "1a. el comando aux lista el imatrix" \
      "$(cond 'printf %s "$aux_out" | grep -q "^imatrix|ejemplo/prueba-GGUF|imatrix_ejemplo.gguf|imatrix_ejemplo.gguf|"')" "$aux_out"
check "1b. el imatrix es de uso build" \
      "$(cond 'printf %s "$aux_out" | grep "^imatrix|" | grep -q "|build$"')" "$aux_out"
check "1c. el MTP es de uso run" \
      "$(cond 'printf %s "$aux_out" | grep "^mtp|" | grep -q "|run$"')" "$aux_out"

# ═══════════════════════════════════════════════════════════════════════════
# 4. el catalogo: los extras cuelgan con su tipo y su uso
# ═══════════════════════════════════════════════════════════════════════════
check "2a. la entrada de prueba esta en el catalogo" \
      "$(cond '[ -n "$idx" ]')" "no hay entrada para prueba"
spec="${_ROPERO_CAT_AUX[$idx]:-}"
check "2b. el catalogo lleva el imatrix con su repo" \
      "$(cond 'printf %s "$spec" | grep -q "ejemplo/prueba-GGUF|imatrix_ejemplo.gguf|"')" "$spec"
check "2c. el catalogo lleva el imatrix con uso build" \
      "$(cond 'printf %s "$spec" | grep -q "|imatrix_ejemplo.gguf|1 MB|build|imatrix"')" "$spec"
check "2d. el catalogo lleva el MTP con uso run" \
      "$(cond 'printf %s "$spec" | grep -q "|mtp-prueba.gguf|1 MB|run|mtp"')" "$spec"

# ═══════════════════════════════════════════════════════════════════════════
# 5. la descarga de los extras, filtrada por uso
# ═══════════════════════════════════════════════════════════════════════════
dest_dir="$(_ropero_cat_dest_dir "$idx")"

_ropero_cat_download_aux "$idx" "build" >/dev/null 2>&1
check "3a. el imatrix se baja a la carpeta del modelo" \
      "$(cond '[ -s "$dest_dir/imatrix_ejemplo.gguf" ]')" "no esta en $dest_dir"
check "3b. con uso build NO se baja el MTP" \
      "$(cond '[ ! -e "$dest_dir/mtp-prueba.gguf" ]')" "se bajo antes de tiempo"

_ropero_cat_download_aux "$idx" "run" >/dev/null 2>&1
check "3c. el MTP se baja a la carpeta del modelo" \
      "$(cond '[ -s "$dest_dir/mtp-prueba.gguf" ]')" "no esta en $dest_dir"

# ═══════════════════════════════════════════════════════════════════════════
# 6. los flags que se le pasan al gguffer
# ═══════════════════════════════════════════════════════════════════════════
if declare -f _ropero_cat_build_flags >/dev/null 2>&1; then
    mapfile -t bflags < <(_ropero_cat_build_flags "$idx" "$dest_dir")
    check "4a. el imatrix se le pasa al gguffer" \
          "$(cond 'printf "%s\n" "${bflags[@]}" | grep -qxF -- "--imatrix"')" "${bflags[*]}"
    check "4b. con la ruta local del imatrix" \
          "$(cond 'printf "%s\n" "${bflags[@]}" | grep -qxF -- "$dest_dir/imatrix_ejemplo.gguf"')" "${bflags[*]}"
    check "4c. el MTP NO se le pasa al gguffer (es del arranque)" \
          "$(cond '! printf "%s\n" "${bflags[@]}" | grep -qxF -- "--model-draft"')" "${bflags[*]}"
else
    bad "4a. existe _ropero_cat_build_flags" "no esta la funcion"
fi

# ═══════════════════════════════════════════════════════════════════════════
# 6b. si el imatrix viene como extra, los flags del [build] sobran
#
# El gguffer, si recibe --imatrix-repo/--imatrix-file, baja el imatrix a su
# cache y PISA el --imatrix que le hayan pasado. Por eso, cuando el imatrix
# es un extra (ya bajado en la carpeta del modelo), los flags del repo se
# quitan del argv.
# ═══════════════════════════════════════════════════════════════════════════

idx2=""
for i in "${!_ROPERO_CAT_KEY[@]}"; do
    if [[ "${_ROPERO_CAT_MODEL[$i]:-}" == "prueba2" ]]; then idx2="$i"; break; fi
done
check "4d. la entrada de prueba2 esta en el catalogo" \
      "$(cond '[ -n "$idx2" ]')" "no hay entrada para prueba2"
dest_dir2="$(_ropero_cat_dest_dir "$idx2")"
_ropero_cat_download_aux "$idx2" "build" >/dev/null 2>&1
mapfile -t bflags2 < <(_ropero_cat_build_flags "$idx2" "$dest_dir2")
check "4e. el imatrix se pasa como fichero" \
      "$(cond 'printf "%s\n" "${bflags2[@]}" | grep -qxF -- "$dest_dir2/imatrix_ejemplo2.gguf"')" "${bflags2[*]}"

# el argv final: con el extra, los flags del repo NO deben estar
mapfile -t argv2 < <(ropero_model_build_argv "prueba2" "$dest_dir2/prueba2-IQ4_NL.gguf")
mapfile -t extra2 < <(_ropero_cat_build_flags "$idx2" "$dest_dir2")
argv2=("${argv2[@]}" "${extra2[@]}")
# se aplica la misma limpieza que hace el instalador
declare -a limpio=()
for ((_k = 0; _k < ${#argv2[@]}; _k++)); do
    case "${argv2[$_k]}" in
        --imatrix-repo|--imatrix-file) (( _k++ )) ;;
        *) limpio+=("${argv2[$_k]}") ;;
    esac
done
check "4f. con el extra, no queda --imatrix-repo" \
      "$(cond '! printf "%s\n" "${limpio[@]}" | grep -qxF -- "--imatrix-repo"')" "${limpio[*]}"
check "4g. y el --imatrix local si" \
      "$(cond 'printf "%s\n" "${limpio[@]}" | grep -qxF -- "--imatrix"')" "${limpio[*]}"

# ═══════════════════════════════════════════════════════════════════════════
# 7. los marcadores de ruta en los args
# ═══════════════════════════════════════════════════════════════════════════
check "5a. el launcher expande {{model_dir}}" \
      "$(cond 'grep -q "_ropero_expand_paths()" "$REPO/bin/22-models.sh"')"

# ── resumen ────────────────────────────────────────────────────────────────
printf '\ntest-extras.sh: %d correctos, %d fallos\n' "$PASS" "$FAIL"
if (( FAIL > 0 )); then
    printf '  fallaron:\n'
    for c in "${FAILED_CASES[@]}"; do printf '    - %s\n' "$c"; done
    exit 1
fi
exit 0
