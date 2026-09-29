# ropero.d/21-download-models.sh
# Descarga los modelos GGUF por defecto que ropero debe tener disponibles
# al instalar. Idempotente: si el modelo ya está, skip. Las preguntas de
# confirmación se skipean en modo desatendido (AUTO_YES / SETUP_MODE=full).
#
# Llamada: ropero_download_default_models
# Sin args. Usa $GGUF_DIR, $SAFETENSORS_DIR (de 01-env).
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
    # Embeddings + chat
    ropero_download_gguf "jinaai/jina-code-embeddings-1.5b-GGUF" "jina-code-embeddings-1.5b-Q8_0.gguf" "jina-code 1.5B Q8" "1.6 GB"
    
    ropero_download_gguf "bartowski/openai_gpt-oss-20b-GGUF-MXFP4-Experimental" "openai_gpt-oss-20b-MXFP4.gguf" "gpt-oss 20B MXFP4" "12.1 GB"
    
    ropero_download_gguf "Achilles1089/fable-coder-35B-A3B-GGUF" "fable-coder-35B-A3B-Q4_K_M.gguf" "Fable Coder Q4" "21.7 GB"

    ropero_download_gguf "unsloth/Qwen3.8-27B-GGUF" "Qwen3.8-27B-UD-Q3_K_XL.gguf" "Qwen3.8" "13.1 GB"
    ropero_download_gguf "unsloth/Qwen3.8-27B-GGUF" "MTP/mtp-Qwen3.8-27B-Q4_0.gguf" "Qwen 3.8 MTP (Q4_0)" "1.37 GB" 


    cp "$HOME/models/gguf/MTP/mtp-Qwen3.8-27B-Q4_0.gguf" "$HOME/models/gguf/"

    ropero_download_gguf "unsloth/Qwen3.8-27B-GGUF" "mmproj-BF16.gguf" "Qwen 3.8 MMPROJ(BF16)" "0.9 GB" "$HOME/models/gguf" "qwen3.8-mmproj-BF16.gguf"

    # nuevo setup de modelos para gunter
    ropero_download_gguf "unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF" "Devstral-Small-2-24B-Instruct-2512-UD-Q4_K_XL.gguf" "Devstral Small UD-Q4_K_XL" "14.5 GB"
    ropero_download_gguf "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF" "Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf" "Qwen3 Coder UD-Q4_K_XL" "17.7 GB"
    ropero_download_gguf "bartowski/agentica-org_DeepCoder-14B-Preview-GGUF" "agentica-org_DeepCoder-14B-Preview-Q4_K_L.gguf" "DeepCoder Q4_K_L" "9.57 GB"

}

