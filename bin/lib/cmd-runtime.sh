#!/usr/bin/env bash
# =============================================================================
# cmd-runtime.sh — comandos de runtime: arranque directo, use, stop, status
# (spec §8.2, §8.3, §8.10 y §8.11, fase 4)
# =============================================================================
# Carga lazy: el dispatcher sourcea ESTE fichero al enrutar use/stop/status o
# un arranque directo de modelo. Aquí se sourcea 04-launch.sh (que arrastra la
# cadena completa: 02-modelconf → 03-engines → 04-launch) y nada más.
#
# Destino de la salida (contrato §5 + plugin de opencode, ADR-4):
#   · stdout: SOLO la línea máquina `ROPERO_PORT=<n>` (printf crudo, sin
#     ANSI), como ÚLTIMA línea tras un arranque exitoso en background (§8.2)
#     y en el caso idempotente de `use` (§8.3).
#   · stderr: todo lo humano (progreso, tablas, errores) vía rp_msg.
#
# bash 3.2: sin namerefs, sin arrays asociativos, sin mapfile (spec §3).
# Seguro de sourcear: sólo definiciones, cero ejecución.
# =============================================================================

if ! declare -F rp_start_model >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/04-launch.sh"
fi

# -----------------------------------------------------------------------------
# Arranque directo: ropero <modelo> [opciones] (§8.2)
# -----------------------------------------------------------------------------
_rp_start_help() {
    cat <<'EOF'
USO: ropero <modelo> [--port N] [--cpu] [--background] [--force]
     ropero <modelo> [--<nivel>] [--<flag>] [--<opcion>=<valor>]...

Sirve el modelo en PRIMER plano (Ctrl-C lo detiene y sale 130). Con
--background espera a que el modelo responda, devuelve el control e imprime
`ROPERO_PORT=<puerto>` como última línea de stdout (la salida máquina que
consumen scripts y el plugin de opencode).

Opciones:
  --port N         puerto donde servir (--port=N también vale; repetido,
                   gana el último, con aviso)
  --cpu            slot CPU (sólo Linux: en macOS es error de uso)
  --background     sirve en background cuando esté listo
  --force          si el puerto destino está ocupado, lo libera
  --<nivel>        nivel de razonamiento declarado en [levels]
  --<flag>         flag booleano declarado en [flags]
  --<opcion>=<v>   opción con valor declarada en [options]

Niveles, flags y opciones deben estar DECLARADOS en el TOML del modelo; si
no, es error de uso (exit 2). `ropero args <modelo>` muestra el argv sin
arrancar nada.

Ejemplos:
  ropero qwen38
  ropero qwen38 --high --background
  ropero gptoss --port 9995 --temp=0.5
EOF
}

rp_cmd_start() {
    local modelo="${1:-}"
    [[ -n "$modelo" ]] || rp_usage_error '' "falta el modelo (ropero <modelo>)"
    shift

    rp_mc_load || return 1

    # Resolución nombre/alias → fila del catálogo. Desconocido = typo casi
    # seguro: error de USO, no de ejecución (§8.2).
    local idx
    idx="$(_rp_mc_idx "$modelo")" || \
        rp_usage_error '' "modelo desconocido: '$modelo' (ropero list)"
    local real="${MODEL_REAL[$idx]}"

    # Lo que el modelo DECLARA (niveles/flags/opciones): la validación de
    # uso es contra el TOML, aquí y una sola vez (port del argparse del
    # legado, la versión 0.x (historial git)).
    local niveles="${MODEL_LEVELS[$idx]}"
    local flags_decl="${MODEL_FLAGS[$idx]}"
    local opciones="${MODEL_OPTIONS[$idx]}"

    local bg=0 cpu=0 force=0 port="" nivel="" mflags="" mopts=""
    local want want_val
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --background) bg=1 ;;
            --cpu)
                # Slot CPU = Linux (spec §4). En macOS no existe el slot:
                # error de USO, no un warning que lo ignore.
                if [[ "$(uname -s)" != "Linux" ]]; then
                    rp_usage_error '' "--cpu sólo existe en Linux (en macOS no hay slot CPU)"
                fi
                cpu=1
                ;;
            --force) force=1 ;;
            --port)
                # El shift de este caso se come el VALOR y el del final del
                # bucle el flag: sin el continue, `--port` al final se come
                # un shift de más (bug histórico del legado, documentado).
                shift
                if [[ $# -gt 0 && "$1" =~ ^[0-9]+$ ]]; then
                    # Repetido: gana el último, pero se dice.
                    [[ -n "$port" ]] && rp_msg warn "--port repetido; gana el último (:$1)"
                    port="$1"
                    shift
                else
                    rp_usage_error '' "--port requiere un número de puerto${1:+ (recibido: '$1')}"
                fi
                continue
                ;;
            --port=*)
                port="${1#--port=}"
                [[ "$port" =~ ^[0-9]+$ ]] || \
                    rp_usage_error '' "--port=$port no es un puerto válido"
                ;;
            -h|--help)
                _rp_start_help
                return 0
                ;;
            --?*)
                # Cosa del MODELO: nivel (sin valor), opción (con valor) o
                # flag. Sin declarar → error de uso, no se ignora: si
                # `ropero x --vision` no encontrara el mmproj y siguiera,
                # el fallo aparecería en la primera imagen, no al arrancar.
                want="${1#--}"
                want_val=""
                if [[ "$want" == *=* ]]; then
                    want_val="${want#*=}"
                    want="${want%%=*}"
                fi
                if [[ -n "$niveles" && " $niveles " == *" $want "* ]]; then
                    [[ -n "$nivel" && "$nivel" != "$want" ]] && \
                        rp_msg warn "nivel repetido: '$nivel' → '$want' (gana el último)"
                    nivel="$want"
                elif [[ -n "$opciones" && " $opciones " == *" $want "* ]]; then
                    if [[ -z "$want_val" ]]; then
                        shift
                        [[ $# -gt 0 ]] || rp_usage_error '' "la opción --$want requiere un valor"
                        want_val="$1"
                    fi
                    mopts+="${mopts:+ }${want}=${want_val}"
                elif [[ -n "$flags_decl" && " $flags_decl " == *" $want "* ]]; then
                    mflags+="${mflags:+ }$want"
                else
                    rp_usage_error '' "'--$want' no está declarado para '$real' (niveles: ${niveles:-(ninguno)} · flags: ${flags_decl:-(ninguno)} · opciones: ${opciones:-(ninguna)})"
                fi
                ;;
            *)
                rp_usage_error '' "opción desconocida: '$1'"
                ;;
        esac
        shift
    done

    # shellcheck disable=SC2086  # mflags/mopts: listas de tokens del TOML,
    # separadas por espacio, sin espacios dentro (misma convención que §5.5)
    rp_start_model "$real" "$bg" "$cpu" "$force" "$port" "$nivel" "$mflags" "$mopts" \
        || return $?

    # La salida máquina del arranque en background (§8.2): ÚLTIMA línea de
    # stdout, cruda. El puerto es el que resolvió el arranque (--port >
    # pineado > slot).
    if [[ "$bg" == "1" ]]; then
        local puerto
        puerto="$(_rp_launch_puerto_destino "$real" "$port" "$cpu")" || {
            rp_msg error "no se pudo resolver el puerto destino de '$real'"
            return 1
        }
        printf 'ROPERO_PORT=%s\n' "$puerto"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# use <modelo> [--port N] [--nivel L] [--flag F]... (§8.3)
#
# CONTRATO DEL PLUGIN DE OPENCODE (ADR-4): idempotente por (modelo, nivel) y
# última línea stdout `ROPERO_PORT=<n>` cruda. NO romper.
# -----------------------------------------------------------------------------
_rp_use_help() {
    cat <<'EOF'
USO: ropero use <modelo> [--port N] [--nivel L] [--flag F]...

Asegura que el modelo pedido SEA el que se sirve en el puerto destino y
espera a que esté listo (no sólo al puerto: al MODELO). Respeta
ROPERO_START_TIMEOUT.

Es IDEMPOTENTE por (modelo, nivel): si el puerto ya sirve ese modelo con ese
nivel, no se toca nada, se imprime `✓ '<modelo>' ya está sirviéndose en :<p>`
y `ROPERO_PORT=<puerto>` sale como última línea de stdout. Con otro nivel se
reinicia. Si el puerto lo ocupa OTRO modelo, se lo para POR PUERTO y se
arranca el pedido.

Opciones:
  --port N     puerto destino (default: el pineado del TOML o el slot base)
  --nivel L    nivel de razonamiento declarado en [levels]
  --flag F     flag booleano declarado en [flags]

Exit: 0 servido · 1 fallo de arranque · 2 uso o modelo desconocido.
EOF
}

rp_cmd_use() {
    local a modelo="" port="" nivel="" mflags=""
    for a in "$@"; do
        [[ "$a" == "--help" || "$a" == "-h" ]] && { _rp_use_help; return 0; }
    done

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --port)
                [[ $# -ge 2 ]] || rp_usage_error use "--port requiere un número de puerto"
                port="$2"; shift 2 ;;
            --port=*)
                port="${1#--port=}"; shift ;;
            --nivel)
                [[ $# -ge 2 ]] || rp_usage_error use "--nivel requiere un valor"
                nivel="$2"; shift 2 ;;
            --flag)
                [[ $# -ge 2 ]] || rp_usage_error use "--flag requiere un valor"
                mflags+="${mflags:+ }$2"; shift 2 ;;
            --*)
                rp_usage_error use "opción desconocida: '$1' (use acepta --port, --nivel y --flag)" ;;
            *)
                [[ -z "$modelo" ]] || \
                    rp_usage_error use "use acepta un solo modelo; recibidos: '$modelo' y '$1'"
                modelo="$1"; shift ;;
        esac
    done
    [[ -n "$modelo" ]] || rp_usage_error use "falta el modelo (ropero use <modelo>)"
    [[ "$port" =~ ^[0-9]+$ ]] || [[ -z "$port" ]] || \
        rp_usage_error use "--port requiere un número de puerto (recibido: '$port')"

    rp_mc_load || return 1
    local idx
    idx="$(_rp_mc_idx "$modelo")" || \
        rp_usage_error use "modelo desconocido: '$modelo' (ropero list)"
    local real="${MODEL_REAL[$idx]}"

    # Nivel y flags: validados contra el TOML (mismo criterio que §8.2).
    local niveles="${MODEL_LEVELS[$idx]}" flags_decl="${MODEL_FLAGS[$idx]}" f
    if [[ -n "$nivel" ]]; then
        case " $niveles " in
            *" $nivel "*) ;;
            *) rp_usage_error use "'$nivel' no es un nivel de '$real' (niveles: ${niveles:-(ninguno)})" ;;
        esac
    fi
    for f in $mflags; do
        case " $flags_decl " in
            *" $f "*) ;;
            *) rp_usage_error use "'--$f' no es un flag de '$real' (flags: ${flags_decl:-(ninguno)})" ;;
        esac
    done

    # Nivel EFECTIVO: el pedido o el default del TOML. Es la clave de la
    # idempotencia: `use x` sin --nivel pide el nivel default, y si el
    # puerto sirve x con OTRO nivel, se reinicia (lo espera el plugin).
    local nivel_efectivo="$nivel"
    [[ -n "$nivel_efectivo" ]] || nivel_efectivo="${MODEL_LEVEL_DEFAULT[$idx]}"

    local puerto
    puerto="$(_rp_launch_puerto_destino "$real" "$port" 0)" || {
        rp_msg error "no se pudo resolver el puerto destino de '$real'"
        return 1
    }

    # ── Idempotencia por (modelo, nivel) ──────────────────────────────────
    if rp_port_listening "$puerto"; then
        local actual="" nivel_estado=""
        actual="$(rp_model_on_port "$puerto" 2>/dev/null)" || actual=""
        if [[ "$actual" == "$real" ]]; then
            nivel_estado="$(rp_state_read "$puerto" LEVEL)" || nivel_estado=""
            if [[ "$nivel_estado" == "$nivel_efectivo" ]]; then
                rp_msg ok "'$real' ya está sirviéndose en :$puerto"
                printf 'ROPERO_PORT=%s\n' "$puerto"
                return 0
            fi
            rp_msg info "'$real' corre en :$puerto al nivel '${nivel_estado:-?}' (pedido: '${nivel_efectivo:-default}'); se reinicia"
            rp_stop_port "$puerto" || true
        else
            # Otra cosa en el puerto destino: se para POR PUERTO, no por
            # nombre (parar por nombre mataría en el primer puerto que
            # encaje, que puede no ser este).
            rp_msg info "parando '${actual:-?}' en :$puerto (es el puerto de '$real')"
            rp_stop_port "$puerto" || true
        fi
    fi

    # El arranque en background ya espera al MODELO y respeta
    # ROPERO_START_TIMEOUT (§5.5).
    # shellcheck disable=SC2086  # mflags: lista de tokens del TOML
    rp_start_model "$real" 1 0 0 "$port" "$nivel" "$mflags" "" || return $?
    printf 'ROPERO_PORT=%s\n' "$puerto"
    return 0
}

# -----------------------------------------------------------------------------
# stop [modelo|all] (§8.10)
# -----------------------------------------------------------------------------
_rp_stop_help() {
    cat <<'EOF'
USO: ropero stop [modelo|all]

Detiene lo gestionado POR PUERTO (nunca pkill global: mataría slots
paralelos). Sin argumentos (o `all`): TODOS los puertos gestionados; si no
hay nada activo, imprime "Nada que detener" y sale 0. Con un modelo: busca
su puerto entre los gestionados y mata SÓLO ese.

Acepta como mucho UN argumento y no tiene opciones. Modelo desconocido →
exit 2 (es un typo, no un fallo de runtime).

Exit: 0 · 1 si el modelo pedido no se está sirviendo · 2 uso.
EOF
}

rp_cmd_stop() {
    local a
    for a in "$@"; do
        [[ "$a" == "--help" || "$a" == "-h" ]] && { _rp_stop_help; return 0; }
    done
    (( $# <= 1 )) || \
        rp_usage_error stop "stop acepta como mucho un argumento (modelo o 'all'); recibidos $#: $*"
    local target="${1:-all}"
    [[ "$target" != -* ]] || \
        rp_usage_error stop "stop no tiene opciones; '$target' no es un modelo (usa 'stop all')"

    local real=""
    if [[ "$target" != "all" ]]; then
        rp_mc_load || return 1
        local idx
        idx="$(_rp_mc_idx "$target")" || \
            rp_usage_error stop "modelo desconocido: '$target' (ropero list)"
        real="${MODEL_REAL[$idx]}"
    fi

    local p actual detenidos=0
    if [[ "$target" == "all" ]]; then
        while IFS= read -r p; do
            [[ -n "$p" ]] || continue
            if rp_port_listening "$p"; then
                actual="$(rp_model_on_port "$p" 2>/dev/null)" || actual=""
                rp_stop_port "$p" || true
                rp_msg ok ":$p detenido ('${actual:-?}')"
                detenidos=$((detenidos + 1))
            else
                # Estado huérfano (crash de un arranque anterior): sin esto
                # el puerto seguiría "gestionado" para siempre.
                rp_state_clear "$p"
            fi
        done < <(rp_managed_ports)
        if (( detenidos == 0 )); then
            rp_msg info "Nada que detener (ningún puerto gestionado activo)"
        else
            rp_msg ok "Detenidos $detenidos slot(s)"
        fi
        return 0
    fi

    # stop <modelo>: iterar TODOS los gestionados (un --port ad-hoc o un
    # [model].port pineado pueden estar fuera de los slots base).
    while IFS= read -r p; do
        [[ -n "$p" ]] || continue
        rp_port_listening "$p" || continue
        actual="$(rp_model_on_port "$p" 2>/dev/null)" || actual=""
        if [[ "$actual" == "$real" ]]; then
            rp_stop_port "$p" || {
                rp_msg error "no se pudo parar :$p"
                return 1
            }
            rp_msg ok "modelo '$real' detenido en :$p"
            return 0
        fi
    done < <(rp_managed_ports)
    rp_msg error "'$target' no se está sirviendo en ningún puerto gestionado"
    return 1
}

# -----------------------------------------------------------------------------
# status [modelo] [--watch] (§8.11)
# -----------------------------------------------------------------------------
_rp_status_help() {
    cat <<'EOF'
USO: ropero status [modelo] [--watch]

Sin argumentos: tabla de los puertos gestionados — PUERTO · ENGINE · MODELO ·
ESTADO (probe real del engine, no sólo "escucha"). Funciona sin alaja y sin
ANSI si NO_COLOR o stderr no es TTY.

  status <modelo>   exit 0 si el modelo se está sirviendo, 1 si no,
                    2 si es desconocido
  --watch           refresca la tabla cada 2s hasta Ctrl-C (exit 0)

`status <modelo>` no acepta --watch (es un check puntual, no un monitor).
EOF
}

# _rp_status_tabla — la tabla de puertos gestionados (todo a stderr: es
# presentación humana; §5).
_rp_status_tabla() {
    rp_header "puertos gestionados de ropero"
    printf '%-8s %-15s %-18s %s\n' 'PUERTO' 'ENGINE' 'MODELO' 'ESTADO' >&2
    local p modelo engine estado
    while IFS= read -r p; do
        [[ -n "$p" ]] || continue
        if ! rp_port_listening "$p"; then
            printf '%-8s %-15s %-18s %s\n' ":$p" '-' '-' 'libre' >&2
            continue
        fi
        modelo="$(rp_model_on_port "$p" 2>/dev/null)" || modelo="?"
        engine="$(rp_state_read "$p" ENGINE)" || engine="?"
        estado='sin responder'
        if [[ "$engine" != "?" ]] && \
           [[ "$(rp_engine_call "$engine" probe "$p" 2>/dev/null)" == "ok" ]]; then
            estado='sirviendo'
        fi
        printf '%-8s %-15s %-18s %s\n' ":$p" "$engine" "$modelo" "$estado" >&2
    done < <(rp_managed_ports)
    return 0
}

rp_cmd_status() {
    local a watch=0
    local -a posiciones=()
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_status_help; return 0 ;;
            --watch)   watch=1 ;;
            --*)       rp_usage_error status "opción desconocida: '$a' (sólo --watch)" ;;
            *)         posiciones+=("$a") ;;
        esac
    done
    (( ${#posiciones[@]} <= 1 )) || \
        rp_usage_error status "status acepta un modelo como máximo (recibidos: ${posiciones[*]})"

    local modelo=""
    if (( ${#posiciones[@]} > 0 )); then
        modelo="${posiciones[0]}"
    fi

    if [[ -n "$modelo" ]]; then
        (( watch == 0 )) || \
            rp_usage_error status "status <modelo> no acepta --watch (es un check puntual)"
        rp_mc_load || return 1
        local idx
        idx="$(_rp_mc_idx "$modelo")" || \
            rp_usage_error status "modelo desconocido: '$modelo' (ropero list)"
        local real="${MODEL_REAL[$idx]}"
        local p actual
        while IFS= read -r p; do
            [[ -n "$p" ]] || continue
            rp_port_listening "$p" || continue
            actual="$(rp_model_on_port "$p" 2>/dev/null)" || actual=""
            if [[ "$actual" == "$real" ]]; then
                rp_msg ok "'$real' se está sirviendo en :$p"
                return 0
            fi
        done < <(rp_managed_ports)
        rp_msg warn "'$real' no se está sirviendo en ningún puerto gestionado"
        return 1
    fi

    if [[ "$watch" == "1" ]]; then
        # Ctrl-C = "basta", no un error: exit 0 (§8.11). El sleep hace que
        # el trap dispare con ≤2s de latencia.
        trap 'printf "\n" >&2; rp_msg info "status --watch detenido"; exit 0' INT
        while :; do
            _rp_status_tabla
            sleep 2
        done
    fi
    _rp_status_tabla
    return 0
}
