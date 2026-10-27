#!/usr/bin/env bash
# Contract test for bin/20-compile-llama.sh.
#
# No compila nada de verdad: sustituye cmake/ninja/nvcc por dobles que
# registran los argumentos y crean un build/bin/llama-server falso. Así se
# verifica que los flags existen en el CMake actual y que macOS usa el cmake
# resuelto (no el shim de asdf).

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
SRC="$REPO/bin/20-compile-llama.sh"

pass=0
fail=0
ok()  { printf '  \033[32m✓\033[0m %s\n' "$1"; pass=$((pass+1)); }
bad() { printf '  \033[31m✗\033[0m %s\n' "$1"; fail=$((fail+1)); }
head2() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }
has() { if grep -qF -- "$2" <<< "$1"; then ok "$3"; else bad "$3"; fi; }
hasnt() { if grep -qF -- "$2" <<< "$1"; then bad "$3"; else ok "$3"; fi; }

[[ -f "$SRC" ]] || { echo "no encuentro $SRC" >&2; exit 1; }

head2 "sintaxis"
if bash -n "$SRC" 2>/dev/null; then ok "bash -n $SRC"; else bad "bash -n $SRC"; fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ropero-compile.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

BIN="$WORK/bin"
NOBIN="$WORK/nobin"
mkdir -p "$BIN" "$NOBIN" "$WORK/cuda/bin"
FAKE_CMAKE="$BIN/cmake"
FAKE_NINJA="$BIN/ninja"
FAKE_NVCC="$WORK/cuda/bin/nvcc"

cat > "$FAKE_CMAKE" <<'FAKE'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "${CMAKE_LOG:?}"
if [[ " $* " == *" --build "* ]]; then
    mkdir -p "${LLAMA_DIR:?}/build/bin"
    printf '#!/bin/sh\nexit 0\n' > "$LLAMA_DIR/build/bin/llama-server"
    chmod +x "$LLAMA_DIR/build/bin/llama-server"
fi
exit 0
FAKE
cat > "$FAKE_NINJA" <<'FAKE'
#!/usr/bin/env bash
exit 0
FAKE
cat > "$FAKE_NVCC" <<'FAKE'
#!/usr/bin/env bash
exit 0
FAKE
chmod +x "$FAKE_CMAKE" "$FAKE_NINJA" "$FAKE_NVCC"

run_compile() {
    local os="$1" dir="$2" log="$3" runpath
    mkdir -p "$dir/llama" "$dir/cuda/bin"
    cp "$FAKE_NVCC" "$dir/cuda/bin/nvcc"
    chmod +x "$dir/cuda/bin/nvcc"
    : >> "$log"
    if [[ "$os" == "macos" ]]; then
        runpath="$NOBIN:$PATH"
    else
        runpath="$BIN:$PATH"
    fi
    env PATH="$runpath" \
        LLAMA_DIR="$dir/llama" \
        CMAKE_LOG="$log" \
        ROPERO_NPROC=2 \
        ROPERO_CUDA_PATH="$dir/cuda" \
        ROPERO_HOST_CXX=/usr/bin/true \
        FAKE_CMAKE="$FAKE_CMAKE" \
        FAKE_NINJA="$FAKE_NINJA" \
        bash -c '
            source "$1" || exit 1
            ropero_info() { printf "INFO %s\n" "$*"; }
            ropero_ok()   { printf "OK %s\n" "$*"; }
            ropero_err()  { printf "ERR %s\n" "$*" >&2; }
            setup_resolve_build_tool() {
                case "$1" in
                    cmake) printf "%s\n" "$FAKE_CMAKE" ;;
                    ninja) printf "%s\n" "$FAKE_NINJA" ;;
                esac
            }
            ropero_compile_llama "$2"
        ' _ "$SRC" "$os"
}

head2 "CachyOS / CUDA"
CACHY_DIR="$WORK/cachy"
CACHY_LOG="$WORK/cachy.cmake.log"
: > "$CACHY_LOG"
cachy_out=$(run_compile cachy "$CACHY_DIR" "$CACHY_LOG" 2>&1); cachy_rc=$?
if (( cachy_rc == 0 )); then ok "compilación Cachy simulada termina bien"; else bad "compilación Cachy falló: $cachy_out"; fi
cachy_cfg=$(sed -n '1p' "$CACHY_LOG")
cachy_build=$(sed -n '2p' "$CACHY_LOG")
has "$cachy_cfg" "-DGGML_CUDA=ON" "activa GGML_CUDA"
has "$cachy_cfg" "-DCMAKE_CUDA_ARCHITECTURES=native" "compila para la GPU nativa (sm_120a en la 5080)"
has "$cachy_cfg" "-DGGML_CUDA_FA_ALL_QUANTS=ON" "compila FlashAttention para todos los quants"
has "$cachy_cfg" "-DGGML_CUDA_COMPRESSION_MODE=speed" "usa compresión CUDA orientada a velocidad"
has "$cachy_cfg" "-DGGML_AVX512_BF16=ON" "activa AVX512-BF16 (Zen 5)"
has "$cachy_cfg" "-DLLAMA_BUILD_APP=OFF" "no compila el binario unificado"
has "$cachy_cfg" "-DLLAMA_BUILD_HTML=OFF" "no compila HTML"
has "$cachy_cfg" "--use_fast_math -O3" "usa fast math sin -lineinfo"
hasnt "$cachy_cfg" "-lineinfo" "no mete -lineinfo (release)"
hasnt "$cachy_cfg" "GGML_CUDA_MMQ_MXFP4" "no usa la opción inexistente GGML_CUDA_MMQ_MXFP4"
hasnt "$cachy_cfg" "GGML_CUDA_MMQ_NVFP4" "no usa la opción inexistente GGML_CUDA_MMQ_NVFP4"
hasnt "$cachy_cfg" "GGML_CUDA_F16" "no usa la opción inexistente GGML_CUDA_F16"
hasnt "$cachy_cfg" "GGML_AVX512_FP16" "no usa la opción inexistente GGML_AVX512_FP16"
hasnt "$cachy_cfg" "LLAMA_BUILD_EXTRA" "no usa la opción inexistente LLAMA_BUILD_EXTRA"
has "$cachy_build" "--build" "lanza cmake --build"
has "$cachy_build" "-j2" "respeta ROPERO_NPROC"
has "$(cat "$CACHY_DIR/llama/build/.ropero-build" 2>/dev/null)" "os=cachy" "escribe sello de build con os=cachy"
has "$(cat "$CACHY_DIR/llama/build/.ropero-build" 2>/dev/null)" "flags=CUDA native" "el sello registra la configuración CUDA"
has "$(cat "$CACHY_DIR/llama/build/.ropero-build" 2>/dev/null)" "jobs=2" "el sello registra los jobs"

# Segunda pasada: el binario ya existe; no debe reconfigurar ni recompilar.
before=$(wc -l < "$CACHY_LOG")
cachy_out2=$(run_compile cachy "$CACHY_DIR" "$CACHY_LOG" 2>&1); cachy_rc2=$?
after=$(wc -l < "$CACHY_LOG")
if (( cachy_rc2 == 0 )) && (( after == before )); then
    ok "segunda pasada hace skip sin llamar a cmake"
else
    bad "la segunda pasada volvió a compilar (rc=$cachy_rc2, líneas $before->$after)"
fi
has "$cachy_out2" "ya compilado (skip)" "el skip se anuncia"

head2 "macOS / Metal"
MAC_DIR="$WORK/macos"
MAC_LOG="$WORK/macos.cmake.log"
: > "$MAC_LOG"
mac_out=$(run_compile macos "$MAC_DIR" "$MAC_LOG" 2>&1); mac_rc=$?
if (( mac_rc == 0 )); then ok "compilación macOS simulada termina bien"; else bad "compilación macOS falló: $mac_out"; fi
mac_cfg=$(sed -n '1p' "$MAC_LOG")
mac_build=$(sed -n '2p' "$MAC_LOG")
has "$mac_cfg" "-DGGML_METAL=ON" "activa Metal"
has "$mac_cfg" "-DGGML_METAL_NDEBUG=ON" "desactiva el debug de Metal"
has "$mac_cfg" "-DGGML_METAL_EMBED_LIBRARY=ON" "embebe la librería Metal"
has "$mac_cfg" "-DGGML_ACCELERATE=ON" "usa Accelerate en CPU"
has "$mac_cfg" "-DLLAMA_BUILD_APP=OFF" "no compila el binario unificado"
has "$mac_cfg" "-DCMAKE_OSX_ARCHITECTURES=arm64" "compila arm64"
hasnt "$mac_cfg" "GGML_METAL_USE_MPS" "no usa la opción inexistente GGML_METAL_USE_MPS"
hasnt "$mac_cfg" "GGML_METAL_FLASH_ATTN" "no usa la opción inexistente GGML_METAL_FLASH_ATTN"
hasnt "$mac_cfg" "GGML_METAL_USE_BF16" "no usa la opción inexistente GGML_METAL_USE_BF16"
has "$mac_build" "--build" "el build de macOS usa el cmake resuelto"
has "$(cat "$MAC_DIR/llama/build/.ropero-build" 2>/dev/null)" "os=macos" "escribe sello de build con os=macos"
has "$(cat "$MAC_DIR/llama/build/.ropero-build" 2>/dev/null)" "flags=macOS Metal arm64" "el sello registra Metal arm64"

printf '\n'
if (( fail == 0 )); then
    printf '\033[32m✔ %d comprobaciones OK\033[0m\n' "$pass"
    exit 0
fi
printf '\033[31m✗ %d fallo(s) de %d comprobaciones\033[0m\n' "$fail" "$((pass+fail))"
exit 1
