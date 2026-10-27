# bin/20-compile-llama.sh
# Compila llama-server desde el repo clonado en $LLAMA_DIR.
# Esta lógica se extrajo del entrypoint ropero (cmd_install → _ropero_compile_llama)
# para modularizar la instalación.
#
# Llamada: ropero_compile_llama <os>
#   os = cachy | arch | macos | termux
#
# Variables esperadas:
#   $LLAMA_DIR       (path al repo de llama.cpp ya clonado)
#   $ROPERO_NPROC    (opcional; si no, nproc/hw.ncpu/8)
#   $ROPERO_CUDA_PATH (opcional; si no, /opt/cuda-13.3 y /opt/cuda)
#   $ROPERO_HOST_CXX  (opcional; si no, g++-14/g++-13/clang++)
#
# Flags de compilación (sólo opciones que existen en el CMake actual):
#   macOS  → Metal nativo, librería Metal embebida, sin debug de Metal
#   Cachy  → CUDA nativo (sm_120a en la RTX 5080), fast math, IPO, FA/MMQ
#   otros  → error (no soportado)
#
# Nota: los kernels MMQ de MXFP4/NVFP4 se compilan por defecto en ggml-cuda;
# no existen opciones GGML_CUDA_MMQ_MXFP4/NVFP4. Tampoco existen
# GGML_METAL_USE_MPS, GGML_METAL_FLASH_ATTN ni GGML_METAL_USE_BF16.

ropero_compile_llama() {
    local os="$1"
    local nproc="${ROPERO_NPROC:-}"
    if [[ -z "$nproc" ]]; then
        if command -v nproc >/dev/null 2>&1; then
            nproc=$(nproc)
        elif command -v sysctl >/dev/null 2>&1; then
            nproc=$(sysctl -n hw.ncpu 2>/dev/null || echo 8)
        else
            nproc=8
        fi
    fi

    if [[ -z "${LLAMA_DIR:-}" ]]; then
        ropero_err "LLAMA_DIR no está seteado"; return 1
    fi
    if [[ ! -d "$LLAMA_DIR" ]]; then
        ropero_err "LLAMA_DIR no existe: $LLAMA_DIR"; return 1
    fi

    local build_dir="$LLAMA_DIR/build"
    local stamp="$build_dir/.ropero-build"
    if [[ -x "$build_dir/bin/llama-server" ]]; then
        ropero_ok "llama-server ya compilado (skip)"
        if [[ -f "$stamp" ]]; then
            ropero_info "  build: $(tr '\n' ' ' < "$stamp")"
        else
            ropero_info "  build sin sello; si cambiaste flags, recompila con 'ropero install engines'"
        fi
        return 0
    fi

    local build_cmake="cmake"
    local config_summary=""

    case "$os" in
        macos)
            ropero_info "Compilando llama.cpp (macOS, Metal)…"
            # macOS: usar cmake/ninja del sistema (no asdf shims). En macOS
            # con asdf, los shims requieren que .tool-versions defina cada
            # comando; si no, falla con "No version is set for command ninja".
            # setup_resolve_build_tool busca primero en /opt/homebrew/bin y
            # /usr/local/bin (brew) antes de caer al PATH (que tiene shims).
            local SYS_CMAKE SYS_NINJA
            SYS_CMAKE=$(setup_resolve_build_tool cmake) || { ropero_err "cmake no disponible"; return 1; }
            SYS_NINJA=$(setup_resolve_build_tool ninja) || { ropero_err "ninja no disponible"; return 1; }
            build_cmake="$SYS_CMAKE"
            ropero_info "  cmake: $SYS_CMAKE | ninja: $SYS_NINJA"
            "$SYS_CMAKE" -B "$build_dir" -S "$LLAMA_DIR" -G Ninja \
                -DCMAKE_MAKE_PROGRAM="$SYS_NINJA" \
                -DCMAKE_BUILD_TYPE=Release \
                -DCMAKE_OSX_ARCHITECTURES=arm64 \
                -DCMAKE_C_COMPILER=clang \
                -DCMAKE_CXX_COMPILER=clang++ \
                -DCMAKE_C_FLAGS="-O3 -mcpu=native" \
                -DCMAKE_CXX_FLAGS="-O3 -mcpu=native" \
                -DGGML_METAL=ON \
                -DGGML_METAL_NDEBUG=ON \
                -DGGML_METAL_EMBED_LIBRARY=ON \
                -DGGML_NATIVE=ON \
                -DGGML_ACCELERATE=ON \
                -DGGML_OPENMP=OFF \
                -DGGML_BLAS=OFF \
                -DLLAMA_OPENSSL=ON \
                -DBUILD_SHARED_LIBS=OFF \
                -DLLAMA_BUILD_SERVER=ON \
                -DLLAMA_BUILD_TOOLS=ON \
                -DLLAMA_BUILD_APP=OFF \
                -DLLAMA_BUILD_EXAMPLES=OFF \
                -DLLAMA_BUILD_TESTS=OFF \
                -DLLAMA_BUILD_UI=OFF \
                -DLLAMA_BUILD_HTML=OFF
            config_summary="macOS Metal arm64"
            ;;
        cachy|arch|wsl2)
            ropero_info "Compilando llama.cpp (Cachy/Arch, CUDA)…"
            local cuda_path="${ROPERO_CUDA_PATH:-/opt/cuda-13.3}"
            if [[ ! -x "$cuda_path/bin/nvcc" && -z "${ROPERO_CUDA_PATH:-}" ]]; then
                cuda_path="/opt/cuda"
            fi
            if [[ ! -x "$cuda_path/bin/nvcc" ]]; then
                ropero_err "nvcc no encontrado en $cuda_path"
                return 1
            fi
            local host_cxx="${ROPERO_HOST_CXX:-}"
            local c
            if [[ -z "$host_cxx" ]]; then
                for c in /usr/bin/g++-14 /usr/bin/g++-13 /usr/bin/clang++; do
                    if [[ -x "$c" ]]; then host_cxx="$c"; break; fi
                done
            fi
            if [[ -z "$host_cxx" ]]; then
                ropero_err "sin host compiler (g++-14/g++-13/clang++)"
                return 1
            fi
            local host_cc="$host_cxx"
            case "$host_cxx" in
                */g++-*) host_cc="${host_cxx/g++/gcc}" ;;
                */clang++) host_cc="${host_cxx/clang++/clang}" ;;
            esac
            cmake -B "$build_dir" -S "$LLAMA_DIR" -G Ninja \
                -DCMAKE_BUILD_TYPE=Release \
                -DCUDA_TOOLKIT_ROOT_DIR="$cuda_path" \
                -DCMAKE_CUDA_COMPILER="$cuda_path/bin/nvcc" \
                -DCMAKE_CUDA_HOST_COMPILER="$host_cxx" \
                -DCMAKE_CXX_COMPILER="$host_cxx" \
                -DCMAKE_C_COMPILER="$host_cc" \
                -DCMAKE_CXX_FLAGS="-march=native -O3" \
                -DCMAKE_C_FLAGS="-march=native -O3" \
                -DCMAKE_CUDA_FLAGS="--use_fast_math -O3" \
                -DCMAKE_CUDA_ARCHITECTURES="native" \
                -DCMAKE_INTERPROCEDURAL_OPTIMIZATION=ON \
                -DGGML_CUDA=ON \
                -DGGML_CUDA_FA=ON \
                -DGGML_CUDA_FA_ALL_QUANTS=ON \
                -DGGML_CUDA_GRAPHS=ON \
                -DGGML_CUDA_FORCE_MMQ=OFF \
                -DGGML_CUDA_FORCE_CUBLAS=OFF \
                -DGGML_CUDA_COMPRESSION_MODE=speed \
                -DGGML_RPC=ON \
                -DGGML_OPENMP=ON \
                -DGGML_NATIVE=ON \
                -DGGML_AVX2=ON \
                -DGGML_AVX512=ON \
                -DGGML_AVX512_VBMI=ON \
                -DGGML_AVX512_VNNI=ON \
                -DGGML_AVX512_BF16=ON \
                -DGGML_FMA=ON \
                -DLLAMA_OPENSSL=ON \
                -DBUILD_SHARED_LIBS=OFF \
                -DLLAMA_BUILD_SERVER=ON \
                -DLLAMA_BUILD_TOOLS=ON \
                -DLLAMA_BUILD_APP=OFF \
                -DLLAMA_BUILD_EXAMPLES=OFF \
                -DLLAMA_BUILD_TESTS=OFF \
                -DLLAMA_BUILD_UI=OFF \
                -DLLAMA_BUILD_HTML=OFF
            config_summary="CUDA native ($cuda_path)"
            # Notas ROCm / Vulkan (sin activar por defecto, ver README):
            #   export HSA_OVERRIDE_GFX_VERSION=11.0.0
            #   -DGGML_HIPBLAS=ON
            #   sudo pacman -S rocm-hip-sdk rocm-core hipblas rocblas
            ;;
        *)
            ropero_err "SO no soportado para compilar llama.cpp: $os"
            return 1
            ;;
    esac

    if ! "$build_cmake" --build "$build_dir" -j"$nproc"; then
        ropero_err "falló la compilación de llama.cpp"
        return 1
    fi
    if [[ ! -x "$build_dir/bin/llama-server" ]]; then
        ropero_err "la compilación terminó pero no existe $build_dir/bin/llama-server"
        return 1
    fi

    # Sello de build: sirve para saber con qué configuración se compiló. No
    # decide si se recompila; el flujo de instalación ya pregunta antes.
    {
        printf 'os=%s\n' "$os"
        printf 'commit=%s\n' "$(git -C "$LLAMA_DIR" rev-parse --short HEAD 2>/dev/null || echo unknown)"
        printf 'arch=%s\n' "$(uname -m 2>/dev/null || echo '?')"
        printf 'jobs=%s\n' "$nproc"
        printf 'flags=%s\n' "$config_summary"
    } > "$stamp" || {
        ropero_err "no se pudo escribir el sello de build $stamp"
        return 1
    }
    ropero_ok "llama-server compilado ($config_summary)"
}
