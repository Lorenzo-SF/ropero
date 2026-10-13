#!/usr/bin/env bash
# =============================================================================
# Modelo: gptossfallback
#
# El seguro. No trabaja a diario: arranca cuando la nube cae a media sesión y
# hay que terminar el trabajo.
#
# POR QUÉ "FRÍO"
# ──────────────
# Frío = descargado y configurado, pero el proceso NO está corriendo. Ocupa
# 0 MB de RAM, solo disco. Arranca en ~15s cuando lo pides.
#
# Es la única forma de que un seguro sea barato: 13,2 GB de disco que no
# ocupan RAM hasta el día que lo necesitas. El resto de la lane va caliente
# porque se usa todos los días.
#
# ── RELACIÓN CON LOS OTROS TRES gpt-oss ──────────────────────────────────────
# NO sustituye a _gptoss_common.sh ni a gptoss_high/medium/low. Sigue habiendo
# cuatro modelos gpt-oss, y esta es la estructura:
#
#   _gptoss_common.sh ──┬─► gptoss_high.sh    alias verifier   ┐
#   (GGUF MXFP4)        ├─► gptoss_medium.sh  alias designer   ├ mismo GGUF,
#                       └─► gptoss_low.sh     (sin alias)     ┘  3 profundidades
#
#   gptossfallback.sh ─────► alias fallback    GGUF UD-Q8_K_XL  ← este, APARTE
#                            puerto :9996                      frío, de guardia
#
# Por qué no comparte GGUF con los otros tres:
#   1. Calidad. MXFP4 son 4 bits y Q8 son 8: el doble de error de
#      cuantización. En un modelo de razonamiento los errores SE ACUMULAN a lo
#      largo de la cadena de pensamiento. Para el rol cuyo trabajo ES razonar
#      bien, pagar 4 bits es hacerlo mal.
#   2. Los 1,1 GB no son el problema: este modelo es frío, y 1,1 GB de disco
#      son unos segundos más de carga en un escenario donde ya aceptas ir
#      despacio.
#   3. Si compartiera fichero, el fallback estaría en disco igual por los
#      modelos del día a día — y entonces no es un seguro, es el mismo activo
#      de siempre.
#
# Por qué SÍ comparte la tabla de n-gramas con ellos:
#   Es el mismo modelo con distinta profundidad de razonamiento, así que la
#   calibración por nivel tiene que ser la MISMA. Por eso sourcea el common y
#   usa su `gptoss_spec_args` en vez de llevar una copia. Una tabla copiada no
#   se actualiza: se ajustaba `high` en el common, el fallback seguía con los
#   valores viejos, y dos modelos de la misma familia se comportaban distinto
#   sin que nada lo dijera.
#
# ── POR QUÉ OTRO MODELO Y NO EL MISMO QUE EL ENRIQUECEDOR ────────────────────
# El rol "razonar" ya no es local: es Opus en el trabajo y MiniMax en lo
# personal. Así que gpt-oss no compite con nadie por ese puesto — está de
# guardia.
#
# Y ser un modelo DISTINTO del trabajador diario es intencionado: si uno tiene
# un punto ciego, el otro lo cubre. Dos modelos de la misma familia serían dos
# veces el mismo punto ciego. La diversidad entre roles no es un coste.
# =============================================================================

source "$(dirname "${BASH_SOURCE[0]}")/_gptoss_common.sh"

# El common pone openai_gpt-oss-20b-MXFP4.gguf en MODEL_GGUF. Aquí se cambia a
# Q8, y solo aquí.
MODEL_GGUF="gpt-oss-20b-UD-Q8_K_XL.gguf"
MODEL_CTX="${ROPERO_FALLBACK_CTX:-131072}"
MODEL_PORT=9996

# Alias público. `ropero fallback` en vez de `ropero gptossfallback`.
MODEL_ALIAS="${MODEL_ALIAS:-fallback}"

# reasoning_effort: medium por defecto. Es un seguro, no el motor del día a día:
# `high` es 3-4x más lento y solo tiene sentido si lo que hay que rehacer es
# una decisión difícil. Se cambia con ROPERO_FALLBACK_EFFORT=high.
# El nivel lo valida gptoss_spec_args (el del common), no una copia de él aquí.
ROPERO_FALLBACK_EFFORT="${ROPERO_FALLBACK_EFFORT:-medium}"

get_model_args_gptossfallback() {
    local -a args=(
        # ---- FORZADO A CPU ----
        # Es un seguro para cuando la nube falla. Si el coder está trabajando
        # (que es justo cuando cae la nube), la GPU es suya. Y si este modelo
        # tomara la VRAM, se llevaría por delante lo que estabas haciendo.
        # El common pone 99; aquí 0. Es lo único de los args que no comparte.
        --n-gpu-layers 0

        # ---- KV cache ----
        # q8_0 y no el default f16: a 131072 de contexto, el KV cache en f16
        # son ~15 GB de RAM. q8_0 lo deja en ~7, y la diferencia de calidad en
        # el KV cache es inapreciable al generar.
        --cache-type-k q8_0
        --cache-type-v q8_0
        --cache-prompt
        --kv-unified

        # ---- Plantilla ----
        --jinja
        --reasoning-format auto
        --chat-template-kwargs "{\"reasoning_effort\":\"$ROPERO_FALLBACK_EFFORT\"}"

        # ---- Sampling (los de gpt-oss) ----
        --temp 0.8
        --top-k 40
        --min-p 0.05
        --repeat-penalty 1.1

        # ---- Batching / threads ----
        --batch-size 4096
        --ubatch-size 1024
        --parallel 1
        --threads 12
        --threads-batch 24

        # ---- Generación ----
        --n-predict 16384
        --keep 1024

        --load-mode auto
    )

    # Misma tabla de n-gramas que verifier/designer/gptoss_low, y por el mismo
    # motivo. El `|| return 1` va sobre una command substitution a propósito:
    # ahí sí se propaga el exit code. Con mapfile sobre process substitution se
    # perdería, porque corre en un subshell — el nivel inválido pasaría y el
    # args saldría sin especificación, sin decir nada.
    local spec_out
    spec_out="$(gptoss_spec_args "$ROPERO_FALLBACK_EFFORT")" || return 1
    local -a spec_args
    mapfile -t spec_args <<<"$spec_out"
    args+=("${spec_args[@]}")

    printf '%s\n' "${args[@]}"
}

# ==============================================================================
# TROUBLESHOOTING
# 1. "Tarda 15 segundos en arrancar"
#    → Son 13,2 GB de disco a RAM. Es el precio de que un seguro no ocupe RAM.
# 2. "Se queda sin RAM si el vault está procesando"
#    → Fallback 13,2 + vault 9,6 = 22,8 GB de 32. Cabe, pero no dejes que
#      claude code y un navegador con 20 pestañas también estén cargados.
# 3. "El modelo escribe fatal en español"
#    → Es un modelo orientado a inglés. Para el vault no es el modelo (para eso
#      está ministral). Para seguir una sesión en inglés, va bien.
# 4. "Arranca con el nivel de razonamiento por defecto en vez del mío"
#    → effort inválido: gptoss_spec_args devuelve 1 y ropero no lo arranca. En
#      el log tiene que estar el ERROR con el valor que le pasaste.
# ==============================================================================
