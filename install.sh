#!/usr/bin/env bash
# =============================================================================
# install.sh — instalador "oneliner" de ropero
# =============================================================================
# USO:
#   curl -fsSL https://raw.githubusercontent.com/Lorenzo-SF/ropero/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/Lorenzo-SF/ropero/main/install.sh | bash -s -- --help
#
# QUÉ INSTALA:
#   - el lanzador `ropero` (con su directorio de módulos) bajo --prefix
#   - un symlink `ropero` en --bin-dir
#
# QUÉ NO INSTALA:
#   - engines ni modelos. Eso es `ropero install engines` / `ropero install models`:
#     requiere GPU, compilaciones y descargas de GB. Un instalador que se come
#     todo eso sin que nadie lo pida es un instalador que asusta a la gente.
#
# POR QUÉ EXISTE ESTE FICHERO:
#   Sustituye al antiguo `ropero --install` / `ropero --check`. La instalación no
#   puede vivir dentro de ropero: para instalar ropero hace falta ropero. Este
#   script se apoya sólo en bash + curl/wget + tar, así que funciona antes de
#   tener nada de ropero en la máquina.
#
# SEGURIDAD:
#   - sin sudo, sin "sudo bash": todo va a --prefix, --bin-dir o ~/.cache
#   - el tarball se inspecciona antes de extraerse (rutas absolutas o con ".."
#     se rechazan) y se extrae en un staging DENTRO de --prefix, de modo que
#     cambiar de versión es un rename y una instalación en curso no se rompe
#   - el único código ajeno que se ejecuta es `<instalación>/bin/ropero --help`
# =============================================================================

set -euo pipefail

# -----------------------------------------------------------------------------
# Bootstrap: el script puede llegar por stdin (curl … | bash), donde $0 es
# "bash" y no hay fichero de origen legible. En ese caso leemos el script
# COMPLETO de stdin y nos re-ejecutamos con los mismos argumentos, para que
# toda la lógica de este fichero corra con sus "$@" intactos.
#
# ROPERO_BOOTSTRAP es el cerrojo: sin él, la segunda pasada intentaría volver a
# leer stdin (ya en EOF), se quedaría sin argumentos y --help no funcionaría.
# -----------------------------------------------------------------------------
_ropero_bootstrap() {
    [[ "${ROPERO_BOOTSTRAP:-0}" == "1" ]] && return 0

    local self="${BASH_SOURCE[0]:-$0}"
    local shell_name; shell_name="$(basename "${BASH:-bash}")"
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
_ropero_bootstrap "$@"

# -----------------------------------------------------------------------------
# Configuración. Prioridad: flag > variable de entorno > valor por defecto.
# -----------------------------------------------------------------------------
ROPERO_REPO="${ROPERO_REPO:-https://github.com/Lorenzo-SF/ropero}"
PREFIX="${ROPERO_PREFIX:-${HOME:-}/.local/share/ropero}"
BIN_DIR="${ROPERO_BIN_DIR:-${HOME:-}/.local/bin}"
VERSION="${ROPERO_VERSION:-main}"
FORCE="${ROPERO_FORCE:-0}"
NO_LINK="${ROPERO_NO_LINK:-0}"

# Token opcional para repositorios privados. Vive SOLO en el entorno: no se
# escribe en disco, no se imprime y no viaja a ningún subproceso salvo al
# curl/wget que hace la descarga.
TOKEN="${GITHUB_TOKEN:-${GH_TOKEN:-}}"

# Estado interno (lo rellenan las fases).
TMP_DIR=""        # temporal de descarga
STAGE_DIR=""      # staging de extracción (dentro de PREFIX)
TARGET_DIR=""     # PREFIX/ropero-<versión>
TAR_BALL=""       # tarball descargado
ENTRY_REL=""      # ruta del lanzador DENTRO del árbol (bin/ropero, ropero…)
LINK_PATH=""      # symlink final en BIN_DIR
LINK_SKIPPED=0
DL_TOOL=""        # curl | wget

# -----------------------------------------------------------------------------
# Salida. Marcadores iguales a los de bin/00-utils.sh para que la
# terminal quede homogénea: [i] info, [✓] ok, [!] aviso, [✗] error.
# -----------------------------------------------------------------------------
if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
    C_OK=$'\033[1;32m'; C_WARN=$'\033[1;33m'; C_ERR=$'\033[0;31m'
    C_HEAD=$'\033[1m'; C_OFF=$'\033[0m'
else
    C_OK=""; C_WARN=""; C_ERR=""; C_HEAD=""; C_OFF=""
fi

info()  { printf '  %s[i]%s %s\n' "$C_HEAD" "$C_OFF" "$*"; }
ok()    { printf '  %s[✓]%s %s\n' "$C_OK" "$C_OFF" "$*"; }
warn()  { printf '  %s[!]%s %s\n' "$C_WARN" "$C_OFF" "$*"; }
err()   { printf '  %s[✗]%s %s\n' "$C_ERR" "$C_OFF" "$*" >&2; }
die()   { err "$*"; exit 1; }
phase() { printf '\n%s▸ [%s/5]%s %s\n' "$C_HEAD" "$1" "$C_OFF" "$2"; }

have()  { command -v "$1" >/dev/null 2>&1; }

# -----------------------------------------------------------------------------
# Limpieza. El trap se registra siempre: el temporal de descarga y el staging
# se borran tanto en éxito como en fallo (Ctrl-C incluido).
# -----------------------------------------------------------------------------
cleanup() {
    if [[ -n "$STAGE_DIR" && -d "$STAGE_DIR" ]]; then rm -rf "$STAGE_DIR" 2>/dev/null || true; fi
    if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then rm -rf "$TMP_DIR" 2>/dev/null || true; fi
    return 0
}
trap cleanup EXIT INT TERM

usage() {
    cat <<EOF
install.sh — instalador de ropero (lanzador + symlink)

USO:
  curl -fsSL ${ROPERO_REPO}/raw/main/install.sh | bash
  curl -fsSL ${ROPERO_REPO}/raw/main/install.sh | bash -s -- --help

FLAGS:
  --prefix <dir>    dónde se instala ropero   (def: ${PREFIX})
  --bin-dir <dir>   dónde se crea el symlink   (def: ${BIN_DIR})
  --version <ref>   versión a instalar: main o un tag  (def: ${VERSION})
  --force           reinstala aunque ya exista
  --no-link         no crea el symlink en --bin-dir
  -h, --help        esta ayuda

VARIABLES DE ENTORNO (equivalentes a los flags; el flag manda):
  ROPERO_PREFIX    ROPERO_BIN_DIR    ROPERO_VERSION
  ROPERO_FORCE     ROPERO_NO_LINK    ROPERO_REPO
  GITHUB_TOKEN / GH_TOKEN   sólo si el repositorio es privado

NOTAS:
  - No hace falta sudo: todo va a --prefix, --bin-dir y ~/.cache.
  - No instala engines ni modelos: luego usa 'ropero install engines' y
    'ropero install models'.
EOF
}

# -----------------------------------------------------------------------------
# parse_args — los flags pisan lo que venga del entorno.
# -----------------------------------------------------------------------------
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --prefix)    [[ $# -ge 2 ]] || die "--prefix necesita un directorio"; PREFIX="$2"; shift 2 ;;
            --prefix=*)  PREFIX="${1#*=}"; shift ;;
            --bin-dir)   [[ $# -ge 2 ]] || die "--bin-dir necesita un directorio"; BIN_DIR="$2"; shift 2 ;;
            --bin-dir=*) BIN_DIR="${1#*=}"; shift ;;
            --version)   [[ $# -ge 2 ]] || die "--version necesita una referencia"; VERSION="$2"; shift 2 ;;
            --version=*) VERSION="${1#*=}"; shift ;;
            --force|-f)  FORCE=1; shift ;;
            --no-link)   NO_LINK=1; shift ;;
            -h|--help)   usage; exit 0 ;;
            *)           err "argumento desconocido: '$1'"; usage >&2; exit 1 ;;
        esac
    done

    [[ -n "${HOME:-}" ]] || die "HOME no está definido: no hay dónde instalar"
    [[ -n "$PREFIX"   ]] || die "--prefix vacío"
    [[ -n "$BIN_DIR"  ]] || die "--bin-dir vacío"
    [[ -n "$VERSION"  ]] || die "--version vacío"

    # Sin barras finales: "<prefix>/ropero-<versión>" no debe salir con "//".
    PREFIX="${PREFIX%/}"
    BIN_DIR="${BIN_DIR%/}"
    [[ -n "$PREFIX" ]] || PREFIX="/"
    [[ -n "$BIN_DIR" ]] || BIN_DIR="/"
}

# -----------------------------------------------------------------------------
# phase_check_system — SO, arquitectura y herramientas.
#
# Aborta sólo si falta algo IMPRESCINDIBLE (tar, curl/wget). python3 avisa y
# sigue: ropero lo necesita para leer los TOML de los modelos, pero alguien
# puede querer instalar sólo el lanzador, y quitarle eso sería quitarle
# opciones sin pedirle permiso.
# -----------------------------------------------------------------------------
phase_check_system() {
    phase 1 "Comprobar el sistema"

    local os arch
    os="$(uname -s 2>/dev/null || echo 'desconocido')"
    arch="$(uname -m 2>/dev/null || echo 'desconocido')"
    ok "sistema: $os ($arch)"

    if (( BASH_VERSINFO[0] < 4 )); then
        warn "bash $BASH_VERSION es muy antiguo (se pide >= 4)"
    else
        ok "bash $BASH_VERSION"
    fi

    have tar || die "falta 'tar', imprescindible para desempaquetar el tarball"
    ok "tar disponible"

    if have curl; then
        DL_TOOL=curl
    elif have wget; then
        DL_TOOL=wget
    else
        die "falta 'curl' (o 'wget'): hace falta uno de los dos para descargar"
    fi
    ok "descargador: $DL_TOOL"

    # python3: aviso, nunca motivo de abortar.
    local pyv="" pymaj="" pymin=""
    if have python3; then
        # Un shim (asdf/pyenv) puede responder con código 0 y no ejecutar
        # python: por eso se parsea la salida y no se mira el exit code.
        pyv="$(python3 -c 'import sys; print("%d.%d" % sys.version_info[:2])' 2>/dev/null | tail -n 1 || true)"
        if [[ "$pyv" =~ ^[0-9]+\.[0-9]+$ ]]; then
            pymaj="${pyv%%.*}"; pymin="${pyv##*.}"
            if (( pymaj > 3 || (pymaj == 3 && pymin >= 8) )); then
                ok "python3 $pyv"
            else
                warn "python3 $pyv es demasiado antiguo (ropero pide >= 3.8)"
            fi
        else
            warn "el 'python3' del PATH no responde (shim sin versión activa)"
        fi
    else
        warn "no se encuentra python3"
    fi
    if [[ ! "$pyv" =~ ^[0-9]+\.[0-9]+$ ]]; then
        warn "ropero sigue instalándose: el lanzador funciona sin python3."
        warn "Los modelos se configuran en TOML y se leen con python3 (>= 3.8):"
        warn "'ropero install models' fallará hasta que haya python3 en el PATH."
    fi

    if have git; then
        ok "git disponible (opcional: no hace falta para instalar)"
    else
        info "git no está: es opcional, el instalador descarga un tarball publicado"
    fi
}

# -----------------------------------------------------------------------------
# make_tmp — temporal de descarga bajo ~/.cache/ropero (lo único que se toca
# fuera de --prefix/--bin-dir, y se borra siempre). Si ~/.cache no es
# escribible se cae al temporal del sistema.
# -----------------------------------------------------------------------------
make_tmp() {
    local cache="${XDG_CACHE_HOME:-${HOME:-}/.cache}/ropero"
    if mkdir -p "$cache" 2>/dev/null; then
        TMP_DIR="$(mktemp -d "$cache/install.XXXXXX" 2>/dev/null || true)"
    fi
    if [[ -z "$TMP_DIR" ]]; then
        TMP_DIR="$(mktemp -d 2>/dev/null || true)"
    fi
    [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]] || die "no se pudo crear el directorio temporal"
}

# -----------------------------------------------------------------------------
# _dl — descarga una URL a un fichero, con reintentos. curl primero, wget
# como plan B. El token va en una cabecera, nunca en la URL (así no acaba en
# un log de shell ni en el historial).
# -----------------------------------------------------------------------------
_dl() {
    local url="$1" out="$2"
    if [[ "$DL_TOOL" == "curl" ]]; then
        if [[ -n "$TOKEN" ]]; then
            curl -fsSL --retry 3 --retry-delay 2 --retry-connrefused \
                 -H "Authorization: token $TOKEN" -o "$out" "$url"
        else
            curl -fsSL --retry 3 --retry-delay 2 --retry-connrefused -o "$out" "$url"
        fi
    else
        if [[ -n "$TOKEN" ]]; then
            wget -q --tries=3 --timeout=30 --header="Authorization: token $TOKEN" -O "$out" "$url"
        else
            wget -q --tries=3 --timeout=30 -O "$out" "$url"
        fi
    fi
}

# -----------------------------------------------------------------------------
# phase_download — baja el tarball de la versión pedida.
#
# GitHub publica en dos sitios: refs/heads/<rama> y refs/tags/<tag>. Para
# "main" se prueba la rama primero; para cualquier otra cosa, el tag primero
# (un tag es lo que se espera al pedir una "versión") y la rama como plan B.
# -----------------------------------------------------------------------------
phase_download() {
    phase 2 "Descargar"

    # Si esa versión ya está instalada no hace falta bajar nada: la segunda
    # ejecución es un no-op rápido y funciona incluso sin red.
    if [[ -d "$TARGET_DIR" && "$FORCE" != "1" ]]; then
        info "ya está instalado en $TARGET_DIR: no hace falta descargar"
        info "  (usa --force para reinstalar/actualizar)"
        return 0
    fi

    make_tmp
    TAR_BALL="$TMP_DIR/ropero.tar.gz"

    local -a urls=()
    if [[ "$VERSION" == "main" ]]; then
        urls+=("$ROPERO_REPO/archive/refs/heads/$VERSION.tar.gz")
        urls+=("$ROPERO_REPO/archive/refs/tags/$VERSION.tar.gz")
    else
        urls+=("$ROPERO_REPO/archive/refs/tags/$VERSION.tar.gz")
        urls+=("$ROPERO_REPO/archive/refs/heads/$VERSION.tar.gz")
    fi

    local url rc=1
    for url in "${urls[@]}"; do
        info "descargando $url"
        set +e
        _dl "$url" "$TAR_BALL"
        rc=$?
        set -e
        if (( rc == 0 )) && [[ -s "$TAR_BALL" ]]; then
            break
        fi
        # `|| true`: un borrado fallido nunca debe abortar la instalación.
        rm -f "$TAR_BALL" || true
        rc=1
    done

    if (( rc != 0 )) || [[ ! -s "$TAR_BALL" ]]; then
        err "no se pudo descargar la versión '$VERSION' desde $ROPERO_REPO"
        if [[ -z "$TOKEN" ]]; then
            err "si el repositorio es privado, exporta GITHUB_TOKEN=... y vuelve a intentarlo"
        fi
        die "descarga fallida"
    fi

    # Tamaño > 0 no basta: comprobamos que el archivo es un gzip válido.
    tar -tzf "$TAR_BALL" >/dev/null 2>&1 || die "el tarball descargado está corrupto"

    ok "descargado ($(wc -c <"$TAR_BALL" | tr -d ' ') bytes)"
}

# -----------------------------------------------------------------------------
# _safe_component — deja un nombre de directorio válido a partir de la versión
# (nada de rutas ni comillas raras dentro del prefix).
# -----------------------------------------------------------------------------
_safe_component() {
    printf '%s' "$1" | tr -c 'A-Za-z0-9._-' '-'
}

# -----------------------------------------------------------------------------
# _target_dir — PREFIX/ropero-<versión>. Se calcula antes de descargar para
# poder saltarse la descarga si esa versión ya está instalada.
# -----------------------------------------------------------------------------
_target_dir() {
    local safe; safe="$(_safe_component "$VERSION")"
    [[ -n "$safe" ]] || safe="main"
    printf '%s' "$PREFIX/ropero-$safe"
}

# -----------------------------------------------------------------------------
# _find_entrypoint — localiza el lanzador dentro de un árbol.
# Se prefiere la estructura nueva (bin/ropero) y se acepta la antigua
# (ropero en la raíz) para que el instalador funcione con cualquier versión
# publicada, no sólo con la que se publique mañana.
# -----------------------------------------------------------------------------
_find_entrypoint() {
    local root="$1" cand

    for cand in "bin/ropero" "ropero"; do
        if [[ -f "$root/$cand" ]]; then
            printf '%s' "$cand"; return 0
        fi
    done
    while IFS= read -r cand; do
        [[ -n "$cand" ]] || continue
        printf '%s' "${cand#"$root"/}"; return 0
    done < <(find "$root" -maxdepth 3 -type f -name ropero 2>/dev/null | head -n 5)

    return 1
}

# -----------------------------------------------------------------------------
# phase_extract — desempaqueta en un staging dentro de --prefix y deja el
# resultado en PREFIX/ropero-<versión>. El cambio de versión es un rename:
# una instalación en curso nunca se ve rota a medias.
# -----------------------------------------------------------------------------
phase_extract() {
    phase 3 "Extraer"

    # Camino de reuso: no se ha descargado nada porque la versión ya estaba
    # instalada. Se reutiliza tal cual y no se toca.
    if [[ -z "$TAR_BALL" ]]; then
        local reuse_entry
        reuse_entry="$(_find_entrypoint "$TARGET_DIR")" \
            || die "la instalación de $TARGET_DIR está incompleta: reinstala con --force"
        ENTRY_REL="$reuse_entry"
        ok "reutilizando la instalación de $TARGET_DIR"
        return 0
    fi

    # --- Seguridad del tarball --------------------------------------------
    # Nada de rutas absolutas ni de ".." que salgan del directorio de destino.
    # Se revisa el listado ANTES de extraer nada.
    local entries
    if ! entries="$(tar --absolute-names -tzf "$TAR_BALL" 2>/dev/null)"; then
        entries="$(tar -tzf "$TAR_BALL" 2>/dev/null)" || die "no se pudo leer el tarball"
    fi
    [[ -n "$entries" ]] || die "el tarball está vacío"

    local e
    while IFS= read -r e; do
        [[ -n "$e" ]] || continue
        case "$e" in
            /*)               die "el tarball contiene una ruta absoluta ($e): no se extrae" ;;
            ../*|*/../*|*/..) die "el tarball contiene una ruta fuera del destino ($e): no se extrae" ;;
        esac
    done <<< "$entries"

    # --strip-components sólo si TODAS las entradas comparten el mismo primer
    # directorio (los tarballs de GitHub siempre: ropero-main/...). Si no, se
    # extrae tal cual para no mutilar el contenido.
    local strip=0 tops ntops
    tops="$(printf '%s\n' "$entries" | awk -F/ 'NF>1 {print $1}' | sort -u)"
    ntops="$(printf '%s\n' "$tops" | grep -c . || true)"
    if [[ "$ntops" == "1" ]]; then
        strip=1
    fi

    mkdir -p "$PREFIX" || die "no se pudo crear $PREFIX"
    STAGE_DIR="$(mktemp -d "$PREFIX/.stage.XXXXXX" 2>/dev/null || true)"
    [[ -n "$STAGE_DIR" && -d "$STAGE_DIR" ]] || die "no se pudo crear el staging en $PREFIX"

    local -a tar_flags=(-xzf "$TAR_BALL" -C "$STAGE_DIR")
    # --no-same-owner sólo existe en GNU tar; en bsdtar no hace falta (no
    # extraemos nada como root).
    if tar --version 2>/dev/null | head -n 1 | grep -q "GNU tar"; then
        tar_flags+=(--no-same-owner)
    fi
    (( strip == 1 )) && tar_flags+=(--strip-components=1)

    tar "${tar_flags[@]}" || die "falló la extracción del tarball"

    # Fallo rápido si el tarball no trae el lanzador.
    _find_entrypoint "$STAGE_DIR" >/dev/null \
        || die "el tarball no contiene el lanzador 'ropero' (ni en bin/ ni en la raíz)"

    # --- Sustitución de la versión anterior ------------------------------
    if [[ -e "$TARGET_DIR" ]]; then
        # Sólo se llega aquí con --force: la instalación existente se aparta y
        # se sustituye. El symlink 'current' nunca queda colgando más de un
        # rename.
        local old="$TARGET_DIR.old.$$"
        mv "$TARGET_DIR" "$old" || die "no se pudo apartar $TARGET_DIR"
        if ! mv "$STAGE_DIR" "$TARGET_DIR"; then
            mv "$old" "$TARGET_DIR" 2>/dev/null || true
            die "no se pudo instalar en $TARGET_DIR (se restauró la versión anterior)"
        fi
        STAGE_DIR=""
        rm -rf "$old" || true
        ok "reinstalado en $TARGET_DIR (--force)"
    else
        mv "$STAGE_DIR" "$TARGET_DIR" || die "no se pudo mover el contenido a $TARGET_DIR"
        STAGE_DIR=""
        ok "extraído en $TARGET_DIR"
    fi
    # mktemp -d deja el staging en 700: la instalación debe tener permisos
    # normales, como si se hubiera descomprimido a mano.
    chmod 755 "$TARGET_DIR" 2>/dev/null || true

    # La ruta del lanzador se recalcula sobre el árbol FINAL (en el camino de
    # reuso el árbol final puede ser más antiguo que el tarball recién Bajado).
    local entry
    entry="$(_find_entrypoint "$TARGET_DIR")" \
        || die "no se encuentra el lanzador en $TARGET_DIR (reinstala con --force)"
    ENTRY_REL="$entry"
    chmod +x "$TARGET_DIR/$ENTRY_REL" 2>/dev/null || true
    ok "lanzador: $ENTRY_REL"
}

# -----------------------------------------------------------------------------
# phase_link — symlink 'current' dentro del prefix y symlink del lanzador en
# --bin-dir.
#
# El symlink de --bin-dir apunta a <prefix>/current/<entrypoint>, de modo que
# actualizar ropero es cambiar el destino de 'current' y el launcher del PATH
# sigue siendo el mismo fichero.
# -----------------------------------------------------------------------------
phase_link() {
    phase 4 "Enlazar"

    # 'current' -> ropero-<versión> (relativo: mover el prefix no lo rompe).
    if [[ -e "$PREFIX/current" && ! -L "$PREFIX/current" ]]; then
        if (( FORCE == 1 )); then
            rm -rf "$PREFIX/current" || die "no se pudo borrar $PREFIX/current"
        else
            die "$PREFIX/current existe y no es un symlink (usa --force para reemplazarlo)"
        fi
    fi
    if [[ -L "$PREFIX/current" ]]; then
        rm -f "$PREFIX/current" || true
    fi
    ln -s "$(basename "$TARGET_DIR")" "$PREFIX/current" \
        || die "no se pudo crear el symlink $PREFIX/current"
    ok "current -> $(basename "$TARGET_DIR")"

    if [[ "$NO_LINK" == "1" ]]; then
        info "--no-link: no se crea el symlink en $BIN_DIR"
        LINK_PATH=""
        return 0
    fi

    mkdir -p "$BIN_DIR" || die "no se pudo crear $BIN_DIR"
    LINK_PATH="$BIN_DIR/ropero"
    local target="$PREFIX/current/$ENTRY_REL"

    if [[ -L "$LINK_PATH" ]]; then
        local prev; prev="$(readlink "$LINK_PATH" 2>/dev/null || true)"
        if [[ "$prev" != "$target" && "$prev" != *"$PREFIX"* ]]; then
            warn "$LINK_PATH apuntaba a otra instalación ($prev); lo repunto a la nueva"
        fi
        rm -f "$LINK_PATH" || true
    elif [[ -e "$LINK_PATH" ]]; then
        # No es un symlink: NO lo pisamos salvo que lo pidan con --force.
        if (( FORCE == 1 )); then
            rm -rf "$LINK_PATH" || die "no se pudo reemplazar $LINK_PATH"
        else
            warn "$LINK_PATH ya existe y NO es un symlink a ropero: no lo he pisado"
            warn "  para reemplazarlo: vuelve a ejecutar con --force"
            LINK_SKIPPED=1
            LINK_PATH=""
            return 0
        fi
    fi

    ln -s "$target" "$LINK_PATH" || die "no se pudo crear el symlink $LINK_PATH"
    ok "$LINK_PATH -> $target"

    # ¿Está el directorio del symlink en el PATH? Avisamos, pero NO tocamos el
    # PATH del usuario a escondidas: eso es cosa de su .bashrc, no nuestra.
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *)
            warn "$BIN_DIR no está en tu PATH"
            printf '       export PATH="%s:$PATH"\n' "$BIN_DIR"
            ;;
    esac
}

# -----------------------------------------------------------------------------
# phase_verify — el sustituto del antiguo `ropero --check` para el launcher.
# Se ejecuta el binario recién instalado con --help y se exige salida 0.
# -----------------------------------------------------------------------------
phase_verify() {
    phase 5 "Verificar"

    local bin="$PREFIX/current/$ENTRY_REL"
    [[ -x "$bin" ]] || die "el lanzador no es ejecutable: $bin"

    local out rc
    set +e
    out="$("$bin" --help 2>&1 </dev/null)"
    rc=$?
    set -e
    if (( rc == 0 )); then
        ok "el lanzador responde (ropero --help → salida 0)"
    else
        err "ropero --help devolvió $rc"
        if [[ -n "$out" ]]; then printf '%s\n' "$out" | head -n 20 >&2; fi
        die "la instalación no funciona: revisa el repositorio o prueba otra versión"
    fi
}

# -----------------------------------------------------------------------------
# summary — qué se ha instalado, dónde y qué hacer ahora.
# -----------------------------------------------------------------------------
summary() {
    printf '\n%s══ ropero instalado ══%s\n' "$C_HEAD" "$C_OFF"
    printf '  versión:   %s\n' "$VERSION"
    printf '  instalado: %s\n' "$TARGET_DIR"
    printf '  current:   %s -> %s\n' "$PREFIX/current" "$(basename "$TARGET_DIR")"
    if [[ "$LINK_SKIPPED" == "1" ]]; then
        printf '  symlink:   NO creado (había algo ahí que no era un symlink)\n'
    elif [[ -n "$LINK_PATH" ]]; then
        printf '  symlink:   %s -> %s\n' "$LINK_PATH" "$PREFIX/current/$ENTRY_REL"
    else
        printf '  symlink:   no creado (--no-link)\n'
    fi
    printf '\n  Siguientes pasos:\n'
    printf '    ropero install engines   # compila/descarga el backend\n'
    printf '    ropero install models    # baja los modelos por defecto\n'
    printf '    ropero list              # lista los modelos disponibles\n'
    printf '\n  Para actualizar:\n'
    printf '    curl -fsSL %s/raw/main/install.sh | bash -s -- --force\n' "$ROPERO_REPO"
}

# -----------------------------------------------------------------------------
# main
# -----------------------------------------------------------------------------
main() {
    parse_args "$@"
    TARGET_DIR="$(_target_dir)"
    phase_check_system
    phase_download
    phase_extract
    phase_link
    phase_verify
    summary
}

main "$@"
