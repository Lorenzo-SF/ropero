#!/usr/bin/env bash
# ==============================================================================
# 🔥 FIRED — modelo fuera del scope base (qwen / fable / gptoss_*).
# Movido a bin/ropero.d/fired/ tras decisión del usuario (2026-09-10).
# Recuperable: mv fired/muse.sh ./ && descargar GGUF (ver bloque abajo).
# ==============================================================================
# --- BLOQUE DE DESCARGA (antes en 21-download-models.sh, ahora comentado) ---
# ropero_download_gguf "unsloth/Muse-Glimmer-30B-GGUF" \
#     "Muse-Glimmer-30B-UD-Q4_K_XL.gguf" "Muse Glimer 30B" "15.9 GB"

# ==============================================================================
# Muse-Glimmer-30B — Config calibrada (RTX 5080 16GB VRAM, 30GB RAM)
# ==============================================================================
# Modelo: meta-models/Muse-Glimmer-30B (cuantizado por unsloth, UD-Q4_K_XL)
#   - DENSE 30B (29.6B con vision encoder 1.8B)
#   - Hybrid: 16 layers local attention (sliding window 2048) + 1 global
#   - Vocab 202,048 (200k BPE + 2048 special)
#   - RoPE θ=500,000
#   - Sampling canónico (repo Meta): temp=1.0, top_p=0.95, top_k=64
#   - Reasoning strength: low/medium/high/xhigh (system prompt)
#   - Speculative decoding: DFlash drafter (16-token blocks)
# ==============================================================================
# Peculiaridades críticas:
#   1. DENSE 30B con Q4_K_XL (15.9GB) — NO cabe entero en 16GB VRAM
#      → usar --n-gpu-layers bajo (20-25) y dejar el resto en CPU
#   2. SWA pattern [L,L,L,G] repetido → DEBE llevar --swa-full
#      (sin esto, attention computation usa ventana completa ineficiente)
#   3. --reasoning-format deepseek limpia tags <think> del output
#   4. RoPE θ=500k se infiere del GGUF metadata; --rope-freq-base fuerza valor
# ==============================================================================

MODEL_ENGINE="llama-server"
MUSE_GGUF="Muse-Glimmer-30B-UD-Q4_K_XL.gguf"

MODEL_GGUF="$MUSE_GGUF"
MODEL_CTX="${ROPERO_MUSE_CTX:-131072}"

get_model_args_muse() {
    local -a args=(
        # ---- GPU offload (modelo denso 30B, 16GB VRAM justos) ----
        --n-gpu-layers 22                               # ~22 de 52 layers en GPU; resto CPU
        --no-kv-offload                                # KV cache en GPU (con 22 layers hay espacio)

        # ---- Atención ----
        # -fa on se pone globalmente en ropero entry; no duplicar
        --swa-full                                     # CRÍTICO: activa sliding window pattern [L,L,L,G]

        # ---- Positional encoding ----
        --rope-freq-base 500000                        # θ=500k del repo

        # ---- KV cache ----
        --cache-type-k q4_0                            # agresivo (SWA usa ventanas pequeñas)
        --cache-type-v q4_0
        --cache-prompt
        --context-shift
        --kv-unified                                   # single user, unified buffer

        # ---- Speculative decoding (DFlash drafter del repo) ----
        --spec-type draft-dflash
        --spec-draft-n-max 16                          # hasta 16 tokens drfteados por bloque

        # ---- Modelo / carga ----
        --jinja                                        # chat template Muse (jinja)
        --reasoning-format deepseek                    # limpia tags <think> del output
        # Reasoning_strength se pasa en el SYSTEM PROMPT del request,
        # NO como chat-template-kwargs (Muse usa system prompt nativo).
        --load-mode none                               # mmap off

        # ---- Sampling (canónico Meta Muse) ----
        --temp 1.0
        --top-p 0.95
        --top-k 64
        --min-p 0.00
        --repeat-penalty 1.0                           # sin penalty (canónico)
        --repeat-last-n 64

        # ---- Batching / threads ----
        --batch-size 4096
        --ubatch-size 1024
        --parallel 1

        # ---- Generación ----
        --n-predict 4096
        --keep 1024

        # ---- Misc ----
        # --log-verbosity lo pone ropero entry (default 4); no duplicar
    )
    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# ==============================================================================
# 1. Si "--spec-type dflash" falla al cargar (DFlash head no detectado):
#      → Quita el flag. Sin spec sigue rindiendo, solo más lento.
#
# 2. Si OOM en GPU con --n-gpu-layers 22:
#      → Baja a 18. Cada 2 layers ≈ 1 GB VRAM adicional.
#
# 3. Si quieres más calidad:
#      → Cambia a Q6_K_XL (~26 GB en disco, mismo offload)
#
# 4. Si el modelo entra en bucles (es razonador, puede pasar):
#      → Añade --repeat-penalty 1.1 --repeat-last-n 128
# ==============================================================================