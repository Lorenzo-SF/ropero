#!/usr/bin/env bash
# =============================================================================
# install.sh — instalador de ropero (contrato de instalación lasaca)
# =============================================================================
#
# LAYOUT CANÓNICO (idéntico en los 8 repos de lasaca)
#
#   <repo>/                    fuente de verdad (en modo --local: el repo ya
#                              clonado; NO se mueve, NO se copia)
#     bin/ropero               entrypoint de la utilidad
#     bin/lib/                 directorio con los módulos (.sh) que se sourcean
#
#   ~/.local/share/ropero/     "farm": directorio REAL lleno de symlinks
#     <entrada de 1er nivel>   → <repo>/<entrada>   (una por elemento)
#     ropero.d                 → <repo>/bin/lib      (alias de módulos)
#
#   ~/.local/bin/ropero        → <repo>/bin/ropero   (SÓLO esto)
#
# REGLAS DURAS
#   R1  en ~/.local/bin queda SÓLO el entrypoint: ni <name>.d, ni módulos
#       sueltos, ni common.sh, ni binarios auxiliares.
#   R2  los ejecutables auxiliares de la utilidad viven en el farm.
#   R3  el repo NO se mueve, NO se copia y NO pierde su .git. install.sh sólo
#       crea symlinks (modo local) o descarga un árbol aparte (modo red).
#   R4  si ~/.local/bin/<name> es un fichero REAL, se avisa y NO se toca: la
#       decisión es del usuario.
#   R5  nunca sudo, nunca compila, nunca descarga modelos, nunca arranca
#       motores. Eso es `ropero install engines|models`.
#   R6  todo el shell es bash 3.2 (macOS): sin readlink -f, sin mapfile,
#       sin ${var^^}.
#
# USO
#   install.sh [--install] [--local] [--dir <ruta>] [--bin-dir <ruta>]
#              [--version <ref>] [--tarball <fichero>] [--check] [--help]
#
#   (sin flags)   instala (modo red)
#   --install     instala
#   --local       instala usando el repo donde vive este script como origen.
#                 Es el modo que usa `lasaca.sh --install`.
#   --check       verifica SIN instalar ni modificar nada. Exit 0/1.
#   --help/-h     ayuda. Exit 0.
#
# Env: ROPERO_DIR (def ~/.local/share/ropero), ROPERO_BIN_DIR (def ~/.local/bin)
# Exit: 0 ok · 1 fallo real · 2 uso.
#
# NOTA DE LECTURA DEL CONTRATO (modo red): en red no hay repo del usuario,
# así que el árbol descargado se instala en $DIR y ÉSE es el farm (sus
# entradas son ficheros reales, no symlinks: un symlink a sí mismo sería un
# bucle). Lo que sí se mantiene en red es el alias <name>.d → <MODULES> y
# el symlink del entrypoint en ~/.local/bin. En modo --local (el que usa
# lasaca.sh) el farm es lo que dice el contrato: symlinks al repo.
#
# MIGRACIÓN: hoy ~/.local/share/ropero/ puede ser una COPIA real de una
# instalación vieja (no un farm). Si $DIR contiene <ENTRY> pero NO es un
# symlink, se migra: se aparta a un lado, se monta el farm y se borra la
# copia. Si $DIR contiene otra cosa, se niega (exit 1): nunca se pisa.
#
# bootstrap: el script puede llegar por stdin (curl … | bash), donde $0 es
# "bash" y no hay fichero legible. En ese caso se lee el script COMPLETO de
# stdin y se re-ejecuta con los mismos argumentos. ROPERO_BOOTSTRAP es el
# cerrojo: sin él, la segunda pasada re-leería un stdin ya en EOF.
# =============================================================================

# -----------------------------------------------------------------------------
# Opciones de shell: SÓLO `pipefail`.
#
# SIN `errexit` (-e) NI `nounset` (-u): un fallo implícito (un comando que no
# encuentra lo que busca, una tubería cuyo primer tramo falla) NO aborta el
# instalador. Los abortos son EXPLÍCITOS y locales (`|| die "..."`) en cada
# ruta crítica: con `errexit` la instalación se paraba a medias, sin mensaje
# (un symlink sin crear, un farm sin montar).
#
# install.sh SE EJECUTA (típicamente `curl … | bash`), no se sourcea: su
# `exit` —el de `main`— termina la shell de quien lo sourcee.
#
# NO reintroducir errexit ni nounset: rompe ese contrato.
# -----------------------------------------------------------------------------
set -o pipefail

# -----------------------------------------------------------------------------
# Bootstrap: stdin → fichero legible, conservando "$@".
# -----------------------------------------------------------------------------
_ropero_bootstrap() {
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
    # Un `exec` con éxito NO vuelve: sólo vuelve si la segunda pasada no ha
    # podido arrancar. Sin esto, el script seguiría a `main` sin haber leído
    # el script de stdin: instalaría a medias, sin saberlo.
    exec bash -c "$script" -- "$@" || {
        printf 'install.sh: no se pudo re-ejecutar el script desde stdin\n' >&2
        exit 1
    }
}
_ropero_bootstrap "$@" || exit 1

# -----------------------------------------------------------------------------
# Parámetros del repo. Flag > variable de entorno > default.
# -----------------------------------------------------------------------------
NAME="ropero"
REPO="Lorenzo-SF/ropero"
ENTRY="bin/ropero"          # entrypoint de la utilidad
MODULES="bin/lib"           # directorio de módulos (.sh) que se sourcean
CODELOAD="https://codeload.github.com/${REPO}/tar.gz"
CLONE_URL="https://github.com/${REPO}.git"

DIR="${ROPERO_DIR:-${HOME:-}/.local/share/ropero}"
BIN_DIR="${ROPERO_BIN_DIR:-${HOME:-}/.local/bin}"
VERSION="main"
TARBALL=""
LOCAL=0
CHECK=0
TOKEN="${GITHUB_TOKEN:-${GH_TOKEN:-}}"

ORIGIN=""        # raíz del árbol de verdad (repo local o árbol descargado)
TMP_DIR=""       # temporal de descarga (se borra siempre)
STAGE_DIR=""     # staging de extracción, JUNTO a $DIR (mismo filesystem)
LINK_SKIPPED=0  # 1 si ~/.local/bin/<name> era un fichero real (R4)

# -----------------------------------------------------------------------------
# Salida: a stderr (presentación humana). Marcadores [i] [✓] [!] [✗].
# Color sólo si stderr es tty y NO_COLOR no está.
# -----------------------------------------------------------------------------
if [[ -t 2 && -z "${NO_COLOR:-}" ]]; then
    C_OK=$'\033[1;32m'; C_WARN=$'\033[1;33m'; C_ERR=$'\033[1;31m'
    C_HEAD=$'\033[1m'; C_OFF=$'\033[0m'
else
    C_OK=""; C_WARN=""; C_ERR=""; C_HEAD=""; C_OFF=""
fi

N_OK=0; N_WARN=0; N_FAIL=0

info() { printf '  %s[i]%s %s\n' "$C_HEAD" "$C_OFF" "$*" >&2; }
ok()   { printf '  %s[✓]%s %s\n' "$C_OK" "$C_OFF" "$*" >&2; N_OK=$((N_OK+1)); }
warn() { printf '  %s[!]%s %s\n' "$C_WARN" "$C_OFF" "$*" >&2; N_WARN=$((N_WARN+1)); }
err()  { printf '  %s[✗]%s %s\n' "$C_ERR" "$C_OFF" "$*" >&2; N_FAIL=$((N_FAIL+1)); }
phase() { printf '\n%s▸ %s%s\n' "$C_HEAD" "$*" "$C_OFF" >&2; }

resumen() {
    printf '\n── Resumen ──\n' >&2
    printf '  %s ok · %s avisos · %s fallos\n' "$N_OK" "$N_WARN" "$N_FAIL" >&2
}

# die <msg> — fallo real: exit 1.
# die_uso <msg> — error de uso: exit 2.
die()     { err "$*"; resumen; exit 1; }
die_uso() { err "$*"; resumen; exit 2; }

have() { command -v "$1" >/dev/null 2>&1; }

# -----------------------------------------------------------------------------
# Limpieza: el temporal y el staging se borran en éxito, fallo y Ctrl-C. Si
# el staging YA se movió a $DIR, STAGE_DIR está vacío (la instalación quedó
# atómica).
# -----------------------------------------------------------------------------
cleanup() {
    if [[ -n "$STAGE_DIR" && -d "$STAGE_DIR" ]]; then rm -rf "$STAGE_DIR" 2>/dev/null || true; fi
    if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then rm -rf "$TMP_DIR" 2>/dev/null || true; fi
    return 0
}
trap cleanup EXIT INT TERM

# -----------------------------------------------------------------------------
# Utilidades de ruta (bash 3.2 / macOS: nada de readlink -f).
# -----------------------------------------------------------------------------

# real_dir <ruta> — directorio real (siguiendo symlinks) de una ruta.
real_dir() {
    local p="$1" dir base
    while [[ -L "$p" ]]; do
        dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
        base="$(readlink "$p")" || return 1
        [[ "$base" != /* ]] && base="$dir/$base"
        p="$base"
    done
    dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
    printf '%s\n' "$dir"
}

# real_path <ruta> — ruta absoluta real (siguiendo symlinks), sin readlink -f.
real_path() {
    local p="$1" dir base
    while [[ -L "$p" ]]; do
        dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
        base="$(readlink "$p")" || return 1
        [[ "$base" != /* ]] && base="$dir/$base"
        p="$base"
    done
    dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
    printf '%s\n' "$dir/$(basename "$p")"
}

# same_path <a> <b> — ¿a y b apuntan al mismo sitio?
same_path() {
    local a="$1" b="$2" ra rb
    [[ -e "$a" && -e "$b" ]] || return 1
    ra="$(real_path "$a")" || return 1
    rb="$(real_path "$b")" || return 1
    [[ "$ra" == "$rb" ]]
}

# entradas <dir> — una entrada de primer nivel por línea (sin .git).
entradas() {
    local d="$1" e base
    for e in "$d"/* "$d"/.[!.]*; do
        [[ -e "$e" || -L "$e" ]] || continue
        base="${e##*/}"
        [[ "$base" == ".git" ]] && continue
        printf '%s\n' "$base"
    done
    return 0
}

# -----------------------------------------------------------------------------
# Uso
# -----------------------------------------------------------------------------
usage() {
    cat <<EOF
install.sh — instalador de ropero (contrato lasaca)

USO:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | bash
  install.sh [--install] [--local] [--dir <ruta>] [--bin-dir <ruta>]
             [--version <ref>] [--tarball <fichero>] [--check] [--help]

FLAGS:
  --install          instala (es lo que pasa si no se da ningún flag)
  --local            instala usando el repo donde vive este script como
                     origen. Es el modo que usa 'lasaca.sh --install'
  --dir <ruta>       dónde vive el farm (default: \${ROPERO_DIR:-\$HOME/.local/share/ropero})
  --bin-dir <ruta>   dónde vive el symlink del entrypoint
                     (default: \${ROPERO_BIN_DIR:-\$HOME/.local/bin})
  --version <ref>    ref de GitHub a instalar (default: main). Sólo modo red
  --tarball <fich>   instala desde un tarball LOCAL (tests/offline)
  --check            verifica SIN instalar ni modificar nada. Exit 0/1
  --help, -h         esta ayuda

VARIABLES DE ENTORNO:
  ROPERO_DIR         igual que --dir
  ROPERO_BIN_DIR     igual que --bin-dir
  GITHUB_TOKEN / GH_TOKEN   sólo si el repositorio es privado (viaja en
                     cabecera, nunca en la URL)

LAYOUT QUE DEJA:
  ~/.local/share/ropero/   farm: un symlink por entrada de primer nivel del
                           repo + ropero.d → <repo>/bin/lib
  ~/.local/bin/ropero      → <repo>/bin/ropero   (y NADA más en ~/.local/bin)

Exit: 0 ok · 1 fallo real · 2 uso.
EOF
}

# -----------------------------------------------------------------------------
# parse_args — los flags pisan al entorno. Flag desconocido → exit 2.
# -----------------------------------------------------------------------------
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --install)
                shift ;;
            --local)
                LOCAL=1; shift ;;
            --check)
                CHECK=1; shift ;;
            --dir)
                [[ $# -ge 2 ]] || die_uso "--dir necesita un directorio"
                DIR="$2"; shift 2 ;;
            --dir=*)
                DIR="${1#*=}"; shift ;;
            --bin-dir)
                [[ $# -ge 2 ]] || die_uso "--bin-dir necesita un directorio"
                BIN_DIR="$2"; shift 2 ;;
            --bin-dir=*)
                BIN_DIR="${1#*=}"; shift ;;
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
    [[ -n "$DIR" ]] || die_uso "--dir vacío"
    [[ -n "$BIN_DIR" ]] || die_uso "--bin-dir vacío"
    [[ -n "$VERSION" ]] || die_uso "--version vacío"

    DIR="${DIR%/}"
    [[ -n "$DIR" ]] || DIR="/"
    BIN_DIR="${BIN_DIR%/}"
    [[ -n "$BIN_DIR" ]] || BIN_DIR="/"

    if [[ -n "$TARBALL" && ! -f "$TARBALL" ]]; then
        die "el tarball local no existe: $TARBALL"
    fi
    if [[ "$LOCAL" == "1" && -n "$TARBALL" ]]; then
        warn "--local ignora --tarball: el origen es el repo de este script"
        TARBALL=""
    fi
    if [[ "$LOCAL" == "1" && "$VERSION" != "main" ]]; then
        info "--local ignora --version ($VERSION): el origen es el repo local"
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
# FASE 1 — preparar
# -----------------------------------------------------------------------------
fase_preparar() {
    phase "preparar"
    mkdir -p "$BIN_DIR" 2>/dev/null || die "no se pudo crear $BIN_DIR"
    ok "$BIN_DIR listo"
    return 0
}

# -----------------------------------------------------------------------------
# FASE 2 — obtener (sólo modo red)
# Descarga el tarball (o usa --tarball), lo INSPECTA antes de extraer y lo
# instala en $DIR con sustitución atómica.
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

fase_obtener() {
    phase "obtener ropero (ref: $VERSION)"

    have tar || die "falta 'tar', imprescindible para desempaquetar"

    if [[ -n "$TARBALL" ]]; then
        info "origen: tarball local $TARBALL (sin red)"
        SRC_TAR="$TARBALL"
    else
        if have curl; then
            DL_TOOL="curl"
        elif have wget; then
            DL_TOOL="wget"
        elif have git; then
            warn "ni curl ni wget: se intentará git clone como fallback"
            DL_TOOL=""
        else
            die "falta 'curl' (o 'wget', o 'git'): no hay forma de obtener ropero"
        fi

        make_tmp
        local out="$TMP_DIR/ropero.tar.gz"
        info "descargando $CODELOAD/$VERSION"
        if [[ -n "$DL_TOOL" ]]; then
            if _dl "$CODELOAD/$VERSION" "$out" 2>/dev/null && [[ -s "$out" ]]; then
                tar -tzf "$out" >/dev/null 2>&1 || die "el tarball descargado está corrupto"
                ok "descargado ($(wc -c <"$out" | tr -d ' ') bytes)"
                SRC_TAR="$out"
            else
                rm -f "$out" 2>/dev/null || true
                warn "la descarga del tarball falló (¿sin red? ¿ref '$VERSION' inexistente?)"
            fi
        fi

        if [[ -z "$SRC_TAR" ]]; then
            have git || die "no se pudo obtener ropero (sin curl/wget no queda fallback de clone: falta git)"
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
            ok "clonado"
            SRC_CLONE=1
            SRC_TAR="$clone"
        fi
    fi

    # ── Extracción: a un staging JUNTO a $DIR (mismo filesystem → el rename
    #    final es atómico).
    local padre
    padre="$(dirname "$DIR")"
    mkdir -p "$padre" 2>/dev/null || die "no se pudo crear $padre"

    if [[ "$SRC_CLONE" == "1" ]]; then
        STAGE_DIR="$SRC_TAR"
        rm -rf "$STAGE_DIR/.git" 2>/dev/null || true
        [[ -f "$STAGE_DIR/$ENTRY" ]] || die "el clon no contiene $ENTRY"
        ok "staging (clone): $STAGE_DIR"
    else
        # ── Seguridad del tarball: nada de rutas absolutas ni ".." que salgan
        #    del destino. Se revisa el listado ANTES de extraer nada.
        local listado=""
        listado="$(tar -tzf "$SRC_TAR" 2>/dev/null)" || die "no se pudo leer el tarball"
        [[ -n "$listado" ]] || die "el tarball está vacío"

        local e
        while IFS= read -r e; do
            [[ -n "$e" ]] || continue
            case "$e" in
                /*)               die "el tarball contiene una ruta absoluta ($e): no se extrae" ;;
                ../*|*/../*|*/..) die "el tarball contiene una ruta fuera del destino ($e): no se extrae" ;;
            esac
        done <<<"$listado"

        # --strip-components sólo si TODAS las entradas comparten el mismo
        # directorio raíz (los tarballs de GitHub siempre: ropero-main/...).
        local strip=0 tops ntops
        tops="$(printf '%s\n' "$listado" | awk -F/ 'NF>1 {print $1}' | sort -u)"
        ntops="$(printf '%s\n' "$tops" | grep -c . || true)"
        [[ "$ntops" == "1" ]] && strip=1

        STAGE_DIR="$(mktemp -d "$padre/.${NAME}-stage.XXXXXX" 2>/dev/null)" \
            || die "no se pudo crear el staging junto a $DIR"

        local -a tar_flags=(-xzf "$SRC_TAR" -C "$STAGE_DIR")
        # --no-same-owner sólo existe en GNU tar; bsdtar no lo necesita.
        if tar --version 2>/dev/null | head -n 1 | grep -q "GNU tar"; then
            tar_flags+=(--no-same-owner)
        fi
        (( strip == 1 )) && tar_flags+=(--strip-components=1)

        tar "${tar_flags[@]}" || die "falló la extracción del tarball"
        [[ -f "$STAGE_DIR/$ENTRY" ]] || die "el tarball no contiene $ENTRY"
        chmod +x "$STAGE_DIR/$ENTRY" 2>/dev/null || true
        ok "extraído en staging: $STAGE_DIR"
    fi

    # ── Sustitución atómica de $DIR.
    if [[ -e "$DIR" || -L "$DIR" ]]; then
        if [[ -L "$DIR" ]]; then
            die "$DIR es un symlink (¿instalación local?): no lo sustituyo. Usa --local o quítalo tú"
        fi
        if [[ ! -f "$DIR/$ENTRY" ]]; then
            # Nunca se pisa lo que no es ropero: un --dir apuntando al home del
            # usuario o a otro programa es error CLARO, no rm -rf.
            die "$DIR ya existe y NO es una instalación de ropero: no lo toco"
        fi
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
    chmod 755 "$DIR" 2>/dev/null || true

    # En red el árbol descargado ES el farm: no hay repo del usuario al que
    # enlazar, así que ORIGIN es el propio $DIR.
    ORIGIN="$DIR"
    return 0
}

# -----------------------------------------------------------------------------
# FASE 3 — origen local (sólo con --local)
#
#   origen = dirname de este script; se verifica que existe <origen>/<ENTRY>.
#   El repo NO se mueve ni se copia: aquí sólo hay symlinks (R3).
# -----------------------------------------------------------------------------
fase_origen_local() {
    phase "origen local"
    local self="${BASH_SOURCE[0]:-$0}"
    if [[ ! -f "$self" ]]; then
        die "--local necesita el repo en disco: install.sh llegó por stdin"
    fi
    ORIGIN="$(real_dir "$self")" || die "no se pudo resolver el directorio de este script"
    [[ -n "$ORIGIN" ]] || die "no se pudo resolver el directorio de este script"
    [[ -f "$ORIGIN/$ENTRY" ]] || die "el repo local no contiene $ENTRY: $ORIGIN/$ENTRY"
    ok "origen: $ORIGIN"
    return 0
}

# -----------------------------------------------------------------------------
# FASE 4 — farm (~/.local/share/<name>)
#
#   Un symlink por entrada de primer nivel del ORIGEN + <name>.d → <MODULES>.
#
#   · Si el farm es un symlink (instalación local previa): no se escribe
#     dentro, se avisa y se sigue.
#   · Si es un directorio real con <ENTRY> (la copia de la instalación
#     vieja): se MIGRA — se aparta a un lado, se monta el farm y se borra.
#   · Si es un directorio real con otra cosa: se niega (nunca se pisa).
# -----------------------------------------------------------------------------
fase_farm() {
    phase "farm: $DIR"
    local farm="$DIR"

    if [[ -L "$farm" ]]; then
        # Instalación local previa: el farm es un symlink. Escribir dentro
        # escribiría en el repo del usuario: no se hace.
        warn "$farm ya es un symlink (instalación local previa): no escribo dentro"
        return 0
    fi
    if [[ -e "$farm" && ! -d "$farm" ]]; then
        die "$farm existe y no es un directorio: no lo toco"
    fi

    if [[ "$ORIGIN" == "$farm" ]]; then
        # Modo red: el árbol descargado ya vive en el farm. Sus entradas son
        # ficheros reales (un symlink a sí mismo sería un bucle), así que
        # sólo se añade el alias de módulos.
        info "el árbol descargado es el farm: sus entradas son ficheros reales"
        farm_alias "$farm"
        return 0
    fi

    # ¿Es una copia real de una instalación anterior? → migrar a farm.
    #
    # OJO: se mira el 'bin' de PRIMER NIVEL, no $farm/bin/ropero. En un farm
    # ya montado, 'bin' es un symlink al repo, así que $farm/bin/ropero es un
    # fichero REAL a través de él: mirarlo por ahí detectaría "copia" en cada
    # ejecución y reharía el farm entero (y perdería lo que hubiera dentro).
    local antigua=""
    local entry_dir="${ENTRY%/*}"
    if [[ "$entry_dir" == "$ENTRY" ]]; then entry_dir=""; fi
    if [[ -n "$entry_dir" && -d "$farm/$entry_dir" && ! -L "$farm/$entry_dir" \
          && -e "$farm/$ENTRY" ]]; then
        antigua="$farm.antigua.$$"
        mv "$farm" "$antigua" || die "no se pudo apartar $farm"
        ok "instalación anterior (copia real) apartada: $antigua"
    fi

    # Si el farm ya contiene cosas REALES que no son nuestras: no se pisa.
    local e base reales=""
    for e in "$farm"/* "$farm"/.[!.]*; do
        [[ -e "$e" || -L "$e" ]] || continue
        [[ -L "$e" ]] && continue
        reales="$reales ${e##*/}"
    done
    if [[ -n "$reales" ]]; then
        die "$farm contiene cosas que no son symlinks ($reales): no lo toco"
    fi

    mkdir -p "$farm" || die "no se pudo crear $farm"

    local destino enlaces=0
    while IFS= read -r base; do
        [[ -n "$base" ]] || continue
        destino="$ORIGIN/$base"
        if [[ -L "$farm/$base" ]]; then
            rm -f "$farm/$base" 2>/dev/null || true
        fi
        if ln -s "$destino" "$farm/$base" 2>/dev/null; then
            enlaces=$((enlaces+1))
        else
            err "no se pudo enlazar $farm/$base"
        fi
    done < <(entradas "$ORIGIN")
    (( enlaces > 0 )) && ok "farm: $enlaces entradas → $ORIGIN"

    farm_alias "$farm"

    # La copia antigua ya está sustituida: se borra.
    if [[ -n "$antigua" ]]; then
        rm -rf "$antigua" 2>/dev/null || true
        ok "instalación anterior eliminada: $antigua"
    fi
    return 0
}

# farm_alias <farm> — <name>.d → <origin>/<MODULES>
farm_alias() {
    local farm="$1" alias="$farm/$NAME.d"
    if [[ -L "$alias" ]]; then
        rm -f "$alias" 2>/dev/null || true
    elif [[ -e "$alias" ]]; then
        warn "$alias existe y NO es un symlink: no lo he tocado"
        return 0
    fi
    ln -s "$ORIGIN/$MODULES" "$alias" || die "no se pudo enlazar $alias"
    ok "$alias → $ORIGIN/$MODULES"
    return 0
}

# -----------------------------------------------------------------------------
# FASE 5 — enlazar (~/.local/bin/<name>)
#
#   El ÚNICO symlink que ropero deja en ~/.local/bin (R1). Si ahí hay un
#   fichero REAL no se toca (R4): la decisión es del usuario.
# -----------------------------------------------------------------------------
fase_enlazar() {
    phase "enlazar: $BIN_DIR/$NAME"
    mkdir -p "$BIN_DIR" || die "no se pudo crear $BIN_DIR"

    local link="$BIN_DIR/$NAME"
    local target="$ORIGIN/$ENTRY"

    if [[ -e "$link" && ! -L "$link" ]]; then
        # R4: un fichero REAL ahí (otro programa, un binario copiado a mano):
        # no se pisa en silencio. El usuario decide.
        warn "$link existe y NO es un symlink: no lo he tocado"
        LINK_SKIPPED=1
    else
        rm -f "$link" 2>/dev/null || true
        ln -s "$target" "$link" || die "no se pudo crear el symlink $link"
        ok "$link → $target"
    fi

    # R1: en ~/.local/bin sólo vive el entrypoint. El enlace heredado
    # <name>.d (layout viejo) se retira; si es un fichero real, ni se toca.
    local legacy="$BIN_DIR/$NAME.d"
    if [[ -L "$legacy" ]]; then
        rm -f "$legacy" 2>/dev/null || true
        warn "enlace obsoleto eliminado: $legacy (el layout nuevo deja sólo el entrypoint)"
    elif [[ -e "$legacy" ]]; then
        warn "$legacy existe y NO es un symlink: no lo he tocado"
    fi

    case ":$PATH:" in
        *":$BIN_DIR:"*) ok "$BIN_DIR está en el PATH" ;;
        *)
            warn "$BIN_DIR no está en tu PATH"
            printf '       export PATH="%s:$PATH"\n' "$BIN_DIR" >&2
            ;;
    esac
    return 0
}

# -----------------------------------------------------------------------------
# FASE 6 — verificar
# El único código que se ejecuta tras instalar: --help. NO compila nada, no
# descarga modelos y no arranca motores (R5).
# -----------------------------------------------------------------------------
fase_verificar() {
    phase "verificar"
    local bin="$ORIGIN/$ENTRY"
    [[ -f "$bin" ]] || die "no existe el entrypoint: $bin"

    local rc=0
    # Sin errexit: el estado se captura a mano y se decide aquí abajo. El
    # toggle de errexit de antes sólo servía para que este comando NO
    # abortara el script; ahora no hace falta porque no hay errexit.
    bash "$bin" --help >/dev/null 2>&1 </dev/null
    rc=$?
    if (( rc == 0 )); then
        ok "el lanzador responde (ropero --help → salida 0)"
    else
        die "ropero --help devolvió $rc: la instalación no funciona (prueba otra --version)"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# --check: verifica SIN instalar ni modificar nada.
# -----------------------------------------------------------------------------
fase_check() {
    phase "comprobar el layout"

    # ── El origen contra el que se compara. Si el symlink del entrypoint
    #    existe, el origen es el repo al que apunta: así el check da igual
    #    cómo se instalara (local o red). Si no, se deduce del modo.
    local link="$BIN_DIR/$NAME"
    local target dir rel=""
    local bruto=""
    if [[ -L "$link" ]]; then
        # Si el readlink falla NO se inventa un origen: con `target` vacío el
        # `case` lo convertiría en "$PWD/" y ORIGIN saldría del padre del
        # $PWD —un path falso contra el que se compararía todo lo demás.
        # Se deja ORIGIN vacío y se cae al camino de abajo (--local o $DIR).
        bruto="$(readlink "$link" 2>/dev/null)" || bruto=""
        target="$bruto"
        if [[ -n "$target" ]]; then
            case "$target" in
                /*) ;;
                *) target="$PWD/$target" ;;
            esac
            dir="$(real_path "$(dirname "$target")")" || dir=""
            if [[ -n "$dir" ]]; then
                rel="${ENTRY%/*}"
                if [[ "$rel" == "$ENTRY" ]]; then
                    ORIGIN="$dir"
                else
                    ORIGIN="$(cd -P "$dir/.." 2>/dev/null && pwd)" || ORIGIN=""
                fi
            fi
        fi
    fi

    if [[ -z "$ORIGIN" ]]; then
        if [[ "$LOCAL" == "1" ]]; then
            local self="${BASH_SOURCE[0]:-$0}"
            ORIGIN="$(real_dir "$self")" || ORIGIN=""
            [[ -n "$ORIGIN" ]] || die "no se pudo resolver el directorio de este script"
            [[ -f "$ORIGIN/$ENTRY" ]] || die "el repo local no contiene $ENTRY: $ORIGIN/$ENTRY"
        else
            ORIGIN="$DIR"
            [[ -d "$ORIGIN" ]] || die "$DIR no existe: $NAME no está instalado"
            [[ -f "$ORIGIN/$ENTRY" ]] || die "$DIR no contiene $ENTRY: $NAME no está instalado"
        fi
    fi
    ok "origen: $ORIGIN"

    # ── 1. El entrypoint en ~/.local/bin.
    if [[ -L "$link" ]]; then
        # El destino se muestra desde el `readlink` ya leído arriba: si falló,
        # se dice, en vez de imprimir un enlace sin destino.
        if [[ -n "$bruto" ]]; then
            ok "$link → $bruto"
        else
            err "$link: no se pudo leer el enlace (falló readlink)"
        fi
        if [[ -e "$link" ]]; then
            ok "el symlink resuelve"
        else
            err "el symlink está roto: $bruto"
        fi
        if same_path "$link" "$ORIGIN/$ENTRY"; then
            ok "apunta al entrypoint: $ORIGIN/$ENTRY"
        else
            err "apunta a otra cosa (esperado: $ORIGIN/$ENTRY)"
        fi
    elif [[ -e "$link" ]]; then
        err "$link existe y NO es un symlink"
    else
        err "$NAME NO está enlazado en $BIN_DIR"
        printf '       → bash %s --local\n' "$ORIGIN/install.sh" >&2
    fi

    # ── 2. R1: en ~/.local/bin sólo el entrypoint.
    if [[ -e "$BIN_DIR/$NAME.d" || -L "$BIN_DIR/$NAME.d" ]]; then
        warn "$BIN_DIR/$NAME.d existe: el layout nuevo deja sólo el entrypoint en ~/.local/bin"
    else
        ok "$BIN_DIR sólo contiene el entrypoint de $NAME"
    fi

    # ── 3. El farm.
    local farm="$DIR"
    if [[ -L "$farm" ]]; then
        warn "$farm es un symlink (instalación local previa): no se comprueba su contenido"
    elif [[ ! -d "$farm" ]]; then
        err "el farm no existe: $farm"
    else
        ok "farm: $farm"

        local alias="$farm/$NAME.d"
        if [[ -L "$alias" ]]; then
            ok "$alias → $(readlink "$alias")"
            if same_path "$alias" "$ORIGIN/$MODULES"; then
                ok "el alias de módulos apunta a $ORIGIN/$MODULES"
            else
                err "el alias de módulos no apunta a $ORIGIN/$MODULES"
            fi
        elif [[ -e "$alias" ]]; then
            err "$alias existe y NO es un symlink"
        else
            err "falta el alias de módulos: $alias"
        fi

        # Una entrada por cada elemento de primer nivel del origen.
        local base destino faltan=0
        while IFS= read -r base; do
            [[ -n "$base" ]] || continue
            destino="$ORIGIN/$base"
            if [[ "$ORIGIN" == "$farm" ]]; then
                # Modo red: el árbol está en el farm; sus entradas son reales.
                [[ -e "$farm/$base" ]] || { err "falta $farm/$base"; faltan=$((faltan+1)); }
            elif [[ -L "$farm/$base" ]]; then
                if same_path "$farm/$base" "$destino"; then
                    :
                else
                    err "$farm/$base no apunta a $destino"
                    faltan=$((faltan+1))
                fi
            elif [[ -e "$farm/$base" ]]; then
                err "$farm/$base existe y NO es un symlink"
                faltan=$((faltan+1))
            else
                err "falta $farm/$base → $destino"
                faltan=$((faltan+1))
            fi
        done < <(entradas "$ORIGIN")
        if (( faltan == 0 )); then
            ok "el farm tiene una entrada por cada elemento de primer nivel del origen"
        fi
    fi

    # ── 4. El entrypoint responde.
    if bash "$ORIGIN/$ENTRY" --help >/dev/null 2>&1 </dev/null; then
        ok "el lanzador responde (ropero --help → salida 0)"
    else
        err "ropero --help NO responde: la instalación está rota"
    fi

    case ":$PATH:" in
        *":$BIN_DIR:"*) ok "$BIN_DIR está en el PATH" ;;
        *) warn "$BIN_DIR no está en tu PATH" ;;
    esac
    return 0
}

# -----------------------------------------------------------------------------
# main
# -----------------------------------------------------------------------------
main() {
    parse_args "$@"

    if [[ "$CHECK" == "1" ]]; then
        fase_check
    else
        fase_preparar
        if [[ "$LOCAL" == "1" ]]; then
            fase_origen_local
        else
            fase_obtener
        fi
        fase_farm
        fase_enlazar
        fase_verificar
    fi

    resumen

    if (( N_FAIL > 0 )); then
        exit 1
    fi

    if [[ "$CHECK" != "1" ]]; then
        printf '\n%s══ %s instalado ══%s\n' "$C_HEAD" "$NAME" "$C_OFF" >&2
        printf '  origen:    %s\n' "$ORIGIN" >&2
        printf '  farm:      %s\n' "$DIR" >&2
        if [[ "$LINK_SKIPPED" == "1" ]]; then
            printf '  symlink:   NO creado (había algo en %s/%s que no era un symlink)\n' \
                "$BIN_DIR" "$NAME" >&2
        else
            printf '  symlink:   %s/%s → %s\n' "$BIN_DIR" "$NAME" "$ORIGIN/$ENTRY" >&2
        fi
        printf '  módulos:   %s/%s → %s\n' "$DIR" "$NAME.d" "$ORIGIN/$MODULES" >&2
        printf '\n  Siguientes pasos:\n' >&2
        printf '    ropero install setup     # dependencias base\n' >&2
        printf '    ropero install engines   # engines compatibles\n' >&2
        printf '    ropero install models    # baja o fabrica modelos\n' >&2
        printf '    ropero list              # catálogo\n' >&2
        printf '\n  Para actualizar:\n' >&2
        printf '    curl -fsSL https://raw.githubusercontent.com/%s/main/install.sh | bash\n' "$REPO" >&2
    fi
    exit 0
}

main "$@"
