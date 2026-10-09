#!/usr/bin/env bash
# ropero.d/21-download-models.sh
# Descarga los modelos por defecto que ropero debe tener disponibles al instalar.
# Idempotente: si el modelo ya está, skip. Las preguntas de confirmación se
# skipean en modo desatendido (AUTO_YES / SETUP_MODE=full).
#
# Llamada: ropero_download_default_models
# Sin args. Usa $GGUF_DIR, $SAFETENSORS_DIR (de 01-env).
#
# DOS TIPOS DE MODELO, DOS FUNCIONES
#
#   ropero_download_gguf <repo> <fichero> <nombre> <tamaño> [dest_dir] [dest_name]
#       Un FICHERO suelto dentro del repo. Para llama.cpp, que carga GGUF.
#
#   ropero_download_safetensors <repo> <dir> <tamaño> [dest_dir] [excludes]
#       Un CHECKPOINT COMPLETO (pesos + config + tokenizer) a un directorio.
#       Para los engines con servidor propio: mlx_lm, airllm, tensorrt-llm.
#
# La razón de que hagan falta las dos: un GGUF no se puede meter en mlx_lm (que
# quiere el layout de pesos de MLX) ni en airllm (que hace AutoModel.from_pretrained
# y espera safetensors), y un checkpoint de HF tampoco se puede meter en llama.cpp.
# Son formatos incompatibles, no variantes del mismo.
#
# SCOPE BASE v4.1 (2026-09-10):
#   - qwen     (Qwen3.8-27B, broad multi-stack, único 8/8 bloques)
#   - fable    (Fable Coder 35B-A3B, MTP nativo, elliot/coder)
#   - gptoss_* (mismo GGUF, reasoning high/medium/low, adam/edna/randall)
#   - embed    (jina-code, soporte graphify/RAG)
#
# Modelos fuera del scope base movidos a ~/bin/ropero.d/fired/ y
# ~/models/gguf/fired/. Recuperables: ver cabecera de cada script en fired/.

ropero_download_default_models() {
    local os
    os=$(ropero_detect_os 2>/dev/null || echo unknown)

    ropero_info "-- modelos GGUF (llama.cpp) --"
    _ropero_download_gguf_set

    # El set de safetensors es PLATAFORM-DEPENDIENTE y va aparte a propósito:
    # el MLX solo corre en Apple Silicon y un checkpoint de 12 GB que en Cachy no
    # sirve de nada no se descarga. Meterlo en la lista común convertiría
    # `ropero --install` en 12 GB de descarga inútil en Linux, que es justo lo
    # que un install no debe hacer.
    case "$os" in
        macos)  _ropero_download_safetensors_mlx "$os" ;;
        *)      ropero_info "safetensors/MLX: solo macOS (mlx_lm es Apple Silicon). Omitido en $os." ;;
    esac
}

# -----------------------------------------------------------------------------
# GGUF: un solo fichero por modelo. Cross-platform.
# -----------------------------------------------------------------------------
_ropero_download_gguf_set() {
    # Embeddings + chat
    ropero_download_gguf "jinaai/jina-code-embeddings-1.5b-GGUF" "jina-code-embeddings-1.5b-Q8_0.gguf" "jina-code 1.5B Q8" "1.6 GB"

    ropero_download_gguf "bartowski/openai_gpt-oss-20b-GGUF-MXFP4-Experimental" "openai_gpt-oss-20b-MXFP4.gguf" "gpt-oss 20B MXFP4" "12.1 GB"

    ropero_download_gguf "Achilles1089/fable-coder-35B-A3B-GGUF" "fable-coder-35B-A3B-Q4_K_M.gguf" "Fable Coder Q4" "21.7 GB"

    ropero_download_gguf "unsloth/Qwen3.8-27B-GGUF" "Qwen3.8-27B-UD-Q3_K_XL.gguf" "Qwen3.8" "13.1 GB"
    
    if [[ -e "$HOME/models/gguf/MTP/mtp-Qwen3.8-27B-Q4_0.gguf" ]]; then
        ropero_download_gguf "unsloth/Qwen3.8-27B-GGUF" "MTP/mtp-Qwen3.8-27B-Q4_0.gguf" "Qwen 3.8 MTP (Q4_0)" "1.37 GB"
        ropero_download_gguf "unsloth/Qwen3.8-27B-GGUF" "mmproj-BF16.gguf" "Qwen 3.8 MMPROJ(BF16)" "0.9 GB" "$GGUF_DIR" "qwen3.8-mmproj-BF16.gguf"

        cp "$HOME/models/gguf/MTP/mtp-Qwen3.8-27B-Q4_0.gguf" "$HOME/models/gguf/"
    fi

    ropero_download_gguf "unsloth/Qwen2.5-VL-7B-Instruct-GGUF" "Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf" "Qwen2.5-VL 7B" "4.79 GB"
    # OJO: el 3er argumento es el NOMBRE legible del modelo, no su tamaño. Con
    # "1.35 GB" aquí, el size se colaba en `name` y la ruta en `size`, así que
    # dest_dir recibía el nombre del fichero de destino: se creaba una CARPETA
    # "Qwen2.5-VL-mmproj.gguf" con el gguf dentro, y el renombrado no ocurría.
    ropero_download_gguf "unsloth/Qwen2.5-VL-7B-Instruct-GGUF" "mmproj-F16.gguf" "Qwen2.5-VL mmproj" "1.35 GB" "$GGUF_DIR" "Qwen2.5-VL-mmproj.gguf"

    # nuevo setup de modelos para gunter
    ropero_download_gguf "unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF" "Devstral-Small-2-24B-Instruct-2512-UD-Q4_K_XL.gguf" "Devstral Small UD-Q4_K_XL" "14.5 GB"
    ropero_download_gguf "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF" "Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf" "Qwen3 Coder UD-Q4_K_XL" "17.7 GB"
    ropero_download_gguf "bartowski/agentica-org_DeepCoder-14B-Preview-GGUF" "agentica-org_DeepCoder-14B-Preview-Q4_K_L.gguf" "DeepCoder Q4_K_L" "9.57 GB"


    ropero_download_safetensors \
        "unsloth/Qwen3.8-27B-NVFP4" \
        "Qwen3.8-27B-NVFP4" \
        "23.4 GB"        
}

# -----------------------------------------------------------------------------
# Safetensors para engines con servidor propio.
#
# macOS → mlx_lm. El destino tiene que llamarse EXACTAMENTE como MODEL_PATH en
# ropero.d/<modelo>.sh, porque el preflight de ropero comprueba ese directorio y
# dice "no encontrado" si no existe. Si cambias uno, cambias el otro.
# -----------------------------------------------------------------------------
_ropero_download_safetensors_mlx() {
    local os="${1:-}"
    [[ "$os" == "macos" ]] || return 0

    ropero_info "-- checkpoints safetensors (mlx_lm) --"

    # gpt-oss-20b en MXFP4 → ropero.d/metaloss.sh (MODEL_PATH=gpt-oss-20b-MXFP4-Q8)
    #
    # Por qué ESTE repo y no otro, medido sobre los config.json reales:
    #   · Los expertos MoE son ~19B de los 21B parámetros y van a MXFP4-4 en
    #     todos los candidatos. Es el formato en el que OpenAI post-entrenó el
    #     modelo y con el que hizo TODAS sus evaluaciones.
    #   · "Q8" = los otros 122 tensores (q/k/v/o, router, embeddings, lm_head) a
    #     8 bits en vez de 4. Cuesta 0.8 GB y es donde se nota la cuantización.
    #   · El "todo a 8 bits" de lmstudio-community (20.7 GB) sube a 8 bits
    #     ~2B parámetros dejando los 19B igual que aquí: 9.5 GB por nada.
    # El directorio DEBE llamarse gpt-oss-20b-MXFP4-Q8.
    ropero_download_safetensors \
        "mlx-community/gpt-oss-20b-MXFP4-Q8" \
        "gpt-oss-20b-MXFP4-Q8" \
        "12.1 GB"


    # ── Lo que NO se descarga aquí, y por qué ───────────────────────────────
    # ropero.d/airgptoss.sh (engine airllm) necesita ~/models/safetensors/
    # gpt-oss-120b, pero son ~130-240 GB según la variante: no es una descarga
    # que un `--install` pueda hacer por sorpresa. Se deja a mano, y el .sh
    # avisa si no está:
    #
    #   ropero_download_safetensors "openai/gpt-oss-120b" "gpt-oss-120b" "130 GB" \
    #       "$SAFETENSORS_DIR/gpt-oss-120b" "original/*"
    #
    # El 5º argumento (excludes) importa: `openai/gpt-oss-120b` trae `original/`
    # con los pesos bf16 SIN cuantizar junto a la versión cuantizada. Sin
    # excluirlo descargas los dos juegos: el doble de disco para nada. Es
    # exactamente el caso que motivó el parámetro.
    #
    # El GGUF de gpt-oss-120b (unsloth/gpt-oss-120b-GGUF) sí es razonable
    # (~63 GB MXFP4 / ~78 GB Q4) si la máquina tiene RAM para ello, pero tampoco
    # cabe en el scope de un install automático.
}