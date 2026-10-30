#!/usr/bin/env bash
# =============================================================================
# bin/lib/cmd-test.sh — `ropero test <modelo>` (spec §8.14)
#
# Prueba UN modelo contra las baterías de tests/prueba*.md usando opencode,
# en un directorio temporal, y deja en el directorio desde el que se lanzó un
# único markdown AUTOCONTENIDO (juez + prueba + log del engine + JSON de
# opencode + sesión exportada) listo para pasárselo a otra IA.
#
# AVISO: por diseño para el entorno — ejecuta `ropero stop` y pkill -9 de
# llama-server/opencode/mcode/claude tras confirmación explícita.
# =============================================================================

if ! declare -F rp_cmd_start >/dev/null 2>&1; then
    . "${ROPERO_ROOT}/bin/lib/cmd-runtime.sh"
fi

_rp_test_pruebas() {
    printf '%s\n' prueba1 prueba2 prueba3 prueba_larga prueba_gorda
}

_rp_test_help() {
    cat <<'HELP'
USO: ropero test <modelo> [opciones]

  --prueba NOMBRE   batería a ejecutar (prueba1|prueba2|prueba3|prueba_larga|
                    prueba_gorda). Default: prueba1
  --todas           ejecuta las cinco en secuencia (un .md por prueba)
  --yes             no pide confirmación
  --timeout SEG     timeout de arranque/espera del modelo (default 300)
  --port N          puerto donde arrancar el modelo
  --nivel N         nivel de razonamiento del modelo
  -h, --help        esta ayuda

AVISO: el test ejecuta `ropero stop` y `pkill -9` de llama-server, opencode,
mcode y claude. No lances esto con trabajo abierto en esos agentes.
HELP
}

# _rp_test_matar_agentes — deja el entorno libre antes de medir.
_rp_test_matar_agentes() {
    rp_cmd_stop all >/dev/null 2>&1 || true
    local p
    for p in llama-server opencode mcode claude; do
        pkill -9 -x "$p" 2>/dev/null || true
    done
}

# _rp_test_sesion_id <prueba.json> — imprime el primer session id o nada.
_rp_test_sesion_id() {
    local f="$1" id=""
    id="$(jq -r 'select(.sessionID or .session_id) | (.sessionID // .session_id)' "$f" 2>/dev/null \
        | sort -u | sed '/^$/d' | head -1)"
    printf '%s' "$id"
}

# _rp_test_informe <modelo> <prueba> <juez> <prompt> <log> <json> <sesion> <destino>
_rp_test_informe() {
    local modelo="$1" prueba="$2" juez="$3" prompt="$4" log="$5" pjson="$6" sesion="$7" destino="$8"
    {
        printf '# ropero test — %s · %s\n\n' "$modelo" "$prueba"
        printf 'Generado: %s\n\n' "$(date '+%Y-%m-%d %H:%M:%S')"
        printf '> Archivo autocontenido: prompt del juez, prompt de la prueba,\n'
        printf '> log del engine, salida JSON de opencode y sesión exportada.\n\n'
        printf '## Juez\n\n````text\n'
        cat "$juez" 2>/dev/null || printf '(no se encontró el juez)\n'
        printf '\n````\n\n## Prueba\n\n````text\n'
        cat "$prompt" 2>/dev/null || printf '(no se encontró la prueba)\n'
        printf '\n````\n\n## Log del engine\n\n````text\n'
        cat "$log" 2>/dev/null || printf '(sin log)\n'
        printf '\n````\n\n## Salida de opencode (JSON)\n\n````json\n'
        cat "$pjson" 2>/dev/null || printf '(sin salida)\n'
        printf '\n````\n\n## Sesión exportada\n\n````json\n'
        cat "$sesion" 2>/dev/null || printf '(sin sesión)\n'
        printf '\n````\n'
    } > "$destino"
}

# _rp_test_una <modelo> <prueba> <origen_pwd> <tmp> <puerto_de_arranque>
_rp_test_una() {
    local modelo="$1" prueba="$2" origen="$3" tmp="$4" puerto="$5"
    local prompt="$ROPERO_ROOT/tests/$prueba.md"
    local juez="$ROPERO_ROOT/tests/juez.md"
    local pjson="$tmp/$prueba.json" sesion="$tmp/$prueba.session.json"
    local log="$LOG_DIR/$modelo.log"
    local ts destino

    if [[ ! -f "$prompt" ]]; then
        rp_msg error "no existe la prueba: $prompt"
        return 1
    fi

    rp_header "prueba: $prueba"
    if ! ( cd "$tmp" && opencode run "$(cat "$prompt")" --format json > "$pjson" ); then
        rp_msg error "opencode falló en la prueba '$prueba'"
        return 1
    fi
    local sid
    sid="$(_rp_test_sesion_id "$pjson")"
    if [[ -z "$sid" ]]; then
        rp_msg error "no se pudo extraer el session id de $pjson"
        return 1
    fi
    if ! ( cd "$tmp" && opencode session export "$sid" > "$sesion" ); then
        rp_msg warn "no se pudo exportar la sesión $sid; el informe seguirá sin ella"
        : > "$sesion"
    fi

    ts="$(rp_now)"
    destino="$origen/ropero-test-${modelo}-${prueba}-${ts}.md"
    _rp_test_informe "$modelo" "$prueba" "$juez" "$prompt" "$log" "$pjson" "$sesion" "$destino"
    rp_msg ok "informe: $destino"
    return 0
}

rp_cmd_test() {
    local a
    for a in "$@"; do
        [[ "$a" == "--help" || "$a" == "-h" ]] && { _rp_test_help; return 0; }
    done

    local modelo="" prueba="" todas=0 yes=0 timeout="${ROPERO_START_TIMEOUT:-300}"
    local puerto="" nivel=""
    while (( $# > 0 )); do
        case "$1" in
            --prueba) [[ $# -ge 2 ]] || rp_usage_error test "--prueba requiere un nombre"; prueba="$2"; shift 2 ;;
            --prueba=*) prueba="${1#--prueba=}"; shift ;;
            --todas) todas=1; shift ;;
            --yes|-y) yes=1; shift ;;
            --timeout) [[ $# -ge 2 ]] || rp_usage_error test "--timeout requiere segundos"; timeout="$2"; shift 2 ;;
            --timeout=*) timeout="${1#--timeout=}"; shift ;;
            --port) [[ $# -ge 2 ]] || rp_usage_error test "--port requiere un número"; puerto="$2"; shift 2 ;;
            --port=*) puerto="${1#--port=}"; shift ;;
            --nivel) [[ $# -ge 2 ]] || rp_usage_error test "--nivel requiere un nivel"; nivel="$2"; shift 2 ;;
            --nivel=*) nivel="${1#--nivel=}"; shift ;;
            --?*) rp_usage_error test "opción desconocida: '$1'" ;;
            *)
                [[ -z "$modelo" ]] || rp_usage_error test "sólo se prueba UN modelo por invocación (recibido también '$1')"
                modelo="$1"; shift ;;
        esac
    done
    [[ -n "$modelo" ]] || rp_usage_error test "falta el modelo: ropero test <modelo>"

    rp_require_cmd opencode "ropero test necesita el CLI de opencode en PATH"
    rp_require_cmd jq "ropero test necesita jq en PATH"

    local -a pruebas=()
    if (( todas == 1 )); then
        [[ -z "$prueba" ]] || rp_usage_error test "--todas y --prueba son excluyentes"
        while IFS= read -r a; do pruebas+=("$a"); done < <(_rp_test_pruebas)
    else
        pruebas=("${prueba:-prueba1}")
    fi
    local p
    for p in ${pruebas[@]+"${pruebas[@]}"}; do
        [[ -f "$ROPERO_ROOT/tests/$p.md" ]] || rp_usage_error test "prueba desconocida: '$p'"
    done

    if (( yes == 0 )); then
        rp_msg warn "ropero test va a ejecutar 'ropero stop' y pkill -9 de:"
        rp_msg warn "  llama-server · opencode · mcode · claude"
        rp_msg warn "Cierra cualquier trabajo abierto en esos agentes antes de continuar."
        rp_confirm "¿Probar '$modelo' con ${#pruebas[@]} prueba(s)?" || {
            rp_msg info "cancelado"
            return 1
        }
    fi

    local origen="$PWD"
    _RP_TEST_TMP=
    local tmp
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/ropero-test.XXXXXX")" || {
        rp_msg error "no se pudo crear el directorio temporal"
        return 1
    }
    _RP_TEST_TMP="$tmp"
    _RP_TEST_LOG_DIR_OLD="$LOG_DIR"
    # El log del engine tiene que quedar en el tmp: es material del informe.
    LOG_DIR="$tmp"
    # shellcheck disable=SC2064
    # shellcheck disable=SC2064
    trap 'rp_cmd_stop all >/dev/null 2>&1 || true; LOG_DIR="${_RP_TEST_LOG_DIR_OLD:-$LOG_DIR}"; rm -rf "${_RP_TEST_TMP:-}"' EXIT INT TERM

    rp_header "ropero test · $modelo"
    rp_msg info "deteniendo entorno (stop + pkill de agentes)..."
    _rp_test_matar_agentes

    local port_args=()
    [[ -n "$puerto" ]] && port_args=(--port "$puerto")
    local nivel_args=()
    [[ -n "$nivel" ]] && nivel_args=(--nivel "$nivel")

    rp_msg info "arrancando '$modelo' (log: $tmp/$modelo.log)"
    if ! ROPERO_START_TIMEOUT="$timeout" rp_cmd_start "$modelo" ${port_args[@]+"${port_args[@]}"} ${nivel_args[@]+"${nivel_args[@]}"} --background; then
        rp_msg error "no se pudo arrancar '$modelo'"
        tail -n 20 "$tmp/$modelo.log" 2>/dev/null | sed 's/^/    /' >&2
        return 1
    fi

    local ok=0 fallos=0 rc=0
    for p in ${pruebas[@]+"${pruebas[@]}"}; do
        if _rp_test_una "$modelo" "$p" "$origen" "$tmp" "$puerto"; then
            ok=$((ok + 1))
        else
            fallos=$((fallos + 1))
        fi
    done

    rp_header "resultado"
    rp_msg ok "$ok ok · $fallos fallo(s)"
    (( fallos == 0 )) || rc=1
    return "$rc"
}
