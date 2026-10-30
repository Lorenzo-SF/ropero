#!/usr/bin/env bash
# =============================================================================
# cmd-models.sh — models / install models: baja o fabrica los modelos
# (spec §8.13, fase 5)
# =============================================================================
# `ropero models` = `ropero install models`. Dos caminos por modelo, decididos
# por su TOML:
#   · [download] → baja el artefacto (GGUF o checkpoint) + auxiliares use=run
#   · [build]    → fabrica el GGUF con el gguffer (UNA implementación, §7.3)
#
# Reglas de uso (§8.13):
#   · modelo con [download] Y [build], sin flag → "especifica --download o
#     --build", exit 2 (en menú interactivo se pregunta).
#   · flag que contradice al TOML → exit 2 ("'x' sólo tiene [download]").
#   · sin modelos: menú interactivo con tamaños; sin TTY → exit 2.
#   · [download].manual = no se ofrece en el menú (checkpoints enormes).
#
# ELEGIR un modelo ya es consentir su descarga: dentro del proceso se fija
# RP_YES=1 para las llamadas a 05-download (la pregunta del tamaño no se
# repite); ROPERO_FORCE sólo con --force.
#
# Los campos del dump que 02-modelconf no expone (repo/file/dest/excludes/
# manual/build) se releen aquí con awk, en vez de tocar la fase 2: una
# llamada puntual, no un hot path (mismo criterio que 04-launch.sh).
# Posiciones 1-based del dump v1: 13 repo · 14 file · 15 dest · 16 size ·
# 17 excludes · 28 manual · 31 build.
#
# bash 3.2: sin namerefs, sin arrays asociativos, sin mapfile (spec §3).
# Seguro de sourcear: sólo definiciones, cero ejecución.
# =============================================================================

if ! declare -F rp_mc_load >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/02-modelconf.sh"
fi
if ! declare -F rp_engine_call >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/03-engines.sh"
fi
if ! declare -F rp_download_gguf >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/05-download.sh"
fi

# Separador del dump de modelconf (0x1F), como constante para awk -F.
RP_MODELS_SEP=$'\x1f'

_rp_models_help() {
    cat <<'EOF'
USO: ropero models [modelo...] [--download|--build] [--force] [--dry-run]
     ropero install models ...   (idéntico)

Consigue los artefactos de los modelos: los que tienen [download] se bajan
de HuggingFace (GGUF o checkpoint + auxiliares); los que tienen [build] se
fabrican con el gguffer según la receta del TOML.

Sin modelos abre un MENÚ interactivo (con tamaños); sin TTY y sin selección
explícita es error de uso.

Opciones:
  --download   fuerza el camino de descarga ([download])
  --build      fuerza el de fabricación ([build])
  --force      re-descarga aunque el artefacto ya esté en disco
  --dry-run    sólo lista lo que haría, no toca disco

Un modelo con [download] Y [build] exige --download o --build (exit 2 sin
ellos). Un flag que contradice al TOML ("--build" en uno que sólo tiene
[download]") también es exit 2.

Exit: 0 · 1 fallo de descarga/fabricación · 2 uso.
EOF
}

# _rp_models_dump_field <nombre-real> <posición> — valor del campo del dump.
# rc 1 si el modelo no está. (Réplica local de _rp_launch_dump_field: fase 5
# no puede depender de 04-launch.sh, el módulo del runtime.)
_rp_models_dump_field() {
    local nombre="$1" pos="$2"
    local py
    py="$(_rp_mc_python)" || return 1
    "$py" "$RP_MODELCONF_PY" dump --root "$RP_MODELS_DIR" --include-fired 2>/dev/null \
        | awk -v fs="$RP_MODELS_SEP" -v objetivo="$nombre" -v pos="$pos" '
            BEGIN { FS = fs }
            $1 == objetivo { print $pos; encontrado = 1; exit }
            END { exit (encontrado ? 0 : 1) }'
}

# _rp_models_tiene_download <real> — 0 si el TOML declara artefacto
# principal descargable: [download].repo, y con format=gguf además .file
# (con [build], [download] puede existir sólo para los auxiliares).
_rp_models_tiene_download() {
    local real="$1"
    local repo file fmt
    repo="$(_rp_models_dump_field "$real" 13)" || return 1
    [[ -n "$repo" ]] || return 1
    fmt="$(rp_model_field "$real" format)" || return 1
    if [[ "$fmt" == "gguf" ]]; then
        file="$(_rp_models_dump_field "$real" 14)" || return 1
        [[ -n "$file" ]] || return 1
    fi
    return 0
}

# _rp_models_tiene_build <real> — 0 si el TOML declara [build] (receta).
_rp_models_tiene_build() {
    local b
    b="$(_rp_models_dump_field "$1" 31)" || return 1
    [[ -n "$b" ]]
}

# _rp_models_es_manual <real> — 0 si [download].manual = true.
_rp_models_es_manual() {
    local m
    m="$(_rp_models_dump_field "$1" 28)" || return 1
    [[ "$m" == "1" ]]
}

# _rp_models_engine_ok <real> — 0 si el engine del modelo es compatible con
# este SO/hw (no se ofrece lo que no va a arrancar).
_rp_models_engine_ok() {
    local motor
    motor="$(rp_engine_for_model "$1" 2>/dev/null)" || return 1
    rp_engine_call "$motor" compatible >/dev/null 2>&1
}

# -----------------------------------------------------------------------------
# Menú interactivo (con TTY). Imprime la lista humana a stderr y los pares
# "número|real" por stdout para que el caller los parsee.
# -----------------------------------------------------------------------------
_rp_models_menu() {
    rp_header "modelos instalables"
    local i real fmt size caminos
    local n=0
    for (( i = 0; i < ${#MODEL_NAMES[@]}; i++ )); do
        [[ "${MODEL_ISALIAS[$i]}" == "0" ]] || continue
        [[ "${MODEL_ORIGIN[$i]}" == "active" ]] || continue
        real="${MODEL_NAMES[$i]}"
        _rp_models_es_manual "$real" && continue          # manual: fuera del menú
        _rp_models_engine_ok "$real" || continue           # engine incompatible
        caminos=""
        _rp_models_tiene_download "$real" && caminos="download"
        _rp_models_tiene_build "$real" && caminos="${caminos:+$caminos+}build"
        [[ -n "$caminos" ]] || continue                    # nada que instalar
        fmt="${MODEL_ENGINE[$i]}"
        size="${MODEL_SIZE[$i]}"
        [[ -n "$size" ]] || size="-"
        n=$((n + 1))
        printf '  %2d) %-16s · %-13s · %-10s · [%s]\n' "$n" "$real" "$fmt" "$size" "$caminos" >&2
        printf '%d|%s\n' "$n" "$real"
    done
    if (( n == 0 )); then
        rp_msg warn "no hay modelos instalables en este SO (con [download] o [build])"
    else
        rp_msg info "selección: números o nombres separados por espacio · all · 0 para salir"
    fi
    return 0
}

# _rp_models_menu_seleccion — pregunta hasta obtener una selección válida.
# Deja los reales elegidos en stdout, uno por línea. Exit 0 con salida vacía
# si el usuario sale (0).
_rp_models_menu_seleccion() {
    local menu
    menu="$(_rp_models_menu)"
    if [[ -z "$menu" ]]; then
        return 0
    fi

    local -a nums=() reales=()
    local par n r
    while IFS= read -r par; do
        [[ "$par" == *"|"* ]] || continue
        n="${par%%|*}"
        r="${par#*|}"
        nums+=("$n")
        reales+=("$r")
    done <<<"$menu"

    local sel token idx valido invalido real
    while :; do
        printf 'Selecciona: ' >&2
        read -r sel || sel="0"
        [[ "$sel" == "0" ]] && return 0
        valido=0 invalido=0
        local -a elegidos=()
        for token in $sel; do
            case "$token" in
                all)
                    valido=1
                    continue ;;
                0) return 0 ;;
            esac
            real=""
            if [[ "$token" =~ ^[0-9]+$ ]]; then
                if (( token >= 1 && token <= ${#nums[@]} )); then
                    real="${reales[$((token - 1))]}"
                fi
            else
                local idx2
                if idx2="$(_rp_mc_idx "$token" 2>/dev/null)"; then
                    real="${MODEL_REAL[$idx2]}"
                fi
            fi
            if [[ -n "$real" ]]; then
                # Aunque venga por nombre, tiene que ser instalable y
                # compatible: el menú no ofrece lo demás.
                if _rp_models_engine_ok "$real" && ! _rp_models_es_manual "$real"; then
                    elegidos+=("$real")
                    valido=1
                else
                    invalido=1
                fi
            else
                invalido=1
            fi
        done
        if (( valido == 1 && invalido == 0 )); then
            # De-duplicar conservando el orden.
            local -a unicos=() e
            for e in ${elegidos[@]+"${elegidos[@]}"}; do
                case " ${unicos[*]+"${unicos[*]}"} " in
                    *" $e "*) ;;
                    *) unicos+=("$e") ;;
                esac
            done
            printf '%s\n' ${unicos[@]+"${unicos[@]}"}
            return 0
        fi
        rp_msg warn "selección no válida o no seleccionable; prueba otra vez (0 para salir)"
    done
}

# -----------------------------------------------------------------------------
# Auxiliares: bajar los del filtro pedido (use=run | use=build). Van a
# $GGUF_DIR/<modelo>/<name>, que es donde [args] los referencia (${mtp},
# ${mmproj}, ${chat_template}) y donde los resuelve `ropero path`.
# -----------------------------------------------------------------------------
_rp_models_aux() {
    local real="$1" filtro="$2" dry="$3" force="$4"
    local linea tipo a_repo a_file a_name a_size a_use
    while IFS='|' read -r tipo a_repo a_file a_name a_size a_use; do
        [[ -n "$a_file" ]] || continue
        [[ "$a_use" == "$filtro" ]] || continue
        if [[ "$dry" == "1" ]]; then
            rp_msg info "  auxiliar [$tipo] $a_name (~${a_size:-?}) → $GGUF_DIR/$real/$a_name"
            continue
        fi
        RP_YES=1 ROPERO_FORCE="$force" \
            rp_download_gguf "$a_repo" "$a_file" "$a_name" "$a_size" \
                "$GGUF_DIR/$real" "$a_name" || return 1
    done < <(rp_model_aux "$real" 2>/dev/null)
    return 0
}

# -----------------------------------------------------------------------------
# Camino download: artefacto principal + auxiliares use=run.
# -----------------------------------------------------------------------------
_rp_models_download() {
    local real="$1" idx="$2" dry="$3" force="$4"
    local repo file dest size excludes fmt mpath dest_dir
    repo="$(_rp_models_dump_field "$real" 13)" || return 1
    file="$(_rp_models_dump_field "$real" 14)" || file=""
    dest="$(_rp_models_dump_field "$real" 15)" || dest=""
    size="$(_rp_models_dump_field "$real" 16)" || size=""
    excludes="$(_rp_models_dump_field "$real" 17)" || excludes=""
    fmt="${MODEL_FORMAT[$idx]}"

    if [[ "$dry" == "1" ]]; then
        if [[ "$fmt" == "gguf" ]]; then
            [[ -n "$dest" ]] || dest="$file"
            rp_msg info "descargaría $file (~${size:-?}) → $GGUF_DIR/$real/$dest"
        else
            mpath="${MODEL_PATH[$idx]}"
            case "$mpath" in
                /*) dest_dir="$mpath" ;;
                *)  dest_dir="$SAFETENSORS_DIR/$mpath" ;;
            esac
            rp_msg info "descargaría el checkpoint $repo (~${size:-?}) → $dest_dir"
        fi
        _rp_models_aux "$real" run 1 0
        return 0
    fi

    if [[ "$fmt" == "gguf" ]]; then
        # consentimiento: la selección ya lo dio (RP_YES=1 en el prefijo)
        RP_YES=1 ROPERO_FORCE="$force" \
            rp_download_gguf "$repo" "$file" "$real" "$size" \
                "$GGUF_DIR/$real" "$dest" || return 1
    else
        mpath="${MODEL_PATH[$idx]}"
        case "$mpath" in
            /*) dest_dir="$mpath" ;;
            *)  dest_dir="$SAFETENSORS_DIR/$mpath" ;;
        esac
        # Del dump llegan con comas; 05-download quiere espacios.
        local excl="${excludes//,/ }"
        RP_YES=1 ROPERO_FORCE="$force" \
            rp_download_safetensors "$repo" "$dest_dir" "$real" "$size" "$excl" || return 1
    fi

    # Auxiliares RUN: los que el arranque referencia (${mtp}, ${mmproj}).
    # Los use=build (imatrix, tokenizer) son cosa del gguffer.
    _rp_models_aux "$real" run 0 "$force" || return 1
    return 0
}

# -----------------------------------------------------------------------------
# Camino build: auxiliares use=build ANTES, gguffer con la receta, y los
# use=run DESPUÉS. DESVIACIÓN documentada del legado (que bajaba TODOS los
# auxiliares en ambos caminos): los use=build sólo sirven para fabricar.
# -----------------------------------------------------------------------------
_rp_models_build() {
    local real="$1" dry="$2" force="$3"

    _rp_models_aux "$real" build "$dry" "$force" || return 1

    # El destino lo pone la instalación, no el TOML: el GGUF final vive en
    # $GGUF_DIR/<modelo>/ con el nombre de [files].gguf (el launcher busca
    # justo ahí). Sin este --out el gguffer escribiría en la RAÍZ de
    # GGUF_DIR y `ropero use` no encontraría el artefacto.
    #
    # Limitación conocida: con varios quants, --out no puede ser un fichero
    # (el gguffer lo rechaza pidiendo un directorio). Ningún modelo actual
    # usa varios; si alguno lo hace, se verá en el error del gguffer.
    local destino=""
    local gguf
    if gguf="$(rp_model_field "$real" gguf 2>/dev/null)" && [[ -n "$gguf" ]]; then
        destino="$GGUF_DIR/$real/$gguf"
    fi
    local -a out_args=()
    [[ -n "$destino" ]] && out_args=(--out "$destino")

    if [[ "$dry" == "1" ]]; then
        local out rc=0
        out="$(rp_model_build_argv "$real" ${out_args[@]+"${out_args[@]}"} 2>/dev/null)" || rc=$?
        if (( rc == 0 )); then
            local force_txt=""
            # --force se añade al argv del gguffer, no al del modelconf: el
            # subcomando `build` no tiene esa opción (es del [build] TOML o
            # de la instalación).
            [[ "$force" == "1" ]] && force_txt="--force"
            rp_msg info "fabricaría '$real' con el gguffer: ropero gguffer $(printf '%s ' $out)${force_txt:+$force_txt }"
        else
            rp_msg info "fabricaría '$real' con el gguffer (la receta no se pudo serializar)"
        fi
        return 0
    fi

    # El gguffer es FASE 6: sin él, el camino build es un error de USO claro
    # (no un fallo silencioso ni una instalación a medias).
    if [[ ! -f "${ROPERO_ROOT}/bin/lib/cmd-gguffer.sh" ]]; then
        rp_usage_error models "el gguffer aún no está portado (fase 6): no se puede fabricar '$real'"
    fi
    # shellcheck source=/dev/null
    . "${ROPERO_ROOT}/bin/lib/cmd-gguffer.sh"
    if ! declare -F rp_cmd_gguffer >/dev/null 2>&1; then
        rp_usage_error models "bin/lib/cmd-gguffer.sh no define rp_cmd_gguffer (fase 6)"
    fi

    local out rc=0
    out="$(rp_model_build_argv "$real" ${out_args[@]+"${out_args[@]}"} 2>&1)" || rc=$?
    if (( rc != 0 )); then
        rp_msg error "la receta [build] de '$real' no es válida:"
        printf '%s\n' "$out" >&2
        return 1
    fi
    # El argv llega UN ARGUMENTO POR LÍNEA: se consume línea a línea, sin
    # word-splitting (spec §3).
    local -a ggargv=()
    local linea
    while IFS= read -r linea; do
        [[ -n "$linea" ]] || continue
        ggargv+=("$linea")
    done <<<"$out"
    # --force del install tiene que llegar al gguffer: sin esto reutilizaría
    # el GGUF ya fabricado y el usuario no podría rehacerlo (el legacy sí lo
    # hacía, borrando el destino antes de fabricar).
    [[ "$force" == "1" ]] && ggargv+=(--force)
    rp_cmd_gguffer ${ggargv[@]+"${ggargv[@]}"} || return 1

    _rp_models_aux "$real" run 0 "$force" || return 1
    return 0
}

# -----------------------------------------------------------------------------
# rp_cmd_models — entrypoint (models / install models)
# -----------------------------------------------------------------------------
rp_cmd_models() {
    local a camino="" dry=0 force=0
    local -a pedidos=()
    for a in "$@"; do
        case "$a" in
            --help|-h) _rp_models_help; return 0 ;;
            --download) [[ -z "$camino" || "$camino" == "download" ]] || \
                            rp_usage_error models "especifica --download O --build, no ambos"
                        camino="download" ;;
            --build)    [[ -z "$camino" || "$camino" == "build" ]] || \
                            rp_usage_error models "especifica --download O --build, no ambos"
                        camino="build" ;;
            --force|-f) force=1 ;;
            --dry-run)  dry=1 ;;
            --*)        rp_usage_error models "opción desconocida: '$a'" ;;
            *)          pedidos+=("$a") ;;
        esac
    done

    rp_mc_load || return 1
    rp_env_setup || return 1

    local -a reales=()
    local interactivo=0
    if (( ${#pedidos[@]} == 0 )); then
        # Sin selección no hay install desatendido (§8.13): sin TTY, exit 2.
        if [[ ! -t 0 ]]; then
            rp_usage_error models "sin TTY no hay selección: indica el modelo(s) (ropero models <modelo>...)"
        fi
        interactivo=1
        local sel
        sel="$(_rp_models_menu_seleccion)"
        [[ -n "$sel" ]] || { rp_msg info "nada seleccionado"; return 0; }
        local linea
        while IFS= read -r linea; do
            [[ -n "$linea" ]] && reales+=("$linea")
        done <<<"$sel"
    else
        local m idx real
        for m in ${pedidos[@]+"${pedidos[@]}"}; do
            idx="$(_rp_mc_idx "$m" 2>/dev/null)" || \
                rp_usage_error models "modelo desconocido: '$m' (ropero list)"
            real="${MODEL_REAL[$idx]}"
            if [[ "${MODEL_ORIGIN[$idx]}" == "fired" ]]; then
                rp_usage_error models "'$real' está retirado (models/*/fired/): no se puede instalar"
            fi
            # De-duplicar (alias y nombre piden lo mismo).
            case " ${reales[*]+"${reales[*]}"} " in
                *" $real "*) ;;
                *) reales+=("$real") ;;
            esac
        done
    fi
    (( ${#reales[@]} > 0 )) || { rp_msg info "nada seleccionado"; return 0; }

    local rc=0 ok=0 fallos=0
    local real idx camino_modelo
    for real in ${reales[@]+"${reales[@]}"}; do
        idx="$(_rp_mc_idx "$real")"
        rp_header "modelo: $real"

        local tiene_dl=0 tiene_bld=0
        _rp_models_tiene_download "$real" && tiene_dl=1
        _rp_models_tiene_build "$real" && tiene_bld=1
        if (( tiene_dl == 0 && tiene_bld == 0 )); then
            rp_usage_error models "'$real' no tiene [download] ni [build]: no es instalable (su artefacto se consigue a mano)"
        fi

        # Camino: el global, el que diga el TOML, o pregunta/error.
        camino_modelo="$camino"
        if [[ -z "$camino_modelo" ]]; then
            if (( tiene_dl == 1 && tiene_bld == 1 )); then
                if [[ "$interactivo" == "1" ]]; then
                    local resp=""
                    printf '¿descargar o fabricar %s? [d/b]: ' "$real" >&2
                    read -r resp || resp=""
                    case "$resp" in
                        d|D|descargar) camino_modelo="download" ;;
                        b|B|fabricar)  camino_modelo="build" ;;
                        *) rp_msg info "'$real' omitido por el usuario"; continue ;;
                    esac
                else
                    rp_usage_error models "'$real' tiene [download] y [build]: especifica --download o --build"
                fi
            elif (( tiene_dl == 1 )); then
                camino_modelo="download"
            else
                camino_modelo="build"
            fi
        else
            # El flag explícito tiene que ser un camino que el TOML declare.
            if [[ "$camino_modelo" == "build" && "$tiene_bld" != "1" ]]; then
                rp_usage_error models "'$real' sólo tiene [download] (no se puede --build)"
            fi
            if [[ "$camino_modelo" == "download" && "$tiene_dl" != "1" ]]; then
                rp_usage_error models "'$real' sólo tiene [build] (no se puede --download)"
            fi
        fi

        # manual: no se ofrece en el menú, pero una petición explícita es
        # consciente del tamaño. Se avisa y adelante.
        if _rp_models_es_manual "$real" && [[ "$camino_modelo" == "download" ]]; then
            rp_msg warn "'$real' es de descarga manual (grande): se baja igual por petición explícita"
        fi

        if [[ "$camino_modelo" == "download" ]]; then
            if _rp_models_download "$real" "$idx" "$dry" "$force"; then
                ok=$((ok + 1))
            else
                fallos=$((fallos + 1)); rc=1
            fi
        else
            if _rp_models_build "$real" "$dry" "$force"; then
                ok=$((ok + 1))
            else
                fallos=$((fallos + 1)); rc=1
            fi
        fi
    done

    if [[ "$dry" == "1" ]]; then
        rp_msg ok "dry-run: $ok modelo(s) planificado(s), $fallos fallo(s) (no se tocó disco)"
    else
        rp_msg ok "models: $ok modelo(s) listo(s), $fallos fallo(s)"
    fi
    return "$rc"
}
