#!/usr/bin/env bash
# ==============================================================================
# Fable-Coder-35B-A3B — Config optimizada para RTX 5080 (16GB VRAM, 32GB RAM)
# ==============================================================================
# Modelo: Achilles1089/fable-coder-35B-A3B
#   - MoE 35B totales / 3B activos por token
#   - Fine-tune Opus-4.7 reasoning + Opus-4.8 tool use sobre Qwen3.6-35B-A3B
#   - MBPP 78.2% (+5.2 sobre base), SWE-Bench 73.4%, Terminal-Bench 51.5%
#   - Hybrid: 48 layers (3 Gated DeltaNet + 1 Gated Attention, repetido)
#   - MTP heads integrados (Qwen3.6 los trae)
# ==============================================================================
# Resultado esperado en RTX 5080:
#   - Prompt eval: 250-400 t/s
#   - Gen 8k ctx:  35-50 t/s
#   - Gen 32k ctx: 18-25 t/s
#   - Gen 131k ctx: 8-12 t/s (KV cache limita)
# ==============================================================================

MODEL_ENGINE="llama-server"
FABLE_GGUF="fable-coder-35B-A3B-Q4_K_M.gguf"   # ~21 GB en disco
MODEL_GGUF="$FABLE_GGUF"
MODEL_CTX="${ROPERO_FABLE_CTX:-131072}"          # 32k es el sweet spot para 16GB VRAM

# ==============================================================================
# Tuning por VRAM disponible — AJUSTAR TRAS VER nvidia-smi
# ==============================================================================
# -cmoe 999 = TODOS los expertos a CPU (max VRAM libre para KV, gen más lento)
# -cmoe  48 = mitad y mitad (balance)
# -cmoe  20 = solo 20 layers con expertos en CPU (tu valor original, KB justo)
#
# Con 16GB VRAM lo óptimo suele ser 35-48: attention+embeddings en GPU,
# expertos en CPU, gen un poco más lento pero context largo usable.

N_CPU_MOE="${ROPERO_FABLE_N_CPU_MOE:-30}"       # 30 = ~30 layers con expertos MoE en CPU (16GB VRAM estricto)
                                                   # bajar a 20 si sobra VRAM (más velocidad); subir a 40 si OOM

get_model_args_fable() {
  local -a args=(
    # ---- GPU offload ----
    --n-gpu-layers 999                            # todos los layers "GPU-eligible"
    --n-cpu-moe "$N_CPU_MOE"                      # expertos MoE en CPU (ajustable)
    --no-kv-offload                               # KV cache en GPU (default on)

    # ---- Atención ----
    # -fa on se pone globalmente en ropero entry; no duplicar
    # --swa-full NO aplicar — Fable no tiene SWA (atención full en gated attn layers)

    # ---- KV cache ----
    --cache-type-k q4_0                           # q4_0 en K para liberar VRAM (V se queda q8_0)
    --cache-type-v q8_0                           # nunca q4_0 en V, degrada tool use
    --cache-prompt
    --cache-reuse 512                             # reusa bloques KV en prompts similares
    --kv-unified                                  # single user, unified buffer
    --context-shift                               # rolling context en sesiones largas

    # ---- Speculative decoding (MTP del propio modelo) ----
    --spec-type draft-mtp                         # SI falla al cargar, ver "TROUBLESHOOTING" abajo
    --spec-draft-n-max 5                          # hasta 5 tokens drfteados por ronda
    --spec-draft-n-min 1                          # no malgastes en rounds de 0 drafts
    --spec-draft-backend-sampling                 # offload sampling a backend

    # ---- Batching / threads ----
    --batch-size 4096
    --ubatch-size 1024
    --parallel 1
    --threads 12                                  # 32GB RAM, CPU moderna
    --threads-batch 16
    --poll 30                                     # polling medio (default 50)

    # ---- Modelo / carga ----
    --jinja                                       # chat template Qwen3.6
    --chat-template-kwargs '{"enable_thinking": false}'   # coder = directo, sin thinking
    --reasoning-format deepseek                   # limpia tags thinking del output

    # ---- Sampling (canónico Qwen3.6 coder) ----
    --temp 0.6
    --top-k 20
    --top-p 0.95
    --min-p 0.05
    --repeat-penalty 1.0                          # sin penalty (Qwen3.6 OK)
    --repeat-last-n 64
    --seed -1

    # ---- Generación ----
    --n-predict 4096                              # tope por response, ajustable
    --keep 1024                                   # preserva system+tools al hacer context-shift

    # ---- Misc ----
    --load-mode none                              # mmap off (con mlock o manual swap)
    # --log-verbosity lo pone ropero entry (default 4); no duplicar
    --metrics                                     # /metrics endpoint
    --perf                                        # libllama perf timings
  )
  printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# ==============================================================================
# 1. Si "--spec-type draft-mtp" falla al arrancar (MTP no detectado):
#      → Quítalo y usa solo el modelo. Fable sin spec sigue rindiendo bien.
#
# 2. Si "--n-cpu-moe 40" deja el modelo demasiado lento:
#      → Baja a 30, mira nvidia-smi, repite. Cada 5 layers en CPU ≈ 1.5 GB VRAM libres.
#
# 3. Si quieres más velocidad a costa de calidad:
#      → Sube --spec-draft-n-max a 6-7
#      → O baja --cache-type-v a q4_0 (ojo: tool use puede romperse)
#
# 4. Si quieres más calidad a costa de velocidad:
#      → --temp 0.5, --top-p 0.9
#      → O cambia a Fable Q5_K_M (~26 GB) si te cabe en RAM
#
# 5. Para tool use agentic (OpenHands, aider, etc):
#      → --n-predict 8192 (algunos tool calls son largos)
#      → --cache-reuse 1024 (más agresivo en reuso de cache)
# ==============================================================================
