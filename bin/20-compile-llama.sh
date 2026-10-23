# bin/20-compile-llama.sh
# Compila llama-server desde el repo clonado en $LLAMA_DIR.
# Esta lógica se extrajo del entrypoint ropero (cmd_install → _ropero_compile_llama)
# para modularizar la instalación.
#
# Llamada: ropero_compile_llama <os>
#   os = cachy | arch | macos | termux
#
# Variables esperadas:
#   $LLAMA_DIR  (path al repo de llama.cpp ya clonado)
#   $ROPERO_NPROC (nproc, para -j)
#
# Flags de compilación:
#   macOS  → Metal (clang, -mcpu=native, GGML_METAL_*)
#   Cachy  → CUDA 13 (gcc-14, sm_120a = RTX 5080, fast math, IPO, MMQ NVFP4/MXFP4)
#   otros  → error (no soportado)

ropero_compile_llama() {
    local os="$1"
    local nproc="${ROPERO_NPROC:-$(command -v nproc >/dev/null && nproc || echo 8)}"

    if [[ -z "${LLAMA_DIR:-}" ]]; then
        ropero_err "LLAMA_DIR no está seteado"; return 1
    fi
    if [[ ! -d "$LLAMA_DIR" ]]; then
        ropero_err "LLAMA_DIR no existe: $LLAMA_DIR"; return 1
    fi
    if [[ -x "$LLAMA_DIR/build/bin/llama-server" ]]; then
        ropero_ok "llama-server ya compilado (skip)"
        return 0
    fi

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
            ropero_info "  cmake: $SYS_CMAKE | ninja: $SYS_NINJA"
            # Spec macOS: clang + Metal + MPS + FlashAttn + BF16 + Embed library
            "$SYS_CMAKE" -B "$LLAMA_DIR/build" -S "$LLAMA_DIR" -G Ninja \
                -DCMAKE_MAKE_PROGRAM="$SYS_NINJA" \
                -DCMAKE_BUILD_TYPE=Release \
                -DCMAKE_OSX_ARCHITECTURES=arm64 \
                -DCMAKE_C_COMPILER=clang \
                -DCMAKE_CXX_COMPILER=clang++ \
                -DCMAKE_C_FLAGS="-O3 -mcpu=native" \
                -DCMAKE_CXX_FLAGS="-O3 -mcpu=native" \
                -DGGML_METAL=ON \
                -DGGML_METAL_USE_MPS=ON \
                -DGGML_METAL_FLASH_ATTN=ON \
                -DGGML_METAL_USE_BF16=ON \
                -DGGML_METAL_EMBED_LIBRARY=ON \
                -DGGML_NATIVE=ON \
                -DGGML_OPENMP=OFF \
                -DGGML_BLAS=OFF \
                -DLLAMA_OPENSSL=ON \
                -DBUILD_SHARED_LIBS=OFF \
                -DLLAMA_BUILD_SERVER=ON \
                -DLLAMA_BUILD_TOOLS=ON \
                -DLLAMA_BUILD_EXAMPLES=OFF \
                -DLLAMA_BUILD_TESTS=OFF \
                -DLLAMA_BUILD_UI=OFF
            ;;
        cachy|arch|wsl2)
            ropero_info "Compilando llama.cpp (Cachy/Arch, CUDA)…"
            local cuda_path="/opt/cuda-13.3"
            [[ -x "$cuda_path/bin/nvcc" ]] || cuda_path="/opt/cuda"
            if [[ ! -x "$cuda_path/bin/nvcc" ]]; then
                ropero_err "nvcc no encontrado en $cuda_path"
                return 1
            fi
            local host_cxx=""
            local c
            for c in /usr/bin/g++-14 /usr/bin/g++-13 /usr/bin/clang++; do
                if [[ -x "$c" ]]; then host_cxx="$c"; break; fi
            done
            if [[ -z "$host_cxx" ]]; then
                ropero_err "sin host compiler (g++-14/g++-13/clang++)"
                return 1
            fi
            local host_cc="$host_cxx"
            case "$host_cxx" in
                */g++-*) host_cc="${host_cxx/g++/gcc}" ;;
                */clang++) host_cc="${host_cxx/clang++/clang}" ;;
            esac
            cmake -B "$LLAMA_DIR/build" -S "$LLAMA_DIR" -G Ninja \
                -DCMAKE_BUILD_TYPE=Release \
                -DCUDA_TOOLKIT_ROOT_DIR="$cuda_path" \
                -DCMAKE_CUDA_COMPILER="$cuda_path/bin/nvcc" \
                -DCMAKE_CUDA_HOST_COMPILER="$host_cxx" \
                -DCMAKE_CXX_COMPILER="$host_cxx" \
                -DCMAKE_C_COMPILER="$host_cc" \
                -DCMAKE_CXX_FLAGS="-march=native -O3 -ftree-vectorize" \
                -DCMAKE_C_FLAGS="-march=native -O3 -ftree-vectorize" \
                -DCMAKE_CUDA_FLAGS="--use_fast_math -O3 -lineinfo" \
                -DCMAKE_CUDA_ARCHITECTURES="120a" \
                -DCMAKE_INTERPROCEDURAL_OPTIMIZATION=ON \
                -DGGML_CUDA_MMQ_MXFP4=ON \
                -DGGML_CUDA_MMQ_NVFP4=ON \
                -DGGML_CUDA=ON \
                -DGGML_CUDA_FA=ON \
                -DGGML_CUDA_FA_ALL_QUANTS=ON \
                -DGGML_CUDA_GRAPHS=ON \
                -DGGML_CUDA_FORCE_MMQ=OFF \
                -DGGML_CUDA_FORCE_CUBLAS=OFF \
                -DGGML_CUDA_F16=OFF \
                -DGGML_CUDA_COMPRESSION_MODE=speed \
                -DGGML_RPC=ON \
                -DGGML_OPENMP=ON \
                -DGGML_NATIVE=ON \
                -DGGML_AVX2=ON \
                -DGGML_AVX512=ON \
                -DGGML_AVX512_VBMI=ON \
                -DGGML_AVX512_VNNI=ON \
                -DGGML_AVX512_BF16=ON \
                -DGGML_AVX512_FP16=ON \
                -DGGML_FMA=ON \
                -DLLAMA_OPENSSL=ON \
                -DBUILD_SHARED_LIBS=OFF \
                -DLLAMA_BUILD_SERVER=ON \
                -DLLAMA_BUILD_TOOLS=ON \
                -DLLAMA_BUILD_EXAMPLES=OFF \
                -DLLAMA_BUILD_TESTS=OFF \
                -DLLAMA_BUILD_UI=OFF \
                -DLLAMA_BUILD_EXTRA=ON
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
    cmake --build "$LLAMA_DIR/build" -j"$nproc"
}
