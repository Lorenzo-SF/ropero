#!/usr/bin/env bash
# =============================================================================
# ropero.d/_engines.sh — REGISTRO ÚNICO de engines que no son llama-server
# =============================================================================
# Seguro de sourcear (solo definiciones). El entrypoint lo sourcea junto a
# 00-utils/01-env. El prefijo `_` lo excluye del loop de modelos.
#
# ── POR QUÉ ESTE FICHERO ──────────────────────────────────────────────────────
# Antes, la rama `airllm|tensorrt-llm|mlx_lm` de start_model tenía el comando
# de cada engine INLINE, escrito a ojo. Consecuencias medidas:
#
#   · airllm:  `python -m airllm serve` — airllm 2.11.0 NO tiene `__main__.py`
#              ni ningún subcomando `serve`. Es una LIBRERÍA pura. Ese comando
#              no ha existido nunca: salía con "No module named airllm.__main__".
#   · trtllm:  `--api-key` — trtllm-serve NO tiene ese flag (verificado contra
#              tensorrt_llm/commands/serve.py y la doc de latest/1.3). El
#              servidor habría abortado con "no such option".
#   · mlx_lm:  `mlx_lm.server` OK, pero `ropero_link_mlx` nunca lo enlazó, así
#              que solo se alcanzaba por el shim de asdf — justo lo que el
#              propio entrypoint dice (ver ropero_link_llamacpp) que no debe pasar.
#
# Aquí vive, en un sitio, para cada engine: cómo se RESUELVE el binario, qué
# flags ACEPTA de verdad, si valida la API key por sí mismo, y qué variables
# de entorno necesita. El launcher y el --check leen de aquí; no hardcodean.
#
# ── CONTRATO QUE EXPORTA (el que consume start_model y cmd_check) ─────────────
#   ropero_engine_is_known <engine>          0 si el launcher lo soporta
#   ropero_engine_auth_native <engine>      0 si el engine valida la key
#   ropero_engine_pinned_model <engine>     nombre de modelo que el engine exige
#                                           ("" si enruta por el que le pasan)
#   ropero_engine_resolve <engine>          ruta al ejecutable ("" si no está)
#   ropero_engine_probe <engine>            "ok"|"missing"|"broken" + detalle
#   ropero_engine_env <engine>              líneas "K=V" para el sub-proceso
#   ropero_engine_cmd <engine> ...          argv, un argumento por línea
#   ropero_engine_internal_port <engine> <port>
#                                           puerto libre para el engine
#   ropero_python_bin                       intérprete con el engine instalado
#   ropero_has_amd_npu                      0 si hay NPU AMD (XDNA) usable
#   ropero_engine_label <engine>            nombre para la columna ENGINE
# =============================================================================

# -----------------------------------------------------------------------------
# Identificadores de engine. El token `llama-server` NO aparece aquí: su flujo
# (flags, --cpu, --n-gpu-layers, offload) es radicalmente distinto y ya vive
# inline en start_model. Meterlo aquí sería forzar a un único molde engines que
# no se parecen en nada.
#
# Sin `readonly` a propósito: el fichero se sourcea más de una vez en algunos
# flujos (tests, --install tras un reload) y un `readonly` reasignado aborta con
# set -e por un motivo que no le explica a nadie.
# -----------------------------------------------------------------------------
ROPERO_ENGINES="airllm mlx_lm tensorrt-llm fastflowlm"

ropero_engine_canonical() {
    case "${1:-}" in
        llama|llamacpp|llama_cpp|llama-cpp|llama-server) printf 'llama-server' ;;
        airllm)                                           printf 'airllm' ;;
        mlx|mlx_lm|mlx-lm)                                printf 'mlx_lm' ;;
        tensorrt|trtllm|tensorrt-llm|tensorrt_llm)        printf 'tensorrt-llm' ;;
        flm|fastflowlm|fastflow_lm)                       printf 'fastflowlm' ;;
        *) return 1 ;;
    esac
}

ropero_engine_compatible() {
    local engine os
    engine=$(ropero_engine_canonical "${1:-}") || return 1
    os=$(ropero_detect_os 2>/dev/null || echo unknown)
    case "$engine" in
        llama-server) [[ "$os" != "termux" ]] ;;
        airllm) return 0 ;;
        mlx_lm)
            # mlx-lm requiere Metal/Apple Silicon; Darwin x86_64 no es
            # compatible aunque el sistema operativo se anuncie como macOS.
            [[ "$os" == "macos" && "$(uname -m 2>/dev/null)" == "arm64" ]]
            ;;
        tensorrt-llm)
            [[ "$os" == "cachy" || "$os" == "arch" ]] && command -v nvidia-smi >/dev/null 2>&1
            ;;
        fastflowlm) [[ "$os" == "cachy" || "$os" == "arch" ]] && ropero_has_amd_npu ;;
    esac
}

# ropero_engine_list → una línea por engine no-llama soportado.
ropero_engine_list() { printf '%s\n' $ROPERO_ENGINES; }

# ropero_engine_is_known <engine> → 0 si lo soporta el launcher.
ropero_engine_is_known() {
    local e="${1:-}" c
    for c in $ROPERO_ENGINES; do
        if [[ "$e" == "$c" ]]; then return 0; fi
    done
    return 1
}

# -----------------------------------------------------------------------------
# ropero_engine_auth_native <engine>
#   0 → el servidor valida `Authorization: Bearer` por sí mismo.
#   1 → NO valida. Entonces ropero pone un proxy delante (ver _authproxy.py).
#
# HECHOS VERIFICADOS, no suposiciones:
#   llama-server     → SÍ, tiene --api-key.
#   airllm           → SÍ, porque el server es NUESTRO (_airllm_server.py) y
#                      se lo añadimos nosotros.
#   mlx_lm.server    → NO. `mlx_lm/server.py` no menciona api_key ni lee
#                      ninguna variable de entorno para auth (grep sobre 0.31.3).
#   trtllm-serve     → NO. La doc oficial pone api_key="tensorrt_llm" en el
#                      cliente, que es el campo que exige el SDK de OpenAI, no
#                      una comprobación del servidor. No existe el flag.
#   flm serve        → NO. La doc dice literalmente "No real API key is needed —
#                      just pass \"flm\" as a placeholder" y "the key is not checked".
#
# Consecuencia de seguridad que esto obliga: con HOST=0.0.0.0 y sin proxy,
# los engines quedarían SIN AUTH expuestos a la LAN. Por eso el proxy es el
# default y solo se desactiva con ROPERO_NO_AUTH_PROXY=1 (que avisa).
# -----------------------------------------------------------------------------
ropero_engine_auth_native() {
    case "${1:-}" in
        airllm) return 0 ;;      # servidor propio, con --api-key
        *)      return 1 ;;
    esac
}

# -----------------------------------------------------------------------------
# ropero_engine_pinned_model <engine>
#   imprime → el ÚNICO nombre de modelo que el engine acepta en el body.
#   imprime "" → el engine enruta por el nombre que le pase ropero.
#
# ── POR QUÉ EXISTE ───────────────────────────────────────────────────────────
# La mayoría de engines SÍ aceptan el nombre que les pasa ropero, porque tienen
# un flag para fijarlo (ver ropero_engine_cmd):
#   llama-server    → --alias
#   airllm          → --model-id
#   trtllm-serve    → --served_model_name
#   flm serve       → (no enruta: `flm serve` habla con su almacén por el tag)
#
# mlx_lm.server NO tiene ninguno de esos flags. Verificado contra
# `mlx_lm.server --help` de 0.32.x: no existen --alias, --served-model-name ni
# --model-name. Y no es que lo ignore por descuido: su `ModelProvider` construye
# `self._model_map = {"default_model": args.model}` y `load()` resuelve con
# `self._model_map.get(model_path, model_path)`. Lo único registrado es
# "default_model". Cualquier otro nombre se usa tal cual, y "tal cual" para un
# engine de HF significa REPO_ID: el cliente manda "gptoss", huggingface_hub
# busca el repo `gptoss`, y la respuesta es un 401 "Repository Not Found" que
# el engine envuelve en un 404.
#
# El síntoma es desconcertante porque el error habla de HuggingFace y parece
# "que le falta el repo descargado" cuando el repo está en disco y el modelo
# cargado. Medido con gpt-oss-20b ya en memoria (11.5 GB, 36864 MiB):
#   model="gptoss"        → 404 + GET huggingface.co/api/models/gptoss
#   model="metaloss"      → 404 + GET huggingface.co/api/models/metaloss
#   model="default_model" → 200
#
# Por eso el arreglo va en el proxy (--rewrite-model, ver _authproxy.py): es el
# que ya anuncia `id: <modelo de ropero>` en /v1/models, así que sin reescribir
# el proxy Estaría prometiendo una identidad que su propio engine rechaza. Y es
# la única capa donde tiene arreglo: el engine no tiene flag, y opencode
# mandaría el id que le ponga el usuario en su config.
# -----------------------------------------------------------------------------
ropero_engine_pinned_model() {
    case "${1:-}" in
        mlx_lm) printf 'default_model' ;;
        *)      ;;   # enrutan por nombre: que llegue el de ropero
    esac
    return 0
}

# -----------------------------------------------------------------------------
# ropero_python_bin → intérprete con el que está instalado el engine.
#
# Por qué no `python3` a pelo: airllm y mlx_lm viven en el Python de asdf, y
# el shim de asdf puede no tener versión fijada para `python3` (el propio
# 20-compile-llama.sh ya sufre de eso con cmake/ninja). Prioridad:
#   1. $ROPERO_PYTHON (override explícito)
#   2. el `python3` real del Python activo, saltándonos el shim
#   3. lo que haya en el PATH
# -----------------------------------------------------------------------------
ropero_python_bin() {
    if [[ -n "${ROPERO_PYTHON:-}" && -x "${ROPERO_PYTHON}" ]]; then
        printf '%s' "$ROPERO_PYTHON"; return 0
    fi
    local d
    for d in "$HOME"/.asdf/installs/python/*/bin; do
        if [[ -x "$d/python3" ]]; then
            printf '%s' "$d/python3"; return 0
        fi
    done
    if command -v python3 >/dev/null 2>&1; then
        command -v python3; return 0
    fi
    return 1
}

# -----------------------------------------------------------------------------
# ropero_engine_resolve <engine> → ruta absoluta al ejecutable del server.
#
# El orden importa y NO es arbitrario: primero $BIN_DIR (lo que ropero linkeó y
# por tanto lo que ropero quiere que gobierne), después las rutas largas
# conocidas (venv de tensorrt, /opt/fastflowlm), y el PATH al final.
#
# Sin `[[ test ]] && { ... }` en ninguna rama: bajo `set -e` (el entrypoint lo
# tiene) una lista `&&` que evalúa a falso ABORTA el script entero. El codebase
# ya lleva comentarios pidiendo cuidado con esto; aquí se aplica el `if`.
# -----------------------------------------------------------------------------
ropero_engine_resolve() {
    local engine="${1:-}"
    local cand d
    case "$engine" in
        llama-server)
            # llama.cpp se compila fuera de este registro, pero el instalador
            # seleccionado necesita el mismo contrato de probe que los demás
            # engines. Preferimos el artefacto de build (fuente de verdad) y
            # después el enlace/PATH para instalaciones ya existentes.
            for cand in "${LLAMA_DIR:-$HOME/llama.cpp}/build/bin/llama-server" "$BIN_DIR/llama-server"; do
                if [[ -x "$cand" ]]; then printf '%s' "$cand"; return 0; fi
            done
            if command -v llama-server >/dev/null 2>&1; then
                command -v llama-server; return 0
            fi
            ;;
        airllm)
            # Servidor NUESTRO (airllm es librería pura). Se linkea como un
            # ejecutable más, exactamente igual que llama-server.
            for cand in "$BIN_DIR/airllm-server" "$MODULES_DIR/_airllm_server.py"; do
                if [[ -x "$cand" ]]; then printf '%s' "$cand"; return 0; fi
            done
            ;;
        mlx_lm)
            for cand in "$BIN_DIR/mlx_lm.server"; do
                if [[ -x "$cand" ]]; then printf '%s' "$cand"; return 0; fi
            done
            # Rutas de los Pythons de asdf: el script pudo instalarse con uno y
            # ejecutarse con otro (mismo problema que _ropero_python_script_dirs).
            for d in "$HOME"/.asdf/installs/python/*/bin; do
                if [[ -x "$d/mlx_lm.server" ]]; then printf '%s' "$d/mlx_lm.server"; return 0; fi
            done
            if command -v mlx_lm.server >/dev/null 2>&1; then
                command -v mlx_lm.server; return 0
            fi
            ;;
        tensorrt-llm)
            for cand in "$BIN_DIR/trtllm-serve" "$HOME/.venv-tensorrt/bin/trtllm-serve"; do
                if [[ -x "$cand" ]]; then printf '%s' "$cand"; return 0; fi
            done
            if command -v trtllm-serve >/dev/null 2>&1; then
                command -v trtllm-serve; return 0
            fi
            ;;
        fastflowlm)
            for cand in "/opt/fastflowlm/bin/flm" "$BIN_DIR/flm"; do
                if [[ -x "$cand" ]]; then printf '%s' "$cand"; return 0; fi
            done
            if command -v flm >/dev/null 2>&1; then
                command -v flm; return 0
            fi
            ;;
    esac
    return 1
}

# -----------------------------------------------------------------------------
# ropero_engine_probe <engine> → "ok" | "missing" | "broken" en stdout, y el
# detalle a stderr. El detalle va a stderr a propósito: el llamante lee el
# veredicto con `$(...)` y no quiere el texto mezclado con el valor.
#
# `broken` es un estado que el código anterior NO tenía, y es el que importa:
# "airllm instalado" no es lo mismo que "airllm utilizable". En esta máquina
# airllm está instalado (2.11.0) y a la vez es inimportable: su
# `airllm/__init__.py` importa `LlamaForCausalLM` de un transformers 5.6.0.dev0
# que ya no lo expone. El `command -v airllm` del install antiguoRespondería
# "instalado" y linkearía cero binarios (airllm no registra ningún entry point).
# Un probe que importa de verdad es lo que separa los dos casos.
# -----------------------------------------------------------------------------
ropero_engine_probe() {
    local engine="${1:-}"
    local bin py out

    if ! bin=$(ropero_engine_resolve "$engine"); then
        printf 'missing'
        ropero_info "  $engine: no encuentro el ejecutable de su server"
        return 1
    fi

    case "$engine" in
        airllm)
            # El server es nuestro, pero su backend es airllm: sin los tres
            # imports no puede cargar ni servir inferencia.
            if ! py=$(ropero_python_bin); then
                printf 'broken'; ropero_info "  airllm: sin intérprete python3"
                return 1
            fi
            if ! out=$("$py" -c 'import airllm, fastapi, uvicorn; print(getattr(airllm,"__version__","?"))' 2>&1); then
                printf 'broken'
                ropero_info "  airllm: no se pueden importar airllm, fastapi y uvicorn"
                ropero_info "    $(printf '%s' "$out" | tail -1)"
                return 1
            fi
            ;;
        mlx_lm)
            if ! py=$(ropero_python_bin); then
                printf 'broken'; ropero_info "  mlx_lm: sin intérprete python3"
                return 1
            fi
            if ! "$py" -c 'import mlx_lm.server' >/dev/null 2>&1; then
                printf 'broken'; ropero_info "  mlx_lm: no se puede importar mlx_lm.server"
                return 1
            fi
            ;;
        tensorrt-llm)
            # El venv puede tener el ejecutable y no tener tensorrt_llm dentro
            # (instalación a medias). El intérprete DEL VENV es el que importa,
            # no el del PATH: por eso se deriva del propio binario resuelto.
            local vpy
            vpy="$(dirname "$bin")/python"
            if [[ ! -x "$vpy" ]]; then
                vpy=$(dirname "$bin")/python3
            fi
            if [[ ! -x "$vpy" ]]; then
                printf 'broken'
                ropero_info "  tensorrt-llm: no encuentro el python de $bin"
                return 1
            fi
            if ! "$vpy" -c 'import tensorrt_llm' >/dev/null 2>&1; then
                printf 'broken'
                ropero_info "  tensorrt-llm: $vpy no puede importar tensorrt_llm"
                return 1
            fi
            ;;
        fastflowlm)
            # `flm validate` es el único que sabe si la NPU está viva. Con el
            # binario instalado y la NPU muerta, el engine no vale para nada.
            if ! out=$("$bin" validate 2>&1); then
                printf 'broken'
                ropero_info "  fastflowlm: 'flm validate' falla"
                ropero_info "    $(printf '%s' "$out" | tail -3 | tr '\n' ' ')"
                ropero_info "    necesita: driver amdxdna + xrt/xrt-plugin-amdxdna + firmware NPU >= 1.1.0.0"
                return 1
            fi
            ;;
    esac
    printf 'ok'
    return 0
}

# -----------------------------------------------------------------------------
# ropero_engine_env <engine> → líneas "K=V" para el sub-proceso.
#
# Punto no negociable: un engine SIN PROXY escucha en 127.0.0.1, nunca en
# $HOST. El proxy es el único que se expone. Si el engine se expusiera también,
# la API key del proxy no valdría nada — sería una puerta trasera sin cerradura.
# -----------------------------------------------------------------------------
ropero_engine_env() {
    case "${1:-}" in
        fastflowlm)
            # Sin el chequeo de versión, `flm` hace una llamada de red en cada
            # arranque. En una máquina "offline y privado" eso es ruido.
            printf '%s\n' "FLM_DISABLE_UPDATE_CHECK=${FLM_DISABLE_UPDATE_CHECK:-1}"
            ;;
    esac
}

# -----------------------------------------------------------------------------
# ropero_engine_cmd <engine> <model_path> <model_id> <host> <port> <ctx>
#
# Imprime el argv, UN ARGUMENTO POR LÍNEA. Motivo: el caller lo lee con
# `mapfile -t` y lo ejecuta con `"${cmd[@]}"`. Así ningún argumento con
# espacios (un --chat-template-args con JSON, un --extra_llm_api_options con
# una ruta) se pierde por un word-splitting. Es el mismo patrón que ya usa
# get_model_args_* para llama-server.
#
# `host` llega ya resuelto por el launcher: 127.0.0.1 si hay proxy, $HOST si el
# engine tiene auth propia (hoy solo airllm). Se mantiene el parámetro para que
# un engine futuro con auth nativa no obligue a tocar esta función.
# -----------------------------------------------------------------------------
ropero_engine_cmd() {
    local engine="${1:-}" path="${2:-}" model_id="${3:-}" host="${4:-}" port="${5:-}" ctx="${6:-}"
    local bin py
    if ! bin=$(ropero_engine_resolve "$engine"); then return 1; fi

    case "$engine" in
        airllm)
            # airllm no trae server: el binario es _airllm_server.py y necesita
            # intérprete. Este SÍ valida la API key, y por eso no lleva proxy.
            if ! py=$(ropero_python_bin); then return 1; fi
            printf '%s\n' \
                "$py" "$bin" \
                --model "$path" \
                --model-id "$model_id" \
                --host "$host" \
                --port "$port" \
                --max-seq-len "$ctx" \
                --api-key "$LLAMA_API_KEY"
            ;;
        mlx_lm)
            # Flags REALES de mlx_lm 0.31.3 (`mlx_lm.server --help`):
            #   NO existe --api-key → de ahí el proxy.
            #   --max-tokens es el default de tokens a GENERAR, no el contexto.
            #   Los que sí sirven para exprimir la GPU de Apple:
            #     --prefill-step-size, --prompt-cache-size, --prompt-cache-bytes,
            #     --decode-concurrency, --prompt-concurrency, --chat-template-args,
            #     --draft-model + --num-draft-tokens (speculative decoding),
            #     --pipeline (pipelining en vez de tensor parallelism).
            #   La plantilla de ropero.d/30-template-mlx.sh los usa.
            printf '%s\n' \
                "$bin" \
                --model "$path" \
                --host "$host" \
                --port "$port"
            ;;
        tensorrt-llm)
            # `trtllm-serve serve [OPTIONS] MODEL` (1.x). La forma sin `serve`
            # existe pero está deprecated. NO hay --api_key: lo pone el proxy.
            # --served_model_name es lo más parecido al `--alias` de
            # llama-server: sin esto /v1/models devuelve el nombre del
            # checkpoint y `ropero stop <nombre>` no reconocería el modelo.
            printf '%s\n' \
                "$bin" serve \
                --model "$path" \
                --host "$host" \
                --port "$port" \
                --served_model_name "$model_id"
            ;;
        fastflowlm)
            # `flm serve [model] --port N` (default 52625, alias -p).
            # --ctx-len se ajusta solo a potencia de 2 (mínimo 512).
            # NO tiene --host: FLM sirve en 127.0.0.1 → proxy obligatorio.
            # --cors 0: el proxy es el que ve al exterior y un endpoint de
            # inferencia no necesita cross-origin.
            printf '%s\n' \
                "$bin" serve "$path" \
                --port "$port" \
                --ctx-len "$ctx" \
                --cors 0
            ;;
    esac
    return 0
}

# -----------------------------------------------------------------------------
# ropero_engine_internal_port <engine> <port> → puerto libre para el engine que va
# detrás del proxy.
#
# Por qué hace falta: el engine NO puede escuchar en el puerto que opencode
# conoce. `ropero stop` mata lo que escucha en ESE puerto; si el engine
# escuchara ahí, sería el engine el muerto y el proxy quedaría huérfano
# reenviando a un puerto muerto. Con el engine en un puerto interno y el proxy
# en el de ropero, matar el puerto de ropero = matar el proxy = el proxy mata a
# su hijo (ver --engine-cmd-file en _authproxy.py).
#
# ── POR QUÉ UN PUERTO FIJO POR ENGINE Y NO "puerto de ropero + 1000" ─────────
# Antes se derivaba del puerto público (9999 → 10999, 9992 → 10992), lo que
# tenía dos fallos medibles:
#
#   1. Si el engine cambia MODEL_PORT, el puerto INTERNO cambia con él. El
#      puerto interno es un detalle de implementación invisible (está detrás del
#      proxy, en loopback) y no debería depender de una decisión que es del
#      cliente: `ropero metaloss --port 9995` movía el engine de 10992 a 10995
#      sin que nadie lo hubiera pedido.
#   2. Todos los engines que comparten slot (airllm, tensorrt-llm y fastflowlm
#      usan los slots :9999/:9998) caían en el MISMO 10999. Hoy no chocan
#      porque el check de "puerto ocupado" impide dos modelos en un slot, pero
#      es una colisión esperando: en cuanto dos engines quisieran convivir (un
#      servidor y su prefill, un draft, una cuantización compartida) el segundo
#      se quedaría sin puerto.
#
# Con un puerto por engine las dos cosas desaparecen: el interno depende sólo
# del engine, y dos engines distintos nunca se pisan. La tabla es fija y
# documentable, que es lo que hace falta para escribir "el engine de metaloss
# escucha en 127.0.0.1:10992" sin quedarse con la duda de si es el de ahora.
#
#   engine          puerto interno   por qué ese
#   ─────────────────────────────────────────────────────────────────────────
#   mlx_lm          10992             :9992 (slot propio de metaloss) + 1000
#   airllm          11999             bloque 11xxx
#   tensorrt-llm    12999             bloque 12xxx
#   fastflowlm      13999             bloque 13xxx
#
# El 1000 de mnemónico es el de siempre (el interno nunca es el público), y
# acaba en 999 para que se lea como "el 999 de siempre, desplazado".
#
# ── Y SI EL PUERTO ESTÁ OCUPADO ──────────────────────────────────────────────
# Se sube de uno en uno (base, +1, ... +49) antes de rendirse. El caso real: un
# launch anterior mal parado dejó el engine vivo. Preferimos un 11093 a un
# "no hay puerto" que deja al usuario sin modelo, y el log dice cuál se usó.
# Rango fijo y determinista a propósito: dos launches seguidos del mismo modelo
# caen en el mismo sitio, que es lo que permite documentarlo.
#
# `port` (segundo argumento) sólo se usa como plan B, para un engine que no esté
# en la tabla: fallback al comportamiento antiguo, puerto + 1000.
# -----------------------------------------------------------------------------
ropero_engine_internal_port() {
    local engine="${1:-}" port="${2:-}" base i p
    case "$engine" in
        mlx_lm)       base=10992 ;;
        airllm)       base=11999 ;;
        tensorrt-llm) base=12999 ;;
        fastflowlm)   base=13999 ;;
        *)
            _ropero_valid_port "$port" || return 1
            base=$(( port + 1000 ))
            ;;
    esac
    for (( i = 0; i < 50; i++ )); do
        p=$(( base + i ))
        if (( p > 65535 )); then break; fi
        if ! port_listening "$p" 2>/dev/null; then
            printf '%s' "$p"; return 0
        fi
    done
    return 1
}

# -----------------------------------------------------------------------------
# ropero_has_amd_npu → 0 si esta máquina tiene una NPU AMD (XDNA) utilizable.
#
# Las tres señales, de más a menos fiable:
#   1. /dev/accel/accel0 — es lo que ve `flm validate`. La señal buena.
#   2. `xrt-smi examine` lista NPU — `flm run` usa XRT (xrt::device(0)), NO el
#      DRM device, así que esto es lo que de verdad decide si puede correr.
#      La guía de Linux de FastFlowLM lo dice explícitamente: que `flm validate`
#      pase NO garantiza que XRT pueda abrir el índice 0.
#   3. `lscpu` menciona XDNA — la más floja: dice que el silicon lo tiene, no
#      que el driver esté.
#
# Se exige Linux: una NPU AMD en macOS no existe, y el stack no es el mismo en
# otro Linux.
# -----------------------------------------------------------------------------
ropero_has_amd_npu() {
    [[ "$(uname -s)" == "Linux" ]] || return 1
    grep -qiE 'GenuineAMD|AuthenticAMD' /proc/cpuinfo 2>/dev/null || return 1
    if [[ -e /dev/accel/accel0 ]]; then return 0; fi
    if command -v xrt-smi >/dev/null 2>&1; then
        if xrt-smi examine 2>/dev/null | grep -qiE 'npu|accel'; then return 0; fi
    fi
    if lscpu 2>/dev/null | grep -qiE 'xdna|ryzen ai'; then return 0; fi
    return 1
}

# -----------------------------------------------------------------------------
# ropero_engine_label <engine> → nombre corto para la columna ENGINE del status.
# -----------------------------------------------------------------------------
ropero_engine_label() {
    case "${1:-}" in
        airllm)       printf 'airllm' ;;
        mlx_lm)       printf 'mlx_lm' ;;
        tensorrt-llm) printf 'tensorrt' ;;
        fastflowlm)   printf 'fastflowlm' ;;
        llama-server) printf 'llama-server' ;;
        *)            printf '%s' "${1:-?}" ;;
    esac
}
