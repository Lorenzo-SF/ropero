#!/usr/bin/env bash
# =============================================================================
# cmd-catalog.sh — comandos de catálogo: list, template, args, levels, path
# (spec §8.4–§8.8, fase 2)
# =============================================================================
# Carga lazy: el dispatcher sourcea ESTE fichero al enrutar uno de estos
# comandos. Aquí se sourcea la dependencia de la cadena (02-modelconf) y nada
# más. La GRAMÁTICA de CLI (qué flags acepta cada comando, qué está declarado
# en el TOML) vive aquí; el montaje del argv vive en modelconf.
#
# Destino de la salida (contrato §5 + plugin de opencode):
#   · stdout: lo que consumen scripts/plugin — las líneas de `list`, los
#     niveles, el argv de `args`, las KEY=ruta de `path`, el volcado de
#     `template`. printf crudo, CERO presentación.
#   · stderr: todo lo humano (cabeceras, avisos, errores) vía rp_msg.
# =============================================================================

if ! declare -F rp_mc_load >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/02-modelconf.sh"
fi

# -----------------------------------------------------------------------------
# list [--all] — una línea por modelo: DOS ESPACIOS + <name> + columnas.
# El plugin de opencode contrata el prefijo (regex ^\s{2}[A-Za-z0-9._-]+\s):
# el nombre va con relleno PERO el formato lleva un espacio literal tras el
# campo, así que la regex aguanta nombres de cualquier longitud.
# -----------------------------------------------------------------------------
_rp_catalog_list_help() {
    cat <<'EOF'
USO: ropero list [--all]

Lista los modelos del catálogo (models/), uno por línea:
  <name>  engine=<motor>  format=<gguf|safetensors>  ctx=<tokens>  <tamaño>

Con --all incluye los modelos retirados (models/<engine>/fired/), marcados
como "retirado": existen en el catálogo pero no se pueden arrancar.

Opciones:
  --all    incluye los modelos retirados
  --help   esta ayuda
EOF
}

rp_cmd_list() {
    local todo=0 a
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_catalog_list_help; return 0 ;;
            --all)     todo=1 ;;
            *)         rp_usage_error list "list no acepta '$a' (sólo --all)" ;;
        esac
    done

    rp_mc_load || return 1

    if [[ "$todo" == "1" ]]; then
        rp_header "Catálogo de modelos (activos + retirados)"
    else
        rp_header "Catálogo de modelos (activos)"
    fi

    local i nombre motor fmt ctx desc extra n=0
    for (( i = 0; i < ${#MODEL_NAMES[@]}; i++ )); do
        # Las filas de alias no son modelos: el alias ya sale en su columna.
        [[ "${MODEL_ISALIAS[$i]}" == "0" ]] || continue
        # Sin --all sólo se listan los activos (§8.7).
        [[ "$todo" == "1" || "${MODEL_ORIGIN[$i]}" == "active" ]] || continue

        nombre="${MODEL_NAMES[$i]}"
        motor="${MODEL_ENGINE[$i]}"
        fmt="${MODEL_FORMAT[$i]}"
        ctx="${MODEL_CONTEXT[$i]}"
        # size/desc: el tamaño declarado si lo hay; si no, lo que identifica
        # al modelo en disco (el gguf o la ruta del checkpoint).
        desc="${MODEL_SIZE[$i]}"
        if [[ -z "$desc" ]]; then
            desc="${MODEL_GGUF[$i]}"
        fi
        if [[ -z "$desc" ]]; then
            desc="${MODEL_PATH[$i]}"
        fi
        [[ -n "$desc" ]] || desc="-"

        extra=""
        [[ -n "${MODEL_ALIAS[$i]}" ]] && extra="alias=${MODEL_ALIAS[$i]} "
        [[ "${MODEL_ORIGIN[$i]}" == "fired" ]] && extra="${extra}retirado"

        printf '  %-16s engine=%-13s format=%-12s ctx=%-7s %s%s\n' \
            "$nombre" "$motor" "$fmt" "$ctx" "$extra" "$desc"
        n=$((n + 1))
    done

    if [[ "$n" == "0" ]]; then
        rp_msg warn "el catálogo no tiene modelos que listar"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# template — vuelca models/_template.toml a stdout TAL CUAL (§8.8, decisión
# 11: sin flags). El fichero es documentación viva del esquema.
# -----------------------------------------------------------------------------
_rp_catalog_template_help() {
    cat <<'EOF'
USO: ropero template

Vuelca models/_template.toml a stdout: el esquema TOML completo de un modelo
(todas las secciones, claves, placeholders y un ejemplo mínimo real). Cópialo
a models/<engine>/<nombre>.toml y ajústalo. No acepta opciones.
EOF
}

rp_cmd_template() {
    local a
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_catalog_template_help; return 0 ;;
            *)         rp_usage_error template "template no acepta argumentos (recibido: '$a')" ;;
        esac
    done
    local f="$RP_MODELS_DIR/_template.toml"
    if [[ ! -f "$f" ]]; then
        rp_msg error "no encuentro la plantilla: $f"
        return 1
    fi
    rp_msg info "plantilla del esquema TOML (cópiala a models/<engine>/<nombre>.toml)"
    cat "$f"
    return 0
}

# -----------------------------------------------------------------------------
# args <modelo> [--<nivel>] [--<flag>] [--<opt>=v]... — el argv final que
# recibiría el server, un argumento por línea, SIN arrancar nada (§8.4).
# Misma gramática que el arranque directo: lo que no está DECLARADO en el
# TOML del modelo es error de uso (exit 2), no se ignora.
# -----------------------------------------------------------------------------
_rp_catalog_args_help() {
    cat <<'EOF'
USO: ropero args <modelo> [--<nivel>] [--<flag>] [--<opcion>=<valor>]...

Imprime el argv final del modelo (el que recibiría el server), un argumento
por línea. Es el modo de verificar el wiring sin GPU. Niveles, flags y
opciones deben estar declarados en el TOML del modelo ([levels]/[flags]/
[options]); si no, es error de uso.

Ejemplos:
  ropero args qwen38
  ropero args qwen38 --high
  ropero args gptoss --flash --temp=0.5
EOF
}

rp_cmd_args() {
    local modelo="" a
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_catalog_args_help; return 0 ;;
        esac
    done
    [[ $# -ge 1 && -n "$1" ]] || rp_usage_error args "falta el modelo (ropero args <modelo>)"
    modelo="$1"
    shift

    rp_mc_load || return 1
    rp_model_exists "$modelo" || \
        rp_usage_error args "modelo desconocido: '$modelo' (ropero list)"

    local idx
    idx=$(_rp_mc_idx "$modelo")
    local niveles="${MODEL_LEVELS[$idx]}"
    local flags="${MODEL_FLAGS[$idx]}"
    local opciones="${MODEL_OPTIONS[$idx]}"

    # Gramática §8.2: todo lo que sigue al modelo va con --. Cada token se
    # valida CONTRA el TOML: nivel/flag/opción sin declarar → exit 2 con la
    # lista de lo que sí acepta (un typo no se ignora en silencio).
    local tok clave nivel=""
    local -a mflags=() mopts=()
    for tok in "$@"; do
        case "$tok" in
            --*) ;;
            *)   rp_usage_error args "se esperaba --<nivel>|--<flag>|--<opt>=<v> (recibido: '$tok')" ;;
        esac
        tok="${tok#--}"
        case "$tok" in
            *=*)
                clave="${tok%%=*}"
                case " $opciones " in
                    *" $clave "*) mopts+=("$tok") ;;
                    *) rp_usage_error args "'--$clave' no es una opción de '$modelo' (declaradas: ${opciones:-(ninguna)})" ;;
                esac
                ;;
            *)
                case " $niveles " in
                    *" $tok "*)
                        # Nivel repetido: gana el último, con aviso (como --port).
                        [[ -n "$nivel" && "$nivel" != "$tok" ]] && \
                            rp_msg warn "nivel repetido: '$nivel' → '$tok' (gana el último)"
                        nivel="$tok"
                        ;;
                    *)
                        case " $flags " in
                            *" $tok "*) mflags+=("$tok") ;;
                            *) rp_usage_error args "'--$tok' no está declarado para '$modelo' (niveles: ${niveles:-(ninguno)} · flags: ${flags:-(ninguno)} · opciones: ${opciones:-(ninguna)})" ;;
                        esac
                        ;;
                esac
                ;;
        esac
    done

    rp_model_argv "$modelo" ${nivel:+"$nivel"} ${mflags[@]+"${mflags[@]}"} ${mopts[@]+"${mopts[@]}"}
    return $?
}

# -----------------------------------------------------------------------------
# levels <modelo> — un nivel por línea (§8.5). Sin niveles → salida vacía,
# exit 0. El plugin parsea ^[A-Za-z0-9._-]+$ por línea: CUALQUIER otra cosa
# por stdout rompe el autocompletado, así que aquí no hay cabeceras.
# -----------------------------------------------------------------------------
_rp_catalog_levels_help() {
    cat <<'EOF'
USO: ropero levels <modelo>

Imprime los niveles de razonamiento que el modelo declara ([levels].list),
uno por línea. Un modelo sin niveles imprime línea vacía (nada) y sale 0:
"no tiene niveles" no es un error, es un hecho.
EOF
}

rp_cmd_levels() {
    local modelo="" a
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_catalog_levels_help; return 0 ;;
        esac
    done
    [[ $# -ge 1 && -n "$1" ]] || rp_usage_error levels "falta el modelo (ropero levels <modelo>)"
    modelo="$1"
    shift
    [[ $# -eq 0 ]] || rp_usage_error levels "levels no acepta más argumentos (recibido: '$1')"

    rp_mc_load || return 1
    rp_model_exists "$modelo" || \
        rp_usage_error levels "modelo desconocido: '$modelo' (ropero list)"

    local niveles lv
    niveles="$(rp_model_field "$modelo" levels)"
    # Word-splitting SEGURO aquí: los niveles son tokens [A-Za-z0-9._-]+.
    for lv in $niveles; do
        printf '%s\n' "$lv"
    done
    return 0
}

# -----------------------------------------------------------------------------
# path <modelo> — las rutas resueltas que usará el arranque (§8.6), en líneas
# CLAVE=ruta: el gguf, los auxiliares RUN, el checkpoint, el binario del
# engine y el log. Es lo que consulta el gguffer para dejar el GGUF donde el
# TOML lo espera.
# -----------------------------------------------------------------------------
_rp_catalog_path_help() {
    cat <<'EOF'
USO: ropero path <modelo>

Imprime las rutas resueltas del modelo, una por línea en forma CLAVE=ruta:

  GGUF=          $GGUF_DIR/<modelo>/<[files].gguf>
  MTP=           fichero MTP (decoding especulativo), si está declarado
  MMPROJ=        proyector de visión, si está declarado
  CHAT_TEMPLATE= plantilla Jinja del bundle de descarga, si está declarada
  MODEL_PATH=    checkpoint ($SAFETENSORS_DIR/<[model].path>) o TAG de flm
  ENGINE=        binario del engine (resolve de bin/lib/03-engines.sh)
  LOG=           log de arranque ($LOG_DIR/<modelo>.log)
EOF
}

rp_cmd_path() {
    local modelo="" a
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_catalog_path_help; return 0 ;;
        esac
    done
    [[ $# -ge 1 && -n "$1" ]] || rp_usage_error path "falta el modelo (ropero path <modelo>)"
    modelo="$1"
    shift
    [[ $# -eq 0 ]] || rp_usage_error path "path no acepta más argumentos (recibido: '$1')"

    rp_mc_load || return 1
    rp_model_exists "$modelo" || \
        rp_usage_error path "modelo desconocido: '$modelo' (ropero list)"

    local idx
    idx=$(_rp_mc_idx "$modelo")
    # Las rutas van por el nombre REAL del modelo: si llegara un alias, su
    # directorio en $GGUF_DIR es el del modelo al que apunta.
    local real="${MODEL_REAL[$idx]}"
    local motor="${MODEL_ENGINE[$idx]}"
    local fmt="${MODEL_FORMAT[$idx]}"
    local mpath="${MODEL_PATH[$idx]}"

    if [[ -n "${MODEL_GGUF[$idx]}" ]]; then
        printf 'GGUF=%s\n' "$GGUF_DIR/$real/${MODEL_GGUF[$idx]}"
    fi
    if [[ -n "${MODEL_MTP[$idx]}" ]]; then
        printf 'MTP=%s\n' "$GGUF_DIR/$real/${MODEL_MTP[$idx]}"
    fi
    if [[ -n "${MODEL_MMPROJ[$idx]}" ]]; then
        printf 'MMPROJ=%s\n' "$GGUF_DIR/$real/${MODEL_MMPROJ[$idx]}"
    fi
    # chat_template: el dump lo trae como aux de descarga (repo|file|name|...).
    if [[ -n "${MODEL_AUX_CHAT[$idx]}" ]]; then
        local _ct_repo ct_file ct_name _ct_size _ct_use
        IFS='|' read -r _ct_repo ct_file ct_name _ct_size _ct_use <<<"${MODEL_AUX_CHAT[$idx]}"
        if [[ -n "$ct_file" ]]; then
            [[ -n "$ct_name" ]] || ct_name="$ct_file"
            printf 'CHAT_TEMPLATE=%s\n' "$GGUF_DIR/$real/$ct_name"
        fi
    fi
    # Checkpoint: safetensors → ruta bajo $SAFETENSORS_DIR; flm → el TAG tal
    # cual (lo resuelve el almacén de FLM, no el filesystem).
    if [[ -n "$mpath" ]]; then
        case "$fmt/$motor" in
            safetensors/*)
                case "$mpath" in
                    /*) printf 'MODEL_PATH=%s\n' "$mpath" ;;
                    *)  printf 'MODEL_PATH=%s\n' "$SAFETENSORS_DIR/$mpath" ;;
                esac
                ;;
            */fastflowlm)
                printf 'MODEL_PATH=%s\n' "$mpath"
                ;;
        esac
    fi
    # Binario del engine: sólo si el módulo de engines está (fase 3+). En
    # fases intermedias la línea simplemente no sale.
    if ! declare -F rp_engine_list >/dev/null 2>&1; then
        if [[ -f "${ROPERO_ROOT}/bin/lib/03-engines.sh" ]]; then
            # shellcheck source=/dev/null
            . "${ROPERO_ROOT}/bin/lib/03-engines.sh"
        fi
    fi
    if declare -F rp_engine_call >/dev/null 2>&1; then
        local bin
        if bin=$(rp_engine_call "$motor" resolve); then
            printf 'ENGINE=%s\n' "$bin"
        else
            rp_msg warn "engine '$motor' no resuelto (¿instalado?)"
        fi
    fi
    printf 'LOG=%s\n' "$LOG_DIR/$real.log"
    return 0
}
