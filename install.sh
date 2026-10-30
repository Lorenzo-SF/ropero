#!/usr/bin/env bash
# =============================================================================
# install.sh — instalador de una línea de ropero 1.0 (spec §9)
# =============================================================================
# USO:
#   curl -fsSL https://raw.githubusercontent.com/Lorenzo-SF/ropero/main/install.sh | bash
#   install.sh [--version <ref>] [--dir <ruta>] [--tarball <fichero>] [--help]
#
# SÓLO OBTIENE ROPERO. Nada de engines, modelos, opencode ni alaja: eso es
# `ropero install ...` DESPUÉS (requiere GPU, compilaciones y GB de disco, y
# un instalador que se come todo eso sin que nadie lo pide asusta a la
# gente). Por qué existe aparte: para instalar ropero hace falta no tener
# ropero; este script se apoya sólo en bash + curl/wget + tar (+ git como
# fallback) y corre en una máquina desnuda.
#
# Obtención: tarball de codeload.github.com (o --tarball local, para
# tests/offline); si la descarga falla, `git clone --depth 1` HTTPS. El token
# (GITHUB_TOKEN/GH_TOKEN, repos privados) viaja SIEMPRE en cabecera, nunca
# en la URL (no acaba en historiales ni logs).
#
# SEGURIDAD:
#   - sin sudo: todo va a --dir, $BIN_DIR y un tmp de descarga
#   - el tarball se inspecciona ANTES de extraerlo: rutas absolutas o con
#     ".." se rechazan (exit 1, no se extrae nada)
#   - sustitución atómica: se extrae a un staging JUNTO a --dir (mismo
#     filesystem) y se renombra; una instalación en curso nunca se rompe
#   - si --dir ya existe y NO es ropero, se niega: nunca pisa otra cosa
#   - el único código que se ejecuta tras instalar es
#     `bash <dir>/bin/ropero --help`
#
# Si python3 es < 3.8 (o no existe): AVISO claro apuntando a
# `ropero install setup`. Aquí no se instala nada más.
#
# Exit: 0 · 1 (fetch, extracción, validación) · 2 (uso).
#
# bash 3.2 (ADR-1); comentarios y mensajes en español (spec §3).
# =============================================================================

set -euo pipefail

# -----------------------------------------------------------------------------
# Bootstrap: el script puede llegar por stdin (curl … | bash), donde $0 es
# "bash" y no hay fichero legible. En ese caso se lee el script COMPLETO de
# stdin y se re-ejecuta con los mismos argumentos, para que toda la lógica
# corra con sus "$@" intactos. ROPERO_BOOTSTRAP es el cerrojo: sin él, la
# segunda pasada re-leería un stdin ya en EOF (port del install.sh viejo).
# -----------------------------------------------------------------------------
_rp_bootstrap() {
    [[ "${ROPERO_BOOTSTRAP:-0}" == "1" ]] && return 0

    local self="${BASH_SOURCE[0]:-$0}"
    local shell_name
    shell_name="$(basename "${BASH:-bash}")"
    if [[ "$self" != "$shell_name" && "$self" != "-" && -f "$self" && -r "$self" ]]; then
        return 0
    fi

    local script
    script="$(cat 2>/dev/null || true)"
    if [[ -z "${script//[[:space:]]/}" ]]; then
        printf 'install.sh: no se pudo leer el script desde stdin\n' >&2
        exit 1
    fi
    export ROPERO_BOOTSTRAP=1
    exec bash -c "$script" -- "$@"
}
_rp_bootstrap "$@"

# -----------------------------------------------------------------------------
# Configuración. Flag > variable de entorno > default.
# -----------------------------------------------------------------------------
REPO="Lorenzo-SF/ropero"
CODELOAD="https://codeload.github.com/${REPO}/tar.gz"
CLONE_URL="https://github.com/${REPO}.git"
DIR="${ROPERO_DIR:-${HOME:-}/.local/share/ropero}"
BIN_DIR="${ROPERO_BIN_DIR:-${HOME:-}/.local/bin}"
VERSION="main"
TARBALL=""
TOKEN="${GITHUB_TOKEN:-${GH_TOKEN:-}}"

TMP_DIR=""      # temporal de descarga (se borra siempre)
STAGE_DIR=""    # staging de extracción, JUNTO a --dir (se vuelve el install)

# -----------------------------------------------------------------------------
# Salida (a stderr: es presentación humana; el instalador no tiene salida
# máquina). Marcadores al estilo de ropero: [i] [✓] [!] [✗].
# -----------------------------------------------------------------------------
if [[ -t 2 && -z "${NO_COLOR:-}" ]]; then
    C_OK=$'\033[1;32m'; C_WARN=$'\033[1;33m'; C_ERR=$'\033[0;31m'
    C_HEAD=$'\033[1m'; C_OFF=$'\033[0m'
else
    C_OK=""; C_WARN=""; C_ERR=""; C_HEAD=""; C_OFF=""
fi

info() { printf '  %s[i]%s %s\n' "$C_HEAD" "$C_OFF" "$*" >&2; }
ok()   { printf '  %s[✓]%s %s\n' "$C_OK" "$C_OFF" "$*" >&2; }
warn() { printf '  %s[!]%s %s\n' "$C_WARN" "$C_OFF" "$*" >&2; }
err()  { printf '  %s[✗]%s %s\n' "$C_ERR" "$C_OFF" "$*" >&2; }
phase() { printf '\n%s▸ %s%s\n' "$C_HEAD" "$*" "$C_OFF" >&2; }

# die <msg>        — fallo real (fetch/extract/validación): exit 1.
# die_uso <msg>    — error de uso: exit 2 (spec §9).
die()     { err "$*"; exit 1; }
die_uso() { err "$*"; exit 2; }

have() { command -v "$1" >/dev/null 2>&1; }

# -----------------------------------------------------------------------------
# Limpieza: el temporal y el staging se borran en éxito, fallo y Ctrl-C. Si
# el staging YA se movió a --dir, STAGE_DIR está vacío y no hay nada que
# hacer (la instalación quedó atómica).
# -----------------------------------------------------------------------------
cleanup() {
    if [[ -n "$STAGE_DIR" && -d "$STAGE_DIR" ]]; then rm -rf "$STAGE_DIR" 2>/dev/null || true; fi
    if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then rm -rf "$TMP_DIR" 2>/dev/null || true; fi
    return 0
}
trap cleanup EXIT INT TERM

usage() {
    cat <<EOF
install.sh — instalador de ropero (SÓLO obtiene ropero)

USO:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | bash
  install.sh [--version <ref>] [--dir <ruta>] [--tarball <fichero>] [--help]

FLAGS:
  --version <ref>   ref de GitHub a instalar (default: main). Sin valor,
                    usa main
  --dir <ruta>      dónde instalar (default: \${ROPERO_DIR:-\$HOME/.local/share/ropero})
  --tarball <fich>  instala desde un tarball LOCAL (tests/offline)
  --help, -h        esta ayuda

VARIABLES DE ENTORNO:
  ROPERO_DIR        igual que --dir        ROPERO_BIN_DIR   (\$HOME/.local/bin)
  GITHUB_TOKEN / GH_TOKEN   sólo si el repositorio es privado (viaja en
                            cabecera, nunca en la URL)

QUÉ HACE:
  1. descarga el tarball de codeload (o usa --tarball); fallback git clone
  2. valida las rutas del tarball (nada de absolutas ni "..") y extrae a un
     staging junto a --dir
  3. sustituye --dir atómicamente (si ya era ropero: actualiza; si es otra
     cosa: se niega)
  4. crea \$BIN_DIR/ropero → <dir>/bin/ropero y verifica que responde

NO instala engines ni modelos: 'ropero install' (después) los gestiona.
Exit: 0 · 1 (fetch/extracción/validación) · 2 (uso).
EOF
}

# -----------------------------------------------------------------------------
# parse_args — los flags pisan al entorno. Desconocido → exit 2 (§9).
# --version acepta el valor OPCIONAL (sin él, main): así `install.sh
# --version` en una máquina sin red falla LIMPIO en el fetch (exit 1) en
# vez de morir por error de uso.
# -----------------------------------------------------------------------------
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --version)
                if [[ $# -ge 2 && "$2" != -* && -n "$2" ]]; then
                    VERSION="$2"; shift 2
                else
                    VERSION="main"; shift
                fi
                ;;
            --version=*)
                VERSION="${1#*=}"
                [[ -n "$VERSION" ]] || die_uso "--version necesita una referencia"
                shift ;;
            --dir)
                [[ $# -ge 2 ]] || die_uso "--dir necesita un directorio"
                DIR="$2"; shift 2 ;;
            --dir=*)
                DIR="${1#*=}"; shift ;;
            --tarball)
                [[ $# -ge 2 ]] || die_uso "--tarball necesita un fichero"
                TARBALL="$2"; shift 2 ;;
            --tarball=*)
                TARBALL="${1#*=}"; shift ;;
            -h|--help)
                usage; exit 0 ;;
            *)
                die_uso "argumento desconocido: '$1' (install.sh --help)" ;;
        esac
    done

    [[ -n "${HOME:-}" ]] || die "HOME no está definido: no hay dónde instalar"
    [[ -n "$DIR" ]] || die "--dir vacío"
    [[ -n "$VERSION" ]] || die "--version vacío"
    DIR="${DIR%/}"
    [[ -n "$DIR" ]] || DIR="/"
    if [[ -n "$TARBALL" && ! -f "$TARBALL" ]]; then
        die "el tarball local no existe: $TARBALL"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# _dl <url> <out> — descarga con reintentos. curl primero, wget de plan B.
# El token va en cabecera, nunca en la URL.
# -----------------------------------------------------------------------------
DL_TOOL=""
_dl() {
    local url="$1" out="$2"
    if [[ "$DL_TOOL" == "curl" ]]; then
        if [[ -n "$TOKEN" ]]; then
            curl -fsSL --retry 3 --retry-delay 2 --retry-connrefused \
                 -H "Authorization: Bearer $TOKEN" -o "$out" "$url"
        else
            curl -fsSL --retry 3 --retry-delay 2 --retry-connrefused -o "$out" "$url"
        fi
    else
        if [[ -n "$TOKEN" ]]; then
            wget -q --tries=3 --timeout=30 --header="Authorization: Bearer $TOKEN" -O "$out" "$url"
        else
            wget -q --tries=3 --timeout=30 -O "$out" "$url"
        fi
    fi
    return $?
}

# -----------------------------------------------------------------------------
# phase_check — herramientas imprescindibles. python3 sólo AVISA (aquí no
# se instala nada: eso es `ropero install setup`, §9).
# -----------------------------------------------------------------------------
phase_check() {
    phase "comprobar herramientas"

    have tar || die "falta 'tar', imprescindible para desempaquetar"
    ok "tar disponible"

    if [[ -n "$TARBALL" ]]; then
        # Con tarball local no hay descarga: curl/wget/git son opcionales.
        info "origen: tarball local $TARBALL (sin red)"
        return 0
    fi

    if have curl; then
        DL_TOOL="curl"
    elif have wget; then
        DL_TOOL="wget"
    else
        # git clone sigue siendo un plan B viable sin curl/wget.
        if have git; then
            warn "ni curl ni wget: se intentará git clone como fallback"
            DL_TOOL=""
            return 0
        fi
        die "falta 'curl' (o 'wget', o 'git'): no hay forma de obtener ropero"
    fi
    ok "descargador: ${DL_TOOL}"
    return 0
}

# -----------------------------------------------------------------------------
# phase_fetch — deja el origen en TMP_DIR/ropero.tar.gz (SRC_TAR) o decide
# clonar (SRC_CLONE=1). Error limpio exit 1 si no hay red.
# -----------------------------------------------------------------------------
SRC_TAR=""
SRC_CLONE=0

make_tmp() {
    local cache="${XDG_CACHE_HOME:-${HOME:-}/.cache}/ropero"
    if mkdir -p "$cache" 2>/dev/null; then
        TMP_DIR="$(mktemp -d "$cache/install.XXXXXX" 2>/dev/null || true)"
    fi
    if [[ -z "${TMP_DIR:-}" ]]; then
        TMP_DIR="$(mktemp -d 2>/dev/null || true)"
    fi
    [[ -n "${TMP_DIR:-}" && -d "${TMP_DIR:-}" ]] || die "no se pudo crear el directorio temporal"
    return 0
}

phase_fetch() {
    phase "obtener ropero (ref: $VERSION)"

    if [[ -n "$TARBALL" ]]; then
        SRC_TAR="$TARBALL"
        return 0
    fi

    make_tmp
    local out="$TMP_DIR/ropero.tar.gz"

    if [[ -n "$DL_TOOL" ]]; then
        info "descargando $CODELOAD/$VERSION"
        # Sin ruido: el error de red se explica aquí, con contexto, y no con
        # el volcado de curl.
        if _dl "$CODELOAD/$VERSION" "$out" 2>/dev/null && [[ -s "$out" ]]; then
            tar -tzf "$out" >/dev/null 2>&1 || die "el tarball descargado está corrupto"
            ok "descargado ($(wc -c <"$out" | tr -d ' ') bytes)"
            SRC_TAR="$out"
            return 0
        fi
        rm -f "$out" 2>/dev/null || true
        warn "la descarga del tarball falló (¿sin red? ¿ref '$VERSION' inexistente?)"
    fi

    # Fallback: clone poco profundo por HTTPS (token en cabecera, §9).
    if have git; then
        info "fallback: git clone --depth 1 $CLONE_URL"
        local clone="$TMP_DIR/ropero-clone"
        if [[ -n "$TOKEN" ]]; then
            git -c "http.extraheader=Authorization: Bearer $TOKEN" \
                clone --depth 1 "$CLONE_URL" "$clone" >/dev/null 2>&1 \
                || die "no se pudo descargar '$VERSION' (tarball y clone fallaron)"
        else
            git clone --depth 1 "$CLONE_URL" "$clone" >/dev/null 2>&1 \
                || die "no se pudo descargar '$VERSION' (tarball y clone fallaron)"
        fi
        if [[ -z "$TOKEN" ]]; then
            :
        fi
        ok "clonado"
        SRC_CLONE=1
        SRC_TAR="$clone"   # el clone es el staging en sí (ver phase_extract)
        return 0
    fi

    die "no se pudo obtener ropero (sin curl/wget no queda fallback de clone: falta git)"
}

# -----------------------------------------------------------------------------
# phase_extract — valida las rutas del tarball, extrae a un staging JUNTO a
# --dir y deja el árbol listo. Con origen=clone, el staging ES el clone.
# -----------------------------------------------------------------------------
phase_extract() {
    phase "extraer"

    local padre
    padre="$(dirname "$DIR")"
    mkdir -p "$padre" 2>/dev/null || die "no se pudo crear $padre"

    if [[ "$SRC_CLONE" == "1" ]]; then
        STAGE_DIR="$SRC_TAR"
        # El .git del clone no debe acabar en la instalación.
        rm -rf "$STAGE_DIR/.git" 2>/dev/null || true
        [[ -f "$STAGE_DIR/bin/ropero" ]] || die "el clone no contiene bin/ropero"
        ok "staging (clone): $STAGE_DIR"
        return 0
    fi

    # ── Seguridad del tarball: nada de rutas absolutas ni ".." que salgan
    # del destino. Se revisa el listado ANTES de extraer nada.
    local entradas=""
    entradas="$(tar -tzf "$SRC_TAR" 2>/dev/null)" || die "no se pudo leer el tarball"
    [[ -n "$entradas" ]] || die "el tarball está vacío"

    local e
    while IFS= read -r e; do
        [[ -n "$e" ]] || continue
        case "$e" in
            /*)               die "el tarball contiene una ruta absoluta ($e): no se extrae" ;;
            ../*|*/../*|*/..) die "el tarball contiene una ruta fuera del destino ($e): no se extrae" ;;
        esac
    done <<<"$entradas"

    # --strip-components sólo si TODAS las entradas comparten el mismo
    # directorio raíz (los tarballs de GitHub siempre: ropero-main/...).
    local strip=0 tops ntops
    tops="$(printf '%s\n' "$entradas" | awk -F/ 'NF>1 {print $1}' | sort -u)"
    ntops="$(printf '%s\n' "$tops" | grep -c . || true)"
    [[ "$ntops" == "1" ]] && strip=1

    STAGE_DIR="$(mktemp -d "$padre/.rp-stage.XXXXXX" 2>/dev/null)" \
        || die "no se pudo crear el staging junto a $DIR"

    local -a tar_flags=(-xzf "$SRC_TAR" -C "$STAGE_DIR")
    # --no-same-owner sólo existe en GNU tar; bsdtar no lo necesita (no se
    # extrae nada como root).
    if tar --version 2>/dev/null | head -n 1 | grep -q "GNU tar"; then
        tar_flags+=(--no-same-owner)
    fi
    (( strip == 1 )) && tar_flags+=(--strip-components=1)

    tar "${tar_flags[@]}" || die "falló la extracción del tarball"

    # Fallo rápido: sin lanzador no hay instalación.
    [[ -f "$STAGE_DIR/bin/ropero" ]] || die "el tarball no contiene bin/ropero"
    chmod +x "$STAGE_DIR/bin/ropero" 2>/dev/null || true
    ok "extraído en staging: $STAGE_DIR"
    return 0
}

# -----------------------------------------------------------------------------
# phase_install — sustitución atómica de --dir.
# -----------------------------------------------------------------------------
phase_install() {
    phase "instalar en $DIR"

    if [[ -e "$DIR" ]]; then
        if [[ ! -f "$DIR/bin/ropero" ]]; then
            # Nunca se pisa lo que no es ropero (§9): un --dir apuntando al
            # home del usuario o a otro programa es error CLARO, no rm -rf.
            die "$DIR ya existe y NO es una instalación de ropero: no lo toco"
        fi
        # Ya era ropero: actualizar = apartar, poner el nuevo, borrar el
        # viejo. El symlink de $BIN_DIR nunca queda colgando más de un rename.
        local old="$DIR.old.$$"
        mv "$DIR" "$old" || die "no se pudo apartar $DIR"
        if ! mv "$STAGE_DIR" "$DIR"; then
            mv "$old" "$DIR" 2>/dev/null || true
            die "no se pudo instalar en $DIR (se restauró la instalación anterior)"
        fi
        STAGE_DIR=""
        rm -rf "$old" 2>/dev/null || true
        ok "actualizado: $DIR (sustitución atómica)"
    else
        mv "$STAGE_DIR" "$DIR" || die "no se pudo mover el staging a $DIR"
        STAGE_DIR=""
        ok "instalado: $DIR"
    fi
    # mktemp -d deja el staging en 700: la instalación debe tener permisos
    # normales, como si se hubiera descomprimido a mano.
    chmod 755 "$DIR" 2>/dev/null || true
    return 0
}

# -----------------------------------------------------------------------------
# phase_link — symlink del lanzador en $BIN_DIR.
# -----------------------------------------------------------------------------
LINK_SKIPPED=0
phase_link() {
    phase "enlazar"

    mkdir -p "$BIN_DIR" || die "no se pudo crear $BIN_DIR"

    if [[ -e "$BIN_DIR/ropero" && ! -L "$BIN_DIR/ropero" ]]; then
        # Un fichero REAL ahí (otro programa, un binario copiado a mano): no
        # se pisa en silencio. El usuario decide.
        warn "$BIN_DIR/ropero existe y NO es un symlink: no lo he tocado"
        LINK_SKIPPED=1
        return 0
    fi

    ln -sfn "$DIR/bin/ropero" "$BIN_DIR/ropero" || die "no se pudo crear el symlink $BIN_DIR/ropero"
    ok "$BIN_DIR/ropero → $DIR/bin/ropero"

    # ¿Está $BIN_DIR en el PATH? Se avisa; el PATH del usuario es suyo.
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *)
            warn "$BIN_DIR no está en tu PATH"
            printf '       export PATH="%s:$PATH"\n' "$BIN_DIR" >&2
            ;;
    esac
    return 0
}

# -----------------------------------------------------------------------------
# phase_verify — el único código que se ejecuta tras instalar: --help.
# -----------------------------------------------------------------------------
phase_verify() {
    phase "verificar"

    local bin="$DIR/bin/ropero"
    [[ -x "$bin" ]] || die "el lanzador no es ejecutable: $bin"

    local rc
    set +e
    bash "$bin" --help >/dev/null 2>&1 </dev/null
    rc=$?
    set -e
    if (( rc == 0 )); then
        ok "el lanzador responde (ropero --help → salida 0)"
    else
        die "ropero --help devolvió $rc: la instalación no funciona (prueba otra --version)"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# phase_python — aviso (y SÓLO aviso) si python3 < 3.8 (§9).
# -----------------------------------------------------------------------------
phase_python() {
    local pyv=""
    if have python3; then
        # Un shim (asdf/pyenv) puede responder 0 y no ejecutar python: se
        # parsea la salida, no el exit code.
        pyv="$(python3 -c 'import sys; print("%d.%d" % sys.version_info[:2])' 2>/dev/null | tail -n 1 || true)"
    fi
    if [[ "$pyv" =~ ^[0-9]+\.[0-9]+$ ]]; then
        local pymaj="${pyv%%.*}" pymin="${pyv##*.}"
        if (( pymaj > 3 || (pymaj == 3 && pymin >= 8) )); then
            ok "python3 $pyv"
            return 0
        fi
        warn "python3 $pyv es demasiado antiguo (ropero pide >= 3.8)"
    else
        warn "no se encuentra python3"
    fi
    warn "ropero se ha instalado igualmente: el lanzador funciona sin él."
    warn "Los modelos se configuran en TOML y se leen con python3 (>= 3.8):"
    warn "'ropero install models' fallará hasta que lo resuelvas con:"
    warn "    ropero install setup"
    return 0
}

# -----------------------------------------------------------------------------
# main
# -----------------------------------------------------------------------------
main() {
    parse_args "$@"
    phase_check
    phase_fetch
    phase_extract
    phase_install
    phase_link
    phase_verify
    phase_python

    printf '\n%s══ ropero instalado ══%s\n' "$C_HEAD" "$C_OFF" >&2
    printf '  versión:   %s\n' "$VERSION" >&2
    printf '  instalado: %s\n' "$DIR" >&2
    if [[ "$LINK_SKIPPED" == "1" ]]; then
        printf '  symlink:   NO creado (había algo en %s/ropero que no era un symlink)\n' "$BIN_DIR" >&2
    else
        printf '  symlink:   %s/ropero → %s/bin/ropero\n' "$BIN_DIR" "$DIR" >&2
    fi
    printf '\n  Siguientes pasos:\n' >&2
    printf '    ropero install setup     # dependencias base\n' >&2
    printf '    ropero install engines   # engines compatibles\n' >&2
    printf '    ropero install models    # baja o fabrica modelos\n' >&2
    printf '    ropero list              # catálogo\n' >&2
    printf '\n  Para actualizar:\n' >&2
    printf '    curl -fsSL https://raw.githubusercontent.com/%s/main/install.sh | bash\n' "$REPO" >&2
    return 0
}

main "$@"
