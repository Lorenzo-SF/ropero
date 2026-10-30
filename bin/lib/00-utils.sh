#!/usr/bin/env bash
# =============================================================================
# 00-utils.sh — TODA la presentación de ropero 1.0 (spec §5.2)
# =============================================================================
# Contrato de este fichero:
#   * Presentación humana → SIEMPRE stderr. stdout queda limpio para la
#     salida máquina (líneas KEY=, dumps, argv): esa va con printf crudo y
#     NUNCA pasa por aquí (spec §5, convención general).
#   * Con alaja en PATH se usa (success|error|warning|info|notice|debug,
#     header, bar). Sin alaja: fallback printf ANSI equivalente (✓/✗/⚠/·).
#   * NO_COLOR o stderr sin TTY → plano, sin códigos ANSI. alaja aplica los
#     mismos criterios por su cuenta (se le redirige la salida a stderr).
#   * Seguro de sourcear: sólo definiciones y defaults, cero ejecución.
#   * bash 3.2: sin namerefs, sin arrays asociativos, sin mapfile (spec §3).
# =============================================================================

# Colores del fallback. ANSI-C quoting $'...' para que \033 sea ESC y no los
# cuatro caracteres literales (bug histórico del repo, verificado con hexdump).
RP_C_RESET=$'\033[0m'
RP_C_NEGRITA=$'\033[1m'
RP_C_VERDE=$'\033[32m'
RP_C_ROJO=$'\033[31m'
RP_C_AMARILLO=$'\033[33m'
RP_C_AZUL=$'\033[34m'
RP_C_CIAN=$'\033[36m'
RP_C_GRIS=$'\033[90m'

# Estado cacheado por rp_ui_init. No leer a mano: rp_ui_init (o el uso
# perezoso de rp_msg) lo mantiene fresco.
RP_UI_ALAJA=0   # 1 = alaja disponible en PATH
RP_UI_ANSI=0    # 1 = stderr es TTY y NO_COLOR sin fijar
RP_UI_INITED=0  # 1 = rp_ui_init ya ha corrido en este shell

# rp_ui_init — cachea la detección de presentación. Idempotente: releerla
# (p.ej. tras cambiar PATH) es seguro y barato.
rp_ui_init() {
    RP_UI_ALAJA=0
    RP_UI_ANSI=0
    if command -v alaja >/dev/null 2>&1; then
        RP_UI_ALAJA=1
    fi
    # La presentación va a stderr: es ESE destino el que decide si hay ANSI.
    if [[ -z "${NO_COLOR:-}" && -t 2 ]]; then
        RP_UI_ANSI=1
    fi
    RP_UI_INITED=1
}

# Inicialización perezosa: todo helper de presentación funciona aunque nadie
# haya llamado a rp_ui_init antes (tests, sources aislados).
_rp_ui_ensure() {
    if [[ "${RP_UI_INITED:-0}" != "1" ]]; then
        rp_ui_init
    fi
}

# rp_has_alaja — exit 0 si alaja está disponible.
rp_has_alaja() {
    _rp_ui_ensure
    [[ "$RP_UI_ALAJA" == "1" ]]
}

# Fallback sin alaja: línea "[<símbolo>] <texto>" con ✓/✗/⚠/· (spec §5.2).
# Con ANSI colorea sólo el símbolo; el texto queda siempre legible en plano.
_rp_msg_plain() {
    local tipo="$1" texto="$2" simbolo color
    case "$tipo" in
        ok)     simbolo="✓" color="$RP_C_VERDE" ;;
        error)  simbolo="✗" color="$RP_C_ROJO" ;;
        warn)   simbolo="⚠" color="$RP_C_AMARILLO" ;;
        info)   simbolo="·" color="$RP_C_CIAN" ;;
        notice) simbolo="·" color="$RP_C_AZUL" ;;
        debug)  simbolo="·" color="$RP_C_GRIS" ;;
        *)      simbolo="·" color="" ;;
    esac
    if [[ "${RP_UI_ANSI:-0}" == "1" && -n "$color" ]]; then
        printf '[%s%s%s] %s\n' "$color" "$simbolo" "$RP_C_RESET" "$texto" >&2
    else
        printf '[%s] %s\n' "$simbolo" "$texto" >&2
    fi
}

# rp_msg <tipo> <texto> — tipo ∈ ok|error|warn|info|notice|debug.
# La salida es para humanos: va a stderr, nunca a stdout.
rp_msg() {
    local tipo="${1:-}"
    shift
    local texto="$*"
    _rp_ui_ensure
    if [[ "$RP_UI_ALAJA" == "1" ]]; then
        # El "--" es obligatorio: sin él, un texto que empiece por "-" se lo
        # traga el parser de opciones de alaja como flag desconocido.
        case "$tipo" in
            ok)     alaja success -- "$texto" >&2 || _rp_msg_plain ok "$texto" ;;
            error)  alaja error   -- "$texto" >&2 || _rp_msg_plain error "$texto" ;;
            warn)   alaja warning -- "$texto" >&2 || _rp_msg_plain warn "$texto" ;;
            info)   alaja info    -- "$texto" >&2 || _rp_msg_plain info "$texto" ;;
            notice) alaja notice  -- "$texto" >&2 || _rp_msg_plain notice "$texto" ;;
            debug)  alaja debug   -- "$texto" >&2 || _rp_msg_plain debug "$texto" ;;
            *)      alaja info    -- "$texto" >&2 || _rp_msg_plain info "$texto" ;;
        esac
        return 0
    fi
    _rp_msg_plain "$tipo" "$texto"
}

# Fallback de cabecera: regla ═══ (40 columnas, el mismo ancho que alaja
# usa cuando no puede medir el terminal) y el título en negrita.
_rp_header_plain() {
    local texto="$1"
    printf '%s\n' "══════════════════════════════════════════"
    if [[ "${RP_UI_ANSI:-0}" == "1" ]]; then
        printf '%s%s%s\n' "$RP_C_NEGRITA" "$texto" "$RP_C_RESET" >&2
    else
        printf '%s\n' "$texto" >&2
    fi
}

# rp_header <texto> — cabecera de sección.
rp_header() {
    local texto="$*"
    _rp_ui_ensure
    if [[ "$RP_UI_ALAJA" == "1" ]]; then
        alaja header -- "$texto" >&2 || _rp_header_plain "$texto"
    else
        _rp_header_plain "$texto"
    fi
}

# rp_step <n> <total> <texto> — progreso de fase (instaladores).
rp_step() {
    local n="$1" total="$2"
    shift 2
    local texto="$*"
    _rp_ui_ensure
    if [[ "$RP_UI_ALAJA" == "1" ]]; then
        alaja bar "$n" --max "$total" --label "$texto" >&2 \
            || printf '[%s/%s] %s\n' "$n" "$total" "$texto" >&2
    else
        printf '[%s/%s] %s\n' "$n" "$total" "$texto" >&2
    fi
}

# rp_confirm <pregunta> — pregunta sí/no con read -r. Default NO: vacío o
# cualquier cosa que no sea afirmación se trata como "no" (lo seguro).
# RP_YES=1 la salta contestando sí (modo --yes / desatendido).
rp_confirm() {
    local pregunta="$1" ans=""
    if [[ "${RP_YES:-0}" == "1" ]]; then
        return 0
    fi
    printf '%s [s/N]: ' "$pregunta" >&2
    read -r ans || ans=""
    # Sin ${var,,} (bash 3.2): minúsculas por tr.
    ans="$(printf '%s' "$ans" | tr '[:upper:]' '[:lower:]')"
    case "$ans" in
        s|si|sí|y|yes) return 0 ;;
        *)             return 1 ;;
    esac
}

# rp_die <código> <msg> — mensaje de error y salida con <código>.
rp_die() {
    local codigo="$1"
    shift
    rp_msg error "$*"
    exit "$codigo"
}

# rp_usage_error <cmd> <msg> — error de uso (exit 2) con hint de ayuda.
# Con <cmd> vacío el hint apunta a la ayuda general.
rp_usage_error() {
    local cmd="${1:-}"
    shift
    rp_msg error "$*"
    if [[ -n "$cmd" ]]; then
        printf '  (ropero %s --help → uso de %s)\n' "$cmd" "$cmd" >&2
    else
        printf '  (ropero help → uso completo)\n' >&2
    fi
    exit 2
}

# rp_require_cmd <cmd> [hint] — exit 1 si falta un binario del PATH.
# El hint (opcional) dice cómo conseguirlo, p.ej. "brew install jq".
rp_require_cmd() {
    local cmd="$1"
    shift
    local hint="${1:-}"
    if command -v "$cmd" >/dev/null 2>&1; then
        return 0
    fi
    rp_msg error "falta '$cmd' en PATH"
    if [[ -n "$hint" ]]; then
        printf '  %s\n' "$hint" >&2
    fi
    exit 1
}

# rp_now — sello de tiempo para nombres de informes (valor por stdout).
rp_now() {
    date +%Y%m%d-%H%M%S
}
