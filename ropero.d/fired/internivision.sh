#!/usr/bin/env bash
# ==============================================================================
# 🔥 FIRED — modelo fuera del scope base (qwen / fable / gptoss_*).
# Movido a bin/ropero.d/fired/ tras decisión del usuario (2026-09-10).
# Recuperable: mv fired/internivision.sh ./ y descargar GGUF + mmproj.
# (NO estaba en 21-download-models.sh actual — es legacy VLM).
# Repo original: bartowski/OpenGVLab_InternVL3_5-30B-A3B-GGUF
# GGUF: OpenGVLab_InternVL3_5-30B-A3B-Q4_K_M.gguf (18.6 GB)
# mmproj: mmproj-internivision-f16.gguf (637 MB)
# ==============================================================================

# =============================================================================
# Modelo: internivision — InternVL3.5-30B-A3B (Q4_K_M, ~18.6 GB, MoE)
# =============================================================================
# VLM "el grande con offload" para ttstt: 30B parámetros totales pero solo ~3B
# activos por inferencia (MoE). Pretende ser la mejor opción local para
# análisis multimodal exigente (espectrogramas, multi-imagen para selección de
# segmentos) en hardware con 16 GB de VRAM.
#
# Repo: bartowski/OpenGVLab_InternVL3_5-30B-A3B-GGUF
# GGUF: OpenGVLab_InternVL3_5-30B-A3B-Q4_K_M.gguf (18.6 GB)
# mmproj: mmproj-internivision-f16.gguf (636 MB, renombrado en 21-download-models.sh)
#
# Por qué este y NO InternVL3-38B denso (unsloth/InternVL3-38B-GGUF):
#   El 38B denso trae mmproj de 11.3 GB (InternViT-6B) → total 31 GB, no cabe
#   en 16 GB ni con offload agresivo. El 30B-A3B MoE trae mmproj de 637 MB
#   (InternViT-300M) → total ~19 GB, cabe con --n-gpu-layers parcial.
#
# Por qué MoE > denso para nosotros:
#   Solo activa ~3B por inferencia → velocidad comparable a un denso de 3-7B,
#   capacidad comparable a 30B. Sweet spot para VRAM limitada.
#
# Por qué InternVL3.5 > InternVL3:
#   Lanzado después (2025-09 vs 2025-04). Mejoras incrementales en benchmarks
#   multimodales. Mismo entrenamiento nativo multimodal pre-training.
#
# Overrides por env (sin editar este archivo):
#   ROPERO_INTERNIVISION_CTX=16384       (ctx, default 16384 — vision no necesita más)
#   ROPERO_INTERNIVISION_NGL=-1          (n-gpu-layers, default -1 = auto-fit a VRAM libre; con 16GB cabe entero)
#   ROPERO_INTERNIVISION_TEMP=0.2
#   ROPERO_INTERNIVISION_TOP_P=0.9
#   ROPERO_INTERNIVISION_TOP_K=20
#   ROPERO_INTERNIVISION_REPEAT=1.05
#
# POR QUÉ ngl=-1 (no 999): con 999 el loader FUERZA cargar TODAS las capas
# en GPU, y como el modelo (~19 GB) + KV cache + mmproj no caben en 16 GB,
# falla con CUDA OOM. Con -1 el loader ajusta dinámicamente al máximo que
# quepa en VRAM. En 16 GB el modelo cabe entero (offload cero). Verificado
# sep-2026 con NGL=-1 en RTX 5080 Laptop.
# =============================================================================
MODEL_ENGINE="llama-server"
INTERNIVISION_GGUF="OpenGVLab_InternVL3_5-30B-A3B-Q4_K_M.gguf"
INTERNIVISION_MMPROJ="mmproj-internivision-f16.gguf"  # renombrado en 21-download-models.sh

MODEL_GGUF="$INTERNIVISION_GGUF"
MODEL_CTX="${ROPERO_INTERNIVISION_CTX:-16384}"

get_model_args_internivision() {
    local ngl="${ROPERO_INTERNIVISION_NGL:--1}"
    local temp="${ROPERO_INTERNIVISION_TEMP:-0.2}"  # Baja temperatura para análisis preciso
    local top_p="${ROPERO_INTERNIVISION_TOP_P:-0.9}"
    local top_k="${ROPERO_INTERNIVISION_TOP_K:-20}"
    local repeat="${ROPERO_INTERNIVISION_REPEAT:-1.05}"

    local -a args=(
        --n-gpu-layers "$ngl"
        --threads 12
        --threads-batch 24
        --temp "$temp"
        --top-p "$top_p"
        --top-k "$top_k"
        --repeat-penalty "$repeat"
        --cache-type-k q8_0
        --cache-type-v q8_0
        --mmproj "$GGUF_DIR/$INTERNIVISION_MMPROJ"
        # n-predict bajo: ttstt solo genera JSON estructurados + razonamiento
        # (CoT). 2048 cubre con margen (~5 frases + JSON). Antes 4096
        # desperdiciaba tokens en预留 KV cache.
        --n-predict 2048
        --slot-prompt-similarity 0.8
        --keep 4096
        --jinja
        --load-mode none
    )

    printf '%s\n' "${args[@]}"
}
