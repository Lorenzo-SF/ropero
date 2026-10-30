#!/usr/bin/env bash
# =============================================================================
# engines/llama-server.sh — el motor principal: fork turboquant de llama.cpp
# (spec §6, fila 1)
# =============================================================================
# GGUF + servidor OpenAI propio con --api-key (auth nativa). La instalación
# clona el fork por HTTPS (no SSH: ADR-5), configura cmake (Metal en macOS,
# CUDA en Linux+NVIDIA), aplica los parches del motor (quant/aplicar-parche-
# motor.sh — NVFP4 de expertos + patterns sin anclar), compila y enlaza el
# binario en $BIN_DIR.
#
# ORDEN de install (importante): clone → cmake configure → parches → build.
# El script de parches COMPILA al final dentro de build/, así que build/
# (con su CMakeCache) tiene que existir ANTES: por eso el configure va
# delante. El build explícito del final es no-op incremental, pero cubre el
# caso "parches ya aplicados" (el script se va sin compilar).
# =============================================================================

rp_eng_llama-server_label() {
    printf 'llama-server · fork turboquant\n'
}

rp_eng_llama-server_compatible() {
    # macOS (Metal) · Linux (CUDA). "otro" (BSD, termux...) no.
    local os
    os="$(rp_detect_os)"
    case "$os" in
        macos|cachyos|arch|linux) return 0 ;;
        *)
            printf 'llama-server: SO no soportado (%s)\n' "$os" >&2
            return 1
            ;;
    esac
}

rp_eng_llama-server_deps() {
    printf 'git cmake ninja curl cc\n'
}

rp_eng_llama-server_install() {
    local force=0 a
    for a in "$@"; do
        case "$a" in
            --force) force=1 ;;
            --*) : ;;
        esac
    done

    _rp_engine_deps_ok git cmake ninja curl cc || return 1

    if rp_eng_llama-server_resolve >/dev/null; then
        if [[ "$force" != "1" ]]; then
            rp_msg ok "llama-server ya instalado: $(rp_eng_llama-server_resolve)"
            return 0
        fi
        rp_msg info "reinstalación forzada (--force)"
    fi

    # ── 1. clone del fork (HTTPS, spec §6) ───────────────────────────────
    if [[ -d "$LLAMA_DIR/.git" && "$force" != "1" ]]; then
        rp_msg info "reutilizo el clon existente: $LLAMA_DIR"
    else
        if [[ -e "$LLAMA_DIR" ]]; then
            rp_msg warn "sustituyo $LLAMA_DIR por un clon limpio del fork"
            rm -rf "$LLAMA_DIR" || return 1
        fi
        rp_step 1 5 "clonando AtomicBot-ai/atomic-llama-cpp-turboquant (HTTPS)..."
        git clone -q https://github.com/AtomicBot-ai/atomic-llama-cpp-turboquant.git \
            "$LLAMA_DIR" || {
            rp_msg error "falló el clone del motor"
            return 1
        }
    fi

    # ── 2. cmake configure: Metal (macOS) o CUDA (Linux+NVIDIA) ──────────
    # Flags portadas de la versión 0.x (historial git).
    local os build_dir="$LLAMA_DIR/build"
    os="$(rp_detect_os)"
    rp_step 2 5 "configurando cmake ($os)..."
    case "$os" in
        macos)
            cmake -B "$build_dir" -S "$LLAMA_DIR" -G Ninja \
                -DCMAKE_BUILD_TYPE=Release \
                -DCMAKE_OSX_ARCHITECTURES=arm64 \
                -DCMAKE_C_COMPILER=clang \
                -DCMAKE_CXX_COMPILER=clang++ \
                -DCMAKE_C_FLAGS='-O3 -mcpu=native' \
                -DCMAKE_CXX_FLAGS='-O3 -mcpu=native' \
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
                -DLLAMA_BUILD_HTML=OFF >/dev/null || {
                rp_msg error "falló el configure de cmake (Metal)"
                return 1
            }
            ;;
        cachyos|arch|linux)
            if ! command -v nvidia-smi >/dev/null 2>&1 || ! nvidia-smi >/dev/null 2>&1; then
                rp_msg error "llama-server en Linux exige CUDA: nvidia-smi no responde"
                rp_msg info "  (en macOS compila con Metal; en Linux sin NVIDIA no hay build)"
                return 1
            fi
            local cuda_path="${ROPERO_CUDA_PATH:-/opt/cuda-13.3}"
            if [[ ! -x "$cuda_path/bin/nvcc" && -z "${ROPERO_CUDA_PATH:-}" ]]; then
                cuda_path="/opt/cuda"
            fi
            if [[ ! -x "$cuda_path/bin/nvcc" ]]; then
                rp_msg error "nvcc no encontrado en $cuda_path (ROPERO_CUDA_PATH para cambiarlo)"
                return 1
            fi
            local host_cxx="${ROPERO_HOST_CXX:-}" host_cc c
            if [[ -z "$host_cxx" ]]; then
                for c in /usr/bin/g++-14 /usr/bin/g++-13 /usr/bin/clang++; do
                    if [[ -x "$c" ]]; then host_cxx="$c"; break; fi
                done
            fi
            if [[ -z "$host_cxx" ]]; then
                rp_msg error "sin host compiler (g++-14/g++-13/clang++)"
                return 1
            fi
            host_cc="$host_cxx"
            case "$host_cxx" in
                */g++-*)     host_cc="${host_cxx/g++/gcc}" ;;
                */clang++)   host_cc="${host_cxx/clang++/clang}" ;;
            esac
            cmake -B "$build_dir" -S "$LLAMA_DIR" -G Ninja \
                -DCMAKE_BUILD_TYPE=Release \
                -DCUDA_TOOLKIT_ROOT_DIR="$cuda_path" \
                -DCMAKE_CUDA_COMPILER="$cuda_path/bin/nvcc" \
                -DCMAKE_CUDA_HOST_COMPILER="$host_cxx" \
                -DCMAKE_CXX_COMPILER="$host_cxx" \
                -DCMAKE_C_COMPILER="$host_cc" \
                -DCMAKE_CXX_FLAGS='-march=native -O3' \
                -DCMAKE_C_FLAGS='-march=native -O3' \
                -DCMAKE_CUDA_FLAGS='--use_fast_math -O3' \
                -DCMAKE_CUDA_ARCHITECTURES=native \
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
                -DLLAMA_BUILD_HTML=OFF >/dev/null || {
                rp_msg error "falló el configure de cmake (CUDA $cuda_path)"
                return 1
            }
            ;;
        *)
            rp_msg error "SO no soportado para compilar llama-server: $os"
            return 1
            ;;
    esac

    # ── 3. parches del motor ─────────────────────────────────────────────
    # El script deriva su PATCHES_DIR de su propia ubicación. Hoy vive en
    # bin/quant/. Se prueban las dos rutas por compatibilidad con clones viejos:
    # depender del orden de las fases.
    local parche=""
    local cand
    for cand in "$ROPERO_ROOT/bin/quant/aplicar-parche-motor.sh" \
                "$ROPERO_ROOT/quant/aplicar-parche-motor.sh"; do
        if [[ -f "$cand" ]]; then
            parche="$cand"
            break
        fi
    done
    rp_step 3 5 "aplicando los parches del motor (NVFP4 + patterns)..."
    if [[ -z "$parche" ]]; then
        rp_msg warn "no encuentro aplicar-parche-motor.sh: compilo SIN los parches turboquant"
    else
        LLAMA_DIR="$LLAMA_DIR" bash "$parche" || {
            rp_msg error "falló la aplicación de parches ($parche)"
            return 1
        }
    fi

    # ── 4. build (no-op si los parches ya compilaron) ────────────────────
    rp_step 4 5 "compilando llama-server..."
    if ! cmake --build "$build_dir" -j"$(_rp_engine_nproc)"; then
        rp_msg error "falló la compilación de llama.cpp"
        return 1
    fi

    # ── 5. verificación y link ───────────────────────────────────────────
    rp_step 5 5 "enlazando $BIN_DIR/llama-server..."
    if [[ ! -x "$build_dir/bin/llama-server" ]]; then
        rp_msg error "la compilación terminó sin $build_dir/bin/llama-server"
        return 1
    fi
    mkdir -p "$BIN_DIR" || return 1
    ln -sf "$build_dir/bin/llama-server" "$BIN_DIR/llama-server" || return 1
    rp_msg ok "llama-server instalado: $BIN_DIR/llama-server"
    return 0
}

rp_eng_llama-server_resolve() {
    # PATH → BIN_DIR → build del clon (spec §5.4).
    local cand ruta
    ruta="$(command -v llama-server 2>/dev/null)"
    [[ -n "$ruta" ]] && { printf '%s\n' "$ruta"; return 0; }
    for cand in "$BIN_DIR/llama-server" "$LLAMA_DIR/build/bin/llama-server"; do
        if [[ -x "$cand" ]]; then
            printf '%s\n' "$cand"
            return 0
        fi
    done
    return 1
}

rp_eng_llama-server_probe() {
    local puerto="${1:-}"
    if ! rp_eng_llama-server_resolve >/dev/null; then
        printf 'missing\n'
        return 1
    fi
    if _rp_engine_http_ok "$puerto"; then
        printf 'ok\n'
        return 0
    fi
    printf 'down\n'
    return 1
}

rp_eng_llama-server_native_auth() {
    printf 'yes\n'
    return 0
}

# shellcheck disable=SC2034        # <modelo> va en la firma (contrato §5.4);
rp_eng_llama-server_launch() {     # llama-server no lo necesita: su identidad
    local modelo="$1" puerto="$2" logfile="$3"   # la pone el argv (--alias/...)
    shift 3

    local bin
    if ! bin="$(rp_eng_llama-server_resolve)"; then
        rp_msg error "llama-server no está instalado (ropero install engines llama-server)"
        return 1
    fi
    if [[ -z "${LLAMA_API_KEY:-}" ]]; then
        rp_msg error "falta LLAMA_API_KEY: el launcher (04) la resuelve antes de llamar al engine"
        return 1
    fi

    # El modelo y sus flags ya vienen en "$@" (rp_model_argv). El engine
    # añade binario + host + puerto + auth y, ANTES de "$@", los defaults
    # del servidor. Los args del TOML van después y por tanto GANAN: un
    # modelo que quiera otros threads los declara en [args].base.
    #
    # Estos defaults son los del launcher del monolito 0.x (start_model):
    # el refactor los perdió y con ellos el --threads 16/16. Sin ellos,
    # llama-server usa su default (-1 = todos los núcleos físicos).
    mkdir -p "$(dirname "$logfile")" 2>/dev/null
    nohup "$bin" \
        --host "${HOST:-127.0.0.1}" \
        --port "$puerto" \
        --api-key "$LLAMA_API_KEY" \
        --threads 16 \
        --threads-batch 16 \
        --flash-attn on \
        --jinja \
        --log-verbosity 4 \
        --parallel 1 \
        --load-mode none \
        --kv-unified \
        --kv-offload \
        --batch-size 4096 \
        --ubatch-size 1024 \
        --cache-reuse 512 \
        --cache-prompt \
        --context-shift \
        --cont-batching \
        --poll 60 \
        --metrics \
        --perf \
        --seed -1 \
        "$@" >>"$logfile" 2>&1 &
    printf '%s\n' "$!"
    return 0
}
