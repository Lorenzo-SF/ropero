#!/usr/bin/env bash
# ==============================================================================
# Qwen3.8-27B Q3_K_XL + MTP draft — CONFIG OPTIMIZADA
# ==============================================================================
# Config validada en test extremo 2026-09-11:
#   - KV q8_0 (mejor precision, +2.3 GB vs q4_0, pero vale la pena)
#   - MTP draft externo con --spec-type draft-mtp + --model-draft
#   - Acceptance rate ~70% en mini-test (1020/1443 drafts)
#   - Mean accepted len 2.84 tokens (suficiente)
#
# Por que esta config es optima:
#   - Q3_K_XL (12.23 GiB) cabe en 16 GB con margen para KV cache q8_0
#   - MTP aporta calidad sin perder velocidad neta (overhead compensado por acceptance)
#   - Context 131072 default = maximo rendimiento con 128k prompt reales
#   - ngl 99 = todos los layers en GPU; el fitting auto-ajusta el draft (-1)
#   - n-predict 16384 = no corta respuestas largas
#
# Peculiaridades:
#   1. Spec-type draft-mtp: MTP head nativo (qwen35.nextn_predict_layers = 1)
#   2. Model-draft: archivo mtp-Qwen3.8-27B-Q4_0.gguf (1.27 GiB, 3.66 BPW)
#   3. KV q8_0: recomendado repo para tool use / precision en contexto largo
#   4. Sin thinking mode (chat_template_kwargs enable_thinking=false) = instruct puro
#
# Override env vars (sin editar este archivo):
#   ROPERO_QWEN_CTX=262144 ropero qwen  # 256k context (limita velocidad a 128k+)
#   ROPERO_QWEN_TEMP=0.5 ropero qwen     # mas determinista
# ==============================================================================

MODEL_ENGINE="llama-server"
MODEL_GGUF="Qwen3.8-27B-UD-Q3_K_XL.gguf"
MODEL_DRAFT="$HOME/models/gguf/mtp-Qwen3.8-27B-Q4_0.gguf"

MODEL_CTX="${ROPERO_QWEN_CTX:-131072}"
# Alias público (lo carga bin/ropero como nombre alternativo del modelo).
# Para cambiar el modelo físico asociado al alias `analyst`, edita este script;
# `opencode.jsonc` no se entera.
MODEL_ALIAS="${MODEL_ALIAS:-analyst}"

get_model_args_qwen() {
    local -a args=(
        # ---- GPU offload ----
        # ngl 99: todos los layers del target en GPU (64 layers + 1 MTP head = 65)
        # -1 NO funciona aqui: el fitting se cuelga con ctx 131k + draft
        --n-gpu-layers 99
        --no-kv-offload

        # ---- Atencion ----
        # -fa on lo pone ropero entry globalmente

        # ---- KV cache (q8_0 = precision sobre velocidad en contexto largo) ----
        --cache-type-k q8_0
        --cache-type-v q8_0
        --cache-prompt
        --context-shift
        --kv-unified                                   # single user, unified buffer

        # ---- Speculative decoding (MTP nativo + draft externo) ----
        # IMPORTANTE: ruta ABSOLUTA en model-draft. ~ no expande en comillas dobles.
        --spec-type draft-mtp
        --model-draft "$MODEL_DRAFT"
        --n-gpu-layers-draft -1                        # auto-fit del draft
        --spec-draft-n-max 5
        --spec-draft-n-min 1
        --spec-draft-backend-sampling

        # ---- Modelo / carga ----
        --jinja                                        # chat template Qwen3.8
        --reasoning-format deepseek                    # limpia tags <think>
        --chat-template-kwargs '{"enable_thinking": false}'  # THINKING OFF
        --load-mode none

        # ---- Sampling (modo INSTRUCT, canonico Qwen3.8) ----
        --temp 0.7
        --top-p 0.80
        --top-k 20
        --min-p 0.0
        --presence-penalty 1.5                         # reduce repeticiones
        --repeat-penalty 1.0
        --repeat-last-n 64

        # ---- Batching / threads ----
        --batch-size 4096
        --ubatch-size 1024
        --parallel 1

        # ---- Generacion ----
        # Subido a 16384: permite respuestas largas sin cortar (megaextremo ~10k tokens)
        --n-predict 16384
        --keep 1024

        # ---- Misc ----
        # --log-verbosity lo pone ropero entry (default 4)
    )
    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# ==============================================================================
# 1. Si "fitting" del extra model cuelga:
#      - ROPERO_QWEN_CTX=65536 ropero qwen  (bajar contexto)
#      - Cambiar --n-gpu-layers a 60 (deja 4 layers en CPU)
#
# 2. Si OOM:
#      - Bajar KV a q4_0 (menos precision, +5% velocidad)
#      - ROPERO_QWEN_CTX=65536 ropero qwen
#
# 3. Si MTP acceptance rate <30%:
#      - Verificar log "draft acceptance rate"
#      - Probar --spec-draft-n-max 8 (mas especulacion por ronda)
#      - O desactivar MTP: quitar --spec-type y --model-draft
#
# 4. Respuestas cortan a 4096 tokens:
#      - YA RESUELTO: --n-predict 16384 (default)
#
# 5. Si quieres 256k context:
#      - ROPERO_QWEN_CTX=262144 ropero qwen
#      - Requiere ngl 56 (no 99), ver auto-fit
# ==============================================================================
