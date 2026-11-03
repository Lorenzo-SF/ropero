#!/usr/bin/env bash
# =============================================================================
# test/lib/test-common.sh — librería compartida de los test/test-<modelo>.sh
# =============================================================================
# bash 3.2 (macOS, ADR-1): sin namerefs, sin arrays asociativos, sin mapfile.
# Seguro de sourcear: sólo definiciones, cero ejecución.
#
# QUÉ HACE (todo lo que antes estaba duplicado en los siete scripts)
#   · resuelve las rutas del repo desde la UBICACIÓN DEL SCRIPT (no del cwd ni
#     de ROPERO_ROOT: el test se puede lanzar desde cualquier sitio)
#   · resuelve la API key del engine local (env > estado > OPENAI_API_KEY)
#   · monta un provider HERMÉTICO de opencode ("ropero-local") en el
#     directorio de cada batería, apuntando a 127.0.0.1:<puerto del modelo>
#   · arranca el modelo con `ropero <modelo> --background`, lee el puerto de
#     la última línea `ROPERO_PORT=<n>` y comprueba que el engine responde
#   · lanza `opencode run` guardando stdout (JSON) y stderr en ficheros
#   · exporta la sesión (`opencode session export`) y copia el log del engine
#   · genera `recopilacion-completa.md` con el CONTENIDO de cada batería
#
# POR QUÉ UN PROVIDER PROPIO Y NO EL DEL USUARIO
#   La config global de opencode del usuario declara providers remotos (con
#   hosts que no son de esta máquina). El test no la lee, no la necesita y no
#   la toca: monta su propio provider "ropero-local" (openai-compatible) en el
#   directorio de cada batería, con el endpoint 127.0.0.1:<puerto> del modelo
#   que acaba de arrancar. Además aísla el HOME de opencode (XDG_*) para que
#   el test no dependa de la config global ni ensucie la base de datos de
#   sesiones del usuario.
#
# Variables que el script de test puede exportar antes de sourcear:
#   TEST_MODEL       (obligatorio: p. ej. groot)
#   TEST_DOCS_DIR    (default: $REPO_ROOT/docs)
#   TEST_LOG_FILE    (default: $HOME/.logs/ropero/<modelo>.log)
#   TEST_OUT_DIR     (default: $HOME/test/<modelo>)
# =============================================================================

# ── Rutas: se resuelven desde la ubicación de ESTE fichero ───────────────────
TEST_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_REPO_ROOT="$(cd "$TEST_LIB_DIR/../.." && pwd)"

# Acumulador de resultados: una línea por batería
#   slug|directorio|json|sesion|log|stderr|estado
TEST_RESULTADOS="${TEST_RESULTADOS:-}"

# ── Salida por pantalla ──────────────────────────────────────────────────────
_test_info()  { printf '[test] %s\n' "$*"; }
_test_warn()  { printf '[test] AVISO: %s\n' "$*" >&2; }
_test_error() { printf '[test] ERROR: %s\n' "$*" >&2; }

# -----------------------------------------------------------------------------
# _test_prueba_slug <num> [largo|gorda] — nombre de la batería.
#   1 → prueba1 · 2 → prueba2 · 3 → prueba3
#   largo → prueba_larga · gorda → prueba_gorda
# Es a la vez el nombre del directorio, del markdown y de los artefactos.
# -----------------------------------------------------------------------------
_test_prueba_slug() {
    local num="${1:-}" sufijo="${2:-}"
    case "$sufijo" in
        largo) printf 'prueba_larga\n' ;;
        gorda) printf 'prueba_gorda\n' ;;
        *)     printf 'prueba%s\n' "$num" ;;
    esac
}

# -----------------------------------------------------------------------------
# _test_baterias — las CINCO baterías, una por línea: "<num> <sufijo>".
# (prueba1, prueba2, prueba3, prueba_larga, prueba_gorda)
# -----------------------------------------------------------------------------
_test_baterias() {
    printf '%s\n' "1" "2" "3" "larga largo" "gorda gorda"
}

# -----------------------------------------------------------------------------
# _test_api_key — resuelve la API key del engine local.
#
# Orden (el mismo que bin/lib/04-launch.sh:rp_api_key, con el último peldaño
# explícito): $LLAMA_API_KEY > $STATE_DIR/api_key > $OPENAI_API_KEY > default.
#
# Se resuelve DESPUÉS de arrancar el modelo a propósito: si no había ninguna
# key, ropero genera una y la guarda en $STATE_DIR/api_key, que es
# exactamente la que acepta el engine que se acaba de levantar. Con la key
# resuelta antes, el probe fallaría con 401 contra un engine recién arrancado.
#
# NUNCA se imprime ni se escribe en el informe: es un secreto.
# -----------------------------------------------------------------------------
_test_api_key() {
    if [[ -n "${LLAMA_API_KEY:-}" ]]; then
        printf '%s' "$LLAMA_API_KEY"
        return 0
    fi
    local f="${STATE_DIR:-$HOME/.local/state/ropero}/api_key" k=""
    if [[ -s "$f" ]]; then
        k="$(head -1 "$f" 2>/dev/null)"
    fi
    if [[ -n "$k" ]]; then
        printf '%s' "$k"
        return 0
    fi
    printf '%s' "${OPENAI_API_KEY:-sk-local-dev-key}"
    return 0
}

# -----------------------------------------------------------------------------
# _test_json_escape <cadena> — escapa una cadena para meterla en un JSON.
# -----------------------------------------------------------------------------
_test_json_escape() {
    printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

# -----------------------------------------------------------------------------
# _test_model_limits <modelo> — "contexto salida" del modelo bajo prueba.
# El contexto se lee del [model].context del TOML del catálogo (models/*/).
# Si el modelo no está en el catálogo, valores prudentes.
# -----------------------------------------------------------------------------
_test_model_limits() {
    local modelo="$1" f ctx="" out=""
    for f in "$TEST_REPO_ROOT"/models/*/"$modelo".toml; do
        [[ -f "$f" ]] || continue
        ctx="$(grep -E '^[[:space:]]*context[[:space:]]*=' "$f" 2>/dev/null \
            | head -1 | cut -d= -f2 | tr -cd '0-9')"
        [[ -n "$ctx" ]] && break
        ctx=""
    done
    [[ -n "$ctx" ]] || ctx=32768
    out=$((ctx - 1024))
    (( out > 65536 )) && out=65536
    (( out < 4096 )) && out=4096
    printf '%s %s\n' "$ctx" "$out"
}

# -----------------------------------------------------------------------------
# _test_write_opencode_config <dir> <modelo> <puerto> <apikey>
#
# Escribe el project config de opencode de la batería:
#   · provider "ropero-local" (openai-compatible) → 127.0.0.1:<puerto>/v1
#   · el modelo bajo prueba, con limit.context/limit.output del catálogo
#   · un SEGUNDO id "titulo" con limit.output=64, que es el `small_model`:
#     opencode usa small_model para "lightweight tasks like title generation",
#     así la petición de título va a un id acotado y no al modelo entero.
#     (upstream anomalyco/opencode#54401; el plugin de ropero lo acota aún
#     más con maxTokens, pero el plugin no se instala en el test)
#   · model/small_model apuntan al provider propio: el test NO depende de la
#     config global de opencode del usuario.
#
# Imprime la ruta del fichero. La key va en el fichero con chmod 600 y no se
# imprime nunca.
# -----------------------------------------------------------------------------
_test_write_opencode_config() {
    local dir="$1" modelo="$2" puerto="$3" key="$4"
    local f="$dir/opencode.json" limits ctx out key_json
    limits="$(_test_model_limits "$modelo")"
    ctx="${limits%% *}"
    out="${limits##* }"
    key_json="$(_test_json_escape "$key")"
    cat >"$f" <<EOF
{
  "\$schema": "https://opencode.ai/config.json",
  "model": "ropero-local/$modelo",
  "small_model": "ropero-local/titulo",
  "providers": {
    "ropero-local": {
      "package": "@opencode/ai/providers/openai-compatible",
      "name": "ropero local",
      "settings": {
        "baseURL": "http://127.0.0.1:$puerto/v1",
        "apiKey": "$key_json"
      },
      "models": {
        "$modelo": {
          "name": "$modelo",
          "limit": { "context": $ctx, "output": $out }
        },
        "titulo": {
          "name": "titulo",
          "limit": { "context": 4096, "output": 64 }
        }
      }
    }
  }
}
EOF
    chmod 600 "$f" 2>/dev/null || true
    printf '%s' "$f"
}

# -----------------------------------------------------------------------------
# _test_probe <puerto> <apikey> — ¿responde el engine en /v1/models?
# Devuelve 0 si responde, 1 si no (el mensaje va a stderr).
# -----------------------------------------------------------------------------
_test_probe() {
    local puerto="$1" key="$2" salida rc=0
    salida="$(curl -fsS --max-time 5 \
        -H "Authorization: Bearer $key" \
        "http://127.0.0.1:$puerto/v1/models" 2>&1)" || rc=$?
    if (( rc != 0 )); then
        printf '%s\n' "$salida" >&2
        return 1
    fi
    if [[ -z "$salida" ]]; then
        printf 'respuesta vacía de /v1/models\n' >&2
        return 1
    fi
    return 0
}

# -----------------------------------------------------------------------------
# _test_parar_entorno — deja el entorno libre antes de medir.
#
# `ropero stop all` (para por puerto) + `pkill -9 -x` de los agentes, como
# hace bin/lib/cmd-test.sh. NUNCA `pkill -9 python`: mataría procesos python
# ajenos a este test.
# -----------------------------------------------------------------------------
_test_parar_entorno() {
    ropero stop all >/dev/null 2>&1 || true
    local p
    for p in ${TEST_MATAR_AGENTES:-llama-server opencode mcode claude}; do
        pkill -9 -x "$p" >/dev/null 2>&1 || true
    done
    return 0
}

# -----------------------------------------------------------------------------
# _test_arrancar_modelo <modelo> <fichero-stderr> — arranca el modelo en
# background e imprime el puerto.
#
# `ropero <modelo> --background` YA ESPERA a que el modelo esté listo (por eso
# no hace falta un `sleep 20` a ciegas) y su stdout termina con la línea
# máquina `ROPERO_PORT=<n>` (contrato de bin/lib/cmd-runtime.sh).
# -----------------------------------------------------------------------------
_test_arrancar_modelo() {
    local modelo="$1" errfile="$2" salida rc=0 puerto=""
    salida="$(ropero "$modelo" --background 2>"$errfile")" || rc=$?
    if (( rc != 0 )); then
        _test_error "no se pudo arrancar '$modelo' (rc=$rc)"
        [[ -s "$errfile" ]] && sed 's/^/    /' "$errfile" >&2
        return 1
    fi
    # La ÚLTIMA línea de stdout es la salida máquina.
    puerto="$(printf '%s\n' "$salida" | tail -1 | sed -n 's/^ROPERO_PORT=\([0-9][0-9]*\)$/\1/p')"
    if [[ -z "$puerto" ]]; then
        puerto="$(printf '%s\n' "$salida" | grep -E '^ROPERO_PORT=[0-9]+$' | tail -1 | cut -d= -f2)"
    fi
    if [[ -z "$puerto" ]]; then
        _test_error "'ropero $modelo --background' no devolvió ROPERO_PORT=<n>"
        printf '%s\n' "$salida" | sed 's/^/    /' >&2
        return 1
    fi
    printf '%s' "$puerto"
    return 0
}

# -----------------------------------------------------------------------------
# _test_session_id <json> — primer session id del JSON de opencode.
# Mismo criterio que bin/lib/cmd-test.sh:_rp_test_sesion_id.
# -----------------------------------------------------------------------------
_test_session_id() {
    local f="$1" id=""
    [[ -s "$f" ]] || return 1
    id="$(jq -r 'select(.sessionID or .session_id) | (.sessionID // .session_id)' "$f" 2>/dev/null \
        | sort -u | sed '/^$/d' | head -1)"
    [[ -n "$id" ]] || return 1
    printf '%s' "$id"
    return 0
}

# -----------------------------------------------------------------------------
# _test_errores_json <json> — imprime los errores REALES del JSON de opencode.
# Esto es lo que antes se comía con `2>/dev/null`: sin esto, un
# provider.no-route salía como un fichero de 0 bytes y sin diagnóstico.
# -----------------------------------------------------------------------------
_test_errores_json() {
    local f="$1" msg
    [[ -s "$f" ]] || return 0
    while IFS= read -r msg; do
        [[ -n "$msg" ]] || continue
        printf '[test] ERROR de opencode: %s\n' "$msg" >&2
    done <<EOF
$(jq -r 'select(.type == "error") | (.error.message // .message // "error sin mensaje")' \
    "$f" 2>/dev/null)
EOF
    return 0
}

# -----------------------------------------------------------------------------
# _test_opencode_env <dir> — aísla el HOME de opencode (XDG_*).
# Así el test no lee la config global del usuario (ni su plugin) y no escribe
# en su base de datos de sesiones.
# -----------------------------------------------------------------------------
_test_opencode_env() {
    local d="$1"
    mkdir -p "$d/config" "$d/data" "$d/state" "$d/cache" || return 1
    XDG_CONFIG_HOME="$d/config"
    XDG_DATA_HOME="$d/data"
    XDG_STATE_HOME="$d/state"
    XDG_CACHE_HOME="$d/cache"
    export XDG_CONFIG_HOME XDG_DATA_HOME XDG_STATE_HOME XDG_CACHE_HOME
    return 0
}

# -----------------------------------------------------------------------------
# _test_record <slug> <dir> <json> <sesion> <log> <stderr> <estado>
# -----------------------------------------------------------------------------
_test_record() {
    TEST_RESULTADOS="${TEST_RESULTADOS:+$TEST_RESULTADOS
}$1|$2|$3|$4|$5|$6|$7"
    return 0
}

# -----------------------------------------------------------------------------
# _test_anexar <salida> <título> <fichero> [max_líneas]
# Mete el CONTENIDO real de un artefacto en el informe (no sólo su ruta).
# -----------------------------------------------------------------------------
_test_anexar() {
    local out="$1" titulo="$2" f="$3" max="${4:-400}" total
    {
        printf '\n### %s\n\n' "$titulo"
        if [[ -z "$f" || ! -f "$f" ]]; then
            printf '_(no hay fichero: %s)_\n' "${f:-(desconocido)}"
            return 0
        fi
        total="$(wc -l <"$f" 2>/dev/null | tr -d ' ')"
        [[ "$total" =~ ^[0-9]+$ ]] || total=0
        printf '````text\n'
        if (( total > max )); then
            printf '_(%s líneas en total; se muestran las últimas %s)_\n' "$total" "$max"
            tail -n "$max" "$f"
        else
            cat "$f"
        fi
        printf '````\n'
    } >>"$out"
    return 0
}

# -----------------------------------------------------------------------------
# _test_run_prueba <num> [largo|gorda] — ejecuta UNA batería y deja en su
# directorio los artefactos:
#   <slug>.json                    salida JSON de opencode
#   sesion-<slug>.md               sesión exportada
#   log-llama-sesion-<slug>.txt    log del engine
#   opencode-err-<slug>.txt        stderr de opencode (NUNCA se descarta)
#   opencode.json                  provider hermético de la batería
#   ropero-arranque.err            stderr del arranque del modelo
# -----------------------------------------------------------------------------
_test_run_prueba() {
    local num="${1:-}" sufijo="${2:-}"
    local slug md_file dir json sesion log err rc=0 puerto key cfg sid

    slug="$(_test_prueba_slug "$num" "$sufijo")"
    md_file="$TEST_DOCS_DIR/$slug.md"
    dir="$TEST_OUT_DIR/$slug"

    printf '\n=== %s (modelo: %s) ===\n' "$slug" "$TEST_MODEL"
    printf 'markdown:   %s\n' "$md_file"
    printf 'directorio: %s\n' "$dir"

    if [[ ! -f "$md_file" ]]; then
        _test_error "falta la batería: $md_file"
        _test_record "$slug" "$dir" "" "" "" "" "sin markdown"
        return 1
    fi
    if ! mkdir -p "$dir"; then
        _test_error "no se pudo crear el directorio $dir"
        _test_record "$slug" "$dir" "" "" "" "" "sin directorio"
        return 1
    fi

    json="$dir/$slug.json"
    sesion="$dir/sesion-$slug.md"
    log="$dir/log-llama-sesion-$slug.txt"
    err="$dir/opencode-err-$slug.txt"

    # 1. Entorno libre: `ropero stop all` + pkill -x de los agentes.
    _test_parar_entorno
    rm -f "$TEST_LOG_FILE" 2>/dev/null || true

    # 2. Arrancar el modelo y leer su puerto de la última línea de stdout.
    puerto="$(_test_arrancar_modelo "$TEST_MODEL" "$dir/ropero-arranque.err")" || {
        _test_record "$slug" "$dir" "$json" "$sesion" "$log" "$err" "el modelo no arrancó"
        return 1
    }
    TEST_PUERTO="$puerto"
    TEST_BASE_URL="http://127.0.0.1:$puerto/v1"
    _test_info "modelo '$TEST_MODEL' listo en :$puerto"

    # 3. API key (DESPUÉS del arranque: ver _test_api_key).
    key="$(_test_api_key)"

    # 4. Provider hermético de opencode en el directorio de la batería.
    cfg="$(_test_write_opencode_config "$dir" "$TEST_MODEL" "$puerto" "$key")"
    _test_info "config: $cfg (baseURL $TEST_BASE_URL)"

    # 5. Probe del engine: si no responde, esta batería aborta con un
    #    mensaje claro en vez de dejar que opencode falle en silencio.
    if ! _test_probe "$puerto" "$key"; then
        _test_error "el engine no responde en $TEST_BASE_URL/models"
        _test_record "$slug" "$dir" "$json" "$sesion" "$log" "$err" "el engine no respondió"
        return 1
    fi

    # 6. opencode run: stdout al JSON, stderr a su propio fichero.
    ( cd "$dir" && opencode run "$(cat "$md_file")" \
        --agent build --model "ropero-local/$TEST_MODEL" \
        --format json --standalone ) >"$json" 2>"$err" || rc=$?
    if (( rc != 0 )); then
        _test_warn "opencode run '$slug' terminó con rc=$rc (stderr en $err)"
    fi

    # 7. Errores reales del JSON (lo que antes se comía con 2>/dev/null).
    _test_errores_json "$json"

    # 8. Sesión exportada (session id en una variable normal: sin `session${i}`
    #    ni eval, que en bash no es una asignación válida).
    sid=""
    sid="$(_test_session_id "$json")" || sid=""
    if [[ -n "$sid" ]]; then
        _test_info "sesión: $sid"
        if ! ( cd "$dir" && opencode session export "$sid" --standalone ) >"$sesion" 2>>"$err"; then
            _test_warn "no se pudo exportar la sesión $sid (ver $err)"
        fi
    else
        _test_warn "no se encontró ningún session id en $json"
    fi

    # 9. Log del engine.
    if [[ -f "$TEST_LOG_FILE" ]]; then
        cp -f "$TEST_LOG_FILE" "$log" 2>/dev/null \
            || _test_warn "no se pudo copiar el log a $log"
    else
        _test_warn "no existe el log del engine $TEST_LOG_FILE"
    fi

    if [[ -s "$json" ]]; then
        _test_record "$slug" "$dir" "$json" "$sesion" "$log" "$err" "ok"
        return 0
    fi
    _test_record "$slug" "$dir" "$json" "$sesion" "$log" "$err" "opencode no devolvió JSON"
    return 1
}

# -----------------------------------------------------------------------------
# _test_generate_summary <archivo-salida> — fichero único con la RECOPILACIÓN
# COMPLETA: por cada batería, la ruta Y el contenido de sus artefactos (log
# del engine, JSON de opencode y sesión exportada).
# -----------------------------------------------------------------------------
_test_generate_summary() {
    local out="$1" modelo="${TEST_MODEL:-desconocido}" fecha
    local slug dir json sesion log err estado
    fecha="$(date '+%Y-%m-%d %H:%M:%S' 2>/dev/null)"
    {
        printf '# Recopilación de test: %s\n\n' "$modelo"
        printf -- '- Fecha: %s\n' "$fecha"
        printf -- '- Modelo: %s\n' "$modelo"
        printf -- '- Provider de opencode: ropero-local (openai-compatible, propio del test)\n'
        printf -- '- Endpoint: %s\n' "${TEST_BASE_URL:-(desconocido)}"
        printf -- '- Puerto: %s\n' "${TEST_PUERTO:-(desconocido)}"
        printf -- '- Baterías (markdown): %s\n' "$TEST_DOCS_DIR"
        printf -- '- Log base del engine: %s\n' "$TEST_LOG_FILE"
        printf -- '- Resultado: %s ok · %s fallo(s)\n' "${TEST_OK:-0}" "${TEST_FALLOS:-0}"
        printf -- '- API key: resuelta y usada sólo en el opencode.json de cada batería (no se imprime)\n'
        printf '\nCada batería deja en su directorio el log del engine, el JSON de opencode,\n'
        printf 'la sesión exportada y el stderr de opencode (nunca se descarta).\n'
        printf '\n'
    } >"$out"

    if [[ -z "$TEST_RESULTADOS" ]]; then
        {
            printf -- '---\n\n'
            printf '_(no se ejecutó ninguna batería)_\n'
        } >>"$out"
        return 0
    fi

    while IFS='|' read -r slug dir json sesion log err estado; do
        [[ -n "$slug" ]] || continue
        {
            printf -- '---\n\n'
            printf '## Batería %s — %s\n\n' "$slug" "$estado"
            printf -- '- Directorio: %s\n' "$dir"
            printf -- '- JSON de opencode: %s\n' "${json:-(no generado)}"
            printf -- '- Sesión exportada: %s\n' "${sesion:-(no generada)}"
            printf -- '- Log del engine: %s\n' "${log:-(no copiado)}"
            printf -- '- stderr de opencode: %s\n' "${err:-(no generado)}"
        } >>"$out"
        _test_anexar "$out" 'Log del engine' "${log:-}" 200
        _test_anexar "$out" 'JSON de opencode' "${json:-}" 400
        _test_anexar "$out" 'Sesión exportada' "${sesion:-}" 400
        _test_anexar "$out" 'stderr de opencode' "${err:-}" 200
    done <<EOF
$TEST_RESULTADOS
EOF
    return 0
}

# -----------------------------------------------------------------------------
# _test_run_all [modelo] — el runner: las CINCO baterías, una detrás de otra.
# Un fallo en una batería NO tira el resto: se anota y se sigue.
# Devuelve 0 si las cinco.ok, 1 si alguna falló, 2 si no se puede ni empezar.
# -----------------------------------------------------------------------------
_test_run_all() {
    local modelo="${1:-${TEST_MODEL:-}}"
    if [[ -z "$modelo" ]]; then
        _test_error "falta el modelo (usa _test_run_all <modelo>)"
        return 2
    fi
    TEST_MODEL="$modelo"
    [[ -n "${TEST_DOCS_DIR:-}" ]] || TEST_DOCS_DIR="$TEST_REPO_ROOT/docs"
    [[ -n "${TEST_LOG_FILE:-}" ]] || TEST_LOG_FILE="$HOME/.logs/ropero/$TEST_MODEL.log"
    [[ -n "${TEST_OUT_DIR:-}" ]] || TEST_OUT_DIR="$HOME/test/$TEST_MODEL"
    TEST_OPENCODE_HOME="${TEST_OPENCODE_HOME:-$TEST_OUT_DIR/.opencode}"

    TEST_OK=0
    TEST_FALLOS=0
    TEST_RESULTADOS=""
    TEST_PUERTO=""
    TEST_BASE_URL=""

    _test_info "modelo:    $TEST_MODEL"
    _test_info "baterías:  $TEST_DOCS_DIR"
    _test_info "salida:    $TEST_OUT_DIR"

    local falta=0
    for cmd in jq opencode ropero; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            _test_error "falta '$cmd' en el PATH"
            falta=1
        fi
    done
    (( falta == 0 )) || return 2

    if ! mkdir -p "$TEST_OUT_DIR"; then
        _test_error "no se pudo crear $TEST_OUT_DIR"
        return 2
    fi
    # HOME de opencode aislado: el test no lee la config global del usuario.
    rm -rf "$TEST_OPENCODE_HOME"
    if ! _test_opencode_env "$TEST_OPENCODE_HOME"; then
        _test_error "no se pudo preparar el entorno aislado de opencode"
        return 2
    fi

    local num sufijo
    while read -r num sufijo; do
        [[ -n "$num" ]] || continue
        if _test_run_prueba "$num" "$sufijo"; then
            TEST_OK=$((TEST_OK + 1))
        else
            TEST_FALLOS=$((TEST_FALLOS + 1))
        fi
    done <<EOF
$(_test_baterias)
EOF

    local resumen="$TEST_OUT_DIR/recopilacion-completa.md"
    _test_generate_summary "$resumen"
    _test_info "recopilación completa: $resumen"

    # El modelo que arrancamos se para al terminar.
    ropero stop all >/dev/null 2>&1 || true

    if (( TEST_FALLOS > 0 )); then
        _test_warn "$TEST_FALLOS batería(s) fallaron"
        return 1
    fi
    _test_info "las 5 baterías.ok"
    return 0
}
