#!/usr/bin/env bash
# =============================================================================
# install.sh — instalador de ropero (contrato de instalación lasaca v2)
# =============================================================================
# LAYOUT QUE DEJA (idéntico en los 8 repos de lasaca):
#
#   <repo>/                    fuente de verdad: NO se mueve, NO se copia,
#                              NO pierde su .git. El origen ES el directorio
#                              donde vive este install.sh (R6).
#   ~/.local/bin/ropero        → <repo>/bin/ropero   symlink; LO ÚNICO que
#                              install.sh pone en el PATH (R1)
#   ~/.local/share/ropero      → <repo>/bin          symlink único; la share
#                              ES el bin del repo (R2). Los módulos (bin/lib)
#                              y los modelos (models/) se alcanzan desde el
#                              entrypoint, no desde la share.
#
# MATRIZ del repo:
#   NAME=ropero · ENTRY=bin/ropero · share → <repo>/bin · sin extras.
#   Env: ROPERO_DIR (def ~/.local/share/ropero), ROPERO_BIN_DIR (def
#   ~/.local/bin). Flags de QA: --dir <ruta>, --bin-dir <ruta>.
#
# REGLAS DURAS (resumen; el contrato completo vive en la spec lasaca v2):
#   R1  sólo symlinks: no copia, no descarga, no compila, no farms de
#       entradas, no borra el repo.
#   R2  ~/.local/share/<name> apunta SIEMPRE a <repo>/bin.
#   R3  idempotencia estricta: re-ejecutar no crea duplicados; un symlink
#       propio (incluso colgante o con target antiguo) se re-crea con
#       ln -sfn y target absoluto. `--check` tras la 2ª ejecución da 0.
#   R4  lo ajeno no se toca: un fichero REAL en ~/.local/bin/<name> se
#       avisa y no se pisa (exit 0 con aviso); un directorio REAL no
#       reconocido como instalación nuestra en la share es [✗] + exit 1.
#   R5  migración reconocida: el layout viejo se sustituye sólo tras
#       reconocerlo como nuestro; el borrado es mv a <dir>.old.$$ + rm -rf
#       de lo apartado (si rm falla, queda el .old.$$ y un [!]).
#   R6  sin red: no hay modo red, ni --tarball, ni --version, ni bootstrap
#       por stdin, ni staging.
#   R7  flags: (sin flags)/--install/--local instalan (--local es alias
#       silencioso del default); --check verifica sin modificar;
#       --dir/--bin-dir redirigen destinos; --help/-h sale 0. Otro
#       argumento: exit 2. --local + --check juntos: gana --check.
#   R8  salida: marcadores [✓]/[!]/[✗]/[-] a stderr, español, recuento
#       final "N ok · M avisos · K fallos". Exit 0 ok · 1 fallo · 2 uso.
#   R9  verificación post-install: sólo `bash <repo>/<ENTRY> --help`.
#   R10 bash 3.2 (macOS): sin readlink -f, sin mapfile, sin ${var^^}.
#   R11 el repo no se contamina: install.sh jamás escribe dentro de
#       <repo>/ (los engines y el venv de gguffer viven FUERA).
#   R12 --check es read-only (ver do_check abajo).
#   R13 si $BIN_DIR no está en $PATH: [!] + hint de export PATH.
#
# HOOK DE MIGRACIÓN PROPIO DE ROPERO (§7.3-7.4 de la spec): antes de borrar
# el farm viejo (~/.local/share/ropero real), se rescatan (a) los enlaces a
# binarios de engines → $BIN_DIR y (b) el venv de gguffer →
# $HOME/.local/state/ropero/venv. Nada de eso puede vivir bajo la share
# nueva: la share ES un symlink al bin del repo (R11).
#
# USO
#   install.sh [--install|--local] [--dir <ruta>] [--bin-dir <ruta>]
#   install.sh --check
#   install.sh --help
#
# Opciones de shell: SÓLO pipefail, SIN errexit ni nounset (un fallo
# implícito no puede abortar la instalación a medias sin mensaje; los
# abortos son explícitos: `|| die`).
# =============================================================================

set -o pipefail

# -----------------------------------------------------------------------------
# Parámetros del repo. Flag > variable de entorno > default.
# -----------------------------------------------------------------------------
NAME="ropero"
ENTRY="bin/ropero"          # entrypoint de la utilidad

DIR="${ROPERO_DIR:-${HOME:-}/.local/share/ropero}"
BIN_DIR="${ROPERO_BIN_DIR:-${HOME:-}/.local/bin}"

ACTION="install"            # install | check
LOCAL=0                     # --local: alias silencioso del default (R7)

ORIGIN=""                   # raíz del repo (dedonde vive este script)

# -----------------------------------------------------------------------------
# Salida: a stderr, marcadores [✓] [!] [✗] [-], español (R8).
# Color sólo si stderr es tty y NO_COLOR no está.
# -----------------------------------------------------------------------------
if [[ -t 2 && -z "${NO_COLOR:-}" ]]; then
    C_OK=$'\033[1;32m'; C_WARN=$'\033[1;33m'; C_ERR=$'\033[1;31m'
    C_HEAD=$'\033[1m'; C_OFF=$'\033[0m'
else
    C_OK=""; C_WARN=""; C_ERR=""; C_HEAD=""; C_OFF=""
fi

N_OK=0; N_WARN=0; N_FAIL=0

info() { printf '  %s[-]%s %s\n' "$C_HEAD" "$C_OFF" "$*" >&2; }
ok()   { printf '  %s[✓]%s %s\n' "$C_OK" "$C_OFF" "$*" >&2; N_OK=$((N_OK+1)); }
warn() { printf '  %s[!]%s %s\n' "$C_WARN" "$C_OFF" "$*" >&2; N_WARN=$((N_WARN+1)); }
err()  { printf '  %s[✗]%s %s\n' "$C_ERR" "$C_OFF" "$*" >&2; N_FAIL=$((N_FAIL+1)); }
phase() { printf '\n%s▸ %s%s\n' "$C_HEAD" "$*" "$C_OFF" >&2; }

resumen() {
    printf '\n── Resumen ──\n' >&2
    printf '  %s ok · %s avisos · %s fallos\n' "$N_OK" "$N_WARN" "$N_FAIL" >&2
}

# die <msg> — fallo real: exit 1.  die_uso <msg> — error de uso: exit 2.
die()     { err "$*"; resumen; exit 1; }
die_uso() { err "$*"; resumen; exit 2; }

uso() {
    cat <<'EOF'
USO: install.sh [--install|--local] [--dir <ruta>] [--bin-dir <ruta>]
     install.sh --check
     install.sh --help

Instala ropero con symlinks desde el repo donde vive este script (sin red,
sin copias): ~/.local/bin/ropero → <repo>/bin/ropero y
~/.local/share/ropero → <repo>/bin. Idempotente; migra el layout viejo
(farm/copias reales) tras reconocerlo como propio, rescatando antes los
engines enlazados y el venv de gguffer.

  --local           alias silencioso del comportamiento por defecto
  --check           verifica SIN modificar nada (exit 0/1)
  --dir <ruta>      destino de la share (def ~/.local/share/ropero)
  --bin-dir <ruta>  destino del entrypoint (def ~/.local/bin)
  --install         igual que el comportamiento por defecto

Env: ROPERO_DIR, ROPERO_BIN_DIR. Exit: 0 ok · 1 fallo · 2 uso.
EOF
}

# -----------------------------------------------------------------------------
# Utilidades de ruta (bash 3.2 / macOS: nada de readlink -f).
# Tope de 40 saltos: un ciclo de symlinks no puede colgar el instalador.
# -----------------------------------------------------------------------------

# real_dir <ruta> — directorio físico (siguiendo symlinks) de una ruta.
real_dir() {
    local p="$1" dir base saltos=0
    while [[ -L "$p" ]]; do
        dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
        base="$(readlink "$p")" || return 1
        [[ "$base" != /* ]] && base="$dir/$base"
        p="$base"
        saltos=$((saltos + 1))
        (( saltos > 40 )) && return 1
    done
    dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
    printf '%s\n' "$dir"
}

# real_path <ruta> — ruta absoluta física (siguiendo symlinks).
real_path() {
    local p="$1" dir base saltos=0
    while [[ -L "$p" ]]; do
        dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
        base="$(readlink "$p")" || return 1
        [[ "$base" != /* ]] && base="$dir/$base"
        p="$base"
        saltos=$((saltos + 1))
        (( saltos > 40 )) && return 1
    done
    dir="$(cd -P "$(dirname "$p")" 2>/dev/null && pwd)" || return 1
    printf '%s\n' "$dir/$(basename "$p")"
}

# -----------------------------------------------------------------------------
# FASE 1 — preparar: resolver el origen (el repo de este script, R6).
# -----------------------------------------------------------------------------
fase_preparar() {
    phase "preparar"
    local self="${BASH_SOURCE[0]:-$0}"
    ORIGIN="$(real_dir "$self")" || die "no se pudo resolver el directorio de este script"
    [[ -n "$ORIGIN" ]] || die "no se pudo resolver el directorio de este script"
    [[ -f "$ORIGIN/$ENTRY" ]] || die "el repo local no contiene $ENTRY: $ORIGIN/$ENTRY"
    ok "origen: $ORIGIN"
    return 0
}

# -----------------------------------------------------------------------------
# HOOKS DE MIGRACIÓN (sólo ropero, §7.3-7.4): antes de borrar el farm viejo
# se rescata lo que vive dentro y no puede quedar bajo la share nueva.
# -----------------------------------------------------------------------------

# _rescatar_engines <dir_viejo> — los enlaces a binarios de engines del farm
# viejo pasan a $BIN_DIR (donde ya viven los de uv/brew). Sólo si NO hay ya
# uno en $BIN_DIR (como fichero o como symlink): un rescatado nunca pisa.
# NOTA: no se consulta `command -v` a secas — con HOME sandbox el PATH real
# puede traer un engine de OTRO home y el rescate seguiría siendo correcto;
# lo que se evita es duplicar DENTRO del $BIN_DIR de esta instalación.
_rescatar_engines() {
    local viejo="$1" name real
    for name in llama-server llama-server-official vllm mlx-serve \
                trtllm-serve flm strata; do
        [[ -L "$viejo/$name" ]] || continue
        if [[ -e "$BIN_DIR/$name" ]]; then
            info "$name ya está en $BIN_DIR: no lo rescato"
            continue
        fi
        real="$(real_path "$viejo/$name" 2>/dev/null || true)"
        if [[ -z "$real" || ! -x "$real" ]]; then
            info "$name: enlace colgante en el farm viejo, no lo rescato"
            continue
        fi
        if ln -sfn "$real" "$BIN_DIR/$name" 2>/dev/null; then
            ok "engine rescatado: $BIN_DIR/$name → $real"
        else
            warn "no se pudo rescatar $name a $BIN_DIR/$name"
        fi
    done
    return 0
}

# _rescatar_venv <dir_viejo> — el venv de gguffer pasa a
# $HOME/.local/state/ropero/venv (junto al STATE_DIR del runtime). Si ya
# hay uno, el del farm se aparta con sufijo .farm.$$ (no se pisa nada).
_rescatar_venv() {
    local viejo="$1"
    local estado="$HOME/.local/state/ropero"
    [[ -d "$viejo/venv" && ! -L "$viejo/venv" ]] || return 0
    if ! mkdir -p "$estado" 2>/dev/null; then
        warn "no se pudo crear $estado: el venv del farm viejo se pierde al borrarlo"
        return 0
    fi
    if [[ -e "$estado/venv" ]]; then
        if mv "$viejo/venv" "$estado/venv.farm.$$" 2>/dev/null; then
            warn "ya había un venv en $estado/venv: el del farm viejo queda en $estado/venv.farm.$$"
        else
            warn "no se pudo apartar el venv del farm viejo (se pierde al borrarlo)"
        fi
        return 0
    fi
    if mv "$viejo/venv" "$estado/venv" 2>/dev/null; then
        ok "venv de gguffer rescatado: $estado/venv"
    else
        warn "no se pudo mover el venv ($viejo/venv → $estado/venv)"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# FASE 2 — migrar (§6): sustituir el layout viejo, SÓLO si se reconoce como
# nuestro. Deja $DIR listo para crear el symlink en la fase enlazar:
#   · no existe            → nada (se crea en enlazar)
#   · symlink canónico     → nada (idempotente, R3)
#   · symlink nuestro      → se re-crea en enlazar con ln -sfn
#   · symlink ajeno        → [!] y NO se toca (la share no se crea)
#   · dir real reconocido  → hooks de rescate + mv aparte + rm -rf (R5)
#   · dir real desconocido → [✗] + exit 1 (R4); fichero real → idem
# -----------------------------------------------------------------------------
SHARE_SKIP=0   # 1 si la share es un symlink ajeno: no se crea

fase_migrar() {
    phase "migrar"
    local dest="" rel=""
    local base_entry="$(basename "$ENTRY")"

    if [[ ! -e "$DIR" && ! -L "$DIR" ]]; then
        info "no hay instalación previa en $DIR"
        return 0
    fi

    if [[ -L "$DIR" ]]; then
        dest="$(real_dir "$DIR" 2>/dev/null || true)"
        if [[ -z "$dest" ]]; then
            # Colgante: sin destino no hay forma de saber de quién es. R3
            # manda: un symlink propio colgante se re-crea. Se re-crea.
            info "$DIR es un symlink colgante: se re-crea"
            return 0
        fi
        if [[ "$dest" == "$ORIGIN/bin" ]]; then
            ok "$DIR ya apunta a $ORIGIN/bin (idempotente)"
            return 0
        fi
        # Symlink al repo entero (contiene bin/ropero) o a un bin/ viejo
        # (contiene ropero): es nuestro, se re-crea canónico (§6.2).
        if [[ -e "$dest/$ENTRY" || -e "$dest/$base_entry" ]]; then
            info "$DIR es un symlink nuestro con target antiguo: se re-crea"
            return 0
        fi
        warn "$DIR es un symlink ajeno → $dest: no lo toco (la share no se crea)"
        SHARE_SKIP=1
        return 0
    fi

    if [[ -f "$DIR" ]]; then
        die "$DIR es un fichero real y no una instalación de $NAME: no lo toco"
    fi

    if [[ -d "$DIR" ]]; then
        # §6.3: copia real del modo red (a), farm local viejo (b) o (c).
        if [[ -f "$DIR/$ENTRY" ]] || [[ -L "$DIR/$NAME.d" ]] \
           || { [[ -d "$DIR/bin" && -L "$DIR/bin" ]]; }; then
            info "instalación vieja reconocida en $DIR: migrando"
            # Hooks de ropero ANTES de borrar nada (§10.5.1).
            _rescatar_engines "$DIR"
            _rescatar_venv "$DIR"
            local aparte="$DIR.old.$$"
            if ! mv "$DIR" "$aparte" 2>/dev/null; then
                die "no se pudo apartar $DIR a $aparte"
            fi
            if ! rm -rf "$aparte" 2>/dev/null; then
                warn "no se pudo borrar $aparte (revisa y elimínalo a mano)"
            else
                ok "instalación vieja sustituida (apartada y borrada: $aparte)"
            fi
            return 0
        fi
        die "$DIR ya existe y NO es una instalación de $NAME: no lo toco"
    fi

    die "$DIR existe y no sé qué es: no lo toco"
}

# -----------------------------------------------------------------------------
# FASE 3 — enlazar: el entrypoint en $BIN_DIR y la share (R1-R2).
# -----------------------------------------------------------------------------
fase_enlazar() {
    phase "enlazar: $BIN_DIR/$NAME y $DIR"
    mkdir -p "$BIN_DIR" 2>/dev/null || die "no se pudo crear $BIN_DIR"
    mkdir -p "$(dirname "$DIR")" 2>/dev/null || die "no se pudo crear $(dirname "$DIR")"

    # ── El entrypoint (§6.5): lo ÚNICO que install.sh pone en el PATH.
    local link="$BIN_DIR/$NAME"
    local target="$ORIGIN/$ENTRY"
    if [[ -e "$link" && ! -L "$link" ]]; then
        # R4: un fichero REAL ahí (otro programa, un binario copiado a
        # mano): no se pisa en silencio. La decisión es del usuario.
        warn "$link existe y NO es un symlink: no lo he tocado"
    else
        # R3: symlink propio, colgante o con target antiguo → ln -sfn.
        if ln -sfn "$target" "$link" 2>/dev/null; then
            ok "$link → $target"
        else
            die "no se pudo crear el symlink $link"
        fi
    fi

    # ── La share (R2): ~/.local/share/<name> → <repo>/bin.
    if [[ "$SHARE_SKIP" == "1" ]]; then
        info "share omitida (symlink ajeno); el bin link sí está creado"
    elif [[ -L "$DIR" && "$(real_dir "$DIR" 2>/dev/null || true)" == "$ORIGIN/bin" ]]; then
        ok "$DIR ya apunta a $ORIGIN/bin"
    else
        if [[ -e "$DIR" && ! -L "$DIR" ]]; then
            # No debería llegar aquí (migrar exit 1 antes), por si acaso.
            die "$DIR existe y NO es un symlink: no lo toco"
        fi
        if ln -sfn "$ORIGIN/bin" "$DIR" 2>/dev/null; then
            ok "$DIR → $ORIGIN/bin"
        else
            die "no se pudo crear el symlink $DIR"
        fi
    fi

    # ── Limpieza legacy en $BIN_DIR (§6.6: sólo symlinks, nunca reales).
    local legacy="$BIN_DIR/$NAME.d"
    if [[ -L "$legacy" ]]; then
        rm -f "$legacy" 2>/dev/null || true
        ok "resto del layout viejo eliminado: $legacy"
    elif [[ -e "$legacy" ]]; then
        warn "$legacy existe y NO es un symlink: no lo he tocado"
    fi

    # R13: el PATH del usuario es suyo; sólo se avisa.
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
# FASE 4 — verificar (R9): el entrypoint responde a --help. No se ejecuta
# nada más (ni compila, ni descarga, ni arranca motores).
# -----------------------------------------------------------------------------
fase_verificar() {
    phase "verificar"
    local rc=0
    bash "$ORIGIN/$ENTRY" --help >/dev/null 2>&1 </dev/null
    rc=$?
    if (( rc == 0 )); then
        ok "el lanzador responde ($NAME --help → salida 0)"
    else
        die "$NAME --help devolvió $rc: la instalación no funciona"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# --check (R12): verifica SIN instalar ni modificar nada. Exit 0/1.
#   (1) $BIN_DIR/<name> es symlink y resuelve a un ejecutable existente.
#   (2) la share es symlink cuyo destino resuelto está DENTRO del repo de
#       la utilidad (mismo origen que el entrypoint).
#   (3) [sólo tools: common.sh] — no aplica a ropero.
#   (4) bash <entry> --help sale 0.
#   (5) avisos de restos legacy ($BIN_DIR/<name>.d, $DIR.old.*).
# -----------------------------------------------------------------------------
do_check() {
    phase "comprobar el layout"
    local rc=0 link="$BIN_DIR/$NAME" phys="" repo="" dest="" resto

    # (1) entrypoint: symlink a un ejecutable existente.
    if [[ -L "$link" ]]; then
        phys="$(real_path "$link" 2>/dev/null || true)"
        if [[ -n "$phys" && -f "$phys" && -x "$phys" ]]; then
            ok "entrypoint: $link → $phys"
            # El repo esperado: el origen del PROPIO entrypoint instalado
            # (igual que el de este script si todo viene del mismo sitio).
            case "$phys" in
                */$ENTRY) repo="${phys%/$ENTRY}" ;;
                *)        repo="$ORIGIN" ;;
            esac
        else
            err "$link es un symlink roto (→ $(readlink "$link" 2>/dev/null))"
            rc=1
        fi
    elif [[ -e "$link" ]]; then
        err "$link existe y NO es un symlink (el instalador no lo toca)"
        rc=1
    else
        err "$link no existe: $NAME no está instalado"
        rc=1
    fi
    [[ -n "$repo" ]] || repo="$ORIGIN"

    # (2) share: symlink cuyo destino resuelto está dentro del repo.
    if [[ -L "$DIR" ]]; then
        dest="$(real_dir "$DIR" 2>/dev/null || true)"
        if [[ -n "$dest" ]]; then
            if [[ "$dest" == "$repo" || "$dest" == "$repo"/* ]]; then
                ok "share: $DIR → $dest"
            else
                err "share: $DIR → $dest (fuera del repo $repo)"
                rc=1
            fi
        else
            err "share: $DIR es un symlink roto (→ $(readlink "$DIR" 2>/dev/null))"
            rc=1
        fi
    elif [[ -e "$DIR" ]]; then
        err "share: $DIR existe y NO es un symlink (layout viejo o ajeno)"
        rc=1
    else
        err "share: $DIR no existe"
        rc=1
    fi

    # (4) el entrypoint instalado responde.
    local entry="$phys"
    [[ -n "$entry" && -f "$entry" ]] || entry="$ORIGIN/$ENTRY"
    if bash "$entry" --help >/dev/null 2>&1 </dev/null; then
        ok "$NAME --help responde (salida 0)"
    else
        err "$NAME --help NO responde"
        rc=1
    fi

    # (5) restos legacy del layout viejo (avisos, no fallos).
    if [[ -e "$BIN_DIR/$NAME.d" ]]; then
        warn "resto legacy: $BIN_DIR/$NAME.d (el layout v2 no lo usa)"
    fi
    for resto in "$DIR".old.*; do
        [[ -e "$resto" || -L "$resto" ]] || continue
        warn "resto de una migración: $resto (puedes borrarlo)"
    done

    if (( rc == 0 )); then
        ok "check de $NAME en orden"
    else
        err "el check encontró problemas (ver arriba)"
    fi
    resumen
    return "$rc"
}

# -----------------------------------------------------------------------------
# main
# -----------------------------------------------------------------------------
main() {
    local arg=""
    while (( $# > 0 )); do
        arg="$1"
        case "$arg" in
            --local|--install)
                LOCAL=1 ;;
            --check)
                ACTION="check" ;;
            --dir)
                (( $# >= 2 )) || die_uso "--dir necesita una ruta"
                DIR="$2"; shift ;;
            --dir=*)
                DIR="${arg#--dir=}" ;;
            --bin-dir)
                (( $# >= 2 )) || die_uso "--bin-dir necesita una ruta"
                BIN_DIR="$2"; shift ;;
            --bin-dir=*)
                BIN_DIR="${arg#--bin-dir=}" ;;
            --help|-h)
                uso; exit 0 ;;
            --tarball|--version)
                die_uso "$arg ya no existe: el instalador es symlink-only (sin red)" ;;
            *)
                die_uso "opción desconocida: '$arg' (ver --help)" ;;
        esac
        shift
    done
    # R7: --local + --check juntos → gana --check.

    fase_preparar

    if [[ "$ACTION" == "check" ]]; then
        do_check
        return $?
    fi

    fase_migrar
    fase_enlazar
    fase_verificar
    resumen
    return 0
}

main "$@"
exit $?
